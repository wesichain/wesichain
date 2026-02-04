use std::collections::HashMap;

pub struct AppendVec;
impl AppendVec {
    pub fn merge<T: Clone>(current: &Vec<T>, mut update: Vec<T>) -> Vec<T> {
        let mut out = current.clone();
        out.append(&mut update);
        out
    }
}

pub struct MergeMap;
impl MergeMap {
    pub fn merge<K: Eq + std::hash::Hash + Clone, V: Clone>(
        current: &HashMap<K, V>,
        update: HashMap<K, V>,
    ) -> HashMap<K, V> {
        let mut out = current.clone();
        out.extend(update);
        out
    }
}

pub struct AddCounter;
impl AddCounter {
    pub fn merge(current: &i64, update: i64) -> i64 {
        current + update
    }
}

pub struct Override;
impl Override {
    pub fn merge<T>(_current: &T, update: T) -> T {
        update
    }
}
