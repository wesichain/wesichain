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
        AgentEvent::ToolDispatched { step_id, tool_name: None },
    ];

    match outcome {
        ToolDispatchOutcome::Completed => {
            events.push(AgentEvent::ToolCompleted { step_id, tool_name: None, result: None })
        }
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
    cancellation: CancellationToken,
    _marker: std::marker::PhantomData<(S, T, P, Phase)>,
}

impl<S, T, P, Phase> std::fmt::Debug for AgentRuntime<S, T, P, Phase> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentRuntime")
            .field("remaining_budget", &self.remaining_budget)
            .field("cancelled", &self.cancellation.is_cancelled())
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

    fn map_model_transport_error(_error: wesichain_core::WesichainError) -> AgentError {
        AgentError::ModelTransport
    }

    fn cancellation_is_requested(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    fn transition<NextPhase>(self) -> AgentRuntime<S, T, P, NextPhase> {
        AgentRuntime {
            remaining_budget: self.remaining_budget,
            cancellation: self.cancellation,
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
        Self::with_budget_and_cancellation(remaining_budget, CancellationToken::new())
    }

    pub fn with_cancellation(cancellation: CancellationToken) -> Self {
        Self::with_budget_and_cancellation(u32::MAX, cancellation)
    }

    pub fn with_budget_and_cancellation(
        remaining_budget: u32,
        cancellation: CancellationToken,
    ) -> Self {
        Self {
            remaining_budget,
            cancellation,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn think(self) -> AgentRuntime<S, T, P, Thinking> {
        self.transition()
    }

    pub fn begin_thinking(self) -> LoopTransition<S, T, P> {
        if self.cancellation_is_requested() {
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
        if self.cancellation_is_requested() {
            return Ok((LoopTransition::Interrupted(self.interrupt()), Vec::new()));
        }

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

    pub fn on_model_transport_error(
        self,
        error: wesichain_core::WesichainError,
    ) -> Result<LoopTransition<S, T, P>, AgentError> {
        self.on_model_transport_error_with_events(error)
            .map(|(transition, _events)| transition)
    }

    pub fn on_model_transport_error_with_events(
        self,
        error: wesichain_core::WesichainError,
    ) -> Result<TransitionWithEvents<S, T, P>, AgentError> {
        if self.cancellation_is_requested() {
            return Ok((LoopTransition::Interrupted(self.interrupt()), Vec::new()));
        }

        self.on_model_error_with_events(Self::map_model_transport_error(error))
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

    pub fn on_tool_success(self) -> LoopTransition<S, T, P> {
        if self.cancellation_is_requested() {
            return LoopTransition::Interrupted(self.interrupt());
        }

        LoopTransition::Observing(self.observe())
    }

    pub fn on_tool_success_with_events(self, step_id: u32) -> TransitionWithEvents<S, T, P> {
        match self.on_tool_success() {
            LoopTransition::Observing(runtime) => (
                LoopTransition::Observing(runtime),
                vec![AgentEvent::ToolCompleted { step_id, tool_name: None, result: None }],
            ),
            LoopTransition::Interrupted(runtime) => (
                LoopTransition::Interrupted(runtime),
                vec![emit_tool_failure_event(
                    step_id,
                    AgentError::PolicyRuntimeViolation,
                )],
            ),
            _ => unreachable!("on_tool_success only returns Observing or Interrupted"),
        }
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

impl<S, T, P> AgentRuntime<S, T, P, Interrupted> {
    /// Capture a resumable checkpoint from the interrupted runtime.
    ///
    /// The caller supplies the current conversation `messages` and the
    /// `step_id` that was in flight at the time of interruption.  These are
    /// not stored inside `AgentRuntime` (the FSM is state-less with respect to
    /// conversation data) so the agent loop must pass them here.
    pub fn checkpoint(
        &self,
        messages: Vec<wesichain_core::Message>,
        step_id: u32,
    ) -> crate::checkpoint::AgentCheckpoint {
        crate::checkpoint::AgentCheckpoint::new(messages, step_id, self.remaining_budget)
    }
}

impl<S, T, P> AgentRuntime<S, T, P, Idle> {
    /// Reconstruct an `Idle` runtime from a previously saved checkpoint.
    ///
    /// Returns `(runtime, messages, step_id)` so the agent loop can restore
    /// the conversation context and resume at the correct step.
    pub fn resume_from(
        checkpoint: &crate::checkpoint::AgentCheckpoint,
    ) -> (Self, Vec<wesichain_core::Message>, u32) {
        let runtime = Self::with_budget(checkpoint.remaining_budget);
        (runtime, checkpoint.messages.clone(), checkpoint.step_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::{NoopPolicy, PolicyDecision, PolicyEngine, RepromptStrategy};
    use wesichain_core::{LlmResponse, ToolCall};

    // ---------------------------------------------------------------------------
    // Test helpers
    // ---------------------------------------------------------------------------

    type TestRuntime<Phase> = AgentRuntime<(), (), NoopPolicy, Phase>;

    fn final_answer_response(text: &str) -> LlmResponse {
        LlmResponse {
            content: text.to_string(),
            tool_calls: vec![],
            usage: None,
            model: String::new(),
        }
    }

    fn tool_call_response(tool_name: &str) -> LlmResponse {
        LlmResponse {
            content: String::new(),
            tool_calls: vec![ToolCall {
                id: "call_1".to_string(),
                name: tool_name.to_string(),
                args: serde_json::json!({ "query": "test" }),
            }],
            usage: None,
            model: String::new(),
        }
    }

    #[derive(Debug)]
    struct AlwaysRetryPolicy;
    impl PolicyEngine for AlwaysRetryPolicy {
        fn on_model_error(_: &crate::AgentError) -> PolicyDecision {
            PolicyDecision::Retry { consume_budget: true }
        }

        fn on_tool_error(_: &crate::AgentError) -> PolicyDecision {
            PolicyDecision::Retry { consume_budget: true }
        }
    }

    #[derive(Debug)]
    struct AlwaysRepromptPolicy;
    impl PolicyEngine for AlwaysRepromptPolicy {
        fn on_model_error(_: &crate::AgentError) -> PolicyDecision {
            PolicyDecision::reprompt(RepromptStrategy::OnceWithToolCatalog)
        }

        fn on_tool_error(_: &crate::AgentError) -> PolicyDecision {
            PolicyDecision::reprompt(RepromptStrategy::OnceWithToolCatalog)
        }
    }

    // ---------------------------------------------------------------------------
    // Idle → Thinking / Interrupted transitions
    // ---------------------------------------------------------------------------

    #[test]
    fn idle_begins_thinking() {
        let runtime: TestRuntime<Idle> = AgentRuntime::new();
        let transition = runtime.begin_thinking();
        assert!(matches!(
            transition,
            LoopTransition::Thinking {
                reprompt_strategy: None,
                ..
            }
        ));
    }

    #[test]
    fn idle_interrupted_when_cancelled() {
        let token = CancellationToken::new();
        token.cancel();
        let runtime: TestRuntime<Idle> = AgentRuntime::with_cancellation(token);
        let transition = runtime.begin_thinking();
        assert!(matches!(transition, LoopTransition::Interrupted(_)));
    }

    #[test]
    fn idle_think_returns_thinking_runtime() {
        let runtime: TestRuntime<Idle> = AgentRuntime::with_budget(5);
        let thinking = runtime.think();
        assert_eq!(thinking.remaining_budget(), 5);
    }

    // ---------------------------------------------------------------------------
    // Thinking → Completed (FinalAnswer)
    // ---------------------------------------------------------------------------

    #[test]
    fn thinking_completes_on_final_answer() {
        let runtime: TestRuntime<Idle> = AgentRuntime::new();
        let thinking = runtime.think();
        let response = final_answer_response("Done!");
        let transition = thinking
            .on_model_response(1, response, &[])
            .expect("should not error");
        assert!(matches!(transition, LoopTransition::Completed(_)));
    }

    #[test]
    fn thinking_completes_emits_events() {
        let runtime: TestRuntime<Idle> = AgentRuntime::new();
        let thinking = runtime.think();
        let response = final_answer_response("Done!");
        let (transition, events) = thinking
            .on_model_response_with_events(1, response, &[])
            .expect("should not error");
        assert!(matches!(transition, LoopTransition::Completed(_)));
        // Should emit StepStarted, ModelResponded, Completed
        assert_eq!(events.len(), 3);
    }

    // ---------------------------------------------------------------------------
    // Thinking → Acting (ToolCall)
    // ---------------------------------------------------------------------------

    #[test]
    fn thinking_acts_on_valid_tool_call() {
        let runtime: TestRuntime<Idle> = AgentRuntime::new();
        let thinking = runtime.think();
        let response = tool_call_response("search");
        let transition = thinking
            .on_model_response(1, response, &["search".to_string()])
            .expect("should not error");
        assert!(matches!(transition, LoopTransition::Acting(_)));
    }

    #[test]
    fn thinking_errors_on_unknown_tool() {
        let runtime: TestRuntime<Idle> = AgentRuntime::new();
        let thinking = runtime.think();
        let response = tool_call_response("unknown_tool");
        // NoopPolicy fails on model error
        let result = thinking.on_model_response(1, response, &["search".to_string()]);
        assert!(result.is_err());
    }

    // ---------------------------------------------------------------------------
    // Thinking → Interrupted (cancellation mid-flight)
    // ---------------------------------------------------------------------------

    #[test]
    fn thinking_interrupted_if_cancelled_before_response() {
        let token = CancellationToken::new();
        token.cancel();
        let runtime: AgentRuntime<(), (), NoopPolicy, Idle> =
            AgentRuntime::with_cancellation(token);
        let thinking = runtime.think();
        let response = final_answer_response("Done!");
        let (transition, _) = thinking
            .on_model_response_with_events(1, response, &[])
            .expect("should not error");
        assert!(matches!(transition, LoopTransition::Interrupted(_)));
    }

    // ---------------------------------------------------------------------------
    // Budget management
    // ---------------------------------------------------------------------------

    #[test]
    fn budget_decrements_on_retry() {
        let runtime: AgentRuntime<(), (), AlwaysRetryPolicy, Idle> =
            AgentRuntime::with_budget(3);
        let thinking = runtime.think();
        let response = tool_call_response("nonexistent");
        // AlwaysRetryPolicy retries and consumes budget
        let (transition, _) = thinking
            .on_model_response_with_events(1, response, &[])
            .expect("should not error");
        match transition {
            LoopTransition::Thinking { runtime, .. } => {
                assert_eq!(runtime.remaining_budget(), 2);
            }
            other => panic!("Expected Thinking, got {:?}", other),
        }
    }

    #[test]
    fn budget_exhaustion_returns_error() {
        let runtime: AgentRuntime<(), (), AlwaysRetryPolicy, Idle> =
            AgentRuntime::with_budget(1);
        let thinking = runtime.think();
        let response = tool_call_response("nonexistent");
        // With budget=1, retry consumes it → budget=0
        let (transition, _) = thinking
            .on_model_response_with_events(1, response, &[])
            .expect("should transition");
        match transition {
            LoopTransition::Thinking { runtime, .. } => {
                assert_eq!(runtime.remaining_budget(), 0);
                // Retry with exhausted budget should fail
                let response2 = tool_call_response("nonexistent");
                let result = runtime.on_model_response(2, response2, &[]);
                assert!(matches!(result, Err(crate::AgentError::BudgetExceeded)));
            }
            other => panic!("Expected Thinking, got {:?}", other),
        }
    }

    // ---------------------------------------------------------------------------
    // Acting → Observing (tool success)
    // ---------------------------------------------------------------------------

    #[test]
    fn acting_observes_on_tool_success() {
        let runtime: TestRuntime<Idle> = AgentRuntime::new();
        let thinking = runtime.think();
        let response = tool_call_response("search");
        let (transition, _) = thinking
            .on_model_response_with_events(1, response, &["search".to_string()])
            .expect("should not error");
        let acting = match transition {
            LoopTransition::Acting(r) => r,
            other => panic!("Expected Acting, got {:?}", other),
        };
        let next = acting.on_tool_success();
        assert!(matches!(next, LoopTransition::Observing(_)));
    }

    #[test]
    fn acting_tool_success_emits_completed_event() {
        let runtime: TestRuntime<Idle> = AgentRuntime::new();
        let thinking = runtime.think();
        let response = tool_call_response("search");
        let (transition, _) = thinking
            .on_model_response_with_events(1, response, &["search".to_string()])
            .unwrap();
        let acting = match transition {
            LoopTransition::Acting(r) => r,
            _ => panic!(),
        };
        let (next, events) = acting.on_tool_success_with_events(1);
        assert!(matches!(next, LoopTransition::Observing(_)));
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], crate::AgentEvent::ToolCompleted { .. }));
    }

    // ---------------------------------------------------------------------------
    // Acting → policy on tool error
    // ---------------------------------------------------------------------------

    #[test]
    fn acting_fails_on_tool_error_with_noop_policy() {
        let runtime: TestRuntime<Idle> = AgentRuntime::new();
        let thinking = runtime.think();
        let response = tool_call_response("search");
        let (transition, _) = thinking
            .on_model_response_with_events(1, response, &["search".to_string()])
            .unwrap();
        let acting = match transition {
            LoopTransition::Acting(r) => r,
            _ => panic!(),
        };
        let result = acting.on_tool_error(crate::AgentError::ToolDispatch);
        assert!(result.is_err());
    }

    #[test]
    fn acting_retries_on_tool_error_with_retry_policy() {
        let runtime: AgentRuntime<(), (), AlwaysRetryPolicy, Idle> = AgentRuntime::new();
        let thinking = runtime.think();
        let response = tool_call_response("search");
        let (transition, _) = thinking
            .on_model_response_with_events(1, response, &["search".to_string()])
            .unwrap();
        let acting = match transition {
            LoopTransition::Acting(r) => r,
            _ => panic!(),
        };
        let result = acting.on_tool_error(crate::AgentError::ToolDispatch);
        assert!(result.is_ok());
        assert!(matches!(
            result.unwrap(),
            LoopTransition::Thinking { reprompt_strategy: None, .. }
        ));
    }

    #[test]
    fn acting_reprompts_on_tool_error_with_reprompt_policy() {
        let runtime: AgentRuntime<(), (), AlwaysRepromptPolicy, Idle> = AgentRuntime::new();
        let thinking = runtime.think();
        let response = tool_call_response("search");
        let (transition, _) = thinking
            .on_model_response_with_events(1, response, &["search".to_string()])
            .unwrap();
        let acting = match transition {
            LoopTransition::Acting(r) => r,
            _ => panic!(),
        };
        let result = acting.on_tool_error(crate::AgentError::ToolDispatch);
        assert!(result.is_ok());
        assert!(matches!(
            result.unwrap(),
            LoopTransition::Thinking {
                reprompt_strategy: Some(_),
                ..
            }
        ));
    }

    // ---------------------------------------------------------------------------
    // Observing → Thinking
    // ---------------------------------------------------------------------------

    #[test]
    fn observing_returns_to_thinking() {
        let runtime: TestRuntime<Idle> = AgentRuntime::new();
        let thinking = runtime.think();
        let response = tool_call_response("search");
        let (transition, _) = thinking
            .on_model_response_with_events(1, response, &["search".to_string()])
            .unwrap();
        let acting = match transition {
            LoopTransition::Acting(r) => r,
            _ => panic!(),
        };
        let (observing_transition, _) = acting.on_tool_success_with_events(1);
        let observing = match observing_transition {
            LoopTransition::Observing(r) => r,
            _ => panic!(),
        };
        let next_thinking = observing.think();
        assert_eq!(next_thinking.remaining_budget(), u32::MAX);
    }
}
