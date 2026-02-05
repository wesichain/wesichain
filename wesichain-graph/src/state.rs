use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub trait StateSchema:
    Serialize + DeserializeOwned + Clone + Default + Send + Sync + 'static
{
    fn merge(_current: &Self, update: Self) -> Self {
        update
    }
}

pub trait StateReducer: StateSchema {
    fn merge(current: &Self, update: Self) -> Self {
        <Self as StateSchema>::merge(current, update)
    }
}

impl<T: StateSchema> StateReducer for T {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(bound = "S: StateSchema")]
pub struct GraphState<S: StateSchema> {
    pub data: S,
}

impl<S: StateSchema> GraphState<S> {
    pub fn new(data: S) -> Self {
        Self { data }
    }

    pub fn apply_update(self, update: StateUpdate<S>) -> Self {
        Self {
            data: S::merge(&self.data, update.data),
        }
    }

    pub fn apply(self, update: StateUpdate<S>) -> Self {
        self.apply_update(update)
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
