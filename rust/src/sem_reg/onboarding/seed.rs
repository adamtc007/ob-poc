//! Phase B1: Bootstrap seed — one-time write of onboarding manifest data
//! into `sem_reg.snapshots`.
//!
//! Uses a well-known `BOOTSTRAP_SET_ID` to guard against re-runs.
//! All writes occur in a single `snapshot_set` for atomic provenance.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::sem_reg::attribute_def::{
    AttributeConstraints, AttributeDataType, AttributeDefBody, AttributeSource,
};
use crate::sem_reg::entity_type_def::{DbTableMapping, EntityTypeDefBody, LifecycleStateDef};
use crate::sem_reg::ids::{definition_hash, object_id_for};
use crate::sem_reg::relationship_type_def::{RelationshipCardinality, RelationshipTypeDefBody};
use crate::sem_reg::store::SnapshotStore;
use crate::sem_reg::types::{ObjectType, SnapshotMeta};
use crate::sem_reg::verb_contract::VerbContractBody;

use super::entity_infer::{
    EdgeClass, EntityTypeCandidate, InferredCardinality, RelationshipCandidate,
};
use super::manifest::OnboardingManifest;
use super::verb_extract::VerbExtract;
use super::xref::{AttributeCandidate, ColumnClassification};

/// Well-known snapshot_set_id for bootstrap seed.
/// If this ID exists in `sem_reg.snapshot_sets`, the bootstrap has already run.
pub const BOOTSTRAP_SET_ID: Uuid = Uuid::from_bytes([
    0xB0, 0x07, 0x57, 0x4A, 0x50, 0x00, 0x4B, 0x01, 0x80, 0x00, 0x5E, 0xED, 0xDA, 0x7A, 0xBA, 0x5E,
]);

const BOOTSTRAP_CREATED_BY: &str = "bootstrap";

/// Report of what the bootstrap seed wrote.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BootstrapReport {
    pub attribute_defs_written: usize,
    pub attribute_defs_skipped: usize,
    pub verb_contracts_written: usize,
    pub verb_contracts_skipped: usize,
    pub entity_type_defs_written: usize,
    pub entity_type_defs_skipped: usize,
    pub relationship_type_defs_written: usize,
    pub relationship_type_defs_skipped: usize,
}

impl BootstrapReport {
    /// Total snapshots written across all object types.
    pub fn total_written(&self) -> usize {
        self.attribute_defs_written
            + self.verb_contracts_written
            + self.entity_type_defs_written
            + self.relationship_type_defs_written
    }
}

/// Apply the bootstrap seed from an onboarding manifest.
///
/// # Guard
/// Checks if `BOOTSTRAP_SET_ID` already exists. If so, returns an error.
///
/// # Transaction
/// All writes go through `insert_snapshot` with `snapshot_set_id = BOOTSTRAP_SET_ID`.
/// The snapshot_set row is created first, then all individual snapshots.
#[cfg(feature = "database")]
pub async fn apply_bootstrap(
    pool: &PgPool,
    manifest: &OnboardingManifest,
) -> Result<BootstrapReport> {
    // ── Guard: check if bootstrap has already been applied ────
    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sem_reg.snapshot_sets WHERE snapshot_set_id = $1",
    )
    .bind(BOOTSTRAP_SET_ID)
    .fetch_one(pool)
    .await
    .context("Failed to check bootstrap guard")?;

    if existing > 0 {
        return Err(anyhow!(
            "Bootstrap has already been applied (BOOTSTRAP_SET_ID {} exists in snapshot_sets). \
             Use the stewardship changeset workflow for subsequent changes.",
            BOOTSTRAP_SET_ID,
        ));
    }

    // ── Create the bootstrap snapshot set with our well-known ID ──
    sqlx::query(
        r#"
        INSERT INTO sem_reg.snapshot_sets (snapshot_set_id, description, created_by)
        VALUES ($1, $2, $3)
        "#,
    )
    .bind(BOOTSTRAP_SET_ID)
    .bind(Some("bootstrap-seed: onboarding pipeline Phase B1"))
    .bind(BOOTSTRAP_CREATED_BY)
    .execute(pool)
    .await
    .context("Failed to create bootstrap snapshot set")?;

    let mut report = BootstrapReport::default();

    // ── 1. Seed AttributeDefs (VerbConnected + OperationalOrphan) ──
    for candidate in &manifest.attribute_candidates {
        match candidate.classification {
            ColumnClassification::VerbConnected | ColumnClassification::OperationalOrphan => {
                let written = seed_attribute_def(pool, candidate).await?;
                if written {
                    report.attribute_defs_written += 1;
                } else {
                    report.attribute_defs_skipped += 1;
                }
            }
            // Framework and DeadSchema are NOT seeded
            _ => {}
        }
    }

    // ── 2. Seed VerbContracts ──
    for verb in &manifest.verb_extracts {
        let written = seed_verb_contract(pool, verb).await?;
        if written {
            report.verb_contracts_written += 1;
        } else {
            report.verb_contracts_skipped += 1;
        }
    }

    // ── 3. Seed EntityTypeDefs ──
    for entity_type in &manifest.entity_type_candidates {
        let written = seed_entity_type_def(pool, entity_type).await?;
        if written {
            report.entity_type_defs_written += 1;
        } else {
            report.entity_type_defs_skipped += 1;
        }
    }

    // ── 4. Seed RelationshipTypeDefs ──
    for relationship in &manifest.relationship_candidates {
        let written = seed_relationship_type_def(pool, relationship).await?;
        if written {
            report.relationship_type_defs_written += 1;
        } else {
            report.relationship_type_defs_skipped += 1;
        }
    }

    Ok(report)
}

// ── Individual seed functions ────────────────────────────────────

/// Seed a single AttributeDef from an AttributeCandidate.
/// Returns `true` if a new snapshot was written, `false` if skipped (already exists with same hash).
#[cfg(feature = "database")]
async fn seed_attribute_def(pool: &PgPool, candidate: &AttributeCandidate) -> Result<bool> {
    let fqn = format!(
        "{}.{}.{}",
        infer_domain(&candidate.schema),
        candidate.table,
        candidate.column
    );

    let is_orphan = candidate.classification == ColumnClassification::OperationalOrphan;

    let producing_verb = if candidate.verb_refs.is_empty() {
        None
    } else {
        Some(candidate.verb_refs[0].clone())
    };

    let body = AttributeDefBody {
        fqn: fqn.clone(),
        name: column_to_display_name(&candidate.column),
        description: format!(
            "Column {}.{}.{} ({}{})",
            candidate.schema,
            candidate.table,
            candidate.column,
            candidate.sql_type,
            if is_orphan { ", verb_orphan" } else { "" },
        ),
        domain: infer_domain(&candidate.schema),
        data_type: sql_type_to_attribute_data_type(&candidate.sql_type),
        source: Some(AttributeSource {
            producing_verb,
            schema: Some(candidate.schema.clone()),
            table: Some(candidate.table.clone()),
            column: Some(candidate.column.clone()),
            derived: false,
        }),
        constraints: Some(AttributeConstraints {
            required: !candidate.is_nullable,
            unique: false,
            min_length: None,
            max_length: None,
            pattern: None,
            valid_values: None,
        }),
        sinks: vec![],
    };

    publish_idempotent(pool, ObjectType::AttributeDef, &fqn, &body).await
}

/// Seed a single VerbContract from a VerbExtract.
#[cfg(feature = "database")]
async fn seed_verb_contract(pool: &PgPool, verb: &VerbExtract) -> Result<bool> {
    let body = VerbContractBody {
        fqn: verb.fqn.clone(),
        domain: verb.domain.clone(),
        action: verb.action.clone(),
        description: verb.description.clone(),
        behavior: format!("{:?}", verb.behavior).to_lowercase(),
        args: verb
            .inputs
            .iter()
            .map(|i| crate::sem_reg::verb_contract::VerbArgDef {
                name: i.name.clone(),
                arg_type: i.arg_type.clone(),
                required: i.required,
                description: None,
                lookup: None,
                valid_values: None,
                default: None,
            })
            .collect(),
        returns: None,
        preconditions: vec![],
        postconditions: vec![],
        produces: verb
            .output
            .as_ref()
            .map(|o| crate::sem_reg::verb_contract::VerbProducesSpec {
                entity_type: o.produced_type.clone(),
                resolved: false,
            }),
        consumes: vec![],
        invocation_phrases: vec![],
        subject_kinds: vec![],
        phase_tags: vec![],
        requires_subject: true,
        produces_focus: false,
        metadata: None,
        crud_mapping: None,
    };

    publish_idempotent(pool, ObjectType::VerbContract, &verb.fqn, &body).await
}

/// Seed a single EntityTypeDef from an EntityTypeCandidate.
#[cfg(feature = "database")]
async fn seed_entity_type_def(pool: &PgPool, entity: &EntityTypeCandidate) -> Result<bool> {
    let fqn = &entity.fqn;

    let pk_column = entity
        .primary_keys
        .first()
        .cloned()
        .unwrap_or_else(|| "id".into());

    let body = EntityTypeDefBody {
        fqn: fqn.clone(),
        name: entity.display_name.clone(),
        description: format!(
            "Entity type inferred from table {}.{} ({} attributes)",
            entity.schema,
            entity.table,
            entity.attribute_fqns.len(),
        ),
        domain: entity.domain.clone(),
        db_table: Some(DbTableMapping {
            schema: entity.schema.clone(),
            table: entity.table.clone(),
            primary_key: pk_column,
            name_column: None,
        }),
        lifecycle_states: entity
            .lifecycle_states
            .iter()
            .map(|s| LifecycleStateDef {
                name: s.clone(),
                description: None,
                transitions: vec![],
                terminal: false,
            })
            .collect(),
        required_attributes: vec![],
        optional_attributes: entity.attribute_fqns.clone(),
        parent_type: None,
    };

    publish_idempotent(pool, ObjectType::EntityTypeDef, fqn, &body).await
}

/// Seed a single RelationshipTypeDef from a RelationshipCandidate.
#[cfg(feature = "database")]
async fn seed_relationship_type_def(pool: &PgPool, rel: &RelationshipCandidate) -> Result<bool> {
    let fqn = &rel.fqn;

    let cardinality = match rel.cardinality {
        InferredCardinality::OneToOne => RelationshipCardinality::OneToOne,
        InferredCardinality::OneToMany => RelationshipCardinality::OneToMany,
        InferredCardinality::ManyToMany => RelationshipCardinality::ManyToMany,
    };

    let edge_class_str = match rel.edge_class {
        EdgeClass::Structural => "structural",
        EdgeClass::Reference => "reference",
        EdgeClass::Association => "association",
        EdgeClass::Temporal => "temporal",
    };

    let source_domain = infer_domain(&rel.source_schema);
    let target_domain = infer_domain(&rel.target_schema);

    let body = RelationshipTypeDefBody {
        fqn: fqn.clone(),
        name: rel.name.clone(),
        description: format!(
            "FK relationship: {}.{}.{} → {}.{}.{} ({})",
            rel.source_schema,
            rel.source_table,
            rel.source_column,
            rel.target_schema,
            rel.target_table,
            rel.target_column,
            rel.constraint_name,
        ),
        domain: source_domain.clone(),
        source_entity_type_fqn: format!("{}.{}", source_domain, rel.source_table),
        target_entity_type_fqn: format!("{}.{}", target_domain, rel.target_table),
        cardinality,
        edge_class: Some(edge_class_str.to_string()),
        directionality: Some(crate::sem_reg::relationship_type_def::Directionality::Forward),
        inverse_fqn: None,
        constraints: vec![],
    };

    publish_idempotent(pool, ObjectType::RelationshipTypeDef, fqn, &body).await
}

// ── Idempotent publish helper ────────────────────────────────────

/// Publish a snapshot idempotently:
/// - If no active snapshot exists for this object → INSERT new snapshot.
/// - If an active snapshot exists with the SAME definition hash → skip.
/// - If an active snapshot exists with a DIFFERENT definition hash → supersede + INSERT successor.
///
/// Returns `true` if a new snapshot was written, `false` if skipped.
#[cfg(feature = "database")]
async fn publish_idempotent<T: Serialize>(
    pool: &PgPool,
    object_type: ObjectType,
    fqn: &str,
    body: &T,
) -> Result<bool> {
    let object_id = object_id_for(object_type, fqn);
    let definition = serde_json::to_value(body)?;
    let new_hash = definition_hash(&definition);

    // Check for existing active snapshot
    let existing = SnapshotStore::resolve_active(pool, object_type, object_id).await?;

    match existing {
        Some(row) => {
            let existing_hash = definition_hash(&row.definition);
            if existing_hash == new_hash {
                // Same content — skip
                return Ok(false);
            }

            // Definition drifted — publish successor
            let mut meta =
                SnapshotMeta::new_operational(object_type, object_id, BOOTSTRAP_CREATED_BY);
            meta.predecessor_id = Some(row.snapshot_id);
            meta.change_type = crate::sem_reg::types::ChangeType::NonBreaking;
            meta.change_rationale = Some("bootstrap: definition drift detected".into());

            SnapshotStore::publish_snapshot(pool, &meta, &definition, Some(BOOTSTRAP_SET_ID))
                .await?;
            Ok(true)
        }
        None => {
            // No existing snapshot — create new
            let meta = SnapshotMeta::new_operational(object_type, object_id, BOOTSTRAP_CREATED_BY);
            SnapshotStore::insert_snapshot(pool, &meta, &definition, Some(BOOTSTRAP_SET_ID))
                .await?;
            Ok(true)
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────

/// Map SQL type string to AttributeDataType.
fn sql_type_to_attribute_data_type(sql_type: &str) -> AttributeDataType {
    let lower = sql_type.to_lowercase();
    if lower.contains("uuid") {
        AttributeDataType::Uuid
    } else if lower.contains("int") || lower == "serial" || lower == "bigserial" {
        AttributeDataType::Integer
    } else if lower.contains("numeric")
        || lower.contains("decimal")
        || lower.contains("real")
        || lower.contains("double")
        || lower.contains("float")
    {
        AttributeDataType::Decimal
    } else if lower.contains("bool") {
        AttributeDataType::Boolean
    } else if lower == "date" {
        AttributeDataType::Date
    } else if lower.contains("timestamp") {
        AttributeDataType::Timestamp
    } else if lower.contains("json") {
        AttributeDataType::Json
    } else {
        // text, varchar, char, etc.
        AttributeDataType::String
    }
}

/// Infer domain from schema name (matches entity_infer.rs logic).
fn infer_domain(schema: &str) -> String {
    match schema {
        "ob-poc" => "ob_poc".to_string(),
        "kyc" => "kyc".to_string(),
        "sem_reg" => "sem_reg".to_string(),
        other => other.replace('-', "_"),
    }
}

/// Convert column name to human-readable display name.
fn column_to_display_name(column: &str) -> String {
    column
        .replace('_', " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    upper + chars.as_str()
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bootstrap_set_id_is_stable() {
        // Ensure BOOTSTRAP_SET_ID never changes
        assert_eq!(
            BOOTSTRAP_SET_ID.to_string(),
            "b007574a-5000-4b01-8000-5eedda7aba5e"
        );
    }

    #[test]
    fn test_sql_type_mapping() {
        assert!(matches!(
            sql_type_to_attribute_data_type("uuid"),
            AttributeDataType::Uuid
        ));
        assert!(matches!(
            sql_type_to_attribute_data_type("integer"),
            AttributeDataType::Integer
        ));
        assert!(matches!(
            sql_type_to_attribute_data_type("bigint"),
            AttributeDataType::Integer
        ));
        assert!(matches!(
            sql_type_to_attribute_data_type("numeric(10,2)"),
            AttributeDataType::Decimal
        ));
        assert!(matches!(
            sql_type_to_attribute_data_type("boolean"),
            AttributeDataType::Boolean
        ));
        assert!(matches!(
            sql_type_to_attribute_data_type("date"),
            AttributeDataType::Date
        ));
        assert!(matches!(
            sql_type_to_attribute_data_type("timestamp with time zone"),
            AttributeDataType::Timestamp
        ));
        assert!(matches!(
            sql_type_to_attribute_data_type("jsonb"),
            AttributeDataType::Json
        ));
        assert!(matches!(
            sql_type_to_attribute_data_type("character varying"),
            AttributeDataType::String
        ));
        assert!(matches!(
            sql_type_to_attribute_data_type("text"),
            AttributeDataType::String
        ));
    }

    #[test]
    fn test_column_to_display_name() {
        assert_eq!(
            column_to_display_name("jurisdiction_code"),
            "Jurisdiction Code"
        );
        assert_eq!(column_to_display_name("entity_id"), "Entity Id");
        assert_eq!(column_to_display_name("name"), "Name");
    }

    #[test]
    fn test_bootstrap_report_total() {
        let report = BootstrapReport {
            attribute_defs_written: 10,
            attribute_defs_skipped: 5,
            verb_contracts_written: 20,
            verb_contracts_skipped: 3,
            entity_type_defs_written: 8,
            entity_type_defs_skipped: 2,
            relationship_type_defs_written: 15,
            relationship_type_defs_skipped: 1,
        };
        assert_eq!(report.total_written(), 53);
    }
}
