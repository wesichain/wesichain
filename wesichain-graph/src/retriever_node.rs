use std::sync::Arc;

use async_trait::async_trait;
use futures::stream::StreamExt;
use wesichain_core::{
    Document, Embedding, EmbeddingError, HasMetadataFilter, HasQuery, HasRetrievedDocs,
    MetadataFilter, Runnable, SearchResult, StoreError, StreamEvent, VectorStore, WesichainError,
};
use wesichain_retrieval::Retriever;

use crate::{GraphState, StateSchema, StateUpdate};

/// Newtype wrapper to implement `Embedding` for `Arc<dyn Embedding>`.
///
/// Required by Rust's orphan rule: we can't implement a foreign trait (`Embedding`)
/// on a foreign type (`Arc<dyn Embedding>`) directly. This wrapper delegates all
/// methods and is used internally by [`RetrieverNode`].
struct DynEmbedding(Arc<dyn Embedding>);

#[async_trait]
impl Embedding for DynEmbedding {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        self.0.embed(text).await
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        self.0.embed_batch(texts).await
    }

    fn dimension(&self) -> usize {
        self.0.dimension()
    }
}

/// Newtype wrapper to implement `VectorStore` for `Arc<dyn VectorStore>`.
///
/// Same orphan-rule workaround as [`DynEmbedding`]. Used internally by
/// [`RetrieverNode`] to bridge the `Arc<dyn VectorStore>` constructor parameter
/// into the concrete `Retriever<E, V>` type.
struct DynVectorStore(Arc<dyn VectorStore>);

#[async_trait]
impl VectorStore for DynVectorStore {
    async fn add(&self, docs: Vec<Document>) -> Result<(), StoreError> {
        self.0.add(docs).await
    }

    async fn search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
        filter: Option<&MetadataFilter>,
    ) -> Result<Vec<SearchResult>, StoreError> {
        self.0.search(query_embedding, top_k, filter).await
    }

    async fn delete(&self, ids: &[String]) -> Result<(), StoreError> {
        self.0.delete(ids).await
    }
}

pub struct RetrieverNode {
    retriever: Retriever<DynEmbedding, DynVectorStore>,
    top_k: usize,
    score_threshold: Option<f32>,
}

impl RetrieverNode {
    pub fn new(
        embedder: Arc<dyn Embedding>,
        store: Arc<dyn VectorStore>,
        top_k: usize,
        score_threshold: Option<f32>,
    ) -> Self {
        Self {
            retriever: Retriever::new(DynEmbedding(embedder), DynVectorStore(store)),
            top_k,
            score_threshold,
        }
    }

    fn apply_threshold(&self, mut results: Vec<SearchResult>) -> Vec<SearchResult> {
        if let Some(threshold) = self.score_threshold {
            results.retain(|res| res.score >= threshold);
        }
        results
    }
}

#[async_trait]
impl<S> Runnable<GraphState<S>, StateUpdate<S>> for RetrieverNode
where
    S: StateSchema<Update = S> + HasQuery + HasRetrievedDocs + HasMetadataFilter,
{
    async fn invoke(&self, input: GraphState<S>) -> Result<StateUpdate<S>, WesichainError> {
        let query = input.data.query();
        let filter = input.data.metadata_filter();
        let results = self
            .retriever
            .retrieve(query, self.top_k, filter.as_ref())
            .await
            .map_err(|err| WesichainError::Custom(err.to_string()))?;
        let results = self.apply_threshold(results);
        let docs = results.into_iter().map(|res| res.document).collect();

        let mut state = input;
        state.data.set_retrieved_docs(docs);
        Ok(StateUpdate::new(state.data))
    }

    fn stream(
        &self,
        _input: GraphState<S>,
    ) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }
}
