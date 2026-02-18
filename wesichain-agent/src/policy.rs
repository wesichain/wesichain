#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDecision {
    Fail,
    Retry {
        consume_budget: bool,
    },
    Reprompt {
        strategy: RepromptStrategy,
        consume_budget: bool,
    },
    Interrupt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepromptStrategy {
    OnceWithToolCatalog,
    N { n: u32 },
}

pub struct NoopPolicy;

impl PolicyDecision {
    pub fn retry() -> Self {
        Self::Retry {
            consume_budget: true,
        }
    }

    pub fn reprompt(strategy: RepromptStrategy) -> Self {
        Self::Reprompt {
            strategy,
            consume_budget: true,
        }
    }
}

pub trait PolicyEngine {
    fn on_model_error(_error: &crate::AgentError) -> PolicyDecision {
        PolicyDecision::Fail
    }

    fn on_tool_error(_error: &crate::AgentError) -> PolicyDecision {
        PolicyDecision::Fail
    }
}

impl PolicyEngine for NoopPolicy {}
