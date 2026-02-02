use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub trait StateSchema:
    Serialize + DeserializeOwned + Clone + Default + Send + Sync + 'static
{
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(bound = "S: StateSchema")]
pub struct GraphState<S: StateSchema> {
    pub data: S,
}

impl<S: StateSchema> GraphState<S> {
    pub fn new(data: S) -> Self {
        Self { data }
    }

    pub fn apply(mut self, update: StateUpdate<S>) -> Self {
        self.data = update.data;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(bound = "S: StateSchema")]
pub struct StateUpdate<S: StateSchema> {
    pub data: S,
}

impl<S: StateSchema> StateUpdate<S> {
    pub fn new(data: S) -> Self {
        Self { data }
    }
}
