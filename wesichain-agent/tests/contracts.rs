use wesichain_agent::{AgentError, AgentEvent, PolicyDecision, RepromptStrategy};

#[test]
fn policy_decision_has_required_variants() {
    let _ = PolicyDecision::Fail;
    let _ = PolicyDecision::Retry {
        consume_budget: true,
    };
    let _ = PolicyDecision::Reprompt {
        strategy: RepromptStrategy::OnceWithToolCatalog,
        consume_budget: true,
    };
    let _ = PolicyDecision::Interrupt;
}

#[test]
fn invalid_model_action_carries_debug_payload() {
    let err = AgentError::InvalidModelAction {
        step_id: 2,
        tool_name: Some("calculator".to_string()),
        received_args: serde_json::json!({"bad": true}),
        raw_response: serde_json::json!({"tool_calls": []}),
    };

    assert!(err.to_string().contains("Invalid model action"));
}

#[test]
fn agent_event_has_step_started_and_completed() {
    let start = AgentEvent::StepStarted { step_id: 1 };
    let done = AgentEvent::Completed { step_id: 1 };

    assert_ne!(format!("{start:?}"), format!("{done:?}"));
}
