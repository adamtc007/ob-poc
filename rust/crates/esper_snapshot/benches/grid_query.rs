//! Benchmarks for grid spatial query performance.
//!
//! Run with: cargo bench -p esper_snapshot

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use esper_snapshot::{GridSnapshot, Rect, Vec2};

/// Build a grid with N entities evenly distributed.
fn build_grid(entity_count: usize, cell_size: f32) -> GridSnapshot {
    let side = (entity_count as f32).sqrt().ceil() as usize;
    let world_size = side as f32 * 10.0; // 10 units spacing

    let bounds = Rect::new(0.0, 0.0, world_size, world_size);
    let mut builder = esper_snapshot::grid::GridBuilder::new(bounds, cell_size);

    for i in 0..entity_count {
        let x = (i % side) as f32 * 10.0 + 5.0;
        let y = (i / side) as f32 * 10.0 + 5.0;
        builder.add_entity(i as u32, Vec2::new(x, y));
    }

    builder.build()
}

fn bench_grid_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("grid_query");

    for entity_count in [1_000, 10_000, 100_000] {
        let grid = build_grid(entity_count, 100.0);
        let world_size = (entity_count as f32).sqrt().ceil() * 10.0;

        // Query covering ~10% of the world
        let viewport_size = world_size * 0.316; // sqrt(0.1)
        let viewport = Rect::new(0.0, 0.0, viewport_size, viewport_size);

        group.bench_with_input(
            BenchmarkId::new("10pct_viewport", entity_count),
            &entity_count,
            |b, _| {
                b.iter(|| {
                    let count: usize = grid.query_visible(black_box(viewport)).count();
                    black_box(count)
                });
            },
        );

        // Query covering single cell
        let single_cell = Rect::new(50.0, 50.0, 60.0, 60.0);
        group.bench_with_input(
            BenchmarkId::new("single_cell", entity_count),
            &entity_count,
            |b, _| {
                b.iter(|| {
                    let count: usize = grid.query_visible(black_box(single_cell)).count();
                    black_box(count)
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_grid_query);
criterion_main!(benches);
