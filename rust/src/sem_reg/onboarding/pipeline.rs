//! Core onboarding pipeline: 6-step entity type registration.
//!
//! Steps:
//!   1. Create/update `EntityTypeDef` snapshot
//!   2. Create/update `AttributeDef` snapshots for each attribute
//!   3. Create/update `VerbContract` snapshots for CRUD verbs
//!   4. Taxonomy placement — create `MembershipRule` linking to taxonomies
//!   5. View assignment — add columns to relevant `ViewDef`s
//!   6. Evidence requirements — create `EvidenceRequirement` snapshots

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::sem_reg::ids::{definition_hash, object_id_for};
use crate::sem_reg::store::SnapshotStore;
use crate::sem_reg::types::{ChangeType, ObjectType, SnapshotMeta};

use crate::sem_reg::{
    AttributeDefBody, EntityTypeDefBody, EvidenceRequirementBody, VerbContractBody, ViewDefBody,
};

use super::defaults;
use super::validators;

// ── Request / Result types ──────────────────────────────────────────────────

/// Specification for onboarding a single entity type into the semantic registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingRequest {
    /// The entity type to create (must include `fqn`, `name`, `domain`).
    pub entity_type: EntityTypeDefBody,

    /// Attributes belonging to this entity type.
    /// If empty, defaults are generated from the entity type's required/optional attributes.
    #[serde(default)]
    pub attributes: Vec<AttributeDefBody>,

    /// Verb contracts. If empty, standard CRUD contracts are generated.
    #[serde(default)]
    pub verb_contracts: Vec<VerbContractBody>,

    /// FQNs of taxonomies to place this entity type into.
    /// If empty, domain-based defaults are used.
    #[serde(default)]
    pub taxonomy_fqns: Vec<String>,

    /// FQNs of views to add this entity type's attributes to.
    /// If empty, domain-based defaults are used.
    #[serde(default)]
    pub view_fqns: Vec<String>,

    /// Evidence requirements. If empty, no evidence requirements are created.
    #[serde(default)]
    pub evidence_requirements: Vec<EvidenceRequirementBody>,

    /// If true, report what would happen without writing.
    #[serde(default)]
    pub dry_run: bool,

    /// Attribution for `created_by` on all snapshots.
    #[serde(default = "default_created_by")]
    pub created_by: String,
}

fn default_created_by() -> String {
    "onboarding_pipeline".to_string()
}

/// Aggregated result of the full onboarding pipeline.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OnboardingResult {
    pub entity_type_step: StepResult,
    pub attributes_step: StepResult,
    pub verb_contracts_step: StepResult,
    pub taxonomy_step: StepResult,
    pub views_step: StepResult,
    pub evidence_step: StepResult,

    /// Snapshot set ID grouping all publishes (None in dry-run mode).
    pub snapshot_set_id: Option<Uuid>,

    /// Whether this was a dry run.
    pub dry_run: bool,
}

impl OnboardingResult {
    pub fn total_published(&self) -> usize {
        self.entity_type_step.published
            + self.attributes_step.published
            + self.verb_contracts_step.published
            + self.taxonomy_step.published
            + self.views_step.published
            + self.evidence_step.published
    }

    pub fn total_skipped(&self) -> usize {
        self.entity_type_step.skipped
            + self.attributes_step.skipped
            + self.verb_contracts_step.skipped
            + self.taxonomy_step.skipped
            + self.views_step.skipped
            + self.evidence_step.skipped
    }

    pub fn total_updated(&self) -> usize {
        self.entity_type_step.updated
            + self.attributes_step.updated
            + self.verb_contracts_step.updated
            + self.taxonomy_step.updated
            + self.views_step.updated
            + self.evidence_step.updated
    }
}

/// Per-step publish counters.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StepResult {
    pub published: usize,
    pub skipped: usize,
    pub updated: usize,
    pub errors: Vec<String>,
}

impl StepResult {
    fn record_publish(&mut self) {
        self.published += 1;
    }
    fn record_skip(&mut self) {
        self.skipped += 1;
    }
    fn record_update(&mut self) {
        self.updated += 1;
    }
    fn record_error(&mut self, msg: String) {
        self.errors.push(msg);
    }
}

// ── Pipeline ────────────────────────────────────────────────────────────────

/// Orchestrates the 6-step onboarding pipeline.
pub struct OnboardingPipeline;

impl OnboardingPipeline {
    /// Run the full 6-step pipeline.
    pub async fn run(pool: &PgPool, request: &OnboardingRequest) -> Result<OnboardingResult> {
        // Validate the request before doing anything
        validators::validate_request(request)?;

        let mut result = OnboardingResult {
            dry_run: request.dry_run,
            ..Default::default()
        };

        // Create a snapshot set to group all publishes (unless dry run)
        let set_id = if request.dry_run {
            None
        } else {
            let label = format!("onboarding:{}", request.entity_type.fqn);
            Some(
                SnapshotStore::create_snapshot_set(pool, Some(&label), &request.created_by)
                    .await
                    .context("Failed to create snapshot set")?,
            )
        };
        result.snapshot_set_id = set_id;

        // Step 1: Entity type definition
        result.entity_type_step = Self::step_entity_type(pool, request, set_id).await?;

        // Step 2: Attribute definitions
        result.attributes_step = Self::step_attributes(pool, request, set_id).await?;

        // Step 3: Verb contracts
        result.verb_contracts_step = Self::step_verb_contracts(pool, request, set_id).await?;

        // Step 4: Taxonomy placement
        result.taxonomy_step = Self::step_taxonomy_placement(pool, request, set_id).await?;

        // Step 5: View assignment
        result.views_step = Self::step_view_assignment(pool, request, set_id).await?;

        // Step 6: Evidence requirements
        result.evidence_step = Self::step_evidence_requirements(pool, request, set_id).await?;

        Ok(result)
    }

    // ── Step 1: Entity Type ─────────────────────────────────────────────

    async fn step_entity_type(
        pool: &PgPool,
        request: &OnboardingRequest,
        set_id: Option<Uuid>,
    ) -> Result<StepResult> {
        let mut step = StepResult::default();
        let body = &request.entity_type;

        if request.dry_run {
            step.record_publish();
            return Ok(step);
        }

        publish_idempotent(
            pool,
            ObjectType::EntityTypeDef,
            &body.fqn,
            body,
            &request.created_by,
            set_id,
            &mut step,
        )
        .await
        .context("Step 1 (entity_type) failed")?;

        Ok(step)
    }

    // ── Step 2: Attributes ──────────────────────────────────────────────

    async fn step_attributes(
        pool: &PgPool,
        request: &OnboardingRequest,
        set_id: Option<Uuid>,
    ) -> Result<StepResult> {
        let mut step = StepResult::default();

        // Use provided attributes or generate defaults
        let attrs = if request.attributes.is_empty() {
            defaults::default_attributes_for_entity_type(&request.entity_type)
        } else {
            request.attributes.clone()
        };

        for attr in &attrs {
            if request.dry_run {
                step.record_publish();
                continue;
            }

            if let Err(e) = publish_idempotent(
                pool,
                ObjectType::AttributeDef,
                &attr.fqn,
                attr,
                &request.created_by,
                set_id,
                &mut step,
            )
            .await
            {
                step.record_error(format!("Attribute '{}': {e}", attr.fqn));
            }
        }

        Ok(step)
    }

    // ── Step 3: Verb Contracts ──────────────────────────────────────────

    async fn step_verb_contracts(
        pool: &PgPool,
        request: &OnboardingRequest,
        set_id: Option<Uuid>,
    ) -> Result<StepResult> {
        let mut step = StepResult::default();

        // Use provided contracts or generate CRUD defaults
        let contracts = if request.verb_contracts.is_empty() {
            defaults::default_verb_contracts_for_entity_type(&request.entity_type)
        } else {
            request.verb_contracts.clone()
        };

        for contract in &contracts {
            if request.dry_run {
                step.record_publish();
                continue;
            }

            if let Err(e) = publish_idempotent(
                pool,
                ObjectType::VerbContract,
                &contract.fqn,
                contract,
                &request.created_by,
                set_id,
                &mut step,
            )
            .await
            {
                step.record_error(format!("VerbContract '{}': {e}", contract.fqn));
            }
        }

        Ok(step)
    }

    // ── Step 4: Taxonomy Placement ──────────────────────────────────────

    async fn step_taxonomy_placement(
        pool: &PgPool,
        request: &OnboardingRequest,
        set_id: Option<Uuid>,
    ) -> Result<StepResult> {
        let mut step = StepResult::default();

        // Use provided taxonomy FQNs or derive from domain
        let taxonomy_fqns = if request.taxonomy_fqns.is_empty() {
            defaults::default_taxonomy_fqns_for_entity_type(&request.entity_type)
        } else {
            request.taxonomy_fqns.clone()
        };

        for taxonomy_fqn in &taxonomy_fqns {
            let rule = defaults::membership_rule_for_entity_in_taxonomy(
                &request.entity_type.fqn,
                taxonomy_fqn,
            );

            if request.dry_run {
                step.record_publish();
                continue;
            }

            if let Err(e) = publish_idempotent(
                pool,
                ObjectType::MembershipRule,
                &rule.fqn,
                &rule,
                &request.created_by,
                set_id,
                &mut step,
            )
            .await
            {
                step.record_error(format!("MembershipRule '{}': {e}", rule.fqn));
            }
        }

        Ok(step)
    }

    // ── Step 5: View Assignment ─────────────────────────────────────────

    async fn step_view_assignment(
        pool: &PgPool,
        request: &OnboardingRequest,
        set_id: Option<Uuid>,
    ) -> Result<StepResult> {
        let mut step = StepResult::default();

        let view_fqns = if request.view_fqns.is_empty() {
            defaults::default_view_fqns_for_entity_type(&request.entity_type)
        } else {
            request.view_fqns.clone()
        };

        if view_fqns.is_empty() {
            return Ok(step);
        }

        if request.dry_run {
            step.published = view_fqns.len();
            return Ok(step);
        }

        // For each view, check if it exists and update its columns to include
        // the new entity type's attributes.
        for view_fqn in &view_fqns {
            match SnapshotStore::find_active_by_definition_field(
                pool,
                ObjectType::ViewDef,
                "fqn",
                view_fqn,
            )
            .await?
            {
                Some(existing_row) => {
                    let mut view_body: ViewDefBody =
                        existing_row.parse_definition().map_err(|e| {
                            anyhow::anyhow!("Failed to parse ViewDef '{}': {e}", view_fqn)
                        })?;

                    let new_columns =
                        defaults::columns_for_entity_in_view(&request.entity_type, view_fqn);
                    let mut changed = false;
                    for col in new_columns {
                        if !view_body
                            .columns
                            .iter()
                            .any(|c| c.attribute_fqn == col.attribute_fqn)
                        {
                            view_body.columns.push(col);
                            changed = true;
                        }
                    }

                    if changed {
                        let definition = serde_json::to_value(&view_body)?;
                        let object_id = object_id_for(ObjectType::ViewDef, view_fqn);
                        let mut meta = SnapshotMeta::new_operational(
                            ObjectType::ViewDef,
                            object_id,
                            &request.created_by,
                        );
                        meta.predecessor_id = Some(existing_row.snapshot_id);
                        meta.version_major = existing_row.version_major;
                        meta.version_minor = existing_row.version_minor + 1;
                        meta.change_type = ChangeType::NonBreaking;
                        meta.change_rationale =
                            Some(format!("Added columns for {}", request.entity_type.fqn));
                        SnapshotStore::publish_snapshot(pool, &meta, &definition, set_id).await?;
                        step.record_update();
                    } else {
                        step.record_skip();
                    }
                }
                None => {
                    // View doesn't exist yet — skip with a note
                    step.record_error(format!("View '{view_fqn}' not found, skipped column merge"));
                }
            }
        }

        Ok(step)
    }

    // ── Step 6: Evidence Requirements ───────────────────────────────────

    async fn step_evidence_requirements(
        pool: &PgPool,
        request: &OnboardingRequest,
        set_id: Option<Uuid>,
    ) -> Result<StepResult> {
        let mut step = StepResult::default();

        if request.evidence_requirements.is_empty() {
            return Ok(step);
        }

        for req in &request.evidence_requirements {
            if request.dry_run {
                step.record_publish();
                continue;
            }

            if let Err(e) = publish_idempotent(
                pool,
                ObjectType::EvidenceRequirement,
                &req.fqn,
                req,
                &request.created_by,
                set_id,
                &mut step,
            )
            .await
            {
                step.record_error(format!("EvidenceRequirement '{}': {e}", req.fqn));
            }
        }

        Ok(step)
    }
}

// ── Shared idempotent publish helper ────────────────────────────────────────

/// Publish a snapshot idempotently: skip if unchanged, publish successor if
/// drifted, insert fresh if new. Same pattern used by the scanner.
async fn publish_idempotent<T: Serialize>(
    pool: &PgPool,
    object_type: ObjectType,
    fqn: &str,
    body: &T,
    created_by: &str,
    set_id: Option<Uuid>,
    step: &mut StepResult,
) -> Result<()> {
    let object_id = object_id_for(object_type, fqn);
    let definition = serde_json::to_value(body)?;
    let new_hash = definition_hash(&definition);

    let existing =
        SnapshotStore::find_active_by_definition_field(pool, object_type, "fqn", fqn).await?;

    match existing {
        Some(row) => {
            let old_hash = definition_hash(&row.definition);
            if old_hash == new_hash {
                step.record_skip();
            } else {
                let mut meta = SnapshotMeta::new_operational(object_type, object_id, created_by);
                meta.predecessor_id = Some(row.snapshot_id);
                meta.version_major = row.version_major;
                meta.version_minor = row.version_minor + 1;
                meta.change_type = ChangeType::NonBreaking;
                meta.change_rationale = Some("Onboarding pipeline drift update".into());
                SnapshotStore::publish_snapshot(pool, &meta, &definition, set_id).await?;
                step.record_update();
            }
        }
        None => {
            let meta = SnapshotMeta::new_operational(object_type, object_id, created_by);
            SnapshotStore::insert_snapshot(pool, &meta, &definition, set_id).await?;
            step.record_publish();
        }
    }

    Ok(())
}

// ── Display ─────────────────────────────────────────────────────────────────

impl std::fmt::Display for OnboardingResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mode = if self.dry_run { " (dry run)" } else { "" };
        writeln!(f, "Onboarding Pipeline Result{mode}")?;
        writeln!(
            f,
            "  Entity type:     {} published, {} skipped, {} updated",
            self.entity_type_step.published,
            self.entity_type_step.skipped,
            self.entity_type_step.updated
        )?;
        writeln!(
            f,
            "  Attributes:      {} published, {} skipped, {} updated",
            self.attributes_step.published,
            self.attributes_step.skipped,
            self.attributes_step.updated
        )?;
        writeln!(
            f,
            "  Verb contracts:  {} published, {} skipped, {} updated",
            self.verb_contracts_step.published,
            self.verb_contracts_step.skipped,
            self.verb_contracts_step.updated
        )?;
        writeln!(
            f,
            "  Taxonomy:        {} published, {} skipped, {} updated",
            self.taxonomy_step.published, self.taxonomy_step.skipped, self.taxonomy_step.updated
        )?;
        writeln!(
            f,
            "  Views:           {} published, {} skipped, {} updated",
            self.views_step.published, self.views_step.skipped, self.views_step.updated
        )?;
        writeln!(
            f,
            "  Evidence:        {} published, {} skipped, {} updated",
            self.evidence_step.published, self.evidence_step.skipped, self.evidence_step.updated
        )?;
        writeln!(
            f,
            "  Total:           {} published, {} skipped, {} updated",
            self.total_published(),
            self.total_skipped(),
            self.total_updated()
        )?;

        // Report errors
        let all_errors: Vec<&String> = self
            .entity_type_step
            .errors
            .iter()
            .chain(&self.attributes_step.errors)
            .chain(&self.verb_contracts_step.errors)
            .chain(&self.taxonomy_step.errors)
            .chain(&self.views_step.errors)
            .chain(&self.evidence_step.errors)
            .collect();
        if !all_errors.is_empty() {
            writeln!(f, "  Errors ({}):", all_errors.len())?;
            for err in all_errors {
                writeln!(f, "    - {err}")?;
            }
        }

        Ok(())
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sem_reg::entity_type_def::EntityTypeDefBody;

    fn sample_entity_type() -> EntityTypeDefBody {
        EntityTypeDefBody {
            fqn: "entity.test-widget".to_string(),
            name: "Test Widget".to_string(),
            description: "A test entity type for unit tests".to_string(),
            domain: "test".to_string(),
            db_table: None,
            lifecycle_states: vec![],
            required_attributes: vec![
                "test.widget-name".to_string(),
                "test.widget-status".to_string(),
            ],
            optional_attributes: vec!["test.widget-description".to_string()],
            parent_type: None,
        }
    }

    fn sample_request() -> OnboardingRequest {
        OnboardingRequest {
            entity_type: sample_entity_type(),
            attributes: vec![],
            verb_contracts: vec![],
            taxonomy_fqns: vec![],
            view_fqns: vec![],
            evidence_requirements: vec![],
            dry_run: true,
            created_by: "test".to_string(),
        }
    }

    #[test]
    fn test_request_serde_round_trip() {
        let req = sample_request();
        let json = serde_json::to_string_pretty(&req).unwrap();
        let decoded: OnboardingRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.entity_type.fqn, "entity.test-widget");
        assert!(decoded.dry_run);
        assert_eq!(decoded.created_by, "test");
    }

    #[test]
    fn test_result_totals() {
        let mut result = OnboardingResult::default();
        result.entity_type_step.published = 1;
        result.attributes_step.published = 3;
        result.attributes_step.skipped = 1;
        result.verb_contracts_step.published = 4;
        result.taxonomy_step.published = 2;
        result.views_step.updated = 1;
        result.evidence_step.published = 1;

        assert_eq!(result.total_published(), 11);
        assert_eq!(result.total_skipped(), 1);
        assert_eq!(result.total_updated(), 1);
    }

    #[test]
    fn test_result_display() {
        let mut result = OnboardingResult {
            dry_run: true,
            ..Default::default()
        };
        result.entity_type_step.published = 1;
        result.attributes_step.published = 3;
        result.verb_contracts_step.published = 4;
        result
            .taxonomy_step
            .errors
            .push("missing taxonomy".to_string());

        let display = format!("{result}");
        assert!(display.contains("dry run"));
        assert!(display.contains("Entity type:"));
        assert!(display.contains("missing taxonomy"));
    }

    #[test]
    fn test_step_result_operations() {
        let mut step = StepResult::default();
        step.record_publish();
        step.record_publish();
        step.record_skip();
        step.record_update();
        step.record_error("test error".to_string());

        assert_eq!(step.published, 2);
        assert_eq!(step.skipped, 1);
        assert_eq!(step.updated, 1);
        assert_eq!(step.errors.len(), 1);
    }

    #[test]
    fn test_default_created_by() {
        let json = r#"{"entity_type":{"fqn":"e.t","name":"T","description":"D","domain":"d"},"dry_run":true}"#;
        let req: OnboardingRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.created_by, "onboarding_pipeline");
    }
}
