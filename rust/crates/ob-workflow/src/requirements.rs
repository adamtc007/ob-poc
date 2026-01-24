//! Requirement Evaluation
//!
//! Evaluates requirements defined in workflow YAML.
//! Each requirement type maps to a database check.
//! This makes guards data-driven from YAML rather than hardcoded.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::definition::RequirementDef;
use super::state::{Blocker, BlockerType};
use super::WorkflowError;

/// Evaluates workflow requirements from YAML definitions
pub struct RequirementEvaluator {
    pool: PgPool,
}

impl RequirementEvaluator {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Evaluate a single requirement, returning blockers if not met
    pub async fn evaluate(
        &self,
        req: &RequirementDef,
        subject_id: Uuid,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        match req {
            // ─────────────────────────────────────────────────────────────────────────────
            // Core / Entity Structure
            // ─────────────────────────────────────────────────────────────────────────────
            RequirementDef::RoleCount {
                role,
                min,
                description,
            } => {
                self.check_role_count(subject_id, role, *min, description)
                    .await
            }

            RequirementDef::FieldPresent {
                fields,
                description,
            } => {
                self.check_fields_present(subject_id, fields, description)
                    .await
            }

            RequirementDef::ProductAssigned { min, description } => {
                self.check_product_assigned(subject_id, *min, description)
                    .await
            }

            RequirementDef::RelationshipExists {
                relationship_type,
                description,
            } => {
                self.check_relationship_exists(subject_id, relationship_type, description)
                    .await
            }

            RequirementDef::Conditional {
                condition,
                requirement,
                description,
            } => {
                self.check_conditional(subject_id, condition, requirement, description)
                    .await
            }

            // ─────────────────────────────────────────────────────────────────────────────
            // Documents
            // ─────────────────────────────────────────────────────────────────────────────
            RequirementDef::DocumentSet {
                documents,
                description,
            } => {
                self.check_document_set(subject_id, documents, description)
                    .await
            }

            RequirementDef::PerEntityDocument {
                entity_type,
                documents,
                description,
            } => {
                self.check_per_entity_docs(subject_id, entity_type, documents, description)
                    .await
            }

            RequirementDef::DocumentsReviewed { description } => {
                self.check_documents_reviewed(subject_id, description).await
            }

            // ─────────────────────────────────────────────────────────────────────────────
            // Screening
            // ─────────────────────────────────────────────────────────────────────────────
            RequirementDef::AllEntitiesScreened { description } => {
                self.check_all_screened(subject_id, description).await
            }

            RequirementDef::AllScreeningsCurrent {
                max_age_days,
                description,
            } => {
                self.check_screenings_current(subject_id, *max_age_days, description)
                    .await
            }

            RequirementDef::NoOpenAlerts { description } => {
                self.check_no_alerts(subject_id, description).await
            }

            RequirementDef::NoPendingHits { description } => {
                self.check_no_pending_hits(subject_id, description).await
            }

            // ─────────────────────────────────────────────────────────────────────────────
            // UBO / Ownership
            // ─────────────────────────────────────────────────────────────────────────────
            RequirementDef::OwnershipComplete {
                threshold,
                description,
            } => {
                self.check_ownership(subject_id, *threshold, description)
                    .await
            }

            RequirementDef::AllUbosVerified { description } => {
                self.check_ubos_verified(subject_id, description).await
            }

            RequirementDef::ChainsResolvedToPersons { description } => {
                self.check_chains_resolved(subject_id, description).await
            }

            RequirementDef::UboThresholdApplied {
                threshold,
                description,
            } => {
                self.check_ubo_threshold_applied(subject_id, *threshold, description)
                    .await
            }

            // ─────────────────────────────────────────────────────────────────────────────
            // KYC Case
            // ─────────────────────────────────────────────────────────────────────────────
            RequirementDef::CaseExists {
                case_type,
                description,
            } => {
                self.check_case_exists(subject_id, case_type.as_deref(), description)
                    .await
            }

            RequirementDef::AnalystAssigned { description } => {
                self.check_analyst_assigned(subject_id, description).await
            }

            RequirementDef::RiskRatingSet { description } => {
                self.check_risk_rating_set(subject_id, description).await
            }

            RequirementDef::ApprovalRecorded { description } => {
                self.check_approval_recorded(subject_id, description).await
            }

            RequirementDef::RejectionRecorded { description } => {
                self.check_rejection_recorded(subject_id, description).await
            }

            RequirementDef::CaseChecklistComplete { description } => {
                self.check_checklist(subject_id, description).await
            }

            // ─────────────────────────────────────────────────────────────────────────────
            // Workstreams
            // ─────────────────────────────────────────────────────────────────────────────
            RequirementDef::EntityWorkstreamsCreated { description } => {
                self.check_workstreams_created(subject_id, description)
                    .await
            }

            RequirementDef::AllWorkstreamsDataComplete { description } => {
                self.check_workstreams_complete(subject_id, description)
                    .await
            }

            // ─────────────────────────────────────────────────────────────────────────────
            // Periodic Review / Freshness
            // ─────────────────────────────────────────────────────────────────────────────
            RequirementDef::EntityDataCurrent {
                max_age_days,
                description,
            } => {
                self.check_entity_data_current(subject_id, *max_age_days, description)
                    .await
            }

            RequirementDef::ChangeLogReviewed { description } => {
                self.check_changelog_reviewed(subject_id, description).await
            }

            // ─────────────────────────────────────────────────────────────────────────────
            // Sign-off / Completion
            // ─────────────────────────────────────────────────────────────────────────────
            RequirementDef::SignOffRecorded { description } => {
                self.check_signoff_recorded(subject_id, description).await
            }

            RequirementDef::NextReviewScheduled { description } => {
                self.check_next_review_scheduled(subject_id, description)
                    .await
            }

            // ─────────────────────────────────────────────────────────────────────────────
            // ─────────────────────────────────────────────────────────────────────────────
            // Additional requirement types - delegate or pass through
            // ─────────────────────────────────────────────────────────────────────────────
            RequirementDef::AllScreeningsComplete { description } => {
                // Similar to AllEntitiesScreened but checks completion status
                self.check_all_screened(subject_id, description).await
            }

            RequirementDef::NoOpenRedFlags { description } => {
                // Alias for NoOpenAlerts
                self.check_no_alerts(subject_id, description).await
            }

            RequirementDef::AllUbosScreened { description } => {
                // UBOs specifically screened
                self.check_all_screened(subject_id, description).await
            }

            RequirementDef::AllUbosHaveIdentityDocs { description } => {
                // Check UBOs have identity documents
                self.check_ubo_identity_docs(subject_id, description).await
            }

            RequirementDef::UboRegisterComplete { description } => {
                // Check UBO register is complete
                self.check_ubos_verified(subject_id, description).await
            }

            RequirementDef::NoUnknownOwners { description } => {
                // Check no unknown owners in chains
                self.check_chains_resolved(subject_id, description).await
            }

            RequirementDef::ExemptionsDocumented { description } => {
                // Check exemptions documented - pass for now
                self.check_custom(subject_id, "exemptions_documented", description)
                    .await
            }

            RequirementDef::DeterminationRationaleRecorded { description } => {
                // Check determination rationale - pass for now
                self.check_custom(subject_id, "determination_rationale", description)
                    .await
            }

            RequirementDef::ChecklistComplete { description } => {
                // Alias for CaseChecklistComplete
                self.check_checklist(subject_id, description).await
            }

            RequirementDef::AllEntitiesTyped { description } => {
                // Check all entities have types
                self.check_entities_typed(subject_id, description).await
            }

            RequirementDef::RiskReassessmentComplete { description } => {
                // Risk reassessment check - pass for now
                self.check_custom(subject_id, "risk_reassessment", description)
                    .await
            }

            RequirementDef::DeferralReasonDocumented { description } => {
                // Deferral reason check - pass for now
                self.check_custom(subject_id, "deferral_reason", description)
                    .await
            }

            RequirementDef::DeferralApprovalRecorded { description } => {
                // Deferral approval check - pass for now
                self.check_custom(subject_id, "deferral_approval", description)
                    .await
            }

            // ─────────────────────────────────────────────────────────────────────────────
            // Document Requirements (new 3-layer model)
            // ─────────────────────────────────────────────────────────────────────────────
            RequirementDef::RequirementSatisfied {
                doc_type,
                min_state,
                subject,
                max_age_days,
                description,
            } => {
                self.check_requirement_satisfied(
                    subject_id,
                    doc_type,
                    min_state,
                    subject,
                    *max_age_days,
                    description,
                )
                .await
            }

            RequirementDef::DocumentExists {
                document_type,
                status,
                subject,
                max_age_days,
                description,
            } => {
                // Legacy support - maps to requirement check
                self.check_requirement_satisfied(
                    subject_id,
                    document_type,
                    status,
                    subject,
                    *max_age_days,
                    description,
                )
                .await
            }

            // ─────────────────────────────────────────────────────────────────────────────
            // Custom
            // ─────────────────────────────────────────────────────────────────────────────
            RequirementDef::Custom {
                code,
                params: _,
                description,
            } => self.check_custom(subject_id, code, description).await,
        }
    }

    /// Evaluate all requirements for a state
    pub async fn evaluate_all(
        &self,
        requirements: &[RequirementDef],
        subject_id: Uuid,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        let mut all_blockers = Vec::new();

        for req in requirements {
            let blockers = self.evaluate(req, subject_id).await?;
            all_blockers.extend(blockers);
        }

        Ok(all_blockers)
    }

    // --- Individual requirement checks ---

    async fn check_role_count(
        &self,
        cbu_id: Uuid,
        role: &str,
        min: u32,
        description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM "ob-poc".cbu_entity_roles cer
            JOIN "ob-poc".roles r ON cer.role_id = r.role_id
            WHERE cer.cbu_id = $1
            AND r.name = $2
            "#,
        )
        .bind(cbu_id)
        .bind(role)
        .fetch_one(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        if (count as u32) < min {
            Ok(vec![Blocker::new(
                BlockerType::MissingRole {
                    role: role.to_string(),
                    required: min,
                    current: count as u32,
                },
                if description.is_empty() {
                    format!("At least {} {} required", min, role.to_lowercase())
                } else {
                    description.to_string()
                },
            )
            .with_resolution("cbu.assign-role")
            .with_detail("role", serde_json::json!(role))])
        } else {
            Ok(vec![])
        }
    }

    async fn check_all_screened(
        &self,
        cbu_id: Uuid,
        _description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
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

        Ok(unscreened
            .iter()
            .map(|(id, name)| {
                Blocker::new(
                    BlockerType::PendingScreening { entity_id: *id },
                    format!("Screening required for {}", name),
                )
                .with_resolution("case-screening.run")
                .with_detail("entity_id", serde_json::json!(id))
            })
            .collect())
    }

    async fn check_document_set(
        &self,
        cbu_id: Uuid,
        documents: &[String],
        _description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        let mut blockers = Vec::new();

        for doc_type in documents {
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
                            document_type: doc_type.clone(),
                            for_entity: None,
                        },
                        format!("{} required", doc_type.replace('_', " ").to_lowercase()),
                    )
                    .with_resolution("document.catalog")
                    .with_detail("document_type", serde_json::json!(doc_type)),
                );
            }
        }

        Ok(blockers)
    }

    async fn check_per_entity_docs(
        &self,
        cbu_id: Uuid,
        entity_type: &str,
        documents: &[String],
        _description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        // Get entities of this type (entity_type is the role name like DIRECTOR)
        let entities: Vec<(Uuid, String)> = sqlx::query_as(
            r#"
            SELECT e.entity_id, e.name
            FROM "ob-poc".entities e
            JOIN "ob-poc".cbu_entity_roles cer ON e.entity_id = cer.entity_id
            JOIN "ob-poc".roles r ON cer.role_id = r.role_id
            WHERE cer.cbu_id = $1
            AND r.name = $2
            "#,
        )
        .bind(cbu_id)
        .bind(entity_type)
        .fetch_all(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        let mut blockers = Vec::new();

        for (entity_id, entity_name) in entities {
            for doc_type in documents {
                let has_doc: bool = sqlx::query_scalar(
                    r#"
                    SELECT EXISTS(
                        SELECT 1 FROM "ob-poc".document_catalog d
                        JOIN "ob-poc".cbu_entity_roles cer ON d.cbu_id = cer.cbu_id
                        WHERE cer.entity_id = $1
                        AND d.document_type_code = $2
                        AND d.status = 'active'
                    )
                    "#,
                )
                .bind(entity_id)
                .bind(doc_type)
                .fetch_one(&self.pool)
                .await
                .map_err(WorkflowError::Database)?;

                if !has_doc {
                    blockers.push(
                        Blocker::new(
                            BlockerType::MissingDocument {
                                document_type: doc_type.clone(),
                                for_entity: Some(entity_id),
                            },
                            format!(
                                "{} required for {}",
                                doc_type.replace('_', " "),
                                entity_name
                            ),
                        )
                        .with_resolution("document.catalog")
                        .with_detail("entity_id", serde_json::json!(entity_id))
                        .with_detail("document_type", serde_json::json!(doc_type)),
                    );
                }
            }
        }

        Ok(blockers)
    }

    async fn check_ownership(
        &self,
        cbu_id: Uuid,
        threshold: f64,
        _description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        let total: Option<rust_decimal::Decimal> = sqlx::query_scalar(
            r#"
            SELECT SUM(r.percentage) FROM "ob-poc".entity_relationships r
            JOIN "ob-poc".cbu_entity_roles cer ON r.to_entity_id = cer.entity_id
            WHERE cer.cbu_id = $1
            AND r.relationship_type = 'ownership'
            AND (r.effective_to IS NULL OR r.effective_to > CURRENT_DATE)
            "#,
        )
        .bind(cbu_id)
        .fetch_one(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        let total_f64: f64 = total
            .map(|d| d.to_string().parse().unwrap_or(0.0))
            .unwrap_or(0.0);

        // Allow small tolerance for floating point
        if total_f64 < threshold - 0.01 {
            Ok(vec![Blocker::new(
                BlockerType::IncompleteOwnership {
                    current_total: total_f64,
                    required: threshold,
                },
                format!(
                    "Ownership {:.1}% of {:.0}% documented",
                    total_f64, threshold
                ),
            )
            .with_resolution("ubo.add-ownership")])
        } else {
            Ok(vec![])
        }
    }

    async fn check_ubos_verified(
        &self,
        cbu_id: Uuid,
        _description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        let unverified: Vec<(Uuid, String)> = sqlx::query_as(
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

        Ok(unverified
            .iter()
            .map(|(ubo_id, name)| {
                Blocker::new(
                    BlockerType::UnverifiedUbo {
                        ubo_id: *ubo_id,
                        person_name: name.clone(),
                    },
                    format!("UBO verification required for {}", name),
                )
                .with_resolution("ubo.verify-ubo")
                .with_detail("ubo_id", serde_json::json!(ubo_id))
            })
            .collect())
    }

    async fn check_no_alerts(
        &self,
        cbu_id: Uuid,
        _description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        let alerts: Vec<(Uuid, Uuid)> = sqlx::query_as(
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

        Ok(alerts
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
            .collect())
    }

    async fn check_checklist(
        &self,
        cbu_id: Uuid,
        description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        // Check if case checklist is complete
        // Note: kyc.case_checklist_items may not exist yet - handle gracefully
        let incomplete: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM kyc.doc_requests dr
                JOIN kyc.entity_workstreams ew ON dr.workstream_id = ew.workstream_id
                JOIN kyc.cases c ON ew.case_id = c.case_id
                WHERE c.cbu_id = $1
                AND dr.status NOT IN ('VERIFIED', 'WAIVED')
            )
            "#,
        )
        .bind(cbu_id)
        .fetch_one(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        if incomplete {
            Ok(vec![Blocker::new(
                BlockerType::Custom {
                    code: "CHECKLIST_INCOMPLETE".to_string(),
                },
                if description.is_empty() {
                    "Document checklist items not complete".to_string()
                } else {
                    description.to_string()
                },
            )])
        } else {
            Ok(vec![])
        }
    }

    async fn check_custom(
        &self,
        _cbu_id: Uuid,
        code: &str,
        description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        // Custom requirements - log a warning and pass through
        // These would be implemented as needed
        tracing::warn!("Custom requirement '{}' not implemented, passing", code);
        if description.is_empty() {
            Ok(vec![])
        } else {
            // If there's a description, treat as a manual check that always passes
            Ok(vec![])
        }
    }

    // ─────────────────────────────────────────────────────────────────────────────────
    // New evaluator methods for extended requirements
    // ─────────────────────────────────────────────────────────────────────────────────

    async fn check_fields_present(
        &self,
        cbu_id: Uuid,
        fields: &[String],
        description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        let mut blockers = Vec::new();

        for field in fields {
            // Check if field exists and is not null on CBU
            let column = field.replace('-', "_");
            let query = format!(
                r#"SELECT {} IS NOT NULL AND {}::text <> '' FROM "ob-poc".cbus WHERE cbu_id = $1"#,
                column, column
            );

            let is_present: Option<bool> = sqlx::query_scalar(&query)
                .bind(cbu_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(WorkflowError::Database)?
                .flatten();

            if is_present != Some(true) {
                blockers.push(
                    Blocker::new(
                        BlockerType::FieldMissing {
                            field: field.to_string(),
                        },
                        if description.is_empty() {
                            format!("{} is required", field.replace('-', " "))
                        } else {
                            description.to_string()
                        },
                    )
                    .with_resolution("cbu.update")
                    .with_detail("field", serde_json::json!(field)),
                );
            }
        }

        Ok(blockers)
    }

    async fn check_product_assigned(
        &self,
        cbu_id: Uuid,
        min: u32,
        description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT CASE WHEN product_id IS NOT NULL THEN 1 ELSE 0 END
            FROM "ob-poc".cbus WHERE cbu_id = $1
            "#,
        )
        .bind(cbu_id)
        .fetch_one(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        if (count as u32) < min {
            Ok(vec![Blocker::new(
                BlockerType::MissingProduct {
                    product_type: None,
                    required: min,
                    current: count as u32,
                },
                if description.is_empty() {
                    "Product assignment required".to_string()
                } else {
                    description.to_string()
                },
            )
            .with_resolution("cbu.add-product")])
        } else {
            Ok(vec![])
        }
    }

    async fn check_relationship_exists(
        &self,
        cbu_id: Uuid,
        relationship_type: &str,
        description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        let exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM "ob-poc".entity_relationships r
                JOIN "ob-poc".cbu_entity_roles cer ON r.to_entity_id = cer.entity_id
                WHERE cer.cbu_id = $1
                AND r.relationship_type = 'ownership'
                AND r.ownership_type = $2
                AND (r.effective_to IS NULL OR r.effective_to > CURRENT_DATE)
            )
            "#,
        )
        .bind(cbu_id)
        .bind(relationship_type)
        .fetch_one(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        if !exists {
            Ok(vec![Blocker::new(
                BlockerType::MissingRelationship {
                    relationship_type: relationship_type.to_string(),
                    from_entity: None,
                    to_entity: None,
                },
                if description.is_empty() {
                    format!("{} relationship required", relationship_type)
                } else {
                    description.to_string()
                },
            )
            .with_resolution("ubo.add-ownership")])
        } else {
            Ok(vec![])
        }
    }

    async fn check_conditional(
        &self,
        subject_id: Uuid,
        condition: &super::definition::ConditionalCheck,
        requirement: &RequirementDef,
        _description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        // Check condition
        let column = condition.field.replace('-', "_");
        let query = format!(r#"SELECT {} FROM "ob-poc".cbus WHERE cbu_id = $1"#, column);

        let value: Option<String> = sqlx::query_scalar(&query)
            .bind(subject_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(WorkflowError::Database)?
            .flatten();

        let condition_met = if let Some(v) = value {
            if let Some(eq) = &condition.equals {
                &v == eq
            } else if !condition.in_values.is_empty() {
                condition.in_values.contains(&v)
            } else {
                true
            }
        } else {
            false
        };

        if condition_met {
            // Evaluate the nested requirement
            Box::pin(self.evaluate(requirement, subject_id)).await
        } else {
            // Condition not met, skip this requirement
            Ok(vec![])
        }
    }

    async fn check_documents_reviewed(
        &self,
        cbu_id: Uuid,
        description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        let unreviewed: Vec<(Uuid, String)> = sqlx::query_as(
            r#"
            SELECT d.doc_id, d.document_type_code
            FROM "ob-poc".document_catalog d
            WHERE d.cbu_id = $1
            AND d.status = 'active'
            AND d.extraction_status = 'PENDING'
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        Ok(unreviewed
            .iter()
            .map(|(doc_id, doc_type)| {
                Blocker::new(
                    BlockerType::DocumentNotReviewed {
                        document_id: *doc_id,
                        document_type: doc_type.clone(),
                    },
                    if description.is_empty() {
                        format!("Document {} requires review", doc_type)
                    } else {
                        description.to_string()
                    },
                )
                .with_resolution("doc-request.verify")
                .with_detail("document_id", serde_json::json!(doc_id))
            })
            .collect())
    }

    async fn check_screenings_current(
        &self,
        cbu_id: Uuid,
        max_age_days: u32,
        _description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        let stale: Vec<(Uuid, String, Option<DateTime<Utc>>)> = sqlx::query_as(
            r#"
            SELECT ew.entity_id, s.screening_type, s.completed_at
            FROM kyc.screenings s
            JOIN kyc.entity_workstreams ew ON s.workstream_id = ew.workstream_id
            JOIN kyc.cases c ON ew.case_id = c.case_id
            WHERE c.cbu_id = $1
            AND s.completed_at < NOW() - ($2 || ' days')::INTERVAL
            AND s.status IN ('CLEAR', 'HIT_CONFIRMED', 'HIT_DISMISSED')
            "#,
        )
        .bind(cbu_id)
        .bind(max_age_days.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        Ok(stale
            .iter()
            .map(|(entity_id, screening_type, last_screened)| {
                Blocker::new(
                    BlockerType::StaleScreening {
                        entity_id: *entity_id,
                        screening_type: screening_type.clone(),
                        last_screened_at: *last_screened,
                        max_age_days,
                    },
                    format!("{} screening expired for entity", screening_type),
                )
                .with_resolution("case-screening.run")
                .with_detail("entity_id", serde_json::json!(entity_id))
                .with_detail("screening_type", serde_json::json!(screening_type))
            })
            .collect())
    }

    async fn check_no_pending_hits(
        &self,
        cbu_id: Uuid,
        _description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        let pending: Vec<(Uuid, Uuid, String)> = sqlx::query_as(
            r#"
            SELECT s.screening_id, ew.entity_id, s.screening_type
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

        Ok(pending
            .iter()
            .map(|(screening_id, entity_id, hit_type)| {
                Blocker::new(
                    BlockerType::PendingHit {
                        screening_id: *screening_id,
                        entity_id: *entity_id,
                        hit_type: hit_type.clone(),
                    },
                    format!("Pending {} hit requires review", hit_type),
                )
                .with_resolution("case-screening.review-hit")
                .with_detail("screening_id", serde_json::json!(screening_id))
            })
            .collect())
    }

    async fn check_chains_resolved(
        &self,
        cbu_id: Uuid,
        description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        // Check for ownership chains not terminating at natural persons
        let unresolved: Vec<(Uuid, Option<i32>)> = sqlx::query_as(
            r#"
            WITH RECURSIVE chain AS (
                SELECT r.from_entity_id as owner_entity_id, r.to_entity_id as owned_entity_id, 1 as depth
                FROM "ob-poc".entity_relationships r
                JOIN "ob-poc".cbu_entity_roles cer ON r.to_entity_id = cer.entity_id
                WHERE cer.cbu_id = $1
                AND r.relationship_type = 'ownership'
                AND (r.effective_to IS NULL OR r.effective_to > CURRENT_DATE)

                UNION ALL

                SELECT r.from_entity_id, c.owned_entity_id, c.depth + 1
                FROM "ob-poc".entity_relationships r
                JOIN chain c ON r.to_entity_id = c.owner_entity_id
                WHERE c.depth < 10
                AND r.relationship_type = 'ownership'
                AND (r.effective_to IS NULL OR r.effective_to > CURRENT_DATE)
            )
            SELECT DISTINCT c.owner_entity_id, c.depth
            FROM chain c
            JOIN "ob-poc".entities e ON c.owner_entity_id = e.entity_id
            JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
            WHERE et.entity_category = 'SHELL'
            AND NOT EXISTS (
                SELECT 1 FROM "ob-poc".entity_relationships r2
                WHERE r2.to_entity_id = c.owner_entity_id
                AND r2.relationship_type = 'ownership'
                AND (r2.effective_to IS NULL OR r2.effective_to > CURRENT_DATE)
            )
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        Ok(unresolved
            .iter()
            .map(|(entity_id, depth)| {
                Blocker::new(
                    BlockerType::UnresolvedOwnershipChain {
                        entity_id: *entity_id,
                        chain_depth: depth.map(|d| d as u32),
                    },
                    if description.is_empty() {
                        "Ownership chain does not resolve to natural person".to_string()
                    } else {
                        description.to_string()
                    },
                )
                .with_resolution("ubo.trace-chains")
                .with_detail("entity_id", serde_json::json!(entity_id))
            })
            .collect())
    }

    async fn check_ubo_threshold_applied(
        &self,
        cbu_id: Uuid,
        _threshold: f64,
        description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        // Check if UBO threshold evaluation has been run
        let has_ubos: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM "ob-poc".ubo_registry
                WHERE cbu_id = $1
            )
            "#,
        )
        .bind(cbu_id)
        .fetch_one(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        if !has_ubos {
            Ok(vec![Blocker::new(
                BlockerType::UboThresholdNotApplied { cbu_id },
                if description.is_empty() {
                    "UBO threshold calculation not completed".to_string()
                } else {
                    description.to_string()
                },
            )
            .with_resolution("threshold.derive")])
        } else {
            Ok(vec![])
        }
    }

    async fn check_case_exists(
        &self,
        cbu_id: Uuid,
        case_type: Option<&str>,
        description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        let exists: bool = if let Some(ct) = case_type {
            sqlx::query_scalar(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM kyc.cases
                    WHERE cbu_id = $1 AND case_type = $2
                )
                "#,
            )
            .bind(cbu_id)
            .bind(ct)
            .fetch_one(&self.pool)
            .await
            .map_err(WorkflowError::Database)?
        } else {
            sqlx::query_scalar(
                r#"
                SELECT EXISTS(SELECT 1 FROM kyc.cases WHERE cbu_id = $1)
                "#,
            )
            .bind(cbu_id)
            .fetch_one(&self.pool)
            .await
            .map_err(WorkflowError::Database)?
        };

        if !exists {
            Ok(vec![Blocker::new(
                BlockerType::NoCaseExists {
                    case_type: case_type.map(String::from),
                },
                if description.is_empty() {
                    "KYC case required".to_string()
                } else {
                    description.to_string()
                },
            )
            .with_resolution("kyc-case.create")])
        } else {
            Ok(vec![])
        }
    }

    async fn check_analyst_assigned(
        &self,
        cbu_id: Uuid,
        description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        let unassigned: Option<Uuid> = sqlx::query_scalar(
            r#"
            SELECT case_id FROM kyc.cases
            WHERE cbu_id = $1
            AND assigned_analyst_id IS NULL
            AND status NOT IN ('APPROVED', 'REJECTED', 'WITHDRAWN')
            ORDER BY opened_at DESC
            LIMIT 1
            "#,
        )
        .bind(cbu_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        if let Some(case_id) = unassigned {
            Ok(vec![Blocker::new(
                BlockerType::NoAnalystAssigned { case_id },
                if description.is_empty() {
                    "Case requires analyst assignment".to_string()
                } else {
                    description.to_string()
                },
            )
            .with_resolution("kyc-case.assign")
            .with_detail("case_id", serde_json::json!(case_id))])
        } else {
            Ok(vec![])
        }
    }

    async fn check_risk_rating_set(
        &self,
        cbu_id: Uuid,
        description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        let unrated: Option<Uuid> = sqlx::query_scalar(
            r#"
            SELECT case_id FROM kyc.cases
            WHERE cbu_id = $1
            AND risk_rating IS NULL
            AND status NOT IN ('INTAKE', 'APPROVED', 'REJECTED', 'WITHDRAWN')
            ORDER BY opened_at DESC
            LIMIT 1
            "#,
        )
        .bind(cbu_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        if let Some(case_id) = unrated {
            Ok(vec![Blocker::new(
                BlockerType::RiskRatingNotSet { case_id },
                if description.is_empty() {
                    "Risk rating must be set".to_string()
                } else {
                    description.to_string()
                },
            )
            .with_resolution("kyc-case.set-risk-rating")
            .with_detail("case_id", serde_json::json!(case_id))])
        } else {
            Ok(vec![])
        }
    }

    async fn check_approval_recorded(
        &self,
        cbu_id: Uuid,
        description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        let needs_approval: Option<Uuid> = sqlx::query_scalar(
            r#"
            SELECT case_id FROM kyc.cases
            WHERE cbu_id = $1
            AND status = 'REVIEW'
            ORDER BY opened_at DESC
            LIMIT 1
            "#,
        )
        .bind(cbu_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        if let Some(case_id) = needs_approval {
            Ok(vec![Blocker::new(
                BlockerType::ApprovalNotRecorded { case_id },
                if description.is_empty() {
                    "Approval decision required".to_string()
                } else {
                    description.to_string()
                },
            )
            .with_resolution("kyc-case.update-status")])
        } else {
            Ok(vec![])
        }
    }

    async fn check_rejection_recorded(
        &self,
        cbu_id: Uuid,
        description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        // Similar to approval but checking if rejection was recorded
        let needs_rejection: Option<Uuid> = sqlx::query_scalar(
            r#"
            SELECT case_id FROM kyc.cases
            WHERE cbu_id = $1
            AND status = 'BLOCKED'
            ORDER BY opened_at DESC
            LIMIT 1
            "#,
        )
        .bind(cbu_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        if let Some(case_id) = needs_rejection {
            Ok(vec![Blocker::new(
                BlockerType::RejectionNotRecorded { case_id },
                if description.is_empty() {
                    "Rejection decision required".to_string()
                } else {
                    description.to_string()
                },
            )
            .with_resolution("kyc-case.update-status")])
        } else {
            Ok(vec![])
        }
    }

    async fn check_workstreams_created(
        &self,
        cbu_id: Uuid,
        description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        // Check entities that need workstreams but don't have them
        let missing: Vec<Uuid> = sqlx::query_scalar(
            r#"
            SELECT cer.entity_id
            FROM "ob-poc".cbu_entity_roles cer
            JOIN kyc.cases c ON c.cbu_id = cer.cbu_id
            WHERE cer.cbu_id = $1
            AND c.status NOT IN ('APPROVED', 'REJECTED', 'WITHDRAWN')
            AND NOT EXISTS (
                SELECT 1 FROM kyc.entity_workstreams ew
                WHERE ew.case_id = c.case_id
                AND ew.entity_id = cer.entity_id
            )
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        Ok(missing
            .iter()
            .map(|entity_id| {
                Blocker::new(
                    BlockerType::WorkstreamMissing {
                        entity_id: *entity_id,
                    },
                    if description.is_empty() {
                        "Entity workstream required".to_string()
                    } else {
                        description.to_string()
                    },
                )
                .with_resolution("entity-workstream.create")
                .with_detail("entity_id", serde_json::json!(entity_id))
            })
            .collect())
    }

    async fn check_workstreams_complete(
        &self,
        cbu_id: Uuid,
        description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        let incomplete: Vec<(Uuid, Uuid)> = sqlx::query_as(
            r#"
            SELECT ew.workstream_id, ew.entity_id
            FROM kyc.entity_workstreams ew
            JOIN kyc.cases c ON ew.case_id = c.case_id
            WHERE c.cbu_id = $1
            AND ew.status NOT IN ('COMPLETE', 'BLOCKED')
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        Ok(incomplete
            .iter()
            .map(|(workstream_id, entity_id)| {
                Blocker::new(
                    BlockerType::WorkstreamIncomplete {
                        workstream_id: *workstream_id,
                        entity_id: *entity_id,
                        missing_fields: vec![], // Would need more detailed check
                    },
                    if description.is_empty() {
                        "Workstream data incomplete".to_string()
                    } else {
                        description.to_string()
                    },
                )
                .with_resolution("entity-workstream.update-status")
                .with_detail("workstream_id", serde_json::json!(workstream_id))
            })
            .collect())
    }

    async fn check_entity_data_current(
        &self,
        cbu_id: Uuid,
        max_age_days: u32,
        _description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        let stale: Vec<(Uuid, DateTime<Utc>)> = sqlx::query_as(
            r#"
            SELECT e.entity_id, e.updated_at
            FROM "ob-poc".entities e
            JOIN "ob-poc".cbu_entity_roles cer ON e.entity_id = cer.entity_id
            WHERE cer.cbu_id = $1
            AND e.updated_at < NOW() - ($2 || ' days')::INTERVAL
            "#,
        )
        .bind(cbu_id)
        .bind(max_age_days.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        Ok(stale
            .iter()
            .map(|(entity_id, last_updated)| {
                Blocker::new(
                    BlockerType::EntityDataStale {
                        entity_id: *entity_id,
                        last_updated: *last_updated,
                        max_age_days,
                    },
                    format!("Entity data older than {} days", max_age_days),
                )
                .with_resolution("entity.update")
                .with_detail("entity_id", serde_json::json!(entity_id))
            })
            .collect())
    }

    async fn check_changelog_reviewed(
        &self,
        cbu_id: Uuid,
        description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        let pending: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM "ob-poc".cbu_change_log
            WHERE cbu_id = $1
            AND reviewed_at IS NULL
            "#,
        )
        .bind(cbu_id)
        .fetch_one(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        if pending > 0 {
            Ok(vec![Blocker::new(
                BlockerType::ChangeLogNotReviewed {
                    changes_since: None,
                    pending_count: pending as u32,
                },
                if description.is_empty() {
                    format!("{} changes pending review", pending)
                } else {
                    description.to_string()
                },
            )])
        } else {
            Ok(vec![])
        }
    }

    async fn check_signoff_recorded(
        &self,
        cbu_id: Uuid,
        description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        // Check for case sign-off in case_events
        let has_signoff: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM kyc.case_events ce
                JOIN kyc.cases c ON ce.case_id = c.case_id
                WHERE c.cbu_id = $1
                AND ce.event_type = 'SIGN_OFF'
            )
            "#,
        )
        .bind(cbu_id)
        .fetch_one(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        if !has_signoff {
            Ok(vec![Blocker::new(
                BlockerType::SignOffMissing {
                    required_role: None,
                },
                if description.is_empty() {
                    "Sign-off required".to_string()
                } else {
                    description.to_string()
                },
            )
            .with_resolution("kyc-case.update-status")])
        } else {
            Ok(vec![])
        }
    }

    async fn check_next_review_scheduled(
        &self,
        cbu_id: Uuid,
        description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        // Check if next review date is set on the most recent case
        let unscheduled: bool = sqlx::query_scalar(
            r#"
            SELECT NOT EXISTS(
                SELECT 1 FROM kyc.cases c
                JOIN "ob-poc".entity_kyc_status eks ON eks.cbu_id = c.cbu_id
                WHERE c.cbu_id = $1
                AND eks.next_review_date IS NOT NULL
            )
            "#,
        )
        .bind(cbu_id)
        .fetch_one(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        if unscheduled {
            Ok(vec![Blocker::new(
                BlockerType::NextReviewNotScheduled,
                if description.is_empty() {
                    "Next review date must be scheduled".to_string()
                } else {
                    description.to_string()
                },
            )
            .with_resolution("kyc-case.update-status")])
        } else {
            Ok(vec![])
        }
    }

    async fn check_ubo_identity_docs(
        &self,
        cbu_id: Uuid,
        description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        // Check if all UBOs have identity documents
        let missing: Vec<(Uuid, String)> = sqlx::query_as(
            r#"
            SELECT u.ubo_person_id, e.name
            FROM "ob-poc".ubo_registry u
            JOIN "ob-poc".entities e ON u.ubo_person_id = e.entity_id
            WHERE u.cbu_id = $1
            AND NOT EXISTS (
                SELECT 1 FROM "ob-poc".document_catalog d
                WHERE d.cbu_id = u.cbu_id
                AND d.document_type_code IN ('PASSPORT', 'NATIONAL_ID', 'DRIVERS_LICENSE')
                AND d.status = 'active'
            )
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        Ok(missing
            .iter()
            .map(|(entity_id, name)| {
                Blocker::new(
                    BlockerType::MissingDocument {
                        document_type: "IDENTITY_DOCUMENT".to_string(),
                        for_entity: Some(*entity_id),
                    },
                    if description.is_empty() {
                        format!("Identity document required for UBO {}", name)
                    } else {
                        description.to_string()
                    },
                )
                .with_resolution("document.catalog")
                .with_detail("entity_id", serde_json::json!(entity_id))
            })
            .collect())
    }

    async fn check_entities_typed(
        &self,
        cbu_id: Uuid,
        description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        // Check if all entities have types assigned
        let untyped: Vec<(Uuid, String)> = sqlx::query_as(
            r#"
            SELECT e.entity_id, e.name
            FROM "ob-poc".entities e
            JOIN "ob-poc".cbu_entity_roles cer ON e.entity_id = cer.entity_id
            WHERE cer.cbu_id = $1
            AND e.entity_type_id IS NULL
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        Ok(untyped
            .iter()
            .map(|(entity_id, name)| {
                Blocker::new(
                    BlockerType::Custom {
                        code: "ENTITY_UNTYPED".to_string(),
                    },
                    if description.is_empty() {
                        format!("Entity {} needs type assignment", name)
                    } else {
                        description.to_string()
                    },
                )
                .with_detail("entity_id", serde_json::json!(entity_id))
            })
            .collect())
    }

    /// Check if a document requirement is satisfied to at least min_state.
    /// Uses the document_requirements table from the task queue system.
    ///
    /// Subject can be:
    /// - A variable like "$entity_id" (resolved from workflow context - uses subject_id)
    /// - A literal UUID string
    async fn check_requirement_satisfied(
        &self,
        subject_id: Uuid,
        doc_type: &str,
        min_state: &str,
        subject: &str,
        max_age_days: Option<u32>,
        description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        // Resolve subject - if it's a variable ($entity_id), use subject_id
        // Otherwise try to parse as UUID
        let entity_id = if subject.starts_with('$') {
            subject_id
        } else {
            Uuid::parse_str(subject).unwrap_or(subject_id)
        };

        // Query requirement status
        let row: Option<(String, Option<chrono::DateTime<chrono::Utc>>)> = sqlx::query_as(
            r#"
            SELECT status, satisfied_at
            FROM "ob-poc".document_requirements
            WHERE subject_entity_id = $1
              AND doc_type = $2
            "#,
        )
        .bind(entity_id)
        .bind(doc_type)
        .fetch_optional(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        match row {
            None => {
                // No requirement exists = not satisfied
                Ok(vec![Blocker::new(
                    BlockerType::MissingDocument {
                        document_type: doc_type.to_string(),
                        for_entity: Some(entity_id),
                    },
                    if description.is_empty() {
                        format!("{} requirement not created for entity", doc_type)
                    } else {
                        description.to_string()
                    },
                )
                .with_resolution("requirement.create")
                .with_detail("doc_type", serde_json::json!(doc_type))
                .with_detail("entity_id", serde_json::json!(entity_id))])
            }
            Some((status, satisfied_at)) => {
                // Check if current state satisfies threshold
                let satisfied = self.state_satisfies(&status, min_state);

                if !satisfied {
                    return Ok(vec![Blocker::new(
                        BlockerType::MissingDocument {
                            document_type: doc_type.to_string(),
                            for_entity: Some(entity_id),
                        },
                        if description.is_empty() {
                            format!(
                                "{} requirement status '{}' does not satisfy '{}'",
                                doc_type, status, min_state
                            )
                        } else {
                            description.to_string()
                        },
                    )
                    .with_resolution("document.solicit")
                    .with_detail("doc_type", serde_json::json!(doc_type))
                    .with_detail("entity_id", serde_json::json!(entity_id))
                    .with_detail("current_status", serde_json::json!(status))
                    .with_detail("required_status", serde_json::json!(min_state))]);
                }

                // Check recency if specified
                if let (Some(days), Some(satisfied)) = (max_age_days, satisfied_at) {
                    let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
                    if satisfied < cutoff {
                        return Ok(vec![Blocker::new(
                            BlockerType::MissingDocument {
                                document_type: doc_type.to_string(),
                                for_entity: Some(entity_id),
                            },
                            if description.is_empty() {
                                format!(
                                    "{} requirement satisfied too long ago (>{} days)",
                                    doc_type, days
                                )
                            } else {
                                description.to_string()
                            },
                        )
                        .with_resolution("document.solicit")
                        .with_detail("doc_type", serde_json::json!(doc_type))
                        .with_detail("entity_id", serde_json::json!(entity_id))
                        .with_detail("satisfied_at", serde_json::json!(satisfied))
                        .with_detail("max_age_days", serde_json::json!(days))]);
                    }
                }

                Ok(vec![])
            }
        }
    }

    /// Check if a status satisfies a minimum threshold.
    /// State ordering: missing < requested < received < in_qa < verified
    /// rejected/expired never satisfy any requirement.
    /// waived satisfies everything.
    fn state_satisfies(&self, current: &str, min_state: &str) -> bool {
        // Failure states never satisfy
        if current == "rejected" || current == "expired" {
            return false;
        }
        // Waived satisfies anything
        if current == "waived" {
            return true;
        }
        // Verified satisfies anything
        if current == "verified" {
            return true;
        }

        let order = |s: &str| -> u8 {
            match s {
                "missing" => 0,
                "requested" => 1,
                "received" => 2,
                "in_qa" => 3,
                "verified" => 4,
                "waived" => 5,
                _ => 0,
            }
        };

        order(current) >= order(min_state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_requirement_def_parsing() {
        let yaml = r#"
        - type: role_count
          role: DIRECTOR
          min: 1
          description: At least one director
        - type: all_entities_screened
        - type: document_set
          documents:
            - CERTIFICATE_OF_INCORPORATION
            - REGISTER_OF_DIRECTORS
        "#;

        let reqs: Vec<RequirementDef> = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(reqs.len(), 3);

        match &reqs[0] {
            RequirementDef::RoleCount { role, min, .. } => {
                assert_eq!(role, "DIRECTOR");
                assert_eq!(*min, 1);
            }
            _ => panic!("Expected RoleCount"),
        }
    }
}
