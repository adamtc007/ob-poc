# WIP Refinements - Future Enhancements

> **Purpose:** Capture ideas, refinements, and next steps that aren't ready for implementation.
> **Rule:** Don't implement these until the core system is working and the pain point is felt.

---

## Viewport Guard Rails

**When:** After initial testing reveals edge cases where users "lose" their view.

**The Problem:** User can accidentally zoom to pixel level, pan into void, or lose focus target.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         VIEWPORT GUARD RAILS                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ZOOM LIMITS                                                                │
│  ├── Floor: Node radius >= 8px (readable)                                   │
│  ├── Ceiling: Entire graph fits with 20% margin                             │
│  └── If limit hit: Ignore input, subtle bounce/resistance                   │
│                                                                             │
│  PAN LIMITS (geofence)                                                      │
│  ├── Viewport center must stay within canvas bounds + margin                │
│  ├── Can't pan into void (nothing visible)                                  │
│  └── Elastic resistance at edges, snaps back                                │
│                                                                             │
│  FOCUS PRESERVATION                                                         │
│  ├── If focus target leaves viewport → auto-pan to keep visible             │
│  ├── If focus target deleted → focus nearest or ascend                      │
│  └── If enhance produces zero visible nodes → stay at current level         │
│                                                                             │
│  RETURN TO HOME                                                             │
│  ├── VIEWPORT.reset() → fit all, focus root, enhance L1                     │
│  ├── Double-tap/double-click on void → reset                                │
│  └── Escape key → ascend or reset if at root                                │
│                                                                             │
│  SANITY CHECKS                                                              │
│  ├── Min 1 entity visible at all times (unless empty graph)                 │
│  ├── Zoom velocity damping (momentum can't exceed 2x per frame)             │
│  └── Focus stack depth limit (50? prevent infinite descend loops)           │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Extended Intent Taxonomy

**When:** Adding voice/NL agent interface that needs to classify user intent.

**The Idea:** Higher-level intent layer that dispatches to specific verbs.

| Intent | Maps To |
|--------|---------|
| NAVIGATE | focus, zoom, pan, orbit, fit |
| REVEAL | enhance, descend, expand, trace path |
| FILTER | view, filter by predicate (not yet implemented) |
| INSPECT | show detail panel without viewport change (gap) |
| UNDO/BACK | ascend, history stack (partial) |

**Gaps to fill when needed:**
- `VIEWPORT.inspect()` - detail panel without enhance level change
- `VIEWPORT.filter.by(predicate)` - in-place filtering
- `VIEWPORT.undo()` / `VIEWPORT.redo()` - proper history stack
- `VIEWPORT.history.mark()` / `VIEWPORT.history.restore(mark)` - bookmarks

---

## Isolate Verb

**When:** User wants to focus on one entity and hide everything else.

**The Gap:** Current `view()` and `filter()` reduce what's shown, but don't do "show ONLY this".

```rust
// Proposed
VIEWPORT.isolate(entity_ref)      // Hide everything except this entity + direct connections
VIEWPORT.isolate.with_depth(2)    // Include 2-hop connections
VIEWPORT.unisolate()              // Restore previous visibility
```

**Use case:** "Just show me this entity" - strips away noise for screenshot or focused analysis.

---

## Named Bookmarks

**When:** User wants to save and return to specific viewport states.

**Current state:** `CbuViewMemory` tracks last view per CBU, but no named saves.

```rust
// Proposed
VIEWPORT.bookmark("AGI ownership view")      // Save current state with name
VIEWPORT.goto_bookmark("AGI ownership view") // Restore named state
VIEWPORT.list_bookmarks()                    // Show available bookmarks
VIEWPORT.delete_bookmark("old view")         // Cleanup
```

**Storage:** Per-CBU, per-user. Persist in `CbuViewMemory` or separate table.

**Voice mapping:**
- "Bookmark this as X" → `VIEWPORT.bookmark("X")`
- "Go back to X" → `VIEWPORT.goto_bookmark("X")`

---

## Agent Delta Narration

**When:** Voice/agent interface needs to confirm what changed.

**The Pattern:** After every viewport command, agent narrates the delta:

```
User: "Focus on AGI Lux, enhance"
Agent executes: VIEWPORT.focus(lei:529900...) | VIEWPORT.enhance(+)
Agent responds: "Focused on Allianz Global Investors Luxembourg. 
                Showing custody and fund accounting links. 
                32 nodes visible, 47 relationships."
```

**What to include in narration:**
- Entity name (resolved)
- What's now visible (relationship types, counts)
- Node/edge counts
- Any filters active
- Suggestions for next action (optional)

**Implementation:**
```rust
struct ViewportDelta {
    focused_entity: Option<String>,
    nodes_added: usize,
    nodes_removed: usize,
    total_visible: usize,
    relationships_shown: Vec<RelationType>,
    active_filters: Vec<String>,
}

impl ViewportDelta {
    fn narrate(&self) -> String {
        // Generate human-readable summary
    }
}
```

**Guardrail:** If agent confidence < 0.8 on entity resolution, ask disambiguation:
- "Allianz Global Investors (Lux) or AGI US?"

---

## SDF Library Triggers

**When:** During CBU implementation, track these pain points:

- [ ] Hit testing on overlapping nodes - SDF distance fields help
- [ ] Confidence halo rendering - SDF glow is cleaner than texture
- [ ] Cluster visualization - SDF blob merging vs convex hull
- [ ] Edge proximity detection - SDF gradient gives direction

**Decision:** If 2+ pain points emerge, implement `rust/crates/sdf/` crate.

---

## Audio Cues (Esper Aesthetic)

**When:** Voice integration is mature and we want full Esper experience.

```rust
fn on_click_step(&self) {
    self.audio.play("esper_click.wav");  // Subtle mechanical click
}
```

**Assets needed:**
- `esper_click.wav` - subtle mechanical click for enhance transitions
- Consider: zoom sounds, focus acquisition tone

---

## Performance Emergency Fixes

**When:** Testing reveals stutter. Checklist of known fixes:

| Symptom | Fix |
|---------|-----|
| DB resolution slow (>200ms) | Add Redis/in-memory cache in entity-gateway |
| Layout too slow | Freeze existing node positions, only layout new nodes |
| Hit test slow (500+ nodes) | Implement R-tree in `spatial.rs` (use `rstar` crate) |
| View switch slow | Cache all 7 view layouts, swap pointers don't recompute |
| Animation jank | Move interpolation to separate thread, atomic read |

---

## 3D / Orbit Navigation

**When:** Graph complexity demands Z-axis for layering or 3D ownership chains.

**Considerations:**
- Orbit camera controls (not just pan/zoom)
- Depth cues for layered structures
- May require WebGPU/wgpu upgrade from egui_glow

---

## Collaborative / Multi-User

**When:** Multiple users need to view same CBU simultaneously.

**Considerations:**
- Viewport state sync
- Cursor presence (who's looking where)
- Conflict resolution on edits

---

## Export Formats

**When:** Users need to share viewport state outside the app.

**Candidates:**
- PNG/SVG snapshot (current view)
- GraphML (structure export)
- PDF report (compliance documentation)
- Shareable link (viewport state in URL)

---

## DSL Entity Locking & Execution Receipts

**When:** Multi-user environment where concurrent DSL execution can cause race conditions.

**The Problem:** TOCTOU (time-of-check vs time-of-use) gap between lint validation and execution.

### Core Principle: Optimistic Where Possible

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    LOCK DECISION MATRIX                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│                          SINGLETON            BATCH / MACRO                 │
│  ─────────────────────────────────────────────────────────────────────────  │
│                                                                             │
│  READ verb              No lock               No lock                       │
│  (VIEWPORT.focus,       (fail = report)       (fail = report)               │
│   CBU.search)                                                               │
│                                                                             │
│  WRITE verb             NO LOCK               LOCK shared UUIDs             │
│  (CBU.add_director,     (optimistic -         (prevent partial              │
│   link operations)       fail = report,        state across                 │
│                          no harm done)         batch)                       │
│                                                                             │
│  DESTRUCTIVE verb       LOCK                  LOCK ALL                      │
│  (PERSON.delete,        (prevent race)        (prevent race)                │
│   CBU.remove)                                                               │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘

KEY INSIGHT: Singleton write failure is FINE - just report it. 
            No partial state, no harm. Locks are expensive - avoid if possible.
```

### Verb Access Mode Configuration

```yaml
# Verb definitions declare their access mode
verbs:
  # READ - never locks
  VIEWPORT.focus:
    params:
      - role: target
        type: any
        access: read

  # WRITE - optimistic singleton, locked in batch
  CBU.add_director:
    params:
      - role: cbu
        type: CBU
        access: write
      - role: person
        type: Person
        access: write

  # EXCLUSIVE - always locks (destructive)
  PERSON.delete:
    params:
      - role: target
        type: Person
        access: exclusive
```

### Batch Shared UUID Detection

```rust
// Batch: person:123 appears in 3 verbs - lock it ONCE
(batch
  (CBU.link cbu:AAA person:123)   // person:123 - write
  (CBU.link cbu:BBB person:123)   // person:123 - write (same UUID)
  (CBU.link cbu:CCC person:123))  // person:123 - write (same UUID)

// Executor collects UNIQUE UUIDs needing locks:
lock_set = {
  cbu:AAA (write),
  cbu:BBB (write),
  cbu:CCC (write),
  person:123 (write)   // ← ONCE, not 3 times
}

// All-or-nothing: acquire ALL locks or FAIL FAST before any execution
```

### Lock Strategy Determination

```rust
fn determine_lock_strategy(
    accesses: &[(EntityRef, AccessMode)],
    is_batch: bool
) -> LockStrategy {
    let has_exclusive = accesses.iter().any(|(_, m)| *m == AccessMode::Exclusive);
    let has_write = accesses.iter().any(|(_, m)| *m == AccessMode::Write);
    
    match (is_batch, has_exclusive, has_write) {
        // Batch with any writes → lock shared UUIDs
        (true, _, true) | (true, true, _) => {
            let refs = deduplicate_write_refs(accesses);
            LockStrategy::LockRequired(refs)
        }
        // Singleton with exclusive → lock
        (false, true, _) => {
            let refs = exclusive_refs(accesses);
            LockStrategy::LockRequired(refs)
        }
        // Singleton with write only → OPTIMISTIC (no lock, fail = report)
        (false, false, true) => LockStrategy::Optimistic,
        // Read only → no locks ever
        (_, false, false) => LockStrategy::None,
    }
}
```

### Pipeline Result Propagation

```
┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐
│  PARSE   │───▶│   LINT   │───▶│  EXPAND  │───▶│  LOCK    │───▶│ EXECUTE  │
│          │    │          │    │  (macro) │    │ (maybe)  │    │          │
└────┬─────┘    └────┬─────┘    └────┬─────┘    └────┬─────┘    └────┬─────┘
     │               │               │               │               │
     ▼               ▼               ▼               ▼               ▼
 ParseResult    LintResult      ExpandResult    LockResult      ExecResult
     │               │               │               │               │
     └───────────────┴───────────────┴───────────────┴───────────────┘
                                     │
                                     ▼
                              ┌─────────────┐
                              │ DslReceipt  │ ← Full trace returned to caller
                              └─────────────┘
```

### DslReceipt Structure

```rust
pub struct DslReceipt {
    pub original_expr: String,
    pub status: BatchStatus,
    pub pipeline_trace: PipelineTrace,
    pub verb_results: Vec<VerbResult>,
    pub mutations: Vec<Mutation>,       // Empty if failed (atomic)
    pub lock_info: Option<LockInfo>,
    pub timing: TimingInfo,
}

pub struct VerbResult {
    pub verb: String,
    pub entity_refs: Vec<EntityRef>,
    pub status: VerbStatus,             // Success | Skipped | Failed
    pub error: Option<VerbError>,
}

pub enum VerbError {
    EntityNotFound {
        entity_ref: EntityRef,
        existed_at_lint: bool,          // TOCTOU detection
        tombstone: Option<TombstoneInfo>,
    },
    EntityLocked {
        entity_ref: EntityRef,
        locked_by: UserId,
        expires_at: DateTime<Utc>,
    },
    ConstraintViolation { reason: String },
}

pub struct TombstoneInfo {
    pub deleted_by: UserId,
    pub deleted_at: DateTime<Utc>,
    pub deleted_by_operation: Option<Uuid>,
}
```

### Example Receipt - Batch Failure

```
DslReceipt {
  original_expr: "(batch (CBU.link cbu:AAA person:123) ...)"
  status: Failed
  
  pipeline_trace:
    parse:  ✓
    lint:   ✓ (person:123 existed at 14:32:00.000)
    expand: ✓ (3 verbs)
    lock:   ✗ FAILED
            acquired: [cbu:AAA, cbu:BBB, cbu:CCC]
            failed: person:123 (not found)
            
  verb_results:
    [0] CBU.link cbu:AAA person:123 - Skipped (batch aborted)
    [1] CBU.link cbu:BBB person:123 - Skipped (batch aborted)  
    [2] CBU.link cbu:CCC person:123 - Skipped (batch aborted)
    
  error: EntityNotFound {
    entity_ref: person:123,
    existed_at_lint: true,              ← TOCTOU: was there, now gone
    tombstone: {
      deleted_by: jane.doe@bny.com,
      deleted_at: 14:32:00.500,         ← 500ms after lint
    }
  }
  
  mutations: []                          ← Nothing changed (atomic)
}
```

### Database Support

```sql
-- Short-lived locks (auto-expire on crash)
CREATE TABLE entity_locks (
    entity_type VARCHAR(50) NOT NULL,
    entity_id UUID NOT NULL,
    locked_by UUID NOT NULL,
    locked_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,    -- 30s default TTL
    operation_id UUID NOT NULL,
    PRIMARY KEY (entity_type, entity_id)
);

-- Tombstones for "what happened to deleted entities"
CREATE TABLE entity_tombstones (
    entity_type VARCHAR(50) NOT NULL,
    entity_id UUID NOT NULL,
    deleted_by UUID NOT NULL,
    deleted_at TIMESTAMPTZ NOT NULL,
    deleted_by_operation UUID,
    PRIMARY KEY (entity_type, entity_id)
);
```

### Deadlock Prevention

```rust
pub struct LockConfig {
    pub default_ttl: Duration,      // 30s - auto-release on crash
    pub max_wait: Duration,         // 5s - fail fast if can't acquire  
    pub acquire_order: LockOrder,   // Sort by UUID - prevents deadlock
}

// Everyone acquires in same order → no circular wait
let sorted_refs = refs.sort_by(|a, b| a.uuid.cmp(&b.uuid));
for ref in sorted_refs {
    acquire_or_fail(ref)?;
}
```

### User Feedback

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ ✗ Batch operation failed                                                    │
│                                                                             │
│ Person "John Smith" (person:123) was deleted by jane.doe@bny.com            │
│ 500ms after your request was validated.                                     │
│                                                                             │
│ No changes were made to CBU AAA, BBB, or CCC.                               │
│                                                                             │
│ [Retry without John Smith]  [View audit log]  [Cancel]                      │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

*Add new sections as ideas emerge. Don't implement until pain is felt.*
