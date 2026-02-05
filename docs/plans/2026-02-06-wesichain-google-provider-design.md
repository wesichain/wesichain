# Wesichain Google Provider Design (Gemini API)

Date: 2026-02-06
Status: Validated

## Goal
Add first-class Google provider support to Wesichain for both LLM generation and embeddings using Gemini API (AI Studio), while preserving existing provider patterns, explicit configuration, and low-overhead runtime behavior.

## Success Criteria
- Wesichain users can call Google Gemini chat models through `wesichain-llm` with text generation, tool calling, and streaming.
- Wesichain users can generate embeddings through `wesichain-embeddings` using single and batch APIs.
- Integration is fully feature-gated (`google`) and does not change behavior for users who do not enable the feature.
- No default model is assumed; model configuration is explicit.
- Existing ReAct and retrieval flows work without API shape changes.

## Scope (MVP)
- `wesichain-llm`: add `GoogleClient` for Gemini API `generateContent` and `streamGenerateContent`.
- `wesichain-embeddings`: add `GoogleEmbedding` for `embedContent` and `batchEmbedContents`.
- Tool calling support mapped from `ToolSpec`/`ToolCall`.
- Streaming support mapped to `StreamEvent`.
- Unit tests for mapping/parsing/error handling; ignored live integration tests gated by `GOOGLE_API_KEY`.

## Non-goals (MVP)
- Vertex AI auth and project/location routing.
- Multimodal inputs/outputs (images, audio, file uploads).
- Built-in retry policies in provider clients.
- New public core trait changes in `wesichain-core`.

## Approach Options
- Custom Google clients in existing crates (chosen): thin reqwest clients in `wesichain-llm` and `wesichain-embeddings`; lowest complexity and best consistency.
- Generic Gemini-compatible abstraction + wrappers: useful for future variants but unnecessary complexity for current MVP.
- SDK/discovery-generated client: larger dependency and control surface, weaker fit with current project style.

## Architecture Overview
### `wesichain-llm`
- Add `wesichain-llm/src/providers/google.rs` behind `#[cfg(feature = "google")]`.
- Export from `wesichain-llm/src/providers/mod.rs` and `wesichain-llm/src/lib.rs`.
- Public type: `GoogleClient` implementing `Runnable<LlmRequest, LlmResponse>`.

### `wesichain-embeddings`
- Add `wesichain-embeddings/src/google.rs` behind `#[cfg(feature = "google")]`.
- Export from `wesichain-embeddings/src/lib.rs`.
- Public type: `GoogleEmbedding` implementing `Embedding`.

### Configuration
- No default model names.
- Required: API key and explicit model string.
- Optional: timeout and base URL override (default `https://generativelanguage.googleapis.com`).
- API key stored as `secrecy::SecretString`.

## API Endpoints
Gemini API v1beta endpoints used by MVP:
- `POST v1beta/{model=models/*}:generateContent`
- `POST v1beta/{model=models/*}:streamGenerateContent`
- `POST v1beta/{model=models/*}:embedContent`
- `POST v1beta/{model=models/*}:batchEmbedContents`

Model normalization behavior:
- Input `gemini-1.5-pro` becomes `models/gemini-1.5-pro`.
- Input already in `models/...` format is preserved.

## LLM Request Mapping
Input type remains `wesichain_core::LlmRequest`.

### Role mapping
- `Role::User` -> Gemini `Content.role = "user"`.
- `Role::Assistant` -> Gemini `Content.role = "model"`.
- `Role::System` -> folded into `systemInstruction` as text parts.
- `Role::Tool` -> represented as function response content (`role = "user"` with `parts.functionResponse`).

### System instruction handling
- Collect all `Role::System` messages.
- Join with blank lines.
- Emit one `systemInstruction` content with text part.

### Tool declaration mapping
- Map `Vec<ToolSpec>` to one Gemini `Tool` containing `functionDeclarations`.
- `ToolSpec.parameters` is passed as `parametersJsonSchema`.
- Set `toolConfig.functionCallingConfig.mode = "AUTO"` when tools are provided.
- Set `allowedFunctionNames` from tool names for predictable model behavior.

### Tool history mapping
- Assistant messages with tool calls are encoded using `parts.functionCall`.
- Tool outputs are encoded using `parts.functionResponse` in a `role = "user"` content.
- When multiple tool results are available for a turn, bundle them as multiple `functionResponse` parts in one content message.

## LLM Response Mapping
Read first candidate for MVP parity with existing providers.

### Text output
- Concatenate all `parts.text` in candidate content into `LlmResponse.content`.

### Tool calls
- Convert each `parts.functionCall` to `ToolCall { id, name, args }`.
- Gemini may omit call IDs; synthesize deterministic IDs (`google_call_1`, `google_call_2`, ...).
- If args are not an object, wrap into `{ "value": <args> }` for downstream safety.

## Streaming Semantics
`GoogleClient.stream()` uses `streamGenerateContent`.

- Emit incremental text as `StreamEvent::ContentChunk`.
- Accumulate text for final output.
- Buffer function-call payloads until valid JSON for safe emission.
- Emit terminal `StreamEvent::FinalAnswer(accumulated_text)` on normal completion.
- On stream transport/parse failure, emit terminal `Err(...)` and stop.

MVP note: function-call partial argument streaming is not required.

## Embedding Request and Response Mapping
`GoogleEmbedding` implements `Embedding`:
- `embed(text)` -> `embedContent`.
- `embed_batch(texts)` -> `batchEmbedContents`.

Request behavior:
- Explicit model required.
- Optional task type supported (for example, retrieval query/document modes).

Validation behavior:
- `embed_batch` validates returned embedding count equals input count.
- All vectors must match configured dimension.
- Mismatch returns `EmbeddingError::InvalidResponse`.

## Error Handling
### LLM client
- Reqwest transport and timeout errors -> `WesichainError::LlmProvider`.
- Non-2xx responses parse structured Google error body when available.
- Fallback includes HTTP status and raw body text if structured parsing fails.
- Handle common statuses explicitly: 400, 401/403, 404, 429, 5xx.

### Finish reason handling
- If blocked finish reason (`SAFETY`, `RECITATION`, or equivalent) with no usable content/tool calls, return provider error with reason.
- If partial text exists, return partial text and empty tool calls.

### Embeddings client
- Transport failures -> `EmbeddingError::Provider`.
- Schema/count/dimension mismatch -> `EmbeddingError::InvalidResponse`.

## Testing Plan
### `wesichain-llm` unit tests
- Role and system instruction mapping.
- Tool declaration and tool history mapping.
- Response parsing: text + function calls + synthetic IDs.
- Error mapping from structured and unstructured error bodies.
- Streaming chunk parsing, ordering, and terminal behavior.

### `wesichain-embeddings` unit tests
- Single and batch request shape.
- Count and dimension validation.
- Optional task type propagation.

### Ignored live integration tests
- Enabled only with `GOOGLE_API_KEY`.
- Non-streaming generation.
- Streaming generation.
- Tool-calling round trip.
- Batch embeddings.

## Rollout Plan
1. Add `google` feature flags and exports in both crates.
2. Implement `GoogleClient` and parser/mappers.
3. Implement `GoogleEmbedding` with validation.
4. Add unit tests and ignored live tests.
5. Update docs/examples with explicit-model usage.

## Public API Additions (MVP)
- `wesichain_llm::GoogleClient` (feature `google`).
- `wesichain_embeddings::GoogleEmbedding` (feature `google`).

No default models are introduced.

## Risks and Mitigations
- API schema drift: isolate request/response structs and keep parser strict with safe fallbacks.
- Streaming edge cases: fail fast on malformed chunks, avoid emitting malformed tool calls.
- Quota/rate limits: propagate explicit errors; leave retries to caller policies.

## Future Work (Post-MVP)
- Vertex AI auth/project/location support.
- Optional retry/backoff strategies.
- Multimodal support (`inlineData`, `fileData`).
- Extended streaming function-call argument deltas.
