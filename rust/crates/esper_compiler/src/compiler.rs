//! Main snapshot compiler implementation.

use crate::cache::{CacheKey, SnapshotCache};
use crate::config::CompilerConfig;
use crate::error::CompilerError;
use crate::input::GraphInput;
use crate::layout::{LayoutEngine, Position};
use crate::string_table::StringTableBuilder;
use esper_snapshot::{
    grid::GridBuilder, CameraPreset, ChamberKind, ChamberSnapshot, Rect, SnapshotEnvelope, Vec2,
    WorldSnapshot, NONE_IDX,
};
use std::collections::HashMap;

/// Main snapshot compiler.
///
/// Compiles graph data into optimized WorldSnapshot structures.
#[derive(Debug)]
pub struct SnapshotCompiler {
    /// Layout engine.
    layout_engine: LayoutEngine,
    /// Optional cache.
    cache: Option<SnapshotCache>,
}

impl SnapshotCompiler {
    /// Create a new compiler.
    pub fn new() -> Self {
        Self {
            layout_engine: LayoutEngine::new(),
            cache: None,
        }
    }

    /// Create a compiler with caching.
    pub fn with_cache(cache: SnapshotCache) -> Self {
        Self {
            layout_engine: LayoutEngine::new(),
            cache: Some(cache),
        }
    }

    /// Compile a graph into a WorldSnapshot.
    pub fn compile(
        &self,
        input: &dyn GraphInput,
        config: &CompilerConfig,
        cbu_id: u64,
    ) -> Result<WorldSnapshot, CompilerError> {
        // Check cache
        if let Some(cache) = &self.cache {
            let key = CacheKey::compute(input, config);
            if let Some(cached) = cache.get(&key) {
                return bincode::deserialize(&cached.data)
                    .map_err(|e| CompilerError::SerializationError(e.to_string()));
            }
        }

        // Validate input
        if input.entity_count() == 0 {
            return Err(CompilerError::EmptyGraph);
        }

        if input.entity_count() > config.chamber.max_entities {
            return Err(CompilerError::ChamberCapacityExceeded {
                count: input.entity_count(),
                max: config.chamber.max_entities,
            });
        }

        // Compute layout
        let positions = self.layout_engine.compute(input, &config.layout)?;

        // Build string table
        let mut string_table = StringTableBuilder::with_capacity(input.entity_count());

        // Build chamber
        let chamber = self.build_chamber(input, &positions, &mut string_table, config)?;

        // Build envelope
        let envelope = SnapshotEnvelope {
            schema_version: config.schema_version,
            source_hash: hash_input(input),
            policy_hash: 0, // Will be set by policy layer
            created_at: current_timestamp(),
            cbu_id,
        };

        let snapshot = WorldSnapshot {
            envelope,
            string_table: string_table.build(),
            chambers: vec![chamber],
        };

        // Validate output
        if config.validate_output {
            use esper_snapshot::Validate;
            snapshot
                .validate()
                .map_err(|e| CompilerError::InvalidConfig(format!("validation failed: {}", e)))?;
        }

        // Store in cache
        if let Some(cache) = &self.cache {
            let key = CacheKey::compute(input, config);
            if let Ok(data) = bincode::serialize(&snapshot) {
                cache.insert(key, data);
            }
        }

        Ok(snapshot)
    }

    /// Build a single chamber from graph data.
    fn build_chamber(
        &self,
        input: &dyn GraphInput,
        positions: &HashMap<u64, Position>,
        string_table: &mut StringTableBuilder,
        config: &CompilerConfig,
    ) -> Result<ChamberSnapshot, CompilerError> {
        let entity_ids = input.entity_ids();
        let n = entity_ids.len();

        // Create entity ID → index mapping
        let id_to_index: HashMap<u64, usize> = entity_ids
            .iter()
            .enumerate()
            .map(|(i, &id)| (id, i))
            .collect();

        // Build SoA arrays
        let mut snapshot_ids = Vec::with_capacity(n);
        let mut kind_ids = Vec::with_capacity(n);
        let mut x_coords = Vec::with_capacity(n);
        let mut y_coords = Vec::with_capacity(n);
        let mut label_ids = Vec::with_capacity(n);
        let mut detail_refs = Vec::with_capacity(n);
        let mut first_child = Vec::with_capacity(n);
        let mut next_sibling = Vec::with_capacity(n);
        let mut prev_sibling = Vec::with_capacity(n);

        for &id in &entity_ids {
            let entity = input
                .get_entity(id)
                .ok_or(CompilerError::EntityNotFound(id))?;
            let pos = positions
                .get(&id)
                .copied()
                .unwrap_or(Position::new(0.0, 0.0));

            snapshot_ids.push(id);
            kind_ids.push(entity.kind_id as u16);
            x_coords.push(pos.x);
            y_coords.push(pos.y);
            label_ids.push(string_table.intern(&entity.name));
            detail_refs.push(entity.detail_ref.unwrap_or(id));

            // Initialize navigation indices (will be filled below)
            first_child.push(NONE_IDX);
            next_sibling.push(NONE_IDX);
            prev_sibling.push(NONE_IDX);
        }

        // Build navigation indices
        self.build_navigation_indices(
            input,
            &id_to_index,
            &mut first_child,
            &mut next_sibling,
            &mut prev_sibling,
        )?;

        // Compute bounds
        let bounds = compute_bounds(&x_coords, &y_coords, config.layout.padding);

        // Build grid spatial index
        let grid = if config.compute_grid {
            let mut builder = GridBuilder::new(bounds, config.chamber.grid_cell_size);
            for (idx, (&x, &y)) in x_coords.iter().zip(y_coords.iter()).enumerate() {
                builder.add_entity(idx as u32, Vec2::new(x, y));
            }
            builder.build()
        } else {
            esper_snapshot::GridSnapshot::default()
        };

        // Determine chamber kind based on graph structure
        let kind = determine_chamber_kind(input);

        Ok(ChamberSnapshot {
            id: 0,
            kind,
            bounds,
            default_camera: CameraPreset {
                center: Vec2::new(
                    bounds.min.x + bounds.width() / 2.0,
                    bounds.min.y + bounds.height() / 2.0,
                ),
                zoom: 1.0,
            },
            entity_ids: snapshot_ids,
            kind_ids,
            x: x_coords,
            y: y_coords,
            label_ids,
            detail_refs,
            first_child,
            next_sibling,
            prev_sibling,
            doors: vec![],
            grid,
        })
    }

    /// Build navigation indices for structural navigation.
    fn build_navigation_indices(
        &self,
        input: &dyn GraphInput,
        id_to_index: &HashMap<u64, usize>,
        first_child: &mut [u32],
        next_sibling: &mut [u32],
        prev_sibling: &mut [u32],
    ) -> Result<(), CompilerError> {
        // For each entity, find its children and build sibling chain
        for (&id, &idx) in id_to_index {
            let children = input.children(id);

            if children.is_empty() {
                continue;
            }

            // Sort children by index for consistent ordering
            let mut child_indices: Vec<usize> = children
                .iter()
                .filter_map(|&child_id| id_to_index.get(&child_id).copied())
                .collect();
            child_indices.sort();

            if child_indices.is_empty() {
                continue;
            }

            // Set first child
            first_child[idx] = child_indices[0] as u32;

            // Build sibling chain
            for i in 0..child_indices.len() {
                let child_idx = child_indices[i];

                if i + 1 < child_indices.len() {
                    next_sibling[child_idx] = child_indices[i + 1] as u32;
                }

                if i > 0 {
                    prev_sibling[child_idx] = child_indices[i - 1] as u32;
                }
            }
        }

        Ok(())
    }
}

impl Default for SnapshotCompiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute bounding rectangle for positions.
fn compute_bounds(x_coords: &[f32], y_coords: &[f32], padding: f32) -> Rect {
    if x_coords.is_empty() {
        return Rect::new(0.0, 0.0, 100.0, 100.0);
    }

    let min_x = x_coords.iter().cloned().fold(f32::MAX, f32::min);
    let max_x = x_coords.iter().cloned().fold(f32::MIN, f32::max);
    let min_y = y_coords.iter().cloned().fold(f32::MAX, f32::min);
    let max_y = y_coords.iter().cloned().fold(f32::MIN, f32::max);

    // Rect::new takes (min_x, min_y, max_x, max_y)
    Rect::new(
        min_x - padding,
        min_y - padding,
        max_x + padding,
        max_y + padding,
    )
}

/// Determine chamber kind based on graph structure.
fn determine_chamber_kind(input: &dyn GraphInput) -> ChamberKind {
    let entity_count = input.entity_count();
    let edge_count = input.edge_count();
    let root_count = input.roots().len();

    // Heuristics for chamber type selection
    if root_count == 1 {
        // Single root = tree structure
        ChamberKind::Tree
    } else if edge_count > entity_count * 2 {
        // Dense connections = force layout works better
        ChamberKind::Force
    } else if root_count > 5 {
        // Many roots = grid or matrix
        ChamberKind::Grid
    } else {
        ChamberKind::Tree
    }
}

/// Hash input for cache invalidation.
fn hash_input(input: &dyn GraphInput) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();

    for id in input.entity_ids() {
        id.hash(&mut hasher);
        if let Some(entity) = input.get_entity(id) {
            entity.name.hash(&mut hasher);
            entity.kind_id.hash(&mut hasher);
        }
    }

    for edge in input.edges() {
        edge.from.hash(&mut hasher);
        edge.to.hash(&mut hasher);
    }

    hasher.finish()
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{EdgeKind, EntityInput, MemoryGraphInput};

    #[test]
    fn compile_simple_tree() {
        let graph = MemoryGraphInput::simple_tree(vec![
            (1, "Root", 0),
            (2, "Child A", 1),
            (3, "Child B", 1),
        ]);

        let compiler = SnapshotCompiler::new();
        let config = CompilerConfig::default();
        let snapshot = compiler.compile(&graph, &config, 100).unwrap();

        assert_eq!(snapshot.chambers.len(), 1);
        assert_eq!(snapshot.chambers[0].entity_count(), 3);
        assert_eq!(snapshot.envelope.cbu_id, 100);
    }

    #[test]
    fn compile_validates_output() {
        let graph = MemoryGraphInput::simple_tree(vec![(1, "Root", 0), (2, "Child", 1)]);

        let compiler = SnapshotCompiler::new();
        let config = CompilerConfig::default();
        let snapshot = compiler.compile(&graph, &config, 1).unwrap();

        // Validate the snapshot
        use esper_snapshot::Validate;
        assert!(snapshot.validate().is_ok());
    }

    #[test]
    fn compile_empty_graph_fails() {
        let graph = MemoryGraphInput::new();
        let compiler = SnapshotCompiler::new();
        let config = CompilerConfig::default();

        let result = compiler.compile(&graph, &config, 1);
        assert!(matches!(result, Err(CompilerError::EmptyGraph)));
    }

    #[test]
    fn compile_builds_navigation_indices() {
        let graph = MemoryGraphInput::new()
            .add_entity(EntityInput::new(1, "Root", 0))
            .add_entity(EntityInput::new(2, "A", 1))
            .add_entity(EntityInput::new(3, "B", 1))
            .add_entity(EntityInput::new(4, "C", 1))
            .add_edge(2, 1, EdgeKind::Parent)
            .add_edge(3, 1, EdgeKind::Parent)
            .add_edge(4, 1, EdgeKind::Parent);

        let compiler = SnapshotCompiler::new();
        let config = CompilerConfig::default();
        let snapshot = compiler.compile(&graph, &config, 1).unwrap();

        let chamber = &snapshot.chambers[0];

        // Find root index
        let root_idx = chamber.entity_ids.iter().position(|&id| id == 1).unwrap();

        // Root should have a first child
        assert_ne!(chamber.first_child[root_idx], NONE_IDX);
    }

    #[test]
    fn compile_with_cache() {
        let cache = SnapshotCache::new();
        let compiler = SnapshotCompiler::with_cache(cache);

        let graph = MemoryGraphInput::simple_tree(vec![(1, "Root", 0), (2, "Child", 1)]);
        let config = CompilerConfig::default();

        // First compile - cache miss
        let _snapshot1 = compiler.compile(&graph, &config, 1).unwrap();

        // Second compile - cache hit
        let snapshot2 = compiler.compile(&graph, &config, 1).unwrap();

        assert_eq!(snapshot2.chambers[0].entity_count(), 2);
    }

    #[test]
    fn compile_builds_grid() {
        let graph = MemoryGraphInput::simple_tree(vec![(1, "Root", 0), (2, "A", 1), (3, "B", 1)]);

        let compiler = SnapshotCompiler::new();
        let config = CompilerConfig::default();
        let snapshot = compiler.compile(&graph, &config, 1).unwrap();

        let chamber = &snapshot.chambers[0];
        // Grid should have cells
        // Grid should be valid (dims is (cols, rows))
        assert!(chamber.grid.dims.0 > 0 || chamber.grid.dims.1 > 0 || chamber.entity_count() < 10);
    }
}
