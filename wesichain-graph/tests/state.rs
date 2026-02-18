use serde::{Deserialize, Serialize};
use wesichain_graph::{GraphState, StateSchema, StateUpdate};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct DemoState {
    count: i32,
}

impl StateSchema for DemoState {
    type Update = Self;
    fn apply(_: &Self, update: Self) -> Self {
        update
    }
}

#[test]
fn state_update_merges_last_write() {
    let base = GraphState::new(DemoState { count: 1 });
    let update = StateUpdate::new(DemoState { count: 2 });
    let merged = base.apply(update);
    assert_eq!(merged.data.count, 2);
}
