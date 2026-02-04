use std::collections::HashMap;

use petgraph::graph::{Graph, NodeIndex};

use crate::{GraphState, StateSchema, StateUpdate};
use wesichain_core::Runnable;

pub struct NodeData<S: StateSchema> {
    pub name: String,
    pub runnable: Box<dyn Runnable<GraphState<S>, StateUpdate<S>> + Send + Sync>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeKind {
    Default,
}

pub struct GraphProgram<S: StateSchema> {
    pub graph: Graph<NodeData<S>, EdgeKind>,
    pub name_to_index: HashMap<String, NodeIndex>,
}
