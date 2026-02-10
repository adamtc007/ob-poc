use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};

/// Gateway direction for parallel/exclusive gateways.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GatewayDirection {
    Diverging,
    Converging,
}

/// Timer specification.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TimerSpec {
    Duration { ms: u64 },
    Date { deadline_ms: u64 },
    Cycle { interval_ms: u64, max_fires: u32 },
}

/// Condition expression for XOR gateway edges.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConditionExpr {
    pub flag_name: String,
    pub op: ConditionOp,
    pub literal: ConditionLiteral,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ConditionOp {
    Eq,
    Neq,
    Lt,
    Gt,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ConditionLiteral {
    Bool(bool),
    I64(i64),
}

/// IR node — one per BPMN element.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum IRNode {
    Start {
        id: String,
    },
    End {
        id: String,
        terminate: bool,
    },
    ServiceTask {
        id: String,
        name: String,
        task_type: String,
    },
    GatewayXor {
        id: String,
        name: String,
    },
    GatewayAnd {
        id: String,
        name: String,
        direction: GatewayDirection,
    },
    TimerWait {
        id: String,
        spec: TimerSpec,
    },
    MessageWait {
        id: String,
        name: String,
        corr_key_source: String,
    },
    HumanWait {
        id: String,
        name: String,
        task_kind: String,
        corr_key_source: String,
    },
    BoundaryTimer {
        id: String,
        attached_to: String,
        spec: TimerSpec,
        interrupting: bool,
    },
    BoundaryError {
        id: String,
        attached_to: String,
        error_code: Option<String>,
    },
    GatewayInclusive {
        id: String,
        name: String,
        direction: GatewayDirection,
    },
}

impl IRNode {
    pub fn id(&self) -> &str {
        match self {
            IRNode::Start { id } => id,
            IRNode::End { id, .. } => id,
            IRNode::ServiceTask { id, .. } => id,
            IRNode::GatewayXor { id, .. } => id,
            IRNode::GatewayAnd { id, .. } => id,
            IRNode::TimerWait { id, .. } => id,
            IRNode::MessageWait { id, .. } => id,
            IRNode::HumanWait { id, .. } => id,
            IRNode::BoundaryTimer { id, .. } => id,
            IRNode::BoundaryError { id, .. } => id,
            IRNode::GatewayInclusive { id, .. } => id,
        }
    }
}

/// IR edge — one per sequence flow.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IREdge {
    pub id: String,
    pub condition: Option<ConditionExpr>,
}

/// The intermediate representation — a directed graph of BPMN elements.
pub type IRGraph = DiGraph<IRNode, IREdge>;

/// Helper to find a node by its BPMN element id.
pub fn find_node_by_id(graph: &IRGraph, element_id: &str) -> Option<NodeIndex> {
    graph
        .node_indices()
        .find(|&idx| graph[idx].id() == element_id)
}

/// Helper to find the start node.
pub fn find_start(graph: &IRGraph) -> Option<NodeIndex> {
    graph
        .node_indices()
        .find(|&idx| matches!(&graph[idx], IRNode::Start { .. }))
}
