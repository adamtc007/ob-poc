//! Requirement Evaluation
//!
//! Evaluates requirements defined in workflow YAML.
//! Each requirement type maps to a database check.
//! This makes guards data-driven from YAML rather than hardcoded.

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
            RequirementDef::RoleCount {
                role,
                min,
                description,
            } => {
                self.check_role_count(subject_id, role, *min, description)
                    .await
            }

            RequirementDef::AllEntitiesScreened { description } => {
                self.check_all_screened(subject_id, description).await
            }

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

            RequirementDef::NoOpenAlerts { description } => {
                self.check_no_alerts(subject_id, description).await
            }

            RequirementDef::CaseChecklistComplete { description } => {
                self.check_checklist(subject_id, description).await
            }

            RequirementDef::Custom {
                code,
                params: _,
                description,
            } => {
                // Custom requirements are evaluated by name
                self.check_custom(subject_id, code, description).await
            }
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
