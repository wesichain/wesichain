use std::collections::HashMap;

use wesichain_core::{
    Document, HasMetadataFilter, HasQuery, HasRetrievedDocs, MetadataFilter, Value,
};

struct ExampleState {
    query: String,
    retrieved_docs: Vec<Document>,
    metadata_filter: Option<MetadataFilter>,
}

impl HasQuery for ExampleState {
    fn query(&self) -> &str {
        &self.query
    }
}

impl HasRetrievedDocs for ExampleState {
    fn retrieved_docs(&self) -> &[Document] {
        &self.retrieved_docs
    }
}

impl HasMetadataFilter for ExampleState {
    fn metadata_filter(&self) -> Option<&MetadataFilter> {
        self.metadata_filter.as_ref()
    }
}

fn assert_query<T: HasQuery>(state: &T) -> &str {
    state.query()
}

fn assert_docs<T: HasRetrievedDocs>(state: &T) -> &[Document] {
    state.retrieved_docs()
}

fn assert_filter<T: HasMetadataFilter>(state: &T) -> Option<&MetadataFilter> {
    state.metadata_filter()
}

#[test]
fn retrieval_state_traits_compile() {
    let mut metadata = HashMap::new();
    metadata.insert("source".to_string(), Value::String("unit".to_string()));

    let doc = Document {
        id: "doc-1".to_string(),
        content: "hello".to_string(),
        metadata,
        embedding: None,
    };

    let state = ExampleState {
        query: "find docs".to_string(),
        retrieved_docs: vec![doc],
        metadata_filter: Some(MetadataFilter::Eq(
            "source".to_string(),
            Value::String("unit".to_string()),
        )),
    };

    let _ = assert_query(&state);
    let _ = assert_docs(&state);
    let _ = assert_filter(&state);
}
