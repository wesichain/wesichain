use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use serde::Deserialize;
use serde_json::{Map, Value};
use wesichain_weaviate::mapper::{CONTENT_PAYLOAD_KEY, METADATA_PAYLOAD_KEY};

const DATASET_NAME: &str = "weaviate-synthetic-1k-shared";
const DATASET_PATH: &str = "docs/benchmarks/datasets/qdrant-synthetic-1k.jsonl";
const DATASET_COMMIT_HASH: &str = "4f6d2b74239dca8de7c6e8e183571d729b005a63";
const DATASET_KIND: &str = "synthetic";
const DATASET_SIZE: usize = 1_000;
const EMBEDDING_DIM: usize = 128;
const BENCHMARK_SCOPE: &str = "local_payload_construction";
const NETWORK_MODE: &str = "none";
const BASELINE_IMPL: &str = "langchain_style_rust_shape_emulation";
const BASELINE_LABEL: &str = "langchain_style_rust_object_payload";
const BENCH_GROUP_NAME: &str = "weaviate_payload_vs_langchain_style_rust";

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

fn wesichain_object_payload(dataset: &[BenchDoc]) -> Vec<Value> {
    let metadata_payload = format!("{{\"source\":\"{}\"}}", DATASET_NAME);

    dataset
        .iter()
        .map(|doc| {
            let mut properties = Map::new();
            properties.insert(
                CONTENT_PAYLOAD_KEY.to_string(),
                Value::String(doc.content.clone()),
            );
            properties.insert(
                METADATA_PAYLOAD_KEY.to_string(),
                Value::String(metadata_payload.clone()),
            );

            let vector = doc
                .embedding
                .iter()
                .map(|value| Value::from(*value))
                .collect::<Vec<Value>>();

            let mut object = Map::new();
            object.insert("class".to_string(), Value::String("BenchDoc".to_string()));
            object.insert("id".to_string(), Value::String(doc.id.clone()));
            object.insert("vector".to_string(), Value::Array(vector));
            object.insert("properties".to_string(), Value::Object(properties));
            Value::Object(object)
        })
        .collect()
}

fn langchain_style_rust_object_payload(dataset: &[BenchDoc]) -> Vec<Value> {
    dataset
        .iter()
        .map(|doc| {
            serde_json::json!({
                "class": "BenchDoc",
                "id": doc.id,
                "vector": doc.embedding,
                "properties": {
                    "page_content": doc.content,
                    "metadata": {
                        "dataset": DATASET_NAME,
                        "dataset_kind": DATASET_KIND,
                    }
                }
            })
        })
        .collect()
}

fn emit_reproducibility_metadata(dataset_path: &str, dataset_size: usize) {
    let metadata = serde_json::json!({
        "benchmark": "wesichain-weaviate/benches/vs_langchain.rs",
        "benchmark_scope": BENCHMARK_SCOPE,
        "network": NETWORK_MODE,
        "baseline_impl": BASELINE_IMPL,
        "dataset": {
            "name": DATASET_NAME,
            "path": dataset_path,
            "commit_hash": DATASET_COMMIT_HASH,
            "kind": DATASET_KIND,
            "size": dataset_size,
            "embedding_dim": EMBEDDING_DIM
        },
        "notes": "Synthetic dataset benchmark for local payload construction only; baseline is Rust shape emulation (no Python runtime execution), and no live Weaviate network calls."
    });

    println!("reproducibility_metadata={metadata}");
}

fn bench_vs_langchain(c: &mut Criterion) {
    let dataset_path = resolve_dataset_path();
    let dataset = load_dataset(&dataset_path);

    assert_eq!(
        dataset.len(),
        DATASET_SIZE,
        "dataset should contain exactly {DATASET_SIZE} docs"
    );
    assert!(
        dataset
            .iter()
            .all(|doc| doc.embedding.len() == EMBEDDING_DIM),
        "every embedding should have exactly {EMBEDDING_DIM} dimensions"
    );

    emit_reproducibility_metadata(&dataset_path, dataset.len());

    let mut group = c.benchmark_group(BENCH_GROUP_NAME);
    group.sample_size(10);

    group.bench_function("wesichain_object_payload", |b| {
        b.iter(|| black_box(wesichain_object_payload(black_box(&dataset))))
    });

    group.bench_function(BASELINE_LABEL, |b| {
        b.iter(|| black_box(langchain_style_rust_object_payload(black_box(&dataset))))
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
