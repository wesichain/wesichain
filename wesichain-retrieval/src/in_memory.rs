use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use wesichain_core::{Document, MetadataFilter, SearchResult, StoreError, Value, VectorStore};

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

            if let Some(&index) = inner.id_map.get(&doc.id) {
                inner.docs[index] = Some(doc);
                inner.embeddings[index] = Some(embedding);
            } else {
                let index = inner.docs.len();
                inner.id_map.insert(doc.id.clone(), index);
                inner.docs.push(Some(doc));
                inner.embeddings.push(Some(embedding));
            }
        }
        Ok(())
    }

    async fn search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
        filter: Option<&MetadataFilter>,
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
            let Some(doc) = inner.docs[idx].as_ref() else {
                continue;
            };
            if let Some(filter) = filter {
                if !metadata_matches(filter, &doc.metadata) {
                    continue;
                }
            }
            let mut score = cosine_similarity(query_embedding, embedding);
            if score.is_nan() {
                score = f32::NEG_INFINITY;
            }
            let mut result_doc = doc.clone();
            result_doc.embedding = None;
            scored.push(SearchResult {
                document: result_doc,
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

fn metadata_matches(filter: &MetadataFilter, metadata: &HashMap<String, Value>) -> bool {
    match filter {
        MetadataFilter::Eq(key, value) => metadata.get(key).map_or(false, |entry| entry == value),
        MetadataFilter::In(key, values) => metadata
            .get(key)
            .map_or(false, |entry| values.iter().any(|value| value == entry)),
        MetadataFilter::Range { key, min, max } => {
            let Some(value) = metadata.get(key) else {
                return false;
            };
            let Some(value) = value.as_f64() else {
                return false;
            };
            if let Some(min_value) = min {
                let Some(min_value) = min_value.as_f64() else {
                    return false;
                };
                if value < min_value {
                    return false;
                }
            }
            if let Some(max_value) = max {
                let Some(max_value) = max_value.as_f64() else {
                    return false;
                };
                if value > max_value {
                    return false;
                }
            }
            true
        }
        MetadataFilter::All(filters) => filters
            .iter()
            .all(|filter| metadata_matches(filter, metadata)),
        MetadataFilter::Any(filters) => filters
            .iter()
            .any(|filter| metadata_matches(filter, metadata)),
    }
}
