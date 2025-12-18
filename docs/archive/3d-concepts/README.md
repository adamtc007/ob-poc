# Archived: 3D Visualization Concepts

These TODOs explored 3D "silo" visualization for containers:
- Flying into containers as 3D tubes
- GPU instancing for 10,000+ items
- wgpu rendering pipeline
- 3D ray-casting for hit testing
- LOD (Level of Detail) with hysteresis
- SpringVec3 3D spring animations

**Decision:** Virtual scrolling through slide-in panels is sufficient for
the business viewer use case. Container contents are searched/browsed via
EntityGateway with pagination, not rendered as 3D geometry.

**Why this approach is better:**
1. egui cannot handle true 3D rendering efficiently
2. EntityGateway already provides fuzzy search infrastructure
3. Slide-in panels follow existing Resolution Panel patterns
4. Virtual scrolling handles 10,000+ items without GPU complexity
5. Business users prefer list/table views for data exploration

The following concepts from these TODOs ARE still relevant and have been
incorporated into TODO-CBU-CONTAINERS.md:
- Container data model (is_container, child_count, browse_nickname)
- GraphNode container fields
- Database tables for share_classes, investor_holdings

Archived: 2025-12-18
