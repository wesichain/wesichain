use crate::{Document, MetadataFilter};

pub trait HasQuery {
    fn query(&self) -> &str;
}

pub trait HasRetrievedDocs {
    fn retrieved_docs(&self) -> &[Document];
}

pub trait HasMetadataFilter {
    fn metadata_filter(&self) -> Option<&MetadataFilter>;
}
