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
