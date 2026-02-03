mod error {
    use thiserror::Error;

    #[derive(Debug, Error)]
    #[error("retrieval error")]
    pub struct RetrievalError;
}

mod in_memory {
    #[derive(Debug, Default, Clone)]
    pub struct InMemoryVectorStore;
}

mod indexer {
    #[derive(Debug, Default, Clone)]
    pub struct Indexer;
}

mod loader {
    #[derive(Debug, Default, Clone)]
    pub struct PdfLoader;

    #[derive(Debug, Default, Clone)]
    pub struct TextLoader;
}

mod retriever {
    #[derive(Debug, Default, Clone)]
    pub struct Retriever;
}

mod splitter {
    #[derive(Debug, Default, Clone)]
    pub struct TextSplitter;
}

pub use error::RetrievalError;
pub use in_memory::InMemoryVectorStore;
pub use indexer::Indexer;
pub use loader::{PdfLoader, TextLoader};
pub use retriever::Retriever;
pub use splitter::TextSplitter;
