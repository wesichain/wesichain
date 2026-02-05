use std::collections::HashMap;

use petgraph::graph::{Graph, NodeIndex};
use petgraph::visit::EdgeRef;

use crate::graph::GraphNode;
use crate::StateSchema;

pub struct NodeData<S: StateSchema> {
    pub name: String,
    pub runnable: Box<dyn GraphNode<S>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeKind {
    Default,
}

pub struct GraphProgram<S: StateSchema> {
    graph: Graph<NodeData<S>, EdgeKind>,
    name_to_index: HashMap<String, NodeIndex>,
}

impl<S: StateSchema> GraphProgram<S> {
    pub(crate) fn new(
        graph: Graph<NodeData<S>, EdgeKind>,
        name_to_index: HashMap<String, NodeIndex>,
    ) -> Self {
        Self {
            graph,
            name_to_index,
        }
    }

    pub fn node_names(&self) -> Vec<String> {
        self.name_to_index.keys().cloned().collect()
    }

    pub fn edge_names(&self) -> Vec<(String, String)> {
        self.graph
            .edge_references()
            .filter_map(|edge| {
                let from = self.graph.node_weight(edge.source())?;
                let to = self.graph.node_weight(edge.target())?;
                Some((from.name.clone(), to.name.clone()))
            })
            .collect()
    }
}
