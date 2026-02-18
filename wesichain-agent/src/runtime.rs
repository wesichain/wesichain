use crate::phase::{Acting, Completed, Idle, Interrupted, Observing, Thinking};
use crate::{validation, AgentError, ModelAction, PolicyDecision, PolicyEngine};

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
    Thinking(AgentRuntime<S, T, P, Thinking>),
    Acting(AgentRuntime<S, T, P, Acting>),
    Completed(AgentRuntime<S, T, P, Completed>),
    Interrupted(AgentRuntime<S, T, P, Interrupted>),
}

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
        match Self::validate_model_action(step_id, response, allowed_tools) {
            Ok(ModelAction::ToolCall { .. }) => Ok(LoopTransition::Acting(self.act())),
            Ok(ModelAction::FinalAnswer { .. }) => Ok(LoopTransition::Completed(self.complete())),
            Err(error) => self.on_model_error(error),
        }
    }

    fn on_model_error(self, error: AgentError) -> Result<LoopTransition<S, T, P>, AgentError> {
        let decision = P::on_model_error(&error);
        self.apply_policy_decision(error, decision)
    }

    fn apply_policy_decision(
        self,
        error: AgentError,
        decision: PolicyDecision,
    ) -> Result<LoopTransition<S, T, P>, AgentError> {
        match decision {
            PolicyDecision::Fail => Err(error),
            PolicyDecision::Interrupt => Ok(LoopTransition::Interrupted(self.interrupt())),
            PolicyDecision::Retry { consume_budget } => {
                let runtime = self.consume_budget(consume_budget)?;
                Ok(LoopTransition::Thinking(runtime))
            }
            PolicyDecision::Reprompt { consume_budget, .. } => {
                let runtime = self.consume_budget(consume_budget)?;
                Ok(LoopTransition::Thinking(runtime))
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

    pub fn interrupt(self) -> AgentRuntime<S, T, P, Interrupted> {
        self.transition()
    }

    pub fn on_tool_error(self, error: AgentError) -> Result<LoopTransition<S, T, P>, AgentError> {
        let decision = P::on_tool_error(&error);
        match decision {
            PolicyDecision::Fail => Err(error),
            PolicyDecision::Interrupt => Ok(LoopTransition::Interrupted(self.interrupt())),
            PolicyDecision::Retry { consume_budget }
            | PolicyDecision::Reprompt { consume_budget, .. } => {
                let runtime = self.consume_budget(consume_budget)?;
                Ok(LoopTransition::Thinking(runtime.transition()))
            }
        }
    }
}

impl<S, T, P> AgentRuntime<S, T, P, Observing> {
    pub fn think(self) -> AgentRuntime<S, T, P, Thinking> {
        self.transition()
    }
}
