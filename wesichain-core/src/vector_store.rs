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

    async fn delete_strs(&self, ids: &[&str]) -> Result<(), StoreError>
    where
        Self: Sized,
    {
        let owned: Vec<String> = ids.iter().map(|id| (*id).to_string()).collect();
        self.delete(&owned).await
    }

    async fn delete_ref<T>(&self, ids: &[T]) -> Result<(), StoreError>
    where
        T: AsRef<str> + Sync,
        Self: Sized,
    {
        let owned: Vec<String> = ids.iter().map(|id| id.as_ref().to_string()).collect();
        self.delete(&owned).await
    }
}

#[async_trait]
impl<T: VectorStore + ?Sized> VectorStore for std::sync::Arc<T> {
    async fn add(&self, docs: Vec<Document>) -> Result<(), StoreError> {
        self.as_ref().add(docs).await
    }

    async fn search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
        filter: Option<&MetadataFilter>,
    ) -> Result<Vec<SearchResult>, StoreError> {
        self.as_ref().search(query_embedding, top_k, filter).await
    }

    async fn delete(&self, ids: &[String]) -> Result<(), StoreError> {
        self.as_ref().delete(ids).await
    }
}

pub async fn delete_strs_dyn(store: &dyn VectorStore, ids: &[&str]) -> Result<(), StoreError> {
    let owned: Vec<String> = ids.iter().map(|id| (*id).to_string()).collect();
    store.delete(&owned).await
}

pub async fn delete_ref_dyn<T: AsRef<str> + Sync>(
    store: &dyn VectorStore,
    ids: &[T],
) -> Result<(), StoreError> {
    let owned: Vec<String> = ids.iter().map(|id| id.as_ref().to_string()).collect();
    store.delete(&owned).await
}
