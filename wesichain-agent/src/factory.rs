use crate::action::{ActionAgent, AgentAction, AgentFinish, AgentStep};
use async_trait::async_trait;
use futures::StreamExt;
use serde_json::json;
use wesichain_core::{LlmRequest, LlmResponse, Runnable, Tool, ToolCallingLlm, WesichainError};
use wesichain_prompt::ChatPromptTemplate;

/// A struct representing the tool calling agent runnable.
/// It wraps the chain: Prompt -> LLM -> OutputParser logic.
pub struct ToolCallingAgentRunnable {
    _prompt: ChatPromptTemplate,
    llm: Box<dyn Runnable<LlmRequest, LlmResponse> + Send + Sync>,
}

#[async_trait]
impl Runnable<LlmRequest, AgentStep> for ToolCallingAgentRunnable {
    async fn invoke(&self, input: LlmRequest) -> Result<AgentStep, WesichainError> {
        // 1. Format Messages (Input is LlmRequest which has messages... wait)
        // create_tool_calling_agent usually takes `input` as variables for prompt.
        // But `ActionAgent` takes `LlmRequest`.
        // If we use `LlmRequest` as the "State", then we don't need `ChatPromptTemplate` to format?
        // OR `ChatPromptTemplate` formats input variables -> Messages.

        // Let's assume input LlmRequest ALREADY has messages (history).
        // The `prompt` argument in `create_tool_calling_agent` usually formats the system message or structure.

        // Simplified flow for Wesichain migration:
        // Input (LlmRequest) -> LLM -> Output (LlmResponse) -> AgentStep

        // We need to bind tools to the LLM if not already bound.
        // But `llm` in struct is already bound? No, `create_tool_calling_agent` binds them.

        let response = self.llm.invoke(input).await?;

        // Parse response
        if let Some(tool_calls) = response.tool_calls.first() {
            Ok(AgentStep::Action(AgentAction {
                tool: tool_calls.name.clone(),
                tool_input: tool_calls.args.clone(),
                log: format!("Invoking tool {}", tool_calls.name),
            }))
        } else {
            Ok(AgentStep::Finish(AgentFinish {
                return_values: json!({ "output": response.content }),
                log: response.content,
            }))
        }
    }

    // Stream not implemented
    fn stream(
        &self,
        _input: LlmRequest,
    ) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}

#[async_trait]
impl ActionAgent for ToolCallingAgentRunnable {}

/// Create an agent that uses tool calling to take actions.
pub fn create_tool_calling_agent(
    llm: Box<dyn ToolCallingLlm>,
    tools: Vec<Box<dyn Tool>>,
    _prompt: ChatPromptTemplate, // Unused for now in this simplified version, or we assume it's pre-applied
) -> impl ActionAgent {
    // In a full implementation, we would bind tools to LLM here.
    // Since ToolCallingLlm trait doesn't expose `bind` directly (Bindable is on LlmRequest),
    // we assume the Caller or the `invoke` step handles binding.
    // BUT `bind` modifies `self` or returns new.
    // `ToolCallingLlm` describes the `invoke` capability. `Runnable` has `bind`.
    // If `llm` is `Box<dyn ToolCallingLlm>`, it might not implement `RunnableExt` methods easily without casting.

    // For this migration phase, we'll return a custom struct that handles the logic.
    // The binding of tools normally happens on the Request in Wesichain's current design (LlmRequest implements Bindable).
    // So the AgentRunnable needs to insert tools into LlmRequest before calling LLM.

    // Let's wrap the LLM.

    // We construct the tool specs.
    let tool_specs: Vec<_> = tools
        .iter()
        .map(|t| wesichain_core::ToolSpec {
            name: t.name().to_string(),
            description: t.description().to_string(),
            parameters: t.schema(),
        })
        .collect();

    // We need a wrapper that injects tools into the request.
    struct ToolBindingLlm {
        inner: Box<dyn ToolCallingLlm>,
        tools: Vec<wesichain_core::ToolSpec>,
    }

    #[async_trait]
    impl Runnable<LlmRequest, LlmResponse> for ToolBindingLlm {
        async fn invoke(&self, mut request: LlmRequest) -> Result<LlmResponse, WesichainError> {
            request.tools.extend(self.tools.clone());
            self.inner.invoke(request).await
        }

        fn stream(
            &self,
            mut input: LlmRequest,
        ) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>>
        {
            input.tools.extend(self.tools.clone());
            self.inner.stream(input)
        }
    }

    let bound_llm = ToolBindingLlm {
        inner: llm,
        tools: tool_specs,
    };

    ToolCallingAgentRunnable {
        _prompt,
        llm: Box::new(bound_llm),
    }
}
