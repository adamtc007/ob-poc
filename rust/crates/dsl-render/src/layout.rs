//! Topological left-to-right layout for bpmn-lite railway graphs.
//!
//! Uses BFS from the start node to assign column depths, then distributes
//! nodes evenly within each column.

use std::collections::{HashMap, VecDeque};

use dsl_bpmn_frontend::RailwayGraph;

use crate::shapes::{NodeLayout, Point};

// Layout constants
const NODE_W: f64 = 120.0;
const NODE_H: f64 = 50.0;
const GW_SIZE: f64 = 50.0;
const H_GAP: f64 = 70.0;
const V_GAP: f64 = 50.0;
const MARGIN: f64 = 40.0;

/// Compute a left-to-right topological layout for all nodes in the graph.
///
/// Each node is assigned a column based on BFS depth from the start node.
/// Nodes at the same depth are stacked vertically.
pub fn compute_layout(graph: &RailwayGraph) -> HashMap<String, NodeLayout> {
    // Collect all node IDs (nodes + gateways + parallel joins)
    let all_node_ids: Vec<String> = graph
        .nodes
        .keys()
        .chain(graph.gateways.keys())
        .chain(graph.parallel_joins.keys())
        .cloned()
        .collect();

    if all_node_ids.is_empty() {
        return HashMap::new();
    }

    // BFS from start_node (or first node alphabetically) to get depths
    let start = graph
        .start_node
        .clone()
        .or_else(|| {
            let mut sorted = all_node_ids.clone();
            sorted.sort();
            sorted.into_iter().next()
        })
        .unwrap_or_default();

    let mut depth: HashMap<String, usize> = HashMap::new();
    let mut queue = VecDeque::new();
    if !start.is_empty() {
        queue.push_back((start.clone(), 0usize));
        depth.insert(start, 0);
    }

    while let Some((node_id, d)) = queue.pop_front() {
        for edge in &graph.edges {
            if edge.source == node_id && !depth.contains_key(&edge.target) {
                depth.insert(edge.target.clone(), d + 1);
                queue.push_back((edge.target.clone(), d + 1));
            }
        }
    }

    // Assign unreachable nodes (including boundary events) to the column after the max
    let max_depth = depth.values().copied().max().unwrap_or(0);
    let mut extra_col = max_depth + 1;
    for id in &all_node_ids {
        if !depth.contains_key(id) {
            depth.insert(id.clone(), extra_col);
            extra_col += 1;
        }
    }

    // Group by depth, sort nodes within each depth for a stable layout
    let mut by_depth: HashMap<usize, Vec<String>> = HashMap::new();
    for (id, d) in &depth {
        by_depth.entry(*d).or_default().push(id.clone());
    }
    for nodes in by_depth.values_mut() {
        nodes.sort();
    }

    // Build layout map
    let mut layout = HashMap::new();
    for (d, nodes) in &by_depth {
        let col = *d;
        for (row, id) in nodes.iter().enumerate() {
            let is_gateway = graph.gateways.contains_key(id);
            let is_parallel_join = graph.parallel_joins.contains_key(id);
            let (w, h) = if is_gateway || is_parallel_join {
                (GW_SIZE, GW_SIZE)
            } else {
                (NODE_W, NODE_H)
            };

            // Centre gateways (smaller) in the same cell used by tasks
            let cell_w = NODE_W + H_GAP;
            let cell_h = NODE_H + V_GAP;
            let cell_x = MARGIN + col as f64 * cell_w;
            let cell_y = MARGIN + row as f64 * cell_h;

            // Offset so the shape is centred inside its grid cell
            let x = cell_x + (NODE_W - w) / 2.0;
            let y = cell_y + (NODE_H - h) / 2.0;

            layout.insert(
                id.clone(),
                NodeLayout {
                    id: id.clone(),
                    pos: Point { x, y },
                    width: w,
                    height: h,
                },
            );
        }
    }

    layout
}
