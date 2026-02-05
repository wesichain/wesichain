use crate::{Document, MetadataFilter};

pub trait HasQuery {
    fn query(&self) -> &str;
}

pub trait HasRetrievedDocs {
    fn set_retrieved_docs(&mut self, docs: Vec<Document>);
}

pub trait HasMetadataFilter {
    fn metadata_filter(&self) -> Option<MetadataFilter>;
}
