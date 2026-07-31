//! Disambiguation prompts and escalation records.
//!
//! When the agent encounters ambiguity or insufficient confidence,
//! it records a `DisambiguationPrompt` for human clarification.
//! When a decision requires human intervention, an `EscalationRecord`
//! is created with context and required actions.
//!
//! Both are INSERT-only from this module's application-level API --
//! `mcp_tools.rs`'s `sem_reg_record_escalation` / `sem_reg_record_disambiguation`
//! MCP tools are the only production callers, both write-only. The read/update
//! CRUD half (answer/resolve/load/list) was removed as dead code (zero callers,
//! including tests) in Phase 14 dead-code remediation; the `resolved_at`/
//! `resolution` columns remain UPDATE-able directly in SQL if that surface is
//! ever needed again.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

// ── Agent Disambiguation Prompt ───────────────────────────────

/// A disambiguation prompt created by the agent when context is ambiguous.
///
/// Named `AgentDisambiguationPrompt` to avoid collision with the
/// Phase 7 `DisambiguationPrompt` type in context_resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AgentDisambiguationPrompt {
    /// Unique prompt identifier.
    pub prompt_id: Uuid,
    /// Decision that triggered this prompt.
    #[serde(default)]
    pub decision_id: Option<Uuid>,
    /// Plan this prompt relates to.
    #[serde(default)]
    pub plan_id: Option<Uuid>,
    /// The question being asked.
    pub question: String,
    /// Available options for the human to choose from.
    pub options: Vec<PromptOption>,
    /// Context snapshot at the time of the prompt.
    #[serde(default)]
    pub context_snapshot: Option<serde_json::Value>,
    /// Whether the prompt has been answered.
    #[serde(default)]
    pub answered: bool,
    /// The chosen option (populated after answer).
    #[serde(default)]
    pub chosen_option: Option<String>,
    /// Who answered.
    #[serde(default)]
    pub answered_by: Option<String>,
    /// When answered.
    #[serde(default)]
    pub answered_at: Option<DateTime<Utc>>,
    /// When created.
    pub created_at: DateTime<Utc>,
}

/// An option in a disambiguation prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PromptOption {
    /// Option identifier.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Description of what this option means.
    #[serde(default)]
    pub description: Option<String>,
}

// ── Agent Escalation Record ───────────────────────────────────

/// An escalation record when the agent requires human intervention.
///
/// Escalations are created when confidence is too low, when policy
/// requires human approval, or when the agent encounters a situation
/// it cannot resolve autonomously.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AgentEscalationRecord {
    /// Unique escalation identifier.
    pub escalation_id: Uuid,
    /// Decision that triggered this escalation.
    #[serde(default)]
    pub decision_id: Option<Uuid>,
    /// Reason for escalation.
    pub reason: String,
    /// Severity: `info`, `warning`, `critical`.
    pub severity: String,
    /// Context snapshot at escalation time.
    #[serde(default)]
    pub context_snapshot: Option<serde_json::Value>,
    /// What human action is required.
    pub required_human_action: String,
    /// Who it was assigned to (if any).
    #[serde(default)]
    pub assigned_to: Option<String>,
    /// When it was resolved (UPDATE-only field).
    #[serde(default)]
    pub resolved_at: Option<DateTime<Utc>>,
    /// Resolution description (UPDATE-only field).
    #[serde(default)]
    pub resolution: Option<String>,
    /// Who created this escalation.
    pub created_by: String,
    /// When created.
    pub created_at: DateTime<Utc>,
}

// ── Escalation Store ──────────────────────────────────────────

/// Database operations for disambiguation prompts and escalation records.
pub(crate) struct EscalationStore;

impl EscalationStore {
    // ── Disambiguation Prompts ────────────────────────────────

    /// Insert a new disambiguation prompt (immutable INSERT).
    pub(crate) async fn insert_prompt(pool: &PgPool, prompt: &AgentDisambiguationPrompt) -> Result<Uuid> {
        let options_json = serde_json::to_value(&prompt.options)?;

        let prompt_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO sem_reg.disambiguation_prompts (
                prompt_id, decision_id, plan_id,
                question, options, context_snapshot,
                answered, chosen_option, answered_by, answered_at
            ) VALUES (
                $1, $2, $3,
                $4, $5, $6,
                $7, $8, $9, $10
            )
            RETURNING prompt_id
            "#,
        )
        .bind(prompt.prompt_id)
        .bind(prompt.decision_id)
        .bind(prompt.plan_id)
        .bind(&prompt.question)
        .bind(&options_json)
        .bind(&prompt.context_snapshot)
        .bind(prompt.answered)
        .bind(&prompt.chosen_option)
        .bind(&prompt.answered_by)
        .bind(prompt.answered_at)
        .fetch_one(pool)
        .await?;

        Ok(prompt_id)
    }

    // ── Escalation Records ────────────────────────────────────

    /// Insert a new escalation record.
    pub(crate) async fn insert_escalation(pool: &PgPool, record: &AgentEscalationRecord) -> Result<Uuid> {
        let escalation_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO sem_reg.escalation_records (
                escalation_id, decision_id, reason, severity,
                context_snapshot, required_human_action,
                assigned_to, created_by
            ) VALUES (
                $1, $2, $3, $4,
                $5, $6,
                $7, $8
            )
            RETURNING escalation_id
            "#,
        )
        .bind(record.escalation_id)
        .bind(record.decision_id)
        .bind(&record.reason)
        .bind(&record.severity)
        .bind(&record.context_snapshot)
        .bind(&record.required_human_action)
        .bind(&record.assigned_to)
        .bind(&record.created_by)
        .fetch_one(pool)
        .await?;

        Ok(escalation_id)
    }

}

// ── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disambiguation_prompt_serde() {
        let prompt = AgentDisambiguationPrompt {
            prompt_id: Uuid::new_v4(),
            decision_id: Some(Uuid::new_v4()),
            plan_id: None,
            question: "Which entity type should we create?".into(),
            options: vec![
                PromptOption {
                    id: "proper_person".into(),
                    label: "Natural Person".into(),
                    description: Some("A human individual".into()),
                },
                PromptOption {
                    id: "legal_entity".into(),
                    label: "Legal Entity".into(),
                    description: Some("A company or organization".into()),
                },
            ],
            context_snapshot: Some(serde_json::json!({"case_id": "abc"})),
            answered: false,
            chosen_option: None,
            answered_by: None,
            answered_at: None,
            created_at: Utc::now(),
        };

        let json = serde_json::to_value(&prompt).unwrap();
        let round: AgentDisambiguationPrompt = serde_json::from_value(json).unwrap();
        assert_eq!(round.prompt_id, prompt.prompt_id);
        assert_eq!(round.options.len(), 2);
        assert!(!round.answered);
    }

    #[test]
    fn test_escalation_record_serde() {
        let record = AgentEscalationRecord {
            escalation_id: Uuid::new_v4(),
            decision_id: Some(Uuid::new_v4()),
            reason: "Confidence below threshold".into(),
            severity: "warning".into(),
            context_snapshot: Some(serde_json::json!({"verbs": ["ubo.discover"]})),
            required_human_action: "Verify UBO structure manually".into(),
            assigned_to: Some("compliance-team".into()),
            resolved_at: None,
            resolution: None,
            created_by: "agent-1".into(),
            created_at: Utc::now(),
        };

        let json = serde_json::to_value(&record).unwrap();
        let round: AgentEscalationRecord = serde_json::from_value(json).unwrap();
        assert_eq!(round.escalation_id, record.escalation_id);
        assert_eq!(round.reason, "Confidence below threshold");
        assert_eq!(round.severity, "warning");
        assert!(round.resolved_at.is_none());
    }

    #[test]
    fn test_prompt_option_serde() {
        let opt = PromptOption {
            id: "option_a".into(),
            label: "Option A".into(),
            description: None,
        };
        let json = serde_json::to_value(&opt).unwrap();
        let round: PromptOption = serde_json::from_value(json).unwrap();
        assert_eq!(round.id, "option_a");
        assert!(round.description.is_none());
    }

    #[test]
    fn test_escalation_resolved() {
        let record = AgentEscalationRecord {
            escalation_id: Uuid::new_v4(),
            decision_id: None,
            reason: "Manual review required".into(),
            severity: "critical".into(),
            context_snapshot: None,
            required_human_action: "Approve sanctions screening result".into(),
            assigned_to: Some("john.doe@bank.com".into()),
            resolved_at: Some(Utc::now()),
            resolution: Some("Approved after manual review".into()),
            created_by: "agent-1".into(),
            created_at: Utc::now(),
        };

        assert!(record.resolved_at.is_some());
        assert_eq!(
            record.resolution.as_deref(),
            Some("Approved after manual review")
        );
    }
}
