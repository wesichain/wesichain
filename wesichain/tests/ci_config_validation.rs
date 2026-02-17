use std::fs;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("wesichain crate should live at workspace root/wesichain")
        .to_path_buf()
}

fn contains_line(content: &str, expected: &str) -> bool {
    content.lines().any(|line| line.trim() == expected)
}

#[test]
fn ci_config_files_exist_and_match_locked_policy() {
    let root = workspace_root();

    let required_files = [
        ".github/workflows/pr-checks.yml",
        ".github/workflows/nightly-bench.yml",
        "tools/bench/thresholds.toml",
        "tools/ci/impact-map.toml",
        "WAIVERS.yml",
    ];

    for rel_path in required_files {
        let path = root.join(rel_path);
        assert!(path.exists(), "required CI file missing: {rel_path}");
    }

    let thresholds = fs::read_to_string(root.join("tools/bench/thresholds.toml"))
        .expect("thresholds.toml must be readable");
    for expected in [
        "query_p50 = 10",
        "query_p95 = 15",
        "query_p99 = 25",
        "index_throughput = 20",
        "peak_memory = 30",
        "error_rate = 0",
    ] {
        assert!(
            thresholds.contains(expected),
            "missing locked threshold setting: {expected}"
        );
    }

    let impact_map = fs::read_to_string(root.join("tools/ci/impact-map.toml"))
        .expect("impact-map.toml must be readable");
    assert!(impact_map.contains("connector_examples"));
    assert!(impact_map.contains("core_trait_change"));
    assert!(impact_map.contains("wesichain-core/src/*.rs"));
    assert!(
        impact_map.contains("wesichain-weaviate:rag_integration"),
        "impact map must include weaviate connector example fan-out"
    );

    let pr_checks = fs::read_to_string(root.join(".github/workflows/pr-checks.yml"))
        .expect("pr-checks.yml must be readable");
    assert!(
        contains_line(
            &pr_checks,
            "run: cargo test -p ${{ matrix.crate }} --tests -- --nocapture"
        ),
        "PR touched-crate job must run tests"
    );
    assert!(
        pr_checks.contains("connector_examples=${{ steps.detect.outputs.connector_examples }}")
            || pr_checks.contains("connector_examples={json.dumps(connector_examples)}"),
        "PR impact detection must emit connector_examples output"
    );
    assert!(
        contains_line(
            &pr_checks,
            "run: cargo bench -p wesichain-weaviate --bench vs_langchain -- --sample-size 10",
        ),
        "PR checks must include weaviate advisory benchmark"
    );
    assert!(
        contains_line(
            &pr_checks,
            "run: cargo clippy --all-targets --all-features -- -D warnings",
        ),
        "PR clippy job must include all-targets and all-features"
    );

    let nightly = fs::read_to_string(root.join(".github/workflows/nightly-bench.yml"))
        .expect("nightly-bench.yml must be readable");
    assert!(nightly.contains("--waivers WAIVERS.yml"));
    assert!(
        nightly.contains("/usr/bin/time -v"),
        "nightly benchmark must capture benchmark process RSS"
    );
    assert!(
        contains_line(
            &nightly,
            "cargo bench -p wesichain-weaviate --bench vs_langchain -- --sample-size 10",
        ),
        "nightly benchmarks must include weaviate threshold-gated benchmark"
    );
    assert!(
        contains_line(
            &nightly,
            "--criterion-benchmark-name wesichain_object_payload \\",
        ),
        "nightly weaviate gate must pass explicit criterion benchmark name"
    );
}
