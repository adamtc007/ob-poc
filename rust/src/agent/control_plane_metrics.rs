//! T7.2 (EOP-PLAN-CONTROLPLANE-001): read-only metrics over the three
//! `control_plane_*` tables built by T2.7 (`control_plane_shadow_decisions`),
//! T4.2 (`control_plane_envelopes`), and T5.3 (`control_plane_write_attestations`).
//!
//! Every query here is purely observational — no function in this module
//! mutates state or feeds any dispatch/admission decision. V&S §6.14 also
//! asks for "exception ageing" and "replay success" metrics; both are
//! deliberately omitted here because no exception-tracking table and no
//! decision-replay job exist yet (T7.3, not attempted this tranche — see
//! the ownership ledger) — reporting a metric with nothing behind it would
//! be worse than reporting fewer, honest ones.

use serde::Serialize;

/// One row of the per-gate outcome breakdown: how many recorded shadow
/// decisions graded each `GateId` as `Success` / `Failure` / `NotEvaluated`
/// / `NotImplemented`. `gate_results` is stored as `{"GateName": "<Debug
/// string>"}` (`control_plane_shadow.rs::report_to_json`) — this module
/// classifies by the outcome variant's textual prefix rather than parsing
/// the full `Debug` string, since `Failure(reason)`/`NotEvaluated {
/// blocked_by }` payloads vary per row and only the variant matters for a
/// rejection-rate metric.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GateOutcomeCount {
    pub gate: String,
    pub outcome_kind: String,
    pub count: i64,
}

/// Shadow-vs-legacy divergence over every persisted `control_plane_shadow_decisions`
/// row (T2.7). A non-zero `diverged` count is the graduation-blocking signal
/// plan §0 names: a gate cannot move to enforce mode while divergence exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ShadowDivergenceStats {
    pub total_decisions: i64,
    pub diverged: i64,
}

impl ShadowDivergenceStats {
    pub fn divergence_rate(&self) -> f64 {
        if self.total_decisions == 0 {
            0.0
        } else {
            self.diverged as f64 / self.total_decisions as f64
        }
    }
}

/// Write-set attestation breach counts over every persisted
/// `control_plane_write_attestations` row (T5.3, currently only populated by
/// live-DB tests — no production caller invokes `commit_attested` yet, see
/// the ownership ledger's C-032 entry).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct WriteAttestationBreachStats {
    pub total_attestations: i64,
    pub breaches: i64,
}

impl WriteAttestationBreachStats {
    pub fn breach_rate(&self) -> f64 {
        if self.total_attestations == 0 {
            0.0
        } else {
            self.breaches as f64 / self.total_attestations as f64
        }
    }
}

/// One row of the envelope status breakdown (`control_plane_envelopes.status`,
/// T4.2): `sealed` / `consumed` / `expired` / `voided`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EnvelopeStatusCount {
    pub status: String,
    pub count: i64,
}

/// T10.1's graduation-telemetry answer to "what fraction of Path A would
/// enforce cleanly today" — per verb family, over every recorded
/// `control_plane_shadow_decisions` row (not a separate counter; derived
/// from the same `gate_results` JSONB the shadow-divergence metric already
/// reads, so this can never drift from what was actually observed at
/// shadow-evaluation time). "Sealable" means every gate `decision::evaluate`
/// requires for `ApprovedStp` reported `Success` in that row: the eight
/// proof-bearing gates (`PROOF_BEARING_GATES`, `decision.rs`) plus
/// `RunbookProof` and `StpClassifier`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SealableRateByVerb {
    pub verb_fqn: String,
    pub total: i64,
    pub sealable: i64,
}


#[cfg(feature = "database")]
pub(crate) async fn gate_outcome_counts(
    pool: &sqlx::PgPool,
) -> Result<Vec<GateOutcomeCount>, sqlx::Error> {
    let rows: Vec<(String, String, i64)> = sqlx::query_as(
        r#"
        SELECT
            kv.key AS gate,
            CASE
                WHEN kv.value = 'Success' THEN 'Success'
                WHEN kv.value LIKE 'Failure%' THEN 'Failure'
                WHEN kv.value LIKE 'NotEvaluated%' THEN 'NotEvaluated'
                WHEN kv.value = 'NotImplemented' THEN 'NotImplemented'
                ELSE 'Unrecognised'
            END AS outcome_kind,
            COUNT(*) AS count
        FROM "ob-poc".control_plane_shadow_decisions d,
             LATERAL jsonb_each_text(d.gate_results) AS kv(key, value)
        GROUP BY kv.key, outcome_kind
        ORDER BY kv.key, outcome_kind
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(gate, outcome_kind, count)| GateOutcomeCount {
            gate,
            outcome_kind,
            count,
        })
        .collect())
}

#[cfg(feature = "database")]
pub(crate) async fn shadow_divergence_stats(
    pool: &sqlx::PgPool,
) -> Result<ShadowDivergenceStats, sqlx::Error> {
    let (total_decisions, diverged): (i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) AS total_decisions,
            COUNT(*) FILTER (WHERE diverged) AS diverged
        FROM "ob-poc".control_plane_shadow_decisions
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok(ShadowDivergenceStats {
        total_decisions,
        diverged,
    })
}

#[cfg(feature = "database")]
pub(crate) async fn write_attestation_breach_stats(
    pool: &sqlx::PgPool,
) -> Result<WriteAttestationBreachStats, sqlx::Error> {
    let (total_attestations, breaches): (i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) AS total_attestations,
            COUNT(*) FILTER (WHERE NOT bounded) AS breaches
        FROM "ob-poc".control_plane_write_attestations
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok(WriteAttestationBreachStats {
        total_attestations,
        breaches,
    })
}

#[cfg(feature = "database")]
pub(crate) async fn envelope_status_counts(
    pool: &sqlx::PgPool,
) -> Result<Vec<EnvelopeStatusCount>, sqlx::Error> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT status, COUNT(*) AS count
        FROM "ob-poc".control_plane_envelopes
        GROUP BY status
        ORDER BY status
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(status, count)| EnvelopeStatusCount { status, count })
        .collect())
}

#[cfg(feature = "database")]
pub(crate) async fn sealable_rate_by_verb(
    pool: &sqlx::PgPool,
) -> Result<Vec<SealableRateByVerb>, sqlx::Error> {
    let rows: Vec<(String, i64, i64)> = sqlx::query_as(
        r#"
        SELECT
            verb_fqn,
            COUNT(*) AS total,
            COUNT(*) FILTER (WHERE
                gate_results->>'IntentAdmission' = 'Success' AND
                gate_results->>'EntityBinding' = 'Success' AND
                gate_results->>'PackResolution' = 'Success' AND
                gate_results->>'DagProof' = 'Success' AND
                gate_results->>'Authority' = 'Success' AND
                gate_results->>'Evidence' = 'Success' AND
                gate_results->>'WriteSet' = 'Success' AND
                gate_results->>'DecisionSnapshot' = 'Success' AND
                gate_results->>'RunbookProof' = 'Success' AND
                gate_results->>'StpClassifier' = 'Success'
            ) AS sealable
        FROM "ob-poc".control_plane_shadow_decisions
        GROUP BY verb_fqn
        ORDER BY verb_fqn
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(verb_fqn, total, sealable)| SealableRateByVerb {
            verb_fqn,
            total,
            sealable,
        })
        .collect())
}

#[cfg(all(test, feature = "database"))]
mod t7_2_metrics_tests {
    use super::*;
    use uuid::Uuid;

    async fn test_pool() -> sqlx::PgPool {
        let database_url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set for control_plane_metrics live-DB tests");
        sqlx::PgPool::connect(&database_url)
            .await
            .expect("failed to connect to test database")
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn gate_outcome_counts_classifies_by_variant_prefix_not_full_debug_string() {
        let pool = test_pool().await;
        let session_id = Uuid::new_v4();

        let row = crate::agent::control_plane_shadow::build_shadow_decision_row(
            session_id,
            Uuid::new_v4(),
            "cbu.confirm",
            &{
                let mut report = ob_poc_control_plane::gate::EvaluationReport::default();
                report.results.insert(
                    ob_poc_control_plane::gate::GateId::IntentAdmission,
                    ob_poc_control_plane::gate::GateResult::Success,
                );
                report.results.insert(
                    ob_poc_control_plane::gate::GateId::Authority,
                    ob_poc_control_plane::gate::GateResult::Failure("denied".to_string()),
                );
                report
            },
            false,
        );
        assert!(crate::agent::control_plane_shadow::insert_shadow_decision(&pool, &row).await);

        let counts = gate_outcome_counts(&pool).await.expect("query failed");
        let intent_admission_success = counts
            .iter()
            .find(|c| c.gate == "IntentAdmission" && c.outcome_kind == "Success");
        assert!(
            intent_admission_success.is_some_and(|c| c.count >= 1),
            "expected at least one IntentAdmission/Success row, got {counts:?}"
        );
        let authority_failure = counts
            .iter()
            .find(|c| c.gate == "Authority" && c.outcome_kind == "Failure");
        assert!(
            authority_failure.is_some_and(|c| c.count >= 1),
            "expected Failure(\"denied\") to classify as outcome_kind=Failure, not \
             leak its payload into the grouping key; got {counts:?}"
        );
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn shadow_divergence_stats_counts_only_diverged_rows() {
        let pool = test_pool().await;
        let session_id = Uuid::new_v4();

        let before = shadow_divergence_stats(&pool).await.expect("query failed");

        let diverging_row = crate::agent::control_plane_shadow::build_shadow_decision_row(
            session_id,
            Uuid::new_v4(),
            "cbu.confirm",
            &ob_poc_control_plane::gate::EvaluationReport::default(),
            false,
        );
        assert!(diverging_row.diverged, "fixture must actually diverge");
        assert!(
            crate::agent::control_plane_shadow::insert_shadow_decision(&pool, &diverging_row)
                .await
        );

        let after = shadow_divergence_stats(&pool).await.expect("query failed");
        // `control_plane_shadow_decisions` is insert-only (no UPDATE/DELETE
        // path exists), so a `>=` monotonic-increase check is race-safe
        // against sibling tests concurrently inserting into the same shared
        // table — an exact-equality delta is not (PIR-D-004: fails under
        // default parallel `cargo test`, only passed under
        // `--test-threads=1`).
        assert!(after.total_decisions >= before.total_decisions + 1);
        assert!(after.diverged >= before.diverged + 1);
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn write_attestation_breach_stats_counts_only_unbounded_rows() {
        let pool = test_pool().await;
        let scope_id = ob_poc_types::TransactionScopeId(Uuid::new_v4());

        let before = write_attestation_breach_stats(&pool)
            .await
            .expect("query failed");

        assert!(
            crate::agent::control_plane_write_attestation_store::persist_attestation(
                &pool, scope_id, None, Some("cbu.confirm"), &[], true, &[],
            )
            .await
        );
        assert!(
            crate::agent::control_plane_write_attestation_store::persist_attestation(
                &pool, scope_id, None, Some("cbu.confirm"), &[], false, &[],
            )
            .await
        );

        let after = write_attestation_breach_stats(&pool)
            .await
            .expect("query failed");
        // `control_plane_write_attestations` is insert-only — same
        // race-safety reasoning as shadow_divergence_stats_counts_only_diverged_rows
        // above (PIR-D-004).
        assert!(after.total_attestations >= before.total_attestations + 2);
        assert!(after.breaches >= before.breaches + 1);
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn envelope_status_counts_reflects_current_row_statuses() {
        let pool = test_pool().await;

        // Unlike the other two `control_plane_*` tables, `status` here is
        // mutable (control_plane_envelope_store.rs performs in-place UPDATEs
        // for consume/expire/void), so a whole-table `sealed` count is
        // neither monotonic nor race-safe: a concurrent sibling test
        // transitioning an envelope OUT of `sealed` between the before/after
        // reads can decrease it, and a before/after delta can therefore fail
        // in either direction under default parallel `cargo test`
        // (PIR-D-004). Scope the assertion to the specific row this test
        // inserted instead of the shared-table aggregate.
        let envelope_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".control_plane_envelopes (
                envelope_id, content_hash, session_id, verb_fqn, status, not_before, not_after
            ) VALUES ($1, $2, $3, $4, 'sealed', now(), now() + interval '1 hour')
            "#,
        )
        .bind(envelope_id)
        .bind("deadbeef")
        .bind(Uuid::new_v4())
        .bind("cbu.confirm")
        .execute(&pool)
        .await
        .expect("seed insert failed");

        let this_row_status: String = sqlx::query_scalar(
            r#"SELECT status FROM "ob-poc".control_plane_envelopes WHERE envelope_id = $1"#,
        )
        .bind(envelope_id)
        .fetch_one(&pool)
        .await
        .expect("row must exist");
        assert_eq!(this_row_status, "sealed");

        // Still exercises the real production aggregate query end-to-end —
        // just doesn't rely on its whole-table count being stable across
        // concurrent sibling activity for the assertion.
        let counts = envelope_status_counts(&pool).await.expect("query failed");
        let sealed_count = counts
            .iter()
            .find(|c| c.status == "sealed")
            .map(|c| c.count)
            .unwrap_or(0);
        assert!(
            sealed_count >= 1,
            "expected at least the row just inserted to appear in the sealed bucket, got {counts:?}"
        );
    }

    /// T10.1: uses a unique-per-run `verb_fqn` (not a real verb) so this
    /// test's exact counts can't be polluted by sibling tests concurrently
    /// inserting `control_plane_shadow_decisions` rows for `cbu.confirm`
    /// (same discipline as the write-attestation test above's
    /// `>=`-vs-exact reasoning — here exact is safe because the verb_fqn
    /// itself is exclusive to this test run).
    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn sealable_rate_by_verb_counts_only_rows_with_every_required_gate_success() {
        let pool = test_pool().await;
        let verb_fqn = format!("test.sealable-rate-{}", Uuid::new_v4());

        let sealable_report = {
            let mut report = ob_poc_control_plane::gate::EvaluationReport::default();
            for gate in [
                ob_poc_control_plane::gate::GateId::IntentAdmission,
                ob_poc_control_plane::gate::GateId::EntityBinding,
                ob_poc_control_plane::gate::GateId::PackResolution,
                ob_poc_control_plane::gate::GateId::DagProof,
                ob_poc_control_plane::gate::GateId::Authority,
                ob_poc_control_plane::gate::GateId::Evidence,
                ob_poc_control_plane::gate::GateId::WriteSet,
                ob_poc_control_plane::gate::GateId::DecisionSnapshot,
                ob_poc_control_plane::gate::GateId::RunbookProof,
                ob_poc_control_plane::gate::GateId::StpClassifier,
            ] {
                report
                    .results
                    .insert(gate, ob_poc_control_plane::gate::GateResult::Success);
            }
            report
        };
        let unsealable_report = {
            let mut report = sealable_report.clone();
            report.results.insert(
                ob_poc_control_plane::gate::GateId::Authority,
                ob_poc_control_plane::gate::GateResult::Failure("denied".to_string()),
            );
            report
        };

        for report in [&sealable_report, &sealable_report, &unsealable_report] {
            let row = crate::agent::control_plane_shadow::build_shadow_decision_row(
                Uuid::new_v4(),
                Uuid::new_v4(),
                &verb_fqn,
                report,
                false,
            );
            assert!(crate::agent::control_plane_shadow::insert_shadow_decision(&pool, &row).await);
        }

        let rates = sealable_rate_by_verb(&pool).await.expect("query failed");
        let row = rates
            .iter()
            .find(|r| r.verb_fqn == verb_fqn)
            .unwrap_or_else(|| panic!("expected a row for {verb_fqn}, got {rates:?}"));
        assert_eq!(row.total, 3);
        assert_eq!(row.sealable, 2);
        assert!((row.sealable as f64 / row.total as f64 - (2.0 / 3.0)).abs() < 1e-9);
    }
}
