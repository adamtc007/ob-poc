//! Agent enrichment — computes onboarding state view and verb surface
//! for responses routed through the REPL V2 orchestrator.
//!
//! These functions extract capabilities from the agent pipeline (Sage narration,
//! SemOS governance, constellation state) and make them available to the
//! response adapter without requiring UnifiedSession.

use sqlx::PgPool;
use uuid::Uuid;

use crate::repl::response_v2::ReplResponseV2;

/// Try to extract onboarding state from the REPL response's session feedback.
///
/// This reads from the hydrated constellation on the session's TOS — the same
/// DAG the compiler and narration engine use. Avoids the raw SQL path in
/// `composite_state_loader.rs` (SE-10/SE-11 in the audit).
///
/// Returns `None` if the response doesn't carry session feedback with a
/// hydrated constellation (e.g., during tollgate states before workspace selection).
/// In that case the caller should fall back to `compute_onboarding_state_from_db`.
pub fn try_onboarding_from_repl_response(
    response: &ReplResponseV2,
    group_name: Option<&str>,
) -> Option<ob_poc_types::onboarding_state::OnboardingStateView> {
    let feedback = response.session_feedback.as_ref()?;
    let constellation = feedback.tos.hydrated_constellation.as_ref()?;

    // Project from hydrated slots — same data the narration engine reads.
    // We build a lightweight GroupCompositeState from the constellation slots
    // rather than querying the database independently.
    let composite = crate::agent::composite_state::GroupCompositeState::from_hydrated_constellation(
        constellation,
    );

    Some(crate::agent::onboarding_state_view::project_onboarding_state(&composite, group_name))
}

/// Compute the onboarding state view from the database (legacy path).
///
/// TRANSITIONAL: This is the SE-10/SE-11 side entrance — it loads CBU state
/// from raw SQL queries instead of reading from the session's hydrated DAG.
/// Kept as a fallback for when the REPL response doesn't carry a hydrated
/// constellation (pre-workspace states). Use `try_onboarding_from_repl_response`
/// as the preferred path.
#[cfg(feature = "database")]
pub async fn compute_onboarding_state_from_db(
    pool: &PgPool,
    session_id: Uuid,
    group_name: Option<&str>,
) -> Option<ob_poc_types::onboarding_state::OnboardingStateView> {
    // Load CBU IDs from the scope graph (same query as get_session_scope_graph)
    let cbu_ids = match sqlx::query_scalar::<_, Uuid>(
        r#"SELECT cbu_id FROM "ob-poc".session_cbus WHERE session_id = $1"#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    {
        Ok(ids) if !ids.is_empty() => ids,
        _ => return None,
    };

    // Load group composite state from DB (legacy — SE-10)
    match crate::agent::composite_state_loader::load_group_composite_state(pool, &cbu_ids).await {
        Ok(Some(composite)) => Some(
            crate::agent::onboarding_state_view::project_onboarding_state(&composite, group_name),
        ),
        _ => None,
    }
}

/// Enrichment data to merge into a ChatResponse after REPL processing.
#[derive(Default)]
pub struct ResponseEnrichment {
    pub onboarding_state: Option<ob_poc_types::onboarding_state::OnboardingStateView>,
    pub available_verbs: Option<Vec<ob_poc_types::chat::VerbProfile>>,
    pub surface_fingerprint: Option<String>,
}
