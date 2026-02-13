use crate::{LlmResponse, Runnable, StreamEvent, WesichainError};
use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::marker::PhantomData;

/// Trait for output parsers that can transform input into a specific output.
/// This is a specialized version of Runnable for parsing logic.
#[async_trait]
pub trait BaseOutputParser<Input: Send + Sync + 'static, Output: Send + Sync + 'static>:
    Runnable<Input, Output> + Send + Sync
{
    async fn parse(&self, input: Input) -> Result<Output, WesichainError>;
}

/// A parser that converts `LlmResponse` or `String` into a `String`.
/// If input is `LlmResponse`, it extracts the `content`.
#[derive(Clone, Default)]
pub struct StrOutputParser;

#[async_trait]
impl Runnable<LlmResponse, String> for StrOutputParser {
    async fn invoke(&self, input: LlmResponse) -> Result<String, WesichainError> {
        Ok(input.content)
    }

    fn stream(&self, input: LlmResponse) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        futures::stream::once(async move { Ok(StreamEvent::ContentChunk(input.content)) }).boxed()
    }

    fn to_serializable(&self) -> Option<crate::serde::SerializableRunnable> {
        Some(crate::serde::SerializableRunnable::Parser {
            kind: "str".to_string(),
            target_type: None,
        })
    }
}

#[async_trait]
impl Runnable<String, String> for StrOutputParser {
    async fn invoke(&self, input: String) -> Result<String, WesichainError> {
        Ok(input)
    }

    fn stream(&self, input: String) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        futures::stream::once(async move { Ok(StreamEvent::ContentChunk(input)) }).boxed()
    }
}

/// A parser that parses a JSON string (or LlmResponse content) into a structured type or Value.
#[derive(Clone, Default)]
pub struct JsonOutputParser<T = Value> {
    _marker: PhantomData<T>,
}

impl<T> JsonOutputParser<T> {
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

#[async_trait]
impl<T: DeserializeOwned + serde::Serialize + Send + Sync + 'static> Runnable<String, T>
    for JsonOutputParser<T>
{
    async fn invoke(&self, input: String) -> Result<T, WesichainError> {
        // Basic cleanup of markdown code blocks if present
        let cleaned = input.trim();
        let cleaned = if cleaned.starts_with("```json") {
            cleaned
                .trim_start_matches("```json")
                .trim_end_matches("```")
                .trim()
        } else if cleaned.starts_with("```") {
            cleaned
                .trim_start_matches("```")
                .trim_end_matches("```")
                .trim()
        } else {
            cleaned
        };

        serde_json::from_str(cleaned).map_err(WesichainError::Serde)
    }

    fn stream(&self, input: String) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        futures::stream::once(async move {
            let res = self.invoke(input).await?;
            Ok(StreamEvent::Metadata {
                key: "param".to_string(),
                value: serde_json::to_value(res).unwrap_or(Value::Null),
            })
        })
        .boxed()
    }

    fn to_serializable(&self) -> Option<crate::serde::SerializableRunnable> {
        Some(crate::serde::SerializableRunnable::Parser {
            kind: "json".to_string(),
            target_type: Some(std::any::type_name::<T>().to_string()),
        })
    }
}

#[async_trait]
impl<T: DeserializeOwned + serde::Serialize + Send + Sync + 'static> Runnable<LlmResponse, T>
    for JsonOutputParser<T>
{
    async fn invoke(&self, input: LlmResponse) -> Result<T, WesichainError> {
        // First check for JSON content
        // If that fails, or if empty, we might check tool calls?
        // But JsonOutputParser specifically targets JSON string content.
        // For structured output via tools, we need a different parser or logic.
        Runnable::<String, T>::invoke(self, input.content).await
    }

    fn stream(&self, input: LlmResponse) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        Runnable::<String, T>::stream(self, input.content)
    }
}

/// A parser that extracts structured output from `LlmResponse`.
/// It prioritizes `tool_calls` (first call args), then falls back to parsing `content` as JSON.
#[derive(Clone, Default)]
pub struct StructuredOutputParser<T = Value> {
    _marker: PhantomData<T>,
}

impl<T> StructuredOutputParser<T> {
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

#[async_trait]
impl<T: DeserializeOwned + serde::Serialize + Send + Sync + 'static> Runnable<LlmResponse, T>
    for StructuredOutputParser<T>
{
    async fn invoke(&self, input: LlmResponse) -> Result<T, WesichainError> {
        // 1. Check tool calls
        if let Some(call) = input.tool_calls.first() {
            // Args is expected to be Value, which we can deserialize to T
            return serde_json::from_value(call.args.clone()).map_err(WesichainError::Serde);
        }

        // 2. Fallback to content parsing (reuse logic from JsonOutputParser)
        let content = input.content.trim();
        let cleaned = if content.starts_with("```json") {
            content
                .trim_start_matches("```json")
                .trim_end_matches("```")
                .trim()
        } else if content.starts_with("```") {
            content
                .trim_start_matches("```")
                .trim_end_matches("```")
                .trim()
        } else {
            content
        };

        if cleaned.is_empty() {
            return Err(WesichainError::Custom(
                "No structured output found in tool calls or content".to_string(),
            ));
        }

        serde_json::from_str(cleaned).map_err(WesichainError::Serde)
    }

    fn stream(&self, _input: LlmResponse) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        // Structured parser hard to stream partial results unless we implement partial JSON parsing.
        // For now, empty stream or just wait for invoke?
        // Let's just return empty stream as we rely on invoke.
        futures::stream::empty().boxed()
    }

    fn to_serializable(&self) -> Option<crate::serde::SerializableRunnable> {
        Some(crate::serde::SerializableRunnable::Parser {
            kind: "structured".to_string(),
            target_type: Some(std::any::type_name::<T>().to_string()),
        })
    }
}
