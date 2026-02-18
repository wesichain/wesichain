#![allow(deprecated)]

use async_trait::async_trait;
use futures::stream::StreamExt;
use wesichain_core::WesichainError;
use wesichain_llm::{Llm, LlmRequest, LlmResponse};

struct DummyLlm;

#[async_trait]
impl wesichain_core::Runnable<LlmRequest, LlmResponse> for DummyLlm {
    async fn invoke(&self, _input: LlmRequest) -> Result<LlmResponse, WesichainError> {
        Ok(LlmResponse {
            content: "ok".to_string(),
            tool_calls: vec![],
        })
    }

    fn stream(
        &self,
        _input: LlmRequest,
    ) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}

fn assert_llm<T: Llm>() {}

#[test]
fn dummy_llm_implements_llm() {
    assert_llm::<DummyLlm>();
}
