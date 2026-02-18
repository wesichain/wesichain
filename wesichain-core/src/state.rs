use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashSet;
use std::hash::Hash;

/// Trait for defining how to merge concurrent updates for a specific field.
pub trait Reducer<T>: Send + Sync {
    fn reduce(&self, current: T, update: T) -> T;
}

/// Reducer that overwrites the current value with the update (Last-Write-Wins).
pub struct Overwrite;
impl<T> Reducer<T> for Overwrite {
    fn reduce(&self, _current: T, update: T) -> T {
        update
    }
}

/// Reducer that appends the update to the current value (for Vec<T>).
pub struct Append;
impl<T> Reducer<Vec<T>> for Append {
    fn reduce(&self, mut current: Vec<T>, mut update: Vec<T>) -> Vec<T> {
        current.append(&mut update);
        current
    }
}

/// Reducer that computes the union of the current and update sets (for HashSet<T>).
pub struct Union;
impl<T: Eq + Hash> Reducer<HashSet<T>> for Union {
    fn reduce(&self, mut current: HashSet<T>, update: HashSet<T>) -> HashSet<T> {
        current.extend(update);
        current
    }
}

pub trait StateSchema:
    Serialize + DeserializeOwned + Clone + Default + Send + Sync + std::fmt::Debug + 'static
{
    type Update: Serialize + DeserializeOwned + Clone + Default + Send + Sync + std::fmt::Debug + 'static;

    fn apply(current: &Self, update: Self::Update) -> Self;

    /// Human-readable representation for tracing/debugging.
    /// Override for custom formatting; default uses JSON serialization.
    fn trace_repr(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "<unserializable>".to_string())
    }
}

pub trait StateReducer: StateSchema {
    fn reduce(current: &Self, update: Self::Update) -> Self {
        <Self as StateSchema>::apply(current, update)
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
            data: S::apply(&self.data, update.data),
        }
    }

    pub fn apply(self, update: StateUpdate<S>) -> Self {
        self.apply_update(update)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(bound = "S: StateSchema")]
pub struct StateUpdate<S: StateSchema> {
    pub data: S::Update,
}
#[cfg(test)]
#[path = "state_tests.rs"]
mod state_tests;
impl<S: StateSchema> StateUpdate<S> {
    pub fn new(data: S::Update) -> Self {
        Self { data }
    }
}
