use std::collections::HashSet;

use async_trait::async_trait;
use wesichain_core::{LlmRequest, LlmResponse, Message, MetadataFilter, Role, Runnable, SearchResult};

use crate::{BaseRetriever, RetrievalError};

const QUERY_GENERATION_PROMPT: &str = r#"You are an AI assistant helping to improve document retrieval.

Given the following question, generate {num_queries} different search queries that approach the topic from different angles. Each query should:
- Use different terminology or phrasing
- Focus on a distinct aspect of the question
- Help retrieve documents that might not match the original question directly

Original question: {question}

Generate exactly {num_queries} alternative queries as a JSON array of strings. Do not include the original question.

Example format: ["query 1", "query 2", "query 3"]
"#;

/// Multi-query retriever that generates multiple query variants and deduplicates results.
///
/// This retriever uses an LLM to generate alternative phrasings of the query,
/// executes all queries in parallel, and returns deduplicated results.
pub struct MultiQueryRetriever<L, R> {
    llm: L,
    base_retriever: R,
    num_queries: usize,
    prompt_template: String,
}

impl<L, R> MultiQueryRetriever<L, R>
where
    L: Runnable<LlmRequest, LlmResponse> + Clone + Send + Sync,
    R: BaseRetriever + Clone + Send + Sync,
{
    /// Create a new MultiQueryRetriever with default settings.
    ///
    /// Uses the default prompt template with 3 query variants.
    pub fn new(llm: L, base_retriever: R) -> Self {
        Self {
            llm,
            base_retriever,
            num_queries: 3,
            prompt_template: QUERY_GENERATION_PROMPT.to_string(),
        }
    }

    /// Set the number of query variants to generate.
    pub fn with_num_queries(mut self, num_queries: usize) -> Self {
        self.num_queries = num_queries;
        self
    }

    /// Set a custom prompt template for query generation.
    ///
    /// The template should include `{num_queries}` and `{question}` placeholders.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let custom_prompt = r#"Generate {num_queries} search queries for: {question}
    /// Focus on medical terminology and clinical contexts.
    /// Return as JSON array: ["query1", "query2"]"#;
    ///
    /// let retriever = MultiQueryRetriever::new(llm, base)
    ///     .with_prompt(custom_prompt.to_string());
    /// ```
    pub fn with_prompt(mut self, prompt_template: String) -> Self {
        self.prompt_template = prompt_template;
        self
    }

    /// Generate query variants using the LLM.
    async fn generate_queries(&self, query: &str) -> Result<Vec<String>, RetrievalError> {
        let prompt = self
            .prompt_template
            .replace("{num_queries}", &self.num_queries.to_string())
            .replace("{question}", query);

        let request = LlmRequest {
            model: String::new(), // Model will be set by LLM implementation
            messages: vec![Message {
                role: Role::User,
                content: prompt,
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools: vec![],
        };

        let response = self
            .llm
            .invoke(request)
            .await
            .map_err(|e| RetrievalError::Other(format!("LLM query generation failed: {}", e)))?;

        // Parse JSON response
        let content = response.content.trim();
        let queries: Vec<String> = serde_json::from_str(content)
            .map_err(|e| RetrievalError::Other(format!("Failed to parse query variants: {}", e)))?;

        // Add original query
        let mut all_queries = vec![query.to_string()];
        all_queries.extend(queries);

        Ok(all_queries)
    }

    /// Execute all queries in parallel and collect results.
    async fn parallel_retrieve(
        &self,
        queries: &[String],
        top_k: usize,
        filter: Option<&MetadataFilter>,
    ) -> Vec<SearchResult> {
        let futures: Vec<_> = queries
            .iter()
            .map(|q| {
                let retriever = self.base_retriever.clone();
                let query = q.clone();
                async move { (query.clone(), retriever.retrieve(&query, top_k, filter).await) }
            })
            .collect();

        let results = futures::future::join_all(futures).await;

        let mut all_docs = Vec::new();
        for (query, result) in results {
            match result {
                Ok(docs) => all_docs.extend(docs),
                Err(e) => {
                    // Log warning but continue with partial results
                    eprintln!("Query '{}' failed: {}", query, e);
                }
            }
        }

        all_docs
    }

    /// Deduplicate results by document ID.
    fn deduplicate(&self, results: Vec<SearchResult>) -> Vec<SearchResult> {
        let mut seen_ids = HashSet::new();
        let mut unique_results = Vec::new();

        for result in results {
            if seen_ids.insert(result.document.id.clone()) {
                unique_results.push(result);
            }
        }

        unique_results
    }
}

#[async_trait]
impl<L, R> BaseRetriever for MultiQueryRetriever<L, R>
where
    L: Runnable<LlmRequest, LlmResponse> + Clone + Send + Sync,
    R: BaseRetriever + Clone + Send + Sync,
{
    async fn retrieve(
        &self,
        query: &str,
        top_k: usize,
        filter: Option<&MetadataFilter>,
    ) -> Result<Vec<SearchResult>, RetrievalError> {
        // 1. Generate query variants
        let queries = self.generate_queries(query).await?;

        // 2. Execute queries in parallel
        let all_results = self.parallel_retrieve(&queries, top_k, filter).await;

        // 3. Check if we got any results
        if all_results.is_empty() {
            return Err(RetrievalError::Other(
                "All retrieval queries failed or returned no results".to_string(),
            ));
        }

        // 4. Deduplicate by document ID
        let unique_results = self.deduplicate(all_results);

        // 5. Return top-k unique results
        Ok(unique_results.into_iter().take(top_k).collect())
    }
}
