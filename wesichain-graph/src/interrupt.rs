use crate::{GraphState, StateSchema};

#[derive(Clone, Debug)]
pub struct GraphInterrupt<S: StateSchema> {
    pub node: String,
    pub state: GraphState<S>,
}
