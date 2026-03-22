//! SSE helpers: convert a `BoxStream<StreamEvent>` into an Axum SSE response.

use axum::{
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
};
use futures::stream::BoxStream;
use futures::StreamExt;
use serde_json::json;
use wesichain_core::{StreamEvent, WesichainError};

/// Convert a stream of [`StreamEvent`]s into an Axum SSE response.
///
/// Event mapping:
/// - `ContentChunk` → `{"type":"chunk","text":"..."}`
/// - `FinalAnswer`  → `{"type":"done","text":"..."}`  (event name: `done`)
/// - `ToolCallStart/Delta/Result` → `{"type":"tool_call",...}`
/// - `Metadata`     → `{"type":"metadata","key":...,"value":...}`
/// - `AwaitingApproval` → `{"type":"awaiting_approval",...}` (event name: `approval`)
/// - stream errors  → `{"type":"error","message":"..."}` then stream closes
pub fn stream_to_sse(
    stream: BoxStream<'static, Result<StreamEvent, WesichainError>>,
) -> impl IntoResponse {
    let mapped = stream.map(|item| match item {
        Ok(event) => {
            let (data, event_name) = event_to_json(event);
            let mut sse = Event::default().data(data.to_string());
            if let Some(name) = event_name {
                sse = sse.event(name);
            }
            Ok::<Event, std::convert::Infallible>(sse)
        }
        Err(e) => {
            let data = json!({"type": "error", "message": e.to_string()});
            Ok(Event::default().event("error").data(data.to_string()))
        }
    });

    Sse::new(mapped).keep_alive(KeepAlive::default())
}

fn event_to_json(event: StreamEvent) -> (serde_json::Value, Option<&'static str>) {
    match event {
        StreamEvent::ContentChunk(text) => {
            (json!({"type": "chunk", "text": text}), None)
        }
        StreamEvent::FinalAnswer(text) => {
            (json!({"type": "done", "text": text}), Some("done"))
        }
        StreamEvent::ToolCallStart { id, name } => {
            (json!({"type": "tool_call", "phase": "start", "id": id, "name": name}), None)
        }
        StreamEvent::ToolCallDelta { id, delta } => {
            (json!({"type": "tool_call", "phase": "delta", "id": id, "delta": delta}), None)
        }
        StreamEvent::ToolCallResult { id, output } => {
            (json!({"type": "tool_call", "phase": "result", "id": id, "output": output}), None)
        }
        StreamEvent::Metadata { key, value } => {
            (json!({"type": "metadata", "key": key, "value": value}), None)
        }
        StreamEvent::AwaitingApproval { run_id, prompt, checkpoint_id } => (
            json!({
                "type": "awaiting_approval",
                "run_id": run_id,
                "prompt": prompt,
                "checkpoint_id": checkpoint_id,
            }),
            Some("approval"),
        ),
        StreamEvent::ThinkingChunk(text) => {
            (json!({"type": "thinking", "text": text}), Some("thinking"))
        }
        StreamEvent::UsageUpdate { input_tokens, output_tokens, cache_read_tokens, cache_write_tokens } => (
            json!({
                "type": "usage",
                "input_tokens": input_tokens,
                "output_tokens": output_tokens,
                "cache_read_tokens": cache_read_tokens,
                "cache_write_tokens": cache_write_tokens,
            }),
            Some("usage"),
        ),
    }
}
