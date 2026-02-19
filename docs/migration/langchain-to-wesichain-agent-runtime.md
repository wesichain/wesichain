# LangChain to Wesichain Agent Runtime Migration

This guide maps common LangChain-style manual agent loops to the v0.3 `wesichain-agent` runtime model.

## Why this migration exists

- Keep Python-style ergonomics where practical.
- Move correctness guarantees into compile-time/runtime contracts.
- Remove repeated manual loop and JSON parsing boilerplate.

## Mapping: manual loop -> `AgentRuntime`

Typical manual flow in dynamic frameworks:

1. call model
2. inspect tool_calls
3. dispatch tool
4. append tool result
5. repeat with ad-hoc retries

Wesichain v0.3 model:

- `AgentRuntime<..., Idle>::think()` starts the loop.
- `on_model_response(...)` validates model output and transitions to `Acting` or `Completed`.
- `on_tool_success()` transitions from `Acting` to `Observing`.
- `Observing::think()` begins the next cycle.
- Policy decisions (`Retry`, `Reprompt`, `Fail`, `Interrupt`) are explicit and budget-aware.

This removes bespoke while-loops and centralizes failure semantics.

## Mapping: manual `ToolSpec` JSON -> `TypedTool` + `ToolSet`

Manual JSON-heavy pattern:

- hand-written schema blobs
- dynamic arg extraction from JSON values
- runtime failures from shape mismatches

Wesichain v0.3 typed pattern:

1. define `TypedTool::Args` and `TypedTool::Output` with serde + schemars
2. implement `TypedTool::run(...)`
3. register tools in `ToolSet`

```rust
let tools = ToolSet::new()
    .register::<CalculatorTool>()
    .register_with(CalculatorTool::default())
    .build()?;
```

`ToolSet` exports a schema catalog for model-facing tool descriptions while dispatch remains strongly typed.

## Error handling differences

- Unknown tools and malformed arg payloads are deterministic runtime validation errors.
- Policy controls retry/reprompt behavior with explicit budget consumption.
- Cancellation transitions to terminal `Interrupted` in v0.3.

## Practical migration sequence

1. Move one agent loop to `AgentRuntime` first.
2. Convert one tool from manual JSON to `TypedTool`.
3. Register with `ToolSet` and verify schema/output parity.
4. Add event assertions for ordering and tool cardinality.
5. Remove old manual loop/utilities once behavior matches.
