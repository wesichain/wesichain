use std::io::Write;
use std::path::PathBuf;
use tempfile::NamedTempFile;
use wesichain_retrieval::load_file_async;

#[tokio::test]
async fn test_load_html_basic() {
    let html_content = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <title>Test Page</title>
</head>
<body>
    <h1>Main Heading</h1>
    <p>This is a paragraph.</p>
    <p>Another paragraph with <strong>bold</strong> text.</p>
</body>
</html>
"#;

    let mut temp_file = NamedTempFile::with_suffix(".html").unwrap();
    temp_file.write_all(html_content.as_bytes()).unwrap();
    temp_file.flush().unwrap();

    let path = PathBuf::from(temp_file.path());
    let documents = load_file_async(path).await.unwrap();

    assert_eq!(documents.len(), 1);
    let doc = &documents[0];

    // Check content extraction
    assert!(doc.content.contains("Main Heading"));
    assert!(doc.content.contains("This is a paragraph"));
    assert!(doc.content.contains("bold"));

    // Check metadata
    assert_eq!(
        doc.metadata.get("title").and_then(|v| v.as_str()),
        Some("Test Page")
    );
    assert_eq!(
        doc.metadata.get("lang").and_then(|v| v.as_str()),
        Some("en")
    );
}

#[tokio::test]
async fn test_load_html_strips_scripts_and_styles() {
    let html_content = r#"
<!DOCTYPE html>
<html>
<head>
    <style>
        body { color: red; }
    </style>
    <script>
        console.log("test");
    </script>
</head>
<body>
    <p>Visible content</p>
    <script>alert("more js")</script>
    <nav>Navigation menu</nav>
    <header>Header content</header>
    <footer>Footer content</footer>
</body>
</html>
"#;

    let mut temp_file = NamedTempFile::with_suffix(".html").unwrap();
    temp_file.write_all(html_content.as_bytes()).unwrap();
    temp_file.flush().unwrap();

    let path = PathBuf::from(temp_file.path());
    let documents = load_file_async(path).await.unwrap();

    let doc = &documents[0];

    // Should include visible content
    assert!(doc.content.contains("Visible content"));

    // Should NOT include script/style/nav/header/footer content
    assert!(!doc.content.contains("color: red"));
    assert!(!doc.content.contains("console.log"));
    assert!(!doc.content.contains("alert"));
    assert!(!doc.content.contains("Navigation menu"));
    assert!(!doc.content.contains("Header content"));
    assert!(!doc.content.contains("Footer content"));
}

#[tokio::test]
async fn test_load_html_preserves_structure() {
    let html_content = r#"
<!DOCTYPE html>
<html>
<body>
    <div>
        <h2>Section 1</h2>
        <p>Content 1</p>
    </div>
    <div>
        <h2>Section 2</h2>
        <p>Content 2</p>
    </div>
</body>
</html>
"#;

    let mut temp_file = NamedTempFile::with_suffix(".html").unwrap();
    temp_file.write_all(html_content.as_bytes()).unwrap();
    temp_file.flush().unwrap();

    let path = PathBuf::from(temp_file.path());
    let documents = load_file_async(path).await.unwrap();

    let doc = &documents[0];

    // All content should be present
    assert!(doc.content.contains("Section 1"));
    assert!(doc.content.contains("Content 1"));
    assert!(doc.content.contains("Section 2"));
    assert!(doc.content.contains("Content 2"));
}

#[tokio::test]
async fn test_load_html_malformed_markup() {
    let html_content = r#"
<!DOCTYPE html>
<html>
<body>
    <h1>Unclosed heading
    <p>Paragraph with <strong>unclosed bold and <a href="">nested anchor</a>
    <div>
        <p>Content in unclosed div
        <ul>
            <li>Unclosed list item
            <li>Another item
    </div>
    <p>Recovery after malformed section</p>
</body>
</html>
"#;

    let mut temp_file = NamedTempFile::with_suffix(".html").unwrap();
    temp_file.write_all(html_content.as_bytes()).unwrap();
    temp_file.flush().unwrap();

    let path = PathBuf::from(temp_file.path());
    let documents = load_file_async(path).await.unwrap();

    assert_eq!(documents.len(), 1);
    let doc = &documents[0];

    // Should extract text despite malformed markup
    assert!(doc.content.contains("Unclosed heading"));
    assert!(doc.content.contains("Paragraph with"));
    assert!(doc.content.contains("unclosed bold"));
    assert!(doc.content.contains("nested anchor"));
    assert!(doc.content.contains("Unclosed list item"));
    assert!(doc.content.contains("Recovery after malformed section"));

    // Content should not be empty
    assert!(!doc.content.trim().is_empty());
    assert!(doc.content.len() > 50);
}
