# Retrieval Benchmarks

These benchmarks compare Wesichain's in-memory retrieval against a Python baseline.

## Rust

```bash
cargo bench -p wesichain-retrieval --bench in_memory -- --sample-size 10
```

## Memory

```bash
cargo run --manifest-path tools/bench/Cargo.toml --bin measure_rss
cargo run --manifest-path tools/bench/Cargo.toml --bin measure_rss -- 12345
```

Outputs `rss_bytes=<value>` in bytes.

## Python Baseline

```bash
python tools/bench/langchain_baseline.py
```
