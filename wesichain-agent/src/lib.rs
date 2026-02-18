mod error;
mod event;
mod phase;
mod policy;
mod state;

pub use error::AgentError;
pub use event::AgentEvent;
pub use phase::{Acting, Completed, Failed, Idle, Interrupted, Observing, Thinking};
pub use policy::{NoopPolicy, PolicyDecision, RepromptStrategy};
pub use state::AgentState;

pub struct AgentRuntime<S, T, P, Phase> {
    _marker: std::marker::PhantomData<(S, T, P, Phase)>,
}
