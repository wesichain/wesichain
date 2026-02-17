#[cfg(test)]
mod tests {
    use crate::Memory;
    use std::collections::HashMap;
    use std::sync::Arc;
    use wesichain_core::checkpoint::InMemoryCheckpointer;
    use wesichain_core::{LlmRequest, LlmResponse, Runnable, StreamEvent};

    // Mock LLM for testing
    struct MockSummarizationLlm;

    #[async_trait::async_trait]
    impl Runnable<LlmRequest, LlmResponse> for MockSummarizationLlm {
        async fn invoke(&self, request: LlmRequest) -> Result<LlmResponse, wesichain_core::WesichainError> {
            // Extract conversation from the prompt
            let content = &request.messages[0].content;
            
            // Simple mock: just return a shorter version
            let summary = if content.contains("Current summary:") {
                "Summarized conversation content"
            } else {
                "Initial summary"
            };

            Ok(LlmResponse {
                content: summary.to_string(),
                tool_calls: Vec::new(),
            })
        }

        fn stream(
            &self,
            _input: LlmRequest,
        ) -> futures::stream::BoxStream<'static, Result<StreamEvent, wesichain_core::WesichainError>> {
            Box::pin(futures::stream::empty())
        }
    }

    #[tokio::test]
    async fn test_summary_memory_basic() {
        use crate::summary::SummaryMemoryState;
        
        let checkpointer = Arc::new(InMemoryCheckpointer::<SummaryMemoryState>::default());
        let llm = Arc::new(MockSummarizationLlm);
        let memory = crate::summary::ConversationSummaryMemory::new(checkpointer, llm)
            .with_buffer_size(2); // Small buffer to trigger summarization

        let thread_id = "test_thread";

        // First interaction
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), serde_json::json!("Hello"));
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), serde_json::json!("Hi there!"));

        memory.save_context(thread_id, &inputs, &outputs).await.unwrap();

        // Second interaction
        inputs.insert("input".to_string(), serde_json::json!("How are you?"));
        outputs.insert("output".to_string(), serde_json::json!("I'm good!"));
        memory.save_context(thread_id, &inputs, &outputs).await.unwrap();

        // Third interaction - should trigger summarization
        inputs.insert("input".to_string(), serde_json::json!("What's the weather?"));
        outputs.insert("output".to_string(), serde_json::json!("Sunny"));
        memory.save_context(thread_id, &inputs, &outputs).await.unwrap();

        let vars = memory.load_memory_variables(thread_id).await.unwrap();
        let history = vars.get("history").unwrap().as_str().unwrap();

        // Should have summary + recent buffer
        assert!(history.contains("Summary") || history.contains("Summarized"));
        assert!(history.contains("Recent conversation"));
    }

    #[tokio::test]
    async fn test_summary_memory_clear() {
        use crate::summary::SummaryMemoryState;
        
        let checkpointer = Arc::new(InMemoryCheckpointer::<SummaryMemoryState>::default());
        let llm = Arc::new(MockSummarizationLlm);
        let memory = crate::summary::ConversationSummaryMemory::new(checkpointer, llm);

        let thread_id = "test_clear";

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), serde_json::json!("Test"));
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), serde_json::json!("Response"));

        memory.save_context(thread_id, &inputs, &outputs).await.unwrap();
        memory.clear(thread_id).await.unwrap();

        let vars = memory.load_memory_variables(thread_id).await.unwrap();
        let history = vars.get("history").unwrap().as_str().unwrap();
        
        assert!(history.is_empty());
    }
}
