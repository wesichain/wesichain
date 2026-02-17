use std::io::Write;
use std::path::PathBuf;
use tempfile::NamedTempFile;
use wesichain_retrieval::load_file_async;

#[tokio::test]
async fn test_load_markdown_basic() {
    let markdown_content = r#"# Main Title

This is a paragraph.

## Subsection

Another paragraph with **bold** and *italic* text.

### Level 3

Final content.
"#;

    let mut temp_file = NamedTempFile::with_suffix(".md").unwrap();
    temp_file.write_all(markdown_content.as_bytes()).unwrap();
    temp_file.flush().unwrap();

    let path = PathBuf::from(temp_file.path());
    let documents = load_file_async(path).await.unwrap();

    assert_eq!(documents.len(), 1);
    let doc = &documents[0];

    // Check content extraction
    assert!(doc.content.contains("Main Title"));
    assert!(doc.content.contains("This is a paragraph"));
    assert!(doc.content.contains("Subsection"));
    assert!(doc.content.contains("bold"));
    assert!(doc.content.contains("italic"));
}

#[tokio::test]
async fn test_load_markdown_header_metadata() {
    let markdown_content = r#"# Top Level

Content under top.

## Second Level

More content.

### Third Level

Even more.
"#;

    let mut temp_file = NamedTempFile::with_suffix(".md").unwrap();
    temp_file.write_all(markdown_content.as_bytes()).unwrap();
    temp_file.flush().unwrap();

    let path = PathBuf::from(temp_file.path());
    let documents = load_file_async(path).await.unwrap();

    let doc = &documents[0];

    // Check headers metadata exists
    let headers = doc.metadata.get("headers").and_then(|v| v.as_array());
    assert!(headers.is_some());

    let headers = headers.unwrap();
    assert_eq!(headers.len(), 3);

    // Check first header
    let h1 = &headers[0];
    assert_eq!(h1.get("level").and_then(|v| v.as_u64()), Some(1));
    assert_eq!(
        h1.get("text").and_then(|v| v.as_str()),
        Some("Top Level")
    );

    // Check second header
    let h2 = &headers[1];
    assert_eq!(h2.get("level").and_then(|v| v.as_u64()), Some(2));
    assert_eq!(
        h2.get("text").and_then(|v| v.as_str()),
        Some("Second Level")
    );

    // Check third header
    let h3 = &headers[2];
    assert_eq!(h3.get("level").and_then(|v| v.as_u64()), Some(3));
    assert_eq!(
        h3.get("text").and_then(|v| v.as_str()),
        Some("Third Level")
    );
}

#[tokio::test]
async fn test_load_markdown_with_code_blocks() {
    let markdown_content = r#"# Code Example

Here's some code:

```rust
fn main() {
    println!("Hello");
}
```

And back to text.
"#;

    let mut temp_file = NamedTempFile::with_suffix(".md").unwrap();
    temp_file.write_all(markdown_content.as_bytes()).unwrap();
    temp_file.flush().unwrap();

    let path = PathBuf::from(temp_file.path());
    let documents = load_file_async(path).await.unwrap();

    let doc = &documents[0];

    // Code content should be included
    assert!(doc.content.contains("fn main"));
    assert!(doc.content.contains("println"));
}

#[tokio::test]
async fn test_load_markdown_with_lists() {
    let markdown_content = r#"# Lists

* Item 1
* Item 2
  * Nested item
* Item 3

1. Numbered one
2. Numbered two
"#;

    let mut temp_file = NamedTempFile::with_suffix(".md").unwrap();
    temp_file.write_all(markdown_content.as_bytes()).unwrap();
    temp_file.flush().unwrap();

    let path = PathBuf::from(temp_file.path());
    let documents = load_file_async(path).await.unwrap();

    let doc = &documents[0];

    // List items should be included
    assert!(doc.content.contains("Item 1"));
    assert!(doc.content.contains("Item 2"));
    assert!(doc.content.contains("Nested item"));
    assert!(doc.content.contains("Numbered one"));
}
