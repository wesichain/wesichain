use futures::stream::BoxStream;
use futures::StreamExt;
use wesichain_core::{Runnable, StreamEvent, WesichainError};

struct Number(i32);

#[async_trait::async_trait]
impl Runnable<i32, i32> for Number {
    async fn invoke(&self, input: i32) -> Result<i32, WesichainError> {
        if input < 0 {
            Err(WesichainError::Custom("Negative number".to_string()))
        } else {
            Ok(input * 2)
        }
    }

    fn stream(&self, input: i32) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}

#[tokio::test]
async fn batch_processes_concurrently_and_returns_results() {
    let runner = Number(0);
    let inputs = vec![1, 2, -1, 4];

    let results = runner.batch(inputs).await;

    assert_eq!(results.len(), 4);
    assert_eq!(results[0].as_ref().unwrap(), &2);
    assert_eq!(results[1].as_ref().unwrap(), &4);
    assert!(results[2].is_err());
    assert_eq!(results[3].as_ref().unwrap(), &8);
}

#[tokio::test]
async fn abatch_is_alias_for_batch() {
    let runner = Number(0);
    let inputs = vec![10, 20];
    let results = runner.abatch(inputs).await;

    assert_eq!(results[0].as_ref().unwrap(), &20);
    assert_eq!(results[1].as_ref().unwrap(), &40);
}
