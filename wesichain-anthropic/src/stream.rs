//! SSE stream parser for the Anthropic Messages API.
//!
//! Anthropic uses a two-line SSE format where each event has an `event:` line
//! followed by a `data:` line.  This differs from OpenAI, which only uses
//! `data:` lines.

use bytes::BytesMut;
use futures::stream::{self, BoxStream, StreamExt};
use serde_json::Value;
use wesichain_core::{StreamEvent, WesichainError};

use crate::types::{ContentBlockType, ContentDelta, SseEvent};

/// State for a single tool-use content block that is being streamed.
struct ToolBlockState {
    index: u32,
    id: String,
    name: String,
    json_buf: String,
}

/// Parse an Anthropic SSE streaming response into a stream of [`StreamEvent`]s.
pub fn parse_anthropic_stream(
    response: reqwest::Response,
) -> BoxStream<'static, Result<StreamEvent, WesichainError>> {
    let byte_stream = response.bytes_stream();
    let mut buffer = BytesMut::new();

    // Accumulated state across chunks
    let mut accumulated_text = String::new();
    let mut pending_event_type: Option<String> = None;
    let mut tool_blocks: Vec<ToolBlockState> = Vec::new();
    let mut done = false;

    byte_stream
        .flat_map(move |chunk_result| {
            if done {
                return stream::iter(vec![]).boxed();
            }

            let bytes = match chunk_result {
                Ok(b) => b,
                Err(err) => {
                    done = true;
                    return stream::iter(vec![Err(WesichainError::LlmProvider(
                        err.to_string(),
                    ))])
                    .boxed();
                }
            };

            buffer.extend_from_slice(&bytes);
            let mut events: Vec<Result<StreamEvent, WesichainError>> = Vec::new();

            // Process complete lines from the buffer
            loop {
                let newline_pos = buffer.iter().position(|&b| b == b'\n');
                let Some(pos) = newline_pos else { break };

                let raw_line = buffer.split_to(pos + 1);
                // Strip trailing CR+LF or LF
                let line = String::from_utf8_lossy(&raw_line);
                let line = line.trim_end_matches(['\r', '\n']);

                if line.is_empty() {
                    // Empty line: end of SSE event — reset pending type
                    pending_event_type = None;
                    continue;
                }

                if let Some(event_type) = line.strip_prefix("event:") {
                    pending_event_type = Some(event_type.trim().to_string());
                    continue;
                }

                let data = match line.strip_prefix("data:") {
                    Some(d) => d.trim(),
                    None => continue,
                };

                // Parse the JSON payload using the event type hint if available
                // We inject the type into the JSON if necessary, because
                // `SseEvent` is tagged by "type".
                let sse_event: SseEvent = match inject_type_and_parse(
                    data,
                    pending_event_type.as_deref(),
                ) {
                    Ok(ev) => ev,
                    Err(err) => {
                        done = true;
                        events.push(Err(WesichainError::ParseFailed {
                            output: data.to_string(),
                            reason: err.to_string(),
                        }));
                        break;
                    }
                };

                match sse_event {
                    SseEvent::ContentBlockStart {
                        index,
                        content_block,
                    } => match content_block {
                        ContentBlockType::ToolUse { id, name } => {
                            tool_blocks.push(ToolBlockState {
                                index,
                                id,
                                name,
                                json_buf: String::new(),
                            });
                        }
                        ContentBlockType::Text { .. } | ContentBlockType::Thinking { .. } => {}
                    },

                    SseEvent::ContentBlockDelta { index, delta } => match delta {
                        ContentDelta::TextDelta { text } => {
                            accumulated_text.push_str(&text);
                            events.push(Ok(StreamEvent::ContentChunk(text)));
                        }
                        ContentDelta::InputJsonDelta { partial_json } => {
                            if let Some(tool) =
                                tool_blocks.iter_mut().find(|t| t.index == index)
                            {
                                tool.json_buf.push_str(&partial_json);
                            }
                        }
                        ContentDelta::ThinkingDelta { thinking } => {
                            events.push(Ok(StreamEvent::ThinkingChunk(thinking)));
                        }
                    },

                    SseEvent::ContentBlockStop { index } => {
                        // If this was a tool-use block, emit ToolCallStart + ToolCallDelta
                        if let Some(pos) =
                            tool_blocks.iter().position(|t| t.index == index)
                        {
                            let tool = tool_blocks.remove(pos);
                            let parsed: Value =
                                serde_json::from_str(&tool.json_buf).unwrap_or_else(
                                    |_| serde_json::json!({ "raw": tool.json_buf }),
                                );
                            events.push(Ok(StreamEvent::ToolCallStart {
                                id: tool.id.clone(),
                                name: tool.name.clone(),
                            }));
                            events.push(Ok(StreamEvent::ToolCallDelta {
                                id: tool.id,
                                delta: parsed,
                            }));
                        }
                    }

                    SseEvent::MessageStop => {
                        events.push(Ok(StreamEvent::FinalAnswer(
                            accumulated_text.clone(),
                        )));
                        done = true;
                    }

                    // message_start, message_delta, ping, other — no StreamEvent to emit
                    _ => {}
                }
            }

            stream::iter(events).boxed()
        })
        .boxed()
}

/// Anthropic SSE events carry a separate `event:` line with the type name,
/// but the `data:` JSON also contains a `"type"` field.  When the JSON already
/// has a `"type"` key we parse it directly.  If not (e.g. `message_stop` data
/// is `{}`), we inject the type from the `event:` line before parsing.
fn inject_type_and_parse(
    data: &str,
    event_type: Option<&str>,
) -> Result<SseEvent, serde_json::Error> {
    // Fast path: the data already contains a "type" field
    if data.contains("\"type\"") {
        return serde_json::from_str(data);
    }

    // Slow path: inject the type field from the `event:` line
    if let Some(ev_type) = event_type {
        // Convert kebab-case event names to snake_case if needed
        // Anthropic uses snake_case already (e.g. "message_stop"), but be safe.
        let type_str = ev_type.replace('-', "_");
        let mut value: Value = serde_json::from_str(data)?;
        if let Some(obj) = value.as_object_mut() {
            obj.insert("type".to_string(), Value::String(type_str));
        }
        return serde_json::from_value(value);
    }

    // Last resort: try parsing as-is and let serde handle the error
    serde_json::from_str(data)
}
