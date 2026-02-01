## 16.3 Tuning & Experimentation (TODO Pack)

This section exists to ensure that **all parameters you expect to tweak during testing** are:
- externalized (policy/config)
- observable (debug overlay + logs)
- safe to change (validated)
- reproducible (hashes + deterministic snapshot)

### 16.3.1 TODO — Policy schema coverage
- [ ] Define a formal policy schema with versioning (policy `:version`).
- [ ] Add validation errors with precise paths (e.g. `flyover.dwell-ticks must be >= 0`).
- [ ] Canonicalize policy (stable ordering) before hashing.

### 16.3.2 TODO — Hot reload in dev
- [ ] File watcher for `config/policies/*.sexp` (or chosen format).
- [ ] On change: reload + validate; if valid, update active policy and invalidate snapshot cache by new `policy_hash`.
- [ ] If invalid: keep last-known-good policy active and surface error in logs + UI overlay.

### 16.3.3 TODO — End-to-end tunables (minimum set)
**Fly-over + phases**
- [ ] `dwell-ticks`, `settle-duration`, easing function for `focus_t`.
- [ ] Mode-specific defaults (spatial vs structural).

**LOD thresholds + hysteresis**
- [ ] `zoom-max` per tier; hysteresis per tier.
- [ ] Manual LOD cycling order for ENHANCE.

**Budgets**
- [ ] `label_budget_count`, `full_budget_count`
- [ ] `shape_budget_ms_per_frame` (client-side) — cap shaping time rather than entity count alone.

**Spatial index**
- [ ] `grid.cell_size` per chamber kind (matrix vs CBU graph).

**Text**
- [ ] `label_cache.max_entries`, width quantization buckets, eviction policy.

**Structural mode density cutovers**
- [ ] thresholds for icon/label/full per level density.

### 16.3.4 TODO — Debug overlay (client)
- [ ] Display: `source_hash`, `policy_hash`, `schema_version`.
- [ ] Display: current `NavigationPhase`, `focus_t`, manual/auto LOD mode.
- [ ] Display: visible counts per LOD tier (icons/labels/full).
- [ ] Display: label cache hit rate + current entries.
- [ ] Display: per-frame timings (visible query ms, shaping ms, paint ms) in dev builds.

### 16.3.5 TODO — Metrics + regression tests (server)
- [ ] Structured timing spans per stage (graph/chamberize/layout/intern/emit/compress).
- [ ] Snapshot size metrics (string table bytes, chamber bytes, total bytes).
- [ ] Determinism test: same `(source_hash, policy_hash, schema_version)` → same snapshot hash (run nightly / CI).

### 16.3.6 Acceptance Criteria
- [ ] A policy change (e.g. label budget 250→120) takes effect **without recompilation** and results in a new `policy_hash`.
- [ ] UI debug overlay shows the new hash and visible-tier counts change as expected.
- [ ] During rapid NEXT/NEXT navigation, shaping time remains ~0ms (MOVING phase).
- [ ] Snapshots remain deterministic for identical inputs.

