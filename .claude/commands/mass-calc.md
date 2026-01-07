# Mass Calculation Implementation

Implement the mass calculation system that determines view mode and blast radius.

## Files to Create/Modify

1. `rust/crates/ob-poc-types/src/layout/mass.rs` - Mass types and computation
2. `rust/crates/ob-poc-types/src/layout/mod.rs` - Module exports

## Types to Implement

### StructMass
```rust
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct StructMass {
    /// Weighted total mass
    pub total: u32,
    /// Breakdown by entity type
    pub breakdown: MassBreakdown,
    /// Mass per node (total / node_count)
    pub density: f32,
    /// Maximum hierarchy depth
    pub depth: u32,
    /// Edge to node ratio
    pub complexity: f32,
}

#[derive(Debug, Clone, Default)]
pub struct MassBreakdown {
    pub cbus: u32,
    pub persons: u32,
    pub holdings: u32,
    pub edges: u32,
    pub floating: u32,  // Entities with no structural edges
}
```

### Configuration Types
```rust
#[derive(Debug, Clone, Deserialize)]
pub struct MassWeights {
    pub cbu: u32,
    pub person: u32,
    pub holding: u32,
    pub edge: u32,
    pub floating: u32,
}

impl Default for MassWeights {
    fn default() -> Self {
        Self {
            cbu: 100,
            person: 10,
            holding: 20,
            edge: 5,
            floating: 15,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MassThresholds {
    /// Above this → AstroOverview
    pub astro_threshold: u32,
    /// Above this (but below astro) → HybridDrilldown
    pub hybrid_threshold: u32,
}

impl Default for MassThresholds {
    fn default() -> Self {
        Self {
            astro_threshold: 500,
            hybrid_threshold: 100,
        }
    }
}
```

### Mass Computation
```rust
impl StructMass {
    pub fn compute(graph: &SemanticGraph, weights: &MassWeights) -> Self {
        let mut breakdown = MassBreakdown::default();
        
        // Count entities by type
        for node in &graph.nodes {
            match node.entity_type.as_str() {
                "CBU" | "cbu" => breakdown.cbus += 1,
                "PERSON" | "person" | "Person" => breakdown.persons += 1,
                "HOLDING" | "INTERMEDIATE" | "holding" => breakdown.holdings += 1,
                _ => {}
            }
            
            // Detect floating nodes (no structural edges)
            let has_structural_edge = graph.edges.iter().any(|e| {
                (e.source == node.id || e.target == node.id) 
                    && is_structural_edge(&e.edge_type)
            });
            
            if !has_structural_edge {
                breakdown.floating += 1;
            }
        }
        
        breakdown.edges = graph.edges.len() as u32;
        
        // Compute weighted total
        let total = 
            breakdown.cbus * weights.cbu +
            breakdown.persons * weights.person +
            breakdown.holdings * weights.holding +
            breakdown.edges * weights.edge +
            breakdown.floating * weights.floating;
        
        // Compute depth (max hierarchy levels)
        let depth = compute_max_depth(graph);
        
        // Compute complexity (edges per node)
        let node_count = graph.nodes.len().max(1);
        let complexity = breakdown.edges as f32 / node_count as f32;
        let density = total as f32 / node_count as f32;
        
        Self {
            total,
            breakdown,
            density,
            depth,
            complexity,
        }
    }
    
    /// Determine suggested view mode based on mass
    pub fn suggested_view(&self, thresholds: &MassThresholds) -> ViewMode {
        if self.total > thresholds.astro_threshold {
            ViewMode::AstroOverview
        } else if self.total > thresholds.hybrid_threshold {
            ViewMode::HybridDrilldown
        } else if self.breakdown.cbus > 1 {
            ViewMode::MultiCbuDetail
        } else {
            ViewMode::SingleCbuPyramid
        }
    }
    
    /// Human-readable summary
    pub fn summary(&self) -> String {
        format!(
            "Mass: {} ({} CBUs, {} persons, {} holdings, {} floating)",
            self.total,
            self.breakdown.cbus,
            self.breakdown.persons,
            self.breakdown.holdings,
            self.breakdown.floating
        )
    }
}

fn is_structural_edge(edge_type: &str) -> bool {
    matches!(
        edge_type.to_uppercase().as_str(),
        "OWNS" | "CONTROLS" | "BENEFICIAL_OWNER" | "SHAREHOLDER" | 
        "PARENT" | "SUBSIDIARY" | "DIRECTOR" | "SIGNATORY"
    )
}

fn compute_max_depth(graph: &SemanticGraph) -> u32 {
    // Find root nodes (no incoming structural edges)
    let roots: Vec<_> = graph.nodes.iter()
        .filter(|n| {
            !graph.edges.iter().any(|e| 
                e.target == n.id && is_structural_edge(&e.edge_type)
            )
        })
        .collect();
    
    if roots.is_empty() {
        return 0;
    }
    
    // BFS from each root to find max depth
    let mut max_depth = 0u32;
    
    for root in roots {
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back((root.id, 0u32));
        
        while let Some((node_id, depth)) = queue.pop_front() {
            if visited.contains(&node_id) {
                continue;
            }
            visited.insert(node_id);
            max_depth = max_depth.max(depth);
            
            // Find children
            for edge in &graph.edges {
                if edge.source == node_id && is_structural_edge(&edge.edge_type) {
                    queue.push_back((edge.target, depth + 1));
                }
            }
        }
    }
    
    max_depth
}
```

### Blast Radius Check
```rust
#[derive(Debug)]
pub struct BlastRadiusCheck {
    pub operation: String,
    pub scope: String,
    pub affected_entities: usize,
    pub affected_cbus: usize,
    pub mass: u32,
    pub requires_confirmation: bool,
}

impl BlastRadiusCheck {
    pub fn new(
        operation: &str,
        scope: &str,
        mass: &StructMass,
        confirmation_threshold: u32,
    ) -> Self {
        Self {
            operation: operation.to_string(),
            scope: scope.to_string(),
            affected_entities: (mass.breakdown.cbus + mass.breakdown.persons + mass.breakdown.holdings) as usize,
            affected_cbus: mass.breakdown.cbus as usize,
            mass: mass.total,
            requires_confirmation: mass.total > confirmation_threshold,
        }
    }
    
    pub fn display(&self) -> String {
        if self.requires_confirmation {
            format!(
                "⚠️  CONFIRM: '{}' will affect {} entities ({} CBUs) in scope [{}]\n\
                 Mass: {} — Type 'yes' to proceed:",
                self.operation,
                self.affected_entities,
                self.affected_cbus,
                self.scope,
                self.mass
            )
        } else {
            format!(
                "→ '{}' targeting {} entities in [{}]",
                self.operation,
                self.affected_entities,
                self.scope
            )
        }
    }
}
```

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_graph(cbu_count: usize, person_count: usize) -> SemanticGraph {
        let mut nodes = vec![];
        let mut edges = vec![];
        
        for i in 0..cbu_count {
            nodes.push(Node {
                id: EntityId(i as u64),
                name: format!("CBU-{}", i),
                entity_type: "CBU".to_string(),
                ..Default::default()
            });
        }
        
        for i in 0..person_count {
            nodes.push(Node {
                id: EntityId((cbu_count + i) as u64),
                name: format!("Person-{}", i),
                entity_type: "PERSON".to_string(),
                ..Default::default()
            });
            
            // Link half the persons to CBUs
            if i < person_count / 2 && !nodes.is_empty() {
                edges.push(Edge {
                    source: EntityId((cbu_count + i) as u64),
                    target: EntityId((i % cbu_count) as u64),
                    edge_type: "OWNS".to_string(),
                });
            }
        }
        
        SemanticGraph { nodes, edges }
    }
    
    #[test]
    fn test_mass_computation() {
        let graph = create_test_graph(5, 20);
        let weights = MassWeights::default();
        let mass = StructMass::compute(&graph, &weights);
        
        assert_eq!(mass.breakdown.cbus, 5);
        assert_eq!(mass.breakdown.persons, 20);
        // Half are floating (not linked)
        assert_eq!(mass.breakdown.floating, 10);
    }
    
    #[test]
    fn test_suggested_view_small() {
        let graph = create_test_graph(1, 5);
        let mass = StructMass::compute(&graph, &MassWeights::default());
        let thresholds = MassThresholds::default();
        
        assert_eq!(mass.suggested_view(&thresholds), ViewMode::SingleCbuPyramid);
    }
    
    #[test]
    fn test_suggested_view_large() {
        let graph = create_test_graph(50, 200);
        let mass = StructMass::compute(&graph, &MassWeights::default());
        let thresholds = MassThresholds::default();
        
        assert_eq!(mass.suggested_view(&thresholds), ViewMode::AstroOverview);
    }
    
    #[test]
    fn test_blast_radius_confirmation() {
        let graph = create_test_graph(10, 50);
        let mass = StructMass::compute(&graph, &MassWeights::default());
        
        let check = BlastRadiusCheck::new(
            "delete",
            "allianz.trading",
            &mass,
            100, // confirmation threshold
        );
        
        assert!(check.requires_confirmation);
    }
}
```

## Acceptance Criteria

- [ ] Mass computed correctly from graph structure
- [ ] Floating entities (no structural edges) detected
- [ ] Weights configurable via MassWeights
- [ ] suggested_view() returns appropriate ViewMode
- [ ] BlastRadiusCheck warns for large operations
- [ ] Depth computed via BFS from root nodes
