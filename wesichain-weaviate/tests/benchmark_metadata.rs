use std::fs;
use std::path::Path;

use serde_json::Value;

#[test]
fn benchmark_metadata_includes_dataset_commit_hash() {
    let artifact_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("docs")
        .join("benchmarks")
        .join("data")
        .join("weaviate-2026-02-16.json");

    let artifact = fs::read_to_string(&artifact_path).unwrap_or_else(|error| {
        panic!("failed to read benchmark artifact {artifact_path:?}: {error}")
    });
    let parsed: Value = serde_json::from_str(&artifact)
        .unwrap_or_else(|error| panic!("failed to parse benchmark artifact JSON: {error}"));

    let commit_hash = parsed
        .get("dataset")
        .and_then(|dataset| dataset.get("commit_hash"))
        .and_then(Value::as_str)
        .expect("dataset.commit_hash should be present and string");

    assert!(
        !commit_hash.trim().is_empty(),
        "dataset.commit_hash should not be empty"
    );
    assert_eq!(
        commit_hash.len(),
        40,
        "dataset.commit_hash should be a 40-char git sha"
    );
    assert!(
        commit_hash.chars().all(|ch| ch.is_ascii_hexdigit()),
        "dataset.commit_hash should contain only hex digits"
    );
}
