//! Write-only store for intent telemetry events.
//! Best-effort: failures are logged, never propagated to callers.

#[cfg(feature = "database")]
use sqlx::PgPool;

use super::IntentEventRow;

/// Insert an intent event row. Returns Ok(true) on success, Ok(false) on failure.
/// Never returns Err — telemetry must not break the pipeline.
#[cfg(feature = "database")]
pub async fn insert_intent_event(pool: &PgPool, row: &IntentEventRow) -> bool {
    let result = sqlx::query(
        r#"
        INSERT INTO agent.intent_events (
            event_id, session_id, actor_id, entrypoint,
            utterance_hash, utterance_preview, scope,
            subject_ref_type, subject_ref_id,
            semreg_mode, semreg_denied_verbs,
            verb_candidates_pre, verb_candidates_post,
            chosen_verb_fqn, selection_source, forced_verb_fqn,
            outcome, dsl_hash, run_sheet_entry_id,
            macro_semreg_checked, macro_denied_verbs,
            prompt_version, error_code,
            dominant_entity_id, dominant_entity_kind, entity_kind_filtered
        ) VALUES (
            $1, $2, $3, $4,
            $5, $6, $7,
            $8, $9,
            $10, $11,
            $12, $13,
            $14, $15, $16,
            $17, $18, $19,
            $20, $21,
            $22, $23,
            $24, $25, $26
        )
        "#,
    )
    .bind(row.event_id)
    .bind(row.session_id)
    .bind(&row.actor_id)
    .bind(&row.entrypoint)
    .bind(&row.utterance_hash)
    .bind(&row.utterance_preview)
    .bind(&row.scope)
    .bind(&row.subject_ref_type)
    .bind(row.subject_ref_id)
    .bind(&row.semreg_mode)
    .bind(&row.semreg_denied_verbs)
    .bind(&row.verb_candidates_pre)
    .bind(&row.verb_candidates_post)
    .bind(&row.chosen_verb_fqn)
    .bind(&row.selection_source)
    .bind(&row.forced_verb_fqn)
    .bind(&row.outcome)
    .bind(&row.dsl_hash)
    .bind(row.run_sheet_entry_id)
    .bind(row.macro_semreg_checked)
    .bind(&row.macro_denied_verbs)
    .bind(&row.prompt_version)
    .bind(&row.error_code)
    .bind(row.dominant_entity_id)
    .bind(&row.dominant_entity_kind)
    .bind(row.entity_kind_filtered)
    .execute(pool)
    .await;

    match result {
        Ok(_) => {
            tracing::debug!(event_id = %row.event_id, "Intent telemetry event persisted");
            true
        }
        Err(e) => {
            tracing::warn!(
                event_id = %row.event_id,
                error = %e,
                "Failed to persist intent telemetry event (non-fatal)"
            );
            false
        }
    }
}

#[cfg(test)]
mod tests {
    /// Static guard: insert_intent_event must only be called from the orchestrator module.
    /// This test greps the source tree to ensure no other module calls it.
    #[test]
    fn test_insert_intent_event_only_called_from_orchestrator() {
        use std::process::Command;

        let output = Command::new("grep")
            .args(["-rn", "insert_intent_event", "src/"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .expect("grep failed");

        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout
            .lines()
            .filter(|l| !l.contains("mod tests") && !l.contains("//") && !l.contains("#["))
            .filter(|l| !l.contains("pub async fn insert_intent_event")) // definition
            .filter(|l| !l.contains("fn test_")) // test functions
            .collect();

        for line in &lines {
            assert!(
                line.starts_with("src/agent/orchestrator.rs")
                    || line.starts_with("src/agent/telemetry/"),
                "insert_intent_event called outside orchestrator/telemetry: {}",
                line
            );
        }
    }
}
