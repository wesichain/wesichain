# Retrieval Benchmarks

These benchmarks compare Wesichain's in-memory retrieval against a Python baseline.

## Rust

```bash
cargo bench -p wesichain-retrieval --bench in_memory -- --sample-size 10
cargo bench -p wesichain-qdrant --bench vs_langchain -- --sample-size 10
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

## Memory

```bash
cargo run --manifest-path tools/bench/Cargo.toml --bin measure_rss
cargo run --manifest-path tools/bench/Cargo.toml --bin measure_rss -- 12345
```

Outputs `rss_bytes=<value>` in bytes.

## Python Baseline

```bash
python tools/bench/python_baseline.py
```
