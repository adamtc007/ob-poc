//! Agent enrichment — computes onboarding state view and verb surface
//! for responses routed through the REPL V2 orchestrator.
//!
//! These functions extract capabilities from the agent pipeline (Sage narration,
//! SemOS governance, constellation state) and make them available to the
//! response adapter without requiring UnifiedSession.

use sqlx::PgPool;
use uuid::Uuid;

/// Compute the onboarding state view for the current group scope.
///
/// Loads CBU IDs from the scope graph, then builds the composite state
/// and projects the onboarding view. Returns `None` if no CBUs are in scope.
#[cfg(feature = "database")]
pub async fn compute_onboarding_state(
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

    // Load group composite state
    match crate::agent::composite_state_loader::load_group_composite_state(pool, &cbu_ids).await {
        Ok(Some(composite)) => Some(
            crate::agent::onboarding_state_view::project_onboarding_state(
                &composite, group_name,
            ),
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
