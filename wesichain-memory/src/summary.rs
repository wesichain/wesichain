use crate::Memory;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use wesichain_core::checkpoint::{Checkpoint, Checkpointer};
use wesichain_core::state::{GraphState, StateSchema};
use wesichain_core::{LlmRequest, LlmResponse, Runnable, WesichainError};
use wesichain_llm::{Message, Role};

const DEFAULT_SUMMARIZATION_PROMPT: &str = r#"Progressively summarize the lines of conversation provided, adding onto the previous summary returning a new summary.

Current summary:
{summary}

New lines of conversation:
{new_lines}

New summary:"#;

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct SummaryMemoryState {
    pub summary: String,
    pub buffer: Vec<Message>,
}

impl StateSchema for SummaryMemoryState {
    type Update = SummaryMemoryState;

    fn apply(_current: &Self, update: Self::Update) -> Self {
        update
    }
}

/// Memory that uses an LLM to progressively summarize conversation history.
///
/// This provides bounded memory regardless of conversation length.
/// Recent messages are kept in a buffer, and older messages are summarized.
pub struct ConversationSummaryMemory<C, L>
where
    C: Checkpointer<SummaryMemoryState> + Send + Sync,
    L: Runnable<LlmRequest, LlmResponse> + Send + Sync,
{
    checkpointer: Arc<C>,
    llm: Arc<L>,
    memory_key: String,
    buffer_size: usize,
    summarization_prompt: String,
}

impl<C, L> ConversationSummaryMemory<C, L>
where
    C: Checkpointer<SummaryMemoryState> + Send + Sync,
    L: Runnable<LlmRequest, LlmResponse> + Send + Sync,
{
    pub fn new(checkpointer: Arc<C>, llm: Arc<L>) -> Self {
        Self {
            checkpointer,
            llm,
            memory_key: "history".to_string(),
            buffer_size: 4,
            summarization_prompt: DEFAULT_SUMMARIZATION_PROMPT.to_string(),
        }
    }

    pub fn with_memory_key(mut self, memory_key: impl Into<String>) -> Self {
        self.memory_key = memory_key.into();
        self
    }

    pub fn with_buffer_size(mut self, buffer_size: usize) -> Self {
        self.buffer_size = buffer_size;
        self
    }

    pub fn with_summarization_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.summarization_prompt = prompt.into();
        self
    }

    async fn summarize(&self, current_summary: &str, new_messages: &[Message]) -> Result<String, WesichainError> {
        let new_lines = new_messages
            .iter()
            .map(|m| {
                let role = match m.role {
                    Role::User => "Human",
                    Role::Assistant => "AI",
                    _ => "System",
                };
                format!("{}: {}", role, m.content)
            })
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = self
            .summarization_prompt
            .replace("{summary}", current_summary)
            .replace("{new_lines}", &new_lines);

        let request = LlmRequest {
            model: String::new(),
            messages: vec![Message {
                role: Role::User,
                content: prompt,
                tool_call_id: None,
                tool_calls: Vec::new(),
            }],
            tools: Vec::new(),
        };

        let response = self.llm.invoke(request).await?;
        Ok(response.content.trim().to_string())
    }
}

#[async_trait]
impl<C, L> Memory for ConversationSummaryMemory<C, L>
where
    C: Checkpointer<SummaryMemoryState> + Send + Sync,
    L: Runnable<LlmRequest, LlmResponse> + Send + Sync,
{
    async fn load_memory_variables(
        &self,
        thread_id: &str,
    ) -> Result<HashMap<String, Value>, WesichainError> {
        let checkpoint = self.checkpointer.load(thread_id).await?;
        let state = match checkpoint {
            Some(cp) => cp.state.data,
            None => SummaryMemoryState::default(),
        };

        // Return summary + recent buffer messages
        let mut content_parts = Vec::new();
        
        if !state.summary.is_empty() {
            content_parts.push(format!("Summary of earlier conversation:\n{}", state.summary));
        }

        if !state.buffer.is_empty() {
            let recent = state
                .buffer
                .iter()
                .map(|m| {
                    let role = match m.role {
                        Role::User => "Human",
                        Role::Assistant => "AI",
                        _ => "System",
                    };
                    format!("{}: {}", role, m.content)
                })
                .collect::<Vec<_>>()
                .join("\n");
            content_parts.push(format!("Recent conversation:\n{}", recent));
        }

        let history = content_parts.join("\n\n");

        let mut vars = HashMap::new();
        vars.insert(self.memory_key.clone(), Value::String(history));
        Ok(vars)
    }

    async fn save_context(
        &self,
        thread_id: &str,
        inputs: &HashMap<String, Value>,
        outputs: &HashMap<String, Value>,
    ) -> Result<(), WesichainError> {
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

        // Load current state
        let current_checkpoint = self.checkpointer.load(thread_id).await?;
        let (mut state, step) = match current_checkpoint {
            Some(cp) => (cp.state.data, cp.step),
            None => (SummaryMemoryState::default(), 0),
        };

        // Add new messages to buffer
        state.buffer.extend(new_messages);

        // If buffer exceeds size, summarize oldest messages
        if state.buffer.len() > self.buffer_size {
            let to_summarize = state.buffer.len() - self.buffer_size;
            let messages_to_summarize: Vec<Message> = state.buffer.drain(..to_summarize).collect();
            
            state.summary = self.summarize(&state.summary, &messages_to_summarize).await?;
        }

        // Save new checkpoint
        let new_checkpoint = Checkpoint::new(
            thread_id.to_string(),
            GraphState::new(state),
            step + 1,
            "summary_memory".to_string(),
            Vec::new(),
        );

        self.checkpointer.save(&new_checkpoint).await?;
        Ok(())
    }

    async fn clear(&self, thread_id: &str) -> Result<(), WesichainError> {
        let new_checkpoint = Checkpoint::new(
            thread_id.to_string(),
            GraphState::new(SummaryMemoryState::default()),
            0,
            "summary_memory".to_string(),
            Vec::new(),
        );
        self.checkpointer.save(&new_checkpoint).await?;
        Ok(())
    }
}
