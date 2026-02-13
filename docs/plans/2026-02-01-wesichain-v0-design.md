# Wesichain v0 Design

Date: 2026-02-01
Status: Locked

## Goal
Build a Rust-native RAG/agent framework focused on easy migration from Python ecosystems, with a flagship ReAct agent that demonstrates lower memory and CPU usage, streaming, and resumable execution.

## Success Criteria
- A Rust developer can port a ~50-line Python ReAct agent, run the same query, and observe lower memory/CPU usage.
- The agent can resume from a saved checkpoint after process restart.
- The ReAct example feels familiar and idiomatic in Rust (.then() composition, builders).
- Benchmarks are reproducible via Criterion and optional heaptrack.

## Non-Goals (v0)
- Full state-graph orchestration and multi-agent workflows.
- Broad integration catalog (vector DBs, tools, providers).
- UI, server, or hosted execution platform.
- Python bindings or FFI.

## Architecture Overview
Wesichain is a workspace of small crates with an umbrella crate for ergonomics. Core abstraction: Runnable<Input, Output> with .then() composition. ReAct is implemented as a dedicated loop on top of runnables. Graph support is deferred to v0.1+.

### Crate Layout (v0)
- wesichain-core: core traits (Runnable, Chain, Tool, Checkpointer, Memory), stream events, errors, retries/timeouts, small helpers.
- wesichain-prompt: prompt templates and formatting.
- wesichain-llm: provider-agnostic LLM trait + provider adapters behind features.
- wesichain-agent: ReAct agent, parsers, tool registry, memory implementations (v0), checkpointer implementations.
- wesichain: umbrella crate re-exporting a prelude and common builders.
- Deferred crates: wesichain-graph, wesichain-retrieval, wesichain-tools, wesichain-memory, provider-specific checkpointers.

### Feature Flags
- wesichain-llm provider features: openai, ollama, mistral, candle.
- Umbrella crate mirrors provider features: llm-openai, llm-ollama, etc.
- Default features: core, prompt, agent (no provider by default).
- Quickstart docs use Ollama (llm-ollama) for zero API keys; OpenAI documented as alternative.

## Core Abstractions
- Runnable<Input, Output>:
  - async fn invoke(&self, input) -> Result<Output>
  - fn stream(&self, input) -> impl Stream<Item = StreamEvent> (default impl via invoke where possible)
  - async fn batch(&self, inputs: Vec<Input>) -> Result<Vec<Output>> (default via join_all)
- Chain<A, B> and .then() composition:
  - A: Runnable, B: Runnable<Input = A::Output>
- StreamEvent enum:
  - Token(String), ToolCall { id, name, args }, ToolResult { id, output }, Trace(String), Final(String)
- Tool:
  - name, description, schema: ToolSchema, async fn call(Value) -> Result<Value>
  - ToolSchema is a lightweight builder; optional schemars feature for deriving schema.
- Llm:
  - Provider-agnostic request/response (messages + tool schema -> LlmOutput with content and tool calls).
- Checkpointer:
  - save(thread_id, Checkpoint), load(thread_id) -> Option<Checkpoint>
- Memory:
  - load() -> Vec<Message>, append(Message)

## ReAct Agent Execution Flow
1. Load memory and optional prior checkpoint for thread_id.
2. Render prompt (system + history + scratchpad + tool schemas).
3. Call LLM; parse output into Final or ToolCalls.
4. If ToolCalls:
   - Validate args (best-effort).
   - Execute sequentially by default; optional bounded parallelism via tokio::Semaphore.
   - Append tool results to memory.
   - Save checkpoint.
5. Exit on Final or max_iterations.

### Parser Modes
- Classic ReAct text format (Thought/Action/Action Input).
- Structured JSON tool calling (OpenAI/Mistral-compatible).
- Parser returns AgentStep enum: FinalAnswer, ToolCalls, Retry, Error.

### Outputs
AgentOutput includes final text, optional structured output, tool trace, and metadata.

### Streaming
A single stream emits StreamEvent for tokens, tool calls, tool results, and final output.

## Checkpointing
- MemoryCheckpointer: in-memory hashmap for development.
- JsonFileCheckpointer: serde JSON snapshots in a directory keyed by thread_id.
- Checkpoint includes thread_id, iteration, messages, scratchpad, tool trace.

## Error Handling and Observability
- WesichainError with explicit variants: LlmProvider, ToolNotFound, ToolSchemaInvalid, ToolCallFailed, ParseFailed, CheckpointFailed, Timeout, Cancelled, MaxIterationsReached.
- Policies:
  - ParseFailurePolicy: retry with assistant guidance or fail fast.
  - ToolFailurePolicy: fail fast or append error observation.
- Structured tracing with tracing events per iteration and tool call.

## Testing and Benchmarks
- Unit tests: chaining, streaming propagation, error bubbling.
- ReAct loop tests with MockLlm + MockTool:
  - final-on-first-iteration, tool-call success, tool-call failure, malformed output retry.
- Checkpoint tests: save/load symmetry; resume after crash.
- Benchmarks (Criterion):
  - loop iteration latency (mock LLM), tool overhead, checkpoint serialize/deserialize.
- Profiling docs: heaptrack workflow for 50-turn loops.

## Migration and Examples
- README includes side-by-side Python ReAct -> Wesichain ReAct with the same prompt and tools.
- Flagship example: Research Assistant (search + calculator + time).
- Short Why Wesichain paragraph emphasizing memory/CPU savings, no GC pauses, Tokio async, and resumable execution.

## MVP Deliverables (v0)
- Runnable + .then() composition.
- ReAct agent (streaming, tool schema, memory).
- Checkpointer trait + MemoryCheckpointer + JsonFileCheckpointer.
- One LLM provider (Ollama recommended for quickstart) + MockLlm.
- 2-3 tools (calculator, HTTP fetch/search stub, time).
- Bench harness + profiling docs.
- Migration guide with side-by-side examples.

## Tech Stack
- Rust 1.75+
- tokio, async-trait
- serde, serde_json
- thiserror, anyhow
- reqwest, uuid
- Optional: schemars, candle-core, provider SDKs

## Risks and Mitigations
- Scope creep: lock v0 deliverables; defer graph and retrieval.
- Ecosystem gaps: provide minimal fallbacks; keep provider adapters thin.
- Performance shortfalls: prototype early, benchmark with mock loops, optimize hot paths.
- Adoption: open source early with clear examples and migration guide.

## Future Work (v0.1+)
- wesichain-graph (StateGraph + checkpointing per node).
- wesichain-retrieval (embeddings, vector stores).
- Additional checkpointers (SQLite/Redis/Postgres).
- CLI and optional Python bindings.
