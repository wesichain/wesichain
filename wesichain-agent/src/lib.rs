pub struct AgentRuntime<S, T, P, Phase> {
    _marker: std::marker::PhantomData<(S, T, P, Phase)>,
}

pub struct Idle;
pub struct NoopPolicy;
