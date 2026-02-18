use async_trait::async_trait;
use wesichain_core::{Embedding, MetadataFilter, SearchResult, VectorStore};

use crate::{BaseRetriever, RetrievalError};

pub struct Retriever<E, S> {
    embedder: E,
    store: S,
}

impl<E, S> Retriever<E, S>
where
    E: Embedding,
    S: VectorStore,
{
    pub fn new(embedder: E, store: S) -> Self {
        Self { embedder, store }
    }

    pub async fn retrieve(
        &self,
        query: &str,
        top_k: usize,
        filter: Option<&MetadataFilter>,
    ) -> Result<Vec<SearchResult>, RetrievalError> {
        let embedding = self.embedder.embed(query).await?;
        let results = self.store.search(&embedding, top_k, filter).await?;
        Ok(results)
    }
}

#[async_trait]
impl<E, S> BaseRetriever for Retriever<E, S>
where
    E: Embedding + Send + Sync,
    S: VectorStore + Send + Sync,
{
    async fn retrieve(
        &self,
        query: &str,
        top_k: usize,
        filter: Option<&MetadataFilter>,
    ) -> Result<Vec<SearchResult>, RetrievalError> {
        let embedding = self.embedder.embed(query).await?;
        let results = self.store.search(&embedding, top_k, filter).await?;
        Ok(results)
    }
}
