#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunType {
    Chain,
    Tool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Clone, Debug)]
pub enum RunEvent {
    Start,
    Update,
}
