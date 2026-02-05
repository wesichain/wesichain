use uuid::Uuid;
use wesichain_langsmith::{RunContextStore, RunStatus};

#[test]
fn first_terminal_event_is_authoritative() {
    let store = RunContextStore::default();
    let run_id = Uuid::new_v4();

    store.record_start(run_id, None);
    let first = store.apply_update(run_id, Some("boom".to_string()));
    let second = store.apply_update(run_id, None);

    assert_eq!(first.status, RunStatus::Failed);
    assert_eq!(second.status, RunStatus::Failed);
    assert_eq!(second.error.as_deref(), Some("boom"));
}
