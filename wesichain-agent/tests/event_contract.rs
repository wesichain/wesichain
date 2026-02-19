use wesichain_agent::{
    validate_completed_once, validate_step_started_precedes_terminal,
    validate_tool_dispatch_cardinality, AgentError, AgentEvent, AgentRuntime, LoopTransition,
    NoopPolicy, PolicyDecision, PolicyEngine,
};

struct RetryToolPolicy;

impl PolicyEngine for RetryToolPolicy {
    fn on_tool_error(_error: &AgentError) -> PolicyDecision {
        PolicyDecision::retry()
    }
}

#[test]
fn step_started_precedes_terminal_event_for_each_step() {
    let runtime = AgentRuntime::<(), (), NoopPolicy, _>::new().think();
    let response = wesichain_core::LlmResponse {
        content: "done".to_string(),
        tool_calls: Vec::new(),
    };

    let (_, events) = runtime
        .on_model_response_with_events(1, response, &[])
        .expect("runtime transition should succeed");

    validate_step_started_precedes_terminal(&events).unwrap();

    let first = events.first().expect("at least one event");
    assert!(matches!(first, AgentEvent::StepStarted { step_id: 1 }));
}

#[test]
fn each_tool_dispatched_has_exactly_one_completion_or_failure_counterpart() {
    let thinking = AgentRuntime::<(), (), RetryToolPolicy, _>::new().think();
    let response = wesichain_core::LlmResponse {
        content: "need tool".to_string(),
        tool_calls: vec![wesichain_core::ToolCall {
            id: "call-1".to_string(),
            name: "calculator".to_string(),
            args: wesichain_core::Value::Null,
        }],
    };

    let (transition, mut events) = thinking
        .on_model_response_with_events(1, response, &["calculator".to_string()])
        .expect("model response should transition to acting");

    let acting = match transition {
        LoopTransition::Acting(runtime) => runtime,
        _ => panic!("expected acting transition"),
    };

    let (_, failure_events) = acting
        .on_tool_error_with_events(1, AgentError::ToolDispatch)
        .expect("tool error should map to retry transition");
    events.extend(failure_events);

    validate_tool_dispatch_cardinality(&events).unwrap();
}

#[test]
fn completed_event_is_emitted_only_once() {
    let runtime = AgentRuntime::<(), (), NoopPolicy, _>::new().think();
    let response = wesichain_core::LlmResponse {
        content: "done".to_string(),
        tool_calls: Vec::new(),
    };

    let (_, events) = runtime
        .on_model_response_with_events(7, response, &[])
        .expect("runtime transition should succeed");

    validate_completed_once(&events).unwrap();
    let completed_count = events
        .iter()
        .filter(|event| matches!(event, AgentEvent::Completed { .. }))
        .count();
    assert_eq!(completed_count, 1, "runtime should emit Completed once");
}
