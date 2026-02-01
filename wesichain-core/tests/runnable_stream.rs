use futures::StreamExt;
use serde_json::json;
use wesichain_core::{Runnable, StreamEvent, WesichainError};

struct Dummy;

#[async_trait::async_trait]
impl Runnable<String, String> for Dummy {
    async fn invoke(&self, input: String) -> Result<String, WesichainError> {
        Ok(format!("{input}!"))
    }

    fn stream(
        &self,
        input: String,
    ) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        let events = vec![
            Ok(StreamEvent::ContentChunk(input.clone())),
            Ok(StreamEvent::ToolCallStart {
                id: "tool-1".to_string(),
                name: "echo".to_string(),
            }),
            Ok(StreamEvent::ToolCallDelta {
                id: "tool-1".to_string(),
                delta: json!({"chunk": "hi"}),
            }),
            Ok(StreamEvent::ToolCallResult {
                id: "tool-1".to_string(),
                output: json!({"result": "hi"}),
            }),
            Ok(StreamEvent::Metadata {
                key: "model".to_string(),
                value: json!("wesichain"),
            }),
            Ok(StreamEvent::FinalAnswer(format!("{input}!"))),
        ];
        futures::stream::iter(events).boxed()
    }
}

#[tokio::test]
async fn runnable_stream_emits_events_in_order() {
    let dummy = Dummy;
    let events: Vec<_> = dummy.stream("hi".to_string()).collect().await;
    let events: Vec<_> = events.into_iter().collect::<Result<Vec<_>, _>>().unwrap();

    let expected = vec![
        StreamEvent::ContentChunk("hi".to_string()),
        StreamEvent::ToolCallStart {
            id: "tool-1".to_string(),
            name: "echo".to_string(),
        },
        StreamEvent::ToolCallDelta {
            id: "tool-1".to_string(),
            delta: json!({"chunk": "hi"}),
        },
        StreamEvent::ToolCallResult {
            id: "tool-1".to_string(),
            output: json!({"result": "hi"}),
        },
        StreamEvent::Metadata {
            key: "model".to_string(),
            value: json!("wesichain"),
        },
        StreamEvent::FinalAnswer("hi!".to_string()),
    ];

    let cloned = expected.clone();
    assert_eq!(cloned.len(), expected.len());
    assert_eq!(events, expected);
}
