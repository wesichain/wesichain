use wesichain_retrieval::TextSplitter;

#[test]
fn text_splitter_slices_fixed_size_with_overlap() {
    let chunks = TextSplitter::split("abcdefghij", 4, 1);

    assert_eq!(chunks, vec!["abcd", "defg", "ghij"]);
}

#[test]
fn text_splitter_preserves_utf8_boundaries() {
    let chunks = TextSplitter::split("a🙂b🙂c", 2, 1);

    assert_eq!(chunks, vec!["a🙂", "🙂b", "b🙂", "🙂c"]);
}

#[test]
fn text_splitter_clamps_overlap_to_allow_progress() {
    let chunks = TextSplitter::split("abcd", 3, 5);

    assert_eq!(chunks, vec!["abc", "bcd"]);
}
