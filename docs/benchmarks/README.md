# Performance Benchmarks

## Recursive Character Text Splitter

Throughput measured on macOS (Darwin) with `cargo bench -p wesichain-retrieval --bench recursive_splitter`.

### Results

| Input Size | Time (avg) | Throughput | Notes |
|------------|------------|------------|-------|
| 16 KB      | 70.8 µs    | 221 MiB/s  | Baseline performance |
| 128 KB     | 572 µs     | 218 MiB/s  | Consistent scaling |
| 1 MB       | 4.97 ms    | 201 MiB/s  | Slight overhead on large inputs |

### Test Parameters

- **Default separators:** `["\n\n", "\n", " ", ""]`
- **Chunk size:** 1000 characters
- **Overlap:** 200 characters
- **Character-based:** UTF-8 safe (not byte-based)

### Comparison Notes

Typical Python recursive text splitters often achieve ~50-100 MiB/s for similar workloads. Rust implementation shows **~2-4x throughput improvement** with zero-copy semantics and compile-time safety.

Running benchmark:
```bash
cargo bench -p wesichain-retrieval --bench recursive_splitter
```

Results saved to `target/criterion/` for detailed analysis.

## Vector Store Benchmarks

Nightly and release-oriented connector benchmarks are tracked as JSON snapshots.

- Qdrant snapshot: `docs/benchmarks/data/qdrant-2026-02-16.json`
- Weaviate snapshot: `docs/benchmarks/data/weaviate-2026-02-16.json`
- Weekly summary: `docs/benchmarks/data/weekly/2026-W08.md`

Threshold evaluation helpers live in `tools/bench/`:

```bash
python3 tools/bench/evaluate_thresholds.py --input docs/benchmarks/data/qdrant-2026-02-16.json
python3 tools/bench/evaluate_thresholds.py --input docs/benchmarks/data/weaviate-2026-02-16.json
```
