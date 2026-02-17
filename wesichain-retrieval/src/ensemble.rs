use std::collections::HashMap;

use async_trait::async_trait;
use wesichain_core::{Document, MetadataFilter, SearchResult};

use crate::{BaseRetriever, RetrievalError};

/// Ensemble retriever that combines multiple retrievers with weighted scoring.
///
/// This retriever runs multiple retrievers in parallel and merges results using
/// weighted Reciprocal Rank Fusion (RRF) or simple weighted averaging of scores.
pub struct EnsembleRetriever<R> {
    retrievers: Vec<R>,
    weights: Vec<f32>,
    /// Constant for Reciprocal Rank Fusion formula
    rank_constant: f32,
}

impl<R> EnsembleRetriever<R>
where
    R: BaseRetriever + Clone + Send + Sync,
{
    /// Create a new EnsembleRetriever with equal weights.
    pub fn new(retrievers: Vec<R>) -> Result<Self, RetrievalError> {
        if retrievers.is_empty() {
            return Err(RetrievalError::Other(
                "EnsembleRetriever requires at least one retriever".to_string(),
            ));
        }
        
        let count = retrievers.len();
        let weights = vec![1.0 / count as f32; count];
        
        Ok(Self {
            retrievers,
            weights,
            rank_constant: 60.0,
        })
    }

    /// Set custom weights for each retriever.
    ///
    /// Weights will be normalized to sum to 1.0.
    ///
    /// # Panics
    ///
    /// Panics if weights length doesn't match retrievers length.
    pub fn with_weights(mut self, weights: Vec<f32>) -> Self {
        assert_eq!(
            weights.len(),
            self.retrievers.len(),
            "Weights length must match retrievers length"
        );
        
        // Normalize weights
        let sum: f32 = weights.iter().sum();
        self.weights = weights.iter().map(|w| w / sum).collect();
        self
    }

    /// Set the rank constant for RRF scoring (default: 60.0).
    ///
    /// Higher values give more weight to lower-ranked results.
    pub fn with_rank_constant(mut self, k: f32) -> Self {
        self.rank_constant = k;
        self
    }

    /// Compute Reciprocal Rank Fusion score for a document.
    fn compute_rrf_score(
        &self,
        doc_id: &str,
        results_per_retriever: &[Vec<SearchResult>],
    ) -> f32 {
        let mut total_score = 0.0;
        
        for (retriever_idx, results) in results_per_retriever.iter().enumerate() {
            if let Some(rank) = results.iter().position(|r| {
                // Match on doc content as ID proxy since we don't have stable IDs
                r.document.content == doc_id
            }) {
                let rrf = 1.0 / (self.rank_constant + (rank as f32 + 1.0));
                total_score += self.weights[retriever_idx] * rrf;
            }
        }
        
        total_score
    }
}

#[async_trait]
impl<R> BaseRetriever for EnsembleRetriever<R>
where
    R: BaseRetriever + Clone + Send + Sync + 'static,
{
    async fn retrieve(
        &self,
        query: &str,
        top_k: usize,
        filter: Option<&MetadataFilter>,
    ) -> Result<Vec<SearchResult>, RetrievalError> {
        // Run all retrievers in parallel
        let mut handles = Vec::new();
        
        for retriever in &self.retrievers {
            let retriever = retriever.clone();
            let query = query.to_string();
            let filter = filter.cloned();
            
            handles.push(tokio::spawn(async move {
                retriever.retrieve(&query, top_k, filter.as_ref()).await
            }));
        }
        
        // Collect results
        let mut results_per_retriever = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(Ok(results)) => results_per_retriever.push(results),
                Ok(Err(e)) => return Err(e),
                Err(e) => {
                    return Err(RetrievalError::Other(format!(
                        "Retriever task failed: {}",
                        e
                    )))
                }
            }
        }
        
        // Deduplicate and score using RRF
        let mut doc_scores: HashMap<String, (f32, Document)> = HashMap::new();
        
        for results in &results_per_retriever {
            for result in results {
                let doc_id = result.document.content.clone(); // Use content as ID
                
                doc_scores.entry(doc_id).or_insert_with(|| {
                    let score = self.compute_rrf_score(&result.document.content, &results_per_retriever);
                    (score, result.document.clone())
                });
            }
        }
        
        // Sort by score and return top_k
        let mut scored_docs: Vec<(f32, Document)> = doc_scores.into_values().collect();
        scored_docs.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        
        Ok(scored_docs
            .into_iter()
            .take(top_k)
            .map(|(score, doc)| SearchResult {
                document: doc,
                score,
            })
            .collect())
    }
}
