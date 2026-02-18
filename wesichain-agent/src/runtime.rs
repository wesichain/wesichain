use crate::phase::{Acting, Completed, Idle, Observing, Thinking};

pub struct AgentRuntime<S, T, P, Phase> {
    _marker: std::marker::PhantomData<(S, T, P, Phase)>,
}

impl<S, T, P, Phase> Default for AgentRuntime<S, T, P, Phase> {
    fn default() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

impl<S, T, P> AgentRuntime<S, T, P, Idle> {
    pub fn think(self) -> AgentRuntime<S, T, P, Thinking> {
        AgentRuntime::default()
    }
}

impl<S, T, P> AgentRuntime<S, T, P, Thinking> {
    pub fn act(self) -> AgentRuntime<S, T, P, Acting> {
        AgentRuntime::default()
    }

    pub fn complete(self) -> AgentRuntime<S, T, P, Completed> {
        AgentRuntime::default()
    }
}

impl<S, T, P> AgentRuntime<S, T, P, Acting> {
    pub fn observe(self) -> AgentRuntime<S, T, P, Observing> {
        AgentRuntime::default()
    }
}
