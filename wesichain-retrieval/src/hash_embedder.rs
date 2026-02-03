use std::hash::{Hash, Hasher};

use async_trait::async_trait;
use wesichain_core::{Embedding, EmbeddingError};

#[derive(Clone)]
pub struct HashEmbedder {
    dimension: usize,
}

impl HashEmbedder {
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }

    fn hash_to_vec(&self, text: &str) -> Vec<f32> {
        let mut vec = Vec::with_capacity(self.dimension);
        for idx in 0..self.dimension {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            text.hash(&mut hasher);
            idx.hash(&mut hasher);
            let value = hasher.finish();
            let normalized = (value % 10_000) as f32 / 10_000.0;
            vec.push(normalized);
        }
        vec
    }
}

#[async_trait]
impl Embedding for HashEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        Ok(self.hash_to_vec(text))
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        Ok(texts.iter().map(|text| self.hash_to_vec(text)).collect())
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}
