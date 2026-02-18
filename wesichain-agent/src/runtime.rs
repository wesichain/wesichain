use crate::phase::{Acting, Completed, Idle, Observing, Thinking};
use crate::{validation, AgentError, ModelAction};

pub struct AgentRuntime<S, T, P, Phase> {
    _marker: std::marker::PhantomData<(S, T, P, Phase)>,
}

impl<S, T, P, Phase> AgentRuntime<S, T, P, Phase> {
    pub fn validate_model_action(
        step_id: u32,
        response: wesichain_core::LlmResponse,
        allowed_tools: &[String],
    ) -> Result<ModelAction, AgentError> {
        validation::validate_model_action(step_id, response, allowed_tools)
    }
}

impl<S, T, P> AgentRuntime<S, T, P, Idle> {
    pub fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }

    pub fn think(self) -> AgentRuntime<S, T, P, Thinking> {
        AgentRuntime {
            _marker: std::marker::PhantomData,
        }
    }
}

impl<S, T, P> Default for AgentRuntime<S, T, P, Idle> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S, T, P> AgentRuntime<S, T, P, Thinking> {
    pub fn act(self) -> AgentRuntime<S, T, P, Acting> {
        AgentRuntime {
            _marker: std::marker::PhantomData,
        }
    }

    pub fn complete(self) -> AgentRuntime<S, T, P, Completed> {
        AgentRuntime {
            _marker: std::marker::PhantomData,
        }
    }
}

impl<S, T, P> AgentRuntime<S, T, P, Acting> {
    pub fn observe(self) -> AgentRuntime<S, T, P, Observing> {
        AgentRuntime {
            _marker: std::marker::PhantomData,
        }
    }
}
