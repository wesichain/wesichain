use std::time::Duration;

use tokio_util::sync::CancellationToken;
use wesichain_agent::{AgentError, AgentEvent, AgentRuntime, Idle, LoopTransition, NoopPolicy};
use wesichain_core::{LlmResponse, ToolCall, Value, WesichainError};

#[test]
fn llm_transport_failure_maps_to_model_transport_error() {
    let result = AgentRuntime::<(), (), NoopPolicy, Idle>::new()
        .think()
        .on_model_transport_error(WesichainError::Timeout(Duration::from_millis(5)));

    assert!(matches!(result, Err(AgentError::ModelTransport)));
}

#[test]
fn cancellation_before_thinking_transitions_to_interrupted() {
    let cancellation = CancellationToken::new();
    cancellation.cancel();

    let transition =
        AgentRuntime::<(), (), NoopPolicy, Idle>::with_cancellation(cancellation).begin_thinking();

    assert!(matches!(transition, LoopTransition::Interrupted(_)));
}

#[test]
fn cancellation_before_tool_dispatch_transitions_to_interrupted() {
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let runtime = AgentRuntime::<(), (), NoopPolicy, Idle>::with_cancellation(cancellation).think();
    let response = LlmResponse {
        content: String::new(),
        tool_calls: vec![ToolCall {
            id: "call-1".to_string(),
            name: "calculator".to_string(),
            args: Value::String("{\"expression\":\"2+2\"}".to_string()),
        }],
    };
    let allowed_tools = vec!["calculator".to_string()];

    let (transition, events) = runtime
        .on_model_response_with_events(1, response, &allowed_tools)
        .expect("cancellation should return interrupted transition");

    assert!(matches!(transition, LoopTransition::Interrupted(_)));
    assert!(!events
        .iter()
        .any(|event| matches!(event, AgentEvent::ToolDispatched { .. })));
}

#[test]
fn cancellation_before_observing_append_transitions_to_interrupted() {
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let runtime = AgentRuntime::<(), (), NoopPolicy, Idle>::with_cancellation(cancellation)
        .think()
        .act();

    let (transition, events) = runtime.on_tool_success_with_events(11);

    assert!(matches!(transition, LoopTransition::Interrupted(_)));
    assert!(events
        .iter()
        .any(|event| matches!(event, AgentEvent::StepFailed { step_id: 11, .. })));
}
