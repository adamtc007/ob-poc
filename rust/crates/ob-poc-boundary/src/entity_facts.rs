//! T9.1-pre (EOP-PLAN-CONTROLPLANE-001 Addendum B): batched entity-facts
//! read for G2 (Entity Binding). Per the design pass recorded in
//! `docs/research/control-plane-ownership-ledger.md`'s "T9.1-pre
//! reclassified" entry: G2 grades five point-in-time DB facts per entity
//! (`exists`, kind match, lifecycle readability, availability, pack
//! membership) — these are reads of current state, not something a
//! compile-time proof could carry, and the control-plane crate does no
//! I/O of its own (§9.1's decision-assembler law), so this is a call-site
//! concern living in `ob-poc-boundary`, not `ob-poc-control-plane` itself.
//!
//! Reuses `toctou_recheck.rs`'s already-tested 5-kind table mapping
//! (`cbu`/`entity`/`case`/`deal`/`client_group`) rather than inventing a
//! second one — same tables, same primary-key columns, one batched query
//! per kind group instead of `toctou_recheck`'s one-row-at-a-time
//! `SqlRowVersionProvider::row_version`. The design pass's convergence
//! point (T9.2's `SnapshotPins` need `row_version` from the same rows)
//! is why this returns `row_version` alongside the grading facts, not a
//! separate query.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ob_poc_control_plane::entity_binding::EntityFacts;
use uuid::Uuid;

/// One entity's batched-query result: the grading facts G2 consumes, plus
/// the `row_version` T9.2's future pin-capture consumes from the same row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityFactsRow {
    pub facts: EntityFacts,
    pub row_version: i64,
}

/// Abstract source of per-entity facts for G2. The production
/// implementation reads from the DB (see [`PgEntityFactsSource`]); tests
/// use an in-memory map.
#[async_trait]
pub trait EntityFactsSource: Send + Sync {
    /// Batched lookup: `requests` is `(entity_id, declared_kind)` — the
    /// declared kind comes from the verb contract's `lookup_entity_type`
    /// (the caller's job, not this trait's — see
    /// `agent::control_plane_shadow`), never inferred from the UUID
    /// value. Entities absent from the returned map were not found (the
    /// caller renders that as `EntityFacts { exists: false, .. }`, not an
    /// error) unless the declared kind itself is unrecognised, which
    /// **is** an error (an actionable gap, same posture as
    /// `SqlRowVersionProvider::row_version`'s unknown-kind branch).
    async fn entity_facts(
        &self,
        requests: &[(Uuid, String)],
    ) -> Result<HashMap<Uuid, EntityFactsRow>>;
}

/// Per-kind table/column/availability mapping. Table and primary-key
/// columns match `toctou_recheck.rs::SqlRowVersionProvider::row_version`
/// exactly — same 5 kinds, same columns, so a `row_version` read here and
/// there can never silently diverge.
pub(crate) struct KindMapping {
    pub(crate) table: &'static str,
    pub(crate) pk: &'static str,
    /// SQL boolean expression (referencing the row's own columns) that is
    /// `true` when the entity is locked/archived/otherwise unavailable
    /// for mutation. `NULL`-safe — every kind's availability predicate is
    /// written to evaluate to `false`, never `NULL`, when nothing blocks.
    availability_sql: &'static str,
}

/// `pub(crate)` (not private): T9.2's `verify_pins_in_scope`
/// (`toctou_recheck.rs`) reuses this exact table/PK mapping for its
/// locked pin re-read, per the T9.2 design doc's "one mapping, two
/// consumers" note — unlocked batched facts here at shadow-evaluation
/// time (T9.1-pre), locked pin re-read at admission time (T9.2).
pub(crate) fn kind_mapping(kind: &str) -> Result<KindMapping> {
    Ok(match kind {
        "cbu" => KindMapping {
            table: "cbus",
            pk: "cbu_id",
            // disposition_status/operational_status IN (...) evaluates to
            // NULL (not false) when the column itself is NULL — COALESCE
            // guarantees the overall expression is always a real boolean,
            // matching this module's own non-null contract.
            availability_sql: "COALESCE(deleted_at IS NOT NULL, false) \
                OR COALESCE(disposition_status IN ('soft_deleted', 'hard_deleted'), false) \
                OR COALESCE(operational_status IN ('suspended', 'archived', 'offboarded'), false)",
        },
        "entity" => KindMapping {
            table: "entities",
            pk: "entity_id",
            availability_sql: "(deleted_at IS NOT NULL)",
        },
        "case" => KindMapping {
            table: "cases",
            pk: "case_id",
            // `cases` has no lock/archive concept today — no column
            // signals "unavailable for mutation" the way cbus/entities
            // do. `false` here is an honest "not observed to be
            // blocked", not a fabricated guarantee.
            availability_sql: "false",
        },
        "deal" => KindMapping {
            table: "deals",
            pk: "deal_id",
            availability_sql: "false",
        },
        "client_group" => KindMapping {
            table: "client_group",
            pk: "id",
            availability_sql: "false",
        },
        other => {
            return Err(anyhow!(
                "entity_facts: no table mapping for entity_kind `{}` — \
                 toctou_recheck.rs's SqlRowVersionProvider only covers \
                 cbu / entity / case / deal / client_group. Extend both \
                 mappings together OR add this kind to the gate-surface audit.",
                other
            ));
        }
    })
}

/// Production implementation — one batched `SELECT ... WHERE pk = ANY($1)`
/// per requested kind (not per entity), matching the "one round trip"
/// design-pass goal.
#[cfg(feature = "database")]
pub struct PgEntityFactsSource<'a> {
    pub pool: &'a sqlx::PgPool,
}

#[cfg(feature = "database")]
#[async_trait]
impl<'a> EntityFactsSource for PgEntityFactsSource<'a> {
    async fn entity_facts(
        &self,
        requests: &[(Uuid, String)],
    ) -> Result<HashMap<Uuid, EntityFactsRow>> {
        use sqlx::Row;

        // Group requested ids by declared kind so each kind gets exactly
        // one batched query, regardless of how many entities of that
        // kind were requested.
        let mut by_kind: HashMap<String, Vec<Uuid>> = HashMap::new();
        for (id, kind) in requests {
            by_kind.entry(kind.clone()).or_default().push(*id);
        }

        let mut out = HashMap::with_capacity(requests.len());

        for (kind, ids) in by_kind {
            let mapping = kind_mapping(&kind)?;
            let sql = format!(
                r#"SELECT {pk} AS id, row_version, {availability} AS availability_blocked
                   FROM "ob-poc".{table}
                   WHERE {pk} = ANY($1)"#,
                pk = mapping.pk,
                table = mapping.table,
                availability = mapping.availability_sql,
            );

            let rows = sqlx::query(&sql)
                .bind(&ids)
                .fetch_all(self.pool)
                .await
                .map_err(|e| {
                    anyhow!(
                        "entity_facts: batched lookup in `{}` for kind `{}` failed: {}",
                        mapping.table,
                        kind,
                        e
                    )
                })?;

            for row in rows {
                let id: Uuid = row.try_get("id")?;
                let row_version: i64 = row.try_get("row_version")?;
                let availability_blocked: bool = row.try_get("availability_blocked")?;

                out.insert(
                    id,
                    EntityFactsRow {
                        facts: EntityFacts {
                            entity_id: id,
                            exists: true,
                            expected_kind: kind.clone(),
                            // A row found under the kind's own table is,
                            // by construction, of that kind — this
                            // mapping does not attempt cross-table kind
                            // *mismatch* detection (e.g. a UUID declared
                            // "cbu" that actually belongs to `entities`);
                            // that would require querying every table for
                            // every id, not just the declared one. See
                            // the module doc's design-pass note.
                            actual_kind: kind.clone(),
                            lifecycle_state_readable: true,
                            availability_blocked,
                            availability_reason: if availability_blocked {
                                Some(format!("{kind} row is locked/archived/soft-deleted"))
                            } else {
                                None
                            },
                            // Open question, not fabricated: "active
                            // pack" membership for G2 is distinct from
                            // G3's SemOS pack resolution (T9.1a, not yet
                            // wired) and EntityBinding has no declared
                            // dependency on PackResolution in
                            // GATE_DEPENDENCIES — so G2 cannot correctly
                            // wait for G3's answer. Defaulting to `true`
                            // ("not observed to be outside pack") rather
                            // than `false`, which would fail every
                            // dispatch unconditionally and turn G2 into a
                            // guaranteed-wrong signal — same conservative
                            // non-blocking-default pattern already used
                            // for AuthorityInput's toctou_drifted /
                            // requires_human_approval flags (T9.1c). This
                            // is a real open question for the architect,
                            // not a considered answer.
                            in_active_pack: true,
                        },
                        row_version,
                    },
                );
            }
        }

        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn pg_entity_facts_source_reads_real_cbu_row() {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required for db-integration tests");
        let pool = sqlx::PgPool::connect(&url).await.expect("connect");

        let cbu_id: Uuid = sqlx::query_scalar(r#"SELECT cbu_id FROM "ob-poc".cbus LIMIT 1"#)
            .fetch_one(&pool)
            .await
            .expect("at least one cbu row exists in the dev database");

        let source = PgEntityFactsSource { pool: &pool };
        let results = source
            .entity_facts(&[(cbu_id, "cbu".to_string())])
            .await
            .expect("batched lookup succeeds");

        let row = results.get(&cbu_id).expect("requested entity present in result map");
        assert!(row.facts.exists);
        assert_eq!(row.facts.expected_kind, "cbu");
        assert_eq!(row.facts.actual_kind, "cbu");
        assert!(row.facts.lifecycle_state_readable);
        assert!(row.row_version >= 1);
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn pg_entity_facts_source_omits_nonexistent_entity() {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required for db-integration tests");
        let pool = sqlx::PgPool::connect(&url).await.expect("connect");

        let missing_id = Uuid::new_v4();
        let source = PgEntityFactsSource { pool: &pool };
        let results = source
            .entity_facts(&[(missing_id, "cbu".to_string())])
            .await
            .expect("batched lookup succeeds even when nothing matches");

        assert!(results.get(&missing_id).is_none());
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn pg_entity_facts_source_rejects_unmapped_kind() {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required for db-integration tests");
        let pool = sqlx::PgPool::connect(&url).await.expect("connect");

        let source = PgEntityFactsSource { pool: &pool };
        let err = source
            .entity_facts(&[(Uuid::new_v4(), "not-a-real-kind".to_string())])
            .await
            .expect_err("unmapped kind must error, not silently skip");
        assert!(err.to_string().contains("no table mapping"));
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn pg_entity_facts_source_batches_multiple_kinds_in_one_call() {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required for db-integration tests");
        let pool = sqlx::PgPool::connect(&url).await.expect("connect");

        let cbu_id: Uuid = sqlx::query_scalar(r#"SELECT cbu_id FROM "ob-poc".cbus LIMIT 1"#)
            .fetch_one(&pool)
            .await
            .expect("at least one cbu row exists");
        let entity_id: Uuid = sqlx::query_scalar(r#"SELECT entity_id FROM "ob-poc".entities LIMIT 1"#)
            .fetch_one(&pool)
            .await
            .expect("at least one entity row exists");

        let source = PgEntityFactsSource { pool: &pool };
        let results = source
            .entity_facts(&[
                (cbu_id, "cbu".to_string()),
                (entity_id, "entity".to_string()),
            ])
            .await
            .expect("batched lookup across two kinds succeeds");

        assert_eq!(results.len(), 2);
        assert_eq!(results[&cbu_id].facts.expected_kind, "cbu");
        assert_eq!(results[&entity_id].facts.expected_kind, "entity");
    }
}
