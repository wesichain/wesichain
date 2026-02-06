mod error;
mod hash_embedder;
mod in_memory;
mod indexer;
mod loader;
mod retriever;
mod splitter;

use std::path::PathBuf;

use wesichain_core::Document;

pub use error::{IngestionError, RetrievalError};
pub use hash_embedder::HashEmbedder;
pub use in_memory::InMemoryVectorStore;
pub use indexer::Indexer;
pub use loader::{load_file_async, load_files_async, PdfLoader, TextLoader};
pub use retriever::Retriever;
pub use splitter::{RecursiveCharacterTextSplitter, SplitterConfigError, TextSplitter};

pub async fn load_and_split_recursive(
    paths: Vec<PathBuf>,
    splitter: &RecursiveCharacterTextSplitter,
) -> Result<Vec<Document>, IngestionError> {
    let documents = load_files_async(paths).await?;
    Ok(splitter.split_documents(&documents))
}
