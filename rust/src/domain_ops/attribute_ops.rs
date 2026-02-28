//! Attribute custom operations
//!
//! Operations for attribute dictionary management, document-attribute
//! mappings, and lineage tracing.

use anyhow::Result;
use async_trait::async_trait;
use governed_query_proc::governed_query;
use ob_poc_macros::register_custom_op;
use serde_json::json;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

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
        let attr = sqlx::query!(
            r#"
            SELECT id, display_name, category, value_type, domain,
                   requires_authoritative_source
            FROM "ob-poc".attribute_registry
            WHERE id = $1
            "#,
            attr_id
        )
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
                "id": attr.id,
                "display_name": attr.display_name,
                "category": attr.category,
                "value_type": attr.value_type,
                "domain": attr.domain,
                "requires_authoritative_source": attr.requires_authoritative_source
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
