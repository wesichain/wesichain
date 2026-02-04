// Run: cargo run -p wesichain-graph --example react_agent

use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use wesichain_agent::Tool;
use wesichain_core::{Runnable, StreamEvent, Value, WesichainError};
use wesichain_graph::{ExecutionConfig, GraphBuilder, GraphError, GraphState, HasToolCalls, StateSchema, StateUpdate, ToolNode};
use wesichain_llm::{Message, ToolCall};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct AgentState {
    input: String,
    tool_calls: Vec<ToolCall>,
    tool_results: Vec<Message>,
    answer: Option<String>,
}

impl StateSchema for AgentState {
    fn merge(current: &Self, update: Self) -> Self {
        let input = if update.input.is_empty() {
            current.input.clone()
        } else {
            update.input
        };

        let tool_calls = update.tool_calls;

        let tool_results = if update.tool_results.is_empty() {
            current.tool_results.clone()
        } else {
            update.tool_results
        };

        let answer = if update.answer.is_some() {
            update.answer
        } else {
            current.answer.clone()
        };

        Self {
            input,
            tool_calls,
            tool_results,
            answer,
        }
    }
}

impl HasToolCalls for AgentState {
    fn tool_calls(&self) -> &Vec<ToolCall> {
        &self.tool_calls
    }

    fn push_tool_result(&mut self, message: Message) {
        self.tool_results.push(message);
    }
}

struct Agent;

#[async_trait]
impl Runnable<GraphState<AgentState>, StateUpdate<AgentState>> for Agent {
    async fn invoke(
        &self,
        input: GraphState<AgentState>,
    ) -> Result<StateUpdate<AgentState>, WesichainError> {
        if input.data.tool_results.is_empty() {
            let call = ToolCall {
                id: "call-1".to_string(),
                name: "echo".to_string(),
                args: serde_json::json!({"text": input.data.input}),
            };
            Ok(StateUpdate::new(AgentState {
                input: String::new(),
                tool_calls: vec![call],
                tool_results: Vec::new(),
                answer: None,
            }))
        } else {
            let last = input.data.tool_results.last().cloned();
            Ok(StateUpdate::new(AgentState {
                input: String::new(),
                tool_calls: Vec::new(),
                tool_results: Vec::new(),
                answer: Some(format!(
                    "Tool said: {}",
                    last.map(|msg| msg.content).unwrap_or_default()
                )),
            }))
        }
    }

    fn stream(
        &self,
        _input: GraphState<AgentState>,
    ) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        stream::empty().boxed()
    }
}

struct Final;

#[async_trait]
impl Runnable<GraphState<AgentState>, StateUpdate<AgentState>> for Final {
    async fn invoke(
        &self,
        input: GraphState<AgentState>,
    ) -> Result<StateUpdate<AgentState>, WesichainError> {
        Ok(StateUpdate::new(input.data))
    }

    fn stream(
        &self,
        _input: GraphState<AgentState>,
    ) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        stream::empty().boxed()
    }
}

#[derive(Default)]
struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }

    fn description(&self) -> &str {
        "echo input"
    }

    fn schema(&self) -> Value {
        serde_json::json!({"type": "object"})
    }

    async fn call(&self, input: Value) -> Result<Value, WesichainError> {
        Ok(input)
    }
}

#[tokio::main]
async fn main() -> Result<(), GraphError> {
    let tool_node = ToolNode::new(vec![Arc::new(EchoTool::default())]);
    let graph = GraphBuilder::new()
        .add_node("agent", Agent)
        .add_node("tools", tool_node)
        .add_node("final", Final)
        .add_conditional_edge("agent", |state: &GraphState<AgentState>| {
            if state.data.tool_calls.is_empty() {
                "final".to_string()
            } else {
                "tools".to_string()
            }
        })
        .add_edge("tools", "agent")
        .with_default_config(ExecutionConfig {
            max_steps: Some(5),
            cycle_detection: false,
            cycle_window: 5,
        })
        .set_entry("agent")
        .build();

    let state = GraphState::new(AgentState {
        input: "hello".to_string(),
        ..AgentState::default()
    });
    let out = graph.invoke_graph(state).await?;
    println!("{}", out.data.answer.unwrap_or_default());
    Ok(())
}
