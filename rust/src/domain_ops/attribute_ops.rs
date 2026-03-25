//! Attribute custom operations
//!
//! Operations for attribute dictionary management, document-attribute
//! mappings, and lineage tracing.

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use governed_query_proc::governed_query;
use ob_poc_macros::register_custom_op;
use serde_json::json;
use uuid::Uuid;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};
use crate::sem_reg::derivation_spec::{
    DerivationExpression, DerivationInput, DerivationSpecBody, FreshnessRule, NullSemantics,
};
use crate::sem_reg::store::SnapshotStore;
use crate::sem_reg::types::{ChangeType, ObjectType, SnapshotMeta, SnapshotRow, SnapshotStatus};
use crate::services::attribute_identity_service::AttributeIdentityService;
use sem_os_core::attribute_def::{AttributeDataType, AttributeDefBody, AttributeSource};
use sem_os_core::types::EvidenceGrade;

#[cfg(feature = "database")]
use sqlx::{PgPool, Postgres, Row, Transaction};

/// List all document types that provide (SOURCE) a given attribute
///
/// Rationale: Requires join across document_attribute_links and document_types
/// with filtering by direction and ordering by proof strength.
#[register_custom_op]
pub struct AttributeListSourcesOp;

#[async_trait]
impl CustomOperation for AttributeListSourcesOp {
    fn domain(&self) -> &'static str {
        "attribute"
    }
    fn verb(&self) -> &'static str {
        "list-sources"
    }
    fn rationale(&self) -> &'static str {
        "Requires join across attribute registry and document links with proof strength ordering"
    }

    #[cfg(feature = "database")]
    #[governed_query(verb = "attribute.list-sources", skip_principal_check = true)]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let attr_id = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "attribute")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing attribute argument"))?;

        let rows = sqlx::query!(
            r#"
            SELECT dt.type_code, dt.display_name, dt.category,
                   dal.extraction_method, dal.is_authoritative, dal.proof_strength,
                   dal.extraction_confidence_default
            FROM "ob-poc".document_attribute_links dal
            JOIN "ob-poc".document_types dt ON dt.type_id = dal.document_type_id
            JOIN "ob-poc".attribute_registry ar ON ar.uuid = dal.attribute_id
            WHERE ar.id = $1 AND dal.direction IN ('SOURCE', 'BOTH')
            ORDER BY dal.is_authoritative DESC,
                     CASE dal.proof_strength
                         WHEN 'PRIMARY' THEN 1
                         WHEN 'SECONDARY' THEN 2
                         WHEN 'SUPPORTING' THEN 3
                         ELSE 4
                     END,
                     dt.type_code
            "#,
            attr_id
        )
        .fetch_all(pool)
        .await?;

        let sources: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                json!({
                    "document_type": r.type_code,
                    "display_name": r.display_name,
                    "category": r.category,
                    "extraction_method": r.extraction_method,
                    "is_authoritative": r.is_authoritative,
                    "proof_strength": r.proof_strength,
                    "confidence_default": r.extraction_confidence_default
                })
            })
            .collect();

        Ok(ExecutionResult::Record(json!({
            "attribute": attr_id,
            "source_count": sources.len(),
            "sources": sources
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(json!({"sources": []})))
    }
}

/// List all document types that require (SINK) a given attribute
///
/// Rationale: Requires join across document_attribute_links and document_types
/// with filtering by direction.
#[register_custom_op]
pub struct AttributeListSinksOp;

#[async_trait]
impl CustomOperation for AttributeListSinksOp {
    fn domain(&self) -> &'static str {
        "attribute"
    }
    fn verb(&self) -> &'static str {
        "list-sinks"
    }
    fn rationale(&self) -> &'static str {
        "Requires join across attribute registry and document links for sink relationships"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let attr_id = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "attribute")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing attribute argument"))?;

        let rows = sqlx::query!(
            r#"
            SELECT dt.type_code, dt.display_name, dt.category,
                   dal.proof_strength
            FROM "ob-poc".document_attribute_links dal
            JOIN "ob-poc".document_types dt ON dt.type_id = dal.document_type_id
            JOIN "ob-poc".attribute_registry ar ON ar.uuid = dal.attribute_id
            WHERE ar.id = $1 AND dal.direction IN ('SINK', 'BOTH')
            ORDER BY dt.type_code
            "#,
            attr_id
        )
        .fetch_all(pool)
        .await?;

        let sinks: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                json!({
                    "document_type": r.type_code,
                    "display_name": r.display_name,
                    "category": r.category,
                    "proof_strength": r.proof_strength
                })
            })
            .collect();

        Ok(ExecutionResult::Record(json!({
            "attribute": attr_id,
            "sink_count": sinks.len(),
            "sinks": sinks
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(json!({"sinks": []})))
    }
}

/// Trace complete lineage for an attribute - sources, sinks, and resources
///
/// Rationale: Requires multiple queries across attribute_registry,
/// document_attribute_links, document_types, and resource_attribute_requirements
/// to build a complete lineage view.
#[register_custom_op]
pub struct AttributeTraceLineageOp;

#[async_trait]
impl CustomOperation for AttributeTraceLineageOp {
    fn domain(&self) -> &'static str {
        "attribute"
    }
    fn verb(&self) -> &'static str {
        "trace-lineage"
    }
    fn rationale(&self) -> &'static str {
        "Requires multiple queries to build complete attribute lineage including sources, sinks, and resource requirements"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let attr_id = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "attribute")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing attribute argument"))?;

        // Get attribute details
        let attr = sqlx::query(
            r#"
            SELECT id, display_name, category, value_type, domain
            FROM "ob-poc".attribute_registry
            WHERE id = $1
            "#,
        )
        .bind(attr_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Attribute not found: {}", attr_id))?;

        // Get sources (documents that provide this attribute)
        let sources = sqlx::query!(
            r#"
            SELECT dt.type_code, dt.display_name as doc_name, dt.category,
                   dal.extraction_method, dal.is_authoritative, dal.proof_strength,
                   dal.extraction_confidence_default
            FROM "ob-poc".document_attribute_links dal
            JOIN "ob-poc".document_types dt ON dt.type_id = dal.document_type_id
            JOIN "ob-poc".attribute_registry ar ON ar.uuid = dal.attribute_id
            WHERE ar.id = $1 AND dal.direction IN ('SOURCE', 'BOTH')
            ORDER BY dal.is_authoritative DESC, dal.proof_strength
            "#,
            attr_id
        )
        .fetch_all(pool)
        .await?;

        // Get sinks (documents that require this attribute)
        let sinks = sqlx::query!(
            r#"
            SELECT dt.type_code, dt.display_name as doc_name, dt.category,
                   dal.proof_strength
            FROM "ob-poc".document_attribute_links dal
            JOIN "ob-poc".document_types dt ON dt.type_id = dal.document_type_id
            JOIN "ob-poc".attribute_registry ar ON ar.uuid = dal.attribute_id
            WHERE ar.id = $1 AND dal.direction IN ('SINK', 'BOTH')
            ORDER BY dt.type_code
            "#,
            attr_id
        )
        .fetch_all(pool)
        .await?;

        // Get resources that require this attribute
        let resources = sqlx::query!(
            r#"
            SELECT srt.resource_code, srt.name as resource_name, rar.is_mandatory
            FROM "ob-poc".resource_attribute_requirements rar
            JOIN "ob-poc".service_resource_types srt ON srt.resource_id = rar.resource_id
            JOIN "ob-poc".attribute_registry ar ON ar.uuid = rar.attribute_id
            WHERE ar.id = $1
            ORDER BY rar.is_mandatory DESC, srt.resource_code
            "#,
            attr_id
        )
        .fetch_all(pool)
        .await?;

        // Build lineage response
        let sources_json: Vec<serde_json::Value> = sources
            .iter()
            .map(|s| {
                json!({
                    "document_type": s.type_code,
                    "display_name": s.doc_name,
                    "category": s.category,
                    "extraction_method": s.extraction_method,
                    "is_authoritative": s.is_authoritative,
                    "proof_strength": s.proof_strength,
                    "confidence_default": s.extraction_confidence_default
                })
            })
            .collect();

        let sinks_json: Vec<serde_json::Value> = sinks
            .iter()
            .map(|s| {
                json!({
                    "document_type": s.type_code,
                    "display_name": s.doc_name,
                    "category": s.category,
                    "proof_strength": s.proof_strength
                })
            })
            .collect();

        let resources_json: Vec<serde_json::Value> = resources
            .iter()
            .map(|r| {
                json!({
                    "resource_code": r.resource_code,
                    "resource_name": r.resource_name,
                    "is_mandatory": r.is_mandatory
                })
            })
            .collect();

        // Calculate coverage metrics
        let has_authoritative_source = sources.iter().any(|s| s.is_authoritative.unwrap_or(false));
        let primary_sources = sources
            .iter()
            .filter(|s| s.proof_strength.as_deref() == Some("PRIMARY"))
            .count();

        Ok(ExecutionResult::Record(json!({
            "attribute": {
                "id": attr.get::<String, _>("id"),
                "display_name": attr.get::<String, _>("display_name"),
                "category": attr.get::<String, _>("category"),
                "value_type": attr.get::<String, _>("value_type"),
                "domain": attr.get::<Option<String>, _>("domain")
            },
            "sources": {
                "count": sources_json.len(),
                "has_authoritative": has_authoritative_source,
                "primary_count": primary_sources,
                "documents": sources_json
            },
            "sinks": {
                "count": sinks_json.len(),
                "documents": sinks_json
            },
            "required_by_resources": {
                "count": resources_json.len(),
                "resources": resources_json
            },
            "coverage_status": if has_authoritative_source || primary_sources > 0 {
                "GOOD"
            } else if !sources_json.is_empty() {
                "PARTIAL"
            } else {
                "NO_SOURCE"
            }
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(json!({
            "attribute": {},
            "sources": {"count": 0, "documents": []},
            "sinks": {"count": 0, "documents": []},
            "required_by_resources": {"count": 0, "resources": []}
        })))
    }
}

/// List all attributes linked to a document type
///
/// Rationale: Requires join across document_attribute_links and attribute_registry
/// with direction filtering.
#[register_custom_op]
pub struct AttributeListByDocumentOp;

#[async_trait]
impl CustomOperation for AttributeListByDocumentOp {
    fn domain(&self) -> &'static str {
        "attribute"
    }
    fn verb(&self) -> &'static str {
        "list-by-document"
    }
    fn rationale(&self) -> &'static str {
        "Requires join across document types, links, and attribute registry"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let doc_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "document-type")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing document-type argument"))?;

        let direction_filter = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "direction")
            .and_then(|a| a.value.as_string());

        // Use single query with optional direction filter
        let rows = sqlx::query!(
            r#"
            SELECT ar.id as attr_id, ar.display_name, ar.category, ar.value_type,
                   dal.direction, dal.extraction_method, dal.is_authoritative,
                   dal.proof_strength, dal.extraction_confidence_default
            FROM "ob-poc".document_attribute_links dal
            JOIN "ob-poc".document_types dt ON dt.type_id = dal.document_type_id
            JOIN "ob-poc".attribute_registry ar ON ar.uuid = dal.attribute_id
            WHERE dt.type_code = $1 AND ($2::text IS NULL OR dal.direction = $2)
            ORDER BY dal.direction, ar.category, ar.id
            "#,
            doc_type,
            direction_filter
        )
        .fetch_all(pool)
        .await?;

        let attributes: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                json!({
                    "attribute_id": r.attr_id,
                    "display_name": r.display_name,
                    "category": r.category,
                    "value_type": r.value_type,
                    "direction": r.direction,
                    "extraction_method": r.extraction_method,
                    "is_authoritative": r.is_authoritative,
                    "proof_strength": r.proof_strength,
                    "confidence_default": r.extraction_confidence_default
                })
            })
            .collect();

        // Group by direction for summary
        let source_count = rows
            .iter()
            .filter(|r| r.direction == "SOURCE" || r.direction == "BOTH")
            .count();
        let sink_count = rows
            .iter()
            .filter(|r| r.direction == "SINK" || r.direction == "BOTH")
            .count();

        Ok(ExecutionResult::Record(json!({
            "document_type": doc_type,
            "attribute_count": attributes.len(),
            "source_count": source_count,
            "sink_count": sink_count,
            "attributes": attributes
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(json!({"attributes": []})))
    }
}

/// Check attribute coverage for a document type
///
/// Rationale: Compares required_attributes JSONB against document_attribute_links
/// to identify gaps in coverage.
#[register_custom_op]
pub struct AttributeCheckCoverageOp;

#[async_trait]
impl CustomOperation for AttributeCheckCoverageOp {
    fn domain(&self) -> &'static str {
        "attribute"
    }
    fn verb(&self) -> &'static str {
        "check-coverage"
    }
    fn rationale(&self) -> &'static str {
        "Requires comparison of required_attributes JSONB against actual document_attribute_links mappings"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let doc_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "document-type")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing document-type argument"))?;

        // Get document type with required_attributes
        let doc = sqlx::query!(
            r#"
            SELECT type_id, type_code, display_name, required_attributes
            FROM "ob-poc".document_types
            WHERE type_code = $1
            "#,
            doc_type
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Document type not found: {}", doc_type))?;

        // Get all SOURCE attributes linked to this document type
        let linked_attrs = sqlx::query!(
            r#"
            SELECT ar.id as attr_id
            FROM "ob-poc".document_attribute_links dal
            JOIN "ob-poc".attribute_registry ar ON ar.uuid = dal.attribute_id
            WHERE dal.document_type_id = $1 AND dal.direction IN ('SOURCE', 'BOTH')
            "#,
            doc.type_id
        )
        .fetch_all(pool)
        .await?;

        let linked_set: std::collections::HashSet<String> =
            linked_attrs.iter().map(|r| r.attr_id.clone()).collect();

        // Parse required_attributes JSONB
        let required: serde_json::Value = doc
            .required_attributes
            .unwrap_or_else(|| serde_json::json!({}));

        let mut mandatory_missing = Vec::new();
        let mut mandatory_covered = Vec::new();

        if let Some(mandatory) = required.get("mandatory").and_then(|m| m.as_array()) {
            for attr in mandatory {
                if let Some(attr_id) = attr.as_str() {
                    if linked_set.contains(attr_id) {
                        mandatory_covered.push(attr_id.to_string());
                    } else {
                        mandatory_missing.push(attr_id.to_string());
                    }
                }
            }
        }

        let total_mandatory = mandatory_covered.len() + mandatory_missing.len();
        let coverage_pct = if total_mandatory > 0 {
            (mandatory_covered.len() as f64 / total_mandatory as f64) * 100.0
        } else {
            100.0
        };

        Ok(ExecutionResult::Record(json!({
            "document_type": doc_type,
            "display_name": doc.display_name,
            "linked_attributes": linked_set.len(),
            "mandatory_coverage": {
                "total": total_mandatory,
                "covered": mandatory_covered.len(),
                "missing": mandatory_missing.len(),
                "coverage_percentage": format!("{:.1}%", coverage_pct),
                "covered_list": mandatory_covered,
                "missing_list": mandatory_missing
            },
            "status": if mandatory_missing.is_empty() {
                "COMPLETE"
            } else {
                "INCOMPLETE"
            }
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(json!({"status": "UNKNOWN"})))
    }
}

/// List all attributes for a document type (document.list-attributes handler)
///
/// This is the handler for document.list-attributes verb.
#[register_custom_op]
pub struct DocumentListAttributesOp;

#[async_trait]
impl CustomOperation for DocumentListAttributesOp {
    fn domain(&self) -> &'static str {
        "document"
    }
    fn verb(&self) -> &'static str {
        "list-attributes"
    }
    fn rationale(&self) -> &'static str {
        "Requires join across document types, links, and attribute registry"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Delegate to AttributeListByDocumentOp
        AttributeListByDocumentOp
            .execute(verb_call, ctx, pool)
            .await
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(json!({"attributes": []})))
    }
}

/// Check document extraction coverage for an entity
///
/// Rationale: Analyzes what attributes are required for an entity vs what
/// documents are available and what can be extracted.
#[register_custom_op]
pub struct DocumentCheckExtractionCoverageOp;

#[async_trait]
impl CustomOperation for DocumentCheckExtractionCoverageOp {
    fn domain(&self) -> &'static str {
        "document"
    }
    fn verb(&self) -> &'static str {
        "check-extraction-coverage"
    }
    fn rationale(&self) -> &'static str {
        "Requires complex analysis of entity requirements vs available documents and their extraction capabilities"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        let entity_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing entity-id argument"))?;

        let cbu_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "cbu-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });

        // Get documents cataloged for this entity
        let documents = sqlx::query!(
            r#"
            SELECT dc.doc_id, dt.type_code, dt.display_name
            FROM "ob-poc".document_catalog dc
            JOIN "ob-poc".document_types dt ON dt.type_id = dc.document_type_id
            WHERE dc.entity_id = $1
            AND ($2::uuid IS NULL OR dc.cbu_id = $2)
            "#,
            entity_id,
            cbu_id
        )
        .fetch_all(pool)
        .await?;

        // Get all attributes that can be sourced from these documents
        let doc_type_codes: Vec<String> = documents.iter().map(|d| d.type_code.clone()).collect();

        let sourceable_attrs = if !doc_type_codes.is_empty() {
            sqlx::query!(
                r#"
                SELECT DISTINCT ar.id as attr_id, ar.display_name, ar.category,
                       dal.is_authoritative, dal.proof_strength
                FROM "ob-poc".document_attribute_links dal
                JOIN "ob-poc".document_types dt ON dt.type_id = dal.document_type_id
                JOIN "ob-poc".attribute_registry ar ON ar.uuid = dal.attribute_id
                WHERE dt.type_code = ANY($1) AND dal.direction IN ('SOURCE', 'BOTH')
                ORDER BY ar.category, ar.id
                "#,
                &doc_type_codes
            )
            .fetch_all(pool)
            .await?
        } else {
            vec![]
        };

        // Get entity type to determine required attributes
        let entity = sqlx::query!(
            r#"
            SELECT e.entity_id, et.type_code as entity_type
            FROM "ob-poc".entities e
            JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
            WHERE e.entity_id = $1
              AND e.deleted_at IS NULL
            "#,
            entity_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Entity not found: {}", entity_id))?;

        // Build response
        let available_docs: Vec<serde_json::Value> = documents
            .iter()
            .map(|d| {
                json!({
                    "doc_id": d.doc_id,
                    "type_code": d.type_code,
                    "display_name": d.display_name
                })
            })
            .collect();

        let sourceable: Vec<serde_json::Value> = sourceable_attrs
            .iter()
            .map(|a| {
                json!({
                    "attribute_id": a.attr_id,
                    "display_name": a.display_name,
                    "category": a.category,
                    "is_authoritative": a.is_authoritative,
                    "proof_strength": a.proof_strength
                })
            })
            .collect();

        let has_authoritative = sourceable_attrs
            .iter()
            .any(|a| a.is_authoritative.unwrap_or(false));

        Ok(ExecutionResult::Record(json!({
            "entity_id": entity_id,
            "entity_type": entity.entity_type,
            "available_documents": {
                "count": available_docs.len(),
                "documents": available_docs
            },
            "sourceable_attributes": {
                "count": sourceable.len(),
                "has_authoritative": has_authoritative,
                "attributes": sourceable
            },
            "coverage_summary": {
                "document_count": documents.len(),
                "attribute_count": sourceable_attrs.len(),
                "has_identity_docs": doc_type_codes.iter().any(|c| c.contains("PASSPORT") || c.contains("NATIONAL_ID")),
                "has_address_proof": doc_type_codes.iter().any(|c| c.contains("UTILITY") || c.contains("BANK_STATEMENT"))
            }
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(json!({
            "available_documents": {"count": 0},
            "sourceable_attributes": {"count": 0}
        })))
    }
}

#[cfg(feature = "database")]
#[derive(Debug, Clone)]
struct RegistryAttributeRow {
    uuid: Uuid,
}

#[cfg(feature = "database")]
#[derive(Debug, Clone)]
struct AttributeSnapshotContext {
    registry_uuid: Uuid,
    registry_id: String,
    fqn: String,
    active_snapshot: Option<SnapshotRow>,
}

#[cfg(feature = "database")]
fn string_arg(verb_call: &VerbCall, name: &str) -> Result<String> {
    verb_call
        .arguments
        .iter()
        .find(|arg| arg.key == name)
        .and_then(|arg| arg.value.as_string())
        .map(str::to_owned)
        .ok_or_else(|| anyhow!("Missing {} argument", name))
}

#[cfg(feature = "database")]
fn optional_string_arg(verb_call: &VerbCall, name: &str) -> Option<String> {
    verb_call
        .arguments
        .iter()
        .find(|arg| arg.key == name)
        .and_then(|arg| arg.value.as_string())
        .map(str::to_owned)
}

#[cfg(feature = "database")]
fn optional_int_arg(verb_call: &VerbCall, name: &str) -> Option<i64> {
    verb_call
        .arguments
        .iter()
        .find(|arg| arg.key == name)
        .and_then(|arg| arg.value.as_integer())
}

#[cfg(feature = "database")]
fn parse_json_arg(verb_call: &VerbCall, name: &str) -> Result<Option<serde_json::Value>> {
    let Some(raw) = optional_string_arg(verb_call, name) else {
        return Ok(None);
    };
    let value = serde_json::from_str(&raw)
        .with_context(|| format!("Failed to parse JSON argument {}", name))?;
    Ok(Some(value))
}

#[cfg(feature = "database")]
fn normalize_attribute_id(raw_id: &str, domain: Option<&str>) -> String {
    if raw_id.contains('.') || domain.is_none() {
        raw_id.to_string()
    } else {
        format!("{}.{}", domain.unwrap_or_default(), raw_id)
    }
}

#[cfg(feature = "database")]
fn parse_evidence_grade(raw: Option<String>, default: EvidenceGrade) -> Result<EvidenceGrade> {
    match raw {
        None => Ok(default),
        Some(value) => value
            .parse::<EvidenceGrade>()
            .map_err(|_| anyhow!("Invalid evidence-grade '{}'", value)),
    }
}

#[cfg(feature = "database")]
fn parse_data_type(value_type: &str) -> Result<AttributeDataType> {
    AttributeDataType::from_pg_check_value(value_type)
        .ok_or_else(|| anyhow!("Unsupported attribute value-type '{}'", value_type))
}

#[cfg(feature = "database")]
fn parse_null_semantics(raw: Option<String>) -> Result<NullSemantics> {
    match raw.as_deref().unwrap_or("propagate") {
        "propagate" => Ok(NullSemantics::Propagate),
        "skip" => Ok(NullSemantics::Skip),
        "error" => Ok(NullSemantics::Error),
        other => Err(anyhow!("Unsupported null-semantics '{}'", other)),
    }
}

#[cfg(feature = "database")]
fn effective_description(display_name: &str, semos_description: Option<String>) -> String {
    semos_description.unwrap_or_else(|| display_name.to_string())
}

#[cfg(feature = "database")]
#[cfg(feature = "database")]
fn build_attribute_def_body(
    semantic_id: &str,
    display_name: &str,
    description: String,
    domain: String,
    value_type: &str,
    evidence_grade: EvidenceGrade,
    derived: bool,
) -> Result<AttributeDefBody> {
    Ok(AttributeDefBody {
        fqn: semantic_id.to_string(),
        name: display_name.to_string(),
        description,
        domain,
        data_type: parse_data_type(value_type)?,
        evidence_grade,
        source: Some(AttributeSource {
            producing_verb: None,
            schema: Some("ob-poc".to_string()),
            table: Some("attribute_registry".to_string()),
            column: Some(semantic_id.to_string()),
            derived,
        }),
        constraints: None,
        sinks: Vec::new(),
    })
}

#[cfg(feature = "database")]
#[allow(clippy::too_many_arguments)]
fn build_derivation_spec_body(
    semantic_id: &str,
    display_name: &str,
    description: String,
    evidence_grade: EvidenceGrade,
    function_name: &str,
    inputs_json: serde_json::Value,
    null_semantics: NullSemantics,
    freshness_seconds: Option<i64>,
) -> Result<DerivationSpecBody> {
    let inputs: Vec<DerivationInput> = serde_json::from_value(inputs_json)
        .context("derivation-inputs must be a JSON array of DerivationInput objects")?;
    let output_attribute_fqn = if semantic_id.ends_with("_value") {
        semantic_id.to_string()
    } else {
        format!("{semantic_id}_value")
    };
    Ok(DerivationSpecBody {
        fqn: semantic_id.to_string(),
        name: display_name.to_string(),
        description,
        output_attribute_fqn,
        inputs,
        expression: DerivationExpression::FunctionRef {
            ref_name: function_name.to_string(),
        },
        null_semantics,
        freshness_rule: freshness_seconds.map(|seconds| FreshnessRule {
            max_age_seconds: seconds.max(0) as u64,
        }),
        security_inheritance: Default::default(),
        evidence_grade,
        tests: Vec::new(),
    })
}

#[cfg(feature = "database")]
#[allow(clippy::too_many_arguments)]
async fn upsert_attribute_registry(
    tx: &mut Transaction<'_, Postgres>,
    semantic_id: &str,
    display_name: &str,
    category: &str,
    value_type: &str,
    domain: Option<&str>,
    validation_rules: Option<serde_json::Value>,
    applicability: Option<serde_json::Value>,
    evidence_grade: EvidenceGrade,
    is_derived: bool,
    derivation_spec_fqn: Option<&str>,
    metadata_patch: Option<serde_json::Value>,
) -> Result<RegistryAttributeRow> {
    let uuid = Uuid::new_v4();
    let metadata_patch = metadata_patch.unwrap_or_else(|| json!({}));
    let row = sqlx::query(
        r#"
        INSERT INTO "ob-poc".attribute_registry (
            id, uuid, display_name, category, value_type, domain,
            validation_rules, applicability, evidence_grade, is_derived, derivation_spec_fqn, metadata
        )
        VALUES (
            $1, $2, $3, $4, $5, $6,
            COALESCE($7, '{}'::jsonb),
            COALESCE($8, '{}'::jsonb),
            $9,
            $10,
            $11,
            CASE
                WHEN $12::jsonb = '{}'::jsonb THEN '{}'::jsonb
                ELSE jsonb_build_object('sem_os', $12::jsonb)
            END
        )
        ON CONFLICT (id) DO UPDATE SET
            display_name = EXCLUDED.display_name,
            category = EXCLUDED.category,
            value_type = EXCLUDED.value_type,
            domain = EXCLUDED.domain,
            validation_rules = COALESCE(EXCLUDED.validation_rules, "ob-poc".attribute_registry.validation_rules),
            applicability = COALESCE(EXCLUDED.applicability, "ob-poc".attribute_registry.applicability),
            evidence_grade = EXCLUDED.evidence_grade,
            is_derived = EXCLUDED.is_derived,
            derivation_spec_fqn = EXCLUDED.derivation_spec_fqn,
            metadata = CASE
                WHEN $12::jsonb = '{}'::jsonb THEN COALESCE("ob-poc".attribute_registry.metadata, '{}'::jsonb)
                ELSE jsonb_set(
                    COALESCE("ob-poc".attribute_registry.metadata, '{}'::jsonb),
                    '{sem_os}',
                    COALESCE("ob-poc".attribute_registry.metadata->'sem_os', '{}'::jsonb) || $12::jsonb,
                    true
                )
            END,
            updated_at = NOW()
        RETURNING id, uuid, display_name, category, value_type, domain, metadata
        "#,
    )
    .bind(semantic_id)
    .bind(uuid)
    .bind(display_name)
    .bind(category)
    .bind(value_type)
    .bind(domain)
    .bind(validation_rules)
    .bind(applicability)
    .bind(evidence_grade.to_string())
    .bind(is_derived)
    .bind(derivation_spec_fqn)
    .bind(metadata_patch)
    .fetch_one(&mut **tx)
    .await?;

    Ok(RegistryAttributeRow {
        uuid: row.get("uuid"),
    })
}

#[cfg(feature = "database")]
async fn patch_attribute_semos_metadata(
    tx: &mut Transaction<'_, Postgres>,
    semantic_id: &str,
    semos_patch: serde_json::Value,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE "ob-poc".attribute_registry
        SET metadata = jsonb_set(
                COALESCE(metadata, '{}'::jsonb),
                '{sem_os}',
                COALESCE(metadata->'sem_os', '{}'::jsonb) || $2::jsonb,
                true
            ),
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(semantic_id)
    .bind(semos_patch)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(feature = "database")]
async fn sync_attribute_registry_governance(
    tx: &mut Transaction<'_, Postgres>,
    semantic_id: &str,
    sem_reg_snapshot_id: Option<Uuid>,
    is_derived: bool,
    derivation_spec_fqn: Option<&str>,
    evidence_grade: EvidenceGrade,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE "ob-poc".attribute_registry
        SET sem_reg_snapshot_id = COALESCE($2, sem_reg_snapshot_id),
            is_derived = $3,
            derivation_spec_fqn = $4,
            evidence_grade = $5,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(semantic_id)
    .bind(sem_reg_snapshot_id)
    .bind(is_derived)
    .bind(derivation_spec_fqn)
    .bind(evidence_grade.to_string())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(feature = "database")]
async fn load_active_attribute_context(
    pool: &PgPool,
    reference: &str,
) -> Result<AttributeSnapshotContext> {
    let identity_service = AttributeIdentityService::new(pool.clone());
    let identity = identity_service
        .resolve_reference(reference)
        .await?
        .ok_or_else(|| anyhow!("Attribute '{}' not found", reference))?;
    let registry_uuid = identity
        .runtime_uuid()
        .ok_or_else(|| anyhow!("Attribute '{}' has no operational registry UUID", reference))?;
    let registry_id = identity
        .registry_id
        .clone()
        .or_else(|| identity.semos_attribute_fqn.clone())
        .or_else(|| identity.attribute_fqn.clone())
        .unwrap_or_else(|| reference.to_string());
    let fqn = identity
        .semos_attribute_fqn
        .clone()
        .or_else(|| identity.attribute_fqn.clone())
        .unwrap_or_else(|| registry_id.clone());
    let active_snapshot =
        SnapshotStore::resolve_active(pool, ObjectType::AttributeDef, registry_uuid)
            .await?
            .or(SnapshotStore::find_active_by_definition_field(
                pool,
                ObjectType::AttributeDef,
                "fqn",
                &fqn,
            )
            .await?);

    Ok(AttributeSnapshotContext {
        registry_uuid,
        registry_id,
        fqn,
        active_snapshot,
    })
}

#[cfg(feature = "database")]
async fn load_active_derivation_snapshot(pool: &PgPool, fqn: &str) -> Result<Option<SnapshotRow>> {
    SnapshotStore::find_active_by_definition_field(pool, ObjectType::DerivationSpec, "fqn", fqn)
        .await
}

#[cfg(feature = "database")]
fn next_meta_from_predecessor(
    predecessor: Option<&SnapshotRow>,
    object_type: ObjectType,
    object_id: Uuid,
    created_by: &str,
    change_type: ChangeType,
    change_rationale: Option<String>,
    status: SnapshotStatus,
) -> SnapshotMeta {
    let mut meta = SnapshotMeta::new_operational(object_type, object_id, created_by.to_string());
    meta.change_type = change_type;
    meta.change_rationale = change_rationale;
    meta.status = status;
    if let Some(pred) = predecessor {
        meta.version_major = pred.version_major;
        meta.version_minor = pred.version_minor + 1;
        meta.predecessor_id = Some(pred.snapshot_id);
    }
    meta
}

#[cfg(feature = "database")]
async fn publish_snapshot_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    meta: &SnapshotMeta,
    definition: &serde_json::Value,
) -> Result<Uuid> {
    if let Some(predecessor_id) = meta.predecessor_id {
        let affected = sqlx::query(
            r#"
            UPDATE sem_reg.snapshots
            SET effective_until = NOW()
            WHERE snapshot_id = $1 AND effective_until IS NULL
            "#,
        )
        .bind(predecessor_id)
        .execute(&mut **tx)
        .await?
        .rows_affected();
        if affected == 0 {
            return Err(anyhow!(
                "Predecessor snapshot {} not found or already superseded",
                predecessor_id
            ));
        }
    }

    let security_label = serde_json::to_value(&meta.security_label)?;
    let snapshot_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO sem_reg.snapshots (
            snapshot_set_id, object_type, object_id,
            version_major, version_minor, status,
            governance_tier, trust_class, security_label,
            predecessor_id, change_type, change_rationale,
            created_by, approved_by, definition
        ) VALUES (
            NULL, $1::sem_reg.object_type, $2,
            $3, $4, $5::sem_reg.snapshot_status,
            $6::sem_reg.governance_tier, $7::sem_reg.trust_class, $8,
            $9, $10::sem_reg.change_type, $11,
            $12, $13, $14
        )
        RETURNING snapshot_id
        "#,
    )
    .bind(meta.object_type.as_ref())
    .bind(meta.object_id)
    .bind(meta.version_major)
    .bind(meta.version_minor)
    .bind(meta.status.as_ref())
    .bind(meta.governance_tier.as_ref())
    .bind(meta.trust_class.as_ref())
    .bind(security_label)
    .bind(meta.predecessor_id)
    .bind(meta.change_type.as_ref())
    .bind(&meta.change_rationale)
    .bind(&meta.created_by)
    .bind(&meta.approved_by)
    .bind(definition)
    .fetch_one(&mut **tx)
    .await?;

    Ok(snapshot_id)
}

#[cfg(feature = "database")]
fn hashes_match(snapshot: Option<&SnapshotRow>, definition: &serde_json::Value) -> bool {
    snapshot
        .map(|existing| {
            crate::sem_reg::ids::definition_hash(&existing.definition)
                == crate::sem_reg::ids::definition_hash(definition)
        })
        .unwrap_or(false)
}

/// Define or update a governed attribute with dual-write into SemOS snapshots.
#[register_custom_op]
pub struct AttributeDefineGovernedOp;

#[async_trait]
impl CustomOperation for AttributeDefineGovernedOp {
    fn domain(&self) -> &'static str {
        "attribute"
    }

    fn verb(&self) -> &'static str {
        "define"
    }

    fn rationale(&self) -> &'static str {
        "Dual-writes operational attribute_registry and governed AttributeDef snapshots"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let raw_id = string_arg(verb_call, "id")?;
        let display_name = string_arg(verb_call, "display-name")?;
        let category = string_arg(verb_call, "category")?;
        let value_type = string_arg(verb_call, "value-type")?;
        let domain = optional_string_arg(verb_call, "domain");
        let semantic_id = normalize_attribute_id(&raw_id, domain.as_deref());
        let description = effective_description(
            &display_name,
            optional_string_arg(verb_call, "semos-description"),
        );
        let evidence_grade = parse_evidence_grade(
            optional_string_arg(verb_call, "evidence-grade"),
            EvidenceGrade::None,
        )?;
        let validation_rules = parse_json_arg(verb_call, "validation-rules")?;
        let applicability = parse_json_arg(verb_call, "applicability")?;

        let mut tx = pool.begin().await?;
        let registry = upsert_attribute_registry(
            &mut tx,
            &semantic_id,
            &display_name,
            &category,
            &value_type,
            domain.as_deref(),
            validation_rules,
            applicability,
            evidence_grade,
            false,
            None,
            None,
        )
        .await?;

        let body = build_attribute_def_body(
            &semantic_id,
            &display_name,
            description,
            domain.clone().unwrap_or_else(|| {
                semantic_id
                    .split('.')
                    .next()
                    .unwrap_or("attribute")
                    .to_string()
            }),
            &value_type,
            evidence_grade,
            false,
        )?;
        let definition = serde_json::to_value(&body)?;
        let predecessor =
            SnapshotStore::resolve_active(pool, ObjectType::AttributeDef, registry.uuid).await?;
        let snapshot_id = if hashes_match(predecessor.as_ref(), &definition) {
            predecessor
                .as_ref()
                .map(|row| row.snapshot_id)
                .ok_or_else(|| {
                    anyhow!(
                        "Failed to resolve existing AttributeDef snapshot for {}",
                        semantic_id
                    )
                })?
        } else {
            let meta = next_meta_from_predecessor(
                predecessor.as_ref(),
                ObjectType::AttributeDef,
                registry.uuid,
                ctx.audit_user.as_deref().unwrap_or("attribute.define"),
                if predecessor.is_some() {
                    ChangeType::NonBreaking
                } else {
                    ChangeType::Created
                },
                None,
                SnapshotStatus::Active,
            );
            publish_snapshot_in_tx(&mut tx, &meta, &definition).await?
        };

        patch_attribute_semos_metadata(
            &mut tx,
            &semantic_id,
            json!({
                "snapshot_id": snapshot_id,
                "object_id": registry.uuid,
                "attribute_fqn": semantic_id,
            }),
        )
        .await?;
        sync_attribute_registry_governance(
            &mut tx,
            &semantic_id,
            Some(snapshot_id),
            false,
            None,
            evidence_grade,
        )
        .await?;

        tx.commit().await?;
        ctx.bind("attribute", registry.uuid);
        Ok(ExecutionResult::Uuid(registry.uuid))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("attribute.define requires database"))
    }
}

/// Define or update a derived attribute together with its derivation spec.
#[register_custom_op]
pub struct AttributeDefineDerivedOp;

#[async_trait]
impl CustomOperation for AttributeDefineDerivedOp {
    fn domain(&self) -> &'static str {
        "attribute"
    }

    fn verb(&self) -> &'static str {
        "define-derived"
    }

    fn rationale(&self) -> &'static str {
        "Atomically publishes coupled AttributeDef and DerivationSpec snapshots"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let raw_id = string_arg(verb_call, "id")?;
        let domain = Some(string_arg(verb_call, "domain")?);
        let semantic_id = normalize_attribute_id(&raw_id, domain.as_deref());
        let display_name = string_arg(verb_call, "display-name")?;
        let category = string_arg(verb_call, "category")?;
        let value_type = string_arg(verb_call, "value-type")?;
        let description = effective_description(
            &display_name,
            optional_string_arg(verb_call, "semos-description"),
        );
        let evidence_grade = parse_evidence_grade(
            optional_string_arg(verb_call, "evidence-grade"),
            EvidenceGrade::Prohibited,
        )?;
        let derivation_function = string_arg(verb_call, "derivation-function")?;
        let derivation_inputs = parse_json_arg(verb_call, "derivation-inputs")?
            .ok_or_else(|| anyhow!("Missing derivation-inputs argument"))?;
        let null_semantics =
            parse_null_semantics(optional_string_arg(verb_call, "null-semantics"))?;
        let freshness_seconds = optional_int_arg(verb_call, "freshness-seconds");

        let mut tx = pool.begin().await?;
        let registry = upsert_attribute_registry(
            &mut tx,
            &semantic_id,
            &display_name,
            &category,
            &value_type,
            domain.as_deref(),
            None,
            None,
            evidence_grade,
            true,
            Some(&semantic_id),
            Some(json!({"lineage_plane": "below_line"})),
        )
        .await?;

        let attr_body = build_attribute_def_body(
            &semantic_id,
            &display_name,
            description.clone(),
            domain.clone().unwrap_or_else(|| "attribute".to_string()),
            &value_type,
            evidence_grade,
            true,
        )?;
        let derivation_body = build_derivation_spec_body(
            &semantic_id,
            &display_name,
            description,
            evidence_grade,
            &derivation_function,
            derivation_inputs,
            null_semantics,
            freshness_seconds,
        )?;
        let attr_definition = serde_json::to_value(&attr_body)?;
        let derivation_definition = serde_json::to_value(&derivation_body)?;

        let attr_predecessor =
            SnapshotStore::resolve_active(pool, ObjectType::AttributeDef, registry.uuid).await?;
        let derivation_object_id =
            crate::sem_reg::ids::object_id_for(ObjectType::DerivationSpec, &semantic_id);
        let derivation_predecessor =
            SnapshotStore::resolve_active(pool, ObjectType::DerivationSpec, derivation_object_id)
                .await?;

        let attr_snapshot_id = if hashes_match(attr_predecessor.as_ref(), &attr_definition)
            && hashes_match(derivation_predecessor.as_ref(), &derivation_definition)
        {
            attr_predecessor
                .as_ref()
                .map(|row| row.snapshot_id)
                .ok_or_else(|| {
                    anyhow!("Missing existing AttributeDef snapshot for {}", semantic_id)
                })?
        } else {
            let attr_meta = next_meta_from_predecessor(
                attr_predecessor.as_ref(),
                ObjectType::AttributeDef,
                registry.uuid,
                ctx.audit_user
                    .as_deref()
                    .unwrap_or("attribute.define-derived"),
                if attr_predecessor.is_some() {
                    ChangeType::NonBreaking
                } else {
                    ChangeType::Created
                },
                None,
                SnapshotStatus::Active,
            );
            publish_snapshot_in_tx(&mut tx, &attr_meta, &attr_definition).await?
        };

        let derivation_snapshot_id = if hashes_match(attr_predecessor.as_ref(), &attr_definition)
            && hashes_match(derivation_predecessor.as_ref(), &derivation_definition)
        {
            derivation_predecessor
                .as_ref()
                .map(|row| row.snapshot_id)
                .ok_or_else(|| {
                    anyhow!(
                        "Missing existing DerivationSpec snapshot for {}",
                        semantic_id
                    )
                })?
        } else {
            let derivation_meta = next_meta_from_predecessor(
                derivation_predecessor.as_ref(),
                ObjectType::DerivationSpec,
                derivation_object_id,
                ctx.audit_user
                    .as_deref()
                    .unwrap_or("attribute.define-derived"),
                if derivation_predecessor.is_some() {
                    ChangeType::NonBreaking
                } else {
                    ChangeType::Created
                },
                None,
                SnapshotStatus::Active,
            );
            publish_snapshot_in_tx(&mut tx, &derivation_meta, &derivation_definition).await?
        };

        patch_attribute_semos_metadata(
            &mut tx,
            &semantic_id,
            json!({
                "snapshot_id": attr_snapshot_id,
                "object_id": registry.uuid,
                "attribute_fqn": semantic_id,
                "derivation_snapshot_id": derivation_snapshot_id,
                "derivation_object_id": derivation_object_id,
                "lineage_plane": "below_line",
                "derived": true,
            }),
        )
        .await?;
        sync_attribute_registry_governance(
            &mut tx,
            &semantic_id,
            Some(attr_snapshot_id),
            true,
            Some(&semantic_id),
            evidence_grade,
        )
        .await?;

        tx.commit().await?;
        ctx.bind("attribute", registry.uuid);
        Ok(ExecutionResult::Uuid(registry.uuid))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("attribute.define-derived requires database"))
    }
}

/// Update the evidence grade on a governed attribute definition and linked derivation.
#[register_custom_op]
pub struct AttributeSetEvidenceGradeOp;

#[async_trait]
impl CustomOperation for AttributeSetEvidenceGradeOp {
    fn domain(&self) -> &'static str {
        "attribute"
    }

    fn verb(&self) -> &'static str {
        "set-evidence-grade"
    }

    fn rationale(&self) -> &'static str {
        "Publishes a new governed AttributeDef version and keeps a linked DerivationSpec in sync"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let reference = string_arg(verb_call, "id")?;
        let new_grade = parse_evidence_grade(
            Some(string_arg(verb_call, "evidence-grade")?),
            EvidenceGrade::None,
        )?;
        let context = load_active_attribute_context(pool, &reference).await?;
        let active = context
            .active_snapshot
            .clone()
            .ok_or_else(|| anyhow!("No active AttributeDef snapshot found for {}", context.fqn))?;
        let mut body: AttributeDefBody = active.parse_definition()?;
        if body.evidence_grade == new_grade {
            return Ok(ExecutionResult::Record(json!({
                "attribute": context.registry_id,
                "snapshot_id": active.snapshot_id,
                "evidence_grade": new_grade.to_string(),
                "updated": false,
            })));
        }

        let mut tx = pool.begin().await?;
        body.evidence_grade = new_grade;
        let definition = serde_json::to_value(&body)?;
        let meta = next_meta_from_predecessor(
            Some(&active),
            ObjectType::AttributeDef,
            context.registry_uuid,
            "attribute.set-evidence-grade",
            ChangeType::NonBreaking,
            Some(format!("evidence_grade -> {}", new_grade)),
            SnapshotStatus::Active,
        );
        let attr_snapshot_id = publish_snapshot_in_tx(&mut tx, &meta, &definition).await?;

        if body.source.as_ref().is_some_and(|source| source.derived) {
            if let Some(derivation_snapshot) =
                load_active_derivation_snapshot(pool, &context.fqn).await?
            {
                let mut derivation_body: DerivationSpecBody =
                    derivation_snapshot.parse_definition()?;
                derivation_body.evidence_grade = new_grade;
                let derivation_definition = serde_json::to_value(&derivation_body)?;
                let derivation_meta = next_meta_from_predecessor(
                    Some(&derivation_snapshot),
                    ObjectType::DerivationSpec,
                    derivation_snapshot.object_id,
                    "attribute.set-evidence-grade",
                    ChangeType::NonBreaking,
                    Some(format!("evidence_grade -> {}", new_grade)),
                    SnapshotStatus::Active,
                );
                let derivation_snapshot_id =
                    publish_snapshot_in_tx(&mut tx, &derivation_meta, &derivation_definition)
                        .await?;
                patch_attribute_semos_metadata(
                    &mut tx,
                    &context.registry_id,
                    json!({
                        "derivation_snapshot_id": derivation_snapshot_id,
                    }),
                )
                .await?;
            }
        }

        patch_attribute_semos_metadata(
            &mut tx,
            &context.registry_id,
            json!({
                "snapshot_id": attr_snapshot_id,
                "evidence_grade": new_grade.to_string(),
            }),
        )
        .await?;
        sync_attribute_registry_governance(
            &mut tx,
            &context.registry_id,
            Some(attr_snapshot_id),
            body.source.as_ref().is_some_and(|source| source.derived),
            if body.source.as_ref().is_some_and(|source| source.derived) {
                Some(context.fqn.as_str())
            } else {
                None
            },
            new_grade,
        )
        .await?;

        tx.commit().await?;

        Ok(ExecutionResult::Record(json!({
            "attribute": context.registry_id,
            "snapshot_id": attr_snapshot_id,
            "evidence_grade": new_grade.to_string(),
            "updated": true,
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("attribute.set-evidence-grade requires database"))
    }
}

/// Deprecate a governed attribute while leaving operational references intact.
#[register_custom_op]
pub struct AttributeDeprecateOp;

#[async_trait]
impl CustomOperation for AttributeDeprecateOp {
    fn domain(&self) -> &'static str {
        "attribute"
    }

    fn verb(&self) -> &'static str {
        "deprecate"
    }

    fn rationale(&self) -> &'static str {
        "Soft-deprecates governed snapshots without deleting operational attribute rows"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let reference = string_arg(verb_call, "id")?;
        let reason = string_arg(verb_call, "reason")?;
        let replacement = optional_string_arg(verb_call, "replacement");
        let context = load_active_attribute_context(pool, &reference).await?;
        let active = context
            .active_snapshot
            .clone()
            .ok_or_else(|| anyhow!("No active AttributeDef snapshot found for {}", context.fqn))?;
        let body: AttributeDefBody = active.parse_definition()?;

        let mut tx = pool.begin().await?;
        let meta = next_meta_from_predecessor(
            Some(&active),
            ObjectType::AttributeDef,
            context.registry_uuid,
            "attribute.deprecate",
            ChangeType::Deprecation,
            Some(reason.clone()),
            SnapshotStatus::Deprecated,
        );
        let attr_snapshot_id =
            publish_snapshot_in_tx(&mut tx, &meta, &serde_json::to_value(&body)?).await?;

        if body.source.as_ref().is_some_and(|source| source.derived) {
            if let Some(derivation_snapshot) =
                load_active_derivation_snapshot(pool, &context.fqn).await?
            {
                let derivation_body: DerivationSpecBody = derivation_snapshot.parse_definition()?;
                let derivation_meta = next_meta_from_predecessor(
                    Some(&derivation_snapshot),
                    ObjectType::DerivationSpec,
                    derivation_snapshot.object_id,
                    "attribute.deprecate",
                    ChangeType::Deprecation,
                    Some(reason.clone()),
                    SnapshotStatus::Deprecated,
                );
                let derivation_snapshot_id = publish_snapshot_in_tx(
                    &mut tx,
                    &derivation_meta,
                    &serde_json::to_value(&derivation_body)?,
                )
                .await?;
                patch_attribute_semos_metadata(
                    &mut tx,
                    &context.registry_id,
                    json!({
                        "derivation_snapshot_id": derivation_snapshot_id,
                    }),
                )
                .await?;
            }
        }

        patch_attribute_semos_metadata(
            &mut tx,
            &context.registry_id,
            json!({
                "snapshot_id": attr_snapshot_id,
                "deprecated": true,
                "deprecation_reason": reason,
                "replacement": replacement,
            }),
        )
        .await?;
        sync_attribute_registry_governance(
            &mut tx,
            &context.registry_id,
            Some(attr_snapshot_id),
            body.source.as_ref().is_some_and(|source| source.derived),
            if body.source.as_ref().is_some_and(|source| source.derived) {
                Some(context.fqn.as_str())
            } else {
                None
            },
            body.evidence_grade,
        )
        .await?;

        tx.commit().await?;

        Ok(ExecutionResult::Record(json!({
            "attribute": context.registry_id,
            "snapshot_id": attr_snapshot_id,
            "deprecated": true,
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("attribute.deprecate requires database"))
    }
}

/// Inspect a governed attribute across operational and SemOS sources.
#[register_custom_op]
pub struct AttributeInspectOp;

#[async_trait]
impl CustomOperation for AttributeInspectOp {
    fn domain(&self) -> &'static str {
        "attribute"
    }

    fn verb(&self) -> &'static str {
        "inspect"
    }

    fn rationale(&self) -> &'static str {
        "Aggregates operational registry state, governed snapshots, derivation metadata, and usage counts"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let reference = string_arg(verb_call, "id")?;
        let context = load_active_attribute_context(pool, &reference).await?;
        let row = sqlx::query(
            r#"
            SELECT *
            FROM "ob-poc".v_attribute_registry_reconciled
            WHERE uuid = $1
            "#,
        )
        .bind(context.registry_uuid)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| {
            anyhow!(
                "Attribute '{}' is missing from v_attribute_registry_reconciled",
                context.registry_id
            )
        })?;

        let derivation_definition: Option<serde_json::Value> = row.get("derivation_definition");
        let derivation = derivation_definition
            .map(|definition| {
                let function = definition
                    .get("expression")
                    .and_then(|expr| expr.get("ref_name"))
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string);
                json!({
                    "function": function,
                    "inputs": definition.get("inputs").cloned().unwrap_or(json!([])),
                    "null_semantics": definition.get("null_semantics").cloned().unwrap_or(serde_json::Value::Null),
                    "freshness_rule": definition.get("freshness_rule").cloned().unwrap_or(serde_json::Value::Null),
                })
            })
            .unwrap_or_else(|| json!(null));

        Ok(ExecutionResult::Record(json!({
            "identity": {
                "registry_id": row.get::<String, _>("registry_id"),
                "fqn": row.get::<String, _>("fqn"),
                "uuid": row.get::<Uuid, _>("uuid"),
                "display_name": row.get::<String, _>("display_name"),
            },
            "governance": {
                "snapshot_id": row.get::<Option<Uuid>, _>("attribute_snapshot_id"),
                "version": row.get::<Option<String>, _>("attribute_snapshot_version"),
                "status": row.get::<Option<String>, _>("attribute_snapshot_status"),
                "evidence_grade": row.get::<String, _>("evidence_grade"),
                "governance_tier": row.get::<Option<String>, _>("governance_tier"),
            },
            "definition": {
                "data_type": row.get::<String, _>("value_type"),
                "domain": row.get::<Option<String>, _>("domain"),
                "source": row.get::<Option<serde_json::Value>, _>("attribute_source"),
                "constraints": row.get::<Option<serde_json::Value>, _>("attribute_constraints"),
            },
            "derivation": derivation,
            "operational": {
                "active_observations": row.get::<i64, _>("active_observations"),
                "cbu_values": row.get::<i64, _>("cbu_values"),
                "document_sources": row.get::<i64, _>("document_sources"),
            }
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("attribute.inspect requires database"))
    }
}
