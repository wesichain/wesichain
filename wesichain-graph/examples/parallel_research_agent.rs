use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tokio::time::sleep;

use wesichain_core::WesichainError;
use wesichain_graph::{
    state::{Append, Overwrite, Reducer},
    ExecutionConfig, GraphBuilder, GraphContext, GraphNode, GraphState, StateSchema, StateUpdate,
    END,
};

// --- State ---
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct ResearchState {
    query: String, // Input query
    #[serde(default)]
    plan: Vec<String>, // Which tools to run
    #[serde(default)]
    findings: Vec<String>, // Accumulated findings
}

impl StateSchema for ResearchState {
    type Update = Self;

    fn apply(current: &Self, update: Self) -> Self {
        let query = Overwrite.reduce(current.query.clone(), update.query);
        let plan = Overwrite.reduce(current.plan.clone(), update.plan);
        let findings = Append.reduce(current.findings.clone(), update.findings);

        ResearchState {
            query,
            plan,
            findings,
        }
    }
}

// --- Nodes ---

// Planner: Decides which research tools to use based on query.
// For this demo, we hardcode it to use all 3 if query contains "fusion".
struct PlannerNode;

#[async_trait::async_trait]
impl GraphNode<ResearchState> for PlannerNode {
    async fn invoke_with_context(
        &self,
        input: GraphState<ResearchState>,
        _context: &GraphContext,
    ) -> Result<StateUpdate<ResearchState>, WesichainError> {
        println!("Planner: Analyzing query '{}'...", input.data.query);
        sleep(Duration::from_millis(50)).await; // Thinking time

        let plan = vec![
            "search_web".to_string(),
            "search_arxiv".to_string(),
            "search_wiki".to_string(),
        ];
        println!("Planner: Decided to run: {:?}", plan);

        // Update plan in state (optional, just for record)
        Ok(StateUpdate::new(ResearchState {
            plan,
            ..Default::default()
        }))
    }
}

// Mock Research Tools
struct WebSearchNode;
#[async_trait::async_trait]
impl GraphNode<ResearchState> for WebSearchNode {
    async fn invoke_with_context(
        &self,
        _input: GraphState<ResearchState>,
        _: &GraphContext,
    ) -> Result<StateUpdate<ResearchState>, WesichainError> {
        println!("  -> WebSearch: Searching...");
        sleep(Duration::from_millis(200)).await; // Latency
        println!("  <- WebSearch: Found 'Fusion breakthrough 2025'");
        Ok(StateUpdate::new(ResearchState {
            findings: vec!["Web: Fusion breakthrough 2025".to_string()],
            ..Default::default()
        }))
    }
}

struct ArxivSearchNode;
#[async_trait::async_trait]
impl GraphNode<ResearchState> for ArxivSearchNode {
    async fn invoke_with_context(
        &self,
        _input: GraphState<ResearchState>,
        _: &GraphContext,
    ) -> Result<StateUpdate<ResearchState>, WesichainError> {
        println!("  -> ArxivSearch: Searching...");
        sleep(Duration::from_millis(300)).await; // Slower latency
        println!("  <- ArxivSearch: Found 'Magnetic confinement stability analysis'");
        Ok(StateUpdate::new(ResearchState {
            findings: vec!["Arxiv: Magnetic confinement stability analysis".to_string()],
            ..Default::default()
        }))
    }
}

struct WikiSearchNode;
#[async_trait::async_trait]
impl GraphNode<ResearchState> for WikiSearchNode {
    async fn invoke_with_context(
        &self,
        _input: GraphState<ResearchState>,
        _: &GraphContext,
    ) -> Result<StateUpdate<ResearchState>, WesichainError> {
        println!("  -> WikiSearch: Searching...");
        sleep(Duration::from_millis(150)).await; // Fast latency
        println!("  <- WikiSearch: Found 'Fusion power overview'");
        Ok(StateUpdate::new(ResearchState {
            findings: vec!["Wiki: Fusion power overview".to_string()],
            ..Default::default()
        }))
    }
}

// Editor: Aggregates findings
struct EditorNode;
#[async_trait::async_trait]
impl GraphNode<ResearchState> for EditorNode {
    async fn invoke_with_context(
        &self,
        input: GraphState<ResearchState>,
        _context: &GraphContext,
    ) -> Result<StateUpdate<ResearchState>, WesichainError> {
        println!(
            "Editor: Reviewing {} findings...",
            input.data.findings.len()
        );
        // Simple aggregation logic
        let summary = format!("Comparison of {} sources.", input.data.findings.len());
        println!("Editor: Final summary: {}", summary);

        // In a real agent, this would be the final answer.
        Ok(StateUpdate::new(ResearchState::default()))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Parallel Research Agent Demo ===");
    println!("Demonstrating conditional fan-out and parallel execution with state merging.\n");

    let builder = GraphBuilder::<ResearchState>::new()
        .with_default_config(ExecutionConfig {
            cycle_detection: false,
            ..Default::default()
        })
        .add_node("planner", PlannerNode)
        .add_node("search_web", WebSearchNode)
        .add_node("search_arxiv", ArxivSearchNode)
        .add_node("search_wiki", WikiSearchNode)
        .add_node("editor", EditorNode)
        .set_entry("planner")
        // Conditional Edge from Planner -> [Web, Arxiv, Wiki]
        .add_conditional_edge("planner", |state: &GraphState<ResearchState>| {
            // In a real app, this would read state.plan
            // Here we just return the hardcoded branches for demo purposes
            // or read from the plan we just set?
            // Since invoke returns update, valid state in condition has the update merging.
            // But invoke returns update, and it is applied.
            // The condition closure receives the UPDATED state.
            // So we can read state.plan.
            if !state.data.plan.is_empty() {
                state.data.plan.clone()
            } else {
                vec![END.to_string()]
            }
        })
        // Fan-in: All tools -> Editor
        .add_edge("search_web", "editor")
        .add_edge("search_arxiv", "editor")
        .add_edge("search_wiki", "editor")
        .add_edge("editor", END);

    let graph = builder.build();

    let initial_state = ResearchState {
        query: "What is the status of fusion power?".to_string(),
        ..Default::default()
    };

    println!("Starting execution...");
    let start = Instant::now();
    let result = graph
        .invoke(GraphState::new(initial_state))
        .await
        .expect("Graph failed");
    let duration = start.elapsed();

    println!("\n=== Results ===");
    println!("Total Duration: {:?}", duration);
    println!("Findings Collected: {}", result.data.findings.len());
    for finding in &result.data.findings {
        println!(" - {}", finding);
    }

    // Validation logic for the demo
    // Max latency is 300ms (Arxiv) + 50ms (Planner) + overhead.
    // If sequential: 50 + 200 + 300 + 150 = 700ms.
    // If parallel: 50 + 300 = 350ms.
    if duration < Duration::from_millis(500) {
        println!("\n✅ SUCCESS: Execution was PARALLEL (took < 500ms).");
    } else {
        println!("\n❌ FAILURE: Execution was SEQUENTIAL (took >= 500ms).");
    }

    if result.data.findings.len() == 3 {
        println!("✅ SUCCESS: All 3 findings merged correctly.");
    } else {
        println!(
            "❌ FAILURE: Missing findings. Expected 3, got {}.",
            result.data.findings.len()
        );
    }

    Ok(())
}
