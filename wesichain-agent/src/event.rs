use crate::AgentError;
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub enum AgentEvent {
    StepStarted { step_id: u32 },
    ModelResponded { step_id: u32 },
    ToolDispatched { step_id: u32 },
    ToolCompleted { step_id: u32 },
    StepFailed { step_id: u32, error: AgentError },
    Completed { step_id: u32 },
}

impl AgentEvent {
    pub fn step_id(&self) -> u32 {
        match self {
            AgentEvent::StepStarted { step_id }
            | AgentEvent::ModelResponded { step_id }
            | AgentEvent::ToolDispatched { step_id }
            | AgentEvent::ToolCompleted { step_id }
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
            AgentEvent::ToolDispatched { step_id } => {
                seen_dispatch.insert(*step_id);
                *outstanding_by_step.entry(*step_id).or_insert(0) += 1;
            }
            AgentEvent::ToolCompleted { step_id } => {
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
