//! bpmn-lite lowering pass: `RailwayGraph` → `JourneySpec`.
//!
//! Flattens the typed railway graph into the executable form consumed by the
//! bpmn-lite runtime. Declarative atoms are already absent from the graph;
//! the lowering is mechanical.
//!
//! # Format contract
//!
//! `JourneySpec` is the contract between the compiler and the runtime
//! (§5.6.2 of `docs/design/v0.1/session2-compiler-and-runtime.md`).
//! All `Vec` fields are sorted by name for deterministic output.

use dsl_bpmn_frontend::{RailwayGateway, RailwayGraph, RailwayNode};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// JourneySpec and friends
// ---------------------------------------------------------------------------

/// The executable form of a bpmn-lite process.
///
/// Produced by [`lower`] from a [`RailwayGraph`]. This is the contract between
/// the compiler and the runtime (Tranche 6+).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JourneySpec {
    /// Process name (supplied by the caller).
    pub name: String,
    /// Schema version — always `1` for this implementation.
    pub version: u32,
    /// Name of the start node (empty string if none was found).
    pub start_node: String,
    /// All nodes (process nodes + gateways), sorted by name.
    pub nodes: Vec<JourneyNode>,
    /// All directed sequence flows, sorted by (source, target).
    pub edges: Vec<JourneyEdge>,
    /// Boundary event attachments, sorted by (host_node, event_name).
    pub boundary_attachments: Vec<JourneyBoundaryAttachment>,
    /// Parallel join declarations, sorted by name.
    pub parallel_joins: Vec<JourneyParallelJoin>,
}

/// A node in the journey spec (process node or gateway).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JourneyNode {
    /// Node name (unique within the spec).
    pub name: String,
    /// Serialised kind string (e.g. `"service-task"`, `"exclusive"`).
    pub kind: String,
    /// Verb FQN bound to this node, if any.
    pub verb_ref: Option<String>,
}

/// A directed sequence flow in the journey spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JourneyEdge {
    pub source: String,
    pub target: String,
    /// Raw condition expression string, if any.
    pub condition: Option<String>,
    pub is_default: bool,
}

/// A boundary event attachment in the journey spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JourneyBoundaryAttachment {
    pub host_node: String,
    pub event_name: String,
    pub event_kind: String,
    pub interrupting: bool,
}

/// A parallel-join declaration in the journey spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JourneyParallelJoin {
    pub name: String,
    pub expects: Vec<String>,
    pub merge: Vec<JourneyMergeClause>,
}

/// A merge clause in a parallel-join.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JourneyMergeClause {
    pub location: String,
    pub operator: String,
    pub custom_verb: Option<String>,
}

// ---------------------------------------------------------------------------
// Lowering function
// ---------------------------------------------------------------------------

/// Lower a [`RailwayGraph`] into a [`JourneySpec`].
///
/// The lowering is mechanical:
/// 1. Merge nodes and gateways into a flat `Vec<JourneyNode>` keyed by kind string.
/// 2. Convert edges verbatim.
/// 3. Convert boundary attachments verbatim.
/// 4. Convert parallel joins with their merge clauses.
///
/// All output `Vec` fields are sorted for deterministic serialisation.
pub fn lower(graph: &RailwayGraph, process_name: &str) -> JourneySpec {
    // --- Nodes: process nodes ---
    let mut nodes: Vec<JourneyNode> = graph
        .nodes
        .values()
        .map(|n: &RailwayNode| JourneyNode {
            name: n.name.clone(),
            kind: n.kind.as_str().to_owned(),
            verb_ref: n.verb_ref.clone(),
        })
        .collect();

    // --- Nodes: gateways (merged into the same flat list) ---
    for gw in graph
        .gateways
        .values()
        .map(|g: &RailwayGateway| JourneyNode {
            name: g.name.clone(),
            kind: g.kind.as_str().to_owned(),
            verb_ref: None,
        })
    {
        nodes.push(gw);
    }

    // --- Nodes: parallel joins (also part of the node table) ---
    for pj in graph.parallel_joins.values().map(|pj| JourneyNode {
        name: pj.name.clone(),
        kind: "parallel-join".to_owned(),
        verb_ref: None,
    }) {
        nodes.push(pj);
    }

    // Sort nodes by name for determinism
    nodes.sort_by(|a, b| a.name.cmp(&b.name));

    // --- Edges ---
    let mut edges: Vec<JourneyEdge> = graph
        .edges
        .iter()
        .map(|e| JourneyEdge {
            source: e.source.clone(),
            target: e.target.clone(),
            condition: e.condition.clone(),
            is_default: e.is_default,
        })
        .collect();
    edges.sort_by(|a, b| a.source.cmp(&b.source).then(a.target.cmp(&b.target)));

    // --- Boundary attachments ---
    let mut boundary_attachments: Vec<JourneyBoundaryAttachment> = graph
        .boundary_attachments
        .iter()
        .map(|ba| JourneyBoundaryAttachment {
            host_node: ba.host_node.clone(),
            event_name: ba.event_name.clone(),
            event_kind: ba.event_kind.clone(),
            interrupting: ba.interrupting,
        })
        .collect();
    boundary_attachments.sort_by(|a, b| {
        a.host_node
            .cmp(&b.host_node)
            .then(a.event_name.cmp(&b.event_name))
    });

    // --- Parallel joins ---
    let mut parallel_joins: Vec<JourneyParallelJoin> = graph
        .parallel_joins
        .values()
        .map(|pj| JourneyParallelJoin {
            name: pj.name.clone(),
            expects: {
                let mut e = pj.expects.clone();
                e.sort();
                e
            },
            merge: pj
                .merge
                .iter()
                .map(|mc| JourneyMergeClause {
                    location: mc.location.clone(),
                    operator: mc.operator.as_str().to_owned(),
                    custom_verb: mc.custom_verb.clone(),
                })
                .collect(),
        })
        .collect();
    parallel_joins.sort_by(|a, b| a.name.cmp(&b.name));

    JourneySpec {
        name: process_name.to_owned(),
        version: 1,
        start_node: graph.start_node.clone().unwrap_or_default(),
        nodes,
        edges,
        boundary_attachments,
        parallel_joins,
    }
}
