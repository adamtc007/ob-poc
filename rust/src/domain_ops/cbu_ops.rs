//! CBU custom operations
//!
//! Operations for CBU (Client Business Unit) management including
//! product assignment, show, decide, and cascade delete.

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;

use super::helpers::{extract_bool_opt, extract_int_opt, extract_string_opt, get_required_uuid};
use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;
#[cfg(feature = "database")]
use uuid::Uuid;

// ============================================================================
// CBU Product Assignment
// ============================================================================

/// Add a product to a CBU by creating service_delivery_map and cbu_resource_instances entries
///
/// This is a CRITICAL onboarding operation that:
/// 1. Validates CBU exists
/// 2. Looks up product by name and validates it exists
/// 3. Validates product has services defined
/// 4. Creates service_delivery_map entries for ALL services under that product
/// 5. Creates cbu_resource_instances for ALL resource types under each service
///    (via service_resource_capabilities join) - one per (CBU, resource_type)
///
/// NOTE: A CBU can have MULTIPLE products. This verb adds one product at a time.
/// The service_delivery_map is the source of truth for CBU->Product relationships.
/// cbus.product_id is NOT used (legacy field).
///
/// Idempotency: Safe to re-run - uses ON CONFLICT DO NOTHING for all entries
/// Transaction: All operations wrapped in a transaction for atomicity
#[register_custom_op]
pub struct CbuAddProductOp;

#[async_trait]
impl CustomOperation for CbuAddProductOp {
    fn domain(&self) -> &'static str {
        "cbu"
    }
    fn verb(&self) -> &'static str {
        "add-product"
    }
    fn rationale(&self) -> &'static str {
        "Critical onboarding op: links CBU to product and creates service delivery entries"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // =====================================================================
        // Step 1: Extract and validate arguments
        // =====================================================================
        // cbu-id can be: @reference, UUID string, or CBU name string
        let cbu_id_arg = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "cbu-id")
            .ok_or_else(|| anyhow::anyhow!("cbu.add-product: Missing required argument :cbu-id"))?;

        let product_name = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "product")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| {
                anyhow::anyhow!("cbu.add-product: Missing required argument :product")
            })?;

        // =====================================================================
        // Step 2: Resolve CBU - by reference, UUID, or name
        // =====================================================================
        let (cbu_id, cbu_name): (Uuid, String) =
            if let Some(ref_name) = cbu_id_arg.value.as_symbol() {
                // It's a @reference - resolve from context
                let resolved_id = ctx.resolve(ref_name).ok_or_else(|| {
                    anyhow::anyhow!("cbu.add-product: Unresolved reference @{}", ref_name)
                })?;
                let row = sqlx::query!(
                    r#"SELECT cbu_id, name FROM "ob-poc".cbus WHERE cbu_id = $1"#,
                    resolved_id
                )
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!("cbu.add-product: CBU not found with id {}", resolved_id)
                })?;
                (row.cbu_id, row.name)
            } else if let Some(uuid_val) = cbu_id_arg.value.as_uuid() {
                // It's a UUID
                let row = sqlx::query!(
                    r#"SELECT cbu_id, name FROM "ob-poc".cbus WHERE cbu_id = $1"#,
                    uuid_val
                )
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!("cbu.add-product: CBU not found with id {}", uuid_val)
                })?;
                (row.cbu_id, row.name)
            } else if let Some(str_val) = cbu_id_arg.value.as_string() {
                // It's a string - try as UUID first, then as name
                if let Ok(uuid_val) = Uuid::parse_str(str_val) {
                    let row = sqlx::query!(
                        r#"SELECT cbu_id, name FROM "ob-poc".cbus WHERE cbu_id = $1"#,
                        uuid_val
                    )
                    .fetch_optional(pool)
                    .await?
                    .ok_or_else(|| {
                        anyhow::anyhow!("cbu.add-product: CBU not found with id {}", uuid_val)
                    })?;
                    (row.cbu_id, row.name)
                } else {
                    // Look up by name (case-insensitive)
                    let row = sqlx::query!(
                        r#"SELECT cbu_id, name FROM "ob-poc".cbus WHERE LOWER(name) = LOWER($1)"#,
                        str_val
                    )
                    .fetch_optional(pool)
                    .await?
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                        "cbu.add-product: CBU '{}' not found. Use cbu.list to see available CBUs.",
                        str_val
                    )
                    })?;
                    (row.cbu_id, row.name)
                }
            } else {
                return Err(anyhow::anyhow!(
                    "cbu.add-product: :cbu-id must be a @reference, UUID, or CBU name string"
                ));
            };

        // Note: We don't touch cbus.product_id - service_delivery_map is source of truth

        // =====================================================================
        // Step 3: Validate product exists and get its ID (lookup by product_code)
        // =====================================================================
        let product_row = sqlx::query!(
            r#"SELECT product_id, name, product_code FROM "ob-poc".products WHERE product_code = $1"#,
            product_name
        )
        .fetch_optional(pool)
        .await?;

        let product = product_row.ok_or_else(|| {
            anyhow::anyhow!(
                "cbu.add-product: Product '{}' not found. Use product codes: CUSTODY, FUND_ACCOUNTING, TRANSFER_AGENCY, MIDDLE_OFFICE, COLLATERAL_MGMT, MARKETS_FX, ALTS",
                product_name
            )
        })?;

        let product_id = product.product_id;

        // =====================================================================
        // Step 4: Get all services for this product
        // =====================================================================
        let services = sqlx::query!(
            r#"SELECT ps.service_id, s.name as service_name
               FROM "ob-poc".product_services ps
               JOIN "ob-poc".services s ON ps.service_id = s.service_id
               WHERE ps.product_id = $1
               ORDER BY s.name"#,
            product_id
        )
        .fetch_all(pool)
        .await?;

        if services.is_empty() {
            return Err(anyhow::anyhow!(
                "cbu.add-product: Product '{}' has no services defined in product_services. \
                 Cannot add product without services.",
                product_name
            ));
        }

        // =====================================================================
        // Step 5: Execute in transaction
        // =====================================================================
        let mut tx = pool.begin().await?;

        // 5a: Create service_delivery_map entries for each service
        let mut delivery_created: i64 = 0;
        let mut delivery_skipped: i64 = 0;

        for svc in &services {
            let delivery_id = Uuid::new_v4();
            let result = sqlx::query(
                r#"INSERT INTO "ob-poc".service_delivery_map
                   (delivery_id, cbu_id, product_id, service_id, delivery_status)
                   VALUES ($1, $2, $3, $4, 'PENDING')
                   ON CONFLICT (cbu_id, product_id, service_id) DO NOTHING"#,
            )
            .bind(delivery_id)
            .bind(cbu_id)
            .bind(product_id)
            .bind(svc.service_id)
            .execute(&mut *tx)
            .await?;

            if result.rows_affected() > 0 {
                delivery_created += 1;
            } else {
                delivery_skipped += 1;
            }
        }

        // =====================================================================
        // Step 5b: Create cbu_resource_instances for each service's resource types
        // =====================================================================
        let mut resource_created: i64 = 0;
        let mut resource_skipped: i64 = 0;

        // Get all (service, resource_type) pairs for this product
        // Each service-resource combination gets its own instance
        // e.g., SWIFT Connection under Trade Settlement is separate from SWIFT Connection under Income Collection
        let service_resources = sqlx::query!(
            r#"SELECT src.service_id, src.resource_id, srt.resource_code, srt.name as resource_name
               FROM "ob-poc".service_resource_capabilities src
               JOIN "ob-poc".service_resource_types srt ON src.resource_id = srt.resource_id
               WHERE src.service_id IN (
                   SELECT service_id FROM "ob-poc".product_services WHERE product_id = $1
               )
               AND src.is_active = true
               ORDER BY src.service_id, srt.name"#,
            product_id
        )
        .fetch_all(&mut *tx)
        .await?;

        for sr in &service_resources {
            let instance_id = Uuid::new_v4();
            // Generate a unique instance URL using CBU name, resource code, and partial UUID
            let instance_url = format!(
                "urn:ob-poc:{}:{}:{}",
                cbu_name.to_lowercase().replace(' ', "-"),
                sr.resource_code.as_deref().unwrap_or("unknown"),
                &instance_id.to_string()[..8]
            );

            // Unique key is (cbu_id, product_id, service_id, resource_type_id)
            // One resource instance per service per resource type
            let result = sqlx::query(
                r#"INSERT INTO "ob-poc".cbu_resource_instances
                   (instance_id, cbu_id, product_id, service_id, resource_type_id,
                    instance_url, instance_name, status)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, 'PENDING')
                   ON CONFLICT (cbu_id, product_id, service_id, resource_type_id) DO NOTHING"#,
            )
            .bind(instance_id)
            .bind(cbu_id)
            .bind(product_id)
            .bind(sr.service_id)
            .bind(sr.resource_id)
            .bind(&instance_url)
            .bind(&sr.resource_name)
            .execute(&mut *tx)
            .await?;

            if result.rows_affected() > 0 {
                resource_created += 1;
            } else {
                resource_skipped += 1;
            }
        }

        // Commit transaction
        tx.commit().await?;

        // =====================================================================
        // Step 6: Log result for debugging
        // =====================================================================
        tracing::info!(
            cbu_id = %cbu_id,
            cbu_name = %cbu_name,
            product = %product_name,
            services_total = services.len(),
            delivery_entries_created = delivery_created,
            delivery_entries_skipped = delivery_skipped,
            resource_instances_created = resource_created,
            resource_instances_skipped = resource_skipped,
            "cbu.add-product completed"
        );

        // Return total entries created (deliveries + resources)
        Ok(ExecutionResult::Affected(
            (delivery_created + resource_created) as u64,
        ))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Affected(0))
    }
}

// ============================================================================
// CBU Show Operation
// ============================================================================

/// Show full CBU structure including entities, roles, documents, screenings
///
/// Rationale: Requires multiple joins across CBU, entities, roles, documents,
/// screenings, and service deliveries to build a complete picture.
#[register_custom_op]
pub struct CbuShowOp;

#[async_trait]
impl CustomOperation for CbuShowOp {
    fn domain(&self) -> &'static str {
        "cbu"
    }
    fn verb(&self) -> &'static str {
        "show"
    }
    fn rationale(&self) -> &'static str {
        "Requires aggregating data from multiple tables into a structured view"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use chrono::NaiveDate;
        use uuid::Uuid;

        // Get CBU ID
        let cbu_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "cbu-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else if let Some(uuid_val) = a.value.as_uuid() {
                    Some(uuid_val)
                } else if let Some(str_val) = a.value.as_string() {
                    Uuid::parse_str(str_val).ok()
                } else {
                    None
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid cbu-id argument"))?;

        // Get as-of-date (optional, defaults to today)
        let as_of_date: NaiveDate = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "as-of-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());

        // Get basic CBU info
        let cbu = sqlx::query!(
            r#"SELECT cbu_id, name, jurisdiction, client_type, cbu_category,
                      nature_purpose, description, created_at, updated_at
               FROM "ob-poc".cbus WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("CBU not found: {}", cbu_id))?;

        // Get entities with their roles (filtered by as_of_date)
        let entities = sqlx::query!(
            r#"SELECT DISTINCT e.entity_id, e.name, et.type_code as entity_type,
                      COALESCE(lc.jurisdiction, pp.nationality, p.jurisdiction, t.jurisdiction) as jurisdiction
               FROM "ob-poc".cbu_entity_roles cer
               JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
               JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
               LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
               LEFT JOIN "ob-poc".entity_proper_persons pp ON e.entity_id = pp.entity_id
               LEFT JOIN "ob-poc".entity_partnerships p ON e.entity_id = p.entity_id
               LEFT JOIN "ob-poc".entity_trusts t ON e.entity_id = t.entity_id
               WHERE cer.cbu_id = $1
               AND (cer.effective_from IS NULL OR cer.effective_from <= $2)
               AND (cer.effective_to IS NULL OR cer.effective_to >= $2)
               ORDER BY e.name"#,
            cbu_id,
            as_of_date
        )
        .fetch_all(pool)
        .await?;

        // Get roles per entity (filtered by as_of_date)
        let roles = sqlx::query!(
            r#"SELECT cer.entity_id, r.name as role_name
               FROM "ob-poc".cbu_entity_roles cer
               JOIN "ob-poc".roles r ON cer.role_id = r.role_id
               WHERE cer.cbu_id = $1
               AND (cer.effective_from IS NULL OR cer.effective_from <= $2)
               AND (cer.effective_to IS NULL OR cer.effective_to >= $2)
               ORDER BY cer.entity_id, r.name"#,
            cbu_id,
            as_of_date
        )
        .fetch_all(pool)
        .await?;

        // Build entity list with roles
        let entity_list: Vec<serde_json::Value> = entities
            .iter()
            .map(|e| {
                let entity_roles: Vec<String> = roles
                    .iter()
                    .filter(|r| r.entity_id == e.entity_id)
                    .map(|r| r.role_name.clone())
                    .collect();
                serde_json::json!({
                    "entity_id": e.entity_id,
                    "name": e.name,
                    "entity_type": e.entity_type,
                    "jurisdiction": e.jurisdiction,
                    "roles": entity_roles
                })
            })
            .collect();

        // Get documents
        let documents = sqlx::query!(
            r#"SELECT dc.doc_id, dc.document_name, dt.type_code, dt.display_name, dc.status
               FROM "ob-poc".document_catalog dc
               LEFT JOIN "ob-poc".document_types dt ON dc.document_type_id = dt.type_id
               WHERE dc.cbu_id = $1
               ORDER BY dt.type_code"#,
            cbu_id
        )
        .fetch_all(pool)
        .await?;

        let doc_list: Vec<serde_json::Value> = documents
            .iter()
            .map(|d| {
                serde_json::json!({
                    "doc_id": d.doc_id,
                    "name": d.document_name,
                    "type_code": d.type_code,
                    "type_name": d.display_name,
                    "status": d.status
                })
            })
            .collect();

        // Get screenings (via KYC workstreams)
        let screenings = sqlx::query!(
            r#"SELECT s.screening_id, w.entity_id, e.name as entity_name,
                      s.screening_type, s.status, s.result_summary
               FROM kyc.screenings s
               JOIN kyc.entity_workstreams w ON w.workstream_id = s.workstream_id
               JOIN kyc.cases c ON c.case_id = w.case_id
               JOIN "ob-poc".entities e ON e.entity_id = w.entity_id
               WHERE c.cbu_id = $1
               ORDER BY s.screening_type, e.name"#,
            cbu_id
        )
        .fetch_all(pool)
        .await?;

        let screening_list: Vec<serde_json::Value> = screenings
            .iter()
            .map(|s| {
                serde_json::json!({
                    "screening_id": s.screening_id,
                    "entity_id": s.entity_id,
                    "entity_name": s.entity_name,
                    "screening_type": s.screening_type,
                    "status": s.status,
                    "result": s.result_summary
                })
            })
            .collect();

        // Get service deliveries
        let services = sqlx::query!(
            r#"SELECT sdm.delivery_id, p.name as product_name, p.product_code,
                      s.name as service_name, sdm.delivery_status
               FROM "ob-poc".service_delivery_map sdm
               JOIN "ob-poc".products p ON p.product_id = sdm.product_id
               JOIN "ob-poc".services s ON s.service_id = sdm.service_id
               WHERE sdm.cbu_id = $1
               ORDER BY p.name, s.name"#,
            cbu_id
        )
        .fetch_all(pool)
        .await?;

        let service_list: Vec<serde_json::Value> = services
            .iter()
            .map(|s| {
                serde_json::json!({
                    "delivery_id": s.delivery_id,
                    "product": s.product_name,
                    "product_code": s.product_code,
                    "service": s.service_name,
                    "status": s.delivery_status
                })
            })
            .collect();

        // Get KYC cases
        let cases = sqlx::query!(
            r#"SELECT case_id, status, case_type, risk_rating, escalation_level,
                      opened_at, closed_at
               FROM kyc.cases
               WHERE cbu_id = $1
               ORDER BY opened_at DESC"#,
            cbu_id
        )
        .fetch_all(pool)
        .await?;

        let case_list: Vec<serde_json::Value> = cases
            .iter()
            .map(|c| {
                serde_json::json!({
                    "case_id": c.case_id,
                    "status": c.status,
                    "case_type": c.case_type,
                    "risk_rating": c.risk_rating,
                    "escalation_level": c.escalation_level,
                    "opened_at": c.opened_at.to_rfc3339(),
                    "closed_at": c.closed_at.map(|t| t.to_rfc3339())
                })
            })
            .collect();

        // Build complete result
        let result = serde_json::json!({
            "cbu_id": cbu.cbu_id,
            "name": cbu.name,
            "jurisdiction": cbu.jurisdiction,
            "client_type": cbu.client_type,
            "category": cbu.cbu_category,
            "nature_purpose": cbu.nature_purpose,
            "description": cbu.description,
            "created_at": cbu.created_at.map(|t| t.to_rfc3339()),
            "updated_at": cbu.updated_at.map(|t| t.to_rfc3339()),
            "as_of_date": as_of_date.to_string(),
            "entities": entity_list,
            "documents": doc_list,
            "screenings": screening_list,
            "services": service_list,
            "kyc_cases": case_list,
            "summary": {
                "entity_count": entity_list.len(),
                "document_count": doc_list.len(),
                "screening_count": screening_list.len(),
                "service_count": service_list.len(),
                "case_count": case_list.len()
            }
        });

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "error": "Database required for cbu.show"
        })))
    }
}

// ============================================================================
// CBU Decision Operation
// ============================================================================

/// Record KYC/AML decision for CBU collective state
///
/// Rationale: This is the decision point verb. Its execution in DSL history
/// IS the searchable snapshot boundary. Updates CBU status, case status,
/// and creates evaluation snapshot.
#[register_custom_op]
pub struct CbuDecideOp;

#[async_trait]
impl CustomOperation for CbuDecideOp {
    fn domain(&self) -> &'static str {
        "cbu"
    }
    fn verb(&self) -> &'static str {
        "decide"
    }
    fn rationale(&self) -> &'static str {
        "Decision point for CBU collective state - searchable in DSL history"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // Extract required args
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

        let decision = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "decision")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing decision argument"))?;

        let decided_by = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "decided-by")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing decided-by argument"))?;

        let rationale = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "rationale")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing rationale argument"))?;

        // Optional args
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

        let conditions = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "conditions")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let escalation_reason = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "escalation-reason")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        // Validate: REFERRED requires escalation-reason
        if decision == "REFERRED" && escalation_reason.is_none() {
            return Err(anyhow::anyhow!(
                "escalation-reason is required when decision is REFERRED"
            ));
        }

        // Get current CBU
        let cbu = sqlx::query!(
            r#"SELECT name, status FROM "ob-poc".cbus WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("CBU not found: {}", cbu_id))?;

        // Map decision to new CBU status
        // Valid statuses: DISCOVERED, VALIDATION_PENDING, VALIDATED, UPDATE_PENDING_PROOF, VALIDATION_FAILED
        let new_cbu_status = match decision {
            "APPROVED" => "VALIDATED",
            "REJECTED" => "VALIDATION_FAILED",
            "REFERRED" => "VALIDATION_PENDING", // Stays pending, escalated for review
            _ => return Err(anyhow::anyhow!("Invalid decision: {}", decision)),
        };

        // Map decision to case status
        // Valid: INTAKE, DISCOVERY, ASSESSMENT, REVIEW, APPROVED, REJECTED, BLOCKED, WITHDRAWN, EXPIRED, REFER_TO_REGULATOR, DO_NOT_ONBOARD
        let new_case_status = match decision {
            "APPROVED" => "APPROVED",
            "REJECTED" => "REJECTED",
            "REFERRED" => "REVIEW", // Stays in REVIEW with escalation
            _ => "REVIEW",
        };

        // Find or validate case_id
        let case_id = match case_id {
            Some(id) => id,
            None => {
                // Find active case for this CBU
                let row = sqlx::query!(
                    r#"SELECT case_id FROM kyc.cases
                       WHERE cbu_id = $1 AND status NOT IN ('APPROVED', 'REJECTED', 'WITHDRAWN', 'EXPIRED')
                       ORDER BY opened_at DESC LIMIT 1"#,
                    cbu_id
                )
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| anyhow::anyhow!("No active KYC case found for CBU"))?;
                row.case_id
            }
        };

        // Begin transaction
        let mut tx = pool.begin().await?;

        // 1. Update CBU status
        sqlx::query!(
            r#"UPDATE "ob-poc".cbus SET status = $1, updated_at = now() WHERE cbu_id = $2"#,
            new_cbu_status,
            cbu_id
        )
        .execute(&mut *tx)
        .await?;

        // 2. Update case status
        let should_close = matches!(decision, "APPROVED" | "REJECTED");
        if should_close {
            sqlx::query!(
                r#"UPDATE kyc.cases SET status = $1, closed_at = now(), last_activity_at = now() WHERE case_id = $2"#,
                new_case_status,
                case_id
            )
            .execute(&mut *tx)
            .await?;
        } else {
            // REFERRED - update escalation level
            sqlx::query!(
                r#"UPDATE kyc.cases SET escalation_level = 'SENIOR_COMPLIANCE', last_activity_at = now() WHERE case_id = $1"#,
                case_id
            )
            .execute(&mut *tx)
            .await?;
        }

        // 3. Create evaluation snapshot with decision
        let snapshot_id = Uuid::new_v4();
        sqlx::query!(
            r#"INSERT INTO "ob-poc".case_evaluation_snapshots
               (snapshot_id, case_id, soft_count, escalate_count, hard_stop_count, total_score,
                recommended_action, evaluated_by, decision_made, decision_made_at, decision_made_by, decision_notes)
               VALUES ($1, $2, 0, 0, 0, 0, $3, $4, $3, now(), $4, $5)"#,
            snapshot_id,
            case_id,
            decision,
            decided_by,
            rationale
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        // Return decision record
        Ok(ExecutionResult::Record(serde_json::json!({
            "cbu_id": cbu_id,
            "cbu_name": cbu.name,
            "case_id": case_id,
            "snapshot_id": snapshot_id,
            "decision": decision,
            "previous_status": cbu.status,
            "new_status": new_cbu_status,
            "decided_by": decided_by,
            "rationale": rationale,
            "conditions": conditions,
            "escalation_reason": escalation_reason
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "error": "Database required for cbu.decide"
        })))
    }
}

// ============================================================================
// CBU Delete Cascade Operation
// ============================================================================

/// Delete a CBU and all related data with cascade
///
/// Rationale: Requires ordered deletion across 25+ dependent tables in multiple
/// schemas (ob-poc, kyc, custody). Also handles entity deletion with shared-entity
/// check - entities linked to multiple CBUs are preserved.
///
/// WARNING: This is a destructive operation. Use with caution.
#[register_custom_op]
pub struct CbuDeleteCascadeOp;

#[async_trait]
impl CustomOperation for CbuDeleteCascadeOp {
    fn domain(&self) -> &'static str {
        "cbu"
    }
    fn verb(&self) -> &'static str {
        "delete-cascade"
    }
    fn rationale(&self) -> &'static str {
        "Requires ordered deletion across 25+ tables with FK dependencies and shared-entity check"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // Get CBU ID
        let cbu_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "cbu-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else if let Some(uuid_val) = a.value.as_uuid() {
                    Some(uuid_val)
                } else if let Some(str_val) = a.value.as_string() {
                    Uuid::parse_str(str_val).ok()
                } else {
                    None
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid cbu-id argument"))?;

        // Get delete-entities flag (default true)
        let delete_entities = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "delete-entities")
            .and_then(|a| a.value.as_boolean())
            .unwrap_or(true);

        // Verify CBU exists
        let cbu = sqlx::query!(
            r#"SELECT name FROM "ob-poc".cbus WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("CBU not found: {}", cbu_id))?;

        let cbu_name = cbu.name;

        // Track deletion counts
        let mut deleted_counts: std::collections::HashMap<String, i64> =
            std::collections::HashMap::new();

        // Begin transaction
        let mut tx = pool.begin().await?;

        // =====================================================================
        // Phase 1: Delete from kyc schema tables
        // =====================================================================

        // kyc.screenings (via workstreams via cases)
        let result = sqlx::query(
            r#"DELETE FROM kyc.screenings WHERE workstream_id IN (
                SELECT workstream_id FROM kyc.entity_workstreams WHERE case_id IN (
                    SELECT case_id FROM kyc.cases WHERE cbu_id = $1
                )
            )"#,
        )
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;
        deleted_counts.insert("kyc.screenings".to_string(), result.rows_affected() as i64);

        // kyc.doc_requests (via workstreams)
        let result = sqlx::query(
            r#"DELETE FROM kyc.doc_requests WHERE workstream_id IN (
                SELECT workstream_id FROM kyc.entity_workstreams WHERE case_id IN (
                    SELECT case_id FROM kyc.cases WHERE cbu_id = $1
                )
            )"#,
        )
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;
        deleted_counts.insert(
            "kyc.doc_requests".to_string(),
            result.rows_affected() as i64,
        );

        // kyc.red_flags
        let result = sqlx::query(
            r#"DELETE FROM kyc.red_flags WHERE case_id IN (
                SELECT case_id FROM kyc.cases WHERE cbu_id = $1
            )"#,
        )
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;
        deleted_counts.insert("kyc.red_flags".to_string(), result.rows_affected() as i64);

        // kyc.case_events
        let result = sqlx::query(
            r#"DELETE FROM kyc.case_events WHERE case_id IN (
                SELECT case_id FROM kyc.cases WHERE cbu_id = $1
            )"#,
        )
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;
        deleted_counts.insert("kyc.case_events".to_string(), result.rows_affected() as i64);

        // kyc.entity_workstreams
        let result = sqlx::query(
            r#"DELETE FROM kyc.entity_workstreams WHERE case_id IN (
                SELECT case_id FROM kyc.cases WHERE cbu_id = $1
            )"#,
        )
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;
        deleted_counts.insert(
            "kyc.entity_workstreams".to_string(),
            result.rows_affected() as i64,
        );

        // kyc.cases
        let result = sqlx::query(r#"DELETE FROM kyc.cases WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&mut *tx)
            .await?;
        deleted_counts.insert("kyc.cases".to_string(), result.rows_affected() as i64);

        // kyc.share_classes
        let result = sqlx::query(r#"DELETE FROM kyc.share_classes WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&mut *tx)
            .await?;
        deleted_counts.insert(
            "kyc.share_classes".to_string(),
            result.rows_affected() as i64,
        );

        // =====================================================================
        // Phase 2: Delete from custody schema tables
        // =====================================================================

        // custody.ssi_booking_rules
        let result = sqlx::query(r#"DELETE FROM custody.ssi_booking_rules WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&mut *tx)
            .await?;
        deleted_counts.insert(
            "custody.ssi_booking_rules".to_string(),
            result.rows_affected() as i64,
        );

        // custody.cbu_ssi_agent_override (via cbu_ssi)
        let result = sqlx::query(
            r#"DELETE FROM custody.cbu_ssi_agent_override WHERE ssi_id IN (
                SELECT ssi_id FROM custody.cbu_ssi WHERE cbu_id = $1
            )"#,
        )
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;
        deleted_counts.insert(
            "custody.cbu_ssi_agent_override".to_string(),
            result.rows_affected() as i64,
        );

        // custody.cbu_ssi
        let result = sqlx::query(r#"DELETE FROM custody.cbu_ssi WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&mut *tx)
            .await?;
        deleted_counts.insert("custody.cbu_ssi".to_string(), result.rows_affected() as i64);

        // custody.cbu_instrument_universe
        let result =
            sqlx::query(r#"DELETE FROM custody.cbu_instrument_universe WHERE cbu_id = $1"#)
                .bind(cbu_id)
                .execute(&mut *tx)
                .await?;
        deleted_counts.insert(
            "custody.cbu_instrument_universe".to_string(),
            result.rows_affected() as i64,
        );

        // custody.csa_agreements (via isda_agreements)
        let result = sqlx::query(
            r#"DELETE FROM custody.csa_agreements WHERE isda_id IN (
                SELECT isda_id FROM custody.isda_agreements WHERE cbu_id = $1
            )"#,
        )
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;
        deleted_counts.insert(
            "custody.csa_agreements".to_string(),
            result.rows_affected() as i64,
        );

        // custody.isda_product_coverage (via isda_agreements)
        let result = sqlx::query(
            r#"DELETE FROM custody.isda_product_coverage WHERE isda_id IN (
                SELECT isda_id FROM custody.isda_agreements WHERE cbu_id = $1
            )"#,
        )
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;
        deleted_counts.insert(
            "custody.isda_product_coverage".to_string(),
            result.rows_affected() as i64,
        );

        // custody.isda_agreements
        let result = sqlx::query(r#"DELETE FROM custody.isda_agreements WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&mut *tx)
            .await?;
        deleted_counts.insert(
            "custody.isda_agreements".to_string(),
            result.rows_affected() as i64,
        );

        // =====================================================================
        // Phase 3: Delete from ob-poc schema tables (CBU-dependent)
        // =====================================================================

        // resource_instance_attributes (via cbu_resource_instances)
        let result = sqlx::query(
            r#"DELETE FROM "ob-poc".resource_instance_attributes WHERE instance_id IN (
                SELECT instance_id FROM "ob-poc".cbu_resource_instances WHERE cbu_id = $1
            )"#,
        )
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;
        deleted_counts.insert(
            "resource_instance_attributes".to_string(),
            result.rows_affected() as i64,
        );

        // resource_instance_dependencies (via cbu_resource_instances)
        let result = sqlx::query(
            r#"DELETE FROM "ob-poc".resource_instance_dependencies WHERE instance_id IN (
                SELECT instance_id FROM "ob-poc".cbu_resource_instances WHERE cbu_id = $1
            ) OR depends_on_instance_id IN (
                SELECT instance_id FROM "ob-poc".cbu_resource_instances WHERE cbu_id = $1
            )"#,
        )
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;
        deleted_counts.insert(
            "resource_instance_dependencies".to_string(),
            result.rows_affected() as i64,
        );

        // cbu_resource_instances
        let result =
            sqlx::query(r#"DELETE FROM "ob-poc".cbu_resource_instances WHERE cbu_id = $1"#)
                .bind(cbu_id)
                .execute(&mut *tx)
                .await?;
        deleted_counts.insert(
            "cbu_resource_instances".to_string(),
            result.rows_affected() as i64,
        );

        // service_delivery_map
        let result = sqlx::query(r#"DELETE FROM "ob-poc".service_delivery_map WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&mut *tx)
            .await?;
        deleted_counts.insert(
            "service_delivery_map".to_string(),
            result.rows_affected() as i64,
        );

        // cbu_evidence
        let result = sqlx::query(r#"DELETE FROM "ob-poc".cbu_evidence WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&mut *tx)
            .await?;
        deleted_counts.insert("cbu_evidence".to_string(), result.rows_affected() as i64);

        // ubo_evidence (via ubo_registry)
        let result = sqlx::query(
            r#"DELETE FROM "ob-poc".ubo_evidence WHERE ubo_id IN (
                SELECT ubo_id FROM "ob-poc".ubo_registry WHERE cbu_id = $1
            )"#,
        )
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;
        deleted_counts.insert("ubo_evidence".to_string(), result.rows_affected() as i64);

        // ubo_registry
        let result = sqlx::query(r#"DELETE FROM "ob-poc".ubo_registry WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&mut *tx)
            .await?;
        deleted_counts.insert("ubo_registry".to_string(), result.rows_affected() as i64);

        // ubo_snapshots
        let result = sqlx::query(r#"DELETE FROM "ob-poc".ubo_snapshots WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&mut *tx)
            .await?;
        deleted_counts.insert("ubo_snapshots".to_string(), result.rows_affected() as i64);

        // case_evaluation_snapshots (via kyc.cases - already deleted cases but snapshot may remain)
        // Note: FK may reference deleted case, so we use cbu_id directly
        let result = sqlx::query(
            r#"DELETE FROM "ob-poc".case_evaluation_snapshots WHERE case_id IN (
                SELECT case_id FROM kyc.cases WHERE cbu_id = $1
            )"#,
        )
        .bind(cbu_id)
        .execute(&mut *tx)
        .await
        .unwrap_or_else(|_| sqlx::postgres::PgQueryResult::default());
        deleted_counts.insert(
            "case_evaluation_snapshots".to_string(),
            result.rows_affected() as i64,
        );

        // kyc_investigations
        let result = sqlx::query(r#"DELETE FROM "ob-poc".kyc_investigations WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&mut *tx)
            .await?;
        deleted_counts.insert(
            "kyc_investigations".to_string(),
            result.rows_affected() as i64,
        );

        // kyc_decisions
        let result = sqlx::query(r#"DELETE FROM "ob-poc".kyc_decisions WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&mut *tx)
            .await?;
        deleted_counts.insert("kyc_decisions".to_string(), result.rows_affected() as i64);

        // screenings (legacy ob-poc schema)
        let result = sqlx::query(
            r#"DELETE FROM "ob-poc".screenings WHERE investigation_id IN (
            SELECT investigation_id FROM "ob-poc".kyc_investigations WHERE cbu_id = $1
        )"#,
        )
        .bind(cbu_id)
        .execute(&mut *tx)
        .await
        .unwrap_or_else(|_| sqlx::postgres::PgQueryResult::default());
        deleted_counts.insert("screenings".to_string(), result.rows_affected() as i64);

        // document_catalog
        let result = sqlx::query(r#"DELETE FROM "ob-poc".document_catalog WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&mut *tx)
            .await?;
        deleted_counts.insert(
            "document_catalog".to_string(),
            result.rows_affected() as i64,
        );

        // client_allegations
        let result = sqlx::query(r#"DELETE FROM "ob-poc".client_allegations WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&mut *tx)
            .await?;
        deleted_counts.insert(
            "client_allegations".to_string(),
            result.rows_affected() as i64,
        );

        // cbu_trading_profiles
        let result = sqlx::query(r#"DELETE FROM "ob-poc".cbu_trading_profiles WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&mut *tx)
            .await?;
        deleted_counts.insert(
            "cbu_trading_profiles".to_string(),
            result.rows_affected() as i64,
        );

        // cbu_layout_overrides
        let result = sqlx::query(r#"DELETE FROM "ob-poc".cbu_layout_overrides WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&mut *tx)
            .await?;
        deleted_counts.insert(
            "cbu_layout_overrides".to_string(),
            result.rows_affected() as i64,
        );

        // cbu_change_log
        let result = sqlx::query(r#"DELETE FROM "ob-poc".cbu_change_log WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&mut *tx)
            .await?;
        deleted_counts.insert("cbu_change_log".to_string(), result.rows_affected() as i64);

        // =====================================================================
        // Phase 4: Handle entities (with shared-entity check)
        // =====================================================================

        let mut entities_deleted: i64 = 0;
        let mut entities_preserved: i64 = 0;

        if delete_entities {
            // Get entities linked ONLY to this CBU (not shared with other CBUs)
            let exclusive_entities: Vec<Uuid> = sqlx::query_scalar(
                r#"SELECT entity_id FROM "ob-poc".cbu_entity_roles
                   WHERE cbu_id = $1
                   AND entity_id NOT IN (
                       SELECT entity_id FROM "ob-poc".cbu_entity_roles
                       WHERE cbu_id != $1
                   )"#,
            )
            .bind(cbu_id)
            .fetch_all(&mut *tx)
            .await?;

            // Count entities that are shared (preserved)
            let shared_count: Option<i64> = sqlx::query_scalar(
                r#"SELECT COUNT(DISTINCT entity_id)::bigint FROM "ob-poc".cbu_entity_roles
                   WHERE cbu_id = $1
                   AND entity_id IN (
                       SELECT entity_id FROM "ob-poc".cbu_entity_roles
                       WHERE cbu_id != $1
                   )"#,
            )
            .bind(cbu_id)
            .fetch_one(&mut *tx)
            .await?;
            entities_preserved = shared_count.unwrap_or(0);

            // Delete entity extension table records for exclusive entities
            for entity_id in &exclusive_entities {
                // Delete from all extension tables (ignore errors for tables that don't have the entity)
                let _ = sqlx::query(
                    r#"DELETE FROM "ob-poc".entity_proper_persons WHERE entity_id = $1"#,
                )
                .bind(entity_id)
                .execute(&mut *tx)
                .await;
                let _ = sqlx::query(
                    r#"DELETE FROM "ob-poc".entity_limited_companies WHERE entity_id = $1"#,
                )
                .bind(entity_id)
                .execute(&mut *tx)
                .await;
                let _ =
                    sqlx::query(r#"DELETE FROM "ob-poc".entity_partnerships WHERE entity_id = $1"#)
                        .bind(entity_id)
                        .execute(&mut *tx)
                        .await;
                let _ = sqlx::query(r#"DELETE FROM "ob-poc".entity_trusts WHERE entity_id = $1"#)
                    .bind(entity_id)
                    .execute(&mut *tx)
                    .await;
                let _ = sqlx::query(r#"DELETE FROM "ob-poc".entity_funds WHERE entity_id = $1"#)
                    .bind(entity_id)
                    .execute(&mut *tx)
                    .await;
                let _ = sqlx::query(
                    r#"DELETE FROM "ob-poc".entity_share_classes WHERE entity_id = $1"#,
                )
                .bind(entity_id)
                .execute(&mut *tx)
                .await;
                let _ = sqlx::query(r#"DELETE FROM "ob-poc".entity_manco WHERE entity_id = $1"#)
                    .bind(entity_id)
                    .execute(&mut *tx)
                    .await;

                // Delete from entity_kyc_status
                let _ =
                    sqlx::query(r#"DELETE FROM "ob-poc".entity_kyc_status WHERE entity_id = $1"#)
                        .bind(entity_id)
                        .execute(&mut *tx)
                        .await;

                // Delete from entity_relationships (both sides - covers ownership, control, trust_role)
                let _ = sqlx::query(
                    r#"DELETE FROM "ob-poc".entity_relationships
                       WHERE from_entity_id = $1 OR to_entity_id = $1"#,
                )
                .bind(entity_id)
                .execute(&mut *tx)
                .await;

                // Delete attribute_observations for this entity
                let _ = sqlx::query(
                    r#"DELETE FROM "ob-poc".attribute_observations WHERE entity_id = $1"#,
                )
                .bind(entity_id)
                .execute(&mut *tx)
                .await;

                // Delete from base entities table
                let _ = sqlx::query(r#"DELETE FROM "ob-poc".entities WHERE entity_id = $1"#)
                    .bind(entity_id)
                    .execute(&mut *tx)
                    .await;
            }

            entities_deleted = exclusive_entities.len() as i64;
        }

        // Delete cbu_entity_roles (always - removes role links even for preserved entities)
        let result = sqlx::query(r#"DELETE FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&mut *tx)
            .await?;
        deleted_counts.insert(
            "cbu_entity_roles".to_string(),
            result.rows_affected() as i64,
        );

        // =====================================================================
        // Phase 5: Delete the CBU itself
        // =====================================================================

        let result = sqlx::query(r#"DELETE FROM "ob-poc".cbus WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&mut *tx)
            .await?;
        deleted_counts.insert("cbus".to_string(), result.rows_affected() as i64);

        // Commit transaction
        tx.commit().await?;

        // Build summary
        let total_deleted: i64 = deleted_counts.values().sum();

        tracing::info!(
            cbu_id = %cbu_id,
            cbu_name = %cbu_name,
            total_deleted = total_deleted,
            entities_deleted = entities_deleted,
            entities_preserved = entities_preserved,
            "cbu.delete-cascade completed"
        );

        Ok(ExecutionResult::Record(serde_json::json!({
            "cbu_id": cbu_id,
            "cbu_name": cbu_name,
            "deleted": true,
            "total_records_deleted": total_deleted,
            "entities_deleted": entities_deleted,
            "entities_preserved_shared": entities_preserved,
            "by_table": deleted_counts
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "error": "Database required for cbu.delete-cascade"
        })))
    }
}

// ============================================================================
// CBU Create from Client Group
// ============================================================================

/// Entity info from client group query - includes GLEIF category and group role for mapping
#[derive(Debug)]
struct ClientGroupEntity {
    entity_id: Uuid,
    name: String,
    jurisdiction: Option<String>,
    gleif_category: Option<String>,
    group_role: Option<String>,
}

/// Create CBUs from entities in a client group with GLEIF category and role filters
///
/// Rationale: Bulk CBU creation from research results. Queries client_group_entity
/// with optional filters:
/// - `gleif-category`: Filter by GLEIF category (FUND, GENERAL) - recommended for fund onboarding
/// - `role-filter`: Filter by client group role (SUBSIDIARY, ULTIMATE_PARENT)
/// - `jurisdiction-filter`: Filter by entity jurisdiction
///
/// Maps GLEIF roles to CBU entity roles:
/// - FUND entities get ASSET_OWNER role (the fund owns its trading unit)
/// - ULTIMATE_PARENT entities get HOLDING_COMPANY role if added to CBU
/// - Optionally assigns MANAGEMENT_COMPANY and INVESTMENT_MANAGER from provided entity IDs
#[register_custom_op]
pub struct CbuCreateFromClientGroupOp;

#[async_trait]
impl CustomOperation for CbuCreateFromClientGroupOp {
    fn domain(&self) -> &'static str {
        "cbu"
    }
    fn verb(&self) -> &'static str {
        "create-from-client-group"
    }
    fn rationale(&self) -> &'static str {
        "Bulk CBU creation from client group entities - bridges research to onboarding with GLEIF role mapping"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let group_id = get_required_uuid(verb_call, "group-id")?;
        let gleif_category = extract_string_opt(verb_call, "gleif-category");
        let role_filter = extract_string_opt(verb_call, "role-filter");
        let jurisdiction_filter = extract_string_opt(verb_call, "jurisdiction-filter");
        let default_jurisdiction = extract_string_opt(verb_call, "default-jurisdiction")
            .unwrap_or_else(|| "LU".to_string());
        let manco_entity_id = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "manco-entity-id")
            .and_then(|a| a.value.as_uuid());
        let im_entity_id = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "im-entity-id")
            .and_then(|a| a.value.as_uuid());
        let limit = extract_int_opt(verb_call, "limit").unwrap_or(100) as i64;
        let dry_run = extract_bool_opt(verb_call, "dry-run").unwrap_or(false);

        // Build query to get entities from client group with GLEIF category and optional role filter
        // Always fetch gleif_category and group_role for mapping decisions
        let entities: Vec<ClientGroupEntity> =
            sqlx::query_as::<_, (Uuid, String, Option<String>, Option<String>, Option<String>)>(
                r#"
            SELECT DISTINCT
                e.entity_id,
                e.name,
                COALESCE(elc.jurisdiction, ef.jurisdiction) as jurisdiction,
                ef.gleif_category,
                (SELECT r.name FROM "ob-poc".client_group_entity_roles cger
                 JOIN "ob-poc".roles r ON r.role_id = cger.role_id
                 WHERE cger.cge_id = cge.id
                 LIMIT 1) as group_role
            FROM "ob-poc".client_group_entity cge
            JOIN "ob-poc".entities e ON e.entity_id = cge.entity_id
            LEFT JOIN "ob-poc".entity_limited_companies elc ON elc.entity_id = e.entity_id
            LEFT JOIN "ob-poc".entity_funds ef ON ef.entity_id = e.entity_id
            WHERE cge.group_id = $1
              AND cge.membership_type NOT IN ('historical', 'rejected')
              AND ($2::text IS NULL OR ef.gleif_category = $2)
              AND ($3::text IS NULL OR EXISTS (
                  SELECT 1 FROM "ob-poc".client_group_entity_roles cger2
                  JOIN "ob-poc".roles r2 ON r2.role_id = cger2.role_id
                  WHERE cger2.cge_id = cge.id AND r2.name = $3
              ))
              AND ($4::text IS NULL OR COALESCE(elc.jurisdiction, ef.jurisdiction) = $4)
            ORDER BY e.name
            LIMIT $5
            "#,
            )
            .bind(group_id)
            .bind(&gleif_category)
            .bind(&role_filter)
            .bind(&jurisdiction_filter)
            .bind(limit)
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(
                |(entity_id, name, jurisdiction, gleif_category, group_role)| ClientGroupEntity {
                    entity_id,
                    name,
                    jurisdiction,
                    gleif_category,
                    group_role,
                },
            )
            .collect();

        if dry_run {
            let entity_info: Vec<serde_json::Value> = entities
                .iter()
                .map(|ent| {
                    serde_json::json!({
                        "entity_id": ent.entity_id,
                        "name": ent.name,
                        "jurisdiction": ent.jurisdiction.as_deref().unwrap_or(&default_jurisdiction),
                        "gleif_category": ent.gleif_category,
                        "group_role": ent.group_role,
                        "cbu_role_mapping": map_to_cbu_role(&ent.gleif_category, &ent.group_role),
                    })
                })
                .collect();

            return Ok(ExecutionResult::Record(serde_json::json!({
                "dry_run": true,
                "group_id": group_id,
                "gleif_category": gleif_category,
                "role_filter": role_filter,
                "jurisdiction_filter": jurisdiction_filter,
                "entities_found": entities.len(),
                "entities": entity_info,
            })));
        }

        let mut cbus_created = 0i64;
        let mut roles_assigned = 0i64;
        let mut skipped_existing = 0i64;

        // Pre-fetch role IDs for CBU entity role assignment
        let role_ids = fetch_cbu_role_ids(pool).await?;

        for ent in entities {
            let jurisdiction = ent.jurisdiction.as_deref().unwrap_or(&default_jurisdiction);

            // Try to create CBU (upsert by name+jurisdiction)
            let result: (Uuid, bool) = sqlx::query_as(
                r#"
                INSERT INTO "ob-poc".cbus (name, jurisdiction, client_type)
                VALUES ($1, $2, 'FUND')
                ON CONFLICT (name, jurisdiction)
                DO UPDATE SET updated_at = NOW()
                RETURNING cbu_id, (xmax = 0) as is_insert
                "#,
            )
            .bind(&ent.name)
            .bind(jurisdiction)
            .fetch_one(pool)
            .await?;

            let (cbu_id, is_new) = result;

            if is_new {
                cbus_created += 1;
            } else {
                skipped_existing += 1;
            }

            // Map GLEIF category/role to CBU entity role and assign
            let cbu_role = map_to_cbu_role(&ent.gleif_category, &ent.group_role);
            if let Some(role_id) = role_ids.get(&cbu_role) {
                roles_assigned += assign_cbu_role(pool, cbu_id, ent.entity_id, *role_id).await?;
            }

            // Assign MANAGEMENT_COMPANY role if manco provided
            if let Some(manco_id) = manco_entity_id {
                if let Some(role_id) = role_ids.get("MANAGEMENT_COMPANY") {
                    roles_assigned += assign_cbu_role(pool, cbu_id, manco_id, *role_id).await?;
                }
            }

            // Assign INVESTMENT_MANAGER role if im provided
            if let Some(im_id) = im_entity_id {
                if let Some(role_id) = role_ids.get("INVESTMENT_MANAGER") {
                    roles_assigned += assign_cbu_role(pool, cbu_id, im_id, *role_id).await?;
                }
            }
        }

        Ok(ExecutionResult::Record(serde_json::json!({
            "group_id": group_id,
            "gleif_category": gleif_category,
            "role_filter": role_filter,
            "jurisdiction_filter": jurisdiction_filter,
            "cbus_created": cbus_created,
            "skipped_existing": skipped_existing,
            "roles_assigned": roles_assigned,
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "error": "Database required for cbu.create-from-client-group"
        })))
    }
}

/// Map GLEIF category and group role to the appropriate CBU entity role
///
/// Mapping logic:
/// - GLEIF category FUND → ASSET_OWNER (the fund entity owns its CBU)
/// - Group role ULTIMATE_PARENT → HOLDING_COMPANY
/// - Group role SUBSIDIARY with no GLEIF category → SUBSIDIARY (pass-through)
/// - Default → ASSET_OWNER (safe default for onboarding)
fn map_to_cbu_role(gleif_category: &Option<String>, group_role: &Option<String>) -> String {
    // GLEIF category takes precedence - FUND entities are asset owners
    if let Some(cat) = gleif_category {
        if cat.eq_ignore_ascii_case("FUND") {
            return "ASSET_OWNER".to_string();
        }
    }

    // Map corporate hierarchy roles
    if let Some(role) = group_role {
        match role.to_uppercase().as_str() {
            "ULTIMATE_PARENT" => return "HOLDING_COMPANY".to_string(),
            "SUBSIDIARY" => return "SUBSIDIARY".to_string(),
            _ => {}
        }
    }

    // Default to ASSET_OWNER for fund onboarding
    "ASSET_OWNER".to_string()
}

/// Pre-fetch commonly used CBU role IDs
#[cfg(feature = "database")]
async fn fetch_cbu_role_ids(pool: &PgPool) -> Result<std::collections::HashMap<String, Uuid>> {
    let roles: Vec<(String, Uuid)> = sqlx::query_as(
        r#"
        SELECT name, role_id FROM "ob-poc".roles
        WHERE name IN ('ASSET_OWNER', 'MANAGEMENT_COMPANY', 'INVESTMENT_MANAGER', 'HOLDING_COMPANY', 'SUBSIDIARY')
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(roles.into_iter().collect())
}

/// Assign a role to an entity on a CBU (idempotent via ON CONFLICT DO NOTHING)
#[cfg(feature = "database")]
async fn assign_cbu_role(
    pool: &PgPool,
    cbu_id: Uuid,
    entity_id: Uuid,
    role_id: Uuid,
) -> Result<i64> {
    let rows = sqlx::query(
        r#"
        INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id)
        VALUES ($1, $2, $3)
        ON CONFLICT (cbu_id, entity_id, role_id) DO NOTHING
        "#,
    )
    .bind(cbu_id)
    .bind(entity_id)
    .bind(role_id)
    .execute(pool)
    .await?
    .rows_affected();

    Ok(rows as i64)
}
