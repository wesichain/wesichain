use async_trait::async_trait;
use futures::StreamExt;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use wesichain_core::{
    LlmRequest, LlmResponse, Runnable, RunnableExt, RunnableParallel, ToolCall, ToolCallingLlm,
    WesichainError,
};

// Mock Runnable
struct MockRunnable {
    response: Result<String, String>,
}

#[async_trait]
impl Runnable<String, String> for MockRunnable {
    async fn invoke(&self, _input: String) -> Result<String, WesichainError> {
        self.response.clone().map_err(|e| WesichainError::Custom(e))
    }

    fn stream(
        &self,
        _input: String,
    ) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}

#[tokio::test]
async fn test_runnable_parallel() {
    let step1 = Arc::new(MockRunnable {
        response: Ok("Step 1".to_string()),
    });
    let step2 = Arc::new(MockRunnable {
        response: Ok("Step 2".to_string()),
    });

    let mut map = BTreeMap::new();
    map.insert(
        "s1".to_string(),
        step1 as Arc<dyn Runnable<String, String> + Send + Sync>,
    );
    map.insert(
        "s2".to_string(),
        step2 as Arc<dyn Runnable<String, String> + Send + Sync>,
    );

    let parallel = RunnableParallel::new(map);
    let result = parallel.invoke("input".to_string()).await.unwrap();

    assert_eq!(result.get("s1").unwrap(), "Step 1");
    assert_eq!(result.get("s2").unwrap(), "Step 2");
}

#[tokio::test]
async fn test_runnable_fallbacks() {
    // We use the extension method, but we need to cast to trait object or implement Clone for MockRunnable?
    // RunnableExt::with_fallbacks takes self.
    // Our MockRunnable doesn't implement Clone (it has String result).
    // But we are using Arc in RunnableWithFallbacks constructor.
    // The definition of with_fallbacks expects `self` (Sized).
    // And it wraps `Arc::new(self)`.
    // So if we have `start_runnable`, we can call `.with_fallbacks(...)`.

    // However, MockRunnable logic is simple.
    // Let's create a struct that IS cloneable?
    // Or just use the struct directly.

    #[derive(Clone)]
    struct ClonableRunnable {
        response: Result<String, String>,
    }
    #[async_trait]
    impl Runnable<String, String> for ClonableRunnable {
        async fn invoke(&self, _input: String) -> Result<String, WesichainError> {
            self.response.clone().map_err(|e| WesichainError::Custom(e))
        }
        fn stream(
            &self,
            _input: String,
        ) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>>
        {
            futures::stream::empty().boxed()
        }
    }

    let primary = ClonableRunnable {
        response: Err("Primary failed".to_string()),
    };
    let fallback = ClonableRunnable {
        response: Ok("Fallback succeeded".to_string()),
    };

    // We need to pass fallbacks as Vec<Arc<dyn Runnable...>>
    let fallbacks: Vec<Arc<dyn Runnable<String, String> + Send + Sync>> = vec![Arc::new(fallback)];

    let runner = primary.with_fallbacks(fallbacks);
    let result = runner.invoke("input".to_string()).await.unwrap();

    assert_eq!(result, "Fallback succeeded");
}

// Mock LLM for structured output
#[derive(Clone)]
struct MockLlm {
    response: LlmResponse,
}

#[async_trait]
impl Runnable<LlmRequest, LlmResponse> for MockLlm {
    async fn invoke(&self, _request: LlmRequest) -> Result<LlmResponse, WesichainError> {
        Ok(self.response.clone())
    }

    fn stream(
        &self,
        _input: LlmRequest,
    ) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}

#[async_trait]
impl ToolCallingLlm for MockLlm {}

#[derive(Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
struct Person {
    name: String,
    age: u8,
}

#[tokio::test]
async fn test_with_structured_output() {
    use wesichain_core::ToolCallingLlmExt;

    let expected_person = Person {
        name: "Alice".to_string(),
        age: 30,
    };

    let response = LlmResponse {
        content: "".to_string(),
        tool_calls: vec![ToolCall {
            id: "call1".to_string(),
            name: "output_formatter".to_string(),
            args: json!({"name": "Alice", "age": 30}),
        }],
    };

    let llm = MockLlm { response };
    let chain = llm.with_structured_output::<Person>();

    let request = LlmRequest {
        model: "test".to_string(),
        messages: vec![],
        tools: vec![],
    };

    let result = chain.invoke(request).await.unwrap();
    assert_eq!(result, expected_person);
}
