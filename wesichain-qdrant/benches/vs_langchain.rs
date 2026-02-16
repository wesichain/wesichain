use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use serde::Deserialize;
use serde_json::{Map, Value};

const DATASET_NAME: &str = "qdrant-synthetic-1k";
const DATASET_PATH: &str = "docs/benchmarks/datasets/qdrant-synthetic-1k.jsonl";
const DATASET_COMMIT_HASH: &str = "4f6d2b74239dca8de7c6e8e183571d729b005a63";
const DATASET_SIZE: usize = 1_000;
const EMBEDDING_DIM: usize = 128;

#[derive(Deserialize)]
struct BenchDoc {
    id: String,
    content: String,
    embedding: Vec<f32>,
}

fn load_dataset(dataset_path: &str) -> Vec<BenchDoc> {
    let file = File::open(dataset_path)
        .unwrap_or_else(|error| panic!("failed to open dataset file at '{dataset_path}': {error}"));
    let reader = BufReader::new(file);

    reader
        .lines()
        .enumerate()
        .filter_map(|(line_idx, line)| {
            let line = line.unwrap_or_else(|error| {
                panic!("failed to read dataset line {}: {error}", line_idx + 1)
            });

            if line.trim().is_empty() {
                return None;
            }

            let doc = serde_json::from_str::<BenchDoc>(&line).unwrap_or_else(|error| {
                panic!(
                    "invalid dataset JSON at line {} in '{dataset_path}': {error}",
                    line_idx + 1,
                )
            });

            Some(doc)
        })
        .collect()
}

fn resolve_dataset_path() -> String {
    let configured_path =
        std::env::var("DATASET_PATH").unwrap_or_else(|_| DATASET_PATH.to_string());

    let direct_path = PathBuf::from(&configured_path);
    if direct_path.is_file() {
        return direct_path.to_string_lossy().to_string();
    }

    let workspace_relative = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(&configured_path);
    if workspace_relative.is_file() {
        return workspace_relative.to_string_lossy().to_string();
    }

    panic!(
        "unable to resolve dataset path '{configured_path}'; set DATASET_PATH to an existing file"
    );
}

fn wesichain_payload(dataset: &[BenchDoc]) -> Vec<Value> {
    dataset
        .iter()
        .map(|doc| {
            let mut payload = Map::new();
            payload.insert("id".to_string(), Value::String(doc.id.clone()));
            payload.insert("content".to_string(), Value::String(doc.content.clone()));

            let vector = doc
                .embedding
                .iter()
                .map(|value| Value::from(*value))
                .collect::<Vec<Value>>();

            let mut point = Map::new();
            point.insert("id".to_string(), Value::String(doc.id.clone()));
            point.insert("vector".to_string(), Value::Array(vector));
            point.insert("payload".to_string(), Value::Object(payload));
            Value::Object(point)
        })
        .collect()
}

fn langchain_style_payload(dataset: &[BenchDoc]) -> Vec<Value> {
    dataset
        .iter()
        .map(|doc| {
            serde_json::json!({
                "id": doc.id,
                "vector": doc.embedding,
                "payload": {
                    "id": doc.id,
                    "page_content": doc.content,
                    "metadata": {
                        "dataset": DATASET_NAME,
                    }
                }
            })
        })
        .collect()
}

fn bench_vs_langchain(c: &mut Criterion) {
    let dataset_path = resolve_dataset_path();
    let dataset = load_dataset(&dataset_path);

    assert_eq!(
        dataset.len(),
        DATASET_SIZE,
        "dataset should contain exactly {DATASET_SIZE} docs"
    );

    println!(
        "dataset_name={DATASET_NAME} dataset_path={dataset_path} dataset_commit={DATASET_COMMIT_HASH} dataset_size={} embedding_dim={EMBEDDING_DIM}",
        dataset.len(),
    );

    let mut group = c.benchmark_group("qdrant_payload_vs_langchain");
    group.sample_size(10);

    group.bench_function("wesichain_payload", |b| {
        b.iter(|| black_box(wesichain_payload(black_box(&dataset))))
    });

    group.bench_function("langchain_style_payload", |b| {
        b.iter(|| black_box(langchain_style_payload(black_box(&dataset))))
    });

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2));
    targets = bench_vs_langchain
}
criterion_main!(benches);
