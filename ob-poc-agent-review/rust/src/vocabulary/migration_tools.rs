//! Vocabulary migration tools for managing vocabulary changes
//!
//! This module provides tools for migrating vocabulary definitions between
//! versions, handling deprecations, and managing backward compatibility.

use crate::ast::types::VocabularyVerb;
use crate::vocabulary::{ChangeType, VocabularyAuditEntry, VocabularyError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Migration manager for vocabulary changes
pub(crate) struct VocabularyMigrationManager {
    migration_rules: Vec<MigrationRule>,
    version_mappings: HashMap<String, String>,
}

/// Migration rule for vocabulary transformation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MigrationRule {
    pub rule_id: Uuid,
    pub rule_name: String,
    pub from_version: String,
    pub to_version: String,
    pub transformation: VocabularyTransformation,
    pub rollback_available: bool,
}

/// Transformation types for vocabulary migration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum VocabularyTransformation {
    /// Rename a verb
    Rename { old_verb: String, new_verb: String },
    /// Move verb to different domain
    MoveDomain {
        verb: String,
        old_domain: String,
        new_domain: String,
    },
    /// Deprecate verb with replacement
    Deprecate {
        verb: String,
        replacement: Option<String>,
        reason: String,
    },
    /// Split verb into multiple verbs
    Split {
        old_verb: String,
        new_verbs: Vec<String>,
    },
    /// Merge multiple verbs into one
    Merge {
        old_verbs: Vec<String>,
        new_verb: String,
    },
    /// Update verb parameters
    UpdateParameters {
        verb: String,
        new_parameters: serde_json::Value,
    },
}

/// Migration plan containing all transformations
#[derive(Debug)]
pub(crate) struct MigrationPlan {
    pub plan_id: Uuid,
    pub from_version: String,
    pub to_version: String,
    pub transformations: Vec<VocabularyTransformation>,
    pub estimated_impact: MigrationImpact,
}

/// Impact assessment for migration
#[derive(Debug)]
pub(crate) struct MigrationImpact {
    pub affected_verbs: usize,
    pub breaking_changes: usize,
    pub deprecated_verbs: usize,
    pub new_verbs: usize,
    pub estimated_downtime_minutes: u64,
}

/// Migration result tracking
#[derive(Debug)]
pub(crate) struct MigrationResult {
    pub success: bool,
    pub completed_transformations: Vec<String>,
    pub failed_transformations: Vec<(String, String)>, // transformation, error
    pub rollback_available: bool,
    pub audit_entries: Vec<VocabularyAuditEntry>,
}

impl VocabularyMigrationManager {
    /// Create a new migration manager
    pub fn new() -> Self {
        Self {
            migration_rules: Vec::new(),
            version_mappings: HashMap::new(),
        }
    }

    /// Add a migration rule
    pub(crate) fn add_migration_rule(&mut self, rule: MigrationRule) {
        self.migration_rules.push(rule);
        self.version_mappings
            .insert(rule.from_version.clone(), rule.to_version.clone());
    }

    /// Create migration plan between versions
    pub(crate) fn create_migration_plan(
        &self,
        from_version: &str,
        to_version: &str,
    ) -> Result<MigrationPlan, VocabularyError> {
        let mut transformations = Vec::new();
        let mut affected_verbs = 0;
        let mut breaking_changes = 0;
        let mut deprecated_verbs = 0;
        let mut new_verbs = 0;

        // Find applicable migration rules
        for rule in &self.migration_rules {
            if rule.from_version == from_version && rule.to_version == to_version {
                transformations.push(rule.transformation.clone());

                // Estimate impact
                match &rule.transformation {
                    VocabularyTransformation::Rename { .. } => {
                        affected_verbs += 1;
                        breaking_changes += 1;
                    }
                    VocabularyTransformation::MoveDomain { .. } => {
                        affected_verbs += 1;
                        breaking_changes += 1;
                    }
                    VocabularyTransformation::Deprecate { .. } => {
                        affected_verbs += 1;
                        deprecated_verbs += 1;
                    }
                    VocabularyTransformation::Split {
                        new_verbs: verbs, ..
                    } => {
                        affected_verbs += 1;
                        new_verbs += verbs.len();
                        breaking_changes += 1;
                    }
                    VocabularyTransformation::Merge { old_verbs, .. } => {
                        affected_verbs += old_verbs.len();
                        breaking_changes += old_verbs.len();
                    }
                    VocabularyTransformation::UpdateParameters { .. } => {
                        affected_verbs += 1;
                    }
                }
            }
        }

        let estimated_downtime = if breaking_changes > 0 {
            (breaking_changes as u64) * 5 // 5 minutes per breaking change
        } else {
            1 // Minimal downtime for non-breaking changes
        };

        Ok(MigrationPlan {
            plan_id: Uuid::new_v4(),
            from_version: from_version.to_string(),
            to_version: to_version.to_string(),
            transformations,
            estimated_impact: MigrationImpact {
                affected_verbs,
                breaking_changes,
                deprecated_verbs,
                new_verbs,
                estimated_downtime_minutes: estimated_downtime,
            },
        })
    }

    /// Execute migration plan
    pub async fn execute_migration_plan(
        &self,
        plan: &MigrationPlan,
        vocabularies: &mut Vec<VocabularyVerb>,
    ) -> Result<MigrationResult, VocabularyError> {
        let mut completed_transformations = Vec::new();
        let mut failed_transformations = Vec::new();
        let mut audit_entries = Vec::new();

        for transformation in &plan.transformations {
            match self
                .apply_transformation(transformation, vocabularies)
                .await
            {
                Ok(audit_entry) => {
                    completed_transformations.push(format!("{:?}", transformation));
                    if let Some(entry) = audit_entry {
                        audit_entries.push(entry);
                    }
                }
                Err(e) => {
                    failed_transformations.push((format!("{:?}", transformation), e.to_string()));
                }
            }
        }

        let success = failed_transformations.is_empty();

        Ok(MigrationResult {
            success,
            completed_transformations,
            failed_transformations,
            rollback_available: true, // Simplified assumption
            audit_entries,
        })
    }

    /// Apply a single transformation
    async fn apply_transformation(
        &self,
        transformation: &VocabularyTransformation,
        vocabularies: &mut Vec<VocabularyVerb>,
    ) -> Result<Option<VocabularyAuditEntry>, VocabularyError> {
        match transformation {
            VocabularyTransformation::Rename { old_verb, new_verb } => {
                self.apply_rename(old_verb, new_verb, vocabularies).await
            }
            VocabularyTransformation::MoveDomain {
                verb,
                old_domain,
                new_domain,
            } => {
                self.apply_move_domain(verb, old_domain, new_domain, vocabularies)
                    .await
            }
            VocabularyTransformation::Deprecate {
                verb,
                replacement,
                reason,
            } => {
                self.apply_deprecation(verb, replacement.as_ref(), reason, vocabularies)
                    .await
            }
            VocabularyTransformation::Split {
                old_verb,
                new_verbs,
            } => self.apply_split(old_verb, new_verbs, vocabularies).await,
            VocabularyTransformation::Merge {
                old_verbs,
                new_verb,
            } => self.apply_merge(old_verbs, new_verb, vocabularies).await,
            VocabularyTransformation::UpdateParameters {
                verb,
                new_parameters,
            } => {
                self.apply_parameter_update(verb, new_parameters, vocabularies)
                    .await
            }
        }
    }

    /// Apply verb rename transformation
    async fn apply_rename(
        &self,
        old_verb: &str,
        new_verb: &str,
        vocabularies: &mut Vec<VocabularyVerb>,
    ) -> Result<Option<VocabularyAuditEntry>, VocabularyError> {
        for vocab in vocabularies.iter_mut() {
            if vocab.verb == old_verb {
                let old_definition = serde_json::to_value(&vocab).unwrap();
                vocab.verb = new_verb.to_string();
                vocab.updated_at = Utc::now();

                let audit_entry = VocabularyAuditEntry {
                    audit_id: Uuid::new_v4(),
                    domain: vocab.domain.clone(),
                    verb: new_verb.to_string(),
                    change_type: ChangeType::Update,
                    old_definition: Some(old_definition),
                    new_definition: Some(serde_json::to_value(&vocab).unwrap()),
                    changed_by: Some("migration_tool".to_string()),
                    change_reason: Some(format!("Renamed from '{}'", old_verb)),
                    created_at: Utc::now(),
                };

                return Ok(Some(audit_entry));
            }
        }

        Err(VocabularyError::VerbNotFound {
            domain: "unknown".to_string(),
            verb: old_verb.to_string(),
        })
    }

    /// Apply domain move transformation
    async fn apply_move_domain(
        &self,
        verb: &str,
        old_domain: &str,
        new_domain: &str,
        vocabularies: &mut Vec<VocabularyVerb>,
    ) -> Result<Option<VocabularyAuditEntry>, VocabularyError> {
        for vocab in vocabularies.iter_mut() {
            if vocab.verb == verb && vocab.domain == old_domain {
                let old_definition = serde_json::to_value(&vocab).unwrap();
                vocab.domain = new_domain.to_string();
                vocab.updated_at = Utc::now();

                let audit_entry = VocabularyAuditEntry {
                    audit_id: Uuid::new_v4(),
                    domain: new_domain.to_string(),
                    verb: verb.to_string(),
                    change_type: ChangeType::Update,
                    old_definition: Some(old_definition),
                    new_definition: Some(serde_json::to_value(&vocab).unwrap()),
                    changed_by: Some("migration_tool".to_string()),
                    change_reason: Some(format!("Moved from domain '{}'", old_domain)),
                    created_at: Utc::now(),
                };

                return Ok(Some(audit_entry));
            }
        }

        Err(VocabularyError::VerbNotFound {
            domain: old_domain.to_string(),
            verb: verb.to_string(),
        })
    }

    /// Apply deprecation transformation
    async fn apply_deprecation(
        &self,
        verb: &str,
        replacement: Option<&String>,
        reason: &str,
        vocabularies: &mut Vec<VocabularyVerb>,
    ) -> Result<Option<VocabularyAuditEntry>, VocabularyError> {
        for vocab in vocabularies.iter_mut() {
            if vocab.verb == verb {
                let old_definition = serde_json::to_value(&vocab).unwrap();
                vocab.active = false;
                vocab.updated_at = Utc::now();

                let audit_entry = VocabularyAuditEntry {
                    audit_id: Uuid::new_v4(),
                    domain: vocab.domain.clone(),
                    verb: verb.to_string(),
                    change_type: ChangeType::Deprecate,
                    old_definition: Some(old_definition),
                    new_definition: Some(serde_json::to_value(&vocab).unwrap()),
                    changed_by: Some("migration_tool".to_string()),
                    change_reason: Some(format!("Deprecated: {}", reason)),
                    created_at: Utc::now(),
                };

                return Ok(Some(audit_entry));
            }
        }

        Err(VocabularyError::VerbNotFound {
            domain: "unknown".to_string(),
            verb: verb.to_string(),
        })
    }

    /// Apply verb split transformation (stub)
    async fn apply_split(
        &self,
        _old_verb: &str,
        _new_verbs: &[String],
        _vocabularies: &mut Vec<VocabularyVerb>,
    ) -> Result<Option<VocabularyAuditEntry>, VocabularyError> {
        // Stub implementation - would split one verb into multiple
        Ok(None)
    }

    /// Apply verb merge transformation (stub)
    async fn apply_merge(
        &self,
        _old_verbs: &[String],
        _new_verb: &str,
        _vocabularies: &mut Vec<VocabularyVerb>,
    ) -> Result<Option<VocabularyAuditEntry>, VocabularyError> {
        // Stub implementation - would merge multiple verbs into one
        Ok(None)
    }

    /// Apply parameter update transformation
    async fn apply_parameter_update(
        &self,
        verb: &str,
        new_parameters: &serde_json::Value,
        vocabularies: &mut Vec<VocabularyVerb>,
    ) -> Result<Option<VocabularyAuditEntry>, VocabularyError> {
        for vocab in vocabularies.iter_mut() {
            if vocab.verb == verb {
                let old_definition = serde_json::to_value(&vocab).unwrap();
                vocab.parameters = Some(new_parameters.clone());
                vocab.updated_at = Utc::now();

                let audit_entry = VocabularyAuditEntry {
                    audit_id: Uuid::new_v4(),
                    domain: vocab.domain.clone(),
                    verb: verb.to_string(),
                    change_type: ChangeType::Update,
                    old_definition: Some(old_definition),
                    new_definition: Some(serde_json::to_value(&vocab).unwrap()),
                    changed_by: Some("migration_tool".to_string()),
                    change_reason: Some("Updated parameters".to_string()),
                    created_at: Utc::now(),
                };

                return Ok(Some(audit_entry));
            }
        }

        Err(VocabularyError::VerbNotFound {
            domain: "unknown".to_string(),
            verb: verb.to_string(),
        })
    }

    /// Rollback migration (stub)
    pub async fn rollback_migration(
        &self,
        _migration_result: &MigrationResult,
    ) -> Result<(), VocabularyError> {
        // Stub implementation - would reverse applied transformations
        tracing::info!("Rolling back migration");
        Ok(())
    }

    /// Validate migration plan
    pub(crate) fn validate_migration_plan(
        &self,
        plan: &MigrationPlan,
    ) -> Result<Vec<String>, VocabularyError> {
        let mut warnings = Vec::new();

        if plan.estimated_impact.breaking_changes > 0 {
            warnings.push(format!(
                "Migration contains {} breaking changes",
                plan.estimated_impact.breaking_changes
            ));
        }

        if plan.estimated_impact.estimated_downtime_minutes > 30 {
            warnings.push(format!(
                "Estimated downtime is {} minutes",
                plan.estimated_impact.estimated_downtime_minutes
            ));
        }

        Ok(warnings)
    }
}

impl Default for VocabularyMigrationManager {
    fn default() -> Self {
        Self::new()
    }
}

