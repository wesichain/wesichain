use serde::{Deserialize, Serialize};

use crate::Value;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum MetadataFilter {
    Eq(String, Value),
    In(String, Vec<Value>),
    Range {
        key: String,
        min: Option<Value>,
        max: Option<Value>,
    },
    All(Vec<MetadataFilter>),
    Any(Vec<MetadataFilter>),
}
