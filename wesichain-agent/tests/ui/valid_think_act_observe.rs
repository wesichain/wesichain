use wesichain_agent::{AgentRuntime, Idle, NoopPolicy};

fn main() {
    let runtime = AgentRuntime::<(), (), NoopPolicy, Idle>::new();
    let _ = runtime.think().act().observe();
}
