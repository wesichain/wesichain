use crate::Memory;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use wesichain_core::checkpoint::{Checkpoint, Checkpointer};
use wesichain_core::state::{GraphState, StateSchema};
use wesichain_core::WesichainError;
use wesichain_llm::{Message, Role};

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct CheckpointMemoryState {
    pub messages: Vec<Message>,
}

impl StateSchema for CheckpointMemoryState {
    type Update = Vec<Message>;

    fn apply(current: &Self, update: Self::Update) -> Self {
        let mut next = current.clone();
        next.messages.extend(update);
        next
    }
}

pub struct ConversationBufferMemory<C>
where
    C: Checkpointer<CheckpointMemoryState> + Send + Sync,
{
    human_prefix: String,
    ai_prefix: String,
    memory_key: String,
    checkpointer: Arc<C>,
}

impl<C> ConversationBufferMemory<C>
where
    C: Checkpointer<CheckpointMemoryState> + Send + Sync,
{
    pub fn new(checkpointer: Arc<C>) -> Self {
        Self {
            human_prefix: "Human".to_string(),
            ai_prefix: "AI".to_string(),
            memory_key: "history".to_string(),
            checkpointer,
        }
    }

    pub fn with_prefixes(mut self, human_prefix: impl Into<String>, ai_prefix: impl Into<String>) -> Self {
        self.human_prefix = human_prefix.into();
        self.ai_prefix = ai_prefix.into();
        self
    }

    pub fn with_memory_key(mut self, memory_key: impl Into<String>) -> Self {
        self.memory_key = memory_key.into();
        self
    }
}

#[async_trait]
impl<C> Memory for ConversationBufferMemory<C>
where
    C: Checkpointer<CheckpointMemoryState> + Send + Sync,
{
    async fn load_memory_variables(
        &self,
        thread_id: &str,
    ) -> Result<HashMap<String, Value>, WesichainError> {
        let checkpoint = self.checkpointer.load(thread_id).await?;
        let messages = match checkpoint {
            Some(cp) => cp.state.data.messages,
            None => Vec::new(),
        };

        // For buffer memory, we can return messages as a list of objects or a string.
        // Returning as a JSON value of messages array for now.
        let mut vars = HashMap::new();
        vars.insert(
            self.memory_key.clone(),
            serde_json::to_value(messages)?,
        );

        Ok(vars)
    }

    async fn save_context(
        &self,
        thread_id: &str,
        inputs: &HashMap<String, Value>,
        outputs: &HashMap<String, Value>,
    ) -> Result<(), WesichainError> {
        // Simple extraction: assumes "input" or "question" in inputs, "output" or "text" or "answer" in outputs
        let input_text = inputs
            .get("input")
            .or_else(|| inputs.get("question"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        
        let output_text = outputs
            .get("output")
            .or_else(|| outputs.get("text"))
            .or_else(|| outputs.get("answer"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let new_messages = vec![
            Message {
                role: Role::User,
                content: input_text.to_string(),
                tool_call_id: None,
                tool_calls: Vec::new(),
            },
            Message {
                role: Role::Assistant,
                content: output_text.to_string(),
                tool_call_id: None,
                tool_calls: Vec::new(),
            },
        ];

        // Load existing state
        let current_checkpoint = self.checkpointer.load(thread_id).await?;
        let (mut state, step) = match current_checkpoint {
            Some(cp) => (cp.state.data, cp.step),
            None => (CheckpointMemoryState::default(), 0),
        };

        // Update state
        state.messages.extend(new_messages);

        // Save new checkpoint
        let new_checkpoint = Checkpoint::new(
            thread_id.to_string(),
            wesichain_core::state::GraphState::new(state),
            step + 1,
            "memory".to_string(),
            Vec::new(),
        );

        self.checkpointer.save(&new_checkpoint).await?;

        Ok(())
    }

    async fn clear(&self, thread_id: &str) -> Result<(), WesichainError> {
         let new_checkpoint = Checkpoint::new(
            thread_id.to_string(),
            wesichain_core::state::GraphState::new(CheckpointMemoryState::default()),
            0, // Reset step for clear
            "memory".to_string(),
            Vec::new(),
        );
        self.checkpointer.save(&new_checkpoint).await?;
        Ok(())
    }
}
