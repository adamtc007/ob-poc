//! Derivation lineage — edges, run records, forward/reverse graph traversal.
//!
//! Every time a derived attribute is computed, a `DerivationEdge` records which
//! input snapshots produced which output snapshot.  A `RunRecord` captures the
//! execution context (plan step, verb, timing).
//!
//! Queries:
//!   - `query_forward_impact()`: "if this snapshot changes, what is affected?"
//!   - `query_reverse_provenance()`: "where did this value come from?"

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "database")]
use anyhow::Result;
#[cfg(feature = "database")]
use sqlx::PgPool;

// ── Types ────────────────────────────────────────────────────────────────────

/// A node in the lineage graph returned by traversal queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LineageNode {
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

// ── Store ────────────────────────────────────────────────────────────────────

pub(crate) struct LineageStore;

impl LineageStore {
    // ── Read ─────────────────────────────────────────────────────────────

    /// Query forward impact: "if snapshot_id changes, what is affected?"
    /// BFS traversal following output→input edges up to `max_depth`.
    #[cfg(feature = "database")]
    pub(crate) async fn query_forward_impact(
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
    pub(crate) async fn query_reverse_provenance(
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

}

#[cfg(test)]
mod tests {
    use super::*;

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
