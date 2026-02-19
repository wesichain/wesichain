use wesichain_agent::{AgentRuntime, Idle, NoopPolicy};

fn main() {
    let _ = AgentRuntime::<(), (), NoopPolicy, Idle>::default();
}
