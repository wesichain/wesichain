use std::collections::HashMap;
use std::sync::Arc;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use wesichain_core::Document;
use wesichain_retrieval::{HashEmbedder, InMemoryVectorStore, Indexer, Retriever};

fn bench_index_and_query(c: &mut Criterion) {
    let embedder = Arc::new(HashEmbedder::new(64));
    let docs: Vec<Document> = (0..1000)
        .map(|idx| Document {
            id: format!("doc-{idx}"),
            content: format!("document {idx} about rust"),
            metadata: HashMap::new(),
            embedding: None,
        })
        .collect();

    let index_runtime = tokio::runtime::Runtime::new().unwrap();
    c.bench_function("index_1000", |b| {
        b.iter_batched(
            || {
                let store = Arc::new(InMemoryVectorStore::new());
                let indexer = Indexer::new(embedder.clone(), store);
                (indexer, docs.clone())
            },
            |(indexer, batch)| index_runtime.block_on(indexer.index(batch)),
            BatchSize::SmallInput,
        )
    });

    let store = Arc::new(InMemoryVectorStore::new());
    let indexer = Indexer::new(embedder.clone(), store.clone());
    let _ = index_runtime.block_on(indexer.index(docs.clone()));

    let retriever = Retriever::new(embedder.clone(), store);
    let query_runtime = tokio::runtime::Runtime::new().unwrap();
    c.bench_function("query_top5", |b| {
        b.iter(|| query_runtime.block_on(retriever.retrieve("rust", 5, None)))
    });
}

criterion_group!(benches, bench_index_and_query);
criterion_main!(benches);
