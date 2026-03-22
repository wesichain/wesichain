---
name: Sprint 6 complete
description: Sprint 6 coding-agent backlog items delivered (P2-8 semantic memory, P2-9 checkpoint fork, P2-10 Groq/Together AI, P2-11 PromptHub)
type: project
---

Sprint 6 items delivered and all tests passing.

**P2-9 — Checkpoint fork** (`wesichain-core/src/checkpoint.rs`)
- Added `fork(thread_id, at_seq) -> Result<String, WesichainError>` to `HistoryCheckpointer` trait with safe default
- Implemented on `InMemoryCheckpointer`: requires exact checkpoint at `at_seq`, then copies all steps ≤ seq into new UUID thread
- 3 tests

**P2-8 — Semantic memory** (`wesichain-memory/src/semantic.rs`)
- `VectorMemoryStore<E, V>` — embedding-backed; embeds turns on save, searches on load
- `EntityMemory` — HashMap-backed key/value per thread
- `MemoryRouter` — fans out to multiple Memory layers

**P2-10 — Groq + Together AI** (`wesichain-llm/src/providers/groq.rs`, `together.rs`)
- Both wrap `OpenAiCompatibleClient` with provider base URLs
- Feature flags: `groq`, `together`; added to `all-providers`
- Also fixed missing `model` field in google.rs `LlmResponse`

**P2-11 — PromptHub** (`wesichain-prompt/src/hub.rs`, requires `yaml` feature)
- `PromptHub` trait with `load(name, version)` + `list()`
- `LocalPromptHub` scans directory for `name.yaml` / `name@version.yaml` files
- 5 tests

**Why:** Completes Sprint 6 of the coding-agent backlog roadmap.
**How to apply:** Sprint 7 is next: P3-1 (tree-sitter AST editing), P3-3 (ANSI diff viewer), P3-4 (RAG re-ranking), P3-5 (Langfuse).
