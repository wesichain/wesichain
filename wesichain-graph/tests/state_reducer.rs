use serde::{Deserialize, Serialize};
use wesichain_graph::{GraphState, StateSchema, StateUpdate};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct MergeState {
    messages: Vec<String>,
    count: i32,
}

impl StateSchema for MergeState {
    fn merge(current: &Self, update: Self) -> Self {
        let mut merged = current.clone();
        merged.messages.extend(update.messages);
        merged.count += update.count;
        merged
    }
}

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct OverrideState {
    count: i32,
}

impl StateSchema for OverrideState {}

#[test]
fn state_merge_appends_and_adds() {
    let base = GraphState::new(MergeState {
        messages: vec!["a".to_string()],
        count: 1,
    });
    let update = StateUpdate::new(MergeState {
        messages: vec!["b".to_string(), "c".to_string()],
        count: 2,
    });
    let merged = base.apply_update(update);
    assert_eq!(merged.data.messages, vec!["a", "b", "c"]);
    assert_eq!(merged.data.count, 3);
}

#[test]
fn state_merge_defaults_to_override() {
    let base = GraphState::new(OverrideState { count: 1 });
    let update = StateUpdate::new(OverrideState { count: 9 });
    let merged = base.apply_update(update);
    assert_eq!(merged.data.count, 9);
}
