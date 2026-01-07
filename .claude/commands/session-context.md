# Session Context Implementation

Implement the unified session context system that serves as the single source of truth 
between REPL and Viewport.

## Files to Create/Modify

1. `rust/crates/ob-poc-types/src/session/mod.rs` - Module exports
2. `rust/crates/ob-poc-types/src/session/context.rs` - Core types
3. `rust/crates/ob-poc-types/src/session/manager.rs` - Thread-safe manager

## Types to Implement

### SessionContext
```rust
use std::sync::Arc;
use std::time::Instant;
use parking_lot::RwLock;
use tokio::sync::watch;

#[derive(Debug, Clone)]
pub struct SessionContext {
    /// Hierarchical scope path: "allianz.trading.germany"
    pub scope: ScopePath,
    
    /// The semantic graph at current scope
    pub graph: Arc<SemanticGraph>,
    
    /// Computed mass of current scope
    pub mass: StructMass,
    
    /// Currently focused entity (center of attention)
    pub focal_entity: Option<EntityId>,
    
    /// Multi-select for batch operations
    pub selected: Vec<EntityId>,
    
    /// Active filters
    pub filters: FilterSet,
    
    /// Current view mode (derived from mass + user preference)
    pub view_mode: ViewMode,
    
    /// Version counter - increments on any change
    pub version: u64,
    
    /// Last modified timestamp
    pub updated_at: Instant,
}
```

### ScopePath
```rust
#[derive(Debug, Clone, Default)]
pub struct ScopePath {
    pub segments: Vec<ScopeSegment>,
}

#[derive(Debug, Clone)]
pub struct ScopeSegment {
    pub name: String,
    pub entity_type: String,
    pub entity_id: Option<EntityId>,
    pub mass: u32,
}

impl ScopePath {
    pub fn new() -> Self { Self { segments: vec![] } }
    
    pub fn display(&self) -> String {
        self.segments.iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>()
            .join(".")
    }
    
    pub fn depth(&self) -> usize {
        self.segments.len()
    }
    
    pub fn parent(&self) -> Option<ScopePath> {
        if self.segments.len() <= 1 {
            None
        } else {
            Some(ScopePath {
                segments: self.segments[..self.segments.len() - 1].to_vec(),
            })
        }
    }
    
    pub fn push(&self, segment: ScopeSegment) -> ScopePath {
        let mut new_segments = self.segments.clone();
        new_segments.push(segment);
        ScopePath { segments: new_segments }
    }
    
    pub fn from_string(path: &str) -> Self {
        let segments = path.split('.')
            .filter(|s| !s.is_empty())
            .map(|s| ScopeSegment {
                name: s.to_string(),
                entity_type: "unknown".to_string(),
                entity_id: None,
                mass: 0,
            })
            .collect();
        ScopePath { segments }
    }
}
```

### SessionManager
```rust
pub struct SessionManager {
    context: Arc<RwLock<SessionContext>>,
    change_tx: watch::Sender<SessionContext>,
    change_rx: watch::Receiver<SessionContext>,
}

impl SessionManager {
    pub fn new(initial: SessionContext) -> Self {
        let (change_tx, change_rx) = watch::channel(initial.clone());
        Self {
            context: Arc::new(RwLock::new(initial)),
            change_tx,
            change_rx,
        }
    }
    
    /// Get current context (read)
    pub fn current(&self) -> SessionContext {
        self.context.read().clone()
    }
    
    /// Subscribe to changes (for REPL and Viewport)
    pub fn subscribe(&self) -> watch::Receiver<SessionContext> {
        self.change_rx.clone()
    }
    
    /// Navigate to a new scope
    pub fn navigate(&self, new_scope: ScopePath, graph: SemanticGraph) {
        let mut ctx = self.context.write();
        let graph = Arc::new(graph);
        let mass = StructMass::compute(&graph, &MassWeights::default());
        
        ctx.scope = new_scope;
        ctx.graph = graph;
        ctx.mass = mass.clone();
        ctx.view_mode = mass.suggested_view(&MassThresholds::default());
        ctx.focal_entity = None;
        ctx.selected.clear();
        ctx.version += 1;
        ctx.updated_at = Instant::now();
        
        let _ = self.change_tx.send(ctx.clone());
    }
    
    /// Set focal entity
    pub fn set_focus(&self, entity: Option<EntityId>) {
        let mut ctx = self.context.write();
        ctx.focal_entity = entity;
        ctx.version += 1;
        ctx.updated_at = Instant::now();
        let _ = self.change_tx.send(ctx.clone());
    }
    
    /// Drill down into entity
    pub fn drill_down(&self, entity_id: EntityId, subgraph: SemanticGraph) {
        let mut ctx = self.context.write();
        
        let entity_name = ctx.graph.nodes.iter()
            .find(|n| n.id == entity_id)
            .map(|n| n.name.clone())
            .unwrap_or_else(|| format!("{:?}", entity_id));
        
        let new_segment = ScopeSegment {
            name: entity_name,
            entity_type: "CBU".to_string(),
            entity_id: Some(entity_id),
            mass: subgraph.nodes.len() as u32,
        };
        
        let new_scope = ctx.scope.push(new_segment);
        let graph = Arc::new(subgraph);
        let mass = StructMass::compute(&graph, &MassWeights::default());
        
        ctx.scope = new_scope;
        ctx.graph = graph;
        ctx.mass = mass.clone();
        ctx.view_mode = mass.suggested_view(&MassThresholds::default());
        ctx.focal_entity = Some(entity_id);
        ctx.selected.clear();
        ctx.version += 1;
        ctx.updated_at = Instant::now();
        
        let _ = self.change_tx.send(ctx.clone());
    }
    
    /// Navigate up one level
    pub fn navigate_up(&self, parent_graph: SemanticGraph) {
        let mut ctx = self.context.write();
        
        if let Some(parent_scope) = ctx.scope.parent() {
            let graph = Arc::new(parent_graph);
            let mass = StructMass::compute(&graph, &MassWeights::default());
            
            ctx.scope = parent_scope;
            ctx.graph = graph;
            ctx.mass = mass.clone();
            ctx.view_mode = mass.suggested_view(&MassThresholds::default());
            ctx.focal_entity = None;
            ctx.selected.clear();
            ctx.version += 1;
            ctx.updated_at = Instant::now();
            
            let _ = self.change_tx.send(ctx.clone());
        }
    }
}
```

### ViewMode Enum
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewMode {
    AstroOverview,      // > 500 mass, solar system dots
    HybridDrilldown,    // 100-500 mass, CBUs with mini pyramids
    MultiCbuDetail,     // < 100 mass, multiple CBU pyramids
    SingleCbuPyramid,   // Single CBU, full pyramid detail
}
```

### FilterSet
```rust
#[derive(Debug, Clone, Default)]
pub struct FilterSet {
    pub filters: Vec<Filter>,
}

#[derive(Debug, Clone)]
pub enum Filter {
    ByRole(Vec<String>),
    ByEntityType(Vec<String>),
    ByAttribute { key: String, value: String },
    Custom(String),
}

impl FilterSet {
    pub fn add(&mut self, filter: Filter) {
        self.filters.push(filter);
    }
    
    pub fn clear(&mut self) {
        self.filters.clear();
    }
    
    pub fn is_empty(&self) -> bool {
        self.filters.is_empty()
    }
}
```

## Dependencies to Add

In `Cargo.toml`:
```toml
parking_lot = "0.12"
tokio = { version = "1", features = ["sync"] }
```

## Tests to Write

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_scope_path_navigation() {
        let path = ScopePath::from_string("allianz.trading.germany");
        assert_eq!(path.depth(), 3);
        assert_eq!(path.display(), "allianz.trading.germany");
        
        let parent = path.parent().unwrap();
        assert_eq!(parent.display(), "allianz.trading");
    }
    
    #[tokio::test]
    async fn test_session_manager_broadcasts() {
        let initial = SessionContext::default();
        let manager = SessionManager::new(initial);
        
        let mut rx1 = manager.subscribe();
        let mut rx2 = manager.subscribe();
        
        manager.set_focus(Some(EntityId::new()));
        
        assert!(rx1.has_changed().unwrap());
        assert!(rx2.has_changed().unwrap());
    }
}
```

## Acceptance Criteria

- [ ] SessionContext holds all session state
- [ ] ScopePath supports hierarchical navigation
- [ ] Version increments on any mutation
- [ ] All types are Clone + Send + Sync safe
- [ ] Watch channel broadcasts to all subscribers
- [ ] Thread-safe with parking_lot::RwLock
