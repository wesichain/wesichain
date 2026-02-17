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
