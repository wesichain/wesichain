# Retrieval Benchmarks

These benchmarks compare local Rust payload-construction paths, including a
LangChain-style payload-shape baseline emulated in Rust (not Python runtime
execution, and not end-to-end retrieval).

## Rust

```bash
cargo bench -p wesichain-retrieval --bench in_memory -- --sample-size 10
cargo bench -p wesichain-qdrant --bench vs_langchain -- --sample-size 10
cargo bench -p wesichain-weaviate --bench vs_langchain -- --sample-size 10
```

### Qdrant Metadata Policy

- Every qdrant benchmark run must pin dataset metadata in source constants:
  - dataset name
  - dataset path
  - dataset commit hash
- Every run should produce/update a machine-readable artifact in
  `docs/benchmarks/data/` named `qdrant-YYYY-MM-DD.json`.
- Required artifact fields: `run_id`, `commit`, `date`, `dataset`, `hardware`, `results`.
- Use placeholders only for local dry-runs; replace placeholders before publishing results.

### Weaviate Metadata Policy

- Every weaviate benchmark run must pin dataset metadata in source constants:
  - dataset name
  - dataset path
  - dataset commit hash
- Synthetic datasets are allowed for local comparisons, but benchmark output must label
  them clearly as synthetic and non-network if no live Weaviate calls are made.
- Every run should produce/update a machine-readable artifact in
  `docs/benchmarks/data/` named `weaviate-YYYY-MM-DD.json`.
- Required artifact fields: `run_id`, `commit`, `date`, `dataset`, `hardware`, `results`.
- Dry-run artifacts must use explicit status values (for example: `dry_run_not_measured`)
  instead of placeholder text.

## Memory

```bash
cargo run --manifest-path tools/bench/Cargo.toml --bin measure_rss
cargo run --manifest-path tools/bench/Cargo.toml --bin measure_rss -- 12345
```

Outputs `rss_bytes=<value>` in bytes.

## LangChain-Style Baseline (Rust Emulation)

```bash
python tools/bench/python_baseline.py
```

This script is a local reference utility. The Weaviate `vs_langchain` benchmark
compares Rust payload-shape construction only.
