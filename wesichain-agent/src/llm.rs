pub trait LlmAdapter: Send + Sync {
    fn complete(
        &self,
        request: wesichain_core::LlmRequest,
    ) -> Result<wesichain_core::LlmResponse, wesichain_core::WesichainError>;
}
