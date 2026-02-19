use wesichain_agent::{AgentRuntime, NoopPolicy, Thinking};

fn main() {
    let _ = AgentRuntime::<(), (), NoopPolicy, Thinking>::default();
}
