use wesichain_retrieval::TextSplitter;

#[test]
fn text_splitter_slices_fixed_size_with_overlap() {
    let chunks = TextSplitter::split("abcdefghij", 4, 1);

    assert_eq!(chunks, vec!["abcd", "defg", "ghij"]);
}
