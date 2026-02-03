use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::Value;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Document {
    pub id: String,
    pub content: String,
    pub metadata: HashMap<String, Value>,
    pub embedding: Option<Vec<f32>>,
}
