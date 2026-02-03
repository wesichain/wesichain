use async_trait::async_trait;
use wesichain_core::{Embedding, EmbeddingError};

const FNV_OFFSET: u64 = 14695981039346656037;
const FNV_PRIME: u64 = 1099511628211;

fn fnv1a(bytes: &[u8], seed: u64) -> u64 {
    let mut hash = FNV_OFFSET ^ seed;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[derive(Clone)]
pub struct HashEmbedder {
    dimension: usize,
}

impl HashEmbedder {
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }

    fn hash_to_vec(&self, text: &str) -> Vec<f32> {
        let bytes = text.as_bytes();
        let mut vec = Vec::with_capacity(self.dimension);
        for idx in 0..self.dimension {
            let value = fnv1a(bytes, idx as u64);
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
