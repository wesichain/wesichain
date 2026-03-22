use crate::AgentError;
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub enum AgentEvent {
    StepStarted { step_id: u32 },
    ModelResponded { step_id: u32 },
    /// A tool call was dispatched. `tool_name` is the name of the tool invoked,
    /// or `None` when the caller did not supply it.
    ToolDispatched { step_id: u32, tool_name: Option<String> },
    /// A tool call completed successfully. `tool_name` mirrors the dispatched
    /// name; `result` is the JSON-serialised output (if available).
    ToolCompleted {
        step_id: u32,
        tool_name: Option<String>,
        result: Option<serde_json::Value>,
    },
    StepFailed { step_id: u32, error: AgentError },
    Completed { step_id: u32 },
}

impl AgentEvent {
    pub fn step_id(&self) -> u32 {
        match self {
            AgentEvent::StepStarted { step_id }
            | AgentEvent::ModelResponded { step_id }
            | AgentEvent::ToolDispatched { step_id, .. }
            | AgentEvent::ToolCompleted { step_id, .. }
            | AgentEvent::StepFailed { step_id, .. }
            | AgentEvent::Completed { step_id } => *step_id,
        }
    }

    fn is_terminal(&self) -> bool {
        matches!(
            self,
            AgentEvent::ToolCompleted { .. }
                | AgentEvent::StepFailed { .. }
                | AgentEvent::Completed { .. }
        )
    }
}

pub fn validate_step_started_precedes_terminal(events: &[AgentEvent]) -> Result<(), String> {
    let mut started: HashSet<u32> = HashSet::new();

    for (index, event) in events.iter().enumerate() {
        match event {
            AgentEvent::StepStarted { step_id } => {
                started.insert(*step_id);
            }
            _ if event.is_terminal() => {
                let step_id = event.step_id();
                if !started.contains(&step_id) {
                    return Err(format!(
                        "terminal event before StepStarted for step {step_id} at index {index}"
                    ));
                }
            }
            _ => {}
        }
    }

    Ok(())
}

pub fn validate_tool_dispatch_cardinality(events: &[AgentEvent]) -> Result<(), String> {
    let mut outstanding_by_step: HashMap<u32, u32> = HashMap::new();
    let mut seen_dispatch: HashSet<u32> = HashSet::new();

    for (index, event) in events.iter().enumerate() {
        match event {
            AgentEvent::ToolDispatched { step_id, .. } => {
                seen_dispatch.insert(*step_id);
                *outstanding_by_step.entry(*step_id).or_insert(0) += 1;
            }
            AgentEvent::ToolCompleted { step_id, .. } => {
                let outstanding = outstanding_by_step.entry(*step_id).or_insert(0);
                if *outstanding == 0 {
                    return Err(format!(
                        "ToolCompleted without dispatch for step {step_id} at index {index}"
                    ));
                }
                *outstanding -= 1;
            }
            AgentEvent::StepFailed { step_id, .. } => {
                let outstanding = outstanding_by_step.entry(*step_id).or_insert(0);
                if *outstanding > 0 {
                    *outstanding -= 1;
                } else if seen_dispatch.contains(step_id) {
                    return Err(format!(
                        "extra StepFailed counterpart for step {step_id} at index {index}"
                    ));
                }
            }
            _ => {}
        }
    }

    for (step_id, outstanding) in outstanding_by_step {
        if outstanding != 0 {
            return Err(format!(
                "missing completion/failure counterpart for step {step_id}: {outstanding} dispatch(es) still open"
            ));
        }
    }

    Ok(())
}

pub fn validate_completed_once(events: &[AgentEvent]) -> Result<(), String> {
    let mut completed_index: Option<usize> = None;

    for (index, event) in events.iter().enumerate() {
        if matches!(event, AgentEvent::Completed { .. }) {
            if let Some(first_index) = completed_index {
                return Err(format!(
                    "Completed emitted more than once (first at index {first_index}, duplicate at index {index})"
                ));
            }
            completed_index = Some(index);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step_started(step_id: u32) -> AgentEvent {
        AgentEvent::StepStarted { step_id }
    }
    fn model_responded(step_id: u32) -> AgentEvent {
        AgentEvent::ModelResponded { step_id }
    }
    fn tool_dispatched(step_id: u32) -> AgentEvent {
        AgentEvent::ToolDispatched { step_id, tool_name: None }
    }
    fn tool_completed(step_id: u32) -> AgentEvent {
        AgentEvent::ToolCompleted { step_id, tool_name: None, result: None }
    }
    fn step_failed(step_id: u32) -> AgentEvent {
        AgentEvent::StepFailed {
            step_id,
            error: AgentError::ToolDispatch,
        }
    }
    fn completed(step_id: u32) -> AgentEvent {
        AgentEvent::Completed { step_id }
    }

    // ---------------------------------------------------------------------------
    // validate_step_started_precedes_terminal
    // ---------------------------------------------------------------------------

    #[test]
    fn valid_sequence_passes() {
        let events = vec![
            step_started(1),
            model_responded(1),
            tool_dispatched(1),
            tool_completed(1),
        ];
        assert!(validate_step_started_precedes_terminal(&events).is_ok());
    }

    #[test]
    fn terminal_before_step_started_is_error() {
        let events = vec![
            tool_completed(1), // no StepStarted before this
        ];
        assert!(validate_step_started_precedes_terminal(&events).is_err());
    }

    #[test]
    fn completed_without_step_started_is_error() {
        let events = vec![completed(1)];
        assert!(validate_step_started_precedes_terminal(&events).is_err());
    }

    #[test]
    fn step_failed_without_step_started_is_error() {
        let events = vec![step_failed(1)];
        assert!(validate_step_started_precedes_terminal(&events).is_err());
    }

    #[test]
    fn multiple_steps_all_valid() {
        let events = vec![
            step_started(1),
            tool_dispatched(1),
            tool_completed(1),
            step_started(2),
            completed(2),
        ];
        assert!(validate_step_started_precedes_terminal(&events).is_ok());
    }

    // ---------------------------------------------------------------------------
    // validate_tool_dispatch_cardinality
    // ---------------------------------------------------------------------------

    #[test]
    fn balanced_dispatch_and_complete_passes() {
        let events = vec![
            step_started(1),
            tool_dispatched(1),
            tool_completed(1),
        ];
        assert!(validate_tool_dispatch_cardinality(&events).is_ok());
    }

    #[test]
    fn tool_completed_without_dispatch_is_error() {
        let events = vec![
            step_started(1),
            tool_completed(1), // no dispatch
        ];
        assert!(validate_tool_dispatch_cardinality(&events).is_err());
    }

    #[test]
    fn unmatched_dispatch_without_completion_is_error() {
        let events = vec![
            step_started(1),
            tool_dispatched(1),
            // missing ToolCompleted or StepFailed
        ];
        assert!(validate_tool_dispatch_cardinality(&events).is_err());
    }

    #[test]
    fn dispatch_followed_by_step_failed_passes() {
        let events = vec![
            step_started(1),
            tool_dispatched(1),
            step_failed(1),
        ];
        assert!(validate_tool_dispatch_cardinality(&events).is_ok());
    }

    // ---------------------------------------------------------------------------
    // validate_completed_once
    // ---------------------------------------------------------------------------

    #[test]
    fn single_completed_passes() {
        let events = vec![step_started(1), model_responded(1), completed(1)];
        assert!(validate_completed_once(&events).is_ok());
    }

    #[test]
    fn no_completed_passes() {
        let events = vec![step_started(1), tool_dispatched(1), tool_completed(1)];
        assert!(validate_completed_once(&events).is_ok());
    }

    #[test]
    fn completed_twice_is_error() {
        let events = vec![
            step_started(1),
            completed(1),
            step_started(2),
            completed(2), // second Completed event
        ];
        assert!(validate_completed_once(&events).is_err());
    }
}
