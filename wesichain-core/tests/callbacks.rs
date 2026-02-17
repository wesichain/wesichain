use std::collections::BTreeMap;

use wesichain_core::{ensure_object, CallbackManager, RunContext, RunType, Value};

#[test]
fn child_context_inherits_trace_and_parent() {
    let root = RunContext::root(RunType::Graph, "graph".to_string(), vec![], BTreeMap::new());
    let child = root.child(RunType::Chain, "node".to_string());
    assert_eq!(child.parent_run_id, Some(root.run_id));
    assert_eq!(child.trace_id, root.trace_id);
}

#[test]
fn ensure_object_wraps_primitives() {
    let value = Value::String("hello".to_string());
    let wrapped = ensure_object(value);
    assert!(wrapped.is_object());
}

#[test]
fn callback_manager_noop_has_no_handlers() {
    let manager = CallbackManager::noop();
    assert!(manager.is_noop());
}
