//! Guard Evaluation
//!
//! Guards now evaluate requirements from YAML definitions.
//! Two-phase evaluation:
//! 1. Evaluate requirements for target state from workflow definition
//! 2. If transition has a named custom guard, also run that
//!
//! This makes guards data-driven - add requirements to YAML, not code.

use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use super::definition::WorkflowDefinition;
use super::requirements::RequirementEvaluator;
use super::state::{Blocker, BlockerType};
use super::WorkflowError;

/// Evaluates transition guards against the database
pub struct GuardEvaluator {
    pool: PgPool,
    requirement_evaluator: RequirementEvaluator,
    definitions: Arc<HashMap<String, WorkflowDefinition>>,
}

impl GuardEvaluator {
    /// Create a new guard evaluator with workflow definitions
    pub fn new(pool: PgPool, definitions: Arc<HashMap<String, WorkflowDefinition>>) -> Self {
        Self {
            requirement_evaluator: RequirementEvaluator::new(pool.clone()),
            pool,
            definitions,
        }
    }

    /// Evaluate guard for a transition
    ///
    /// Two-phase evaluation:
    /// 1. Evaluate requirements for the TARGET state from YAML
    /// 2. If transition has a named custom guard, also run that
    pub async fn evaluate_for_transition(
        &self,
        workflow_id: &str,
        from_state: &str,
        to_state: &str,
        subject_id: Uuid,
        subject_type: &str,
    ) -> Result<GuardResult, WorkflowError> {
        let definition = match self.definitions.get(workflow_id) {
            Some(d) => d,
            None => {
                return Ok(GuardResult::failed(format!(
                    "Unknown workflow: {}",
                    workflow_id
                )))
            }
        };

        let mut all_blockers = Vec::new();

        // 1. Evaluate requirements for TARGET state
        if let Some(requirements) = definition.requirements.get(to_state) {
            let blockers = self
                .requirement_evaluator
                .evaluate_all(requirements, subject_id)
                .await?;
            all_blockers.extend(blockers);
        }

        // 2. If transition has a named guard, also run that
        if let Some(transition) = definition.get_transition(from_state, to_state) {
            if let Some(guard_name) = &transition.guard {
                let custom_result = self
                    .evaluate_custom_guard(guard_name, subject_id, subject_type)
                    .await?;
                all_blockers.extend(custom_result.blockers);
            }
        }

        if all_blockers.is_empty() {
            Ok(GuardResult::passed())
        } else {
            Ok(GuardResult::blocked(all_blockers))
        }
    }

    /// Legacy: Evaluate a guard by name only (for backward compatibility)
    pub async fn evaluate(
        &self,
        guard_name: &str,
        subject_id: Uuid,
        subject_type: &str,
    ) -> Result<GuardResult, sqlx::Error> {
        self.evaluate_custom_guard(guard_name, subject_id, subject_type)
            .await
            .map_err(|e| match e {
                WorkflowError::Database(e) => e,
                _ => sqlx::Error::Protocol(e.to_string()),
            })
    }

    /// Evaluate a custom named guard (for complex logic not expressible as requirements)
    async fn evaluate_custom_guard(
        &self,
        guard_name: &str,
        subject_id: Uuid,
        _subject_type: &str,
    ) -> Result<GuardResult, WorkflowError> {
        match guard_name {
            // Case status checks (can't be expressed as simple requirements)
            "review_approved" => self.check_case_status(subject_id, "APPROVED").await,
            "review_rejected" => self.check_case_status(subject_id, "REJECTED").await,

            // Legacy guards - now mostly handled by requirements, but keep for compatibility
            "entities_complete" => self.check_entities_complete(subject_id).await,
            "screening_complete" => self.check_screening_complete(subject_id).await,
            "documents_complete" => self.check_documents_complete(subject_id).await,
            "ubo_complete" => self.check_ubo_complete(subject_id).await,

            _ => {
                // Unknown guard - log warning but don't block
                tracing::warn!("Unknown custom guard: {}", guard_name);
                Ok(GuardResult::passed())
            }
        }
    }

    /// Check if case has specific status
    async fn check_case_status(
        &self,
        cbu_id: Uuid,
        required_status: &str,
    ) -> Result<GuardResult, WorkflowError> {
        let case_status: Option<String> = sqlx::query_scalar(
            r#"
            SELECT status FROM kyc.cases
            WHERE cbu_id = $1
            ORDER BY opened_at DESC
            LIMIT 1
            "#,
        )
        .bind(cbu_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        if case_status.as_deref() == Some(required_status) {
            Ok(GuardResult::passed())
        } else {
            Ok(GuardResult::blocked(vec![Blocker::new(
                BlockerType::ManualApprovalRequired,
                format!("Case must be {}", required_status),
            )
            .with_resolution("kyc-case.update-status")]))
        }
    }

    // --- Legacy guard implementations (for backward compatibility) ---
    // These are now mostly replaced by YAML requirements but kept for
    // workflows that still use named guards directly

    async fn check_entities_complete(&self, cbu_id: Uuid) -> Result<GuardResult, WorkflowError> {
        let mut blockers = Vec::new();

        // Check for at least one director
        let director_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM "ob-poc".cbu_entity_roles cer
            JOIN "ob-poc".roles r ON cer.role_id = r.role_id
            WHERE cer.cbu_id = $1
            AND r.name = 'DIRECTOR'
            "#,
        )
        .bind(cbu_id)
        .fetch_one(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        if director_count < 1 {
            blockers.push(
                Blocker::new(
                    BlockerType::MissingRole {
                        role: "DIRECTOR".to_string(),
                        required: 1,
                        current: director_count as u32,
                    },
                    "At least one director required",
                )
                .with_resolution("cbu.assign-role"),
            );
        }

        // Check for at least one authorized signatory
        let sig_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM "ob-poc".cbu_entity_roles cer
            JOIN "ob-poc".roles r ON cer.role_id = r.role_id
            WHERE cer.cbu_id = $1
            AND r.name = 'AUTHORIZED_SIGNATORY'
            "#,
        )
        .bind(cbu_id)
        .fetch_one(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        if sig_count < 1 {
            blockers.push(
                Blocker::new(
                    BlockerType::MissingRole {
                        role: "AUTHORIZED_SIGNATORY".to_string(),
                        required: 1,
                        current: sig_count as u32,
                    },
                    "At least one authorized signatory required",
                )
                .with_resolution("cbu.assign-role"),
            );
        }

        if blockers.is_empty() {
            Ok(GuardResult::passed())
        } else {
            Ok(GuardResult::blocked(blockers))
        }
    }

    async fn check_screening_complete(&self, cbu_id: Uuid) -> Result<GuardResult, WorkflowError> {
        // Find entities linked to this CBU that need screening
        let unscreened: Vec<(Uuid, String)> = sqlx::query_as(
            r#"
            SELECT e.entity_id, e.name
            FROM "ob-poc".entities e
            JOIN "ob-poc".cbu_entity_roles cer ON e.entity_id = cer.entity_id
            WHERE cer.cbu_id = $1
            AND NOT EXISTS (
                SELECT 1 FROM "ob-poc".screenings s
                WHERE s.entity_id = e.entity_id
                AND s.screened_at > NOW() - INTERVAL '90 days'
            )
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        if unscreened.is_empty() {
            // Also check for unresolved alerts via KYC screenings
            let open_alerts: Vec<(Uuid, Uuid)> = sqlx::query_as(
                r#"
                SELECT s.screening_id, ew.entity_id
                FROM kyc.screenings s
                JOIN kyc.entity_workstreams ew ON s.workstream_id = ew.workstream_id
                JOIN kyc.cases c ON ew.case_id = c.case_id
                WHERE c.cbu_id = $1
                AND s.status = 'HIT_PENDING_REVIEW'
                "#,
            )
            .bind(cbu_id)
            .fetch_all(&self.pool)
            .await
            .map_err(WorkflowError::Database)?;

            if open_alerts.is_empty() {
                return Ok(GuardResult::passed());
            }

            let blockers = open_alerts
                .iter()
                .map(|(alert_id, entity_id)| {
                    Blocker::new(
                        BlockerType::UnresolvedAlert {
                            alert_id: *alert_id,
                            entity_id: *entity_id,
                        },
                        "Unresolved screening alert",
                    )
                    .with_resolution("case-screening.review-hit")
                    .with_detail("screening_id", serde_json::json!(alert_id))
                })
                .collect();

            return Ok(GuardResult::blocked(blockers));
        }

        let blockers = unscreened
            .iter()
            .map(|(id, name)| {
                Blocker::new(
                    BlockerType::PendingScreening { entity_id: *id },
                    format!("Screening required for {}", name),
                )
                .with_resolution("case-screening.run")
                .with_detail("entity_id", serde_json::json!(id))
            })
            .collect();

        Ok(GuardResult::blocked(blockers))
    }

    async fn check_documents_complete(&self, cbu_id: Uuid) -> Result<GuardResult, WorkflowError> {
        let mut blockers = Vec::new();

        // Check CBU-level required documents
        let required_cbu_docs = [
            "CERTIFICATE_OF_INCORPORATION",
            "REGISTER_OF_DIRECTORS",
            "REGISTER_OF_SHAREHOLDERS",
        ];

        for doc_type in required_cbu_docs {
            let exists: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM "ob-poc".document_catalog d
                    WHERE d.cbu_id = $1
                    AND d.document_type_code = $2
                    AND d.status = 'active'
                )
                "#,
            )
            .bind(cbu_id)
            .bind(doc_type)
            .fetch_one(&self.pool)
            .await
            .map_err(WorkflowError::Database)?;

            if !exists {
                blockers.push(
                    Blocker::new(
                        BlockerType::MissingDocument {
                            document_type: doc_type.to_string(),
                            for_entity: None,
                        },
                        format!("{} required", doc_type.replace('_', " ").to_lowercase()),
                    )
                    .with_resolution("document.catalog"),
                );
            }
        }

        // Check per-director documents (passport, proof of address)
        let directors: Vec<(Uuid, String)> = sqlx::query_as(
            r#"
            SELECT e.entity_id, e.name
            FROM "ob-poc".entities e
            JOIN "ob-poc".cbu_entity_roles cer ON e.entity_id = cer.entity_id
            JOIN "ob-poc".roles r ON cer.role_id = r.role_id
            WHERE cer.cbu_id = $1
            AND r.name = 'DIRECTOR'
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        for (director_id, director_name) in directors {
            // Check passport
            let has_passport: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM "ob-poc".document_catalog d
                    JOIN "ob-poc".cbu_entity_roles cer ON d.cbu_id = cer.cbu_id
                    WHERE cer.entity_id = $1
                    AND d.document_type_code = 'PASSPORT'
                    AND d.status = 'active'
                )
                "#,
            )
            .bind(director_id)
            .fetch_one(&self.pool)
            .await
            .map_err(WorkflowError::Database)?;

            if !has_passport {
                blockers.push(
                    Blocker::new(
                        BlockerType::MissingDocument {
                            document_type: "PASSPORT".to_string(),
                            for_entity: Some(director_id),
                        },
                        format!("Passport required for {}", director_name),
                    )
                    .with_resolution("document.catalog")
                    .with_detail("entity_id", serde_json::json!(director_id)),
                );
            }
        }

        if blockers.is_empty() {
            Ok(GuardResult::passed())
        } else {
            Ok(GuardResult::blocked(blockers))
        }
    }

    async fn check_ubo_complete(&self, cbu_id: Uuid) -> Result<GuardResult, WorkflowError> {
        let mut blockers = Vec::new();

        // Check ownership totals to approximately 100%
        let total_ownership: Option<rust_decimal::Decimal> = sqlx::query_scalar(
            r#"
            SELECT SUM(ownership_percent) FROM "ob-poc".ownership_relationships o
            JOIN "ob-poc".cbu_entity_roles cer ON o.owned_entity_id = cer.entity_id
            WHERE cer.cbu_id = $1
            AND (o.effective_to IS NULL OR o.effective_to > CURRENT_DATE)
            "#,
        )
        .bind(cbu_id)
        .fetch_one(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        let total: f64 = total_ownership
            .map(|d| d.to_string().parse().unwrap_or(0.0))
            .unwrap_or(0.0);

        if (total - 100.0).abs() > 0.01 && total > 0.0 {
            blockers.push(
                Blocker::new(
                    BlockerType::IncompleteOwnership {
                        current_total: total,
                        required: 100.0,
                    },
                    format!("Ownership structure incomplete ({:.1}% of 100%)", total),
                )
                .with_resolution("ubo.add-ownership"),
            );
        }

        // Check all UBOs are verified
        let unverified_ubos: Vec<(Uuid, String)> = sqlx::query_as(
            r#"
            SELECT u.ubo_id, e.name
            FROM "ob-poc".ubo_registry u
            JOIN "ob-poc".entities e ON u.ubo_person_id = e.entity_id
            WHERE u.cbu_id = $1
            AND u.verification_status NOT IN ('VERIFIED', 'PROVEN')
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        for (ubo_id, name) in unverified_ubos {
            blockers.push(
                Blocker::new(
                    BlockerType::UnverifiedUbo {
                        ubo_id,
                        person_name: name.clone(),
                    },
                    format!("UBO verification required for {}", name),
                )
                .with_resolution("ubo.verify-ubo")
                .with_detail("ubo_id", serde_json::json!(ubo_id)),
            );
        }

        if blockers.is_empty() {
            Ok(GuardResult::passed())
        } else {
            Ok(GuardResult::blocked(blockers))
        }
    }
}

/// Result of guard evaluation
#[derive(Debug)]
pub struct GuardResult {
    /// Did the guard pass?
    pub passed: bool,
    /// Blockers if not passed
    pub blockers: Vec<Blocker>,
}

impl GuardResult {
    /// Guard passed
    pub fn passed() -> Self {
        Self {
            passed: true,
            blockers: vec![],
        }
    }

    /// Guard blocked with specific blockers
    pub fn blocked(blockers: Vec<Blocker>) -> Self {
        Self {
            passed: false,
            blockers,
        }
    }

    /// Guard failed with error
    pub fn failed(reason: String) -> Self {
        Self {
            passed: false,
            blockers: vec![Blocker::new(
                BlockerType::Custom {
                    code: "GUARD_ERROR".to_string(),
                },
                reason,
            )],
        }
    }
}
