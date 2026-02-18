use async_trait::async_trait;

#[async_trait]
pub trait LlmAdapter: Send + Sync {
    async fn complete(
        &self,
        request: wesichain_core::LlmRequest,
    ) -> Result<wesichain_core::LlmResponse, wesichain_core::WesichainError>;
}
