# Retrieval Benchmarks

These benchmarks compare Wesichain's in-memory retrieval against a Python baseline.

## Rust

```bash
cargo bench -p wesichain-retrieval --bench in_memory -- --sample-size 10
```

## Memory

```bash
rustc tools/bench/measure_rss.rs -o /tmp/measure_rss
/tmp/measure_rss
/tmp/measure_rss <pid>
```

Outputs `rss_bytes=<value>` in bytes.

## Python Baseline

```bash
python tools/bench/langchain_baseline.py
```
