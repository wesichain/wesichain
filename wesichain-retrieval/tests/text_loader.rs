use std::io::Write;

use tempfile::NamedTempFile;
use wesichain_retrieval::TextLoader;

#[test]
fn text_loader_loads_text_file_contents() {
    let mut file = NamedTempFile::new().expect("temp file");
    write!(file, "Hello, Wesichain!").expect("write temp file");

    let loader = TextLoader::new();
    let contents = loader.load(file.path()).expect("load temp file");

    assert_eq!(contents, "Hello, Wesichain!");
}

#[test]
fn text_loader_returns_error_for_missing_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let missing_path = dir.path().join("missing.txt");

    let loader = TextLoader::new();
    let result = loader.load(&missing_path);

    assert!(result.is_err());
}
