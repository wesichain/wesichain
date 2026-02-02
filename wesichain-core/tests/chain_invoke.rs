use futures::stream::{BoxStream, StreamExt};

use wesichain_core::{Runnable, RunnableExt, WesichainError};

struct AddPrefix;
struct Uppercase;
struct AddSuffix;

fn empty_stream<'a>() -> BoxStream<'a, Result<wesichain_core::StreamEvent, WesichainError>> {
    futures::stream::iter(vec![Ok(wesichain_core::StreamEvent::FinalAnswer(
        String::new(),
    ))])
    .boxed()
}

#[async_trait::async_trait]
impl Runnable<String, String> for AddPrefix {
    async fn invoke(&self, input: String) -> Result<String, WesichainError> {
        Ok(format!("pre-{input}"))
    }

    fn stream(
        &self,
        _input: String,
    ) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        empty_stream()
    }
}

#[async_trait::async_trait]
impl Runnable<String, String> for Uppercase {
    async fn invoke(&self, input: String) -> Result<String, WesichainError> {
        Ok(input.to_uppercase())
    }

    fn stream(
        &self,
        _input: String,
    ) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        empty_stream()
    }
}

#[async_trait::async_trait]
impl Runnable<String, String> for AddSuffix {
    async fn invoke(&self, input: String) -> Result<String, WesichainError> {
        Ok(format!("{input}-suf"))
    }

    fn stream(
        &self,
        _input: String,
    ) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        empty_stream()
    }
}

#[tokio::test]
async fn chain_invokes_in_sequence() {
    let chain = AddPrefix.then(Uppercase).then(AddSuffix);
    let output = chain.invoke("alpha".to_string()).await.unwrap();
    assert_eq!(output, "PRE-ALPHA-suf".to_string());
}
