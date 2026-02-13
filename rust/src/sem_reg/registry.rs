//! RegistryService — typed publish/resolve for each registry object type.
//!
//! Wraps `SnapshotStore` with type-safe bodies: callers pass typed structs,
//! the service handles JSON serialisation and gate evaluation.

use anyhow::{bail, Result};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::{
    attribute_def::AttributeDefBody,
    derivation_spec::DerivationSpecBody,
    document_type_def::DocumentTypeDefBody,
    entity_type_def::EntityTypeDefBody,
    evidence::EvidenceRequirementBody,
    gates::{evaluate_publish_gates, ExtendedPublishGateResult, GateFailure, GateMode},
    membership::MembershipRuleBody,
    observation_def::ObservationDefBody,
    policy_rule::PolicyRuleBody,
    store::SnapshotStore,
    taxonomy_def::{TaxonomyDefBody, TaxonomyNodeBody},
    types::{ObjectType, SnapshotMeta, SnapshotRow},
    verb_contract::VerbContractBody,
    view_def::ViewDefBody,
};

// ── PublishOutcome ─────────────────────────────────────────────

/// Outcome of a gated publish operation.
#[derive(Debug)]
pub enum PublishOutcome {
    /// Snapshot published successfully (no gate failures).
    Published(Uuid),
    /// Publish blocked by gate failures (Enforce mode).
    Blocked { failures: Vec<GateFailure> },
    /// Published with non-blocking warnings (ReportOnly mode).
    PublishedWithWarnings {
        snapshot_id: Uuid,
        warnings: Vec<GateFailure>,
    },
}

/// Typed registry operations.
pub struct RegistryService;

// Macro to reduce boilerplate for publish/resolve/resolve_by_fqn triplets.
macro_rules! typed_registry_methods {
    (
        $publish_fn:ident, $resolve_fn:ident, $resolve_by_fqn_fn:ident,
        $body_type:ty, $object_type:expr, $doc_noun:expr
    ) => {
        #[doc = concat!("Publish ", $doc_noun, ". Evaluates publish gates before persisting.")]
        pub async fn $publish_fn(
            pool: &PgPool,
            meta: &SnapshotMeta,
            body: &$body_type,
            snapshot_set_id: Option<Uuid>,
        ) -> Result<Uuid> {
            assert_eq!(meta.object_type, $object_type);
            Self::publish_typed(pool, meta, body, snapshot_set_id).await
        }

        #[doc = concat!("Resolve the active ", $doc_noun, " for a given object_id.")]
        pub async fn $resolve_fn(
            pool: &PgPool,
            object_id: Uuid,
        ) -> Result<Option<(SnapshotRow, $body_type)>> {
            Self::resolve_typed(pool, $object_type, object_id).await
        }

        #[doc = concat!("Find an active ", $doc_noun, " by FQN.")]
        pub async fn $resolve_by_fqn_fn(
            pool: &PgPool,
            fqn: &str,
        ) -> Result<Option<(SnapshotRow, $body_type)>> {
            Self::resolve_by_fqn_typed(pool, $object_type, fqn).await
        }
    };
}

impl RegistryService {
    // ── Attribute Definitions ─────────────────────────────────

    typed_registry_methods!(
        publish_attribute_def,
        resolve_attribute_def,
        resolve_attribute_def_by_fqn,
        AttributeDefBody,
        ObjectType::AttributeDef,
        "an attribute definition"
    );

    // ── Entity Type Definitions ───────────────────────────────

    typed_registry_methods!(
        publish_entity_type_def,
        resolve_entity_type_def,
        resolve_entity_type_def_by_fqn,
        EntityTypeDefBody,
        ObjectType::EntityTypeDef,
        "an entity type definition"
    );

    // ── Verb Contracts ────────────────────────────────────────

    typed_registry_methods!(
        publish_verb_contract,
        resolve_verb_contract,
        resolve_verb_contract_by_fqn,
        VerbContractBody,
        ObjectType::VerbContract,
        "a verb contract"
    );

    // ── Taxonomy Definitions ──────────────────────────────────

    typed_registry_methods!(
        publish_taxonomy_def,
        resolve_taxonomy_def,
        resolve_taxonomy_def_by_fqn,
        TaxonomyDefBody,
        ObjectType::TaxonomyDef,
        "a taxonomy definition"
    );

    // ── Taxonomy Nodes ────────────────────────────────────────

    typed_registry_methods!(
        publish_taxonomy_node,
        resolve_taxonomy_node,
        resolve_taxonomy_node_by_fqn,
        TaxonomyNodeBody,
        ObjectType::TaxonomyNode,
        "a taxonomy node"
    );

    // ── Membership Rules ──────────────────────────────────────

    typed_registry_methods!(
        publish_membership_rule,
        resolve_membership_rule,
        resolve_membership_rule_by_fqn,
        MembershipRuleBody,
        ObjectType::MembershipRule,
        "a membership rule"
    );

    // ── View Definitions ──────────────────────────────────────

    typed_registry_methods!(
        publish_view_def,
        resolve_view_def,
        resolve_view_def_by_fqn,
        ViewDefBody,
        ObjectType::ViewDef,
        "a view definition"
    );

    // ── Policy Rules ──────────────────────────────────────────

    typed_registry_methods!(
        publish_policy_rule,
        resolve_policy_rule,
        resolve_policy_rule_by_fqn,
        PolicyRuleBody,
        ObjectType::PolicyRule,
        "a policy rule"
    );

    // ── Evidence Requirements ─────────────────────────────────

    typed_registry_methods!(
        publish_evidence_requirement,
        resolve_evidence_requirement,
        resolve_evidence_requirement_by_fqn,
        EvidenceRequirementBody,
        ObjectType::EvidenceRequirement,
        "an evidence requirement"
    );

    // ── Document Type Definitions ─────────────────────────────

    typed_registry_methods!(
        publish_document_type_def,
        resolve_document_type_def,
        resolve_document_type_def_by_fqn,
        DocumentTypeDefBody,
        ObjectType::DocumentTypeDef,
        "a document type definition"
    );

    // ── Observation Definitions ───────────────────────────────

    typed_registry_methods!(
        publish_observation_def,
        resolve_observation_def,
        resolve_observation_def_by_fqn,
        ObservationDefBody,
        ObjectType::ObservationDef,
        "an observation definition"
    );

    // ── Derivation Specs ──────────────────────────────────────

    typed_registry_methods!(
        publish_derivation_spec,
        resolve_derivation_spec,
        resolve_derivation_spec_by_fqn,
        DerivationSpecBody,
        ObjectType::DerivationSpec,
        "a derivation spec"
    );

    // ── Gated Publish (extended gates) ────────────────────────

    /// Publish any typed body with the extended gate framework.
    ///
    /// Returns `PublishOutcome` which distinguishes between clean publish,
    /// blocked (enforce mode errors), and published-with-warnings.
    pub async fn publish_with_gates<T: Serialize>(
        pool: &PgPool,
        meta: &SnapshotMeta,
        body: &T,
        gate_mode: GateMode,
        extended_gate_failures: Vec<GateFailure>,
    ) -> Result<PublishOutcome> {
        // Step 1: Run the standard publish gates (proof rule, security, approval, version)
        let predecessor = if let Some(pred_id) = meta.predecessor_id {
            sqlx::query_as::<_, SnapshotRow>(
                "SELECT * FROM sem_reg.snapshots WHERE snapshot_id = $1",
            )
            .bind(pred_id)
            .fetch_optional(pool)
            .await?
        } else {
            None
        };

        let standard_gates = evaluate_publish_gates(meta, predecessor.as_ref());
        if !standard_gates.all_passed() {
            // Standard gates always block (they are structural invariants)
            return Ok(PublishOutcome::Blocked {
                failures: standard_gates
                    .failure_messages()
                    .iter()
                    .map(|msg| GateFailure::error("standard_publish_gate", "snapshot", msg.clone()))
                    .collect(),
            });
        }

        // Step 2: Evaluate extended gates with mode control
        let extended_result = ExtendedPublishGateResult {
            failures: extended_gate_failures,
            mode: gate_mode,
        };

        if extended_result.should_block() {
            return Ok(PublishOutcome::Blocked {
                failures: extended_result.failures,
            });
        }

        // Step 3: Publish the snapshot
        let definition = serde_json::to_value(body)?;
        let snapshot_id = SnapshotStore::publish_snapshot(pool, meta, &definition, None).await?;

        // Step 4: Return with any warnings
        if extended_result.has_warnings() || extended_result.has_errors() {
            Ok(PublishOutcome::PublishedWithWarnings {
                snapshot_id,
                warnings: extended_result.failures,
            })
        } else {
            Ok(PublishOutcome::Published(snapshot_id))
        }
    }

    // ── Generic typed helpers ─────────────────────────────────

    /// Publish a typed body with gate evaluation.
    async fn publish_typed<T: Serialize>(
        pool: &PgPool,
        meta: &SnapshotMeta,
        body: &T,
        snapshot_set_id: Option<Uuid>,
    ) -> Result<Uuid> {
        // Evaluate publish gates
        let predecessor = if let Some(pred_id) = meta.predecessor_id {
            sqlx::query_as::<_, SnapshotRow>(
                "SELECT * FROM sem_reg.snapshots WHERE snapshot_id = $1",
            )
            .bind(pred_id)
            .fetch_optional(pool)
            .await?
        } else {
            None
        };

        let gate_result = evaluate_publish_gates(meta, predecessor.as_ref());
        if !gate_result.all_passed() {
            bail!(
                "Publish gates failed: {}",
                gate_result.failure_messages().join("; ")
            );
        }

        // Serialise body to JSONB
        let definition = serde_json::to_value(body)?;

        // Publish (supersede predecessor + insert)
        SnapshotStore::publish_snapshot(pool, meta, &definition, snapshot_set_id).await
    }

    /// Resolve the active snapshot and deserialise its definition.
    async fn resolve_typed<T: serde::de::DeserializeOwned>(
        pool: &PgPool,
        object_type: ObjectType,
        object_id: Uuid,
    ) -> Result<Option<(SnapshotRow, T)>> {
        let row = SnapshotStore::resolve_active(pool, object_type, object_id).await?;
        match row {
            Some(r) => {
                let body: T = r.parse_definition()?;
                Ok(Some((r, body)))
            }
            None => Ok(None),
        }
    }

    /// Find an active snapshot by FQN field in definition JSONB.
    async fn resolve_by_fqn_typed<T: serde::de::DeserializeOwned>(
        pool: &PgPool,
        object_type: ObjectType,
        fqn: &str,
    ) -> Result<Option<(SnapshotRow, T)>> {
        let row =
            SnapshotStore::find_active_by_definition_field(pool, object_type, "fqn", fqn).await?;
        match row {
            Some(r) => {
                let body: T = r.parse_definition()?;
                Ok(Some((r, body)))
            }
            None => Ok(None),
        }
    }

    // ── Statistics ─────────────────────────────────────────────

    /// Get counts of active snapshots by object type.
    pub async fn stats(pool: &PgPool) -> Result<Vec<(ObjectType, i64)>> {
        SnapshotStore::count_active(pool, None).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sem_reg::gates::GateSeverity;
    use crate::sem_reg::types::*;

    #[test]
    fn test_gate_evaluation_rejects_operational_proof() {
        let meta = SnapshotMeta {
            object_type: ObjectType::VerbContract,
            object_id: Uuid::new_v4(),
            version_major: 1,
            version_minor: 0,
            status: SnapshotStatus::Active,
            governance_tier: GovernanceTier::Operational,
            trust_class: TrustClass::Proof,
            security_label: SecurityLabel::default(),
            change_type: ChangeType::Created,
            change_rationale: None,
            created_by: "test".into(),
            approved_by: None,
            predecessor_id: None,
        };
        let gate = evaluate_publish_gates(&meta, None);
        assert!(!gate.all_passed());
    }

    #[test]
    fn test_publish_outcome_blocked_has_failures() {
        let outcome = PublishOutcome::Blocked {
            failures: vec![GateFailure::error(
                "test_gate",
                "test",
                "blocked for reason",
            )],
        };
        match outcome {
            PublishOutcome::Blocked { failures } => {
                assert_eq!(failures.len(), 1);
                assert_eq!(failures[0].severity, GateSeverity::Error);
            }
            _ => panic!("Expected Blocked"),
        }
    }

    #[test]
    fn test_publish_outcome_published_variant() {
        let id = Uuid::new_v4();
        let outcome = PublishOutcome::Published(id);
        match outcome {
            PublishOutcome::Published(sid) => assert_eq!(sid, id),
            _ => panic!("Expected Published"),
        }
    }

    #[test]
    fn test_publish_outcome_with_warnings() {
        let id = Uuid::new_v4();
        let outcome = PublishOutcome::PublishedWithWarnings {
            snapshot_id: id,
            warnings: vec![GateFailure::warning("test_gate", "test", "minor issue")],
        };
        match outcome {
            PublishOutcome::PublishedWithWarnings {
                snapshot_id,
                warnings,
            } => {
                assert_eq!(snapshot_id, id);
                assert_eq!(warnings.len(), 1);
                assert_eq!(warnings[0].severity, GateSeverity::Warning);
            }
            _ => panic!("Expected PublishedWithWarnings"),
        }
    }
}
