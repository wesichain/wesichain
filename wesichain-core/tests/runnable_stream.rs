use futures::StreamExt;
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
            Ok(StreamEvent::FinalAnswer(format!("{input}!"))),
        ];
        futures::stream::iter(events).boxed()
    }
}

#[tokio::test]
async fn runnable_stream_emits_events_in_order() {
    let dummy = Dummy;
    let events: Vec<_> = dummy.stream("hi".to_string()).collect().await;
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0], Ok(StreamEvent::ContentChunk(_))));
    assert!(matches!(events[1], Ok(StreamEvent::FinalAnswer(_))));
}
