use wesichain_core::{Document, HasMetadataFilter, HasQuery, HasRetrievedDocs, MetadataFilter};

struct DemoState {
    docs: Vec<Document>,
    filter: Option<MetadataFilter>,
}

impl HasQuery for DemoState {
    fn query(&self) -> &str {
        ""
    }
}

impl HasRetrievedDocs for DemoState {
    fn set_retrieved_docs(&mut self, docs: Vec<Document>) {
        self.docs = docs;
    }
}

impl HasMetadataFilter for DemoState {
    fn metadata_filter(&self) -> Option<MetadataFilter> {
        self.filter.clone()
    }
}

fn assert_traits<T: HasQuery + HasRetrievedDocs + HasMetadataFilter>() {}

#[test]
fn state_traits_compile() {
    assert_traits::<DemoState>();
}
