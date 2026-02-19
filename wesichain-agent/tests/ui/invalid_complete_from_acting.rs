use wesichain_agent::{AgentRuntime, Idle, NoopPolicy};

fn main() {
    let runtime = AgentRuntime::<(), (), NoopPolicy, Idle>::new();
    let acting = runtime.think().act();
    let _ = acting.complete();
}
