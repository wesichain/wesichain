use wesichain_core::{
    Runnable, WesichainError,
};
use wesichain_core::state::{StateSchema, StateUpdate};
use wesichain_graph::{
    GraphBuilder, GraphState, InMemoryCheckpointer,
};
use serde::{Deserialize, Serialize};
use async_trait::async_trait;
use tokio::time::{sleep, Duration};

// --- 1. Define State ---
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
struct WorkflowState {
    step: usize,
    data: Vec<String>,
}

impl StateSchema for WorkflowState {
    type Update = WorkflowState;
    fn apply(current: &Self, update: WorkflowState) -> Self {
        let mut new = current.clone();
        if update.step > 0 {
            new.step = update.step;
        }
        if !update.data.is_empty() {
            new.data.extend(update.data);
        }
        new
    }
}

// --- 2. Nodes ---

#[derive(Clone)]
struct StepNode {
    name: String,
}

#[async_trait]
impl Runnable<GraphState<WorkflowState>, StateUpdate<WorkflowState>> for StepNode {
    async fn invoke(&self, input: GraphState<WorkflowState>) -> Result<StateUpdate<WorkflowState>, WesichainError> {
        println!("[{}] Processing step {}", self.name, input.data.step);
        sleep(Duration::from_millis(100)).await;
        
        Ok(StateUpdate::new(WorkflowState {
            step: input.data.step + 1,
            data: vec![format!("Processed by {}", self.name)],
        }))
    }

    fn stream<'a>(
        &'a self,
        _input: GraphState<WorkflowState>,
    ) -> std::pin::Pin<Box<dyn futures::Stream<Item = Result<wesichain_core::StreamEvent, WesichainError>> + Send + 'a>> {
        Box::pin(futures::stream::empty())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Wesichain Resumable Workflow Example ===");

    // 1. Setup Graph with Checkpointer
    let checkpointer = InMemoryCheckpointer::default();

    let node_a = StepNode { name: "A".to_string() };
    let node_b = StepNode { name: "B".to_string() };
    let node_c = StepNode { name: "C".to_string() };

    // We want to interrupt BEFORE node B to simulate human approval
    let builder = GraphBuilder::<WorkflowState>::new()
        .add_node("A", node_a.clone())
        .add_node("B", node_b.clone())
        .add_node("C", node_c.clone())
        .add_edge("A", "B")
        .add_edge("B", "C")
        .set_entry("A")
        .with_checkpointer(checkpointer.clone(), "thread_1")
        .with_interrupt_before(vec!["B"]);

    let graph = builder.build();

    // 2. Initial Run
    println!("\n--- Run 1: Start (Should stop before B) ---");
    let initial = GraphState::new(WorkflowState {
        step: 0,
        data: vec![],
    });

    // Use invoke_graph to get GraphError
    let result = graph.invoke_graph(initial).await;
    match result {
        Ok(_) => println!("Run 1 finished unexpectedly!"),
        Err(wesichain_graph::GraphError::Interrupted) => {
            println!("Run 1 interrupted as expected.");
        }
        Err(e) => println!("Run 1 failed: {:?}", e),
    }

    // 3. Inspect Checkpoint
    println!("\n--- Inspecting Checkpoint ---");
    // Verify state at interruption
    use wesichain_core::checkpoint::HistoryCheckpointer;
    let history = checkpointer.list_checkpoints("thread_1").await.unwrap_or_default();
    
    if let Some(latest_meta) = history.first() {
        println!("Latest checkpoint metadata: {:?}", latest_meta);
    }

    // 4. Resume
    println!("\n--- Run 2: Resume (Should execute B and C) ---");
    
    // Load latest checkpoint
    use wesichain_core::checkpoint::Checkpointer;
    let latest_opt = checkpointer.load("thread_1").await?;
    let latest = latest_opt.expect("Checkpoint should exist");
    
    let resume_state = latest.state;
    let resume_queue = latest.queue; // Fixed field name "queue" vs "next_node_queue"

    println!("Resuming from queue: {:?}", resume_queue);

    let resume_options = wesichain_graph::ExecutionOptions {
        initial_queue: Some(resume_queue),
        initial_step: Some(latest.step as usize),
        ..Default::default()
    };
    
    // Rebuild graph for resume
    let builder_resume = GraphBuilder::<WorkflowState>::new()
        .add_node("A", node_a)
        .add_node("B", node_b)
        .add_node("C", node_c)
        .add_edge("A", "B")
        .add_edge("B", "C")
        .set_entry("A")
        .with_checkpointer(checkpointer.clone(), "thread_1");

    let graph_resume = builder_resume.build();

    let result_2 = graph_resume.invoke_graph_with_options(
        resume_state,
        resume_options
    ).await?;
    
    println!("Run 2 Finished: {:?}", result_2.data.data);
    
    Ok(())
}
