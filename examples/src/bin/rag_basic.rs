use std::sync::Arc;
use wesichain_core::{
    CallbackHandler, CallbackManager, RunContext,
    Runnable, WesichainError, RunConfig,
};
use wesichain_core::state::{StateSchema, StateUpdate};
use wesichain_graph::{
    GraphBuilder, GraphState,
};
use serde::{Deserialize, Serialize};
use async_trait::async_trait;
use serde_json::Value;

// --- 1. Define State ---
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
struct RagState {
    query: String,
    docs: Vec<String>,
    answer: Option<String>,
}

impl StateSchema for RagState {
    type Update = RagState;
    fn apply(current: &Self, update: RagState) -> Self {
        let mut new = current.clone();
        if !update.query.is_empty() {
            new.query = update.query;
        }
        if !update.docs.is_empty() {
            new.docs = update.docs; // Overwrite docs? Or append? Let's overwrite for simple retriever
        }
        if let Some(answer) = update.answer {
            new.answer = Some(answer);
        }
        new
    }
}

// --- 2. Nodes ---

#[derive(Clone)]
struct MockRetriever;

#[async_trait]
impl Runnable<GraphState<RagState>, StateUpdate<RagState>> for MockRetriever {
    async fn invoke(&self, input: GraphState<RagState>) -> Result<StateUpdate<RagState>, WesichainError> {
        println!("Retrieving docs for query: {}", input.data.query);
        // Mock retrieval
        let docs = vec![
            "Doc 1: Wesichain is a Rust-native framework.".to_string(),
            "Doc 2: Observability is unified in v0.2.".to_string(),
        ];
        Ok(StateUpdate::new(RagState {
            docs,
            ..Default::default()
        }))
    }

    fn stream<'a>(
        &'a self,
        _input: GraphState<RagState>,
    ) -> std::pin::Pin<Box<dyn futures::Stream<Item = Result<wesichain_core::StreamEvent, WesichainError>> + Send + 'a>> {
        Box::pin(futures::stream::empty())
    }
}

#[derive(Clone)]
struct MockGenerator;

#[async_trait]
impl Runnable<GraphState<RagState>, StateUpdate<RagState>> for MockGenerator {
    async fn invoke(&self, input: GraphState<RagState>) -> Result<StateUpdate<RagState>, WesichainError> {
        let context = input.data.docs.join("\n");
        println!("Generating answer based on context len: {}", context.len());
        
        // Mock generation
        let answer = format!("Based on {}, Wesichain v0.2 has unified observability.", context);
        
        Ok(StateUpdate::new(RagState {
            answer: Some(answer),
            ..Default::default()
        }))
    }

    fn stream<'a>(
        &'a self,
        _input: GraphState<RagState>,
    ) -> std::pin::Pin<Box<dyn futures::Stream<Item = Result<wesichain_core::StreamEvent, WesichainError>> + Send + 'a>> {
        Box::pin(futures::stream::empty())
    }
}

// --- 3. Callback for Streaming (Mock) ---
struct StreamingCallback;

#[async_trait]
impl CallbackHandler for StreamingCallback {
    async fn on_start(&self, _ctx: &RunContext, _inputs: &Value) {}
    async fn on_end(&self, _ctx: &RunContext, _outputs: &Value, _duration_ms: u128) {}
    async fn on_error(&self, _ctx: &RunContext, _error: &Value, _duration_ms: u128) {}

    async fn on_event(&self, _ctx: &RunContext, event: &str, data: &Value) {
        if event == "llm_token" {
            print!("{}", data.as_str().unwrap_or(""));
            use std::io::Write;
            std::io::stdout().flush().unwrap();
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Wesichain RAG Example ===");

    // Build Graph
    let retriever = MockRetriever;
    let generator = MockGenerator;

    let builder = GraphBuilder::<RagState>::new()
        .add_node("retrieve", retriever)
        .add_node("generate", generator)
        .add_edge("retrieve", "generate")
        .set_entry("retrieve");

    let graph = builder.build();

    // Callbacks
    let mut manager = CallbackManager::default();
    manager.add_handler(Arc::new(StreamingCallback));

    let run_config = RunConfig {
        callbacks: Some(manager),
        ..Default::default()
    };

    // Run
    let initial = GraphState::new(RagState {
        query: "What's new in Wesichain?".to_string(),
        ..Default::default()
    });

    let result = graph.invoke_graph_with_options(
        initial,
        wesichain_graph::ExecutionOptions {
            run_config: Some(run_config),
            ..Default::default()
        }
    ).await?;

    println!("\n\nFinal Answer: {:?}", result.data.answer);

    Ok(())
}
