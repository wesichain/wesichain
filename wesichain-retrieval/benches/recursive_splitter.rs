use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use wesichain_retrieval::RecursiveCharacterTextSplitter;

fn make_text(target_bytes: usize) -> String {
    let paragraph = "Wesichain retrieval pipelines split text into deterministic chunks for downstream embedding and indexing.\n";
    let mut text = String::with_capacity(target_bytes + paragraph.len());

    while text.len() < target_bytes {
        text.push_str(paragraph);
    }

    text
}

fn bench_recursive_splitter(c: &mut Criterion) {
    let splitter = RecursiveCharacterTextSplitter::builder()
        .chunk_size(1_000)
        .chunk_overlap(200)
        .build()
        .expect("valid recursive splitter config");

    let mut group = c.benchmark_group("recursive_splitter_throughput");
    for (label, bytes) in [
        ("small_16kb", 16 * 1024usize),
        ("medium_128kb", 128 * 1024usize),
        ("large_1mb", 1_024 * 1_024usize),
    ] {
        let input = make_text(bytes);
        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_with_input(BenchmarkId::new("split", label), &input, |b, text| {
            b.iter(|| splitter.split_text(text))
        });
    }
    group.finish();
}

criterion_group!(benches, bench_recursive_splitter);
criterion_main!(benches);
