# Migration Scoreboard

## Connector Status

| Connector | Status | Guide | Example | Parity Test | Notes |
| --- | --- | --- | --- | --- | --- |
| Qdrant | DONE | [langchain-to-wesichain-qdrant.md](./langchain-to-wesichain-qdrant.md) | [`wesichain-qdrant/examples/rag_integration.rs`](../../wesichain-qdrant/examples/rag_integration.rs) | [`wesichain-qdrant/tests/migration_parity.rs`](../../wesichain-qdrant/tests/migration_parity.rs) | Migration artifacts validated; nightly gate passed and migration-unblocked issue closed. |

## Qdrant Artifact Checklist (DONE)

- [x] Migration guide drafted: [langchain-to-wesichain-qdrant.md](./langchain-to-wesichain-qdrant.md)
- [x] Runnable example present: [`wesichain-qdrant/examples/rag_integration.rs`](../../wesichain-qdrant/examples/rag_integration.rs)
- [x] Deterministic parity test present: [`wesichain-qdrant/tests/migration_parity.rs`](../../wesichain-qdrant/tests/migration_parity.rs)
- [x] Benchmark report placeholder: [`docs/benchmarks/data/qdrant-2026-02-16.json`](../benchmarks/data/qdrant-2026-02-16.json)
- [x] Nightly gate evidence: [Nightly Benchmarks run 22054138510](https://github.com/wesichain/wesichain/actions/runs/22054138510)
- [x] Migration-unblocked issue closure: [Issue #23](https://github.com/wesichain/wesichain/issues/23)
