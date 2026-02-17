use super::StateSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct CounterState {
    count: i32,
    message: String,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
struct CounterUpdate {
    count: Option<i32>,
    message: Option<String>,
}

impl StateSchema for CounterState {
    type Update = CounterUpdate;

    fn apply(current: &Self, update: Self::Update) -> Self {
        Self {
            count: update.count.unwrap_or(current.count),
            message: update.message.unwrap_or_else(|| current.message.clone()),
        }
    }
}

#[test]
fn test_partial_update_application() {
    let initial = CounterState {
        count: 10,
        message: "initial".to_string(),
    };

    let update = CounterUpdate {
        count: Some(20),
        message: None,
    };

    let next = CounterState::apply(&initial, update);

    assert_eq!(next.count, 20);
    assert_eq!(next.message, "initial");
}

#[test]
fn test_full_update_application() {
    let initial = CounterState {
        count: 10,
        message: "initial".to_string(),
    };

    let update = CounterUpdate {
        count: Some(50),
        message: Some("updated".to_string()),
    };

    let next = CounterState::apply(&initial, update);

    assert_eq!(next.count, 50);
    assert_eq!(next.message, "updated");
}
