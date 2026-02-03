use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use wesichain_core::{Document, MetadataFilter, SearchResult, StoreError, VectorStore};

#[derive(Default)]
struct StoreInner {
    docs: Vec<Option<Document>>,
    embeddings: Vec<Option<Vec<f32>>>,
    id_map: HashMap<String, usize>,
    dimension: Option<usize>,
}

#[derive(Clone, Default)]
pub struct InMemoryVectorStore {
    inner: Arc<RwLock<StoreInner>>,
}

impl InMemoryVectorStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait::async_trait]
impl VectorStore for InMemoryVectorStore {
    async fn add(&self, docs: Vec<Document>) -> Result<(), StoreError> {
        let mut inner = self.inner.write().await;
        for mut doc in docs {
            if doc.id.trim().is_empty() {
                return Err(StoreError::InvalidId(doc.id));
            }

            let embedding = doc.embedding.take().ok_or_else(|| {
                StoreError::Internal(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "missing embedding",
                )))
            })?;
            let dimension = embedding.len();
            match inner.dimension {
                Some(expected) if expected != dimension => {
                    return Err(StoreError::DimensionMismatch {
                        expected,
                        got: dimension,
                    });
                }
                None => inner.dimension = Some(dimension),
                _ => {}
            }

            let index = inner.docs.len();
            inner.id_map.insert(doc.id.clone(), index);
            inner.docs.push(Some(doc));
            inner.embeddings.push(Some(embedding));
        }
        Ok(())
    }

    async fn search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
        _filter: Option<&MetadataFilter>,
    ) -> Result<Vec<SearchResult>, StoreError> {
        let inner = self.inner.read().await;
        let expected = inner.dimension.unwrap_or(query_embedding.len());
        if expected != query_embedding.len() {
            return Err(StoreError::DimensionMismatch {
                expected,
                got: query_embedding.len(),
            });
        }

        let mut scored = Vec::new();
        for (idx, embedding) in inner.embeddings.iter().enumerate() {
            let Some(embedding) = embedding else { continue };
            let score = cosine_similarity(query_embedding, embedding);
            let Some(doc) = inner.docs[idx].as_ref() else { continue };
            scored.push(SearchResult {
                document: doc.clone(),
                score,
            });
        }

        scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        scored.truncate(top_k);
        Ok(scored)
    }

    async fn delete(&self, ids: &[String]) -> Result<(), StoreError> {
        let mut inner = self.inner.write().await;
        for id in ids {
            if let Some(idx) = inner.id_map.remove(id) {
                inner.docs[idx] = None;
                inner.embeddings[idx] = None;
            }
        }
        Ok(())
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a.sqrt() * norm_b.sqrt())
}
