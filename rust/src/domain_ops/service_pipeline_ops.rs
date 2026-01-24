//! Service Resource Pipeline Operations
//!
//! DSL plugin handlers for the service intent → discovery → attribute →
//! provisioning → readiness pipeline.

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde_json::json;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

// =============================================================================
// Service Intent Operations
// =============================================================================

/// Create a service intent for a CBU
#[register_custom_op]
pub struct ServiceIntentCreateOp;

#[async_trait]
impl CustomOperation for ServiceIntentCreateOp {
    fn domain(&self) -> &'static str {
        "service-intent"
    }
    fn verb(&self) -> &'static str {
        "create"
    }
    fn rationale(&self) -> &'static str {
        "Creates service intent record linking CBU to product+service"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::service_resources::{NewServiceIntent, ServiceResourcePipelineService};
        use uuid::Uuid;

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

        let product_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "product-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing product-id argument"))?;

        let service_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "service-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing service-id argument"))?;

        // Handle options as map -> JSON
        let options: Option<serde_json::Value> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "options")
            .and_then(|a| {
                if let Some(map) = a.value.as_map() {
                    let json_map: serde_json::Map<String, serde_json::Value> = map
                        .iter()
                        .filter_map(|(k, v)| {
                            v.as_string()
                                .map(|s| (k.clone(), serde_json::Value::String(s.to_string())))
                        })
                        .collect();
                    Some(serde_json::Value::Object(json_map))
                } else {
                    None
                }
            });

        let service = ServiceResourcePipelineService::new(pool.clone());
        let input = NewServiceIntent {
            cbu_id,
            product_id,
            service_id,
            options,
            created_by: None,
        };

        let intent_id = service.create_service_intent(&input).await?;

        Ok(ExecutionResult::Uuid(intent_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("Database feature required"))
    }
}

/// List service intents for a CBU
#[register_custom_op]
pub struct ServiceIntentListOp;

#[async_trait]
impl CustomOperation for ServiceIntentListOp {
    fn domain(&self) -> &'static str {
        "service-intent"
    }
    fn verb(&self) -> &'static str {
        "list"
    }
    fn rationale(&self) -> &'static str {
        "Lists all service intents for a CBU with enrichment"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::service_resources::ServiceResourcePipelineService;
        use uuid::Uuid;

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

        let service = ServiceResourcePipelineService::new(pool.clone());
        let intents = service.get_service_intents(cbu_id).await?;

        Ok(ExecutionResult::RecordSet(
            intents
                .iter()
                .map(|i| {
                    json!({
                        "intent_id": i.intent_id,
                        "cbu_id": i.cbu_id,
                        "product_id": i.product_id,
                        "service_id": i.service_id,
                        "options": i.options,
                        "status": i.status,
                        "created_at": i.created_at.map(|dt| dt.to_rfc3339())
                    })
                })
                .collect(),
        ))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("Database feature required"))
    }
}

/// Supersede an existing service intent
#[register_custom_op]
pub struct ServiceIntentSupersedeOp;

#[async_trait]
impl CustomOperation for ServiceIntentSupersedeOp {
    fn domain(&self) -> &'static str {
        "service-intent"
    }
    fn verb(&self) -> &'static str {
        "supersede"
    }
    fn rationale(&self) -> &'static str {
        "Creates new intent version, marks old as superseded"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        let intent_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "intent-id")
            .and_then(|a| a.value.as_uuid())
            .ok_or_else(|| anyhow::anyhow!("Missing intent-id argument"))?;

        // Handle options as map -> JSON
        let options: serde_json::Value = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "options")
            .and_then(|a| {
                if let Some(map) = a.value.as_map() {
                    let json_map: serde_json::Map<String, serde_json::Value> = map
                        .iter()
                        .filter_map(|(k, v)| {
                            v.as_string()
                                .map(|s| (k.clone(), serde_json::Value::String(s.to_string())))
                        })
                        .collect();
                    Some(serde_json::Value::Object(json_map))
                } else {
                    None
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing options argument"))?;

        // Get the existing intent
        let existing = sqlx::query!(
            r#"SELECT cbu_id, product_id, service_id FROM "ob-poc".service_intents WHERE intent_id = $1"#,
            intent_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Intent not found: {}", intent_id))?;

        // Mark old as inactive (using status column instead of superseded_at)
        sqlx::query!(
            r#"UPDATE "ob-poc".service_intents SET status = 'superseded' WHERE intent_id = $1"#,
            intent_id
        )
        .execute(pool)
        .await?;

        // Create new intent
        let new_id: Uuid = sqlx::query_scalar!(
            r#"
            INSERT INTO "ob-poc".service_intents (cbu_id, product_id, service_id, options)
            VALUES ($1, $2, $3, $4)
            RETURNING intent_id
            "#,
            existing.cbu_id,
            existing.product_id,
            existing.service_id,
            options
        )
        .fetch_one(pool)
        .await?;

        Ok(ExecutionResult::Uuid(new_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("Database feature required"))
    }
}

// =============================================================================
// Discovery Operations
// =============================================================================

/// Run resource discovery for a CBU
#[register_custom_op]
pub struct DiscoveryRunOp;

#[async_trait]
impl CustomOperation for DiscoveryRunOp {
    fn domain(&self) -> &'static str {
        "discovery"
    }
    fn verb(&self) -> &'static str {
        "run"
    }
    fn rationale(&self) -> &'static str {
        "Orchestrates SRDEF discovery from service intents"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::service_resources::{load_srdefs_from_config, run_discovery_pipeline};
        use uuid::Uuid;

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

        let registry = load_srdefs_from_config().unwrap_or_default();
        let result = run_discovery_pipeline(pool, &registry, cbu_id).await?;

        Ok(ExecutionResult::Record(json!({
            "cbu_id": result.cbu_id,
            "srdefs_discovered": result.srdefs_discovered,
            "attrs_rolled_up": result.attrs_rolled_up,
            "attrs_populated": result.attrs_populated,
            "attrs_missing": result.attrs_missing
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("Database feature required"))
    }
}

/// Explain why SRDEFs were discovered
#[register_custom_op]
pub struct DiscoveryExplainOp;

#[async_trait]
impl CustomOperation for DiscoveryExplainOp {
    fn domain(&self) -> &'static str {
        "discovery"
    }
    fn verb(&self) -> &'static str {
        "explain"
    }
    fn rationale(&self) -> &'static str {
        "Returns discovery reasons for audit/debugging"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

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

        let srdef_filter = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "srdef-id")
            .and_then(|a| a.value.as_string());

        let reasons: Vec<DiscoveryReasonRow> = if let Some(srdef_id) = srdef_filter {
            sqlx::query_as(
                r#"
                SELECT srdef_id, service_id, trigger_type, reason_detail, discovered_at as created_at
                FROM "ob-poc".srdef_discovery_reasons
                WHERE cbu_id = $1 AND srdef_id = $2
                ORDER BY discovered_at DESC
                "#,
            )
            .bind(cbu_id)
            .bind(srdef_id)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT srdef_id, service_id, trigger_type, reason_detail, discovered_at as created_at
                FROM "ob-poc".srdef_discovery_reasons
                WHERE cbu_id = $1
                ORDER BY srdef_id, discovered_at DESC
                "#,
            )
            .bind(cbu_id)
            .fetch_all(pool)
            .await?
        };

        Ok(ExecutionResult::RecordSet(
            reasons
                .iter()
                .map(|r| {
                    json!({
                        "srdef_id": r.srdef_id,
                        "service_id": r.service_id,
                        "trigger_type": r.trigger_type,
                        "reason_detail": r.reason_detail,
                        "created_at": r.created_at.to_rfc3339()
                    })
                })
                .collect(),
        ))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("Database feature required"))
    }
}

#[derive(sqlx::FromRow)]
struct DiscoveryReasonRow {
    srdef_id: String,
    service_id: uuid::Uuid,
    trigger_type: String,
    reason_detail: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

// =============================================================================
// Attribute Operations
// =============================================================================

/// Roll up attribute requirements from discovered SRDEFs
#[register_custom_op]
pub struct AttributeRollupOp;

#[async_trait]
impl CustomOperation for AttributeRollupOp {
    fn domain(&self) -> &'static str {
        "attributes"
    }
    fn verb(&self) -> &'static str {
        "rollup"
    }
    fn rationale(&self) -> &'static str {
        "Merges attribute requirements from multiple SRDEFs"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::service_resources::AttributeRollupEngine;
        use uuid::Uuid;

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

        let engine = AttributeRollupEngine::new(pool);
        let result = engine.rollup_for_cbu(cbu_id).await?;

        Ok(ExecutionResult::Record(json!({
            "total_attributes": result.total_attributes,
            "required_count": result.required_count,
            "optional_count": result.optional_count,
            "conflict_count": result.conflict_count
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("Database feature required"))
    }
}

/// Auto-populate attribute values from available sources
#[register_custom_op]
pub struct AttributePopulateOp;

#[async_trait]
impl CustomOperation for AttributePopulateOp {
    fn domain(&self) -> &'static str {
        "attributes"
    }
    fn verb(&self) -> &'static str {
        "populate"
    }
    fn rationale(&self) -> &'static str {
        "Pulls values from entity, CBU, document sources"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::service_resources::PopulationEngine;
        use uuid::Uuid;

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

        let engine = PopulationEngine::new(pool);
        let result = engine.populate_for_cbu(cbu_id).await?;

        Ok(ExecutionResult::Record(json!({
            "populated": result.populated,
            "already_populated": result.already_populated,
            "still_missing": result.still_missing
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("Database feature required"))
    }
}

/// Show attribute gaps for a CBU
#[register_custom_op]
pub struct AttributeGapsOp;

#[async_trait]
impl CustomOperation for AttributeGapsOp {
    fn domain(&self) -> &'static str {
        "attributes"
    }
    fn verb(&self) -> &'static str {
        "gaps"
    }
    fn rationale(&self) -> &'static str {
        "Queries gap view for missing required attributes"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

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

        let gaps: Vec<AttrGapRow> = sqlx::query_as(
            r#"
            SELECT attr_id, attr_code, attr_name, attr_category, has_value
            FROM "ob-poc".v_cbu_attr_gaps
            WHERE cbu_id = $1 AND NOT has_value
            ORDER BY attr_category, attr_name
            "#,
        )
        .bind(cbu_id)
        .fetch_all(pool)
        .await?;

        Ok(ExecutionResult::RecordSet(
            gaps.iter()
                .map(|g| {
                    json!({
                        "attr_id": g.attr_id,
                        "attr_code": g.attr_code,
                        "attr_name": g.attr_name,
                        "attr_category": g.attr_category
                    })
                })
                .collect(),
        ))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("Database feature required"))
    }
}

#[derive(sqlx::FromRow)]
struct AttrGapRow {
    attr_id: uuid::Uuid,
    attr_code: String,
    attr_name: String,
    attr_category: String,
    #[allow(dead_code)]
    has_value: bool,
}

/// Set an attribute value manually
#[register_custom_op]
pub struct AttributeSetOp;

#[async_trait]
impl CustomOperation for AttributeSetOp {
    fn domain(&self) -> &'static str {
        "attributes"
    }
    fn verb(&self) -> &'static str {
        "set"
    }
    fn rationale(&self) -> &'static str {
        "Sets attribute value with evidence tracking"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::service_resources::{
            AttributeSource, ServiceResourcePipelineService, SetCbuAttrValue,
        };
        use uuid::Uuid;

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

        let attr_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "attr-id")
            .and_then(|a| a.value.as_uuid())
            .ok_or_else(|| anyhow::anyhow!("Missing attr-id argument"))?;

        let value = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "value")
            .and_then(|a| a.value.as_string())
            .map(|s| serde_json::Value::String(s.to_string()))
            .ok_or_else(|| anyhow::anyhow!("Missing value argument"))?;

        let service = ServiceResourcePipelineService::new(pool.clone());
        let input = SetCbuAttrValue {
            cbu_id,
            attr_id,
            value,
            source: AttributeSource::Manual,
            evidence_refs: None,
            explain_refs: None,
        };

        service.set_cbu_attr_value(&input).await?;

        Ok(ExecutionResult::Affected(1))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("Database feature required"))
    }
}

// =============================================================================
// Provisioning Operations
// =============================================================================

/// Run provisioning orchestrator
#[register_custom_op]
pub struct ProvisioningRunOp;

#[async_trait]
impl CustomOperation for ProvisioningRunOp {
    fn domain(&self) -> &'static str {
        "provisioning"
    }
    fn verb(&self) -> &'static str {
        "run"
    }
    fn rationale(&self) -> &'static str {
        "Orchestrates resource provisioning with dependency ordering"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::service_resources::{load_srdefs_from_config, run_provisioning_pipeline};
        use uuid::Uuid;

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

        let registry = load_srdefs_from_config().unwrap_or_default();
        let result = run_provisioning_pipeline(pool, &registry, cbu_id).await?;

        Ok(ExecutionResult::Record(json!({
            "cbu_id": result.cbu_id,
            "requests_created": result.requests_created,
            "already_active": result.already_active,
            "not_ready": result.not_ready,
            "services_ready": result.services_ready,
            "services_partial": result.services_partial,
            "services_blocked": result.services_blocked
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("Database feature required"))
    }
}

/// Check provisioning request status
#[register_custom_op]
pub struct ProvisioningStatusOp;

#[async_trait]
impl CustomOperation for ProvisioningStatusOp {
    fn domain(&self) -> &'static str {
        "provisioning"
    }
    fn verb(&self) -> &'static str {
        "status"
    }
    fn rationale(&self) -> &'static str {
        "Queries provisioning request with latest event"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        let request_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "request-id")
            .and_then(|a| a.value.as_uuid())
            .ok_or_else(|| anyhow::anyhow!("Missing request-id argument"))?;

        let request = sqlx::query!(
            r#"
            SELECT pr.request_id, pr.cbu_id, pr.srdef_id, pr.status, pr.requested_at,
                   pe.kind as event_kind, pe.occurred_at as event_at
            FROM "ob-poc".provisioning_requests pr
            LEFT JOIN "ob-poc".provisioning_events pe ON pr.request_id = pe.request_id
            WHERE pr.request_id = $1
            ORDER BY pe.occurred_at DESC NULLS LAST
            LIMIT 1
            "#,
            request_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Request not found: {}", request_id))?;

        Ok(ExecutionResult::Record(json!({
            "request_id": request.request_id,
            "cbu_id": request.cbu_id,
            "srdef_id": request.srdef_id,
            "status": request.status,
            "requested_at": request.requested_at,
            "latest_event": request.event_kind,
            "event_at": request.event_at
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("Database feature required"))
    }
}

// =============================================================================
// Readiness Operations
// =============================================================================

/// Compute service readiness
#[register_custom_op]
pub struct ReadinessComputeOp;

#[async_trait]
impl CustomOperation for ReadinessComputeOp {
    fn domain(&self) -> &'static str {
        "readiness"
    }
    fn verb(&self) -> &'static str {
        "compute"
    }
    fn rationale(&self) -> &'static str {
        "Computes 'good to transact' status per service"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::service_resources::{load_srdefs_from_config, ReadinessEngine};
        use uuid::Uuid;

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

        let registry = load_srdefs_from_config().unwrap_or_default();
        let engine = ReadinessEngine::new(pool, &registry);
        let result = engine.compute_for_cbu(cbu_id).await?;

        Ok(ExecutionResult::Record(json!({
            "total_services": result.total_services,
            "ready": result.ready,
            "partial": result.partial,
            "blocked": result.blocked
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("Database feature required"))
    }
}

/// Explain blocking reasons for services
#[register_custom_op]
pub struct ReadinessExplainOp;

#[async_trait]
impl CustomOperation for ReadinessExplainOp {
    fn domain(&self) -> &'static str {
        "readiness"
    }
    fn verb(&self) -> &'static str {
        "explain"
    }
    fn rationale(&self) -> &'static str {
        "Returns blocking reasons for debugging/remediation"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::service_resources::ServiceResourcePipelineService;
        use uuid::Uuid;

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

        let service_filter: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "service-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });

        let service = ServiceResourcePipelineService::new(pool.clone());
        let readiness = service.get_service_readiness(cbu_id).await?;

        let blocking: Vec<_> = readiness
            .into_iter()
            .filter(|r| service_filter.is_none_or(|sid| r.service_id == sid))
            .filter(|r| r.status != "ready")
            .map(|r| {
                json!({
                    "service_id": r.service_id,
                    "product_id": r.product_id,
                    "status": r.status,
                    "blocking_reasons": r.blocking_reasons
                })
            })
            .collect();

        Ok(ExecutionResult::RecordSet(blocking))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("Database feature required"))
    }
}

// =============================================================================
// Full Pipeline Operation
// =============================================================================

/// Run the entire service resource pipeline
#[register_custom_op]
pub struct PipelineFullOp;

#[async_trait]
impl CustomOperation for PipelineFullOp {
    fn domain(&self) -> &'static str {
        "pipeline"
    }
    fn verb(&self) -> &'static str {
        "full"
    }
    fn rationale(&self) -> &'static str {
        "Orchestrates complete pipeline: discovery → provision → readiness"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::service_resources::{
            load_srdefs_from_config, run_discovery_pipeline, run_provisioning_pipeline,
        };
        use uuid::Uuid;

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

        let registry = load_srdefs_from_config().unwrap_or_default();

        // Run discovery + rollup + populate
        let discovery = run_discovery_pipeline(pool, &registry, cbu_id).await?;

        // Run provisioning + readiness
        let provisioning = run_provisioning_pipeline(pool, &registry, cbu_id).await?;

        Ok(ExecutionResult::Record(json!({
            "cbu_id": cbu_id,
            "discovery": {
                "srdefs_discovered": discovery.srdefs_discovered,
                "attrs_rolled_up": discovery.attrs_rolled_up,
                "attrs_populated": discovery.attrs_populated,
                "attrs_missing": discovery.attrs_missing
            },
            "provisioning": {
                "requests_created": provisioning.requests_created,
                "already_active": provisioning.already_active,
                "not_ready": provisioning.not_ready
            },
            "readiness": {
                "services_ready": provisioning.services_ready,
                "services_partial": provisioning.services_partial,
                "services_blocked": provisioning.services_blocked
            }
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("Database feature required"))
    }
}
