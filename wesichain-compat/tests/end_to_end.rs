use async_trait::async_trait;
use futures::StreamExt;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use wesichain_compat::LangChainRunnable;
use wesichain_core::Bindable;
use wesichain_core::{LlmRequest, LlmResponse, Runnable, Tool, ToolCallingLlm, WesichainError};
use wesichain_macros::tool;
use wesichain_prompt::{ChatPromptTemplate, MessagePromptTemplate};

// 1. Define a tool
#[tool(name = "calculator", description = "Adds two numbers")]
async fn add(a: i32, b: i32) -> Result<i32, String> {
    Ok(a + b)
}

// 2. Mock LLM
#[derive(Clone)]
struct MockLlm {
    last_request: Arc<Mutex<Option<LlmRequest>>>,
}

impl MockLlm {
    fn new() -> Self {
        Self {
            last_request: Arc::new(Mutex::new(None)),
        }
    }
}

#[async_trait]
impl Runnable<LlmRequest, LlmResponse> for MockLlm {
    async fn invoke(&self, request: LlmRequest) -> Result<LlmResponse, WesichainError> {
        let mut last = self.last_request.lock().await;
        *last = Some(request.clone());

        // Simple mock response
        Ok(LlmResponse {
            content: "Mock response".to_string(),
            tool_calls: vec![],
        })
    }

    fn stream(
        &self,
        input: LlmRequest,
    ) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        futures::stream::once(async move {
            let res = self.invoke(input).await?;
            Ok(wesichain_core::StreamEvent::Metadata {
                key: "result".to_string(),
                value: serde_json::to_value(res).unwrap(),
            })
        })
        .boxed()
    }
}

#[async_trait]
impl ToolCallingLlm for MockLlm {}

#[tokio::test]
async fn end_to_end_flow() {
    // 3. Create Prompt Template
    let prompt = ChatPromptTemplate::new(vec![
        MessagePromptTemplate::system("You are a helpful assistant."),
        MessagePromptTemplate::human("Please add {{a}} and {{b}}."),
    ]);

    // 4. Create Tool Definition (from macro generated struct)
    // Note: The macro generates a struct named based on function name. `fn add` -> `ADDTool`.
    let tool_instance = ADDTool;
    let tool_schema = tool_instance.schema();

    // 5. Prepare Mock LLM and Bind Tools
    // We bind the tool definition to the LLM.
    // In a real scenario, we bind "tools" -> list of tool definitions.
    let llm = MockLlm::new();

    // We need to bridge prompt output (Vec<Message>) to LLM input (LlmRequest).
    // prompt | llm_adapter | llm
    // For this test, we'll manually construct LlmRequest from prompt output for simplicity,
    // or assume we have a chain that does it.
    // Let's manually invoke for clarity and to simulate the "glue" code.

    let vars = HashMap::from([("a".to_string(), json!(10)), ("b".to_string(), json!(5))]);

    let messages = prompt.invoke(vars).await.unwrap();

    let mut request = LlmRequest {
        model: "mock-model".to_string(),
        messages,
        tools: vec![],
    };

    // Bind tools to the request (simulating llm.bind(tools))
    // We create a tool spec from the schema
    let tool_spec = json!({
        "tools": [{
            "name": tool_instance.name(),
            "description": tool_instance.description(),
            "parameters": tool_schema
        }]
    });

    request.bind(tool_spec).unwrap();

    // Invoke LLM
    let _response = Runnable::invoke(&llm, request).await.unwrap();

    // Verify LLM received the bound tool
    let last_req = llm.last_request.lock().await.clone().unwrap();
    assert_eq!(last_req.tools.len(), 1);
    assert_eq!(last_req.tools[0].name, "calculator");
    assert_eq!(last_req.messages.len(), 2);
    assert_eq!(last_req.messages[1].content, "Please add 10 and 5.");
}
