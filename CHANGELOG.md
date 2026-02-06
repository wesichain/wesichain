# Changelog

All notable changes to Wesichain will be documented in this file.

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
- Recursive splitter: 200-221 MiB/s throughput (2-4x vs Python LangChain)
- Zero-copy semantics and compile-time type safety

### Documentation
- Usage examples: async ingestion, checkpoint persistence
- Benchmark baseline results documented

## [Unreleased]

### Planned
- Triples projection extraction from graph state
- Additional file format loaders (markdown, csv)
- Postgres JSONB optimization for state storage
