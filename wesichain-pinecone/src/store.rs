use serde_json::Value;
use uuid::Uuid;
use wesichain_core::{Document, Embedding, MetadataFilter, SearchResult, StoreError, VectorStore};

use crate::client::PineconeHttpClient;
use crate::config::PineconeStoreBuilder;
use crate::filter::{to_pinecone_filter_json, PineconeFilter};
use crate::mapper::{doc_to_metadata, match_to_document};
use crate::types::{
    DeleteRequest, IndexStatsResponse, PineconeVector, QueryRequest, QueryResponse, UpsertRequest,
};
use crate::PineconeStoreError;

pub struct PineconeVectorStore<E> {
    pub(crate) embedder: E,
    pub(crate) client: PineconeHttpClient,
    pub(crate) namespace: Option<String>,
    pub(crate) text_key: String,
    pub(crate) index_name: Option<String>,
    pub(crate) validate_dimension: bool,
    pub(crate) max_batch_size: usize,
}

impl<E> PineconeVectorStore<E> {
    pub fn builder(embedder: E) -> PineconeStoreBuilder<E> {
        PineconeStoreBuilder::new(embedder)
    }

    pub(crate) fn new(
        embedder: E,
        client: PineconeHttpClient,
        namespace: Option<String>,
        text_key: String,
        index_name: Option<String>,
        validate_dimension: bool,
        max_batch_size: usize,
    ) -> Self {
        Self {
            embedder,
            client,
            namespace,
            text_key,
            index_name,
            validate_dimension,
            max_batch_size,
        }
    }

    pub fn text_key(&self) -> &str {
        &self.text_key
    }
}

impl<E> PineconeVectorStore<E>
where
    E: Embedding + Send + Sync,
{
    pub(crate) async fn validate_dimension_on_init(&self) {
        if !self.validate_dimension {
            return;
        }

        let response = self
            .client
            .post_typed::<Value, IndexStatsResponse>(
                "/describe_index_stats",
                &Value::Object(serde_json::Map::new()),
            )
            .await;

        match response {
            Ok(stats) => {
                if let Some(index_dim) = stats.dimension {
                    let embedder_dim = self.embedder.dimension();
                    if index_dim != embedder_dim {
                        tracing::warn!(
                            index_name = ?self.index_name,
                            namespace = ?self.namespace,
                            index_dim = index_dim,
                            embedder_dim = embedder_dim,
                            "embedder dimension differs from pinecone index dimension"
                        );
                    }
                } else {
                    tracing::warn!("pinecone describe_index_stats response missing 'dimension'");
                }
            }
            Err(err) => {
                tracing::warn!(error = %err, "failed to validate pinecone index dimension");
            }
        }
    }

    async fn query_with_embedding(
        &self,
        query_embedding: Vec<f32>,
        top_k: usize,
        filter: Option<Value>,
    ) -> Result<Vec<(Document, f32)>, StoreError> {
        let span = tracing::info_span!(
            "pinecone_query",
            namespace = ?self.namespace,
            top_k = top_k,
            text_key = %self.text_key,
        );
        let _guard = span.enter();

        let request = QueryRequest {
            vector: query_embedding,
            top_k,
            include_metadata: true,
            filter,
            namespace: self.namespace.clone(),
        };

        let response: QueryResponse = self
            .client
            .post_typed_with_context("/query", &request, self.namespace.as_deref(), None)
            .await
            .map_err(StoreError::from)?;

        let mut output = Vec::with_capacity(response.matches.len());
        for m in response.matches {
            let metadata = m.metadata.unwrap_or_else(|| Value::Object(serde_json::Map::new()));
            let doc = match_to_document(&m.id, &metadata, &self.text_key).map_err(StoreError::from)?;
            output.push((doc, m.score));
        }

        Ok(output)
    }

    pub async fn add_documents(
        &self,
        docs: Vec<Document>,
        ids: Option<Vec<String>>,
    ) -> Result<(), StoreError> {
        let span = tracing::info_span!(
            "pinecone_upsert",
            namespace = ?self.namespace,
            batch_size = docs.len(),
            text_key = %self.text_key,
        );
        let _guard = span.enter();

        let texts: Vec<String> = docs.iter().map(|doc| doc.content.clone()).collect();
        let embeddings = self
            .embedder
            .embed_batch(&texts)
            .await
            .map_err(|err| StoreError::Internal(Box::new(err)))?;

        if docs.len() != embeddings.len() {
            return Err(PineconeStoreError::BatchMismatch {
                docs: docs.len(),
                embeddings: embeddings.len(),
            }
            .into());
        }

        let expected_dim = self.embedder.dimension();
        if let Some(ids) = &ids {
            if ids.len() != docs.len() {
                return Err(PineconeStoreError::Config(format!(
                    "ids length ({}) must match docs length ({})",
                    ids.len(),
                    docs.len()
                ))
                .into());
            }
        }

        let mut vectors = Vec::with_capacity(docs.len());
        for (idx, (doc, embedding)) in docs.iter().zip(embeddings.into_iter()).enumerate() {
            if embedding.len() != expected_dim {
                return Err(PineconeStoreError::DimensionMismatch {
                    expected: expected_dim,
                    got: embedding.len(),
                }
                .into());
            }

            let id = ids
                .as_ref()
                .and_then(|list| list.get(idx))
                .cloned()
                .or_else(|| {
                    if doc.id.trim().is_empty() {
                        None
                    } else {
                        Some(doc.id.clone())
                    }
                })
                .unwrap_or_else(|| Uuid::new_v4().to_string());

            if id.trim().is_empty() {
                return Err(StoreError::InvalidId(id));
            }

            let metadata_map = doc_to_metadata(doc, &self.text_key);
            let metadata = Value::Object(serde_json::Map::from_iter(metadata_map.into_iter()));

            vectors.push(PineconeVector {
                id,
                values: embedding,
                sparse_values: None,
                metadata: Some(metadata),
            });
        }

        let total_chunks = vectors.len().div_ceil(self.max_batch_size);
        for (chunk_index, chunk) in vectors.chunks(self.max_batch_size).enumerate() {
            let chunk_span = tracing::info_span!(
                "pinecone_upsert_chunk",
                namespace = ?self.namespace,
                chunk_index = chunk_index + 1,
                total_chunks = total_chunks,
                batch_size = chunk.len(),
            );
            let _chunk_guard = chunk_span.enter();

            let request = UpsertRequest {
                vectors: chunk.to_vec(),
                namespace: self.namespace.clone(),
            };

            let _: Value = self
                .client
                .post_typed_with_context(
                    "/vectors/upsert",
                    &request,
                    self.namespace.as_deref(),
                    Some(request.vectors.len()),
                )
                .await
                .map_err(StoreError::from)?;
        }

        Ok(())
    }

    pub async fn similarity_search_by_filter(
        &self,
        query: &str,
        k: usize,
        filter: Option<PineconeFilter>,
    ) -> Result<Vec<Document>, StoreError> {
        let query_embedding = self
            .embedder
            .embed(query)
            .await
            .map_err(|err| StoreError::Internal(Box::new(err)))?;

        let filter_json = filter
            .as_ref()
            .map(to_pinecone_filter_json)
            .transpose()
            .map_err(StoreError::from)?;
        let matches = self.query_with_embedding(query_embedding, k, filter_json).await?;
        Ok(matches.into_iter().map(|(doc, _)| doc).collect())
    }

    pub async fn similarity_search(
        &self,
        query: &str,
        k: usize,
        filter: Option<MetadataFilter>,
    ) -> Result<Vec<Document>, StoreError> {
        self.similarity_search_by_filter(
            query,
            k,
            filter.map(PineconeFilter::Typed),
        )
        .await
    }

    pub async fn similarity_search_raw_filter(
        &self,
        query: &str,
        k: usize,
        filter: Value,
    ) -> Result<Vec<Document>, StoreError> {
        self.similarity_search_by_filter(query, k, Some(PineconeFilter::Raw(filter)))
            .await
    }

    pub async fn similarity_search_with_score_by_filter(
        &self,
        query: &str,
        k: usize,
        filter: Option<PineconeFilter>,
    ) -> Result<Vec<(Document, f32)>, StoreError> {
        let query_embedding = self
            .embedder
            .embed(query)
            .await
            .map_err(|err| StoreError::Internal(Box::new(err)))?;

        let filter_json = filter
            .as_ref()
            .map(to_pinecone_filter_json)
            .transpose()
            .map_err(StoreError::from)?;
        self.query_with_embedding(query_embedding, k, filter_json).await
    }

    pub async fn similarity_search_with_score(
        &self,
        query: &str,
        k: usize,
        filter: Option<MetadataFilter>,
    ) -> Result<Vec<(Document, f32)>, StoreError> {
        self.similarity_search_with_score_by_filter(
            query,
            k,
            filter.map(PineconeFilter::Typed),
        )
        .await
    }

    pub async fn similarity_search_with_score_raw_filter(
        &self,
        query: &str,
        k: usize,
        filter: Value,
    ) -> Result<Vec<(Document, f32)>, StoreError> {
        self.similarity_search_with_score_by_filter(query, k, Some(PineconeFilter::Raw(filter)))
            .await
    }

    pub async fn delete_ids(&self, ids: Vec<String>) -> Result<(), StoreError> {
        let span = tracing::info_span!(
            "pinecone_delete",
            namespace = ?self.namespace,
            id_count = ids.len(),
        );
        let _guard = span.enter();

        let request = DeleteRequest {
            ids,
            namespace: self.namespace.clone(),
        };
        let _: Value = self
            .client
            .post_typed_with_context(
                "/vectors/delete",
                &request,
                self.namespace.as_deref(),
                Some(request.ids.len()),
            )
            .await
            .map_err(StoreError::from)?;
        Ok(())
    }

    pub async fn delete_documents(&self, ids: Vec<String>) -> Result<(), StoreError> {
        self.delete_ids(ids).await
    }

    pub async fn delete(&self, ids: &[String]) -> Result<(), StoreError> {
        self.delete_ids(ids.to_vec()).await
    }

    pub async fn delete_vec(&self, ids: Vec<String>) -> Result<(), StoreError> {
        self.delete_ids(ids).await
    }

    pub async fn from_documents(
        docs: Vec<Document>,
        embedder: E,
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        namespace: Option<String>,
        text_key: Option<String>,
    ) -> Result<Self, StoreError> {
        let mut builder = Self::builder(embedder)
            .base_url(base_url.into())
            .api_key(api_key.into());
        if let Some(ns) = namespace {
            builder = builder.namespace(ns);
        }
        if let Some(key) = text_key {
            builder = builder.text_key(key);
        }
        let store = builder.build().await.map_err(StoreError::from)?;
        store.add_documents(docs, None).await?;
        Ok(store)
    }
}

#[async_trait::async_trait]
impl<E> VectorStore for PineconeVectorStore<E>
where
    E: Embedding + Send + Sync,
{
    async fn add(&self, docs: Vec<Document>) -> Result<(), StoreError> {
        self.add_documents(docs, None).await
    }

    async fn search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
        filter: Option<&MetadataFilter>,
    ) -> Result<Vec<SearchResult>, StoreError> {
        let filter_json = filter
            .cloned()
            .map(|f| to_pinecone_filter_json(&PineconeFilter::Typed(f)))
            .transpose()
            .map_err(StoreError::from)?;

        let matches = self
            .query_with_embedding(query_embedding.to_vec(), top_k, filter_json)
            .await?;
        Ok(matches
            .into_iter()
            .map(|(document, score)| SearchResult { document, score })
            .collect())
    }

    async fn delete(&self, ids: &[String]) -> Result<(), StoreError> {
        self.delete_ids(ids.to_vec()).await
    }
}
