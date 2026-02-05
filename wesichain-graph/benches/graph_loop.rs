use async_trait::async_trait;
use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::runtime::Runtime;
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use wesichain_graph::{
    ExecutionConfig, FileCheckpointer, GraphBuilder, GraphState, StateSchema, StateUpdate,
};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct LoopState {
    count: i32,
}

impl StateSchema for LoopState {}

struct Inc;

#[async_trait]
impl Runnable<GraphState<LoopState>, StateUpdate<LoopState>> for Inc {
    async fn invoke(
        &self,
        input: GraphState<LoopState>,
    ) -> Result<StateUpdate<LoopState>, WesichainError> {
        Ok(StateUpdate::new(LoopState {
            count: input.data.count + 1,
        }))
    }

    fn stream(
        &self,
        _input: GraphState<LoopState>,
    ) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        stream::empty().boxed()
    }
}

struct Done;

#[async_trait]
impl Runnable<GraphState<LoopState>, StateUpdate<LoopState>> for Done {
    async fn invoke(
        &self,
        input: GraphState<LoopState>,
    ) -> Result<StateUpdate<LoopState>, WesichainError> {
        Ok(StateUpdate::new(input.data))
    }

    fn stream(
        &self,
        _input: GraphState<LoopState>,
    ) -> futures::stream::BoxStream<'_, Result<StreamEvent, WesichainError>> {
        stream::empty().boxed()
    }
}

fn build_graph(
    checkpointer: Option<FileCheckpointer>,
) -> wesichain_graph::ExecutableGraph<LoopState> {
    let builder = GraphBuilder::new()
        .add_node("inc", Inc)
        .add_node("done", Done)
        .add_conditional_edge("inc", |state: &GraphState<LoopState>| {
            if state.data.count >= 10 {
                "done".to_string()
            } else {
                "inc".to_string()
            }
        })
        .with_default_config(ExecutionConfig {
            max_steps: Some(25),
            cycle_detection: false,
            cycle_window: 10,
        })
        .set_entry("inc");

    let builder = if let Some(checkpointer) = checkpointer {
        builder.with_checkpointer(checkpointer, "bench-thread")
    } else {
        builder
    };

    builder.build()
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn max_rss() -> i64 {
    unsafe {
        let mut usage: libc::rusage = std::mem::zeroed();
        libc::getrusage(libc::RUSAGE_SELF, &mut usage);
        usage.ru_maxrss
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn max_rss() -> i64 {
    0
}

fn bench_graph_loop(c: &mut Criterion) {
    let rt = Runtime::new().expect("runtime");

    let graph = build_graph(None);
    let before = max_rss();
    let _ = rt.block_on(graph.invoke_graph(GraphState::new(LoopState::default())));
    let after = max_rss();
    println!("no_checkpoint_rss_delta={}", after - before);

    c.bench_function("graph_loop_no_checkpoint", |b| {
        b.iter(|| {
            let state = GraphState::new(LoopState::default());
            rt.block_on(graph.invoke_graph(state)).expect("invoke");
        })
    });

    c.bench_function("graph_loop_jsonl_checkpoint", |b| {
        b.iter_batched(
            || {
                let dir = tempfile::tempdir().expect("tempdir");
                let graph = build_graph(Some(FileCheckpointer::new(dir.path())));
                (graph, dir)
            },
            |(graph, _dir)| {
                let state = GraphState::new(LoopState::default());
                rt.block_on(graph.invoke_graph(state)).expect("invoke");
            },
            BatchSize::SmallInput,
        )
    });

    let dir = tempfile::tempdir().expect("tempdir");
    let graph = build_graph(Some(FileCheckpointer::new(dir.path())));
    let before = max_rss();
    let _ = rt.block_on(graph.invoke_graph(GraphState::new(LoopState::default())));
    let after = max_rss();
    println!("jsonl_checkpoint_rss_delta={}", after - before);
}

criterion_group!(benches, bench_graph_loop);
criterion_main!(benches);
