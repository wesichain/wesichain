use wesichain_agent::{
    emit_single_step_events, emit_tool_step_events, validate_completed_once,
    validate_step_started_precedes_terminal, validate_tool_dispatch_cardinality, AgentError,
    AgentEvent, ToolDispatchOutcome,
};

#[test]
fn step_started_precedes_terminal_event_for_each_step() {
    let events = emit_single_step_events(1);

    validate_step_started_precedes_terminal(&events).unwrap();

    let first = events.first().expect("at least one event");
    assert!(matches!(first, AgentEvent::StepStarted { step_id: 1 }));
}

#[test]
fn each_tool_dispatched_has_exactly_one_completion_or_failure_counterpart() {
    let mut events = emit_tool_step_events(1, ToolDispatchOutcome::Completed);
    events.extend(emit_tool_step_events(
        2,
        ToolDispatchOutcome::Failed(AgentError::ToolDispatch),
    ));

    validate_tool_dispatch_cardinality(&events).unwrap();
}

#[test]
fn completed_event_is_emitted_only_once() {
    let events = vec![
        AgentEvent::StepStarted { step_id: 1 },
        AgentEvent::ModelResponded { step_id: 1 },
        AgentEvent::Completed { step_id: 1 },
        AgentEvent::StepStarted { step_id: 2 },
        AgentEvent::ModelResponded { step_id: 2 },
        AgentEvent::Completed { step_id: 2 },
    ];

    assert!(validate_completed_once(&events).is_err());
}
