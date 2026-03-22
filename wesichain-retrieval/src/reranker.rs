//! Cross-encoder re-ranking for RAG retrieval pipelines.
//!
//! A re-ranker retrieves a larger candidate set from a [`BaseRetriever`], then
//! scores each candidate with a [`Reranker`] and returns the top-k by score.
//!
//! # Built-in scorers
//!
//! - [`KeywordReranker`] — fast BM25-lite term overlap, no network required.
//!
//! # Custom scorer
//!
//! Implement [`Reranker`] for any scoring function — including an LLM call:
//!
//! ```ignore
//! struct MyLlmScorer { llm: MyLlm }
//!
//! #[async_trait::async_trait]
//! impl Reranker for MyLlmScorer {
//!     async fn score(&self, query: &str, doc: &str) -> f32 {
//!         // ask the LLM to rate relevance 0–1
//!         0.0
//!     }
//! }
//! ```

use async_trait::async_trait;
use wesichain_core::{MetadataFilter, SearchResult};

use crate::{BaseRetriever, RetrievalError};

// ── Reranker trait ────────────────────────────────────────────────────────────

/// A scorer that rates the relevance of a document to a query.
///
/// Scores should be in `[0.0, 1.0]` (higher = more relevant), but the only
/// hard requirement is that scores are comparable for ranking purposes.
#[async_trait]
pub trait Reranker: Send + Sync {
    async fn score(&self, query: &str, doc: &str) -> f32;
}

// ── CrossEncoderRetriever ─────────────────────────────────────────────────────

/// A [`BaseRetriever`] wrapper that fetches a larger candidate pool and
/// re-ranks results using a [`Reranker`].
///
/// The `oversample_factor` controls how many candidates to fetch before
/// re-ranking.  A factor of 3 means 3× `top_k` candidates are retrieved,
/// re-ranked, and the best `top_k` are returned.
pub struct CrossEncoderRetriever<R, S> {
    inner: R,
    reranker: S,
    oversample_factor: usize,
}

impl<R, S> CrossEncoderRetriever<R, S>
where
    R: BaseRetriever,
    S: Reranker,
{
    /// Create a new re-ranking retriever.
    ///
    /// - `inner`: the base retriever (e.g. [`Retriever`](crate::Retriever))
    /// - `reranker`: the scorer to apply to candidates
    /// - `oversample_factor`: multiplier on `top_k` for the candidate pool (≥ 1)
    pub fn new(inner: R, reranker: S, oversample_factor: usize) -> Self {
        Self { inner, reranker, oversample_factor: oversample_factor.max(1) }
    }
}

#[async_trait]
impl<R, S> BaseRetriever for CrossEncoderRetriever<R, S>
where
    R: BaseRetriever + Send + Sync,
    S: Reranker + Send + Sync,
{
    async fn retrieve(
        &self,
        query: &str,
        top_k: usize,
        filter: Option<&MetadataFilter>,
    ) -> Result<Vec<SearchResult>, RetrievalError> {
        let candidate_k = top_k.saturating_mul(self.oversample_factor);
        let mut candidates = self.inner.retrieve(query, candidate_k, filter).await?;

        // Score each candidate with the reranker
        for result in &mut candidates {
            let score = self.reranker.score(query, &result.document.content).await;
            result.score = score;
        }

        // Sort by descending score and truncate to top_k
        candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        candidates.truncate(top_k);
        Ok(candidates)
    }
}

// ── KeywordReranker ───────────────────────────────────────────────────────────

/// A fast keyword-overlap re-ranker using BM25-lite term frequency scoring.
///
/// Requires no network calls and no ML models — suitable for local re-ranking
/// when an LLM scorer would be too slow or expensive.
///
/// Scoring formula (simplified BM25):
/// `score = Σ IDF(t) * tf(t, doc) * (k1 + 1) / (tf(t, doc) + k1 * (1 - b + b * |doc| / avgdl))`
///
/// In practice for re-ranking we use a lightweight variant:
/// `score = matched_query_terms / total_query_terms + unique_ratio_bonus`
pub struct KeywordReranker {
    k1: f32,
    b: f32,
}

impl KeywordReranker {
    pub fn new() -> Self {
        Self { k1: 1.2, b: 0.75 }
    }

    /// Override BM25 k1 and b parameters.
    pub fn with_params(k1: f32, b: f32) -> Self {
        Self { k1, b }
    }

    fn tokenize(text: &str) -> Vec<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|t| t.len() > 1)
            .map(String::from)
            .collect()
    }
}

impl Default for KeywordReranker {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Reranker for KeywordReranker {
    async fn score(&self, query: &str, doc: &str) -> f32 {
        let query_terms = Self::tokenize(query);
        if query_terms.is_empty() {
            return 0.0;
        }

        let doc_terms = Self::tokenize(doc);
        let doc_len = doc_terms.len() as f32;
        if doc_len == 0.0 {
            return 0.0;
        }

        // Approximate average document length as 100 words
        let avgdl = 100.0_f32;

        let mut score = 0.0_f32;
        for qt in &query_terms {
            let tf = doc_terms.iter().filter(|t| *t == qt).count() as f32;
            if tf > 0.0 {
                // BM25 numerator/denominator
                let numerator = tf * (self.k1 + 1.0);
                let denominator = tf + self.k1 * (1.0 - self.b + self.b * doc_len / avgdl);
                score += numerator / denominator;
            }
        }

        // Normalise to [0, 1] range relative to a perfect match of all terms
        let max_possible = query_terms.len() as f32 * (self.k1 + 1.0);
        (score / max_possible).min(1.0)
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use wesichain_core::{Document, SearchResult};

    struct FakeRetriever(Vec<SearchResult>);

    #[async_trait]
    impl BaseRetriever for FakeRetriever {
        async fn retrieve(
            &self,
            _query: &str,
            top_k: usize,
            _filter: Option<&MetadataFilter>,
        ) -> Result<Vec<SearchResult>, RetrievalError> {
            Ok(self.0.iter().take(top_k).cloned().collect())
        }
    }

    fn result(id: &str, content: &str, score: f32) -> SearchResult {
        SearchResult {
            document: Document {
                id: id.to_string(),
                content: content.to_string(),
                metadata: Default::default(),
                embedding: None,
            },
            score,
        }
    }

    #[tokio::test]
    async fn keyword_reranker_scores_relevant_higher() {
        let reranker = KeywordReranker::new();
        let relevant = reranker
            .score("Rust async programming", "Rust is great for async programming tasks")
            .await;
        let irrelevant = reranker
            .score("Rust async programming", "The quick brown fox jumps over the lazy dog")
            .await;
        assert!(relevant > irrelevant, "relevant={relevant:.4} irrelevant={irrelevant:.4}");
    }

    #[tokio::test]
    async fn keyword_reranker_empty_query() {
        let reranker = KeywordReranker::new();
        assert_eq!(reranker.score("", "anything").await, 0.0);
    }

    #[tokio::test]
    async fn cross_encoder_reorders_results() {
        // Base retriever returns results in "wrong" order (irrelevant first)
        let base = FakeRetriever(vec![
            result("a", "The quick brown fox jumps over the lazy dog", 0.9),
            result("b", "Rust async programming is great for systems code", 0.5),
        ]);
        // oversample_factor=2 so we fetch 2 candidates for top_k=1
        let retriever = CrossEncoderRetriever::new(base, KeywordReranker::new(), 2);
        let results = retriever
            .retrieve("Rust async programming", 1, None)
            .await
            .unwrap();
        // After re-ranking, doc "b" (Rust-related) should rank first
        assert_eq!(results[0].document.id, "b");
    }

    #[tokio::test]
    async fn cross_encoder_respects_top_k() {
        let base = FakeRetriever(vec![
            result("a", "hello world", 0.8),
            result("b", "hello rust", 0.7),
            result("c", "hello async", 0.6),
        ]);
        let retriever = CrossEncoderRetriever::new(base, KeywordReranker::new(), 3);
        let results = retriever.retrieve("hello", 2, None).await.unwrap();
        assert_eq!(results.len(), 2);
    }
}
