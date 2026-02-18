mod error;
mod event;
mod policy;
mod state;

pub use error::AgentError;
pub use event::AgentEvent;
pub use policy::{NoopPolicy, PolicyDecision, RepromptStrategy};
pub use state::Idle;

pub struct AgentRuntime<S, T, P, Phase> {
    _marker: std::marker::PhantomData<(S, T, P, Phase)>,
}
