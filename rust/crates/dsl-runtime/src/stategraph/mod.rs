//! StateGraph engine for graph-backed Step 2 entity state derivation.
//!
//! The StateGraph engine loads authored graph YAML, validates it against the
//! live verb registry, and walks a grounded entity's current state to produce
//! frontier actions.

use anyhow::{anyhow, Context, Result};
use dsl_core::config::loader::ConfigLoader;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Authored StateGraph definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StateGraph {
    pub graph_id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub entity_types: Vec<String>,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    #[serde(default)]
    pub gates: Vec<GraphGate>,
}

/// A graph node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphNode {
    pub node_id: String,
    pub name: String,
    pub node_type: NodeType,
    pub lane: String,
    #[serde(default)]
    pub satisfied_when: Vec<SignalCondition>,
}

/// Node type in the graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Entry,
    Milestone,
    Gate,
    Terminal,
}

/// A directed graph edge.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphEdge {
    pub edge_id: String,
    pub from_node: String,
    pub to_node: String,
    pub verb_ids: Vec<String>,
    pub edge_type: EdgeType,
    #[serde(default)]
    pub condition: Vec<SignalCondition>,
}

/// Edge behavior.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    Advance,
    Revert,
    Conditional,
}

/// Gate dependency definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphGate {
    pub gate_node: String,
    pub required_nodes: Vec<String>,
}

/// Signal condition used for node or edge evaluation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignalCondition {
    pub signal: String,
    pub description: String,
}

/// Result of walking a graph for a grounded entity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphWalkResult {
    pub entity_id: String,
    pub entity_name: String,
    pub graph_id: String,
    pub satisfied_nodes: Vec<String>,
    pub frontier_nodes: Vec<String>,
    pub valid_verbs: Vec<ValidVerb>,
    pub blocked_verbs: Vec<BlockedVerb>,
    pub gate_status: Vec<GateStatus>,
}

/// Valid action exposed from the graph frontier.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidVerb {
    pub verb_id: String,
    pub edge_id: String,
    pub from_node: String,
    pub to_node: String,
    pub description: String,
    pub polarity: String,
    pub invocation_phrases: Vec<String>,
    pub parameters: Vec<Value>,
    pub lane: String,
    pub is_frontier: bool,
    pub is_revert: bool,
    pub relevance: f32,
}

/// Blocked action with unmet conditions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlockedVerb {
    pub verb_id: String,
    pub edge_id: String,
    pub from_node: String,
    pub to_node: String,
    pub description: String,
    pub unmet_conditions: Vec<String>,
    pub unblocking_verbs: Vec<String>,
}

/// Gate satisfaction status for a graph walk.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GateStatus {
    pub gate_node: String,
    pub gate_name: String,
    pub required_nodes: Vec<GateRequirement>,
    pub all_met: bool,
}

/// Individual gate requirement state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GateRequirement {
    pub node_id: String,
    pub node_name: String,
    pub satisfied: bool,
    pub unmet_signals: Vec<String>,
}

/// Load StateGraph files from disk.
///
/// # Examples
///
/// ```ignore
/// use ob_poc::stategraph::load_state_graphs;
///
/// let graphs = load_state_graphs().unwrap();
/// assert!(!graphs.is_empty());
/// ```
pub fn load_state_graphs() -> Result<Vec<StateGraph>> {
    let graph_dir = discover_stategraph_dir()?;
    let mut graphs = Vec::new();
    for entry in fs::read_dir(&graph_dir)
        .with_context(|| format!("Failed to read {}", graph_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("yaml") {
            continue;
        }
        let yaml = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let graph: StateGraph = serde_yaml::from_str(&yaml)
            .with_context(|| format!("Failed to parse {}", path.display()))?;
        graphs.push(graph);
    }
    graphs.sort_by(|left, right| left.graph_id.cmp(&right.graph_id));
    Ok(graphs)
}

/// Validate StateGraph files against internal consistency and the live registry.
///
/// # Examples
///
/// ```ignore
/// use ob_poc::stategraph::{load_state_graphs, validate_graphs};
///
/// let graphs = load_state_graphs().unwrap();
/// validate_graphs(&graphs).unwrap();
/// ```
pub fn validate_graphs(graphs: &[StateGraph]) -> Result<()> {
    let verbs = ConfigLoader::from_env().load_verbs()?;
    for graph in graphs {
        let mut node_ids = BTreeSet::new();
        for node in &graph.nodes {
            if !node_ids.insert(node.node_id.clone()) {
                return Err(anyhow!(
                    "duplicate node '{}' in graph '{}'",
                    node.node_id,
                    graph.graph_id
                ));
            }
        }
        for edge in &graph.edges {
            if !node_ids.contains(&edge.from_node) || !node_ids.contains(&edge.to_node) {
                return Err(anyhow!(
                    "edge '{}' in graph '{}' references unknown nodes",
                    edge.edge_id,
                    graph.graph_id
                ));
            }
            for verb_id in &edge.verb_ids {
                let (domain, verb) = verb_id.split_once('.').ok_or_else(|| {
                    anyhow!(
                        "invalid verb id '{}' in graph '{}'",
                        verb_id,
                        graph.graph_id
                    )
                })?;
                if verbs
                    .domains
                    .get(domain)
                    .and_then(|cfg| cfg.verbs.get(verb))
                    .is_none()
                {
                    return Err(anyhow!(
                        "graph '{}' references missing verb '{}'",
                        graph.graph_id,
                        verb_id
                    ));
                }
            }
        }
        for gate in &graph.gates {
            if !node_ids.contains(&gate.gate_node) {
                return Err(anyhow!(
                    "graph '{}' gate '{}' references missing node",
                    graph.graph_id,
                    gate.gate_node
                ));
            }
            for required in &gate.required_nodes {
                if !node_ids.contains(required) {
                    return Err(anyhow!(
                        "graph '{}' gate '{}' requires missing node '{}'",
                        graph.graph_id,
                        gate.gate_node,
                        required
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Evaluate a signal condition against a live entity-context payload.
///
/// # Examples
///
/// ```ignore
/// use ob_poc::stategraph::evaluate_signal;
/// use serde_json::json;
///
/// assert!(evaluate_signal("has_active_deal", &json!({"signals":{"has_active_deal":true}})));
/// ```
pub fn evaluate_signal(signal: &str, entity_context: &Value) -> bool {
    let signal_value = &entity_context["signals"][signal];
    signal_value
        .as_bool()
        .unwrap_or_else(|| signal_value.as_i64().unwrap_or(0) > 0)
}

/// Walk a validated graph for a grounded entity context.
///
/// # Examples
///
/// ```ignore
/// use ob_poc::stategraph::{GraphWalkResult, StateGraph, walk_graph};
/// use serde_json::json;
/// use uuid::Uuid;
///
/// # let graph = StateGraph {
/// #     graph_id: "example".to_string(),
/// #     name: "Example".to_string(),
/// #     version: "1".to_string(),
/// #     description: "example".to_string(),
/// #     entity_types: vec!["client-group".to_string()],
/// #     nodes: vec![],
/// #     edges: vec![],
/// #     gates: vec![],
/// # };
/// let context = json!({"entity_id": Uuid::nil(), "name": "Example", "signals": {}});
/// let result = walk_graph(&graph, &context).unwrap();
/// assert_eq!(result.graph_id, "example");
/// ```
pub fn walk_graph(graph: &StateGraph, entity_context: &Value) -> Result<GraphWalkResult> {
    let node_map = graph
        .nodes
        .iter()
        .map(|node| (node.node_id.clone(), node))
        .collect::<BTreeMap<_, _>>();

    let mut satisfied_nodes = BTreeSet::new();
    for node in &graph.nodes {
        if is_node_satisfied(node, entity_context) {
            satisfied_nodes.insert(node.node_id.clone());
        }
    }

    let mut frontier_nodes = BTreeSet::new();
    let mut valid_verbs = Vec::new();
    let mut blocked_verbs = Vec::new();
    let verbs = ConfigLoader::from_env().load_verbs()?;

    for edge in &graph.edges {
        let from_satisfied = satisfied_nodes.contains(&edge.from_node);
        let to_satisfied = satisfied_nodes.contains(&edge.to_node);
        if !from_satisfied || to_satisfied {
            continue;
        }

        frontier_nodes.insert(edge.to_node.clone());
        let unmet = edge
            .condition
            .iter()
            .filter(|condition| !evaluate_signal(&condition.signal, entity_context))
            .map(|condition| condition.signal.clone())
            .collect::<Vec<_>>();

        let to_node = node_map
            .get(&edge.to_node)
            .ok_or_else(|| anyhow!("graph '{}' missing node '{}'", graph.graph_id, edge.to_node))?;

        if unmet.is_empty() {
            for verb_id in &edge.verb_ids {
                let (domain, verb_name) = verb_id
                    .split_once('.')
                    .ok_or_else(|| anyhow!("invalid verb id '{}'", verb_id))?;
                let verb = verbs
                    .domains
                    .get(domain)
                    .and_then(|domain_cfg| domain_cfg.verbs.get(verb_name))
                    .ok_or_else(|| anyhow!("missing verb '{}'", verb_id))?;
                valid_verbs.push(ValidVerb {
                    verb_id: verb_id.clone(),
                    edge_id: edge.edge_id.clone(),
                    from_node: edge.from_node.clone(),
                    to_node: edge.to_node.clone(),
                    description: verb.description.clone(),
                    polarity: verb
                        .metadata
                        .as_ref()
                        .and_then(|metadata| metadata.side_effects.as_deref())
                        .map(|side_effects| {
                            if side_effects == "facts_only" {
                                "read"
                            } else {
                                "write"
                            }
                        })
                        .unwrap_or("write")
                        .to_string(),
                    invocation_phrases: verb.invocation_phrases.clone(),
                    parameters: verb
                        .args
                        .iter()
                        .map(|arg| {
                            serde_json::json!({
                                "name": arg.name,
                                "required": arg.required,
                                "description": arg.description,
                            })
                        })
                        .collect(),
                    lane: to_node.lane.clone(),
                    is_frontier: true,
                    is_revert: matches!(edge.edge_type, EdgeType::Revert),
                    relevance: compute_relevance(edge, &satisfied_nodes),
                });
            }
        } else {
            for verb_id in &edge.verb_ids {
                let (domain, verb_name) = verb_id
                    .split_once('.')
                    .ok_or_else(|| anyhow!("invalid verb id '{}'", verb_id))?;
                let verb = verbs
                    .domains
                    .get(domain)
                    .and_then(|domain_cfg| domain_cfg.verbs.get(verb_name))
                    .ok_or_else(|| anyhow!("missing verb '{}'", verb_id))?;
                blocked_verbs.push(BlockedVerb {
                    verb_id: verb_id.clone(),
                    edge_id: edge.edge_id.clone(),
                    from_node: edge.from_node.clone(),
                    to_node: edge.to_node.clone(),
                    description: verb.description.clone(),
                    unmet_conditions: unmet.clone(),
                    unblocking_verbs: Vec::new(),
                });
            }
        }
    }

    let gate_status = graph
        .gates
        .iter()
        .map(|gate| {
            let requirements = gate
                .required_nodes
                .iter()
                .map(|required| {
                    let node = node_map.get(required).expect("validated node");
                    GateRequirement {
                        node_id: node.node_id.clone(),
                        node_name: node.name.clone(),
                        satisfied: satisfied_nodes.contains(required),
                        unmet_signals: node
                            .satisfied_when
                            .iter()
                            .filter(|condition| !evaluate_signal(&condition.signal, entity_context))
                            .map(|condition| condition.signal.clone())
                            .collect(),
                    }
                })
                .collect::<Vec<_>>();
            let gate_node = node_map.get(&gate.gate_node).expect("validated gate node");
            GateStatus {
                gate_node: gate.gate_node.clone(),
                gate_name: gate_node.name.clone(),
                all_met: requirements.iter().all(|requirement| requirement.satisfied),
                required_nodes: requirements,
            }
        })
        .collect::<Vec<_>>();

    Ok(GraphWalkResult {
        entity_id: entity_context["entity_id"]
            .to_string()
            .trim_matches('"')
            .to_string(),
        entity_name: entity_context["name"]
            .as_str()
            .unwrap_or("unknown")
            .to_string(),
        graph_id: graph.graph_id.clone(),
        satisfied_nodes: satisfied_nodes.into_iter().collect(),
        frontier_nodes: frontier_nodes.into_iter().collect(),
        valid_verbs,
        blocked_verbs,
        gate_status,
    })
}

fn discover_stategraph_dir() -> Result<PathBuf> {
    // Try CWD-relative paths first (server runtime path).
    let candidates = [
        Path::new("config/stategraphs"),
        Path::new("rust/config/stategraphs"),
        // Phase 5a composite-blocker #23 — relative paths walking up
        // from `rust/crates/<crate>/` (cargo test CWD when running
        // from a workspace member crate).
        Path::new("../../config/stategraphs"),
        Path::new("../../../config/stategraphs"),
    ];
    for candidate in candidates {
        if candidate.exists() {
            return Ok(candidate.to_path_buf());
        }
    }
    Err(anyhow!("Could not locate config/stategraphs"))
}

fn is_node_satisfied(node: &GraphNode, entity_context: &Value) -> bool {
    match node.node_type {
        NodeType::Entry => true,
        _ => {
            if node.satisfied_when.is_empty() {
                false
            } else {
                node.satisfied_when
                    .iter()
                    .all(|condition| evaluate_signal(&condition.signal, entity_context))
            }
        }
    }
}

fn compute_relevance(edge: &GraphEdge, satisfied_nodes: &BTreeSet<String>) -> f32 {
    let mut score = 0.5f32;
    if satisfied_nodes.contains(&edge.from_node) {
        score += 0.25;
    }
    if matches!(edge.edge_type, EdgeType::Advance) {
        score += 0.15;
    }
    if matches!(edge.edge_type, EdgeType::Revert) {
        score -= 0.1;
    }
    score.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::{evaluate_signal, load_state_graphs, validate_graphs};
    use serde_json::json;

    #[test]
    fn stategraphs_load_from_disk() {
        let graphs = load_state_graphs().expect("graphs should load");
        assert!(!graphs.is_empty());
    }

    #[test]
    fn stategraphs_validate_against_registry() {
        let graphs = load_state_graphs().expect("graphs should load");
        validate_graphs(&graphs).expect("graphs should validate");
    }

    #[test]
    fn evaluate_signal_accepts_numeric_truthy() {
        let context = json!({"signals":{"pending_document_count": 2}});
        assert!(evaluate_signal("pending_document_count", &context));
    }
}
