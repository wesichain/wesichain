# Wesichain Milestone 1: Persistence and Ingestion Design

Date: 2026-02-06
Status: Validated

## Goals
- Deliver SQL-backed checkpoint persistence with client-friendly projection tables.
- Add ingestion parity for txt, pdf, and docx plus recursive character splitting.
- Keep current Wesichain public APIs stable and additive.
- Stay lean: async I/O, minimal dependencies, deterministic behavior.

## Non-goals
- Projection-based checkpoint reconstruction in Milestone 1.
- Rich DOCX layout fidelity (styles, images, full document semantics).
- Hidden retry loops inside SQL primitives.
- Breaking changes to existing retrieval loaders/splitter.

## Locked Decisions
- Direction A: projection hybrid + shared SQL core.
- Canonical source of truth is serialized checkpoint state.
- Projections are optional and transactionally written side effects.
- Projections are disabled by default and enabled via backend builder config.
- Python benchmark comparison is directional evidence, not a hard release gate.

## Architecture
Milestone 1 adds three crates:
- `wesichain-checkpoint-sql`: shared internals for schema, migrations, ops, projection, and minimal dialect helpers.
- `wesichain-checkpoint-sqlite`: SQLite backend wrapper, pool setup, constructor/builder.
- `wesichain-checkpoint-postgres`: Postgres backend wrapper, pool setup, constructor/builder.

`wesichain-graph` remains source-compatible: `Checkpointer<S>` and `GraphBuilder::with_checkpointer(...)` stay unchanged. Existing `FileCheckpointer` behavior is unaffected.

## Persistence Data Model
Canonical table:
- `checkpoints(thread_id, seq, created_at, node, step, state_json)` with primary key `(thread_id, seq)`.

Projection tables (optional):
- `sessions` for thread/session metadata.
- `messages` for normalized conversation entries (`thread_id`, `seq`, `role`, `content`, timestamps).
- `graph_triples` for normalized triples (`thread_id`, `subject`, `predicate`, `object`, optional `score`).

For Milestone 1, `state_json` is stored as `TEXT` across backends for simplicity and parity. Postgres JSONB optimization is deferred.

## Save/Load Flow
### Save (single transaction)
1. Begin transaction.
2. Compute next sequence per thread using `COALESCE(MAX(seq), 0) + 1` scoped to `thread_id`.
3. Insert canonical checkpoint row.
4. If `enable_projections` is true, run projection mapping and write `sessions/messages/graph_triples`.
5. Commit.

If any projection step fails, rollback the whole transaction.

### Load
- Read latest canonical row for `thread_id` by `seq DESC LIMIT 1`.
- Deserialize `state_json` and return `Checkpoint<S>`.
- Projection-based reconstruction is out of scope for this milestone.

## SQL Abstractions and Builders
- Shared crate provides minimal backend-agnostic ops and projection helpers.
- Backend crates provide builder constructors with flags such as:
  - `enable_projections(false)` by default.
  - connection URL and pool options.
- Dialect helpers stay minimal and only cover real query/DDL differences.

## Ingestion Design (Additive)
Current sync APIs remain unchanged:
- `TextLoader`, `PdfLoader`, `TextSplitter` continue to work as-is.

New additive APIs in `wesichain-retrieval`:
- `load_file_async(path)`
- `load_files_async(paths)`
- `load_and_split_recursive(paths, splitter)`
- `RecursiveCharacterTextSplitter` with configurable `chunk_size`, `chunk_overlap`, and separators.

Extension dispatch:
- `.txt`: async file read.
- `.pdf`: existing PDF extraction path.
- `.docx`: text-first extraction (paragraphs and table cells in reading order), with graceful parse errors.

## Recursive Splitter Behavior
- Default separators: `"\n\n"`, `"\n"`, `" "`, `""`.
- Recursively split using highest-priority separator that still leaves oversized chunks to be further split.
- Preserve overlap windows between chunks.
- Enforce UTF-8 safety and deterministic chunk boundaries.
- Preserve metadata: `source`, `file_type`, `chunk_index`, optional `parent_id`.

Config guards:
- `chunk_size == 0` returns configuration error.
- `chunk_overlap >= chunk_size` is clamped to `chunk_size - 1`.

## Error Handling
Persistence errors use explicit typed enums (connection, migration, serialization, query, projection), then map to graph-level checkpoint errors at integration boundaries.

Ingestion errors are stage-specific (read, unsupported extension, parse pdf/docx, split config) and include source file path context.

SQL primitives are single-attempt. Retry/backoff policy is orchestration-level, not hidden in persistence internals.

## Testing Strategy
### Unit
- Sequence allocation and latest checkpoint load behavior.
- Projection mapper correctness for sessions/messages; triple mapping from existing state fields.
- Transaction rollback on forced projection failure.
- DOCX flattening and malformed input handling.
- Recursive splitter behavior: separator priority, overlap, UTF-8 boundaries, metadata propagation.

### Integration
- SQLite-first end-to-end tests: ingest -> split -> save checkpoint -> assert canonical and projection rows.
- Concurrent save scenarios per thread to validate sequence monotonicity.
- Postgres parity tests feature-gated and run in CI/dev environments where available.

### Benchmarks
- Criterion benchmarks for splitter throughput and ingestion runtime.
- Memory/time measurements for representative txt/pdf/docx datasets.
- Python comparison script provides directional evidence; target is resource improvement, not hard gate.

## Acceptance Criteria
- SQL checkpointers (SQLite and Postgres) compile, initialize schema, save/load latest checkpoint correctly.
- Projection writes are optional, transactional, and consistent.
- Additive async ingestion handles txt/pdf/docx and recursive splitting without breaking existing APIs.
- New unit/integration tests pass for Milestone 1 scope.
- Benchmark outputs are reproducible and documented.

## Implementation Order (Days 1-5)
1. Create `wesichain-checkpoint-sql` shared crate with schema/migration/ops/projection stubs.
2. Implement SQLite backend first, including transactional save/load.
3. Add projection mappings for sessions/messages; triples from existing state when present.
4. Implement Postgres backend reusing shared core.
5. Add async ingestion APIs and recursive splitter in `wesichain-retrieval`.
6. Complete tests, benchmark scripts, and docs/examples.

## Future Work
- Projection-based checkpoint reconstruction fallback.
- Postgres JSONB/index optimizations.
- Richer DOCX structure support.
- Advanced ingestion pipelines and streaming integration.
