use async_trait::async_trait;
use wesichain_core::{MetadataFilter, SearchResult};

use crate::error::RetrievalError;

/// Base trait for document retrievers.
/// 
/// Retrievers take a query string and return relevant documents from a backing store.
/// This trait abstracts over different retrieval strategies (vector search, keyword search,
/// hybrid approaches, etc.).
#[async_trait]
pub trait BaseRetriever: Send + Sync {
    /// Retrieve documents relevant to the given query.
    ///
    /// # Arguments
    /// * `query` - The search query
    /// * `top_k` - Maximum number of results to return
    /// * `filter` - Optional metadata filter to apply
    ///
    /// # Returns
    /// A vector of search results, ordered by relevance (most relevant first)
    async fn retrieve(
        &self,
        query: &str,
        top_k: usize,
        filter: Option<&MetadataFilter>,
    ) -> Result<Vec<SearchResult>, RetrievalError>;
}
