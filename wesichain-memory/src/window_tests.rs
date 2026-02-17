#[cfg(test)]
mod tests {
    use crate::window::ConversationWindowMemory;
    use crate::Memory;
    use serde_json::Value;
    use std::collections::HashMap;
    use std::sync::Arc;
    use wesichain_core::checkpoint::InMemoryCheckpointer;
    use wesichain_llm::{Message, Role};

    #[tokio::test]
    async fn test_window_memory_basic_trimming() {
        let checkpointer = Arc::new(InMemoryCheckpointer::default());
        let memory = ConversationWindowMemory::new(checkpointer.clone(), 2);
        let thread_id = "test-window-1";

        // Save 1st turn
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), Value::String("One".to_string()));
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), Value::String("1".to_string()));
        memory.save_context(thread_id, &inputs, &outputs).await.unwrap();

        let vars = memory.load_memory_variables(thread_id).await.unwrap();
        let history = vars.get("history").unwrap();
        let messages: Vec<Message> = serde_json::from_value(history.clone()).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "One");

        // Save 2nd turn - should keep last 2 messages
        let mut inputs2 = HashMap::new();
        inputs2.insert("input".to_string(), Value::String("Two".to_string()));
        let mut outputs2 = HashMap::new();
        outputs2.insert("output".to_string(), Value::String("2".to_string()));
        memory.save_context(thread_id, &inputs2, &outputs2).await.unwrap();

        let vars = memory.load_memory_variables(thread_id).await.unwrap();
        let history = vars.get("history").unwrap();
        let messages: Vec<Message> = serde_json::from_value(history.clone()).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, Role::User);
        assert_eq!(messages[0].content, "Two");
        assert_eq!(messages[1].role, Role::Assistant);
        assert_eq!(messages[1].content, "2");
    }

    #[tokio::test]
    async fn test_window_boundary_exactly_n_messages() {
        let checkpointer = Arc::new(InMemoryCheckpointer::default());
        let memory = ConversationWindowMemory::new(checkpointer.clone(), 4);
        let thread_id = "test-boundary-n";

        // Add exactly 4 messages (2 turns)
        for i in 1..=2 {
            let mut inputs = HashMap::new();
            inputs.insert("input".to_string(), Value::String(format!("Q{}", i)));
            let mut outputs = HashMap::new();
            outputs.insert("output".to_string(), Value::String(format!("A{}", i)));
            memory.save_context(thread_id, &inputs, &outputs).await.unwrap();
        }

        // Should have exactly 4 messages
        let vars = memory.load_memory_variables(thread_id).await.unwrap();
        let messages: Vec<Message> = serde_json::from_value(vars.get("history").unwrap().clone()).unwrap();
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].content, "Q1");
        assert_eq!(messages[3].content, "A2");
    }

    #[tokio::test]
    async fn test_window_boundary_n_plus_one() {
        let checkpointer = Arc::new(InMemoryCheckpointer::default());
        let memory = ConversationWindowMemory::new(checkpointer.clone(), 4);
        let thread_id = "test-boundary-n-plus-1";

        // Add 5 messages (2.5 turns) - should trim oldest
        let mut inputs1 = HashMap::new();
        inputs1.insert("input".to_string(), Value::String("Q1".to_string()));
        let mut outputs1 = HashMap::new();
        outputs1.insert("output".to_string(), Value::String("A1".to_string()));
        memory.save_context(thread_id, &inputs1, &outputs1).await.unwrap();

        let mut inputs2 = HashMap::new();
        inputs2.insert("input".to_string(), Value::String("Q2".to_string()));
        let mut outputs2 = HashMap::new();
        outputs2.insert("output".to_string(), Value::String("A2".to_string()));
        memory.save_context(thread_id, &inputs2, &outputs2).await.unwrap();

        let mut inputs3 = HashMap::new();
        inputs3.insert("input".to_string(), Value::String("Q3".to_string()));
        let mut outputs3 = HashMap::new();
        outputs3.insert("output".to_string(), Value::String("A3".to_string()));
        memory.save_context(thread_id, &inputs3, &outputs3).await.unwrap();

        // Should have only last 4 messages (Q2, A2, Q3, A3)
        let vars = memory.load_memory_variables(thread_id).await.unwrap();
        let messages: Vec<Message> = serde_json::from_value(vars.get("history").unwrap().clone()).unwrap();
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].content, "Q2"); // Q1 and A1 dropped
        assert_eq!(messages[1].content, "A2");
        assert_eq!(messages[2].content, "Q3");
        assert_eq!(messages[3].content, "A3");
    }

    #[tokio::test]
    async fn test_window_reload_from_checkpointer() {
        let checkpointer = Arc::new(InMemoryCheckpointer::default());
        let thread_id = "test-reload";

        // First memory instance adds 3 turns
        {
            let memory1 = ConversationWindowMemory::new(checkpointer.clone(), 4);
            for i in 1..=3 {
                let mut inputs = HashMap::new();
                inputs.insert("input".to_string(), Value::String(format!("Q{}", i)));
                let mut outputs = HashMap::new();
                outputs.insert("output".to_string(), Value::String(format!("A{}", i)));
                memory1.save_context(thread_id, &inputs, &outputs).await.unwrap();
            }
        }

        // Second memory instance reloads - should get last 4 messages (Q2, A2, Q3, A3)
        {
            let memory2 = ConversationWindowMemory::new(checkpointer.clone(), 4);
            let vars = memory2.load_memory_variables(thread_id).await.unwrap();
            let messages: Vec<Message> = serde_json::from_value(vars.get("history").unwrap().clone()).unwrap();
            assert_eq!(messages.len(), 4);
            assert_eq!(messages[0].content, "Q2");
            assert_eq!(messages[3].content, "A3");

            // Add one more turn
            let mut inputs = HashMap::new();
            inputs.insert("input".to_string(), Value::String("Q4".to_string()));
            let mut outputs = HashMap::new();
            outputs.insert("output".to_string(), Value::String("A4".to_string()));
            memory2.save_context(thread_id, &inputs, &outputs).await.unwrap();
        }

        // Third memory instance should see last 4 (Q3, A3, Q4, A4)
        {
            let memory3 = ConversationWindowMemory::new(checkpointer.clone(), 4);
            let vars = memory3.load_memory_variables(thread_id).await.unwrap();
            let messages: Vec<Message> = serde_json::from_value(vars.get("history").unwrap().clone()).unwrap();
            assert_eq!(messages.len(), 4);
            assert_eq!(messages[0].content, "Q3"); // Q2, A2 dropped
            assert_eq!(messages[3].content, "A4");
        }
    }

    #[tokio::test]
    async fn test_window_clear_resets_correctly() {
        let checkpointer = Arc::new(InMemoryCheckpointer::default());
        let memory = ConversationWindowMemory::new(checkpointer.clone(), 4);
        let thread_id = "test-clear";

        // Add messages
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), Value::String("Hello".to_string()));
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), Value::String("Hi".to_string()));
        memory.save_context(thread_id, &inputs, &outputs).await.unwrap();

        // Clear
        memory.clear(thread_id).await.unwrap();

        // Should be empty
        let vars = memory.load_memory_variables(thread_id).await.unwrap();
        let messages: Vec<Message> = serde_json::from_value(vars.get("history").unwrap().clone()).unwrap();
        assert_eq!(messages.len(), 0);
    }
}
