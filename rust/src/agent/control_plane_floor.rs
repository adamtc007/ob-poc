//! T11.F.2 (EOP-PLAN-CONTROLPLANE-002): the definitional floor's audit
//! persistence and G1 registry-lookup fast path.
//!
//! Design doc: `docs/todo/control-plane/EOP-DESIGN-CONTROLPLANE-T11.F.2-DEFINITIONAL-FLOOR-001.md`.
//! Floor-eligibility classification for G3/G4 lives in
//! `ob_poc_control_plane::floor` (that crate's own module — the single
//! source of truth the design doc's §1.1 drift guard requires). This
//! module hosts what that crate structurally cannot: the G1 check (needs
//! `dsl_v2::runtime_registry`, an `ob-poc`-internal registry) and the
//! audit-record insert (needs `sqlx`, which `ob-poc-control-plane` — the
//! pure decision-model crate — deliberately does not depend on).

use uuid::Uuid;

/// G1's floor check: does `verb_fqn` exist in the runtime verb registry
/// at all?
///
/// Bypasses `ob_poc_control_plane::intent_admission::decide()` entirely —
/// that function has a real, documented defect (ownership ledger,
/// "Defect register — G1") that makes it unable to discriminate "verb
/// doesn't exist" from "verb exists but is policy-denied." This is a
/// direct, cheap, synchronous dictionary-membership check instead: no
/// `SessionVerbSurface`/ABAC/pack context needed, none consulted.
///
/// `verb_fqn` must be `domain.verb` shaped — anything that doesn't parse
/// that way is honestly floor-eligible too (it cannot be a real verb).
pub(crate) fn g1_verb_is_registered(verb_fqn: &str) -> bool {
    let Some((domain, verb)) = verb_fqn.split_once('.') else {
        return false;
    };
    crate::dsl_v2::runtime_registry::runtime_registry()
        .get(domain, verb)
        .is_some()
}

/// One floor-rejection record, ready to persist.
#[derive(Debug, Clone)]
pub(crate) struct FloorRejectionRow {
    pub session_id: Uuid,
    pub entry_id: Uuid,
    pub verb_fqn: String,
    /// `"G1"` | `"G3"` | `"G4"`.
    pub floor_gate: &'static str,
    pub floor_reason: String,
}

/// Persists a floor rejection into `"ob-poc".control_plane_shadow_decisions`
/// (`floor_rejected = true`, `gate_results` carries only the floor's own
/// finding — this is a real rejection record, not a full shadow
/// evaluation, so it does not fabricate outcomes for gates the floor
/// didn't consult).
///
/// # Ordering — the exact asymmetry the design doc requires (§4)
///
/// The floor's *decision* (the caller has already rejected the request
/// before this function is even called — see call sites) never depends on
/// this function's success. But unlike `insert_shadow_decision`'s
/// best-effort/ordinary-`tracing::warn!` posture (shadow observation, not
/// a real rejection — that asymmetry is deliberate and stays unchanged),
/// a floor rejection's audit-write failure is a real audit gap even
/// though the control itself worked correctly, so it is logged at
/// `tracing::error!` — a distinct, alert-grade severity from
/// `insert_shadow_decision`'s `tracing::warn!` — specifically so it is
/// distinguishable in whatever the operator actually monitors, per the
/// design doc's explicit requirement that this failure class not share a
/// log level with ordinary best-effort telemetry failures.
pub(crate) async fn insert_floor_rejection(pool: &sqlx::PgPool, row: &FloorRejectionRow) -> bool {
    let gate_results = serde_json::json!({ row.floor_gate: format!("FloorRejected({})", row.floor_reason) });

    let result = sqlx::query(
        r#"
        INSERT INTO "ob-poc".control_plane_shadow_decisions (
            session_id, entry_id, verb_fqn, gate_results,
            legacy_outcome_blocked, shadow_intent_admission_blocked, diverged,
            floor_rejected, floor_gate, floor_reason
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, true, $8, $9)
        "#,
    )
    .bind(row.session_id)
    .bind(row.entry_id)
    .bind(&row.verb_fqn)
    .bind(&gate_results)
    // A floor rejection IS a legacy-equivalent block (the request did not
    // proceed) by definition — both booleans are `true`, `diverged` is
    // `false`: there is no legacy/shadow divergence to report here, the
    // floor and the outcome agree by construction.
    .bind(true)
    .bind(true)
    .bind(false)
    .bind(row.floor_gate)
    .bind(&row.floor_reason)
    .execute(pool)
    .await;

    match result {
        Ok(_) => true,
        Err(err) => {
            tracing::error!(
                error = %err,
                entry_id = %row.entry_id,
                verb_fqn = %row.verb_fqn,
                floor_gate = row.floor_gate,
                "T11.F.2: floor-rejection audit record failed to persist — \
                 the rejection itself still stands, but this is a real audit \
                 gap, not an ordinary best-effort telemetry miss"
            );
            false
        }
    }
}

#[cfg(all(test, feature = "database"))]
mod tests {
    use super::*;

    async fn test_pool() -> sqlx::PgPool {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required for db-integration tests");
        sqlx::PgPool::connect(&url).await.expect("connect")
    }

    #[test]
    fn g1_registered_verb_is_not_floor_eligible() {
        // cbu.confirm is a real, registered verb throughout this session's
        // other fixtures (T9.2/T10.1/T10.2 tests all use it as the known-
        // good case) — reused here rather than inventing a new one.
        assert!(g1_verb_is_registered("cbu.confirm"));
    }

    #[test]
    fn g1_unknown_verb_is_floor_eligible() {
        assert!(!g1_verb_is_registered("nonexistent.verb"));
    }

    #[test]
    fn g1_malformed_fqn_is_floor_eligible() {
        assert!(!g1_verb_is_registered("not-a-verb-fqn-at-all"));
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn insert_floor_rejection_persists_a_readable_row() {
        let pool = test_pool().await;
        let entry_id = Uuid::new_v4();
        let row = FloorRejectionRow {
            session_id: Uuid::new_v4(),
            entry_id,
            verb_fqn: "nonexistent.verb".to_string(),
            floor_gate: "G1",
            floor_reason: "verb not found in runtime registry".to_string(),
        };

        assert!(insert_floor_rejection(&pool, &row).await);

        let persisted: (bool, Option<String>, Option<String>) = sqlx::query_as(
            r#"SELECT floor_rejected, floor_gate, floor_reason
               FROM "ob-poc".control_plane_shadow_decisions WHERE entry_id = $1"#,
        )
        .bind(entry_id)
        .fetch_one(&pool)
        .await
        .expect("row must exist");

        assert!(persisted.0);
        assert_eq!(persisted.1.as_deref(), Some("G1"));
        assert_eq!(persisted.2.as_deref(), Some("verb not found in runtime registry"));
    }
}
