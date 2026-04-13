//! Loads the group composite state from the database.
//!
//! Given a set of CBU IDs (from the session scope), queries the database
//! to build a [`GroupCompositeState`] snapshot with per-CBU state summaries.
//!
//! TRANSITIONAL (SE-10 in audit): This module queries raw SQL independently from
//! the session's hydrated constellation DAG. The preferred path is
//! `GroupCompositeState::from_hydrated_constellation()` which projects from
//! the same `HydratedSlot` tree the compiler and narration engine use.
//! This module is kept as a fallback for pre-workspace states where the
//! hydrated constellation is not yet available. It should be removed when
//! all callers have migrated to the DAG-sourced path.

use sqlx::PgPool;
use uuid::Uuid;

use super::composite_state::{CbuStateSummary, GroupCompositeState};

/// Load the composite state for a group's CBUs.
///
/// Takes the CBU IDs currently in the session scope and queries
/// the database for downstream entity states (cases, screenings,
/// trading profiles, document coverage).
///
/// Returns `None` if no CBUs are in scope.
#[cfg(feature = "database")]
pub async fn load_group_composite_state(
    pool: &PgPool,
    cbu_ids: &[Uuid],
) -> anyhow::Result<Option<GroupCompositeState>> {
    if cbu_ids.is_empty() {
        return Ok(None);
    }

    // Group-level UBO/control check — uses any entity linked to these CBUs
    let has_ubo = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM "ob-poc".ubo_registry ur
            WHERE ur.subject_entity_id IN (
                SELECT r.entity_id FROM "ob-poc".cbu_entity_roles r
                WHERE r.cbu_id = ANY($1)
            )
        )
        "#,
    )
    .bind(cbu_ids)
    .fetch_one(pool)
    .await
    .unwrap_or(false);

    let has_control = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM "ob-poc".ownership_snapshots os
            WHERE os.root_entity_id IN (
                SELECT r.entity_id FROM "ob-poc".cbu_entity_roles r
                WHERE r.cbu_id = ANY($1)
            )
        )
        "#,
    )
    .bind(cbu_ids)
    .fetch_one(pool)
    .await
    .unwrap_or(false);

    // Per-CBU onboarding state
    let rows = sqlx::query_as::<_, CbuStateRow>(
        r#"
        SELECT
            c.cbu_id,
            c.name AS cbu_name,
            c.status AS lifecycle_state,
            (SELECT COUNT(*)::int FROM "ob-poc".cases k
             WHERE k.cbu_id = c.cbu_id AND k.closed_at IS NULL) AS case_count,
            (SELECT k.status FROM "ob-poc".cases k
             WHERE k.cbu_id = c.cbu_id AND k.closed_at IS NULL
             ORDER BY k.opened_at DESC LIMIT 1) AS latest_case_status,
            EXISTS(
                SELECT 1 FROM "ob-poc".screenings s
                JOIN "ob-poc".entity_workstreams ew ON ew.workstream_id = s.workstream_id
                JOIN "ob-poc".cases k ON k.case_id = ew.case_id
                WHERE k.cbu_id = c.cbu_id
            ) AS has_screening,
            EXISTS(
                SELECT 1 FROM "ob-poc".screenings s
                JOIN "ob-poc".entity_workstreams ew ON ew.workstream_id = s.workstream_id
                JOIN "ob-poc".cases k ON k.case_id = ew.case_id
                WHERE k.cbu_id = c.cbu_id
                  AND s.status = 'CLEAR'
            ) AS screening_complete,
            false AS has_ubo,
            false AS has_trading
        FROM "ob-poc".cbus c
        WHERE c.cbu_id = ANY($1)
          AND c.deleted_at IS NULL
        "#,
    )
    .bind(cbu_ids)
    .fetch_all(pool)
    .await?;

    let cbu_count = rows.len();
    let mut domain_counts = std::collections::HashMap::new();
    let mut cbu_states = Vec::with_capacity(cbu_count);

    let mut total_cases = 0usize;
    let mut total_screenings = 0usize;
    let _total_trading = 0usize;

    for row in &rows {
        let has_kyc_case = row.case_count.unwrap_or(0) > 0;
        let has_screening = row.has_screening.unwrap_or(false);
        let screening_complete = row.screening_complete.unwrap_or(false);
        if has_kyc_case {
            total_cases += 1;
        }
        if has_screening {
            total_screenings += 1;
        }

        cbu_states.push(CbuStateSummary {
            cbu_id: row.cbu_id.to_string(),
            cbu_name: row.cbu_name.clone(),
            lifecycle_state: row.lifecycle_state.clone(),
            has_kyc_case,
            kyc_case_status: row.latest_case_status.clone(),
            has_screening,
            screening_complete,
            document_coverage_pct: None, // TODO: compute from document_requirements
        });
    }

    domain_counts.insert("kyc_case".into(), total_cases);
    domain_counts.insert("screening".into(), total_screenings);

    let mut state = GroupCompositeState {
        cbu_count,
        domain_counts,
        has_ubo_determination: has_ubo,
        has_control_chain: has_control,
        cbu_states,
        next_likely_verbs: Vec::new(),
        blocked_verbs: Vec::new(),
    };

    // Derive next-likely verbs from the as-is state
    state.derive_next_likely_verbs();

    Ok(Some(state))
}

#[cfg(feature = "database")]
#[derive(sqlx::FromRow)]
struct CbuStateRow {
    cbu_id: Uuid,
    cbu_name: Option<String>,
    lifecycle_state: Option<String>,
    case_count: Option<i32>,
    latest_case_status: Option<String>,
    has_screening: Option<bool>,
    screening_complete: Option<bool>,
    #[allow(dead_code)]
    has_ubo: Option<bool>,
    #[allow(dead_code)]
    has_trading: Option<bool>,
}
