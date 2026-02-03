mod error {
    use thiserror::Error;

    #[derive(Debug, Error)]
    #[error("embedding provider error")]
    pub struct EmbeddingProviderError;
}

#[cfg(feature = "openai")]
mod openai {
    #[derive(Debug, Default, Clone)]
    pub struct OpenAiEmbedding;
}

#[cfg(feature = "ollama")]
mod ollama {
    #[derive(Debug, Default, Clone)]
    pub struct OllamaEmbedding;
}

#[cfg(feature = "candle")]
mod candle {
    #[derive(Debug, Default, Clone)]
    pub struct CandleEmbedding;
}

pub use error::EmbeddingProviderError;

#[cfg(feature = "openai")]
pub use openai::OpenAiEmbedding;

#[cfg(feature = "ollama")]
pub use ollama::OllamaEmbedding;

#[cfg(feature = "candle")]
pub use candle::CandleEmbedding;
