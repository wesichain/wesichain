use crate::AgentError;

#[derive(Debug)]
pub enum AgentEvent {
    StepStarted { step_id: u32 },
    ModelResponded { step_id: u32 },
    ToolDispatched { step_id: u32 },
    ToolCompleted { step_id: u32 },
    StepFailed { step_id: u32, error: AgentError },
    Completed { step_id: u32 },
}
