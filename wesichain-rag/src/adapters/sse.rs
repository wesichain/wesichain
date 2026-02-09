use serde_json::json;
use wesichain_core::AgentEvent;

fn format_sse(event_type: &str, payload: serde_json::Value) -> String {
    let data = serde_json::to_string(&payload).expect("SSE payload should serialize");
    format!("event: {event_type}\ndata: {data}\n\n")
}

pub fn to_sse_event(event: &AgentEvent) -> String {
    match event {
        AgentEvent::Status {
            stage,
            message,
            step,
            thread_id,
        } => format_sse(
            "status",
            json!({
                "stage": stage,
                "message": message,
                "step": step,
                "thread_id": thread_id,
            }),
        ),
        AgentEvent::Thought {
            content,
            step,
            metadata,
        } => format_sse(
            "trace",
            json!({
                "step": step,
                "thought": content,
                "metadata": metadata,
            }),
        ),
        AgentEvent::ToolCall {
            id,
            tool_name,
            input,
            step,
        } => format_sse(
            "trace",
            json!({
                "step": step,
                "call_id": id,
                "tool": tool_name,
                "input": input,
            }),
        ),
        AgentEvent::Observation {
            id,
            tool_name,
            output,
            step,
        } => format_sse(
            "trace",
            json!({
                "step": step,
                "call_id": id,
                "tool": tool_name,
                "observation": output,
            }),
        ),
        AgentEvent::Final { content, step } => format_sse(
            "answer",
            json!({
                "content": content,
                "step": step,
            }),
        ),
        AgentEvent::Error {
            message,
            step,
            recoverable,
            source,
        } => format_sse(
            "error",
            json!({
                "message": message,
                "step": step,
                "recoverable": recoverable,
                "source": source,
            }),
        ),
        AgentEvent::Metadata { key, value } => format_sse(
            "trace",
            json!({
                "metadata": {
                    "key": key,
                    "value": value,
                }
            }),
        ),
    }
}

pub fn ping_event() -> String {
    format_sse("ping", json!({}))
}

pub fn done_event() -> String {
    format_sse("done", json!({}))
}
