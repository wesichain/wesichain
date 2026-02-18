use wesichain_agent::{
    AgentError, AgentRuntime, Idle, LoopTransition, PolicyDecision, PolicyEngine, RepromptStrategy,
};
use wesichain_core::{LlmResponse, ToolCall, Value};

#[derive(Debug)]
struct AlwaysReprompt;

impl PolicyEngine for AlwaysReprompt {
    fn on_model_error(_error: &AgentError) -> PolicyDecision {
        PolicyDecision::reprompt(RepromptStrategy::OnceWithToolCatalog)
    }
}

#[test]
fn reprompt_consumes_budget_by_default_and_reaches_budget_exceeded() {
    let allowed_tools = vec!["calculator".to_string()];
    let invalid_response = LlmResponse {
        content: String::new(),
        tool_calls: vec![ToolCall {
            id: "call-1".to_string(),
            name: "weather_lookup".to_string(),
            args: Value::String("{\"city\":\"Berlin\"}".to_string()),
        }],
    };

    let runtime = AgentRuntime::<(), (), AlwaysReprompt, Idle>::with_budget(1).think();
    let runtime = match runtime.on_model_response(1, invalid_response.clone(), &allowed_tools) {
        Ok(LoopTransition::Thinking(runtime)) => runtime,
        other => panic!("expected reprompt back into thinking, got {other:?}"),
    };

    let second = runtime.on_model_response(2, invalid_response, &allowed_tools);
    match second {
        Err(AgentError::BudgetExceeded) => {}
        other => panic!("expected BudgetExceeded, got {other:?}"),
    }
}

#[test]
fn final_answer_transitions_to_completed_terminal_state() {
    let allowed_tools = vec!["calculator".to_string()];
    let response = LlmResponse {
        content: "42".to_string(),
        tool_calls: vec![],
    };

    let runtime = AgentRuntime::<(), (), AlwaysReprompt, Idle>::with_budget(2).think();
    let result = runtime.on_model_response(1, response, &allowed_tools);

    match result {
        Ok(LoopTransition::Completed(_runtime)) => {}
        other => panic!("expected completed transition, got {other:?}"),
    }
}
