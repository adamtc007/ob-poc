# MISSING SPEC: Esper DSL Navigation Verbs

## Context

The UI has a rich Esper navigation vocabulary (Blade Runner-inspired) defined in `rust/crates/ob-poc-ui/src/command.rs` as `NavigationVerb`. However, these commands have **no DSL backend** - they are empty TODO stubs in `app.rs`.

## What Exists Today

### Working DSL Verbs (in `rust/config/verbs/view.yaml`)
```yaml
view.universe      # Show all CBUs clustered by dimension
view.book          # Focus on a specific client's book
view.cbu           # Focus on a specific CBU
view.zoom-in       # Zoom into current focus
view.zoom-out      # Zoom out from current focus
view.back-to       # Navigate back to specific breadcrumb level
```

### NavigationVerb Enum (UI vocabulary, no backend)
```rust
// Scale Navigation (Astronomical Metaphor)
ScaleUniverse,                              // → view.universe ✅
ScaleBook { client_name: String },          // → view.book ✅
ScaleGalaxy { segment: Option<String> },    // → view.universe with filter ✅
ScaleSystem { cbu_id: Option<String> },     // → view.cbu ✅
ScalePlanet { entity_id: Option<String> },  // → NEEDS: view.entity
ScaleSurface,                               // → NEEDS: view.attributes
ScaleCore,                                  // → NEEDS: view.raw

// Depth Navigation (Z-axis through entity structures)
DrillThrough,                               // → NEEDS: view.drill
SurfaceReturn,                              // → NEEDS: view.surface
Xray,                                       // → NEEDS: view.xray
Peel,                                       // → NEEDS: view.peel
CrossSection,                               // → NEEDS: view.cross-section
DepthIndicator,                             // → NEEDS: view.depth-indicator

// Orbital Navigation
Orbit { entity_id: Option<String> },        // → NEEDS: view.orbit
RotateLayer { layer: String },              // → NEEDS: view.rotate-layer
Flip,                                       // → NEEDS: view.flip
Tilt { dimension: String },                 // → NEEDS: view.tilt

// Temporal Navigation
TimeRewind { target_date: Option<String> }, // → NEEDS: view.time-rewind
TimePlay { from, to },                      // → NEEDS: view.time-play
TimeFreeze,                                 // → NEEDS: view.time-freeze
TimeSlice { date1, date2 },                 // → NEEDS: view.time-slice
TimeTrail { entity_id },                    // → NEEDS: view.time-trail

// Investigation Patterns
FollowRabbit { from_entity },               // → NEEDS: view.follow-rabbit
DiveInto { entity_id },                     // → NEEDS: view.dive
WhoControls { entity_id },                  // → NEEDS: view.who-controls
FollowMoney { from_entity },                // → NEEDS: view.follow-money
Illuminate { aspect },                      // → NEEDS: view.illuminate
Shadow,                                     // → NEEDS: view.shadow
RedFlagScan,                                // → NEEDS: view.red-flag-scan
BlackHole,                                  // → NEEDS: view.black-hole

// Context Modes
SetContext { mode },                        // → NEEDS: view.set-context
```

---

## SPEC NEEDED: Define Each Verb

For each verb below, please specify:
1. **Purpose**: What does this navigation do?
2. **Parameters**: What arguments does it take?
3. **ViewState Effect**: How does it modify the session's ViewState?
4. **Graph Effect**: What happens to the graph visualization?
5. **Example DSL**: Example usage

---

### view.entity
**Purpose**: Focus on a specific entity within the current CBU graph.

**Parameters**:
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| entity-id | uuid | yes | The entity to focus on |
| depth | integer | no | How many hops of relationships to show (default: 2) |

**ViewState Effect**: 
- Sets `focus_entity_id` to the target entity
- Updates `visible_depth` to the specified depth
- Clears any filters that would hide this entity

**Graph Effect**:
- Centers camera on entity
- Highlights entity and its immediate relationships
- Fades distant entities

**Example DSL**:
```clojure
(view.entity :entity-id "550e8400-e29b-41d4-a716-446655440000" :depth 3)
```

---

### view.attributes
**Purpose**: Zoom to attribute/detail level for the currently focused entity.

**Parameters**:
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| entity-id | uuid | no | Entity to show attributes for (defaults to current focus) |

**ViewState Effect**:
- Sets `detail_level` to `ATTRIBUTES`
- Enables attribute panel visibility

**Graph Effect**:
- Expands entity node to show attribute cards
- Hides relationship labels to reduce clutter

**Example DSL**:
```clojure
(view.attributes)
(view.attributes :entity-id "...")
```

---

### view.raw
**Purpose**: Show raw JSON/data view for debugging or deep inspection.

**Parameters**:
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| entity-id | uuid | no | Entity to show raw data for |

**ViewState Effect**:
- Sets `detail_level` to `RAW`

**Graph Effect**:
- Replaces visual graph with JSON tree view

**Example DSL**:
```clojure
(view.raw)
```

---

### view.drill
**Purpose**: Drill through the current entity to show its subsidiary/ownership structure.

**Parameters**:
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| entity-id | uuid | no | Entity to drill into (defaults to current focus) |
| direction | string | no | "down" (subsidiaries) or "up" (parents). Default: "down" |

**ViewState Effect**:
- Pushes current view onto navigation stack
- Sets new root to the drilled entity's children/parents

**Graph Effect**:
- Animates transition to show next level of hierarchy
- Previous level fades to background

**Example DSL**:
```clojure
(view.drill :entity-id "..." :direction "down")
```

---

### view.surface
**Purpose**: Return to the top-level view from a drilled position.

**Parameters**: None

**ViewState Effect**:
- Pops navigation stack back to root
- Clears drill depth

**Graph Effect**:
- Animates zoom out to full CBU view

**Example DSL**:
```clojure
(view.surface)
```

---

### view.xray
**Purpose**: Toggle X-ray/transparency mode to see through entity layers.

**Parameters**:
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| enabled | boolean | no | Toggle state (default: toggle current) |
| layers | string[] | no | Which layers to make transparent |

**ViewState Effect**:
- Sets `xray_mode` to enabled/disabled
- Records which layers are transparent

**Graph Effect**:
- Outer entity shells become semi-transparent
- Inner structures (UBOs, control chains) become visible

**Example DSL**:
```clojure
(view.xray)
(view.xray :enabled true :layers ["shell" "services"])
```

---

### view.peel
**Purpose**: Remove the outermost layer to reveal the next level of structure.

**Parameters**:
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| layer | string | no | Specific layer to peel (default: outermost) |

**ViewState Effect**:
- Increments `peel_depth`
- Records peeled layer

**Graph Effect**:
- Outermost entity wrapper animates away
- Inner structure expands to fill space

**Example DSL**:
```clojure
(view.peel)
(view.peel :layer "fund-structure")
```

---

### view.cross-section
**Purpose**: Show a cross-section view cutting through the entity structure.

**Parameters**:
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| axis | string | no | "horizontal", "vertical", "ownership", "control" |
| position | float | no | Where to cut (0.0-1.0) |

**ViewState Effect**:
- Sets `cross_section_axis` and `cross_section_position`

**Graph Effect**:
- Entities on one side of the cut become transparent
- Cut surface shows edge relationships

**Example DSL**:
```clojure
(view.cross-section :axis "ownership" :position 0.5)
```

---

### view.depth-indicator
**Purpose**: Toggle depth indicator overlay showing hierarchical levels.

**Parameters**:
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| enabled | boolean | no | Toggle state |

**ViewState Effect**:
- Sets `show_depth_indicator`

**Graph Effect**:
- Adds colored bands or rings showing depth levels
- Numbers indicate ownership chain depth

**Example DSL**:
```clojure
(view.depth-indicator)
```

---

### view.orbit
**Purpose**: Enter orbital navigation mode around a target entity.

**Parameters**:
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| entity-id | uuid | no | Center of orbit (default: current focus) |
| speed | float | no | Orbit speed (default: 1.0) |

**ViewState Effect**:
- Sets `orbit_center` and `orbit_active`

**Graph Effect**:
- Camera begins rotating around the entity
- Related entities stay in view as camera moves

**Example DSL**:
```clojure
(view.orbit :entity-id "...")
```

---

### view.rotate-layer
**Purpose**: Rotate a specific layer (ownership, services, etc.) to a different angle.

**Parameters**:
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| layer | string | yes | Layer to rotate |
| angle | float | no | Rotation angle in degrees |

**ViewState Effect**:
- Sets `layer_rotations[layer]` to angle

**Graph Effect**:
- The specified layer rotates independently
- Creates parallax effect revealing hidden relationships

**Example DSL**:
```clojure
(view.rotate-layer :layer "ownership" :angle 45)
```

---

### view.flip
**Purpose**: Flip the perspective between top-down and bottom-up views.

**Parameters**: None

**ViewState Effect**:
- Toggles `perspective_flipped`

**Graph Effect**:
- UBOs move from top to bottom (or vice versa)
- Ownership arrows reverse direction

**Example DSL**:
```clojure
(view.flip)
```

---

### view.tilt
**Purpose**: Tilt the view to emphasize a specific dimension.

**Parameters**:
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| dimension | string | yes | "ownership", "control", "services", "time", "risk" |
| angle | float | no | Tilt angle (default: 30 degrees) |

**ViewState Effect**:
- Sets `tilt_dimension` and `tilt_angle`

**Graph Effect**:
- Graph tilts to emphasize the chosen dimension
- Related edges become more prominent

**Example DSL**:
```clojure
(view.tilt :dimension "ownership")
```

---

### view.time-rewind
**Purpose**: Rewind the view to show historical state.

**Parameters**:
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| target-date | date | no | Date to rewind to (default: previous snapshot) |

**ViewState Effect**:
- Sets `as_of_date` to target date
- Loads historical snapshot

**Graph Effect**:
- Entities show their historical state
- Changes since that date are dimmed or hidden

**Example DSL**:
```clojure
(view.time-rewind :target-date "2024-01-01")
```

---

### view.time-play
**Purpose**: Animate changes over a time period.

**Parameters**:
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| from | date | no | Start date |
| to | date | no | End date (default: now) |
| speed | float | no | Playback speed multiplier |

**ViewState Effect**:
- Sets `time_animation_active`, `time_from`, `time_to`

**Graph Effect**:
- Entities animate in/out as they're created/deleted
- Relationships morph as ownership changes

**Example DSL**:
```clojure
(view.time-play :from "2023-01-01" :to "2024-01-01")
```

---

### view.time-freeze
**Purpose**: Pause temporal animation.

**Parameters**: None

**ViewState Effect**:
- Sets `time_animation_active` to false

**Graph Effect**:
- Animation pauses at current frame

**Example DSL**:
```clojure
(view.time-freeze)
```

---

### view.time-slice
**Purpose**: Show a diff between two points in time.

**Parameters**:
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| date1 | date | yes | First snapshot date |
| date2 | date | yes | Second snapshot date |

**ViewState Effect**:
- Sets `time_slice_dates`
- Loads both snapshots for comparison

**Graph Effect**:
- Added entities shown in green
- Removed entities shown in red
- Changed entities shown in yellow

**Example DSL**:
```clojure
(view.time-slice :date1 "2023-01-01" :date2 "2024-01-01")
```

---

### view.time-trail
**Purpose**: Show an entity's history as a trail/timeline.

**Parameters**:
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| entity-id | uuid | no | Entity to trace (default: current focus) |

**ViewState Effect**:
- Sets `time_trail_entity`

**Graph Effect**:
- Entity shows timeline of changes
- Connected entities show their relationship history

**Example DSL**:
```clojure
(view.time-trail :entity-id "...")
```

---

### view.follow-money
**Purpose**: Trace financial flows from an entity.

**Parameters**:
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| from-entity | uuid | no | Starting entity (default: current focus) |
| depth | integer | no | How many hops to trace (default: 5) |

**ViewState Effect**:
- Sets `trace_mode` to "money"
- Records trace path

**Graph Effect**:
- Financial relationship edges highlight
- Flow direction arrows animate
- Non-financial edges fade

**Example DSL**:
```clojure
(view.follow-money :from-entity "..." :depth 10)
```

---

### view.follow-rabbit
**Purpose**: Follow an investigative thread (like "follow the white rabbit").

**Parameters**:
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| from-entity | uuid | no | Starting entity |
| thread | string | no | Type of thread: "risk", "control", "documents", "alerts" |

**ViewState Effect**:
- Sets `trace_mode` to specified thread

**Graph Effect**:
- Highlights entities matching the thread criteria
- Creates a path visualization

**Example DSL**:
```clojure
(view.follow-rabbit :from-entity "..." :thread "risk")
```

---

### view.dive
**Purpose**: Deep dive into an entity's full context.

**Parameters**:
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| entity-id | uuid | yes | Entity to dive into |

**ViewState Effect**:
- Sets focus to entity
- Expands all related panels
- Loads full entity data

**Graph Effect**:
- Entity expands to show all details
- Related entities arrange in constellation around it

**Example DSL**:
```clojure
(view.dive :entity-id "...")
```

---

### view.who-controls
**Purpose**: Highlight all control relationships leading to an entity.

**Parameters**:
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| entity-id | uuid | no | Target entity (default: current focus) |

**ViewState Effect**:
- Sets `highlight_mode` to "control"
- Records control chain entities

**Graph Effect**:
- Control relationship edges turn bold/colored
- Controlling entities highlight
- Non-control relationships fade

**Example DSL**:
```clojure
(view.who-controls :entity-id "...")
```

---

### view.illuminate
**Purpose**: Highlight a specific aspect across all visible entities.

**Parameters**:
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| aspect | string | yes | What to illuminate: "risks", "documents", "screenings", "gaps", "pending" |

**ViewState Effect**:
- Sets `illuminate_aspect`

**Graph Effect**:
- Entities with the aspect glow/highlight
- Entities without fade
- Counts appear on highlighted entities

**Example DSL**:
```clojure
(view.illuminate :aspect "risks")
(view.illuminate :aspect "pending")
```

---

### view.shadow
**Purpose**: Dim everything except high-risk or flagged items.

**Parameters**:
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| threshold | string | no | Risk level threshold: "high", "medium", "any" |

**ViewState Effect**:
- Sets `shadow_mode` to true
- Sets `shadow_threshold`

**Graph Effect**:
- Normal entities become very dim/gray
- Flagged/risky entities remain bright
- Risk indicators pulse

**Example DSL**:
```clojure
(view.shadow)
(view.shadow :threshold "high")
```

---

### view.red-flag-scan
**Purpose**: Highlight all red flags and risk indicators.

**Parameters**:
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| category | string | no | Filter by category: "pep", "sanctions", "adverse-media", "all" |

**ViewState Effect**:
- Sets `red_flag_scan_active`
- Sets category filter

**Graph Effect**:
- Red flag icons appear on affected entities
- Severity color coding
- Count badges

**Example DSL**:
```clojure
(view.red-flag-scan)
(view.red-flag-scan :category "sanctions")
```

---

### view.black-hole
**Purpose**: Highlight missing data and incomplete ownership chains.

**Parameters**:
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| type | string | no | Gap type: "ownership", "documents", "screening", "all" |

**ViewState Effect**:
- Sets `black_hole_mode` to true
- Sets gap type filter

**Graph Effect**:
- Missing data shown as dark voids
- Incomplete chains show broken links
- Gap counts displayed

**Example DSL**:
```clojure
(view.black-hole)
(view.black-hole :type "ownership")
```

---

### view.set-context
**Purpose**: Set the UI context mode for different workflows.

**Parameters**:
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| mode | string | yes | Context mode |

**Valid Modes**:
- `review` - Read-only, summary focus, approval workflow
- `investigation` - Forensic tools, full detail, trace capabilities
- `onboarding` - Workflow-driven, progress focus, checklists
- `monitoring` - Alerts, changes, flags, dashboards
- `remediation` - Issues, resolutions, action items

**ViewState Effect**:
- Sets `context_mode`
- Adjusts available tools/panels

**Graph Effect**:
- UI chrome changes to match mode
- Relevant overlays appear

**Example DSL**:
```clojure
(view.set-context :mode "investigation")
```

---

## Implementation Priority

### Phase 1 (Core Navigation)
1. `view.entity` - Focus on entity
2. `view.drill` - Drill into hierarchy
3. `view.surface` - Return to top
4. `view.who-controls` - Control chain highlight

### Phase 2 (Investigation)
5. `view.follow-money` - Financial tracing
6. `view.illuminate` - Aspect highlighting
7. `view.red-flag-scan` - Risk scanning
8. `view.black-hole` - Gap detection

### Phase 3 (Temporal)
9. `view.time-rewind` - Historical view
10. `view.time-slice` - Point-in-time comparison
11. `view.time-trail` - Entity timeline

### Phase 4 (Advanced Visualization)
12. `view.xray` - Transparency mode
13. `view.peel` - Layer removal
14. `view.orbit` - Orbital navigation
15. `view.tilt` - Dimension emphasis

### Phase 5 (Context & Polish)
16. `view.set-context` - Mode switching
17. `view.attributes` - Detail view
18. `view.cross-section` - Cut views
19. `view.flip` - Perspective flip

---

## Notes for Implementation

1. **All verbs should update ViewState** - The session's ViewState is the single source of truth
2. **Graph effects are client-side** - The DSL returns ViewState, client interprets for rendering
3. **Idempotent operations** - Running same verb twice should be safe
4. **Natural language mapping** - Voice commands should map cleanly to these verbs

---

## Files to Modify

1. `rust/config/verbs/view.yaml` - Add new verb definitions
2. `rust/src/dsl_v2/custom_ops/view_ops.rs` - Implement handlers
3. `rust/src/session/view_state.rs` - Extend ViewState struct
4. `rust/crates/ob-poc-ui/src/app.rs` - Wire up to execute DSL instead of TODO stubs
