use wesichain_retrieval::TextSplitter;

#[test]
fn text_splitter_splits_on_whitespace_with_max_chars() {
    let splitter = TextSplitter::new(10);
    let chunks = splitter.split("hello world from wesichain");

    assert_eq!(chunks, vec!["hello", "world from", "wesichain"]);
}

#[test]
fn text_splitter_returns_empty_for_whitespace_only() {
    let splitter = TextSplitter::new(5);
    let chunks = splitter.split("   \n\t  ");

    assert!(chunks.is_empty());
}
