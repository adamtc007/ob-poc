//! Observation custom operations
//!
//! Operations for attribute observations from various sources,
//! discrepancy detection, and allegation verification.

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

// ============================================================================
// Observation Operations
// ============================================================================

/// Record an observation from a document
#[register_custom_op]
pub struct ObservationFromDocumentOp;

#[async_trait]
impl CustomOperation for ObservationFromDocumentOp {
    fn domain(&self) -> &'static str {
        "observation"
    }
    fn verb(&self) -> &'static str {
        "record-from-document"
    }
    fn rationale(&self) -> &'static str {
        "Requires document lookup, attribute lookup, and automatic confidence/authoritative flags"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // Get entity-id (required)
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

        // Get document-id (required)
        let document_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "document-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing document-id argument"))?;

        // Get attribute name (required) and look up ID
        let attr_name = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "attribute")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing attribute argument"))?;

        let attribute_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT uuid FROM "ob-poc".attribute_registry WHERE id = $1 OR display_name = $1"#,
        )
        .bind(attr_name)
        .fetch_optional(pool)
        .await?;

        let attribute_id =
            attribute_id.ok_or_else(|| anyhow::anyhow!("Unknown attribute: {}", attr_name))?;

        // Get value (required)
        let value = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "value")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing value argument"))?;

        // Get optional extraction method
        let extraction_method = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "extraction-method")
            .and_then(|a| a.value.as_string());

        // Get optional confidence (default 0.80 for document extractions)
        let confidence: f64 = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "confidence")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.80);

        // Look up if this document type is authoritative for this attribute
        let is_authoritative: bool = sqlx::query_scalar(
            r#"SELECT COALESCE(dal.is_authoritative, FALSE)
               FROM "ob-poc".document_catalog dc
               LEFT JOIN "ob-poc".document_attribute_links dal
                 ON dal.document_type_id = dc.document_type_id
                 AND dal.attribute_id = $2
                 AND dal.direction IN ('SOURCE', 'BOTH')
               WHERE dc.doc_id = $1
               LIMIT 1"#,
        )
        .bind(document_id)
        .bind(attribute_id)
        .fetch_optional(pool)
        .await?
        .unwrap_or(false);

        // Insert observation
        let observation_id = Uuid::now_v7();

        sqlx::query(
            r#"INSERT INTO "ob-poc".attribute_observations
               (observation_id, entity_id, attribute_id, value_text, source_type,
                source_document_id, confidence, is_authoritative, extraction_method,
                observed_at, status)
               VALUES ($1, $2, $3, $4, 'DOCUMENT', $5, $6, $7, $8, NOW(), 'ACTIVE')"#,
        )
        .bind(observation_id)
        .bind(entity_id)
        .bind(attribute_id)
        .bind(value)
        .bind(document_id)
        .bind(confidence)
        .bind(is_authoritative)
        .bind(extraction_method)
        .execute(pool)
        .await?;

        ctx.bind("observation", observation_id);
        Ok(ExecutionResult::Uuid(observation_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(uuid::Uuid::now_v7()))
    }
}

/// Get current best observation for an attribute
#[register_custom_op]
pub struct ObservationGetCurrentOp;

#[async_trait]
impl CustomOperation for ObservationGetCurrentOp {
    fn domain(&self) -> &'static str {
        "observation"
    }
    fn verb(&self) -> &'static str {
        "get-current"
    }
    fn rationale(&self) -> &'static str {
        "Requires priority-based selection (authoritative > confidence > recency)"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // Get entity-id (required)
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

        // Get attribute name (required) and look up ID
        let attr_name = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "attribute")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing attribute argument"))?;

        let attribute_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT uuid FROM "ob-poc".attribute_registry WHERE id = $1 OR display_name = $1"#,
        )
        .bind(attr_name)
        .fetch_optional(pool)
        .await?;

        let attribute_id =
            attribute_id.ok_or_else(|| anyhow::anyhow!("Unknown attribute: {}", attr_name))?;

        // Get current best observation from view
        let result: Option<(
            Uuid,
            Option<String>,
            Option<rust_decimal::Decimal>,
            Option<bool>,
            Option<chrono::NaiveDate>,
            String,
            Option<rust_decimal::Decimal>,
            bool,
        )> = sqlx::query_as(
            r#"SELECT observation_id, value_text, value_number, value_boolean, value_date,
                      source_type, confidence, is_authoritative
               FROM "ob-poc".v_attribute_current
               WHERE entity_id = $1 AND attribute_id = $2"#,
        )
        .bind(entity_id)
        .bind(attribute_id)
        .fetch_optional(pool)
        .await?;

        match result {
            Some((
                obs_id,
                value_text,
                value_number,
                value_boolean,
                value_date,
                source_type,
                confidence,
                is_authoritative,
            )) => Ok(ExecutionResult::Record(serde_json::json!({
                "observation_id": obs_id,
                "value_text": value_text,
                "value_number": value_number,
                "value_boolean": value_boolean,
                "value_date": value_date,
                "source_type": source_type,
                "confidence": confidence,
                "is_authoritative": is_authoritative
            }))),
            None => Ok(ExecutionResult::Record(serde_json::json!({
                "found": false,
                "message": "No active observation found for this attribute"
            }))),
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "found": false,
            "message": "No database available"
        })))
    }
}

/// Reconcile observations for an attribute and auto-create discrepancies
///
/// Rationale: Compares all active observations for an entity+attribute,
/// identifies conflicts, and optionally creates discrepancy records.
#[register_custom_op]
pub struct ObservationReconcileOp;

#[async_trait]
impl CustomOperation for ObservationReconcileOp {
    fn domain(&self) -> &'static str {
        "observation"
    }
    fn verb(&self) -> &'static str {
        "reconcile"
    }
    fn rationale(&self) -> &'static str {
        "Requires comparing multiple observations and detecting value conflicts"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // Get entity-id (required)
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

        // Get attribute name
        let attr_name = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "attribute")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing attribute argument"))?;

        // Lookup attribute ID
        let attribute_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT uuid FROM "ob-poc".attribute_registry WHERE id = $1 OR display_name = $1"#,
        )
        .bind(attr_name)
        .fetch_optional(pool)
        .await?;

        let attribute_id =
            attribute_id.ok_or_else(|| anyhow::anyhow!("Unknown attribute: {}", attr_name))?;

        // Optional case-id
        let case_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "case-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });

        // Auto-create discrepancies (default true)
        let auto_create = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "auto-create-discrepancies")
            .and_then(|a| a.value.as_boolean())
            .unwrap_or(true);

        // Get all active observations for this entity+attribute
        let observations: Vec<(
            Uuid,
            Option<String>,
            String,
            Option<rust_decimal::Decimal>,
            bool,
        )> = sqlx::query_as(
            r#"SELECT observation_id, value_text, source_type, confidence, is_authoritative
                   FROM "ob-poc".attribute_observations
                   WHERE entity_id = $1 AND attribute_id = $2 AND status = 'ACTIVE'
                   ORDER BY is_authoritative DESC, confidence DESC NULLS LAST, observed_at DESC"#,
        )
        .bind(entity_id)
        .bind(attribute_id)
        .fetch_all(pool)
        .await?;

        if observations.len() < 2 {
            return Ok(ExecutionResult::Record(serde_json::json!({
                "status": "no_conflict",
                "observation_count": observations.len(),
                "discrepancies_created": 0
            })));
        }

        // Compare values to find discrepancies
        let mut discrepancies_created = 0;
        let first = &observations[0];

        for other in observations.iter().skip(1) {
            // Compare values (simplified - just text comparison for now)
            if first.1 != other.1 && auto_create {
                // Create discrepancy record
                let discrepancy_id = Uuid::now_v7();
                sqlx::query!(
                    r#"INSERT INTO "ob-poc".observation_discrepancies
                       (discrepancy_id, entity_id, attribute_id, observation_1_id, observation_2_id,
                        discrepancy_type, severity, description, case_id, resolution_status)
                       VALUES ($1, $2, $3, $4, $5, 'VALUE_MISMATCH', 'MEDIUM',
                               'Different values observed for same attribute', $6, 'OPEN')"#,
                    discrepancy_id,
                    entity_id,
                    attribute_id,
                    first.0,
                    other.0,
                    case_id
                )
                .execute(pool)
                .await?;
                discrepancies_created += 1;
            }
        }

        Ok(ExecutionResult::Record(serde_json::json!({
            "status": if discrepancies_created > 0 { "conflicts_found" } else { "no_conflict" },
            "observation_count": observations.len(),
            "discrepancies_created": discrepancies_created
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "status": "no_database",
            "discrepancies_created": 0
        })))
    }
}

/// Batch verify pending allegations against observations
///
/// Rationale: Compares pending allegations with authoritative observations
/// and auto-updates verification status.
#[register_custom_op]
pub struct ObservationVerifyAllegationsOp;

#[async_trait]
impl CustomOperation for ObservationVerifyAllegationsOp {
    fn domain(&self) -> &'static str {
        "observation"
    }
    fn verb(&self) -> &'static str {
        "verify-allegations"
    }
    fn rationale(&self) -> &'static str {
        "Requires joining allegations with observations and comparing values"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // Get cbu-id (required)
        let cbu_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "cbu-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing cbu-id argument"))?;

        // Get entity-id (required)
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

        // Get pending allegations for this entity
        let allegations: Vec<(Uuid, Uuid, serde_json::Value, Option<String>)> = sqlx::query_as(
            r#"SELECT allegation_id, attribute_id, alleged_value, alleged_value_display
               FROM "ob-poc".client_allegations
               WHERE cbu_id = $1 AND entity_id = $2 AND verification_status = 'PENDING'"#,
        )
        .bind(cbu_id)
        .bind(entity_id)
        .fetch_all(pool)
        .await?;

        let mut verified = 0;
        let mut contradicted = 0;
        let mut no_observation = 0;

        for (allegation_id, attribute_id, alleged_value, alleged_display) in allegations {
            // Get current observation for this attribute
            let current: Option<(Uuid, Option<String>)> = sqlx::query_as(
                r#"SELECT observation_id, value_text
                   FROM "ob-poc".v_attribute_current
                   WHERE entity_id = $1 AND attribute_id = $2"#,
            )
            .bind(entity_id)
            .bind(attribute_id)
            .fetch_optional(pool)
            .await?;

            match current {
                Some((observation_id, obs_value_text)) => {
                    // Compare alleged value with observation
                    let alleged_str = alleged_display
                        .or_else(|| alleged_value.as_str().map(String::from))
                        .unwrap_or_default();

                    let matches = obs_value_text
                        .as_ref()
                        .map(|v| v.to_lowercase() == alleged_str.to_lowercase())
                        .unwrap_or(false);

                    if matches {
                        // Verify the allegation
                        sqlx::query!(
                            r#"UPDATE "ob-poc".client_allegations
                               SET verification_status = 'VERIFIED',
                                   verified_by_observation_id = $2,
                                   verified_at = NOW()
                               WHERE allegation_id = $1"#,
                            allegation_id,
                            observation_id
                        )
                        .execute(pool)
                        .await?;
                        verified += 1;
                    } else {
                        // Contradict the allegation
                        sqlx::query!(
                            r#"UPDATE "ob-poc".client_allegations
                               SET verification_status = 'CONTRADICTED',
                                   verified_by_observation_id = $2,
                                   verified_at = NOW(),
                                   verification_notes = 'Value does not match observation'
                               WHERE allegation_id = $1"#,
                            allegation_id,
                            observation_id
                        )
                        .execute(pool)
                        .await?;
                        contradicted += 1;
                    }
                }
                None => {
                    no_observation += 1;
                }
            }
        }

        Ok(ExecutionResult::Record(serde_json::json!({
            "verified": verified,
            "contradicted": contradicted,
            "no_observation": no_observation,
            "total_processed": verified + contradicted + no_observation
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "verified": 0,
            "contradicted": 0,
            "no_observation": 0
        })))
    }
}

/// Extract document data and create observations
///
/// Rationale: Uses document_attribute_links to determine what attributes
/// a document can provide, extracts values, and creates observations.
#[register_custom_op]
pub struct DocumentExtractObservationsOp;

#[async_trait]
impl CustomOperation for DocumentExtractObservationsOp {
    fn domain(&self) -> &'static str {
        "document"
    }
    fn verb(&self) -> &'static str {
        "extract-to-observations"
    }
    fn rationale(&self) -> &'static str {
        "Requires joining document with attribute links and creating observations"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // Get document-id (required)
        let document_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "document-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing document-id argument"))?;

        // Get entity-id (required)
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

        // Auto-verify allegations (default true)
        let auto_verify = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "auto-verify-allegations")
            .and_then(|a| a.value.as_boolean())
            .unwrap_or(true);

        // Get document type
        let doc_type: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT document_type_id FROM "ob-poc".document_catalog WHERE doc_id = $1"#,
        )
        .bind(document_id)
        .fetch_optional(pool)
        .await?;

        let doc_type_id =
            doc_type.ok_or_else(|| anyhow::anyhow!("Document not found: {}", document_id))?;

        // Get attributes this document can provide (SOURCE direction)
        let extractable: Vec<(Uuid, String, Option<String>, Option<sqlx::types::BigDecimal>, bool)> =
            sqlx::query_as(
                r#"SELECT dal.attribute_id, ar.id, dal.extraction_method,
                          dal.extraction_confidence_default, dal.is_authoritative
                   FROM "ob-poc".document_attribute_links dal
                   JOIN "ob-poc".attribute_registry ar ON ar.uuid = dal.attribute_id
                   WHERE dal.document_type_id = $1 AND dal.direction = 'SOURCE' AND dal.is_active = TRUE"#,
            )
            .bind(doc_type_id)
            .fetch_all(pool)
            .await?;

        // For now, we create placeholder observations - actual extraction would call AI/OCR
        let mut observations_created = 0;

        for (attribute_id, attr_name, extraction_method, confidence, is_authoritative) in
            &extractable
        {
            let observation_id = Uuid::now_v7();

            // Create observation with placeholder value (real impl would extract from document)
            sqlx::query!(
                r#"INSERT INTO "ob-poc".attribute_observations
                   (observation_id, entity_id, attribute_id, value_text, source_type,
                    source_document_id, extraction_method, confidence, is_authoritative, status)
                   VALUES ($1, $2, $3, $4, 'DOCUMENT', $5, $6, $7, $8, 'ACTIVE')
                   ON CONFLICT (entity_id, attribute_id, source_type, source_document_id)
                   WHERE status = 'ACTIVE'
                   DO NOTHING"#,
                observation_id,
                entity_id,
                attribute_id,
                format!("[Extracted from document - {}]", attr_name), // Placeholder
                document_id,
                extraction_method.as_deref().unwrap_or("MANUAL"),
                confidence.clone(),
                is_authoritative
            )
            .execute(pool)
            .await?;

            observations_created += 1;
        }

        // Auto-verify allegations if requested
        let mut allegations_verified = 0;
        if auto_verify && observations_created > 0 {
            // Get CBU for this entity's document
            let cbu_id: Option<Uuid> = sqlx::query_scalar(
                r#"SELECT cbu_id FROM "ob-poc".document_catalog WHERE doc_id = $1"#,
            )
            .bind(document_id)
            .fetch_optional(pool)
            .await?;

            if let Some(cbu_id) = cbu_id {
                // Update matching pending allegations
                let result = sqlx::query!(
                    r#"UPDATE "ob-poc".client_allegations ca
                       SET verification_status = 'VERIFIED',
                           verified_at = NOW(),
                           verification_notes = 'Auto-verified by document extraction'
                       WHERE ca.cbu_id = $1 AND ca.entity_id = $2
                         AND ca.verification_status = 'PENDING'
                         AND EXISTS (
                           SELECT 1 FROM "ob-poc".attribute_observations ao
                           WHERE ao.entity_id = ca.entity_id
                             AND ao.attribute_id = ca.attribute_id
                             AND ao.source_document_id = $3
                             AND ao.status = 'ACTIVE'
                         )"#,
                    cbu_id,
                    entity_id,
                    document_id
                )
                .execute(pool)
                .await?;

                allegations_verified = result.rows_affected() as i32;
            }
        }

        Ok(ExecutionResult::Record(serde_json::json!({
            "observations_created": observations_created,
            "attributes_extractable": extractable.len(),
            "allegations_verified": allegations_verified
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "observations_created": 0,
            "attributes_extractable": 0,
            "allegations_verified": 0
        })))
    }
}
