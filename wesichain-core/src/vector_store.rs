use async_trait::async_trait;

use crate::{Document, MetadataFilter, StoreError};

#[derive(Clone, Debug)]
pub struct SearchResult {
    pub document: Document,
    pub score: f32,
}

#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn add(&self, docs: Vec<Document>) -> Result<(), StoreError>;
    async fn search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
        filter: Option<&MetadataFilter>,
    ) -> Result<Vec<SearchResult>, StoreError>;
    async fn delete(&self, ids: &[String]) -> Result<(), StoreError>;
}
