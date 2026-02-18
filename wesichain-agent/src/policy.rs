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
