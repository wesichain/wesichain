use crate::phase::{Acting, Completed, Idle, Interrupted, Observing, Thinking};
use crate::{
    validation, AgentError, AgentEvent, ModelAction, PolicyDecision, PolicyEngine, RepromptStrategy,
};
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
pub enum ToolDispatchOutcome {
    Completed,
    Failed(AgentError),
}

pub fn emit_single_step_events(step_id: u32) -> Vec<AgentEvent> {
    vec![
        AgentEvent::StepStarted { step_id },
        AgentEvent::ModelResponded { step_id },
        AgentEvent::Completed { step_id },
    ]
}

pub fn emit_tool_step_events(step_id: u32, outcome: ToolDispatchOutcome) -> Vec<AgentEvent> {
    let mut events = vec![
        AgentEvent::StepStarted { step_id },
        AgentEvent::ModelResponded { step_id },
        AgentEvent::ToolDispatched { step_id },
    ];

    match outcome {
        ToolDispatchOutcome::Completed => events.push(AgentEvent::ToolCompleted { step_id }),
        ToolDispatchOutcome::Failed(error) => {
            events.push(AgentEvent::StepFailed { step_id, error })
        }
    }

    events
}

fn emit_tool_dispatch_events(step_id: u32) -> Vec<AgentEvent> {
    let mut events = emit_tool_step_events(step_id, ToolDispatchOutcome::Completed);
    let _ = events.pop();
    events
}

fn emit_tool_failure_event(step_id: u32, error: AgentError) -> AgentEvent {
    emit_tool_step_events(step_id, ToolDispatchOutcome::Failed(error))
        .pop()
        .expect("emit_tool_step_events always emits at least one event")
}

pub struct AgentRuntime<S, T, P, Phase> {
    remaining_budget: u32,
    _marker: std::marker::PhantomData<(S, T, P, Phase)>,
}

impl<S, T, P, Phase> std::fmt::Debug for AgentRuntime<S, T, P, Phase> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentRuntime")
            .field("remaining_budget", &self.remaining_budget)
            .finish()
    }
}

#[derive(Debug)]
pub enum LoopTransition<S, T, P> {
    Thinking {
        runtime: AgentRuntime<S, T, P, Thinking>,
        reprompt_strategy: Option<RepromptStrategy>,
    },
    Acting(AgentRuntime<S, T, P, Acting>),
    Observing(AgentRuntime<S, T, P, Observing>),
    Completed(AgentRuntime<S, T, P, Completed>),
    Interrupted(AgentRuntime<S, T, P, Interrupted>),
}

pub type TransitionWithEvents<S, T, P> = (LoopTransition<S, T, P>, Vec<AgentEvent>);

impl<S, T, P, Phase> AgentRuntime<S, T, P, Phase> {
    pub fn validate_model_action(
        step_id: u32,
        response: wesichain_core::LlmResponse,
        allowed_tools: &[String],
    ) -> Result<ModelAction, AgentError> {
        validation::validate_model_action(step_id, response, allowed_tools)
    }

    pub fn remaining_budget(&self) -> u32 {
        self.remaining_budget
    }

    pub fn map_model_transport_error(_error: wesichain_core::WesichainError) -> AgentError {
        AgentError::ModelTransport
    }

    fn is_cancelled(cancellation: &CancellationToken) -> bool {
        cancellation.is_cancelled()
    }

    fn transition<NextPhase>(self) -> AgentRuntime<S, T, P, NextPhase> {
        AgentRuntime {
            remaining_budget: self.remaining_budget,
            _marker: std::marker::PhantomData,
        }
    }

    fn consume_budget(mut self, consume: bool) -> Result<Self, AgentError> {
        if !consume {
            return Ok(self);
        }

        if self.remaining_budget == 0 {
            return Err(AgentError::BudgetExceeded);
        }

        self.remaining_budget -= 1;
        Ok(self)
    }
}

impl<S, T, P> AgentRuntime<S, T, P, Idle> {
    pub fn new() -> Self {
        Self::with_budget(u32::MAX)
    }

    pub fn with_budget(remaining_budget: u32) -> Self {
        Self {
            remaining_budget,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn think(self) -> AgentRuntime<S, T, P, Thinking> {
        self.transition()
    }

    pub fn think_if_not_cancelled(
        self,
        cancellation: &CancellationToken,
    ) -> LoopTransition<S, T, P> {
        if Self::is_cancelled(cancellation) {
            return LoopTransition::Interrupted(self.transition());
        }

        LoopTransition::Thinking {
            runtime: self.transition(),
            reprompt_strategy: None,
        }
    }
}

impl<S, T, P> Default for AgentRuntime<S, T, P, Idle> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S, T, P> AgentRuntime<S, T, P, Thinking>
where
    P: PolicyEngine,
{
    pub fn act(self) -> AgentRuntime<S, T, P, Acting> {
        self.transition()
    }

    pub fn complete(self) -> AgentRuntime<S, T, P, Completed> {
        self.transition()
    }

    pub fn interrupt(self) -> AgentRuntime<S, T, P, Interrupted> {
        self.transition()
    }

    pub fn on_model_response(
        self,
        step_id: u32,
        response: wesichain_core::LlmResponse,
        allowed_tools: &[String],
    ) -> Result<LoopTransition<S, T, P>, AgentError> {
        self.on_model_response_with_events(step_id, response, allowed_tools)
            .map(|(transition, _events)| transition)
    }

    pub fn on_model_response_with_events(
        self,
        step_id: u32,
        response: wesichain_core::LlmResponse,
        allowed_tools: &[String],
    ) -> Result<TransitionWithEvents<S, T, P>, AgentError> {
        match Self::validate_model_action(step_id, response, allowed_tools) {
            Ok(ModelAction::ToolCall { .. }) => Ok((
                LoopTransition::Acting(self.act()),
                emit_tool_dispatch_events(step_id),
            )),
            Ok(ModelAction::FinalAnswer { .. }) => Ok((
                LoopTransition::Completed(self.complete()),
                emit_single_step_events(step_id),
            )),
            Err(error) => self.on_model_error_with_events(error),
        }
    }

    pub fn on_model_response_with_events_if_not_cancelled(
        self,
        cancellation: &CancellationToken,
        step_id: u32,
        response: wesichain_core::LlmResponse,
        allowed_tools: &[String],
    ) -> Result<TransitionWithEvents<S, T, P>, AgentError> {
        if Self::is_cancelled(cancellation) {
            return Ok((LoopTransition::Interrupted(self.interrupt()), Vec::new()));
        }

        self.on_model_response_with_events(step_id, response, allowed_tools)
    }

    fn on_model_error_with_events(
        self,
        error: AgentError,
    ) -> Result<TransitionWithEvents<S, T, P>, AgentError> {
        let decision = P::on_model_error(&error);
        self.apply_policy_decision_with_events(error, decision, Vec::new())
    }

    fn apply_policy_decision_with_events(
        self,
        error: AgentError,
        decision: PolicyDecision,
        events: Vec<AgentEvent>,
    ) -> Result<TransitionWithEvents<S, T, P>, AgentError> {
        match decision {
            PolicyDecision::Fail => Err(error),
            PolicyDecision::Interrupt => {
                Ok((LoopTransition::Interrupted(self.interrupt()), events))
            }
            PolicyDecision::Retry { consume_budget } => {
                let runtime = self.consume_budget(consume_budget)?;
                Ok((
                    LoopTransition::Thinking {
                        runtime,
                        reprompt_strategy: None,
                    },
                    events,
                ))
            }
            PolicyDecision::Reprompt {
                strategy,
                consume_budget,
            } => {
                let runtime = self.consume_budget(consume_budget)?;
                Ok((
                    LoopTransition::Thinking {
                        runtime,
                        reprompt_strategy: Some(strategy),
                    },
                    events,
                ))
            }
        }
    }
}

impl<S, T, P> AgentRuntime<S, T, P, Acting>
where
    P: PolicyEngine,
{
    pub fn observe(self) -> AgentRuntime<S, T, P, Observing> {
        self.transition()
    }

    pub fn on_tool_success(self) -> AgentRuntime<S, T, P, Observing> {
        self.observe()
    }

    pub fn on_tool_success_with_events(self, step_id: u32) -> TransitionWithEvents<S, T, P> {
        (
            LoopTransition::Observing(self.observe()),
            vec![AgentEvent::ToolCompleted { step_id }],
        )
    }

    pub fn on_tool_success_with_events_if_not_cancelled(
        self,
        cancellation: &CancellationToken,
        step_id: u32,
    ) -> TransitionWithEvents<S, T, P> {
        if Self::is_cancelled(cancellation) {
            return (LoopTransition::Interrupted(self.interrupt()), Vec::new());
        }

        self.on_tool_success_with_events(step_id)
    }

    pub fn interrupt(self) -> AgentRuntime<S, T, P, Interrupted> {
        self.transition()
    }

    pub fn on_tool_error(self, error: AgentError) -> Result<LoopTransition<S, T, P>, AgentError> {
        self.on_tool_error_internal(None, error)
            .map(|(transition, _events)| transition)
    }

    pub fn on_tool_error_with_events(
        self,
        step_id: u32,
        error: AgentError,
    ) -> Result<TransitionWithEvents<S, T, P>, AgentError> {
        self.on_tool_error_internal(Some(step_id), error)
    }

    fn on_tool_error_internal(
        self,
        step_id: Option<u32>,
        error: AgentError,
    ) -> Result<TransitionWithEvents<S, T, P>, AgentError> {
        let decision = P::on_tool_error(&error);
        let mut events = Vec::new();
        if let Some(step_id) = step_id {
            events.push(emit_tool_failure_event(step_id, error.clone()));
        }

        match decision {
            PolicyDecision::Fail => Err(error),
            PolicyDecision::Interrupt => {
                Ok((LoopTransition::Interrupted(self.interrupt()), events))
            }
            PolicyDecision::Retry { consume_budget } => {
                let runtime = self.consume_budget(consume_budget)?;
                Ok((
                    LoopTransition::Thinking {
                        runtime: runtime.transition(),
                        reprompt_strategy: None,
                    },
                    events,
                ))
            }
            PolicyDecision::Reprompt {
                strategy,
                consume_budget,
            } => {
                let runtime = self.consume_budget(consume_budget)?;
                Ok((
                    LoopTransition::Thinking {
                        runtime: runtime.transition(),
                        reprompt_strategy: Some(strategy),
                    },
                    events,
                ))
            }
        }
    }
}

impl<S, T, P> AgentRuntime<S, T, P, Observing> {
    pub fn think(self) -> AgentRuntime<S, T, P, Thinking> {
        self.transition()
    }
}
