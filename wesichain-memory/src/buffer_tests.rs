#[cfg(test)]
mod tests {
    use crate::buffer::ConversationBufferMemory;
    use crate::Memory;
    use serde_json::Value;
    use std::collections::HashMap;
    use std::sync::Arc;
    use wesichain_core::checkpoint::InMemoryCheckpointer;
    use wesichain_llm::{Message, Role};

    #[tokio::test]
    async fn test_buffer_memory_end_to_end() {
        let checkpointer = Arc::new(InMemoryCheckpointer::default());
        let memory = ConversationBufferMemory::new(checkpointer.clone());
        let thread_id = "test-thread-1";

        // 1. Initial load should be empty
        let vars = memory.load_memory_variables(thread_id).await.unwrap();
        let history = vars.get("history").unwrap();
        let messages: Vec<Message> = serde_json::from_value(history.clone()).unwrap();
        assert!(messages.is_empty());

        // 2. Save context (User input and AI output)
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), Value::String("Hello".to_string()));
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), Value::String("Hi there".to_string()));

        memory
            .save_context(thread_id, &inputs, &outputs)
            .await
            .unwrap();

        // 3. Load again and verify
        let vars = memory.load_memory_variables(thread_id).await.unwrap();
        let history = vars.get("history").unwrap();
        let messages: Vec<Message> = serde_json::from_value(history.clone()).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, Role::User);
        assert_eq!(messages[0].content, "Hello");
        assert_eq!(messages[1].role, Role::Assistant);
        assert_eq!(messages[1].content, "Hi there");

        // 4. Save another turn
        let mut inputs2 = HashMap::new();
        inputs2.insert(
            "input".to_string(),
            Value::String("How are you?".to_string()),
        );
        let mut outputs2 = HashMap::new();
        outputs2.insert("output".to_string(), Value::String("I'm good".to_string()));

        memory
            .save_context(thread_id, &inputs2, &outputs2)
            .await
            .unwrap();

        // 5. Verify order preservation
        let vars = memory.load_memory_variables(thread_id).await.unwrap();
        let history = vars.get("history").unwrap();
        let messages: Vec<Message> = serde_json::from_value(history.clone()).unwrap();
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[2].content, "How are you?");
        assert_eq!(messages[3].content, "I'm good");

        // 6. Verify thread isolation
        let thread_id_2 = "test-thread-2";
        let vars_2 = memory.load_memory_variables(thread_id_2).await.unwrap();
        let history_2 = vars_2.get("history").unwrap();
        let messages_2: Vec<Message> = serde_json::from_value(history_2.clone()).unwrap();
        assert!(messages_2.is_empty());
    }
}
