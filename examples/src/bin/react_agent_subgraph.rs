use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use wesichain_core::{
    CallbackHandler, CallbackManager, LlmRequest, LlmResponse, ReActStep, Role, RunContext,
    Runnable, ScratchpadState, Tool, ToolCall, ToolCallingLlm, Value, WesichainError,
};
use wesichain_graph::{
    react_subgraph::ToolFailurePolicy, GraphState, ReActGraphBuilder, StateSchema,
};

// --- 1. Define State ---
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
struct AgentState {
    input: String,
    scratchpad: Vec<ReActStep>,
    final_output: Option<String>,
    #[serde(default)]
    iterations: u32,
}

impl StateSchema for AgentState {
    type Update = AgentState;
    fn apply(current: &Self, update: AgentState) -> Self {
        let mut new = current.clone();
        if !update.input.is_empty() {
            new.input = update.input;
        }
        if !update.scratchpad.is_empty() {
            new.scratchpad.extend(update.scratchpad);
        }
        if let Some(final_output) = update.final_output {
            new.final_output = Some(final_output);
        }
        new.iterations = update.iterations;
        new
    }
}

// Implement traits required by ReActGraphBuilder
impl wesichain_core::HasUserInput for AgentState {
    fn user_input(&self) -> &str {
        &self.input
    }
}

impl wesichain_core::HasFinalOutput for AgentState {
    fn final_output(&self) -> Option<&str> {
        self.final_output.as_deref()
    }
    fn set_final_output(&mut self, output: String) {
        self.final_output = Some(output);
    }
}

impl ScratchpadState for AgentState {
    fn scratchpad(&self) -> &Vec<ReActStep> {
        &self.scratchpad
    }
    fn scratchpad_mut(&mut self) -> &mut Vec<ReActStep> {
        &mut self.scratchpad
    }
    fn ensure_scratchpad(&mut self) {
        // Vec is already initialized
    }
    fn iteration_count(&self) -> u32 {
        self.iterations
    }
    fn increment_iteration(&mut self) {
        self.iterations += 1;
    }
}

// --- 2. Define Tools ---

#[derive(Clone)]
struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str {
        "calculator"
    }
    fn description(&self) -> &str {
        "Useful for performing basic arithmetic. Input should be a math expression string."
    }
    fn schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "expression": { "type": "string" }
            }
        })
    }
    async fn invoke(&self, args: Value) -> Result<Value, wesichain_core::ToolError> {
        let expr = args["expression"].as_str().unwrap_or("0");
        // Simplified eval
        Ok(Value::String(format!("Calculated: {}", expr)))
    }
}

#[derive(Clone)]
struct TimeTool;

#[async_trait]
impl Tool for TimeTool {
    fn name(&self) -> &str {
        "time"
    }
    fn description(&self) -> &str {
        "Returns the current time."
    }
    fn schema(&self) -> Value {
        serde_json::json!({})
    }
    async fn invoke(&self, _args: Value) -> Result<Value, wesichain_core::ToolError> {
        Ok(Value::String("It is currently 12:00 PM".to_string()))
    }
}

// --- 3. Mock LLM ---
// Simulates an LLM that calls "time" then answers.

#[derive(Clone)]
struct MockAgentLlm;

#[async_trait]
impl Runnable<LlmRequest, LlmResponse> for MockAgentLlm {
    async fn invoke(&self, input: LlmRequest) -> Result<LlmResponse, WesichainError> {
        // Simple deterministic logic based on last message
        let last_msg = input.messages.last().unwrap();

        match last_msg.role {
            Role::User => {
                // First turn: call time tool
                Ok(LlmResponse {
                    content: "".to_string(),
                    tool_calls: vec![ToolCall {
                        id: "call_1".to_string(),
                        name: "time".to_string(),
                        args: serde_json::json!({}),
                    }],
                })
            }
            Role::Tool => {
                // Tool output received: final answer
                Ok(LlmResponse {
                    content: "The current time is 12:00 PM.".to_string(),
                    tool_calls: vec![],
                })
            }
            _ => {
                // Fallback
                Ok(LlmResponse {
                    content: "I don't know.".to_string(),
                    tool_calls: vec![],
                })
            }
        }
    }

    fn stream<'a>(
        &'a self,
        _input: LlmRequest,
    ) -> std::pin::Pin<
        Box<
            dyn futures::Stream<Item = Result<wesichain_core::StreamEvent, WesichainError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(futures::stream::empty())
    }
}

#[async_trait]
impl ToolCallingLlm for MockAgentLlm {}

// --- 4. Custom Callback Handler for Observability ---
struct StdoutCallbackHandler;

#[async_trait]
impl CallbackHandler for StdoutCallbackHandler {
    async fn on_start(&self, ctx: &RunContext, _inputs: &Value) {
        println!(
            "[CALLBACK] Starting node: {} (Type: {:?})",
            ctx.name, ctx.run_type
        );
    }
    async fn on_end(&self, ctx: &RunContext, outputs: &Value, duration_ms: u128) {
        println!(
            "[CALLBACK] Finished node: {} in {}ms. Output len: {}",
            ctx.name,
            duration_ms,
            outputs.to_string().len()
        );
    }
    async fn on_event(&self, _ctx: &RunContext, event: &str, data: &Value) {
        println!("[CALLBACK] Event: {} -> {}", event, data);
    }
    async fn on_error(&self, ctx: &RunContext, error: &Value, _duration_ms: u128) {
        println!("[CALLBACK] Error in {}: {}", ctx.name, error);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Wesichain ReAct Agent Example ===");

    // Setup Tools
    let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(CalculatorTool), Arc::new(TimeTool)];

    // Setup Builder
    let llm = Arc::new(MockAgentLlm);
    let builder = ReActGraphBuilder::new()
        .llm(llm)
        .tools(tools.clone())
        .tool_failure_policy(ToolFailurePolicy::AppendErrorAndContinue);

    let graph = builder.build::<AgentState>()?;

    // Setup Callbacks
    let mut manager = CallbackManager::default();

    manager.add_handler(Arc::new(StdoutCallbackHandler));

    let run_config = wesichain_core::RunConfig {
        callbacks: Some(manager),
        ..Default::default()
    };

    // Initial State
    let initial = GraphState::new(AgentState {
        input: "What time is it?".to_string(),
        scratchpad: vec![],
        final_output: None,
        iterations: 0,
    });

    println!("\nInvoking graph...");
    let result = graph
        .invoke_graph_with_options(
            initial,
            wesichain_graph::ExecutionOptions {
                run_config: Some(run_config),
                ..Default::default()
            },
        )
        .await?;

    println!("\nFinal Result: {:?}", result.data.final_output);
    println!("Scratchpad steps: {}", result.data.scratchpad.len());

    Ok(())
}
