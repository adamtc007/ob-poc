/**
 * Inspector Projection Types
 *
 * These types match the InspectorProjection JSON schema from the Rust backend.
 * See INSPECTOR_FIRST_VISUALIZATION_SPEC_v3_1.md for full specification.
 */

/** Reference to another node in the projection */
export interface NodeRef {
  $ref: string;
}

/** Check if a value is a NodeRef */
export function isNodeRef(value: unknown): value is NodeRef {
  return typeof value === "object" && value !== null && "$ref" in value;
}

/** Level of Detail tiers */
export type LodTier = 0 | 1 | 2 | 3;

/** Policy configuration for projection generation */
export interface ProjectionPolicy {
  lod: LodTier;
  max_depth: number;
  chambers: string[];
  filters?: Record<string, unknown>;
}

/** Node metadata */
export interface NodeMeta {
  chamber: string;
  entity_type?: string;
  lod_generated: LodTier;
  truncated?: boolean;
  child_count?: number;
  total_descendants?: number;
}

/** Primitive field values */
export type PrimitiveValue = string | number | boolean | null;

/** A single field value - can be primitive, ref, array, or object */
export type FieldValue = PrimitiveValue | NodeRef | FieldValue[] | FieldObject;

/** Object with field values */
export interface FieldObject {
  [key: string]: FieldValue;
}

/** A node in the projection tree */
export interface ProjectionNode {
  id: string;
  kind: string;
  label: string;
  label_full?: string;
  meta: NodeMeta;
  fields: Record<string, FieldValue>;
  children?: ProjectionNode[];
}

/** Table cell */
export interface TableCell {
  value: FieldValue;
  style?: "default" | "header" | "highlight" | "warning" | "error";
}

/** Table row */
export interface TableRow {
  id: string;
  cells: TableCell[];
}

/** Table structure for tabular data */
export interface TableData {
  columns: string[];
  rows: TableRow[];
  sortable?: boolean;
  paginated?: boolean;
}

/** The complete projection response */
export interface InspectorProjection {
  version: string;
  generated_at: string;
  policy: ProjectionPolicy;
  root: ProjectionNode;
  tables?: Record<string, TableData>;
}

/** Summary of a node for listing */
export interface NodeSummary {
  id: string;
  kind: string;
  label: string;
  label_full?: string;
  child_count: number;
}

/** Pagination info */
export interface PagingInfo {
  offset: number;
  limit: number;
  total: number;
  has_more: boolean;
}

/** Paginated children response */
export interface ChildrenResponse {
  children: NodeSummary[];
  paging: PagingInfo;
}

/** Projection list item */
export interface ProjectionListItem {
  id: string;
  snapshot_id: string;
  created_at: string;
  policy: ProjectionPolicy;
  node_count: number;
}

/** Validation result */
export interface ValidationResult {
  valid: boolean;
  errors: string[];
  warnings: string[];
}
