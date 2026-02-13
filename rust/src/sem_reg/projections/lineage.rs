//! Derivation lineage — edges, run records, forward/reverse graph traversal.
//!
//! Every time a derived attribute is computed, a `DerivationEdge` records which
//! input snapshots produced which output snapshot.  A `RunRecord` captures the
//! execution context (plan step, verb, timing).
//!
//! Queries:
//!   - `query_forward_impact()`: "if this snapshot changes, what is affected?"
//!   - `query_reverse_provenance()`: "where did this value come from?"

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "database")]
use anyhow::Result;
#[cfg(feature = "database")]
use sqlx::PgPool;

// ── Types ────────────────────────────────────────────────────────────────────

/// Direction for lineage traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineageDirection {
    /// Forward impact: "if this changes, what is affected?"
    Forward,
    /// Reverse provenance: "where did this value come from?"
    Reverse,
}

/// An immutable record linking input snapshots to an output snapshot through a
/// verb execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivationEdge {
    pub edge_id: Uuid,
    /// Snapshots consumed as inputs.
    pub input_snapshot_ids: Vec<Uuid>,
    /// Snapshot produced as output.
    pub output_snapshot_id: Uuid,
    /// The verb that produced the derivation.
    pub verb_fqn: String,
    /// Optional link to a `RunRecord` for richer context.
    pub run_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

/// A run record captures execution context for a batch of derivation edges.
/// Immutable — one record per plan-step execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub run_id: Uuid,
    pub plan_id: Option<Uuid>,
    pub step_id: Option<Uuid>,
    pub verb_fqn: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub duration_ms: i64,
    pub input_count: i32,
    pub output_count: i32,
    /// Free-form metadata (e.g., parameters, errors).
    pub metadata: serde_json::Value,
}

/// A node in the lineage graph returned by traversal queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineageNode {
    pub snapshot_id: Uuid,
    pub object_type: String,
    pub object_id: Uuid,
    pub depth: i32,
    pub via_verb: Option<String>,
    pub via_edge_id: Option<Uuid>,
}

// ── Row types (for clippy::type_complexity) ──────────────────────────────────

#[cfg(feature = "database")]
type LineageRow = (Uuid, String, Uuid, i32, Option<String>, Option<Uuid>);

#[cfg(feature = "database")]
type EdgeRow = (Uuid, Vec<Uuid>, Uuid, String, Option<Uuid>, DateTime<Utc>);

// ── Store ────────────────────────────────────────────────────────────────────

pub struct LineageStore;

impl LineageStore {
    // ── Write ────────────────────────────────────────────────────────────

    /// Record a derivation edge (append-only).
    #[cfg(feature = "database")]
    pub async fn record_derivation_edge(
        pool: &PgPool,
        input_snapshot_ids: &[Uuid],
        output_snapshot_id: Uuid,
        verb_fqn: &str,
        run_id: Option<Uuid>,
    ) -> Result<Uuid> {
        let edge_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO sem_reg.derivation_edges
                (edge_id, input_snapshot_ids, output_snapshot_id, verb_fqn, run_id)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(edge_id)
        .bind(input_snapshot_ids)
        .bind(output_snapshot_id)
        .bind(verb_fqn)
        .bind(run_id)
        .execute(pool)
        .await?;
        Ok(edge_id)
    }

    /// Record a run (append-only).
    #[cfg(feature = "database")]
    #[allow(clippy::too_many_arguments)]
    pub async fn record_run(
        pool: &PgPool,
        plan_id: Option<Uuid>,
        step_id: Option<Uuid>,
        verb_fqn: &str,
        started_at: DateTime<Utc>,
        completed_at: DateTime<Utc>,
        input_count: i32,
        output_count: i32,
        metadata: &serde_json::Value,
    ) -> Result<Uuid> {
        let run_id = Uuid::new_v4();
        let duration_ms = (completed_at - started_at).num_milliseconds();
        sqlx::query(
            r#"
            INSERT INTO sem_reg.run_records
                (run_id, plan_id, step_id, verb_fqn,
                 started_at, completed_at, duration_ms,
                 input_count, output_count, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(run_id)
        .bind(plan_id)
        .bind(step_id)
        .bind(verb_fqn)
        .bind(started_at)
        .bind(completed_at)
        .bind(duration_ms)
        .bind(input_count)
        .bind(output_count)
        .bind(metadata)
        .execute(pool)
        .await?;
        Ok(run_id)
    }

    // ── Read ─────────────────────────────────────────────────────────────

    /// Query forward impact: "if snapshot_id changes, what is affected?"
    /// BFS traversal following output→input edges up to `max_depth`.
    #[cfg(feature = "database")]
    pub async fn query_forward_impact(
        pool: &PgPool,
        snapshot_id: Uuid,
        max_depth: i32,
    ) -> Result<Vec<LineageNode>> {
        let rows: Vec<LineageRow> = sqlx::query_as(
            r#"
            WITH RECURSIVE lineage AS (
                -- Seed: edges where the given snapshot is an input
                SELECT
                    e.output_snapshot_id AS snapshot_id,
                    s.object_type::text,
                    s.object_id,
                    1 AS depth,
                    e.verb_fqn,
                    e.edge_id
                FROM sem_reg.derivation_edges e
                JOIN sem_reg.snapshots s ON s.snapshot_id = e.output_snapshot_id
                WHERE $1 = ANY(e.input_snapshot_ids)

                UNION ALL

                -- Recurse: follow forward edges
                SELECT
                    e2.output_snapshot_id,
                    s2.object_type::text,
                    s2.object_id,
                    l.depth + 1,
                    e2.verb_fqn,
                    e2.edge_id
                FROM lineage l
                JOIN sem_reg.derivation_edges e2
                    ON l.snapshot_id = ANY(e2.input_snapshot_ids)
                JOIN sem_reg.snapshots s2 ON s2.snapshot_id = e2.output_snapshot_id
                WHERE l.depth < $2
            )
            SELECT DISTINCT ON (snapshot_id)
                snapshot_id, object_type, object_id, depth, verb_fqn, edge_id
            FROM lineage
            ORDER BY snapshot_id, depth
            "#,
        )
        .bind(snapshot_id)
        .bind(max_depth)
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(sid, ot, oid, d, v, e)| LineageNode {
                snapshot_id: sid,
                object_type: ot,
                object_id: oid,
                depth: d,
                via_verb: v,
                via_edge_id: e,
            })
            .collect())
    }

    /// Query reverse provenance: "where did this value come from?"
    /// BFS traversal following input←output edges up to `max_depth`.
    #[cfg(feature = "database")]
    pub async fn query_reverse_provenance(
        pool: &PgPool,
        snapshot_id: Uuid,
        max_depth: i32,
    ) -> Result<Vec<LineageNode>> {
        let rows: Vec<LineageRow> = sqlx::query_as(
            r#"
            WITH RECURSIVE lineage AS (
                -- Seed: edges where the given snapshot is an output
                SELECT
                    unnest(e.input_snapshot_ids) AS snapshot_id,
                    1 AS depth,
                    e.verb_fqn,
                    e.edge_id
                FROM sem_reg.derivation_edges e
                WHERE e.output_snapshot_id = $1

                UNION ALL

                -- Recurse: follow reverse edges
                SELECT
                    unnest(e2.input_snapshot_ids),
                    l.depth + 1,
                    e2.verb_fqn,
                    e2.edge_id
                FROM lineage l
                JOIN sem_reg.derivation_edges e2
                    ON e2.output_snapshot_id = l.snapshot_id
                WHERE l.depth < $2
            )
            SELECT DISTINCT ON (l.snapshot_id)
                l.snapshot_id,
                s.object_type::text,
                s.object_id,
                l.depth,
                l.verb_fqn,
                l.edge_id
            FROM lineage l
            JOIN sem_reg.snapshots s ON s.snapshot_id = l.snapshot_id
            ORDER BY l.snapshot_id, l.depth
            "#,
        )
        .bind(snapshot_id)
        .bind(max_depth)
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(sid, ot, oid, d, v, e)| LineageNode {
                snapshot_id: sid,
                object_type: ot,
                object_id: oid,
                depth: d,
                via_verb: v,
                via_edge_id: e,
            })
            .collect())
    }

    /// Get edges for a specific run.
    #[cfg(feature = "database")]
    pub async fn edges_for_run(pool: &PgPool, run_id: Uuid) -> Result<Vec<DerivationEdge>> {
        let rows: Vec<EdgeRow> = sqlx::query_as(
            r#"
                SELECT edge_id, input_snapshot_ids, output_snapshot_id, verb_fqn, run_id, created_at
                FROM sem_reg.derivation_edges
                WHERE run_id = $1
                ORDER BY created_at
                "#,
        )
        .bind(run_id)
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(eid, inputs, output, verb, rid, created)| DerivationEdge {
                edge_id: eid,
                input_snapshot_ids: inputs,
                output_snapshot_id: output,
                verb_fqn: verb,
                run_id: rid,
                created_at: created,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lineage_direction_serde() {
        let json = serde_json::to_string(&LineageDirection::Forward).unwrap();
        assert_eq!(json, r#""forward""#);
        let back: LineageDirection = serde_json::from_str(r#""reverse""#).unwrap();
        assert_eq!(back, LineageDirection::Reverse);
    }

    #[test]
    fn test_derivation_edge_roundtrip() {
        let edge = DerivationEdge {
            edge_id: Uuid::nil(),
            input_snapshot_ids: vec![Uuid::nil()],
            output_snapshot_id: Uuid::nil(),
            verb_fqn: "attr.derive-composite".into(),
            run_id: None,
            created_at: Utc::now(),
        };
        let json = serde_json::to_value(&edge).unwrap();
        assert_eq!(json["verb_fqn"], "attr.derive-composite");
    }

    #[test]
    fn test_run_record_roundtrip() {
        let now = Utc::now();
        let run = RunRecord {
            run_id: Uuid::nil(),
            plan_id: None,
            step_id: None,
            verb_fqn: "ubo.compute".into(),
            started_at: now,
            completed_at: now,
            duration_ms: 42,
            input_count: 3,
            output_count: 1,
            metadata: serde_json::json!({"threshold": 25}),
        };
        let json = serde_json::to_value(&run).unwrap();
        assert_eq!(json["input_count"], 3);
        assert_eq!(json["duration_ms"], 42);
    }

    #[test]
    fn test_lineage_node_serde() {
        let node = LineageNode {
            snapshot_id: Uuid::nil(),
            object_type: "attribute_def".into(),
            object_id: Uuid::nil(),
            depth: 2,
            via_verb: Some("attr.derive-composite".into()),
            via_edge_id: Some(Uuid::nil()),
        };
        let json = serde_json::to_value(&node).unwrap();
        assert_eq!(json["depth"], 2);
        assert!(json["via_verb"].is_string());
    }
}
