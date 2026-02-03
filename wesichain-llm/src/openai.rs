use async_openai::config::OpenAIConfig;
use async_openai::types::{
    ChatCompletionMessageToolCall, ChatCompletionRequestAssistantMessage,
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
    ChatCompletionRequestToolMessage, ChatCompletionRequestUserMessage,
    ChatCompletionRequestUserMessageContent, ChatCompletionTool, ChatCompletionToolType,
    CreateChatCompletionRequestArgs, FunctionCall, FunctionObject, Role as OpenAiRole,
};
use async_openai::Client;

use wesichain_core::{
    LlmRequest, LlmResponse, Message, Role as CoreRole, ToolCall, ToolCallingLlm, ToolSpec,
    WesichainError,
};

#[derive(Clone)]
pub struct OpenAiClient {
    client: Client<OpenAIConfig>,
    model: String,
}

impl OpenAiClient {
    pub fn new(model: String) -> Self {
        Self {
            client: Client::new(),
            model,
        }
    }
}

fn map_tool_call(call: ToolCall) -> Result<ChatCompletionMessageToolCall, WesichainError> {
    let arguments = serde_json::to_string(&call.args)?;
    Ok(ChatCompletionMessageToolCall {
        id: call.id,
        r#type: ChatCompletionToolType::Function,
        function: FunctionCall {
            name: call.name,
            arguments,
        },
    })
}

fn map_message(message: Message) -> Result<ChatCompletionRequestMessage, WesichainError> {
    match message.role {
        CoreRole::System => Ok(ChatCompletionRequestMessage::System(
            ChatCompletionRequestSystemMessage {
                content: message.content,
                role: OpenAiRole::System,
                name: None,
            },
        )),
        CoreRole::User => Ok(ChatCompletionRequestMessage::User(
            ChatCompletionRequestUserMessage {
                content: ChatCompletionRequestUserMessageContent::Text(message.content),
                role: OpenAiRole::User,
                name: None,
            },
        )),
        CoreRole::Assistant => {
            let tool_calls = if message.tool_calls.is_empty() {
                None
            } else {
                Some(
                    message
                        .tool_calls
                        .into_iter()
                        .map(map_tool_call)
                        .collect::<Result<Vec<_>, _>>()?,
                )
            };
            let content = if message.content.is_empty() {
                None
            } else {
                Some(message.content)
            };
            Ok(ChatCompletionRequestMessage::Assistant(
                ChatCompletionRequestAssistantMessage {
                    content,
                    role: OpenAiRole::Assistant,
                    name: None,
                    tool_calls,
                    ..Default::default()
                },
            ))
        }
        CoreRole::Tool => {
            let tool_call_id = message.tool_call_id.ok_or_else(|| {
                WesichainError::InvalidConfig("tool message missing tool_call_id".to_string())
            })?;
            Ok(ChatCompletionRequestMessage::Tool(
                ChatCompletionRequestToolMessage {
                    role: OpenAiRole::Tool,
                    content: message.content,
                    tool_call_id,
                },
            ))
        }
    }
}

fn map_tool_spec(spec: ToolSpec) -> ChatCompletionTool {
    ChatCompletionTool {
        r#type: ChatCompletionToolType::Function,
        function: FunctionObject {
            name: spec.name,
            description: Some(spec.description),
            parameters: Some(spec.parameters),
        },
    }
}

#[async_trait::async_trait]
impl ToolCallingLlm for OpenAiClient {
    async fn invoke(&self, request: LlmRequest) -> Result<LlmResponse, WesichainError> {
        let messages = request
            .messages
            .into_iter()
            .map(map_message)
            .collect::<Result<Vec<_>, _>>()?;
        let tools = request
            .tools
            .into_iter()
            .map(map_tool_spec)
            .collect::<Vec<_>>();

        let mut builder = CreateChatCompletionRequestArgs::default();
        builder.model(&self.model);
        builder.messages(messages);
        if !tools.is_empty() {
            builder.tools(tools);
        }

        let response = self
            .client
            .chat()
            .create(
                builder
                    .build()
                    .map_err(|err| WesichainError::LlmProvider(err.to_string()))?,
            )
            .await
            .map_err(|err| WesichainError::LlmProvider(err.to_string()))?;

        let choice = response.choices.first().ok_or_else(|| {
            WesichainError::LlmProvider("no choices returned".to_string())
        })?;
        let content = choice.message.content.clone().unwrap_or_default();
        let tool_calls = choice
            .message
            .tool_calls
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|call| {
                let args = serde_json::from_str(&call.function.arguments)?;
                Ok(ToolCall {
                    id: call.id,
                    name: call.function.name,
                    args,
                })
            })
            .collect::<Result<Vec<_>, WesichainError>>()?;

        Ok(LlmResponse { content, tool_calls })
    }
}
