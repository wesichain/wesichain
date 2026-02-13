use futures::stream::BoxStream;
use futures::StreamExt;
use wesichain_compat::{LangChainError, LangChainRunnable, StreamEvent};

struct Echo;

#[async_trait::async_trait]
impl LangChainRunnable<String, String> for Echo {
    async fn invoke(&self, input: String) -> Result<String, LangChainError> {
        Ok(input)
    }

    fn stream(&self, input: String) -> BoxStream<'_, Result<StreamEvent, LangChainError>> {
        futures::stream::once(async move { Ok(StreamEvent::FinalAnswer(input)) }).boxed()
    }
}

#[tokio::test]
async fn compat_layer_works() {
    let runner = Echo;
    let result = runner.invoke("hello".to_string()).await.unwrap();
    assert_eq!(result, "hello");

    let batch_res = runner.batch(vec!["a".to_string(), "b".to_string()]).await;
    assert_eq!(batch_res.len(), 2);
    assert_eq!(batch_res[0].as_ref().unwrap(), "a");
}
