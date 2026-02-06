use serde_json::{json, Value};
use wesichain_core::MetadataFilter;

use crate::error::PineconeStoreError;

#[derive(Clone, Debug)]
pub enum PineconeFilter {
    Typed(MetadataFilter),
    Raw(Value),
}

pub fn to_pinecone_filter_json(filter: &PineconeFilter) -> Result<Value, PineconeStoreError> {
    match filter {
        PineconeFilter::Raw(value) => Ok(value.clone()),
        PineconeFilter::Typed(filter) => metadata_filter_to_json(filter),
    }
}

fn metadata_filter_to_json(filter: &MetadataFilter) -> Result<Value, PineconeStoreError> {
    Ok(match filter {
        MetadataFilter::Eq(key, value) => json!({ key: { "$eq": value } }),
        MetadataFilter::In(key, values) => json!({ key: { "$in": values } }),
        MetadataFilter::Range { key, min, max } => {
            let mut inner = serde_json::Map::new();
            if let Some(min) = min {
                inner.insert("$gte".to_string(), min.clone());
            }
            if let Some(max) = max {
                inner.insert("$lte".to_string(), max.clone());
            }
            Value::Object(serde_json::Map::from_iter([(
                key.clone(),
                Value::Object(inner),
            )]))
        }
        MetadataFilter::All(filters) => {
            let list: Result<Vec<_>, _> = filters.iter().map(metadata_filter_to_json).collect();
            json!({ "$and": list? })
        }
        MetadataFilter::Any(filters) => {
            let list: Result<Vec<_>, _> = filters.iter().map(metadata_filter_to_json).collect();
            json!({ "$or": list? })
        }
    })
}
