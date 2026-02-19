use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use wesichain_agent::{
    AgentError, AgentRuntime, Idle, LoopTransition, NoopPolicy, PolicyDecision, PolicyEngine,
};
use wesichain_core::{LlmResponse, ToolCall, Value};

#[derive(Debug)]
struct RepromptOnce;

impl PolicyEngine for RepromptOnce {
    fn on_model_error(_error: &AgentError) -> PolicyDecision {
        PolicyDecision::retry()
    }
}

fn final_answer_response() -> LlmResponse {
    LlmResponse {
        content: "done".to_string(),
        tool_calls: vec![],
    }
}

fn tool_call_response() -> LlmResponse {
    LlmResponse {
        content: String::new(),
        tool_calls: vec![ToolCall {
            id: "call-1".to_string(),
            name: "calculator".to_string(),
            args: Value::String("{\"expression\":\"2+2\"}".to_string()),
        }],
    }
}

fn malformed_tool_response() -> LlmResponse {
    LlmResponse {
        content: String::new(),
        tool_calls: vec![ToolCall {
            id: "call-1".to_string(),
            name: "unknown_tool".to_string(),
            args: Value::String("{}".to_string()),
        }],
    }
}

fn bench_runtime_profiles(c: &mut Criterion) {
    let allowed_tools = vec!["calculator".to_string()];

    let mut group = c.benchmark_group("agent_runtime_profiles");
    group.sample_size(10);

    group.bench_function("no_tool_short_answer", |b| {
        b.iter(|| {
            let runtime = AgentRuntime::<(), (), NoopPolicy, Idle>::with_budget(4).think();
            let transition = runtime
                .on_model_response(1, final_answer_response(), &allowed_tools)
                .expect("final answer transition should succeed");
            black_box(transition);
        })
    });

    group.bench_function("single_tool_hop", |b| {
        b.iter(|| {
            let thinking = AgentRuntime::<(), (), NoopPolicy, Idle>::with_budget(4).think();
            let acting = match thinking
                .on_model_response(1, tool_call_response(), &allowed_tools)
                .expect("tool call transition should succeed")
            {
                LoopTransition::Acting(runtime) => runtime,
                _ => panic!("expected acting transition"),
            };

            let observing = match acting.on_tool_success() {
                LoopTransition::Observing(runtime) => runtime,
                _ => panic!("expected observing transition"),
            };

            let transition = observing
                .think()
                .on_model_response(2, final_answer_response(), &allowed_tools)
                .expect("completion transition should succeed");
            black_box(transition);
        })
    });

    group.bench_function("multi_hop_chain", |b| {
        b.iter(|| {
            let mut thinking = AgentRuntime::<(), (), NoopPolicy, Idle>::with_budget(8).think();

            for step_id in 1..=3 {
                let acting = match thinking
                    .on_model_response(step_id, tool_call_response(), &allowed_tools)
                    .expect("tool call transition should succeed")
                {
                    LoopTransition::Acting(runtime) => runtime,
                    _ => panic!("expected acting transition"),
                };

                let observing = match acting.on_tool_success() {
                    LoopTransition::Observing(runtime) => runtime,
                    _ => panic!("expected observing transition"),
                };

                thinking = observing.think();
            }

            let transition = thinking
                .on_model_response(4, final_answer_response(), &allowed_tools)
                .expect("final transition should succeed");
            black_box(transition);
        })
    });

    group.bench_function("malformed_response_recovery", |b| {
        b.iter(|| {
            let thinking = AgentRuntime::<(), (), RepromptOnce, Idle>::with_budget(2).think();
            let transition = thinking
                .on_model_response(1, malformed_tool_response(), &allowed_tools)
                .expect("policy should reprompt on malformed model action");
            black_box(transition);
        })
    });

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_millis(300))
        .measurement_time(Duration::from_secs(2));
    targets = bench_runtime_profiles
}
criterion_main!(benches);
