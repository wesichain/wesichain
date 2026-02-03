use std::sync::Arc;

use wesichain_core::VectorStore;

#[test]
fn vector_store_trait_object_safe() {
    let _: Option<Arc<dyn VectorStore>> = None;
}
