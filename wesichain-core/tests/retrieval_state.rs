use wesichain_core::{Document, HasMetadataFilter, HasQuery, HasRetrievedDocs, MetadataFilter};

struct DemoState;

impl HasQuery for DemoState {
    fn query(&self) -> &str {
        ""
    }
}

impl HasRetrievedDocs for DemoState {
    fn retrieved_docs(&self) -> &[Document] {
        &[]
    }
}

impl HasMetadataFilter for DemoState {
    fn metadata_filter(&self) -> Option<&MetadataFilter> {
        None
    }
}

fn assert_traits<T: HasQuery + HasRetrievedDocs + HasMetadataFilter>() {}

#[test]
fn state_traits_compile() {
    assert_traits::<DemoState>();
}
