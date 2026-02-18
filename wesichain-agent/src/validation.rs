use crate::AgentError;

#[derive(Debug, Clone, PartialEq)]
pub enum ModelAction {
    FinalAnswer {
        content: String,
    },
    ToolCall {
        id: String,
        tool_name: String,
        args: wesichain_core::Value,
    },
}

pub fn validate_model_action(
    step_id: u32,
    response: wesichain_core::LlmResponse,
    allowed_tools: &[String],
) -> Result<ModelAction, AgentError> {
    let raw_response = format!("{response:?}");
    let tool_call_count = response.tool_calls.len();

    if tool_call_count > 1 {
        return Err(AgentError::InvalidModelAction {
            step_id,
            tool_name: None,
            received_args: format!("tool_calls_len={tool_call_count}"),
            raw_response,
        });
    }

    if let Some(tool_call) = response.tool_calls.into_iter().next() {
        if allowed_tools.iter().any(|name| name == &tool_call.name) {
            return Ok(ModelAction::ToolCall {
                id: tool_call.id,
                tool_name: tool_call.name,
                args: tool_call.args,
            });
        }

        return Err(AgentError::InvalidModelAction {
            step_id,
            tool_name: Some(tool_call.name),
            received_args: tool_call.args.to_string(),
            raw_response,
        });
    }

    Ok(ModelAction::FinalAnswer {
        content: response.content,
    })
}
