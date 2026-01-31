//! Spatial grid for efficient viewport queries.
//!
//! The grid enables O(cells) viewport culling instead of O(entities).
//! For a 1000x1000 world with 100x100 cells, querying a viewport that
//! covers 4 cells is ~250x faster than iterating all entities.

use crate::{Rect, Vec2};
use serde::{Deserialize, Serialize};

/// Spatial index grid for fast viewport queries.
///
/// The grid divides the chamber's bounding box into uniform cells.
/// Each cell stores a range into `cell_entity_indices` which contains
/// the chamber-local entity indices within that cell.
///
/// # Layout
///
/// ```text
/// +---+---+---+---+
/// | 0 | 1 | 2 | 3 |  cell indices (row-major)
/// +---+---+---+---+
/// | 4 | 5 | 6 | 7 |
/// +---+---+---+---+
///
/// cell_ranges: [(start, count), ...]
/// cell_entity_indices: [entity_idx, entity_idx, ...]
/// ```
///
/// # Query Algorithm
///
/// 1. Convert viewport to cell range (min_cell, max_cell)
/// 2. For each cell in range:
///    - Look up (start, count) in cell_ranges
///    - Iterate cell_entity_indices[start..start+count]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GridSnapshot {
    /// Size of each grid cell in world units.
    pub cell_size: f32,

    /// World-space origin of the grid (min corner).
    pub origin: Vec2,

    /// Grid dimensions (columns, rows).
    pub dims: (u32, u32),

    /// Per-cell range into cell_entity_indices: (start, count).
    /// Length = dims.0 * dims.1
    pub cell_ranges: Vec<(u32, u32)>,

    /// Flat array of entity indices, grouped by cell.
    /// Cells reference contiguous ranges in this array.
    pub cell_entity_indices: Vec<u32>,
}

impl GridSnapshot {
    /// Create an empty grid (no spatial indexing).
    pub fn empty() -> Self {
        Self::default()
    }

    /// Check if the grid is empty (no spatial indexing).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.dims.0 == 0 || self.dims.1 == 0
    }

    /// Total number of cells.
    #[inline]
    pub fn cell_count(&self) -> usize {
        (self.dims.0 * self.dims.1) as usize
    }

    /// Convert world position to cell coordinates.
    ///
    /// Returns None if position is outside the grid.
    #[inline]
    pub fn world_to_cell(&self, pos: Vec2) -> Option<(u32, u32)> {
        if self.is_empty() || self.cell_size <= 0.0 {
            return None;
        }

        let local_x = pos.x - self.origin.x;
        let local_y = pos.y - self.origin.y;

        if local_x < 0.0 || local_y < 0.0 {
            return None;
        }

        let cx = (local_x / self.cell_size) as u32;
        let cy = (local_y / self.cell_size) as u32;

        if cx >= self.dims.0 || cy >= self.dims.1 {
            return None;
        }

        Some((cx, cy))
    }

    /// Convert cell coordinates to cell index.
    #[inline]
    fn cell_to_index(&self, cx: u32, cy: u32) -> usize {
        (cy * self.dims.0 + cx) as usize
    }

    /// Get entity indices in a specific cell.
    #[inline]
    pub fn entities_in_cell(&self, cx: u32, cy: u32) -> &[u32] {
        if cx >= self.dims.0 || cy >= self.dims.1 {
            return &[];
        }

        let cell_idx = self.cell_to_index(cx, cy);
        if cell_idx >= self.cell_ranges.len() {
            return &[];
        }

        let (start, count) = self.cell_ranges[cell_idx];
        let start = start as usize;
        let end = start + count as usize;

        if end > self.cell_entity_indices.len() {
            return &[];
        }

        &self.cell_entity_indices[start..end]
    }

    /// Query entities visible in a viewport rectangle.
    ///
    /// Returns an iterator over chamber-local entity indices.
    /// This is the hot path - called every frame.
    ///
    /// # Performance
    ///
    /// O(cells_in_viewport + entities_in_those_cells)
    /// For typical viewports covering 4-16 cells, this is much faster
    /// than iterating all entities.
    pub fn query_visible(&self, viewport: Rect) -> GridQueryIter<'_> {
        // Handle empty grid
        if self.is_empty() {
            return GridQueryIter::empty(self);
        }

        // Compute cell range for viewport
        let min_cell = self.viewport_to_cell_min(viewport);
        let max_cell = self.viewport_to_cell_max(viewport);

        GridQueryIter::new(self, min_cell, max_cell)
    }

    /// Convert viewport min corner to cell coords (clamped).
    #[inline]
    fn viewport_to_cell_min(&self, viewport: Rect) -> (u32, u32) {
        let local_x = (viewport.min.x - self.origin.x).max(0.0);
        let local_y = (viewport.min.y - self.origin.y).max(0.0);

        let cx = ((local_x / self.cell_size) as u32).min(self.dims.0.saturating_sub(1));
        let cy = ((local_y / self.cell_size) as u32).min(self.dims.1.saturating_sub(1));

        (cx, cy)
    }

    /// Convert viewport max corner to cell coords (clamped).
    #[inline]
    fn viewport_to_cell_max(&self, viewport: Rect) -> (u32, u32) {
        let local_x = (viewport.max.x - self.origin.x).max(0.0);
        let local_y = (viewport.max.y - self.origin.y).max(0.0);

        let cx = ((local_x / self.cell_size) as u32).min(self.dims.0.saturating_sub(1));
        let cy = ((local_y / self.cell_size) as u32).min(self.dims.1.saturating_sub(1));

        (cx, cy)
    }
}

/// Iterator over entity indices in a viewport query.
///
/// This is a zero-allocation iterator that walks cells and yields
/// entity indices.
pub struct GridQueryIter<'a> {
    grid: &'a GridSnapshot,
    min_cell: (u32, u32),
    max_cell: (u32, u32),
    current_cell: (u32, u32),
    current_entity_idx: usize,
    current_cell_end: usize,
    empty: bool,
}

impl<'a> GridQueryIter<'a> {
    fn new(grid: &'a GridSnapshot, min_cell: (u32, u32), max_cell: (u32, u32)) -> Self {
        let mut iter = Self {
            grid,
            min_cell,
            max_cell,
            current_cell: min_cell,
            current_entity_idx: 0,
            current_cell_end: 0,
            empty: false,
        };
        iter.advance_to_cell(min_cell.0, min_cell.1);
        iter
    }

    fn empty(grid: &'a GridSnapshot) -> Self {
        Self {
            grid,
            min_cell: (0, 0),
            max_cell: (0, 0),
            current_cell: (0, 0),
            current_entity_idx: 0,
            current_cell_end: 0,
            empty: true,
        }
    }

    /// Advance to a new cell, setting up entity range.
    fn advance_to_cell(&mut self, cx: u32, cy: u32) {
        self.current_cell = (cx, cy);

        let cell_idx = self.grid.cell_to_index(cx, cy);
        if cell_idx >= self.grid.cell_ranges.len() {
            self.current_entity_idx = 0;
            self.current_cell_end = 0;
            return;
        }

        let (start, count) = self.grid.cell_ranges[cell_idx];
        self.current_entity_idx = start as usize;
        self.current_cell_end = start as usize + count as usize;
    }

    /// Move to next cell in row-major order within viewport.
    fn next_cell(&mut self) -> bool {
        let (cx, cy) = self.current_cell;

        // Try next column
        if cx < self.max_cell.0 {
            self.advance_to_cell(cx + 1, cy);
            return true;
        }

        // Try next row
        if cy < self.max_cell.1 {
            self.advance_to_cell(self.min_cell.0, cy + 1);
            return true;
        }

        // No more cells
        false
    }
}

impl<'a> Iterator for GridQueryIter<'a> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.empty {
            return None;
        }

        loop {
            // Try to get next entity in current cell
            if self.current_entity_idx < self.current_cell_end {
                let idx = self.grid.cell_entity_indices[self.current_entity_idx];
                self.current_entity_idx += 1;
                return Some(idx);
            }

            // Move to next cell
            if !self.next_cell() {
                return None;
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // Upper bound is total entities in grid
        (0, Some(self.grid.cell_entity_indices.len()))
    }
}

// =============================================================================
// GRID BUILDER (for compiler)
// =============================================================================

/// Builder for constructing GridSnapshot from entity positions.
#[derive(Debug)]
pub struct GridBuilder {
    cell_size: f32,
    origin: Vec2,
    dims: (u32, u32),
    cells: Vec<Vec<u32>>,
}

impl GridBuilder {
    /// Create a new grid builder.
    ///
    /// # Arguments
    ///
    /// * `bounds` - Bounding box of all entities
    /// * `cell_size` - Target cell size in world units
    pub fn new(bounds: Rect, cell_size: f32) -> Self {
        let cell_size = cell_size.max(1.0); // Prevent zero/negative

        let width = bounds.width().max(cell_size);
        let height = bounds.height().max(cell_size);

        let cols = ((width / cell_size).ceil() as u32).max(1);
        let rows = ((height / cell_size).ceil() as u32).max(1);

        let cells = vec![Vec::new(); (cols * rows) as usize];

        Self {
            cell_size,
            origin: bounds.min,
            dims: (cols, rows),
            cells,
        }
    }

    /// Add an entity to the grid.
    pub fn add_entity(&mut self, entity_idx: u32, pos: Vec2) {
        let local_x = pos.x - self.origin.x;
        let local_y = pos.y - self.origin.y;

        let cx = ((local_x / self.cell_size) as u32).min(self.dims.0.saturating_sub(1));
        let cy = ((local_y / self.cell_size) as u32).min(self.dims.1.saturating_sub(1));

        let cell_idx = (cy * self.dims.0 + cx) as usize;
        if cell_idx < self.cells.len() {
            self.cells[cell_idx].push(entity_idx);
        }
    }

    /// Build the final GridSnapshot.
    pub fn build(self) -> GridSnapshot {
        let mut cell_ranges = Vec::with_capacity(self.cells.len());
        let mut cell_entity_indices = Vec::new();

        for cell in &self.cells {
            let start = cell_entity_indices.len() as u32;
            let count = cell.len() as u32;
            cell_ranges.push((start, count));
            cell_entity_indices.extend_from_slice(cell);
        }

        GridSnapshot {
            cell_size: self.cell_size,
            origin: self.origin,
            dims: self.dims,
            cell_ranges,
            cell_entity_indices,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_grid() {
        let grid = GridSnapshot::empty();
        assert!(grid.is_empty());
        assert_eq!(grid.cell_count(), 0);

        let viewport = Rect::new(0.0, 0.0, 100.0, 100.0);
        let results: Vec<_> = grid.query_visible(viewport).collect();
        assert!(results.is_empty());
    }

    #[test]
    fn single_cell_grid() {
        let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
        let mut builder = GridBuilder::new(bounds, 100.0);

        builder.add_entity(0, Vec2::new(10.0, 10.0));
        builder.add_entity(1, Vec2::new(50.0, 50.0));
        builder.add_entity(2, Vec2::new(90.0, 90.0));

        let grid = builder.build();

        assert_eq!(grid.dims, (1, 1));
        assert_eq!(grid.cell_count(), 1);

        let viewport = Rect::new(0.0, 0.0, 100.0, 100.0);
        let mut results: Vec<_> = grid.query_visible(viewport).collect();
        results.sort();
        assert_eq!(results, vec![0, 1, 2]);
    }

    #[test]
    fn multi_cell_grid() {
        let bounds = Rect::new(0.0, 0.0, 200.0, 200.0);
        let mut builder = GridBuilder::new(bounds, 100.0);

        // Add entities to different cells
        builder.add_entity(0, Vec2::new(10.0, 10.0)); // Cell (0,0)
        builder.add_entity(1, Vec2::new(110.0, 10.0)); // Cell (1,0)
        builder.add_entity(2, Vec2::new(10.0, 110.0)); // Cell (0,1)
        builder.add_entity(3, Vec2::new(110.0, 110.0)); // Cell (1,1)

        let grid = builder.build();

        assert_eq!(grid.dims, (2, 2));
        assert_eq!(grid.cell_count(), 4);

        // Query top-left cell only
        let viewport = Rect::new(0.0, 0.0, 50.0, 50.0);
        let results: Vec<_> = grid.query_visible(viewport).collect();
        assert_eq!(results, vec![0]);

        // Query bottom-right cell only
        let viewport = Rect::new(150.0, 150.0, 200.0, 200.0);
        let results: Vec<_> = grid.query_visible(viewport).collect();
        assert_eq!(results, vec![3]);

        // Query all cells
        let viewport = Rect::new(0.0, 0.0, 200.0, 200.0);
        let mut results: Vec<_> = grid.query_visible(viewport).collect();
        results.sort();
        assert_eq!(results, vec![0, 1, 2, 3]);
    }

    #[test]
    fn world_to_cell() {
        let bounds = Rect::new(0.0, 0.0, 200.0, 200.0);
        let builder = GridBuilder::new(bounds, 100.0);
        let grid = builder.build();

        assert_eq!(grid.world_to_cell(Vec2::new(10.0, 10.0)), Some((0, 0)));
        assert_eq!(grid.world_to_cell(Vec2::new(110.0, 10.0)), Some((1, 0)));
        assert_eq!(grid.world_to_cell(Vec2::new(10.0, 110.0)), Some((0, 1)));
        assert_eq!(grid.world_to_cell(Vec2::new(110.0, 110.0)), Some((1, 1)));

        // Out of bounds
        assert_eq!(grid.world_to_cell(Vec2::new(-10.0, 10.0)), None);
        assert_eq!(grid.world_to_cell(Vec2::new(210.0, 10.0)), None);
    }

    #[test]
    fn entities_in_cell() {
        let bounds = Rect::new(0.0, 0.0, 200.0, 200.0);
        let mut builder = GridBuilder::new(bounds, 100.0);

        builder.add_entity(0, Vec2::new(10.0, 10.0)); // Cell (0,0)
        builder.add_entity(1, Vec2::new(20.0, 20.0)); // Cell (0,0)
        builder.add_entity(2, Vec2::new(110.0, 10.0)); // Cell (1,0)

        let grid = builder.build();

        let cell_00 = grid.entities_in_cell(0, 0);
        assert_eq!(cell_00, &[0, 1]);

        let cell_10 = grid.entities_in_cell(1, 0);
        assert_eq!(cell_10, &[2]);

        let cell_01 = grid.entities_in_cell(0, 1);
        assert!(cell_01.is_empty());
    }
}
