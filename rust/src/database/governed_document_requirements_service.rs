//! Governed Document Requirements Service
//!
//! Bridges published SemOS document-policy objects to the current runtime
//! document inventory for KYC/entity flows.

use std::collections::HashMap;

use anyhow::{Context, Result};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::database::{
    ActiveDocumentPolicyBundle, DocumentPolicyService, PublishedEvidenceStrategy,
    PublishedProofObligation,
};

#[derive(Debug, Clone, FromRow)]
struct EntityPolicyContextRow {
    entity_id: Uuid,
    entity_type: String,
    jurisdiction: Option<String>,
    cbu_id: Option<Uuid>,
    client_type: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct RuntimeDocumentStatusRow {
    document_id: Uuid,
    document_type: String,
    latest_version_id: Option<Uuid>,
    latest_status: Option<String>,
    valid_to: Option<NaiveDate>,
    version_rejection_code: Option<String>,
}

/// Entity-scoped context used for governed policy matching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityPolicyContext {
    pub entity_id: Uuid,
    pub entity_type: String,
    pub jurisdiction: Option<String>,
    pub cbu_id: Option<Uuid>,
    pub client_type: Option<String>,
}

/// One outstanding governed document requirement component.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernedDocumentGap {
    pub obligation_fqn: String,
    pub obligation_category: String,
    pub strategy_fqn: String,
    pub strategy_priority: i32,
    pub document_type_fqn: String,
    pub status: String,
    pub required_state: String,
    pub matched_document_id: Option<Uuid>,
    pub matched_version_id: Option<Uuid>,
    pub last_rejection_code: Option<String>,
}

/// Result of governed requirement computation for one entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernedDocumentRequirements {
    pub context: EntityPolicyContext,
    pub snapshot_set_id: Uuid,
    pub requirement_profile_fqn: String,
    pub gaps: Vec<GovernedDocumentGap>,
}

/// Component-level status within a governed evidence strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernedComponentStatus {
    pub document_type_fqn: String,
    pub status: String,
    pub matched_document_id: Option<Uuid>,
    pub matched_version_id: Option<Uuid>,
    pub last_rejection_code: Option<String>,
}

/// Strategy-level progress within a governed requirement matrix.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernedStrategyStatus {
    pub strategy_fqn: String,
    pub priority: i32,
    pub proof_strength: String,
    pub completeness: f64,
    pub status: String,
    pub components: Vec<GovernedComponentStatus>,
}

/// Obligation-level progress within a governed requirement matrix.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernedObligationStatus {
    pub obligation_fqn: String,
    pub category: String,
    pub strength_required: String,
    pub is_mandatory: bool,
    pub status: String,
    pub active_strategy: Option<GovernedStrategyStatus>,
    pub alternative_strategies: Vec<GovernedStrategyStatus>,
}

/// Category grouping within a governed requirement matrix.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernedObligationCategory {
    pub category: String,
    pub obligations: Vec<GovernedObligationStatus>,
}

/// Governed requirement matrix for one entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernedRequirementMatrix {
    pub entity_id: Uuid,
    pub entity_type: String,
    pub jurisdiction: Option<String>,
    pub client_type: Option<String>,
    pub snapshot_set_id: Uuid,
    pub requirement_profile_fqn: String,
    pub categories: Vec<GovernedObligationCategory>,
    pub total_obligations: usize,
    pub mandatory_obligations: usize,
    pub mandatory_satisfied_obligations: usize,
    pub satisfied_obligations: usize,
    pub partially_satisfied: usize,
    pub unsatisfied_obligations: usize,
    pub mandatory_coverage: f64,
    pub overall_coverage: f64,
}

/// Runtime service for governed document requirement computation.
#[derive(Clone, Debug)]
pub struct GovernedDocumentRequirementsService {
    pool: PgPool,
    document_policy_service: DocumentPolicyService,
}

impl GovernedDocumentRequirementsService {
    /// Create a new governed document requirements service.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let service = GovernedDocumentRequirementsService::new(pool.clone());
    /// ```
    pub fn new(pool: PgPool) -> Self {
        Self {
            document_policy_service: DocumentPolicyService::new(pool.clone()),
            pool,
        }
    }

    /// Compute governed outstanding document requirements for an entity.
    ///
    /// Returns `Ok(None)` when no matching active requirement profile exists.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let governed = service.compute_for_entity(entity_id).await?;
    /// ```
    pub async fn compute_for_entity(
        &self,
        entity_id: Uuid,
    ) -> Result<Option<GovernedDocumentRequirements>> {
        let Some(context) = self.load_entity_policy_context(entity_id).await? else {
            return Ok(None);
        };

        let Some(bundle) = self.resolve_matching_bundle(&context).await? else {
            return Ok(None);
        };

        let runtime_docs = self.load_runtime_document_statuses(entity_id).await?;
        let gaps = self.compute_gaps(&bundle, &runtime_docs);

        Ok(Some(GovernedDocumentRequirements {
            context,
            snapshot_set_id: bundle.snapshot_set_id,
            requirement_profile_fqn: bundle.requirement_profile.body.fqn.clone(),
            gaps,
        }))
    }

    /// Compute a governed requirement matrix for an entity.
    ///
    /// Returns `Ok(None)` when no matching active requirement profile exists.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let matrix = service.compute_matrix_for_entity(entity_id).await?;
    /// ```
    pub async fn compute_matrix_for_entity(
        &self,
        entity_id: Uuid,
    ) -> Result<Option<GovernedRequirementMatrix>> {
        let Some(context) = self.load_entity_policy_context(entity_id).await? else {
            return Ok(None);
        };

        let Some(bundle) = self.resolve_matching_bundle(&context).await? else {
            return Ok(None);
        };

        let runtime_docs = self.load_runtime_document_statuses(entity_id).await?;
        let obligation_statuses = self.compute_obligation_statuses(&bundle, &runtime_docs);

        let mut categories_map: HashMap<String, Vec<GovernedObligationStatus>> = HashMap::new();
        for obligation in obligation_statuses {
            categories_map
                .entry(obligation.category.clone())
                .or_default()
                .push(obligation);
        }

        let mut categories: Vec<GovernedObligationCategory> = categories_map
            .into_iter()
            .map(|(category, mut obligations)| {
                obligations.sort_by(|left, right| left.obligation_fqn.cmp(&right.obligation_fqn));
                GovernedObligationCategory {
                    category,
                    obligations,
                }
            })
            .collect();
        categories.sort_by(|left, right| left.category.cmp(&right.category));

        let all_obligations: Vec<&GovernedObligationStatus> = categories
            .iter()
            .flat_map(|category| category.obligations.iter())
            .collect();
        let total_obligations = all_obligations.len();
        let mandatory_obligations = all_obligations
            .iter()
            .filter(|status| status.is_mandatory)
            .count();
        let satisfied_obligations = all_obligations
            .iter()
            .filter(|status| status.status == "satisfied")
            .count();
        let partially_satisfied = all_obligations
            .iter()
            .filter(|status| status.status == "in_progress")
            .count();
        let unsatisfied_obligations =
            total_obligations.saturating_sub(satisfied_obligations + partially_satisfied);
        let mandatory_satisfied = all_obligations
            .iter()
            .filter(|status| status.is_mandatory && status.status == "satisfied")
            .count();

        Ok(Some(GovernedRequirementMatrix {
            entity_id: context.entity_id,
            entity_type: context.entity_type,
            jurisdiction: context.jurisdiction,
            client_type: context.client_type,
            snapshot_set_id: bundle.snapshot_set_id,
            requirement_profile_fqn: bundle.requirement_profile.body.fqn,
            categories,
            total_obligations,
            mandatory_obligations,
            mandatory_satisfied_obligations: mandatory_satisfied,
            satisfied_obligations,
            partially_satisfied,
            unsatisfied_obligations,
            mandatory_coverage: percent(mandatory_satisfied, mandatory_obligations),
            overall_coverage: percent(satisfied_obligations, total_obligations),
        }))
    }

    async fn load_entity_policy_context(
        &self,
        entity_id: Uuid,
    ) -> Result<Option<EntityPolicyContext>> {
        let row = sqlx::query_as::<_, EntityPolicyContextRow>(
            r#"
            SELECT
                e.entity_id,
                COALESCE(NULLIF(et.type_code, ''), et.name, 'entity') AS entity_type,
                COALESCE(lc.jurisdiction, pp.nationality, p.jurisdiction, t.jurisdiction) AS jurisdiction,
                cbu_link.cbu_id,
                cbu_link.client_type
            FROM "ob-poc".entities e
            JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
            LEFT JOIN "ob-poc".entity_limited_companies lc ON lc.limited_company_id = e.entity_id
            LEFT JOIN "ob-poc".entity_proper_persons pp ON pp.proper_person_id = e.entity_id
            LEFT JOIN "ob-poc".entity_partnerships p ON p.partnership_id = e.entity_id
            LEFT JOIN "ob-poc".entity_trusts t ON t.trust_id = e.entity_id
            LEFT JOIN LATERAL (
                SELECT c.cbu_id, c.client_type
                FROM "ob-poc".cbu_entity_roles cer
                JOIN "ob-poc".cbus c ON c.cbu_id = cer.cbu_id
                WHERE cer.entity_id = e.entity_id
                  AND c.deleted_at IS NULL
                ORDER BY cer.created_at DESC NULLS LAST
                LIMIT 1
            ) cbu_link ON true
            WHERE e.entity_id = $1
              AND e.deleted_at IS NULL
            "#,
        )
        .bind(entity_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to load entity policy context")?;

        Ok(row.map(|row| EntityPolicyContext {
            entity_id: row.entity_id,
            entity_type: normalize_code(&row.entity_type),
            jurisdiction: row.jurisdiction.map(|value| value.to_ascii_uppercase()),
            cbu_id: row.cbu_id,
            client_type: row.client_type.map(|value| value.to_ascii_uppercase()),
        }))
    }

    async fn resolve_matching_bundle(
        &self,
        context: &EntityPolicyContext,
    ) -> Result<Option<ActiveDocumentPolicyBundle>> {
        let profiles = self
            .document_policy_service
            .list_active_requirement_profiles()
            .await?;

        let mut matching: Vec<(usize, String)> = profiles
            .into_iter()
            .filter(|profile| profile_applies(profile, context))
            .map(|profile| {
                (
                    profile_specificity(
                        &profile.body.entity_types,
                        &profile.body.jurisdictions,
                        &profile.body.client_types,
                        &profile.body.contexts,
                    ),
                    profile.body.fqn,
                )
            })
            .collect();

        matching.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));

        let Some((_, fqn)) = matching.into_iter().next() else {
            return Ok(None);
        };

        self.document_policy_service
            .resolve_active_policy_bundle(&fqn)
            .await
    }

    async fn load_runtime_document_statuses(
        &self,
        entity_id: Uuid,
    ) -> Result<Vec<RuntimeDocumentStatusRow>> {
        sqlx::query_as::<_, RuntimeDocumentStatusRow>(
            r#"
            SELECT
                document_id,
                document_type,
                latest_version_id,
                latest_status,
                valid_to,
                NULL::text AS version_rejection_code
            FROM "ob-poc".v_documents_with_status
            WHERE subject_entity_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to load runtime document statuses")
    }

    fn compute_gaps(
        &self,
        bundle: &ActiveDocumentPolicyBundle,
        runtime_docs: &[RuntimeDocumentStatusRow],
    ) -> Vec<GovernedDocumentGap> {
        let obligation_statuses = self.compute_obligation_statuses(bundle, runtime_docs);
        let mut gaps = Vec::new();

        for obligation in obligation_statuses {
            if let Some(strategy) = obligation.active_strategy {
                for component in strategy.components {
                    if component.status != "satisfied" {
                        gaps.push(GovernedDocumentGap {
                            obligation_fqn: obligation.obligation_fqn.clone(),
                            obligation_category: obligation.category.clone(),
                            strategy_fqn: strategy.strategy_fqn.clone(),
                            strategy_priority: strategy.priority,
                            document_type_fqn: component.document_type_fqn,
                            status: component.status,
                            required_state: "verified".to_string(),
                            matched_document_id: component.matched_document_id,
                            matched_version_id: component.matched_version_id,
                            last_rejection_code: component.last_rejection_code,
                        });
                    }
                }
            }
        }

        gaps
    }

    fn compute_obligation_statuses(
        &self,
        bundle: &ActiveDocumentPolicyBundle,
        runtime_docs: &[RuntimeDocumentStatusRow],
    ) -> Vec<GovernedObligationStatus> {
        let mut doc_index: HashMap<String, Vec<&RuntimeDocumentStatusRow>> = HashMap::new();
        for doc in runtime_docs {
            doc_index
                .entry(normalize_code(&doc.document_type))
                .or_default()
                .push(doc);
        }

        let mut strategies_by_obligation: HashMap<&str, Vec<&PublishedEvidenceStrategy>> =
            HashMap::new();
        for strategy in &bundle.evidence_strategies {
            if let Some(obligation_fqn) = strategy.body.obligation_fqn.as_deref() {
                strategies_by_obligation
                    .entry(obligation_fqn)
                    .or_default()
                    .push(strategy);
            }
        }

        for strategies in strategies_by_obligation.values_mut() {
            strategies.sort_by_key(|strategy| strategy.body.priority);
        }

        let mut obligation_statuses = Vec::new();

        for obligation in &bundle.proof_obligations {
            let strategies = strategies_by_obligation
                .get(obligation.body.fqn.as_str())
                .cloned()
                .unwrap_or_default();

            if strategies.is_empty() {
                obligation_statuses.push(GovernedObligationStatus {
                    obligation_fqn: obligation.body.fqn.clone(),
                    category: obligation.body.category.clone(),
                    strength_required: format!("{:?}", obligation.body.strength_required)
                        .to_ascii_lowercase(),
                    is_mandatory: obligation.body.is_mandatory,
                    status: "not_started".to_string(),
                    active_strategy: None,
                    alternative_strategies: Vec::new(),
                });
                continue;
            }

            let mut evaluated_strategies: Vec<GovernedStrategyStatus> = strategies
                .iter()
                .map(|strategy| compute_strategy_status(obligation, strategy, &doc_index))
                .collect();
            evaluated_strategies.sort_by_key(|strategy| strategy.priority);

            let active_strategy = evaluated_strategies.first().cloned();
            let status = evaluated_strategies
                .iter()
                .find(|strategy| strategy.status == "satisfied")
                .map(|_| "satisfied".to_string())
                .unwrap_or_else(|| {
                    if evaluated_strategies
                        .iter()
                        .any(|strategy| strategy.status == "in_progress")
                    {
                        "in_progress".to_string()
                    } else {
                        "not_started".to_string()
                    }
                });

            obligation_statuses.push(GovernedObligationStatus {
                obligation_fqn: obligation.body.fqn.clone(),
                category: obligation.body.category.clone(),
                strength_required: format!("{:?}", obligation.body.strength_required)
                    .to_ascii_lowercase(),
                is_mandatory: obligation.body.is_mandatory,
                status,
                active_strategy,
                alternative_strategies: evaluated_strategies.into_iter().skip(1).collect(),
            });
        }

        obligation_statuses
    }
}

fn compute_strategy_status(
    obligation: &PublishedProofObligation,
    strategy: &PublishedEvidenceStrategy,
    doc_index: &HashMap<String, Vec<&RuntimeDocumentStatusRow>>,
) -> GovernedStrategyStatus {
    let today = chrono::Utc::now().date_naive();
    let mut components = Vec::new();
    let mut satisfied_components = 0usize;
    let mut started_components = 0usize;

    for component in strategy
        .body
        .components
        .iter()
        .filter(|component| component.required)
    {
        let doc_keys = component_doc_keys(&component.document_type_fqn);
        let matched_docs: Vec<&RuntimeDocumentStatusRow> = doc_keys
            .iter()
            .filter_map(|key| doc_index.get(key))
            .flat_map(|docs| docs.iter().copied())
            .collect();

        let maybe_verified = matched_docs.iter().find(|doc| {
            matches!(doc.latest_status.as_deref(), Some("verified"))
                && doc.valid_to.is_none_or(|valid_to| valid_to >= today)
        });

        let best_match = maybe_verified
            .copied()
            .or_else(|| matched_docs.first().copied());
        let status = if maybe_verified.is_some() {
            satisfied_components += 1;
            "satisfied"
        } else if let Some(doc) = best_match {
            started_components += 1;
            match doc.latest_status.as_deref() {
                Some("rejected") => "rejected",
                Some("verified") if doc.valid_to.is_some_and(|valid_to| valid_to < today) => {
                    "expired"
                }
                Some("pending") | Some("in_qa") => "pending",
                Some(other) => other,
                None => "present",
            }
        } else {
            "missing"
        };

        components.push(GovernedComponentStatus {
            document_type_fqn: component.document_type_fqn.clone(),
            status: status.to_string(),
            matched_document_id: best_match.map(|doc| doc.document_id),
            matched_version_id: best_match.and_then(|doc| doc.latest_version_id),
            last_rejection_code: best_match.and_then(|doc| doc.version_rejection_code.clone()),
        });
    }

    let total_components = components.len();
    let completeness = percent(satisfied_components, total_components);
    let status = if total_components == 0 || satisfied_components == total_components {
        "satisfied"
    } else if satisfied_components > 0 || started_components > 0 {
        "in_progress"
    } else {
        "not_started"
    };

    GovernedStrategyStatus {
        strategy_fqn: strategy.body.fqn.clone(),
        priority: strategy.body.priority,
        proof_strength: format!("{:?}", obligation.body.strength_required).to_ascii_lowercase(),
        completeness,
        status: status.to_string(),
        components,
    }
}

fn profile_applies(
    profile: &crate::database::PublishedRequirementProfile,
    context: &EntityPolicyContext,
) -> bool {
    matches_filter(&profile.body.entity_types, &context.entity_type)
        && matches_optional_filter(&profile.body.jurisdictions, context.jurisdiction.as_deref())
        && matches_optional_filter(&profile.body.client_types, context.client_type.as_deref())
        && (profile.body.contexts.is_empty()
            || profile
                .body
                .contexts
                .iter()
                .any(|value| normalize_code(value) == "kyc_entity"))
}

fn matches_filter(filters: &[String], current: &str) -> bool {
    filters.is_empty() || filters.iter().any(|value| normalize_code(value) == current)
}

fn matches_optional_filter(filters: &[String], current: Option<&str>) -> bool {
    filters.is_empty()
        || current.is_some_and(|value| {
            filters
                .iter()
                .any(|candidate| normalize_code(candidate) == normalize_code(value))
        })
}

fn profile_specificity(
    entity_types: &[String],
    jurisdictions: &[String],
    client_types: &[String],
    contexts: &[String],
) -> usize {
    usize::from(!entity_types.is_empty())
        + usize::from(!jurisdictions.is_empty())
        + usize::from(!client_types.is_empty())
        + usize::from(!contexts.is_empty())
}

fn component_doc_keys(document_type_fqn: &str) -> Vec<String> {
    let normalized_fqn = normalize_code(document_type_fqn);
    let last_segment = document_type_fqn
        .rsplit('.')
        .next()
        .map(normalize_code)
        .unwrap_or_else(|| normalized_fqn.clone());

    if normalized_fqn == last_segment {
        vec![normalized_fqn]
    } else {
        vec![normalized_fqn, last_segment]
    }
}

fn normalize_code(value: &str) -> String {
    value.trim().replace('-', "_").to_ascii_lowercase()
}

fn percent(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        100.0
    } else {
        (numerator as f64 / denominator as f64) * 100.0
    }
}
