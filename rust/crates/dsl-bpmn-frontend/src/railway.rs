//! Typed railway graph produced by the bpmn-lite assembly pass.
//!
//! A `RailwayGraph` is the output of [`crate::assemble`]: a structured
//! representation of the process with typed nodes, gateways, directed edges,
//! boundary attachments, and parallel-join merge declarations.

use serde::Serialize;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Node kind
// ---------------------------------------------------------------------------

/// The kind of a process node (event or activity).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum NodeKind {
    // Start events
    StartEvent,
    StartEventMessage,
    StartEventTimer,
    StartEventSignal,
    StartEventError,
    StartEventEscalation,
    StartEventCompensation,
    // End events
    EndEvent,
    EndEventTerminate,
    EndEventError,
    EndEventMessage,
    EndEventSignal,
    EndEventCancel,
    EndEventEscalation,
    EndEventCompensation,
    // Intermediate catch events
    IntermediateCatchMessage,
    IntermediateCatchTimer,
    IntermediateCatchSignal,
    IntermediateCatchLink,
    // Intermediate throw events
    IntermediateThrowMessage,
    IntermediateThrowSignal,
    IntermediateThrowLink,
    IntermediateThrowEscalation,
    IntermediateThrowCompensation,
    // Activities
    ServiceTask,
    UserTask,
    SendTask,
    ReceiveTask,
    ManualTask,
    BusinessRuleTask,
    ScriptTask,
    Subprocess,
    EventSubprocess,
    TransactionSubprocess,
    CallActivity,
}

impl NodeKind {
    /// Parse a node kind string from the `:kind` slot.
    pub(crate) fn from_str(s: &str) -> Option<Self> {
        match s {
            "start-event" => Some(Self::StartEvent),
            "start-event-message" => Some(Self::StartEventMessage),
            "start-event-timer" => Some(Self::StartEventTimer),
            "start-event-signal" => Some(Self::StartEventSignal),
            "start-event-error" => Some(Self::StartEventError),
            "start-event-escalation" => Some(Self::StartEventEscalation),
            "start-event-compensation" => Some(Self::StartEventCompensation),
            "end-event" => Some(Self::EndEvent),
            "end-event-terminate" => Some(Self::EndEventTerminate),
            "end-event-error" => Some(Self::EndEventError),
            "end-event-message" => Some(Self::EndEventMessage),
            "end-event-signal" => Some(Self::EndEventSignal),
            "end-event-cancel" => Some(Self::EndEventCancel),
            "end-event-escalation" => Some(Self::EndEventEscalation),
            "end-event-compensation" => Some(Self::EndEventCompensation),
            "intermediate-catch-message" => Some(Self::IntermediateCatchMessage),
            "intermediate-catch-timer" => Some(Self::IntermediateCatchTimer),
            "intermediate-catch-signal" => Some(Self::IntermediateCatchSignal),
            "intermediate-catch-link" => Some(Self::IntermediateCatchLink),
            "intermediate-throw-message" => Some(Self::IntermediateThrowMessage),
            "intermediate-throw-signal" => Some(Self::IntermediateThrowSignal),
            "intermediate-throw-link" => Some(Self::IntermediateThrowLink),
            "intermediate-throw-escalation" => Some(Self::IntermediateThrowEscalation),
            "intermediate-throw-compensation" => Some(Self::IntermediateThrowCompensation),
            "service-task" => Some(Self::ServiceTask),
            "user-task" => Some(Self::UserTask),
            "send-task" => Some(Self::SendTask),
            "receive-task" => Some(Self::ReceiveTask),
            "manual-task" => Some(Self::ManualTask),
            "business-rule-task" => Some(Self::BusinessRuleTask),
            "script-task" => Some(Self::ScriptTask),
            "subprocess" => Some(Self::Subprocess),
            "event-subprocess" => Some(Self::EventSubprocess),
            "transaction-subprocess" => Some(Self::TransactionSubprocess),
            "call-activity" => Some(Self::CallActivity),
            _ => None,
        }
    }

    /// Returns `true` if this is any start-event variant.
    pub(crate) fn is_start_event(&self) -> bool {
        matches!(
            self,
            Self::StartEvent
                | Self::StartEventMessage
                | Self::StartEventTimer
                | Self::StartEventSignal
                | Self::StartEventError
                | Self::StartEventEscalation
                | Self::StartEventCompensation
        )
    }

    /// Returns `true` if this is any end-event variant.
    pub(crate) fn is_end_event(&self) -> bool {
        matches!(
            self,
            Self::EndEvent
                | Self::EndEventTerminate
                | Self::EndEventError
                | Self::EndEventMessage
                | Self::EndEventSignal
                | Self::EndEventCancel
                | Self::EndEventEscalation
                | Self::EndEventCompensation
        )
    }

    /// Returns `true` if this is an activity (task or subprocess) that can host boundary events.
    pub(crate) fn is_activity(&self) -> bool {
        matches!(
            self,
            Self::ServiceTask
                | Self::UserTask
                | Self::SendTask
                | Self::ReceiveTask
                | Self::ManualTask
                | Self::BusinessRuleTask
                | Self::ScriptTask
                | Self::Subprocess
                | Self::EventSubprocess
                | Self::TransactionSubprocess
                | Self::CallActivity
        )
    }

    /// Serialise to the canonical kind string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::StartEvent => "start-event",
            Self::StartEventMessage => "start-event-message",
            Self::StartEventTimer => "start-event-timer",
            Self::StartEventSignal => "start-event-signal",
            Self::StartEventError => "start-event-error",
            Self::StartEventEscalation => "start-event-escalation",
            Self::StartEventCompensation => "start-event-compensation",
            Self::EndEvent => "end-event",
            Self::EndEventTerminate => "end-event-terminate",
            Self::EndEventError => "end-event-error",
            Self::EndEventMessage => "end-event-message",
            Self::EndEventSignal => "end-event-signal",
            Self::EndEventCancel => "end-event-cancel",
            Self::EndEventEscalation => "end-event-escalation",
            Self::EndEventCompensation => "end-event-compensation",
            Self::IntermediateCatchMessage => "intermediate-catch-message",
            Self::IntermediateCatchTimer => "intermediate-catch-timer",
            Self::IntermediateCatchSignal => "intermediate-catch-signal",
            Self::IntermediateCatchLink => "intermediate-catch-link",
            Self::IntermediateThrowMessage => "intermediate-throw-message",
            Self::IntermediateThrowSignal => "intermediate-throw-signal",
            Self::IntermediateThrowLink => "intermediate-throw-link",
            Self::IntermediateThrowEscalation => "intermediate-throw-escalation",
            Self::IntermediateThrowCompensation => "intermediate-throw-compensation",
            Self::ServiceTask => "service-task",
            Self::UserTask => "user-task",
            Self::SendTask => "send-task",
            Self::ReceiveTask => "receive-task",
            Self::ManualTask => "manual-task",
            Self::BusinessRuleTask => "business-rule-task",
            Self::ScriptTask => "script-task",
            Self::Subprocess => "subprocess",
            Self::EventSubprocess => "event-subprocess",
            Self::TransactionSubprocess => "transaction-subprocess",
            Self::CallActivity => "call-activity",
        }
    }
}

// ---------------------------------------------------------------------------
// Gateway kind
// ---------------------------------------------------------------------------

/// The kind of a process gateway.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum GatewayKind {
    Exclusive,
    Inclusive,
    Parallel,
    EventBased,
    ParallelEventBased,
}

impl GatewayKind {
    /// Parse a gateway kind string from the `:kind` slot.
    pub(crate) fn from_str(s: &str) -> Option<Self> {
        match s {
            "exclusive" => Some(Self::Exclusive),
            "inclusive" => Some(Self::Inclusive),
            "parallel" => Some(Self::Parallel),
            "event-based" => Some(Self::EventBased),
            "parallel-event-based" => Some(Self::ParallelEventBased),
            _ => None,
        }
    }

    /// Serialise to the canonical kind string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Exclusive => "exclusive",
            Self::Inclusive => "inclusive",
            Self::Parallel => "parallel",
            Self::EventBased => "event-based",
            Self::ParallelEventBased => "parallel-event-based",
        }
    }
}

// ---------------------------------------------------------------------------
// Railway node
// ---------------------------------------------------------------------------

/// A typed process node (event or activity).
#[derive(Debug, Clone, Serialize)]
pub struct RailwayNode {
    /// Atom name (unique within the graph).
    pub name: String,
    /// Node kind (event type or activity type).
    pub kind: NodeKind,
    /// Name of the `(invoke ...)` atom bound to this node, if any.
    pub verb_ref: Option<String>,
}

// ---------------------------------------------------------------------------
// Railway gateway
// ---------------------------------------------------------------------------

/// A typed process gateway.
#[derive(Debug, Clone, Serialize)]
pub struct RailwayGateway {
    /// Atom name (unique within the graph).
    pub name: String,
    /// Gateway kind.
    pub kind: GatewayKind,
}

// ---------------------------------------------------------------------------
// Railway edge
// ---------------------------------------------------------------------------

/// A directed sequence flow between two nodes or gateways.
#[derive(Debug, Clone, Serialize)]
pub struct RailwayEdge {
    /// Source node or gateway name.
    pub source: String,
    /// Target node or gateway name.
    pub target: String,
    /// Raw condition expression as a string (not yet evaluated).
    pub condition: Option<String>,
    /// Whether this is the default flow for an exclusive/inclusive gateway.
    pub is_default: bool,
}

// ---------------------------------------------------------------------------
// Boundary attachment
// ---------------------------------------------------------------------------

/// A boundary event attached to a host activity node.
#[derive(Debug, Clone, Serialize)]
pub struct BoundaryAttachmentEntry {
    /// Name of the host activity node.
    pub host_node: String,
    /// Name of the boundary event itself (used as a node name in the graph).
    pub event_name: String,
    /// Event kind: `"error"`, `"timer"`, `"message"`, `"signal"`, `"escalation"`, `"compensation"`, `"cancel"`.
    pub event_kind: String,
    /// Whether this boundary event is interrupting (cancels the host).
    pub interrupting: bool,
}

// ---------------------------------------------------------------------------
// Parallel join
// ---------------------------------------------------------------------------

/// A merge clause on a parallel-join: how conflicting writes at a data location
/// are reconciled.
#[derive(Debug, Clone, Serialize)]
pub struct MergeClause {
    /// Data location name.
    pub location: String,
    /// Merge operator.
    pub operator: MergeOperator,
    /// Custom verb FQN (only for `Custom` operator).
    pub custom_verb: Option<String>,
}

/// The operator used to resolve conflicting writes at a data location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum MergeOperator {
    Max,
    Min,
    Union,
    Concat,
    Sum,
    Latest,
    Earliest,
    Custom,
}

impl MergeOperator {
    pub(crate) fn from_str(s: &str) -> Option<Self> {
        match s {
            "max" => Some(Self::Max),
            "min" => Some(Self::Min),
            "union" => Some(Self::Union),
            "concat" => Some(Self::Concat),
            "sum" => Some(Self::Sum),
            "latest" => Some(Self::Latest),
            "earliest" => Some(Self::Earliest),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }

    /// Serialise to the canonical operator string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Max => "max",
            Self::Min => "min",
            Self::Union => "union",
            Self::Concat => "concat",
            Self::Sum => "sum",
            Self::Latest => "latest",
            Self::Earliest => "earliest",
            Self::Custom => "custom",
        }
    }
}

/// A parallel-join node with explicit merge semantics.
#[derive(Debug, Clone, Serialize)]
pub struct ParallelJoinEntry {
    /// Atom name (unique within the graph).
    pub name: String,
    /// Fork gateway names this join expects tokens from.
    pub expects: Vec<String>,
    /// Merge clauses declared for this join.
    pub merge: Vec<MergeClause>,
}

// ---------------------------------------------------------------------------
// Railway graph
// ---------------------------------------------------------------------------

/// The complete process graph produced by the assembly pass.
///
/// This is the primary output of [`crate::assemble`] and the input to the
/// lowering pass (`dsl-lowering`).
#[derive(Debug, Clone, Serialize)]
pub struct RailwayGraph {
    /// Process nodes keyed by name.
    pub nodes: HashMap<String, RailwayNode>,
    /// Gateway nodes keyed by name.
    pub gateways: HashMap<String, RailwayGateway>,
    /// Parallel-join nodes keyed by name.
    pub parallel_joins: HashMap<String, ParallelJoinEntry>,
    /// All directed edges (sequence flows).
    pub edges: Vec<RailwayEdge>,
    /// Boundary event attachments.
    pub boundary_attachments: Vec<BoundaryAttachmentEntry>,
    /// Name of the start node (if exactly one start event was found).
    pub start_node: Option<String>,
    /// Boundary event names pre-scanned so that flows FROM them are valid.
    /// These are synthetic names not present in nodes/gateways/parallel_joins.
    pub(crate) boundary_event_names: std::collections::HashSet<String>,
}

impl RailwayGraph {
    /// Create an empty graph.
    pub(crate) fn empty() -> Self {
        Self {
            nodes: HashMap::new(),
            gateways: HashMap::new(),
            parallel_joins: HashMap::new(),
            edges: Vec::new(),
            boundary_attachments: Vec::new(),
            start_node: None,
            boundary_event_names: std::collections::HashSet::new(),
        }
    }

    /// Returns `true` if the given name is known as a node, gateway, parallel-join,
    /// or pre-scanned boundary event.
    pub(crate) fn contains(&self, name: &str) -> bool {
        self.nodes.contains_key(name)
            || self.gateways.contains_key(name)
            || self.parallel_joins.contains_key(name)
            || self.boundary_event_names.contains(name)
    }
}
