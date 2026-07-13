//! T7.2 (EOP-PLAN-CONTROLPLANE-001): read-only metrics over the three
//! `control_plane_*` tables built by T2.7 (`control_plane_shadow_decisions`),
//! T4.2 (`control_plane_envelopes`), and T5.3 (`control_plane_write_attestations`),
//! plus (`EOP-DESIGN-CONTROLPLANE-G2-AUDIT-PROVENANCE-001` §3) the
//! `control_plane_audit` stream's later-arriving `consume_seam`/
//! `post_dispatch` gate outcomes.
//!
//! Every query here is purely observational — no function in this module
//! mutates state or feeds any dispatch/admission decision. V&S §6.14 also
//! asks for "exception ageing" and "replay success" metrics; both are
//! deliberately omitted here because no exception-tracking table and no
//! decision-replay job exist yet (T7.3, not attempted this tranche — see
//! the ownership ledger) — reporting a metric with nothing behind it would
//! be worse than reporting fewer, honest ones.

use serde::Serialize;

/// One row of the per-gate outcome breakdown: how many recorded decisions
/// graded each `GateId` as `Success` / `Failure` / `NotEvaluated` /
/// `NotImplemented` / `NotRegistered` (the G2 item 1 sentinel fix, below),
/// broken out by `provenance` (§3, DD-3: `shadow_eval` / `consume_seam` /
/// `post_dispatch`). `gate_results` is stored as `{"GateName": "<Debug
/// string>"}` (`control_plane_shadow.rs::report_to_json`) — this module
/// classifies by the outcome variant's textual prefix rather than parsing
/// the full `Debug` string, since `Failure(reason)`/`NotEvaluated {
/// blocked_by }` payloads vary per row and only the variant matters for a
/// rejection-rate metric.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GateOutcomeCount {
    pub gate: String,
    pub outcome_kind: String,
    pub provenance: String,
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


/// §3 (DD-3): the per-gate outcome breakdown, rebuilt as the three-way
/// UNION the design doc specifies — `shadow_eval` (the pre-existing
/// `control_plane_shadow_decisions.gate_results` source, unchanged logic
/// except for the G2 item 1 sentinel fix below), `consume_seam`
/// (`control_plane_audit` `EnvelopeConsumed` rows, G10), and
/// `post_dispatch` (`control_plane_audit` `DispatchCommitted` rows, G14).
///
/// **G2 item 1 fix, landed in this same rewrite** (per the design doc's
/// own instruction, §3: "one rewrite, both fixes, coordinated in one
/// diff"): `report_to_json` writes the literal string `"missing"` for a
/// gate that was not yet registered in `evaluate_shadow`'s gate map when
/// an older row was persisted (a real, historical-row-only gap — every
/// gate has been wired since T9.7). The old query's `CASE` had no branch
/// for it, so `"missing"` silently fell into the catch-all `ELSE
/// 'Unrecognised'` bucket, indistinguishable from a genuinely corrupt or
/// unexpected value. This rewrite gives it its own `'NotRegistered'`
/// bucket so a triage query can tell "the gate didn't exist yet when this
/// row was written" apart from "something is actually wrong with this
/// row" — see W3's test for the counted delta this produces.
#[cfg(feature = "database")]
pub(crate) async fn gate_outcome_counts(
    pool: &sqlx::PgPool,
) -> Result<Vec<GateOutcomeCount>, sqlx::Error> {
    let rows: Vec<(String, String, String, i64)> = sqlx::query_as(
        r#"
        WITH shadow_eval AS (
            SELECT
                kv.key AS gate,
                CASE
                    WHEN kv.value = 'Success' THEN 'Success'
                    WHEN kv.value LIKE 'Failure%' THEN 'Failure'
                    WHEN kv.value LIKE 'NotEvaluated%' THEN 'NotEvaluated'
                    WHEN kv.value = 'NotImplemented' THEN 'NotImplemented'
                    WHEN kv.value = 'missing' THEN 'NotRegistered'
                    ELSE 'Unrecognised'
                END AS outcome_kind,
                'shadow_eval' AS provenance
            FROM "ob-poc".control_plane_shadow_decisions d,
                 LATERAL jsonb_each_text(d.gate_results) AS kv(key, value)
        ),
        consume_seam AS (
            SELECT
                a.payload -> 'gate_outcome' ->> 'gate' AS gate,
                a.payload -> 'gate_outcome' ->> 'outcome_kind' AS outcome_kind,
                'consume_seam' AS provenance
            FROM "ob-poc".control_plane_audit a
            WHERE a.event_type = 'EnvelopeConsumed'
        ),
        post_dispatch AS (
            SELECT
                a.payload -> 'gate_outcome' ->> 'gate' AS gate,
                a.payload -> 'gate_outcome' ->> 'outcome_kind' AS outcome_kind,
                'post_dispatch' AS provenance
            FROM "ob-poc".control_plane_audit a
            WHERE a.event_type = 'DispatchCommitted'
        ),
        unioned AS (
            SELECT * FROM shadow_eval
            UNION ALL
            SELECT * FROM consume_seam
            UNION ALL
            SELECT * FROM post_dispatch
        )
        SELECT gate, outcome_kind, provenance, COUNT(*) AS count
        FROM unioned
        WHERE gate IS NOT NULL AND outcome_kind IS NOT NULL
        GROUP BY gate, outcome_kind, provenance
        ORDER BY gate, provenance, outcome_kind
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(gate, outcome_kind, provenance, count)| GateOutcomeCount {
            gate,
            outcome_kind,
            provenance,
            count,
        })
        .collect())
}

/// The pre-rewrite `shadow_eval`-only query, kept **only** as a W3
/// regression fixture (`gate_outcome_counts_shadow_eval_matches_legacy_query_modulo_sentinel_fix`)
/// — not called from any production path. Byte-identical to the old
/// production query this module shipped before this session, including
/// the un-fixed `"missing"` -> `'Unrecognised'` sentinel bug, so the test
/// can assert the *only* difference between it and the new query's
/// `shadow_eval` slice is the G2 item 1 sentinel reclassification.
#[cfg(all(test, feature = "database"))]
async fn gate_outcome_counts_legacy_shadow_eval_only(
    pool: &sqlx::PgPool,
) -> Result<Vec<(String, String, i64)>, sqlx::Error> {
    sqlx::query_as(
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
    .await
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

    /// E3 completion-invariant probe (invariant-promotion session,
    /// 2026-07-13): "G1-G14 each evaluated in production (not
    /// `NotImplemented`) with metrics flowing."
    ///
    /// `gate_label` is an EXHAUSTIVE match over `GateId` with no `_` arm —
    /// per the session's governing principle, a 15th gate must break this
    /// at compile time until this probe is updated to cover it, the same
    /// discipline the codebase already applies to `PruneReason` and
    /// `ValidationState`/`OperationalState`/`DispositionState` matches
    /// elsewhere.
    ///
    /// For each of the 14 gates, queries `gate_outcome_counts` (the real,
    /// already-wired T7.2 metrics function this session reuses rather than
    /// duplicates) against real persisted `control_plane_shadow_decisions`
    /// rows and requires at least one `Success` or `Failure` sample —
    /// `NotEvaluated`/`NotImplemented`/`Unrecognised` (including the
    /// `"missing"` sentinel `report_to_json` writes for a gate that wasn't
    /// yet registered in `evaluate_shadow`'s gate map when an older row was
    /// persisted — a real, separately-flagged gap in the existing T7.2
    /// classifier SQL, not fixed here, out of this session's scope) do not
    /// count as "evaluated," matching the invariant's literal text.
    #[test]
    fn e3_gate_label_match_is_exhaustive() {
        // Compile-time proof only — see e3_invariant_probe for the live
        // check. This function's body never runs meaningfully; its only
        // job is to force a compile error if GateId gains a variant this
        // match doesn't cover.
        fn gate_label(id: ob_poc_control_plane::gate::GateId) -> &'static str {
            use ob_poc_control_plane::gate::GateId;
            match id {
                GateId::IntentAdmission => "IntentAdmission",
                GateId::EntityBinding => "EntityBinding",
                GateId::PackResolution => "PackResolution",
                GateId::DagProof => "DagProof",
                GateId::Authority => "Authority",
                GateId::Evidence => "Evidence",
                GateId::WriteSet => "WriteSet",
                GateId::StpClassifier => "StpClassifier",
                GateId::RunbookProof => "RunbookProof",
                GateId::ExecutionEnvelope => "ExecutionEnvelope",
                GateId::AuditReplay => "AuditReplay",
                GateId::VersionPinning => "VersionPinning",
                GateId::DecisionSnapshot => "DecisionSnapshot",
                GateId::WriteSetAttestation => "WriteSetAttestation",
                // NO _ arm — a 15th GateId variant fails this match at
                // compile time, not at gate-run time.
            }
        }
        for id in ob_poc_control_plane::gate::GateId::ALL {
            assert!(!gate_label(id).is_empty());
        }
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn e3_invariant_probe() {
        fn gate_label(id: ob_poc_control_plane::gate::GateId) -> &'static str {
            use ob_poc_control_plane::gate::GateId;
            match id {
                GateId::IntentAdmission => "IntentAdmission",
                GateId::EntityBinding => "EntityBinding",
                GateId::PackResolution => "PackResolution",
                GateId::DagProof => "DagProof",
                GateId::Authority => "Authority",
                GateId::Evidence => "Evidence",
                GateId::WriteSet => "WriteSet",
                GateId::StpClassifier => "StpClassifier",
                GateId::RunbookProof => "RunbookProof",
                GateId::ExecutionEnvelope => "ExecutionEnvelope",
                GateId::AuditReplay => "AuditReplay",
                GateId::VersionPinning => "VersionPinning",
                GateId::DecisionSnapshot => "DecisionSnapshot",
                GateId::WriteSetAttestation => "WriteSetAttestation",
            }
        }

        // Connection/query failures are INFRASTRUCTURE problems, not
        // invariant-failure evidence — an unreachable database proves
        // nothing about whether the gates have production samples. Panic
        // with a distinct E3_INFRASTRUCTURE_FAILURE marker so
        // scripts/check-invariants.sh (and any human reading captured
        // test output) cannot mistake "couldn't verify" for "verified
        // failing" (2026-07-13 review finding #3: an expected-fail
        // ratchet entry is satisfied identically by either, so the
        // distinction has to be made here, in the only place that still
        // has the real error).
        let database_url = std::env::var("DATABASE_URL")
            .expect("E3_INFRASTRUCTURE_FAILURE: DATABASE_URL must be set");
        let pool = match sqlx::PgPool::connect(&database_url).await {
            Ok(p) => p,
            Err(e) => panic!("E3_INFRASTRUCTURE_FAILURE: could not connect to database: {e}"),
        };
        let counts = match gate_outcome_counts(&pool).await {
            Ok(c) => c,
            Err(e) => panic!("E3_INFRASTRUCTURE_FAILURE: gate_outcome_counts query failed: {e}"),
        };

        // §3 assertion change (EOP-DESIGN-CONTROLPLANE-G2-AUDIT-PROVENANCE-001):
        // "substantive samples exist for each gate at its expected
        // provenance — a gate reporting samples only at the wrong
        // provenance FAILS." `wrong_provenance_only` names a stronger
        // failure than `failing` (zero samples anywhere): the gate has
        // production evidence, but not at the locus the per-gate map
        // (`expected_provenance`) says it should be graded at — exactly
        // the sentinel-detection value W4/§3 exist to buy.
        let mut failing: Vec<&'static str> = Vec::new();
        let mut wrong_provenance_only: Vec<&'static str> = Vec::new();
        for id in ob_poc_control_plane::gate::GateId::ALL {
            let label = gate_label(id);
            let expected = ob_poc_control_plane::audit::expected_provenance(id).as_str();

            let substantive_any: i64 = counts
                .iter()
                .filter(|c| c.gate == label && (c.outcome_kind == "Success" || c.outcome_kind == "Failure"))
                .map(|c| c.count)
                .sum();
            let substantive_expected: i64 = counts
                .iter()
                .filter(|c| {
                    c.gate == label
                        && c.provenance == expected
                        && (c.outcome_kind == "Success" || c.outcome_kind == "Failure")
                })
                .map(|c| c.count)
                .sum();
            println!(
                "[E3] {label}: {substantive_any} substantive (Success/Failure) production samples total, \
                 {substantive_expected} at expected provenance={expected}"
            );
            if substantive_any == 0 {
                failing.push(label);
            } else if substantive_expected == 0 {
                wrong_provenance_only.push(label);
            }
        }

        // E3_INVARIANT_FAILURE marker (as opposed to E3_INFRASTRUCTURE_FAILURE
        // above): this is a real, verified, substantive result — the
        // database was reachable, the query ran, and N/14 gates genuinely
        // have zero production samples (or samples only at the wrong
        // provenance).
        assert!(
            failing.is_empty() && wrong_provenance_only.is_empty(),
            "E3_INVARIANT_FAILURE: {} gate(s) have zero substantive production samples anywhere: {failing:?}; \
             {} gate(s) have samples only at the WRONG provenance (expected-provenance mismatch): {wrong_provenance_only:?}",
            failing.len(),
            wrong_provenance_only.len(),
        );
    }

    /// W4 (standing rule 3 window-discipline proof obligation): "no Path-A
    /// gate emits at `ConsumeSeam` or `PostDispatch` provenance except
    /// G10/G14 respectively (the expected-provenance map enforced as a
    /// test, not a comment)." A live-DB check over whatever
    /// `control_plane_audit`/`control_plane_shadow_decisions` rows exist:
    /// no gate other than `ExecutionEnvelope` (G10) has any sample at
    /// `consume_seam`, and no gate other than `WriteSetAttestation` (G14)
    /// has any sample at `post_dispatch`.
    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn w4_no_gate_other_than_g10_g14_emits_at_the_wrong_late_provenance() {
        let pool = test_pool().await;
        let counts = gate_outcome_counts(&pool).await.expect("query failed");

        let consume_seam_violators: Vec<&str> = counts
            .iter()
            .filter(|c| c.provenance == "consume_seam" && c.gate != "ExecutionEnvelope")
            .map(|c| c.gate.as_str())
            .collect();
        assert!(
            consume_seam_violators.is_empty(),
            "W4 violation: only G10 (ExecutionEnvelope) may emit at consume_seam provenance, found: {consume_seam_violators:?}"
        );

        let post_dispatch_violators: Vec<&str> = counts
            .iter()
            .filter(|c| c.provenance == "post_dispatch" && c.gate != "WriteSetAttestation")
            .map(|c| c.gate.as_str())
            .collect();
        assert!(
            post_dispatch_violators.is_empty(),
            "W4 violation: only G14 (WriteSetAttestation) may emit at post_dispatch provenance, found: {post_dispatch_violators:?}"
        );
    }

    /// W3: the rebuilt `gate_outcome_counts`' `shadow_eval`-provenance
    /// slice must return exactly the counts the OLD (pre-rewrite) query
    /// returned on the same data, modulo the G2 item 1 sentinel fix
    /// (`"missing"` -> `NotRegistered` instead of falling into
    /// `Unrecognised`) — that delta gets its own explicit assertion below,
    /// not folded silently into an "expected to differ somewhere" catch-all.
    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn w3_shadow_eval_slice_matches_legacy_query_modulo_sentinel_fix() {
        let pool = test_pool().await;
        let session_id = Uuid::new_v4();

        // A row exercising every branch: Success, Failure, NotEvaluated
        // (via a real dependency block — G3 depends on G2), and a row with
        // a gate simply absent from `report.results` (the "missing"
        // sentinel path `report_to_json` writes as the literal string
        // `"missing"`).
        let mut full_report = ob_poc_control_plane::gate::EvaluationReport::default();
        full_report.results.insert(
            ob_poc_control_plane::gate::GateId::IntentAdmission,
            ob_poc_control_plane::gate::GateResult::Success,
        );
        full_report.results.insert(
            ob_poc_control_plane::gate::GateId::Authority,
            ob_poc_control_plane::gate::GateResult::Failure("denied".to_string()),
        );
        full_report.results.insert(
            ob_poc_control_plane::gate::GateId::PackResolution,
            ob_poc_control_plane::gate::GateResult::NotEvaluated {
                blocked_by: vec![ob_poc_control_plane::gate::GateId::EntityBinding],
            },
        );
        // WriteSetAttestation deliberately left absent from `.results` ->
        // report_to_json renders "missing" for it.
        let row = crate::agent::control_plane_shadow::build_shadow_decision_row(
            session_id,
            Uuid::new_v4(),
            "cbu.confirm",
            &full_report,
            false,
        );
        assert!(crate::agent::control_plane_shadow::insert_shadow_decision(&pool, &row).await);

        let legacy = gate_outcome_counts_legacy_shadow_eval_only(&pool)
            .await
            .expect("legacy query failed");
        let rebuilt = gate_outcome_counts(&pool).await.expect("rebuilt query failed");
        let rebuilt_shadow_eval: Vec<(String, String, i64)> = rebuilt
            .into_iter()
            .filter(|c| c.provenance == "shadow_eval")
            .map(|c| (c.gate, c.outcome_kind, c.count))
            .collect();

        // Every legacy (gate, outcome_kind) bucket other than the
        // "Unrecognised" bucket that used to absorb the "missing" sentinel
        // must have an EXACT count match in the rebuilt shadow_eval slice.
        for (gate, outcome_kind, legacy_count) in &legacy {
            if outcome_kind == "Unrecognised" {
                continue; // may have shrunk — checked separately below
            }
            let rebuilt_count = rebuilt_shadow_eval
                .iter()
                .find(|(g, o, _)| g == gate && o == outcome_kind)
                .map(|(_, _, c)| *c)
                .unwrap_or(0);
            assert_eq!(
                rebuilt_count, *legacy_count,
                "W3 violation: (gate={gate}, outcome_kind={outcome_kind}) count diverged: legacy={legacy_count}, rebuilt={rebuilt_count}"
            );
        }

        // The sentinel-fix delta, enumerated in its own assertion (not
        // folded into the loop above): every "missing" sentinel that used
        // to inflate legacy's `Unrecognised` bucket must now appear under
        // `NotRegistered` in the rebuilt query, and legacy's Unrecognised
        // count for that gate must equal (rebuilt Unrecognised + rebuilt
        // NotRegistered) for the same gate.
        let legacy_unrecognised: std::collections::HashMap<&str, i64> = legacy
            .iter()
            .filter(|(_, o, _)| o == "Unrecognised")
            .map(|(g, _, c)| (g.as_str(), *c))
            .collect();
        for (gate, legacy_count) in &legacy_unrecognised {
            let rebuilt_unrecognised = rebuilt_shadow_eval
                .iter()
                .find(|(g, o, _)| g == gate && o == "Unrecognised")
                .map(|(_, _, c)| *c)
                .unwrap_or(0);
            let rebuilt_not_registered = rebuilt_shadow_eval
                .iter()
                .find(|(g, o, _)| g == gate && o == "NotRegistered")
                .map(|(_, _, c)| *c)
                .unwrap_or(0);
            assert_eq!(
                rebuilt_unrecognised + rebuilt_not_registered,
                *legacy_count,
                "W3 sentinel-fix delta violation for gate={gate}: legacy Unrecognised={legacy_count}, \
                 rebuilt Unrecognised={rebuilt_unrecognised} + NotRegistered={rebuilt_not_registered}"
            );
        }
        // This fixture's own WriteSetAttestation row specifically must
        // have moved from Unrecognised to NotRegistered.
        let write_set_attestation_not_registered = rebuilt_shadow_eval
            .iter()
            .find(|(g, o, _)| g == "WriteSetAttestation" && o == "NotRegistered")
            .map(|(_, _, c)| *c)
            .unwrap_or(0);
        assert!(
            write_set_attestation_not_registered >= 1,
            "expected at least one WriteSetAttestation/NotRegistered row from this fixture's own insert, got {rebuilt_shadow_eval:?}"
        );
    }
}
