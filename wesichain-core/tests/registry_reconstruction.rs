use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::NamedTempFile;
use wesichain_core::{
    load_runnable, save_runnable, serde::SerializableRunnable, LlmRequest, LlmResponse, Message,
    Role, Runnable, RunnableRegistry, Tool, ToolError, Value, WesichainError,
};

// --- Mock LLM ---
#[derive(Clone, Serialize, Deserialize)]
struct MockLlm {
    model_name: String,
}

#[async_trait]
impl Runnable<LlmRequest, LlmResponse> for MockLlm {
    async fn invoke(&self, input: LlmRequest) -> Result<LlmResponse, WesichainError> {
        let last_msg = input
            .messages
            .last()
            .map(|m| m.content.clone())
            .unwrap_or_default();
        Ok(LlmResponse {
            content: format!("Mock answer from {} for: {}", self.model_name, last_msg),
            tool_calls: vec![],
        })
    }

    fn stream<'a>(
        &'a self,
        _input: LlmRequest,
    ) -> futures::stream::BoxStream<'a, Result<wesichain_core::StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }

    fn to_serializable(&self) -> Option<SerializableRunnable> {
        let mut params = HashMap::new();
        // We put "special_param" to verify it passes through
        params.insert(
            "special_param".to_string(),
            serde_json::to_value("test_val").unwrap(),
        );

        Some(SerializableRunnable::Llm {
            model: self.model_name.clone(),
            params,
        })
    }
}

// --- Mock Tool ---
#[derive(Clone, Serialize, Deserialize)]
struct MockTool {
    name: String,
}

#[async_trait]
impl Tool for MockTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "A mock tool"
    }

    fn schema(&self) -> Value {
        serde_json::json!({})
    }

    async fn invoke(&self, input: Value) -> Result<Value, ToolError> {
        let s = input.as_str().unwrap_or("");
        Ok(serde_json::Value::String(format!(
            "Tool {} processed: {}",
            self.name, s
        )))
    }
}

#[async_trait]
impl Runnable<Value, Value> for MockTool {
    async fn invoke(&self, input: Value) -> Result<Value, WesichainError> {
        Tool::invoke(self, input)
            .await
            .map_err(|e| WesichainError::Custom(e.to_string()))
    }

    fn stream<'a>(
        &'a self,
        _input: Value,
    ) -> futures::stream::BoxStream<'a, Result<wesichain_core::StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }

    fn to_serializable(&self) -> Option<SerializableRunnable> {
        Some(SerializableRunnable::Tool {
            name: self.name.clone(),
            description: Some(self.description().to_string()),
            schema: Some(self.schema()),
        })
    }
}

#[tokio::test]
async fn test_registry_chain_reconstruction() {
    // 1. Setup Registry
    let mut registry = RunnableRegistry::new();

    // Register LLM Factory
    registry.register_llm("mock-gpt", |params| {
        // Check params passed from serialization
        if let Some(val) = params.get("special_param") {
            assert_eq!(val.as_str(), Some("test_val"));
        }
        Ok(Arc::new(MockLlm {
            model_name: "mock-gpt".to_string(),
        }))
    });

    // Register Tool Factory
    registry.register_tool("mock_tool_1", |_config| {
        Ok(Arc::new(MockTool {
            name: "mock_tool_1".to_string(),
        }))
    });

    // 2. Create items to serialize
    let llm = MockLlm {
        model_name: "mock-gpt".to_string(),
    };

    // We want to test a Chain: Value -> [LlmAdapter] -> Value
    // But `MockLlm` is `LlmRequest -> LlmResponse`.
    // The `load_runnable` logic wraps LLMs in adapters automatically if we request `Runnable<Value, Value>`.

    // Let's test saving the LLM directly first.
    let file = NamedTempFile::new().unwrap();
    save_runnable(file.path(), &llm).unwrap();

    // Load as Runnable<Value, Value>
    let loaded_llm: Box<dyn Runnable<Value, Value>> =
        load_runnable(file.path(), Some(&registry)).unwrap();

    // Invoke loaded LLM with Value input
    let input_req = LlmRequest {
        model: "mock-gpt".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: "hi".to_string(),
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
    };
    let input = serde_json::to_value(&input_req).unwrap();
    let output = loaded_llm.invoke(input).await.unwrap();

    // Output should be LlmResponse serialized
    let resp: LlmResponse = serde_json::from_value(output).unwrap();
    assert_eq!(resp.content, "Mock answer from mock-gpt for: hi");

    // 3. Test Chain Reconstruction (Chain -> LLM -> StrOutputParser)
    use wesichain_core::{RunnableExt, StrOutputParser};
    let chain = llm.clone().then(StrOutputParser);
    let file_chain = NamedTempFile::new().unwrap();
    save_runnable(file_chain.path(), &chain).unwrap();

    // Load the chain
    let loaded_chain: Box<dyn Runnable<Value, Value>> =
        load_runnable(file_chain.path(), Some(&registry)).unwrap();

    // Invoke chain
    let input_chain = serde_json::to_value(input_req).unwrap();
    let output_chain = loaded_chain.invoke(input_chain).await.unwrap();

    let resp_str: String = serde_json::from_value(output_chain).unwrap();
    assert_eq!(resp_str, "Mock answer from mock-gpt for: hi");

    // 4. Test Parallel Reconstruction
    // We construct a parallel runnable manually or via extensions.
    // Let's use manual construction of SerializableRunnable::Parallel for control.
    use std::collections::HashMap;
    let mut steps_map = HashMap::new();
    steps_map.insert("branch1".to_string(), SerializableRunnable::Passthrough);
    // branch2 could be a tool? But tool needs registry.
    // Let's use a simpler parallel:
    // Parallel { branch1: Passthrough, branch2: Passthrough }
    steps_map.insert("branch2".to_string(), SerializableRunnable::Passthrough);

    let parallel_ser = SerializableRunnable::Parallel { steps: steps_map };

    // Test reconstruction
    let loaded_parallel =
        wesichain_core::reconstruct::<Value, Value>(parallel_ser, Some(&registry)).unwrap();
    let input_par = serde_json::json!("test_input");
    let output_par = loaded_parallel.invoke(input_par).await.unwrap();

    // Output should be a map { "branch1": "test_input", "branch2": "test_input" }
    let res_map: HashMap<String, Value> = serde_json::from_value(output_par).unwrap();
    assert_eq!(
        res_map.get("branch1").unwrap().as_str().unwrap(),
        "test_input"
    );
    assert_eq!(
        res_map.get("branch2").unwrap().as_str().unwrap(),
        "test_input"
    );

    // 5. Test Prompt Reconstruction via Registry
    // Register a prompt factory
    registry.register_prompt("default", |template, _vars| {
        // Simple prompt runnable that just replaces {{var}} with input?
        // Or simply returns the template string combined with input?
        // Let's make a dummy runnable that returns "Formatted: " + template + input
        struct MockPrompt(String);
        #[async_trait]
        impl Runnable<Value, Value> for MockPrompt {
            async fn invoke(&self, input: Value) -> Result<Value, WesichainError> {
                Ok(serde_json::Value::String(format!(
                    "Formatted: {} with {}",
                    self.0, input
                )))
            }
            fn stream<'a>(
                &'a self,
                input: Value,
            ) -> futures::stream::BoxStream<'a, Result<wesichain_core::StreamEvent, WesichainError>>
            {
                futures::stream::once(async move {
                    Ok(wesichain_core::StreamEvent::FinalAnswer(format!(
                        "Formatted: {} with {}",
                        self.0, input
                    )))
                })
                .boxed()
            }
        }
        Ok(Arc::new(MockPrompt(template)))
    });

    let prompt_ser = SerializableRunnable::Prompt {
        template: "Hello {{name}}".to_string(),
        input_variables: vec!["name".to_string()],
    };

    let loaded_prompt =
        wesichain_core::reconstruct::<Value, Value>(prompt_ser, Some(&registry)).unwrap();
    let output_prompt = loaded_prompt
        .invoke(serde_json::json!("World"))
        .await
        .unwrap();
    assert_eq!(
        output_prompt.as_str().unwrap(),
        "Formatted: Hello {{name}} with \"World\""
    );

    // 6. Test Tool Reconstruction
    let tool = MockTool {
        name: "mock_tool_1".to_string(),
    };
    let file_tool = NamedTempFile::new().unwrap();
    save_runnable(file_tool.path(), &tool).unwrap();

    // Load as Runnable<Value, Value>
    let loaded_tool: Box<dyn Runnable<Value, Value>> =
        load_runnable(file_tool.path(), Some(&registry)).unwrap();

    let input_tool = serde_json::Value::String("test args".to_string());

    let output_tool = loaded_tool.invoke(input_tool).await.unwrap();

    assert_eq!(
        output_tool.as_str().unwrap(),
        "Tool mock_tool_1 processed: test args"
    );
}
