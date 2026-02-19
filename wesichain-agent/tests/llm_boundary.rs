use wesichain_agent::{AgentError, AgentRuntime, Idle, NoopPolicy};
use wesichain_core::{LlmResponse, ToolCall, Value};

#[test]
fn runtime_rejects_multi_tool_call_shape_as_invalid_model_action() {
    let response = LlmResponse {
        content: String::new(),
        tool_calls: vec![
            ToolCall {
                id: "call-1".to_string(),
                name: "calculator".to_string(),
                args: Value::String("{\"expression\":\"2+2\"}".to_string()),
            },
            ToolCall {
                id: "call-2".to_string(),
                name: "weather_lookup".to_string(),
                args: Value::String("{\"city\":\"Berlin\"}".to_string()),
            },
        ],
    };

    let allowed_tools = vec!["calculator".to_string(), "weather_lookup".to_string()];
    let result = AgentRuntime::<(), (), NoopPolicy, Idle>::validate_model_action(
        11,
        response,
        &allowed_tools,
    );

    match result {
        Err(AgentError::InvalidModelAction {
            step_id,
            raw_response,
            ..
        }) => {
            assert_eq!(step_id, 11);
            assert!(raw_response.contains("tool_calls"));
            assert!(raw_response.contains("call-1"));
            assert!(raw_response.contains("call-2"));
        }
        _ => panic!("expected InvalidModelAction"),
    }
}

#[test]
fn runtime_maps_unknown_tool_to_invalid_model_action() {
    let response = LlmResponse {
        content: String::new(),
        tool_calls: vec![ToolCall {
            id: "call-1".to_string(),
            name: "weather_lookup".to_string(),
            args: Value::String("{\"city\":\"Berlin\"}".to_string()),
        }],
    };

    let allowed_tools = vec!["calculator".to_string()];
    let result = AgentRuntime::<(), (), NoopPolicy, Idle>::validate_model_action(
        7,
        response,
        &allowed_tools,
    );

    match result {
        Err(AgentError::InvalidModelAction {
            step_id,
            tool_name,
            received_args,
            raw_response,
        }) => {
            assert_eq!(step_id, 7);
            assert_eq!(tool_name.as_deref(), Some("weather_lookup"));
            assert!(received_args.contains("Berlin"));
            assert!(raw_response.contains("weather_lookup"));
        }
        _ => panic!("expected InvalidModelAction"),
    }
}
