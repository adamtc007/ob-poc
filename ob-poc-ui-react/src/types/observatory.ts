/**
 * Observatory Types
 *
 * TypeScript types matching the Rust types in ob-poc-types/src/orientation.rs
 * and ob-poc-types/src/graph_scene.rs.
 */

// ============================================================================
// OrientationContract (matches ob-poc-types/src/orientation.rs)
// ============================================================================

export interface OrientationContract {
  session_mode: string;
  view_level: ViewLevel;
  focus_kind: string;
  focus_identity: FocusIdentity;
  scope: string;
  lens: LensState;
  entry_reason: EntryReason;
  available_actions: ActionDescriptor[];
  delta_from_previous?: OrientationDelta;
  computed_at: string;
}

export type ViewLevel =
  | "universe"
  | "cluster"
  | "system"
  | "planet"
  | "surface"
  | "core";

export interface FocusIdentity {
  canonical_id: string;
  business_label: string;
  object_type?: string;
}

export interface LensState {
  overlay: OverlayState;
  depth_probe?: string;
  cluster_mode: string;
  active_filters: FilterExpression[];
}

export type OverlayState =
  | { mode: "active_only" }
  | { mode: "draft_overlay"; changeset_id: string };

export interface FilterExpression {
  field: string;
  operator: string;
  value: unknown;
}

export type EntryReason =
  | { kind: "direct_navigation" }
  | { kind: "suggestion_accepted"; suggestion_id: string }
  | { kind: "drill_down"; from_level: ViewLevel; from_id: string }
  | { kind: "workflow_step"; step_name: string }
  | { kind: "search_result"; query: string }
  | { kind: "deep_link"; uri: string }
  | { kind: "session_start" }
  | { kind: "history_replay"; direction: string };

export interface ActionDescriptor {
  action_id: string;
  label: string;
  action_kind: string;
  enabled: boolean;
  disabled_reason?: string;
  rank_score: number;
}

export interface OrientationDelta {
  mode_changed?: { from: string; to: string };
  level_changed?: { from: ViewLevel; to: ViewLevel };
  focus_changed?: {
    from_kind: string;
    to_kind: string;
    from_label: string;
    to_label: string;
  };
  lens_changed?: {
    overlay_changed: boolean;
    depth_changed: boolean;
    cluster_changed: boolean;
    filters_changed: boolean;
  };
  scope_changed: boolean;
  actions_added: number;
  actions_removed: number;
  summary: string;
}

// ============================================================================
// GraphSceneModel (matches ob-poc-types/src/graph_scene.rs)
// ============================================================================

export interface GraphSceneModel {
  generation: number;
  level: ViewLevel;
  layout_strategy: string;
  nodes: SceneNode[];
  edges: SceneEdge[];
  groups: SceneGroup[];
  drill_targets: DrillTarget[];
  max_depth: number;
}

export interface SceneNode {
  id: string;
  label: string;
  node_type: string;
  state?: string;
  progress: number;
  blocking: boolean;
  depth: number;
  position_hint?: [number, number];
  badges: SceneBadge[];
  child_count: number;
  group_id?: string;
}

export interface SceneEdge {
  source: string;
  target: string;
  edge_type: string;
  label?: string;
  weight: number;
}

export interface SceneGroup {
  id: string;
  label: string;
  node_ids: string[];
  collapsed: boolean;
  boundary_hint?: { center_x: number; center_y: number; radius: number };
}

export interface DrillTarget {
  node_id: string;
  target_level: ViewLevel;
  drill_label: string;
}

export interface SceneBadge {
  badge_type: string;
  label: string;
  color?: string;
}

// ============================================================================
// Health metrics
// ============================================================================

export interface HealthMetrics {
  pending_changesets: number;
  stale_dryruns: number;
  active_snapshots: number;
  archived_changesets: number;
  embedding_freshness_hours?: number;
  outbox_depth?: number;
}

// ============================================================================
// ObservatoryAction (from egui canvas callbacks)
// ============================================================================

export type ObservatoryAction =
  | { type: "drill"; node_id: string; target_level: ViewLevel }
  | { type: "semantic_zoom_out" }
  | { type: "navigate_history"; direction: "back" | "forward" }
  | { type: "invoke_verb"; verb_fqn: string }
  | { type: "visual_zoom"; delta: number }
  | { type: "pan"; dx: number; dy: number }
  | { type: "select_node"; node_id: string }
  | { type: "deselect_node" }
  | { type: "anchor_node"; node_id: string }
  | { type: "clear_anchor" }
  | { type: "reset_view" };
