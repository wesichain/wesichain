mod error;

#[cfg(feature = "openai")]
mod openai;

#[cfg(feature = "ollama")]
mod ollama;

#[cfg(feature = "candle")]
mod candle;

pub use error::EmbeddingProviderError;

#[cfg(feature = "openai")]
pub use openai::OpenAiEmbedding;

#[cfg(feature = "ollama")]
pub use ollama::OllamaEmbedding;

#[cfg(feature = "candle")]
pub use candle::CandleEmbedding;
