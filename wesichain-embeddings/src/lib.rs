mod error;

#[cfg(feature = "openai")]
mod openai;

#[cfg(feature = "ollama")]
mod ollama;

#[cfg(feature = "google")]
mod google;

#[cfg(feature = "candle")]
mod candle;

pub use error::EmbeddingProviderError;

#[cfg(feature = "openai")]
pub use openai::OpenAiEmbedding;

#[cfg(feature = "ollama")]
pub use ollama::OllamaEmbedding;

#[cfg(feature = "google")]
pub use google::GoogleEmbedding;

#[cfg(feature = "candle")]
pub use candle::CandleEmbedding;
