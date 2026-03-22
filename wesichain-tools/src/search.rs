//! Web search via the Tavily API.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use wesichain_core::{ToolContext, ToolError, TypedTool};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TavilySearchArgs {
    /// Search query string
    pub query: String,
    /// Maximum results to return (default 5, max 10)
    pub max_results: Option<u8>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub content: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct TavilySearchOutput {
    pub results: Vec<SearchResult>,
}

#[derive(Clone)]
pub struct TavilySearchTool {
    api_key: String,
}

impl TavilySearchTool {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self { api_key: api_key.into() }
    }

    /// Read API key from `TAVILY_API_KEY` environment variable.
    pub fn from_env() -> Self {
        Self {
            api_key: std::env::var("TAVILY_API_KEY").unwrap_or_default(),
        }
    }
}

#[derive(Debug, Serialize)]
struct TavilyRequest<'a> {
    api_key: &'a str,
    query: &'a str,
    max_results: u8,
}

#[derive(Debug, Deserialize)]
struct TavilyResponse {
    results: Vec<TavilyResult>,
}

#[derive(Debug, Deserialize)]
struct TavilyResult {
    title: String,
    url: String,
    content: String,
}

#[async_trait::async_trait]
impl TypedTool for TavilySearchTool {
    type Args = TavilySearchArgs;
    type Output = TavilySearchOutput;
    const NAME: &'static str = "web_search";

    async fn run(&self, args: Self::Args, _ctx: ToolContext) -> Result<Self::Output, ToolError> {
        if self.api_key.is_empty() {
            return Err(ToolError::ExecutionFailed(
                "TAVILY_API_KEY is not set".to_string(),
            ));
        }

        let body = TavilyRequest {
            api_key: &self.api_key,
            query: &args.query,
            max_results: args.max_results.unwrap_or(5).min(10),
        };

        let client = reqwest::Client::new();
        let resp = client
            .post("https://api.tavily.com/search")
            .json(&body)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Tavily request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(ToolError::ExecutionFailed(format!(
                "Tavily API returned {status}: {text}"
            )));
        }

        let tavily: TavilyResponse = resp
            .json()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to parse Tavily response: {e}")))?;

        Ok(TavilySearchOutput {
            results: tavily
                .results
                .into_iter()
                .map(|r| SearchResult { title: r.title, url: r.url, content: r.content })
                .collect(),
        })
    }
}
