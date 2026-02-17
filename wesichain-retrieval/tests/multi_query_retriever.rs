use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use wesichain_core::{
    Document, LlmRequest, LlmResponse, MetadataFilter, Runnable, SearchResult, StreamEvent,
    WesichainError,
};
use wesichain_retrieval::{BaseRetriever, MultiQueryRetriever, RetrievalError};

/// Mock LLM that returns predefined query variants
#[derive(Clone)]
struct MockLlm {
    responses: Arc<Mutex<Vec<String>>>,
}

impl MockLlm {
    fn new(responses: Vec<String>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses)),
        }
    }
}

#[async_trait]
impl Runnable<LlmRequest, LlmResponse> for MockLlm {
    async fn invoke(&self, _input: LlmRequest) -> Result<LlmResponse, WesichainError> {
        let mut responses = self.responses.lock().await;
        if let Some(response) = responses.pop() {
            Ok(LlmResponse {
                content: response,
                tool_calls: vec![],
            })
        } else {
            Err(WesichainError::Custom("No more responses".to_string()))
        }
    }

    fn stream(
        &self,
        _input: LlmRequest,
    ) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }

    fn to_serializable(&self) -> Option<wesichain_core::SerializableRunnable> {
        None
    }
}

use futures::StreamExt;

/// Mock retriever that returns controlled results with potential overlap
#[derive(Clone)]
struct MockRetriever {
    results_map: Arc<HashMap<String, Vec<SearchResult>>>,
}

impl MockRetriever {
    fn new(results_map: HashMap<String, Vec<SearchResult>>) -> Self {
        Self {
            results_map: Arc::new(results_map),
        }
    }
}

#[async_trait]
impl BaseRetriever for MockRetriever {
    async fn retrieve(
        &self,
        query: &str,
        top_k: usize,
        _filter: Option<&MetadataFilter>,
    ) -> Result<Vec<SearchResult>, RetrievalError> {
        if let Some(results) = self.results_map.get(query) {
            Ok(results.iter().take(top_k).cloned().collect())
        } else {
            Ok(vec![])
        }
    }
}

fn create_search_result(id: &str, content: &str, score: f32) -> SearchResult {
    SearchResult {
        document: Document {
            id: id.to_string(),
            content: content.to_string(),
            metadata: HashMap::new(),
            embedding: None,
        },
        score,
    }
}

#[tokio::test]
async fn test_multi_query_deduplication() {
    // Setup: Mock LLM returns 2 query variants
    let query_variants = r#"["what is AI?", "artificial intelligence definition"]"#;
    let mock_llm = MockLlm::new(vec![query_variants.to_string()]);

    // Setup: Mock retriever with overlapping results
    let mut results_map = HashMap::new();

    // Original query returns doc1 and doc2
    results_map.insert(
        "what is machine learning?".to_string(),
        vec![
            create_search_result("doc1", "ML is...", 0.9),
            create_search_result("doc2", "Machine learning...", 0.8),
        ],
    );

    // Variant 1 returns doc2 (duplicate) and doc3
    results_map.insert(
        "what is AI?".to_string(),
        vec![
            create_search_result("doc2", "Machine learning...", 0.85),
            create_search_result("doc3", "AI definition...", 0.75),
        ],
    );

    // Variant 2 returns doc1 (duplicate) and doc4
    results_map.insert(
        "artificial intelligence definition".to_string(),
        vec![
            create_search_result("doc1", "ML is...", 0.88),
            create_search_result("doc4", "Artificial intelligence...", 0.7),
        ],
    );

    let mock_retriever = MockRetriever::new(results_map);

    // Create MultiQueryRetriever
    let multi_query = MultiQueryRetriever::new(mock_llm, mock_retriever).with_num_queries(2);

    // Execute
    let results = multi_query
        .retrieve("what is machine learning?", 10, None)
        .await
        .unwrap();

    // Should have 4 unique documents (doc1, doc2, doc3, doc4)
    assert_eq!(results.len(), 4);

    // Check all unique IDs are present
    let ids: Vec<_> = results.iter().map(|r| r.document.id.as_str()).collect();
    assert!(ids.contains(&"doc1"));
    assert!(ids.contains(&"doc2"));
    assert!(ids.contains(&"doc3"));
    assert!(ids.contains(&"doc4"));
}

#[tokio::test]
async fn test_multi_query_partial_failure() {
    // Mock LLM returns 2 variants
    let query_variants = r#"["query a", "query b"]"#;
    let mock_llm = MockLlm::new(vec![query_variants.to_string()]);

    // Mock retriever only has results for original and one variant
    let mut results_map = HashMap::new();
    results_map.insert(
        "original".to_string(),
        vec![create_search_result("doc1", "Content 1", 0.9)],
    );
    results_map.insert(
        "query a".to_string(),
        vec![create_search_result("doc2", "Content 2", 0.8)],
    );
    // "query b" will return empty results (simulating partial failure)

    let mock_retriever = MockRetriever::new(results_map);
    let multi_query = MultiQueryRetriever::new(mock_llm, mock_retriever);

    // Should succeed with partial results
    let results = multi_query.retrieve("original", 10, None).await.unwrap();

    // Should have 2 results (from original and query a)
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_multi_query_respects_top_k() {
    let query_variants = r#"["variant1"]"#;
    let mock_llm = MockLlm::new(vec![query_variants.to_string()]);

    let mut results_map = HashMap::new();
    results_map.insert(
        "query".to_string(),
        vec![
            create_search_result("doc1", "1", 1.0),
            create_search_result("doc2", "2", 0.9),
            create_search_result("doc3", "3", 0.8),
        ],
    );
    results_map.insert(
        "variant1".to_string(),
        vec![
            create_search_result("doc4", "4", 0.7),
            create_search_result("doc5", "5", 0.6),
        ],
    );

    let mock_retriever = MockRetriever::new(results_map);
    let multi_query = MultiQueryRetriever::new(mock_llm, mock_retriever);

    // Request only top 3 results
    let results = multi_query.retrieve("query", 3, None).await.unwrap();

    // Should return exactly 3 unique results
    assert_eq!(results.len(), 3);
}
