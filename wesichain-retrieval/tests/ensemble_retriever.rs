#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use wesichain_core::{Document, MetadataFilter, SearchResult};
    use wesichain_retrieval::{BaseRetriever, EnsembleRetriever, RetrievalError};

    // Mock retriever for testing
    #[derive(Clone)]
    struct MockRetriever {
        results: Vec<(String, f32)>, // (content, score)
    }

    #[async_trait::async_trait]
    impl BaseRetriever for MockRetriever {
        async fn retrieve(
            &self,
            _query: &str,
            top_k: usize,
            _filter: Option<&MetadataFilter>,
        ) -> Result<Vec<SearchResult>, RetrievalError> {
            Ok(self
                .results
                .iter()
                .take(top_k)
                .map(|(content, score)| SearchResult {
                    document: Document {
                        id: String::new(),
                        content: content.clone(),
                        metadata: HashMap::new(),
                        embedding: None,
                    },
                    score: *score,
                })
                .collect())
        }
    }

    #[tokio::test]
    async fn test_ensemble_basic() {
        let retriever1 = MockRetriever {
            results: vec![
                ("doc1".to_string(), 0.9),
                ("doc2".to_string(), 0.7),
                ("doc3".to_string(), 0.5),
            ],
        };

        let retriever2 = MockRetriever {
            results: vec![
                ("doc2".to_string(), 0.8),
                ("doc3".to_string(), 0.6),
                ("doc4".to_string(), 0.4),
            ],
        };

        let ensemble = EnsembleRetriever::new(vec![retriever1, retriever2]).unwrap();
        let results = ensemble.retrieve("test query", 5, None).await.unwrap();

        // Should have 4 unique documents
        assert_eq!(results.len(), 4);

        // doc2 and doc3 appear in both, so should rank higher
        let doc_contents: Vec<&str> = results
            .iter()
            .map(|r| r.document.content.as_str())
            .collect();
        assert!(doc_contents.contains(&"doc2"));
        assert!(doc_contents.contains(&"doc3"));
    }

    #[tokio::test]
    async fn test_ensemble_weighted() {
        let retriever1 = MockRetriever {
            results: vec![("doc1".to_string(), 0.9), ("doc2".to_string(), 0.5)],
        };

        let retriever2 = MockRetriever {
            results: vec![("doc2".to_string(), 0.9), ("doc3".to_string(), 0.5)],
        };

        // Give retriever2 more weight
        let ensemble = EnsembleRetriever::new(vec![retriever1, retriever2])
            .unwrap()
            .with_weights(vec![0.3, 0.7]);

        let results = ensemble.retrieve("test query", 3, None).await.unwrap();
        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn test_ensemble_top_k_limit() {
        let retriever1 = MockRetriever {
            results: vec![
                ("doc1".to_string(), 0.9),
                ("doc2".to_string(), 0.7),
                ("doc3".to_string(), 0.5),
            ],
        };

        let retriever2 = MockRetriever {
            results: vec![("doc4".to_string(), 0.8), ("doc5".to_string(), 0.6)],
        };

        let ensemble = EnsembleRetriever::new(vec![retriever1, retriever2]).unwrap();
        let results = ensemble.retrieve("test query", 2, None).await.unwrap();

        // Should return only top 2
        assert_eq!(results.len(), 2);
    }
}
