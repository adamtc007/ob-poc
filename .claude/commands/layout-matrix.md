# Matrix Layout Implementation

Implement the matrix (grid) layout for trading instruments and tabular data.

## File to Create

`rust/crates/ob-poc-ui/src/layout/matrix.rs`

## Implementation

```rust
use std::collections::HashMap;
use egui::{Pos2, Vec2, Rect, pos2, vec2, Color32};

pub struct MatrixLayout;

impl MatrixLayout {
    pub fn new() -> Self {
        Self
    }
}

impl LayoutEngine for MatrixLayout {
    fn layout(
        &self,
        graph: &SemanticGraph,
        config: &TaxonomyLayoutConfig,
        viewport: Rect,
    ) -> PositionedGraph {
        let matrix_config = config.matrix_config();
        
        // 1. Group nodes by row and column attributes
        let (row_groups, col_groups) = group_by_attributes(
            graph,
            &matrix_config.rows_by,
            &matrix_config.cols_by,
        );
        
        // 2. Determine row and column order
        let row_order = if matrix_config.row_order.is_empty() {
            let mut keys: Vec<_> = row_groups.keys().cloned().collect();
            keys.sort();
            keys
        } else {
            matrix_config.row_order.clone()
        };
        
        let col_order = if matrix_config.col_order.is_empty() {
            let mut keys: Vec<_> = col_groups.keys().cloned().collect();
            keys.sort();
            keys
        } else {
            matrix_config.col_order.clone()
        };
        
        // 3. Compute cell sizes
        let (cell_width, cell_height) = compute_cell_sizes(
            graph,
            &row_groups,
            &col_groups,
            &row_order,
            &col_order,
            matrix_config.cell_padding,
        );
        
        // 4. Position header labels
        let header_height = matrix_config.header_height.unwrap_or(40.0);
        let row_header_width = 120.0;
        
        // 5. Position nodes in grid
        let mut positioned_nodes = vec![];
        
        // Create node lookup
        let node_map: HashMap<EntityId, &Node> = graph.nodes.iter()
            .map(|n| (n.id, n))
            .collect();
        
        // Grid start position
        let grid_start_x = viewport.left() + row_header_width + matrix_config.cell_padding;
        let grid_start_y = viewport.top() + header_height + matrix_config.cell_padding;
        
        for (row_idx, row_key) in row_order.iter().enumerate() {
            for (col_idx, col_key) in col_order.iter().enumerate() {
                // Find nodes that belong to this cell
                let cell_nodes: Vec<EntityId> = graph.nodes.iter()
                    .filter(|n| {
                        let row_val = n.attrs.get(&matrix_config.rows_by)
                            .map(|v| v.as_str())
                            .unwrap_or("");
                        let col_val = n.attrs.get(&matrix_config.cols_by)
                            .map(|v| v.as_str())
                            .unwrap_or("");
                        row_val == row_key && col_val == col_key
                    })
                    .map(|n| n.id)
                    .collect();
                
                // Position each node in the cell
                let cell_x = grid_start_x + col_idx as f32 * (cell_width + matrix_config.cell_padding);
                let cell_y = grid_start_y + row_idx as f32 * (cell_height + matrix_config.cell_padding);
                
                for (node_idx, node_id) in cell_nodes.iter().enumerate() {
                    if let Some(node) = node_map.get(node_id) {
                        // Stack multiple nodes vertically within cell
                        let offset_y = node_idx as f32 * 30.0;
                        let node_size = vec2(cell_width - 10.0, 25.0);
                        
                        let color = get_row_color(row_key);
                        
                        positioned_nodes.push(PositionedNode {
                            id: node.id,
                            name: node.name.clone(),
                            entity_type: node.entity_type.clone(),
                            position: pos2(
                                cell_x + cell_width / 2.0,
                                cell_y + offset_y + node_size.y / 2.0,
                            ),
                            size: node_size,
                            level: row_idx as u32,
                            ring: None,
                            style: NodeStyle {
                                fill_color: color,
                                shape: NodeShape::Rectangle,
                                ..Default::default()
                            },
                            is_floating: false,
                            can_drill_down: false,
                        });
                    }
                }
            }
        }
        
        // 6. Route edges (orthogonal routing)
        let edges = route_orthogonal_edges(graph, &positioned_nodes);
        
        // 7. Compute bounds
        let total_width = row_header_width + col_order.len() as f32 * (cell_width + matrix_config.cell_padding);
        let total_height = header_height + row_order.len() as f32 * (cell_height + matrix_config.cell_padding);
        
        let bounds = Rect::from_min_size(
            viewport.min,
            vec2(total_width, total_height),
        );
        
        PositionedGraph {
            nodes: positioned_nodes,
            edges,
            floating_zone: None,
            bounds,
            // Additional: row/col headers for rendering
            row_headers: Some(row_order),
            col_headers: Some(col_order),
            header_height: Some(header_height),
            row_header_width: Some(row_header_width),
            cell_width: Some(cell_width),
            cell_height: Some(cell_height),
        }
    }
}

/// Group nodes by row and column attributes
fn group_by_attributes(
    graph: &SemanticGraph,
    rows_by: &str,
    cols_by: &str,
) -> (HashMap<String, Vec<EntityId>>, HashMap<String, Vec<EntityId>>) {
    let mut row_groups: HashMap<String, Vec<EntityId>> = HashMap::new();
    let mut col_groups: HashMap<String, Vec<EntityId>> = HashMap::new();
    
    for node in &graph.nodes {
        let row_val = node.attrs.get(rows_by)
            .map(|v| v.clone())
            .unwrap_or_else(|| "OTHER".to_string());
        let col_val = node.attrs.get(cols_by)
            .map(|v| v.clone())
            .unwrap_or_else(|| "OTHER".to_string());
        
        row_groups.entry(row_val).or_default().push(node.id);
        col_groups.entry(col_val).or_default().push(node.id);
    }
    
    (row_groups, col_groups)
}

/// Compute cell sizes based on content
fn compute_cell_sizes(
    graph: &SemanticGraph,
    row_groups: &HashMap<String, Vec<EntityId>>,
    col_groups: &HashMap<String, Vec<EntityId>>,
    row_order: &[String],
    col_order: &[String],
    padding: f32,
) -> (f32, f32) {
    // Base cell size
    let min_width = 100.0;
    let min_height = 60.0;
    
    // Find max nodes in any cell
    let mut max_nodes_in_cell = 1;
    
    for row_key in row_order {
        for col_key in col_order {
            let count = graph.nodes.iter()
                .filter(|n| {
                    let row_val = n.attrs.get("instrument_type")
                        .map(|v| v.as_str())
                        .unwrap_or("");
                    let col_val = n.attrs.get("currency")
                        .map(|v| v.as_str())
                        .unwrap_or("");
                    row_val == row_key && col_val == col_key
                })
                .count();
            max_nodes_in_cell = max_nodes_in_cell.max(count);
        }
    }
    
    // Cell height based on max nodes
    let cell_height = (min_height + max_nodes_in_cell as f32 * 30.0).max(min_height);
    
    (min_width, cell_height)
}

/// Get color based on row category
fn get_row_color(row_key: &str) -> Color32 {
    match row_key.to_uppercase().as_str() {
        "EQUITY" => Color32::from_rgb(52, 152, 219),    // Blue
        "BOND" => Color32::from_rgb(39, 174, 96),       // Green
        "DERIVATIVE" => Color32::from_rgb(231, 76, 60), // Red
        "FUND" => Color32::from_rgb(155, 89, 182),      // Purple
        _ => Color32::from_rgb(127, 140, 141),          // Gray
    }
}

/// Route edges with orthogonal (right-angle) paths
fn route_orthogonal_edges(
    graph: &SemanticGraph,
    nodes: &[PositionedNode],
) -> Vec<PositionedEdge> {
    let node_positions: HashMap<EntityId, &PositionedNode> = nodes
        .iter()
        .map(|n| (n.id, n))
        .collect();
    
    let mut edges = vec![];
    
    for edge in &graph.edges {
        let source = match node_positions.get(&edge.source) {
            Some(n) => n,
            None => continue,
        };
        let target = match node_positions.get(&edge.target) {
            Some(n) => n,
            None => continue,
        };
        
        // Orthogonal routing: go horizontal first, then vertical
        let mid_x = (source.position.x + target.position.x) / 2.0;
        
        let path = vec![
            pos2(source.position.x + source.size.x / 2.0, source.position.y),
            pos2(mid_x, source.position.y),
            pos2(mid_x, target.position.y),
            pos2(target.position.x - target.size.x / 2.0, target.position.y),
        ];
        
        edges.push(PositionedEdge {
            source: edge.source,
            target: edge.target,
            edge_type: edge.edge_type.clone(),
            path,
            style: EdgeStyle {
                color: Color32::from_rgba_unmultiplied(100, 100, 100, 100),
                dashed: true,
                ..Default::default()
            },
        });
    }
    
    edges
}
```

## Extended PositionedGraph for Matrix

Add these fields to support matrix header rendering:

```rust
pub struct PositionedGraph {
    // ... existing fields
    
    /// Row headers (for matrix layout)
    pub row_headers: Option<Vec<String>>,
    /// Column headers (for matrix layout)
    pub col_headers: Option<Vec<String>>,
    /// Header row height
    pub header_height: Option<f32>,
    /// Row header column width  
    pub row_header_width: Option<f32>,
    /// Cell dimensions
    pub cell_width: Option<f32>,
    pub cell_height: Option<f32>,
}
```

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_instrument_graph() -> SemanticGraph {
        let mut nodes = vec![];
        
        // Add some instruments
        for (i, (inst_type, currency)) in [
            ("EQUITY", "USD"),
            ("EQUITY", "EUR"),
            ("BOND", "USD"),
            ("BOND", "GBP"),
            ("DERIVATIVE", "USD"),
        ].iter().enumerate() {
            let mut attrs = HashMap::new();
            attrs.insert("instrument_type".to_string(), inst_type.to_string());
            attrs.insert("currency".to_string(), currency.to_string());
            
            nodes.push(Node {
                id: EntityId(i as u64),
                name: format!("{}-{}", inst_type, currency),
                entity_type: "INSTRUMENT".to_string(),
                attrs,
                ..Default::default()
            });
        }
        
        SemanticGraph { nodes, edges: vec![] }
    }
    
    #[test]
    fn test_matrix_grouping() {
        let graph = create_instrument_graph();
        let (rows, cols) = group_by_attributes(&graph, "instrument_type", "currency");
        
        assert!(rows.contains_key("EQUITY"));
        assert!(rows.contains_key("BOND"));
        assert!(cols.contains_key("USD"));
        assert!(cols.contains_key("EUR"));
    }
    
    #[test]
    fn test_matrix_layout() {
        let graph = create_instrument_graph();
        let config = TaxonomyLayoutConfig {
            taxonomy: "trading_instruments".to_string(),
            layout: LayoutSpec {
                strategy: LayoutStrategy::Matrix,
                params: LayoutParams::Matrix(MatrixConfig {
                    rows_by: "instrument_type".to_string(),
                    cols_by: "currency".to_string(),
                    cell_padding: 10.0,
                    row_order: vec!["EQUITY".into(), "BOND".into(), "DERIVATIVE".into()],
                    col_order: vec!["USD".into(), "EUR".into(), "GBP".into()],
                    ..Default::default()
                }),
            },
            ..Default::default()
        };
        
        let viewport = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
        let layout = MatrixLayout::new();
        let result = layout.layout(&graph, &config, viewport);
        
        assert_eq!(result.nodes.len(), 5);
        assert!(result.row_headers.is_some());
        assert!(result.col_headers.is_some());
    }
}
```

## Acceptance Criteria

- [ ] Nodes grouped correctly by row/column attributes
- [ ] Custom row/column order respected
- [ ] Cell sizes accommodate content
- [ ] Headers generated for rows and columns
- [ ] Orthogonal edge routing (right-angle turns)
- [ ] Color coding by row category
