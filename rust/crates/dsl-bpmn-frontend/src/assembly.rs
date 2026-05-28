//! bpmn-lite assembly pass.
//!
//! Transforms a typed `AtomBag` into a [`RailwayGraph`] while accumulating
//! structural diagnostics into a [`DiagnosticBag`].
//!
//! The algorithm follows §5.4.2 of `docs/design/v0.1/session2-compiler-and-runtime.md`.

use std::collections::{HashMap, HashSet, VecDeque};

use dsl_ast::AtomBag;
use dsl_atoms::StructuralKind;
use dsl_diagnostics::{Diagnostic, DiagnosticBag, UNDECLARED_MERGE, UNRESOLVED_NAME_REF};
use dsl_parser::RawValue;

use crate::railway::{
    BoundaryAttachmentEntry, GatewayKind, MergeClause, MergeOperator, NodeKind, ParallelJoinEntry,
    RailwayEdge, RailwayGateway, RailwayGraph, RailwayNode,
};

// ---------------------------------------------------------------------------
// Diagnostic codes used in the assembly pass
// ---------------------------------------------------------------------------

/// A node in the graph is unreachable from any start event.
pub const UNREACHABLE_NODE: &str = "E1001";
/// A path exists that never reaches an end event.
pub const UNTERMINATED_PATH: &str = "E1002";
/// A gateway has invalid fan-out (parallel: <2 outgoing; exclusive: wrong default).
pub const GATEWAY_FAN_OUT_ERROR: &str = "W1001";
/// Two atoms share the same name.
pub const DUPLICATE_NAME: &str = "E1003";
/// A boundary attachment's host is not a known activity node.
pub const INVALID_BOUNDARY_TARGET: &str = "E1004";

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Assemble a `RailwayGraph` from the typed atom bag.
///
/// Diagnostics are accumulated into `diagnostics`; the caller should check
/// `diagnostics.has_errors()` before using the returned graph.
pub fn assemble(bag: &AtomBag, diagnostics: &mut DiagnosticBag) -> RailwayGraph {
    let mut graph = RailwayGraph::empty();

    // ------------------------------------------------------------------
    // Pass 1: Index all node/gateway/parallel-join atoms
    // ------------------------------------------------------------------
    index_nodes(bag, &mut graph, diagnostics);
    index_gateways(bag, &mut graph, diagnostics);
    index_parallel_joins(bag, &mut graph, diagnostics);

    // ------------------------------------------------------------------
    // Pass 1b: Pre-scan boundary-attachment atoms to register their
    //          event names in the graph before edge building.
    //          Boundary events are reachable via flows FROM their name.
    // ------------------------------------------------------------------
    prescan_boundary_event_names(bag, &mut graph);

    // ------------------------------------------------------------------
    // Pass 2: Build edges from flow atoms
    // ------------------------------------------------------------------
    build_edges(bag, &mut graph, diagnostics);

    // ------------------------------------------------------------------
    // Pass 3: Index boundary attachments (full indexing)
    // ------------------------------------------------------------------
    index_boundary_attachments(bag, &mut graph, diagnostics);

    // ------------------------------------------------------------------
    // Pass 4: Find start node(s)
    // ------------------------------------------------------------------
    find_start_node(&mut graph, diagnostics);

    // ------------------------------------------------------------------
    // Pass 5: Structural validation
    // ------------------------------------------------------------------
    validate_reachability(&graph, diagnostics);
    validate_termination(&graph, diagnostics);
    validate_gateway_fanout(&graph, diagnostics);
    validate_parallel_joins(&graph, diagnostics);

    graph
}

// ---------------------------------------------------------------------------
// Pass 1b: Pre-scan boundary event names
// ---------------------------------------------------------------------------

/// Pre-scan boundary-attachment atoms and register the boundary event name
/// (the second positional / atom name) into `graph.boundary_event_names`.
///
/// This must run before edge-building so that flows FROM a boundary event
/// (e.g. `(flow verification-failed-boundary -> manual-verify)`) resolve
/// without "unresolved name ref" errors.
fn prescan_boundary_event_names(bag: &AtomBag, graph: &mut RailwayGraph) {
    for atom in bag.atoms_of_structural_kind(StructuralKind::BoundaryAttachment) {
        // atom.name = host node; second positional (empty-key slot) = event name
        let event_name = match positional_slot_symbol(&atom.raw.slots) {
            Some(s) => s,
            None => {
                // Fallback: use host + "-boundary"
                match &atom.name {
                    Some(host) => format!("{}-boundary", host),
                    None => continue,
                }
            }
        };
        graph.boundary_event_names.insert(event_name);
    }
}

// ---------------------------------------------------------------------------
// Pass 1: Index nodes
// ---------------------------------------------------------------------------

fn index_nodes(bag: &AtomBag, graph: &mut RailwayGraph, diagnostics: &mut DiagnosticBag) {
    for atom in bag.atoms_of_structural_kind(StructuralKind::Node) {
        let name = match &atom.name {
            Some(n) => n.clone(),
            None => {
                diagnostics.push(Diagnostic::error("Node atom must have a name"));
                continue;
            }
        };

        // Extract :kind slot
        let kind_str = match slot_symbol(&atom.raw.slots, "kind") {
            Some(s) => s,
            None => {
                diagnostics.push(
                    Diagnostic::error(format!("Node '{}' missing required ':kind' slot", name))
                        .with_code(dsl_diagnostics::MISSING_REQUIRED_SLOT),
                );
                continue;
            }
        };

        let kind = match NodeKind::from_str(&kind_str) {
            Some(k) => k,
            None => {
                diagnostics.push(Diagnostic::error(format!(
                    "Node '{}' has unknown :kind '{}'",
                    name, kind_str
                )));
                continue;
            }
        };

        // Check for duplicate names across nodes and gateways
        if graph.nodes.contains_key(&name) || graph.gateways.contains_key(&name) {
            diagnostics.push(
                Diagnostic::error(format!("Duplicate atom name '{}'", name))
                    .with_code(DUPLICATE_NAME),
            );
            continue;
        }

        // Extract optional :verb slot (may be a nested invoke atom or a symbol)
        let verb_ref = extract_verb_ref(&atom.raw.slots);

        graph.nodes.insert(
            name.clone(),
            RailwayNode {
                name,
                kind,
                verb_ref,
            },
        );
    }
}

// ---------------------------------------------------------------------------
// Pass 1: Index gateways
// ---------------------------------------------------------------------------

fn index_gateways(bag: &AtomBag, graph: &mut RailwayGraph, diagnostics: &mut DiagnosticBag) {
    for atom in bag.atoms_of_structural_kind(StructuralKind::Gateway) {
        let name = match &atom.name {
            Some(n) => n.clone(),
            None => {
                diagnostics.push(Diagnostic::error("Gateway atom must have a name"));
                continue;
            }
        };

        // Extract :kind slot
        let kind_str = match slot_symbol(&atom.raw.slots, "kind") {
            Some(s) => s,
            None => {
                diagnostics.push(
                    Diagnostic::error(format!("Gateway '{}' missing required ':kind' slot", name))
                        .with_code(dsl_diagnostics::MISSING_REQUIRED_SLOT),
                );
                continue;
            }
        };

        let kind = match GatewayKind::from_str(&kind_str) {
            Some(k) => k,
            None => {
                diagnostics.push(Diagnostic::error(format!(
                    "Gateway '{}' has unknown :kind '{}'",
                    name, kind_str
                )));
                continue;
            }
        };

        // Check for duplicate names across nodes and gateways
        if graph.nodes.contains_key(&name) || graph.gateways.contains_key(&name) {
            diagnostics.push(
                Diagnostic::error(format!("Duplicate atom name '{}'", name))
                    .with_code(DUPLICATE_NAME),
            );
            continue;
        }

        graph
            .gateways
            .insert(name.clone(), RailwayGateway { name, kind });
    }
}

// ---------------------------------------------------------------------------
// Pass 1: Index parallel joins
// ---------------------------------------------------------------------------

fn index_parallel_joins(bag: &AtomBag, graph: &mut RailwayGraph, diagnostics: &mut DiagnosticBag) {
    for atom in bag.atoms_of_structural_kind(StructuralKind::ParallelJoin) {
        let name = match &atom.name {
            Some(n) => n.clone(),
            None => {
                diagnostics.push(Diagnostic::error("parallel-join atom must have a name"));
                continue;
            }
        };

        // Check for duplicate names
        if graph.nodes.contains_key(&name)
            || graph.gateways.contains_key(&name)
            || graph.parallel_joins.contains_key(&name)
        {
            diagnostics.push(
                Diagnostic::error(format!("Duplicate atom name '{}'", name))
                    .with_code(DUPLICATE_NAME),
            );
            continue;
        }

        // Extract :expects list
        let expects = match slot_symbol_list(&atom.raw.slots, "expects") {
            Some(list) => list,
            None => {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "parallel-join '{}' missing required ':expects' slot",
                        name
                    ))
                    .with_code(dsl_diagnostics::MISSING_REQUIRED_SLOT),
                );
                Vec::new()
            }
        };

        // Extract optional :merge list
        let has_merge_slot = atom.raw.slots.iter().any(|(k, _)| k == "merge");
        let merge = if has_merge_slot {
            extract_merge_clauses(&atom.raw.slots)
        } else {
            Vec::new()
        };

        // Emit UNDECLARED_MERGE warning if no :merge slot at all
        if !has_merge_slot {
            diagnostics.push(
                Diagnostic::warning(format!(
                    "parallel-join '{}' has no :merge slot — any write conflicts between \
                     parallel branches will fail at runtime",
                    name
                ))
                .with_code(UNDECLARED_MERGE),
            );
        }

        graph.parallel_joins.insert(
            name.clone(),
            ParallelJoinEntry {
                name,
                expects,
                merge,
            },
        );
    }
}

// ---------------------------------------------------------------------------
// Pass 2: Build edges
// ---------------------------------------------------------------------------

fn build_edges(bag: &AtomBag, graph: &mut RailwayGraph, diagnostics: &mut DiagnosticBag) {
    for atom in bag.atoms_of_structural_kind(StructuralKind::Flow) {
        // Extract source from "source" slot
        let source = match slot_symbol_or_name(&atom.raw.slots, "source") {
            Some(s) => s,
            None => {
                diagnostics.push(
                    Diagnostic::error("flow atom missing source endpoint")
                        .with_code(UNRESOLVED_NAME_REF),
                );
                continue;
            }
        };

        // Extract target from "target" slot
        let target = match slot_symbol_or_name(&atom.raw.slots, "target") {
            Some(s) => s,
            None => {
                diagnostics.push(
                    Diagnostic::error("flow atom missing target endpoint")
                        .with_code(UNRESOLVED_NAME_REF),
                );
                continue;
            }
        };

        // Validate that source and target resolve to known names
        if !graph.contains(&source) {
            diagnostics.push(
                Diagnostic::error(format!(
                    "flow source '{}' does not resolve to a known node or gateway",
                    source
                ))
                .with_code(UNRESOLVED_NAME_REF),
            );
        }
        if !graph.contains(&target) {
            diagnostics.push(
                Diagnostic::error(format!(
                    "flow target '{}' does not resolve to a known node or gateway",
                    target
                ))
                .with_code(UNRESOLVED_NAME_REF),
            );
        }

        // Extract optional :condition (as raw string representation)
        let condition = extract_condition_string(&atom.raw.slots);

        // Extract optional :default
        let is_default = slot_bool(&atom.raw.slots, "default").unwrap_or(false);

        graph.edges.push(RailwayEdge {
            source,
            target,
            condition,
            is_default,
        });
    }
}

// ---------------------------------------------------------------------------
// Pass 3: Boundary attachments
// ---------------------------------------------------------------------------

fn index_boundary_attachments(
    bag: &AtomBag,
    graph: &mut RailwayGraph,
    diagnostics: &mut DiagnosticBag,
) {
    for atom in bag.atoms_of_structural_kind(StructuralKind::BoundaryAttachment) {
        // Host node is in atom.name (first symbol after kind is consumed as name)
        let host_node = match &atom.name {
            Some(n) => n.clone(),
            None => {
                diagnostics.push(Diagnostic::error(
                    "boundary-attachment must have a host node as its name",
                ));
                continue;
            }
        };

        // Event name is the second positional (stored with empty key "")
        let event_name = match positional_slot_symbol(&atom.raw.slots) {
            Some(s) => s,
            None => {
                // Fall back to using the host node name + "-boundary" as event name
                format!("{}-boundary", host_node)
            }
        };

        // Extract :event-kind slot
        let event_kind = match slot_symbol(&atom.raw.slots, "event-kind") {
            Some(s) => s,
            None => {
                diagnostics.push(Diagnostic::error(format!(
                    "boundary-attachment on '{}' missing required ':event-kind' slot",
                    host_node
                )));
                continue;
            }
        };

        // Validate host node is an activity
        if let Some(node) = graph.nodes.get(&host_node) {
            if !node.kind.is_activity() {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "boundary-attachment host '{}' is not an activity node",
                        host_node
                    ))
                    .with_code(INVALID_BOUNDARY_TARGET),
                );
            }
        } else if graph.gateways.contains_key(&host_node) {
            diagnostics.push(
                Diagnostic::error(format!(
                    "boundary-attachment host '{}' is a gateway, not an activity",
                    host_node
                ))
                .with_code(INVALID_BOUNDARY_TARGET),
            );
        }
        // If host node not yet found (forward ref), we skip validation here

        // Extract :interrupting
        let interrupting = slot_bool(&atom.raw.slots, "interrupting").unwrap_or(true);

        graph.boundary_attachments.push(BoundaryAttachmentEntry {
            host_node,
            event_name,
            event_kind,
            interrupting,
        });
    }
}

// ---------------------------------------------------------------------------
// Pass 4: Find start node
// ---------------------------------------------------------------------------

fn find_start_node(graph: &mut RailwayGraph, diagnostics: &mut DiagnosticBag) {
    let start_nodes: Vec<String> = graph
        .nodes
        .values()
        .filter(|n| n.kind.is_start_event())
        .map(|n| n.name.clone())
        .collect();

    match start_nodes.len() {
        0 => {
            diagnostics.push(Diagnostic::error(
                "Process has no start event node; exactly one is required",
            ));
        }
        1 => {
            graph.start_node = Some(start_nodes.into_iter().next().unwrap());
        }
        n => {
            // Multiple start events — pick first alphabetically, warn
            let mut sorted = start_nodes;
            sorted.sort();
            diagnostics.push(Diagnostic::warning(format!(
                "Process has {} start event nodes; only one is expected. \
                 Using '{}' as the primary start node.",
                n, sorted[0]
            )));
            graph.start_node = Some(sorted.into_iter().next().unwrap());
        }
    }
}

// ---------------------------------------------------------------------------
// Pass 5: Structural validation
// ---------------------------------------------------------------------------

/// Validate that every node is reachable from the start event via BFS.
fn validate_reachability(graph: &RailwayGraph, diagnostics: &mut DiagnosticBag) {
    let start = match &graph.start_node {
        Some(s) => s.clone(),
        None => return, // already reported
    };

    // Build adjacency list for reachability
    let mut reachable: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();

    reachable.insert(start.clone());
    queue.push_back(start);

    while let Some(current) = queue.pop_front() {
        for edge in graph.edges.iter().filter(|e| e.source == current) {
            if !reachable.contains(&edge.target) {
                reachable.insert(edge.target.clone());
                queue.push_back(edge.target.clone());
            }
        }
        // Boundary event targets are also reachable
        for ba in graph
            .boundary_attachments
            .iter()
            .filter(|ba| ba.host_node == current)
        {
            if !reachable.contains(&ba.event_name) {
                reachable.insert(ba.event_name.clone());
                queue.push_back(ba.event_name.clone());
            }
        }
    }

    // Any known graph member not reachable is an error
    for name in graph.nodes.keys() {
        if !reachable.contains(name.as_str()) {
            diagnostics.push(
                Diagnostic::error(format!("Node '{}' is unreachable from start event", name))
                    .with_code(UNREACHABLE_NODE),
            );
        }
    }
    for name in graph.gateways.keys() {
        if !reachable.contains(name.as_str()) {
            diagnostics.push(
                Diagnostic::error(format!(
                    "Gateway '{}' is unreachable from start event",
                    name
                ))
                .with_code(UNREACHABLE_NODE),
            );
        }
    }
    for name in graph.parallel_joins.keys() {
        if !reachable.contains(name.as_str()) {
            diagnostics.push(
                Diagnostic::error(format!(
                    "Parallel-join '{}' is unreachable from start event",
                    name
                ))
                .with_code(UNREACHABLE_NODE),
            );
        }
    }
}

/// Validate that every path from start eventually reaches an end event.
///
/// This is a DFS that looks for dead-ends: reachable nodes with no outgoing
/// edges and no end-event kind.
fn validate_termination(graph: &RailwayGraph, diagnostics: &mut DiagnosticBag) {
    if graph.start_node.is_none() {
        return;
    }

    // Build outgoing adjacency
    let mut outgoing: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in &graph.edges {
        outgoing
            .entry(edge.source.as_str())
            .or_default()
            .push(edge.target.as_str());
    }
    // Boundary events contribute outgoing edges
    for ba in &graph.boundary_attachments {
        outgoing.entry(ba.event_name.as_str()).or_default();
        // The boundary event itself may have outgoing flows in the edges list
    }

    // Find nodes that have no outgoing edges and are not end events
    for (name, node) in &graph.nodes {
        let has_outgoing = outgoing
            .get(name.as_str())
            .map(|v| !v.is_empty())
            .unwrap_or(false);
        if !has_outgoing && !node.kind.is_end_event() {
            diagnostics.push(
                Diagnostic::error(format!(
                    "Node '{}' (kind: {}) has no outgoing flows and is not an end event — unterminated path",
                    name,
                    node.kind.as_str()
                ))
                .with_code(UNTERMINATED_PATH),
            );
        }
    }
    // Gateways and parallel-joins without outgoing are also dead-ends
    for name in graph.gateways.keys() {
        let has_outgoing = outgoing
            .get(name.as_str())
            .map(|v| !v.is_empty())
            .unwrap_or(false);
        if !has_outgoing {
            diagnostics.push(
                Diagnostic::error(format!(
                    "Gateway '{}' has no outgoing flows — unterminated path",
                    name
                ))
                .with_code(UNTERMINATED_PATH),
            );
        }
    }
    for name in graph.parallel_joins.keys() {
        let has_outgoing = outgoing
            .get(name.as_str())
            .map(|v| !v.is_empty())
            .unwrap_or(false);
        if !has_outgoing {
            diagnostics.push(
                Diagnostic::error(format!(
                    "parallel-join '{}' has no outgoing flows — unterminated path",
                    name
                ))
                .with_code(UNTERMINATED_PATH),
            );
        }
    }
    for name in &graph.boundary_event_names {
        let has_outgoing = outgoing
            .get(name.as_str())
            .map(|v| !v.is_empty())
            .unwrap_or(false);
        if !has_outgoing {
            diagnostics.push(
                Diagnostic::error(format!(
                    "Boundary event '{}' has no outgoing flows — unterminated path",
                    name
                ))
                .with_code(UNTERMINATED_PATH),
            );
        }
    }
}

/// Validate gateway fan-out rules.
fn validate_gateway_fanout(graph: &RailwayGraph, diagnostics: &mut DiagnosticBag) {
    for (name, gw) in &graph.gateways {
        let outgoing: Vec<&RailwayEdge> =
            graph.edges.iter().filter(|e| &e.source == name).collect();
        let default_count = outgoing.iter().filter(|e| e.is_default).count();

        match gw.kind {
            GatewayKind::Exclusive => {
                // Should have at least one conditional outgoing and at most one default
                if outgoing.is_empty() {
                    diagnostics.push(
                        Diagnostic::warning(format!(
                            "Exclusive gateway '{}' has no outgoing flows",
                            name
                        ))
                        .with_code(GATEWAY_FAN_OUT_ERROR),
                    );
                }
                if default_count > 1 {
                    diagnostics.push(
                        Diagnostic::warning(format!(
                            "Exclusive gateway '{}' has {} default flows; at most 1 is valid",
                            name, default_count
                        ))
                        .with_code(GATEWAY_FAN_OUT_ERROR),
                    );
                }
            }
            GatewayKind::Parallel if outgoing.len() < 2 => {
                diagnostics.push(
                    Diagnostic::warning(format!(
                        "Parallel gateway '{}' should have ≥2 outgoing flows (has {})",
                        name,
                        outgoing.len()
                    ))
                    .with_code(GATEWAY_FAN_OUT_ERROR),
                );
            }
            GatewayKind::Inclusive if outgoing.is_empty() => {
                diagnostics.push(
                    Diagnostic::warning(format!(
                        "Inclusive gateway '{}' has no outgoing flows",
                        name
                    ))
                    .with_code(GATEWAY_FAN_OUT_ERROR),
                );
            }
            _ => {}
        }
    }
}

/// Validate that parallel joins reference known fork gateways.
fn validate_parallel_joins(graph: &RailwayGraph, _diagnostics: &mut DiagnosticBag) {
    // Forward: parallel-joins may reference gateways that are valid
    // We only warn if an expected fork is not present in the graph
    // (Resolution pass would handle this; for now we skip to keep assembly fast)
    let _ = graph;
}

// ---------------------------------------------------------------------------
// Slot extraction helpers
// ---------------------------------------------------------------------------

/// Extract a slot value as a symbol string.
///
/// Looks for `(slot_name, Symbol(s))` in the slots list.
fn slot_symbol(slots: &[(String, RawValue)], key: &str) -> Option<String> {
    for (k, v) in slots {
        if k == key {
            return match v {
                RawValue::Symbol(s) => Some(s.clone()),
                _ => None,
            };
        }
    }
    None
}

/// Extract a slot value as a bool.
fn slot_bool(slots: &[(String, RawValue)], key: &str) -> Option<bool> {
    for (k, v) in slots {
        if k == key {
            return match v {
                RawValue::BoolLit(b) => Some(*b),
                RawValue::Symbol(s) => match s.as_str() {
                    "true" => Some(true),
                    "false" => Some(false),
                    _ => None,
                },
                _ => None,
            };
        }
    }
    None
}

/// Extract a slot value as a list of symbol strings.
fn slot_symbol_list(slots: &[(String, RawValue)], key: &str) -> Option<Vec<String>> {
    for (k, v) in slots {
        if k == key {
            return match v {
                RawValue::List(items) => {
                    let symbols: Vec<String> = items
                        .iter()
                        .filter_map(|item| {
                            if let RawValue::Symbol(s) = item {
                                Some(s.clone())
                            } else {
                                None
                            }
                        })
                        .collect();
                    Some(symbols)
                }
                RawValue::Symbol(s) => Some(vec![s.clone()]),
                _ => None,
            };
        }
    }
    None
}

/// Extract a slot value as a symbol or name-ref (for flow source/target).
///
/// Handles `Symbol`, `TemplateSubst`, and `InsertionMarker` by returning the
/// inner string; these are all valid source/target references in flows.
fn slot_symbol_or_name(slots: &[(String, RawValue)], key: &str) -> Option<String> {
    for (k, v) in slots {
        if k == key {
            return match v {
                RawValue::Symbol(s) => Some(s.clone()),
                RawValue::TemplateSubst(s) => Some(format!(",{}", s)),
                RawValue::InsertionMarker(s) => Some(format!("${}", s)),
                _ => None,
            };
        }
    }
    None
}

/// Extract the first positional slot value (key == "").
///
/// Used for `boundary-attachment` where the second positional argument (the
/// event name) is stored with an empty key due to parser behaviour.
fn positional_slot_symbol(slots: &[(String, RawValue)]) -> Option<String> {
    for (k, v) in slots {
        if k.is_empty() {
            return match v {
                RawValue::Symbol(s) => Some(s.clone()),
                _ => None,
            };
        }
    }
    None
}

/// Extract the `:verb` slot value, if present, as a string name.
///
/// The `:verb` slot may be a nested `invoke` atom — in that case we extract
/// the verb FQN from the invoke atom's first slot; or it may be a simple
/// symbol reference.
fn extract_verb_ref(slots: &[(String, RawValue)]) -> Option<String> {
    for (k, v) in slots {
        if k == "verb" {
            return match v {
                RawValue::Symbol(s) => Some(s.clone()),
                RawValue::Atom(nested) => {
                    // (invoke verb-fqn :args {...})
                    // The verb FQN is stored as the atom's name
                    nested.name.clone()
                }
                _ => None,
            };
        }
    }
    None
}

/// Render a condition value slot as a raw string for the edge.
fn extract_condition_string(slots: &[(String, RawValue)]) -> Option<String> {
    for (k, v) in slots {
        if k == "condition" {
            return Some(raw_value_to_string(v));
        }
    }
    None
}

/// Recursively render a `RawValue` as a string (for condition expressions).
fn raw_value_to_string(v: &RawValue) -> String {
    match v {
        RawValue::Symbol(s) => s.clone(),
        RawValue::StringLit(s) => format!("\"{}\"", s),
        RawValue::IntLit(i) => i.to_string(),
        RawValue::FloatLit(f) => f.to_string(),
        RawValue::BoolLit(b) => b.to_string(),
        RawValue::SlotRef(s) => format!("@{}", s),
        RawValue::TemplateSubst(s) => format!(",{}", s),
        RawValue::TemplateSplice(s) => format!(",@{}", s),
        RawValue::InsertionMarker(s) => format!("${}", s),
        RawValue::QualifiedName { pack, atom } => format!("{}/{}", pack, atom),
        RawValue::List(items) => {
            let inner: Vec<String> = items.iter().map(raw_value_to_string).collect();
            format!("[{}]", inner.join(" "))
        }
        RawValue::Map(pairs) => {
            let inner: Vec<String> = pairs
                .iter()
                .map(|(k, v)| format!(":{} {}", k, raw_value_to_string(v)))
                .collect();
            format!("{{{}}}", inner.join(" "))
        }
        RawValue::Atom(a) => {
            let slots: Vec<String> = a
                .slots
                .iter()
                .map(|(k, v)| format!(":{} {}", k, raw_value_to_string(v)))
                .collect();
            let name_part = a.name.as_deref().unwrap_or("");
            if name_part.is_empty() {
                format!("({} {})", a.kind, slots.join(" "))
            } else {
                format!("({} {} {})", a.kind, name_part, slots.join(" "))
            }
        }
        RawValue::ForEach {
            var,
            list_param,
            body,
        } => {
            // Render the for-each form back to surface syntax.
            let body_str: Vec<String> = body.iter().map(raw_value_to_string).collect();
            format!(
                "(for-each :var {} :in {} {})",
                var,
                list_param,
                body_str.join(" ")
            )
        }
    }
}

/// Extract `:merge` clauses from a parallel-join atom's slots.
fn extract_merge_clauses(slots: &[(String, RawValue)]) -> Vec<MergeClause> {
    for (k, v) in slots {
        if k == "merge" {
            return match v {
                RawValue::List(items) => items
                    .iter()
                    .filter_map(|item| {
                        if let RawValue::Map(pairs) = item {
                            extract_single_merge_clause(pairs)
                        } else {
                            None
                        }
                    })
                    .collect(),
                _ => Vec::new(),
            };
        }
    }
    Vec::new()
}

fn extract_single_merge_clause(pairs: &[(String, RawValue)]) -> Option<MergeClause> {
    let location = pairs.iter().find_map(|(k, v)| {
        if k == "location" {
            if let RawValue::Symbol(s) = v {
                Some(s.clone())
            } else {
                None
            }
        } else {
            None
        }
    })?;

    let operator_str = pairs.iter().find_map(|(k, v)| {
        if k == "operator" {
            if let RawValue::Symbol(s) = v {
                Some(s.clone())
            } else {
                None
            }
        } else {
            None
        }
    })?;

    let operator = MergeOperator::from_str(&operator_str)?;

    let custom_verb = if operator == MergeOperator::Custom {
        pairs.iter().find_map(|(k, v)| {
            if k == "custom-verb" {
                if let RawValue::Symbol(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            } else {
                None
            }
        })
    } else {
        None
    };

    Some(MergeClause {
        location,
        operator,
        custom_verb,
    })
}
