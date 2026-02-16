# Migration Scoreboard

## Connector Status

| Connector | Status | Guide | Example | Parity Test | Notes |
| --- | --- | --- | --- | --- | --- |
| Qdrant | WIP | [langchain-to-wesichain-qdrant.md](./langchain-to-wesichain-qdrant.md) | [`wesichain-qdrant/examples/rag_integration.rs`](../../wesichain-qdrant/examples/rag_integration.rs) | [`wesichain-qdrant/tests/migration_parity.rs`](../../wesichain-qdrant/tests/migration_parity.rs) | Migration + benchmark artifacts are in place; waiting for nightly evidence and issue closure to mark DONE. |

## Qdrant Artifact Checklist (WIP)

- [x] Migration guide drafted: [langchain-to-wesichain-qdrant.md](./langchain-to-wesichain-qdrant.md)
- [x] Runnable example present: [`wesichain-qdrant/examples/rag_integration.rs`](../../wesichain-qdrant/examples/rag_integration.rs)
- [x] Deterministic parity test present: [`wesichain-qdrant/tests/migration_parity.rs`](../../wesichain-qdrant/tests/migration_parity.rs)
- [x] Benchmark report placeholder: [`docs/benchmarks/data/qdrant-2026-02-16.json`](../benchmarks/data/qdrant-2026-02-16.json)
- [ ] Nightly gate evidence placeholder: `<nightly-build-url>`
- [ ] Migration-unblocked issue closure placeholder: `<issue-url>`
