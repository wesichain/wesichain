# Changelog

All notable changes to Wesichain will be documented in this file.

## [0.3.1] - 2026-03-23

### Fixed
- `wesichain-rag`: `GenerateAnswerNode` was a stub returning placeholder text instead of calling the LLM. Now builds a real `LlmRequest` with system prompt + retrieved context and calls the configured LLM. `WesichainRagBuilder::with_llm()` was silently dropping the argument — it now stores it. Returns a clear `InvalidConfig` error if `query()` is called without an LLM configured.

## [0.3.0] - 2026-03-22

### New Crates
- `wesichain-anthropic`: Anthropic Claude client with streaming, tool use, and extended thinking
- `wesichain-tools`: Coding tools — `ReadFileTool`, `WriteFileTool`, `EditFileTool`, `ReplaceLinesTool`, `GlobTool`, `GrepTool`, `PatchTool`, `BashExecTool`, git tools, `PathGuard` sandbox
- `wesichain-mcp`: MCP client (MCP 2024-11-05) over stdio and HTTP/SSE transport, resources + sampling support
- `wesichain-session`: Session persistence via `FileSessionStore`, cost/token tracking, token budget enforcement
- `wesichain-server`: Axum HTTP server with Bearer auth, rate limiting, SSE streaming, and body-size guard
- `wesichain-cli`: `wesichain new` scaffolding and `wesichain run` interactive REPL with ANSI diff viewer
- `wesichain-langfuse`: Langfuse observability callback handler with trace batching and PII redaction
- `wesichain-otel`: Fixed OpenTelemetry span parenting with W3C traceparent propagation and OTLP export
- `wesichain-rag`: Full RAG pipeline — document loaders, recursive text splitter, vector store integration, reranking

### New APIs in Existing Crates
- `wesichain-core`: `ModelCapabilities`, `TokenBudget`, `capability::for_model()`, `TimeLimited`, `RateLimiter`, `ApprovalHandler`
- `wesichain-agent`: `AgentCheckpoint`, `ToolSet::tool_specs()`, `ToolCallEnvelope`, FSM-based runtime, `PermissionPolicy`, `AsToolExt`
- `wesichain-graph`: Supervisor pattern, HITL (human-in-the-loop) nodes, parallel agents, `fork_from_checkpoint()`
- `wesichain-memory`: `VectorMemoryStore`, `EntityMemory`, semantic memory (`SemanticMemoryStore`)
- `wesichain-retrieval`: `CrossEncoderRetriever`, `KeywordReranker`, `Reranker` trait
- `wesichain-llm`: Groq, Together AI, Azure OpenAI, and Mistral providers
- `wesichain-prompt`: `PromptHub` trait, `LocalPromptHub` (YAML directory scanner)
- `wesichain-core/checkpoint`: `HistoryCheckpointer::fork()` for time-travel branching

### Metadata
- All 29 publishable crates now have `keywords`, `categories`, and `readme` fields
- README.md files added for all new crates

## [0.1.0] - 2026-02-06

### Added
- **Checkpoint Persistence**
  - `wesichain-checkpoint-sql`: Shared SQL checkpoint core with backend-agnostic operations
  - `wesichain-checkpoint-sqlite`: SQLite backend with in-memory and file database support
  - `wesichain-checkpoint-postgres`: Postgres backend with connection pooling
  - CheckpointSave trait integration with wesichain-graph
  - Optional relational projections with transactional rollback
  - Sequence allocation per-thread with conflict retry (8 attempts)

- **Document Ingestion**
  - Async file loading API (`load_file_async`, `load_files_async`)
  - DOCX text-first extraction (paragraphs, tables, run boundary preservation)
  - Recursive character text splitter with builder API
  - Overlap and separator configuration (UTF-8 safe)
  - Metadata propagation (chunk_index, source)

- **Testing & Benchmarks**
  - 80+ tests across all new crates
  - Integration tests for SQLite (primary) and Postgres (DATABASE_URL-gated)
  - Criterion benchmark for recursive splitter throughput (~200 MiB/s)

### Changed
- Extended `IngestionError` with stage-specific variants (IO, Parse, Split)
- Added `load_and_split_recursive()` convenience function

### Performance
- Recursive splitter: 200-221 MiB/s throughput (2-4x vs typical Python baselines)
- Zero-copy semantics and compile-time type safety

### Documentation
- Usage examples: async ingestion, checkpoint persistence
- Benchmark baseline results documented

## [Unreleased]

### Added
- `wesichain-qdrant` crate with Qdrant `VectorStore` integration
- Qdrant metadata filter translation for `Eq`, `In`, `Range`, `All`, and `Any`
- Migration artifacts for LangChain-to-Wesichain Qdrant parity (guide, example, parity test)
- Benchmark harness and threshold tooling for Qdrant slice validation
- `wesichain-weaviate` crate with Weaviate `VectorStore` integration
- Weaviate GraphQL metadata filter translation for `Eq`, `In`, `Range`, `All`, and `Any`
- Migration artifacts for LangChain-to-Wesichain Weaviate parity (guide, example, parity test)
- Benchmark harness and CI threshold-gate coverage for Weaviate slice validation

### Planned
- Triples projection extraction from graph state
- Additional file format loaders (markdown, csv)
- Postgres JSONB optimization for state storage
