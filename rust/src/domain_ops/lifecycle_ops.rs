//! Lifecycle resource instance custom operations
//!
//! Operations for instrument lifecycle resource provisioning, gap analysis,
//! and readiness checking. Parallel to service-resource operations but for
//! the Instrument → Lifecycle → Resource taxonomy.

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;

use super::CustomOperation;

#[cfg(feature = "database")]
use sqlx::PgPool;

// ============================================================================
// PROVISION OPERATION
// ============================================================================

/// Provision a lifecycle resource instance for a CBU
///
/// Rationale: Requires lookup of resource_type_id from lifecycle_resource_types,
/// context scoping (market, currency, counterparty), and auto-generation of
/// instance URLs for dependency tracking.
#[register_custom_op]
pub struct LifecycleProvisionOp;

#[async_trait]
impl CustomOperation for LifecycleProvisionOp {
    fn domain(&self) -> &'static str {
        "lifecycle"
    }
    fn verb(&self) -> &'static str {
        "provision"
    }
    fn rationale(&self) -> &'static str {
        "Requires resource_type lookup, context scoping, and instance URL generation"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use uuid::Uuid;

        // Get CBU ID (required)
        let cbu_id: Uuid = super::helpers::json_extract_uuid(args, ctx, "cbu-id")?;

        // Get resource type code (required)
        let resource_type_code = super::helpers::json_extract_string(args, "resource-type")?;

        // Get resource type with context requirements
        let rt: Option<(Uuid, bool, bool, bool)> = sqlx::query_as(
            r#"SELECT resource_type_id, per_market, per_currency, per_counterparty
               FROM "ob-poc".lifecycle_resource_types WHERE code = $1"#,
        )
        .bind(&resource_type_code)
        .fetch_optional(pool)
        .await?;

        let (resource_type_id, per_market, _per_currency, per_counterparty) =
            rt.ok_or_else(|| {
                anyhow::anyhow!("Unknown lifecycle resource type: {}", resource_type_code)
            })?;

        // Get optional context arguments
        // Market may be provided as UUID/@symbol or as MIC string
        let market_id: Option<Uuid> = super::helpers::json_extract_uuid_opt(args, ctx, "market");

        // If market was provided as MIC string, look it up
        let market_id: Option<Uuid> = if market_id.is_none() {
            if let Some(mic) = super::helpers::json_extract_string_opt(args, "market") {
                sqlx::query_scalar(r#"SELECT market_id FROM "ob-poc".markets WHERE mic = $1"#)
                    .bind(mic)
                    .fetch_optional(pool)
                    .await?
            } else {
                None
            }
        } else {
            market_id
        };

        let currency: Option<String> = super::helpers::json_extract_string_opt(args, "currency");

        let counterparty_id: Option<Uuid> =
            super::helpers::json_extract_uuid_opt(args, ctx, "counterparty");

        // Validate context requirements
        if per_market && market_id.is_none() {
            return Err(anyhow::anyhow!(
                "market is required for resource type {}",
                resource_type_code
            ));
        }
        if per_counterparty && counterparty_id.is_none() {
            return Err(anyhow::anyhow!(
                "counterparty is required for resource type {}",
                resource_type_code
            ));
        }

        // Get optional provider details
        let provider_code = super::helpers::json_extract_string_opt(args, "provider");

        let provider_account = super::helpers::json_extract_string_opt(args, "provider-account");

        let provider_bic = super::helpers::json_extract_string_opt(args, "provider-bic");

        let config: Option<serde_json::Value> = args
            .get("config")
            .and_then(|v| if v.is_object() { Some(v.clone()) } else { None });

        // Generate instance URL (unique identifier for this resource)
        let context_suffix = market_id
            .map(|m| m.to_string())
            .or_else(|| counterparty_id.map(|c| c.to_string()))
            .or_else(|| currency.clone())
            .unwrap_or_else(|| "default".to_string());

        let instance_url = format!(
            "cbu:{}/lifecycle/{}/{}",
            cbu_id, resource_type_code, context_suffix
        );

        // Get instance identifier if provided
        let instance_identifier =
            super::helpers::json_extract_string_opt(args, "instance-identifier");

        // Idempotent upsert
        let instance_id = uuid::Uuid::new_v4();

        let row: (Uuid,) = sqlx::query_as(
            r#"WITH ins AS (
                INSERT INTO "ob-poc".cbu_lifecycle_instances
                (instance_id, cbu_id, resource_type_id, instance_identifier, instance_url,
                 market_id, currency, counterparty_entity_id, status,
                 provider_code, provider_account, provider_bic, config, provisioned_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'PROVISIONED', $9, $10, $11, $12, NOW())
                ON CONFLICT (instance_url) DO UPDATE SET
                    provider_code = COALESCE(EXCLUDED.provider_code, cbu_lifecycle_instances.provider_code),
                    provider_account = COALESCE(EXCLUDED.provider_account, cbu_lifecycle_instances.provider_account),
                    provider_bic = COALESCE(EXCLUDED.provider_bic, cbu_lifecycle_instances.provider_bic),
                    config = COALESCE(EXCLUDED.config, cbu_lifecycle_instances.config),
                    updated_at = NOW()
                RETURNING instance_id
            )
            SELECT instance_id FROM ins
            UNION ALL
            SELECT instance_id FROM "ob-poc".cbu_lifecycle_instances
            WHERE instance_url = $5
            AND NOT EXISTS (SELECT 1 FROM ins)
            LIMIT 1"#,
        )
        .bind(instance_id)
        .bind(cbu_id)
        .bind(resource_type_id)
        .bind(&instance_identifier)
        .bind(&instance_url)
        .bind(market_id)
        .bind(&currency)
        .bind(counterparty_id)
        .bind(&provider_code)
        .bind(&provider_account)
        .bind(&provider_bic)
        .bind(&config)
        .fetch_one(pool)
        .await?;

        let result_id = row.0;
        ctx.bind("instance", result_id);

        Ok(dsl_runtime::VerbExecutionOutcome::Record(serde_json::json!({
            "instance_id": result_id,
            "instance_url": instance_url,
            "status": "PROVISIONED"
        })))
    }
    fn is_migrated(&self) -> bool {
        true
    }
}


// ============================================================================
// GAP ANALYSIS OPERATION
// ============================================================================

/// Analyze lifecycle provisioning gaps for a CBU
///
/// Rationale: Complex query against v_cbu_lifecycle_gaps view that identifies
/// missing lifecycle resources based on the CBU's instrument universe.
#[register_custom_op]
pub struct LifecycleAnalyzeGapsOp;

#[async_trait]
impl CustomOperation for LifecycleAnalyzeGapsOp {
    fn domain(&self) -> &'static str {
        "lifecycle"
    }
    fn verb(&self) -> &'static str {
        "analyze-gaps"
    }
    fn rationale(&self) -> &'static str {
        "Complex gap analysis query against view joining universe, lifecycles, and instances"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use uuid::Uuid;

        let cbu_id: Uuid = super::helpers::json_extract_uuid(args, ctx, "cbu-id")?;

        // Query the gap view
        let gaps: Vec<(
            Uuid,           // cbu_id
            String,         // cbu_name
            String,         // instrument_class
            Option<String>, // market
            Option<String>, // counterparty_name
            String,         // lifecycle_code
            String,         // lifecycle_name
            bool,           // is_mandatory
            String,         // missing_resource_code
            String,         // missing_resource_name
            Option<String>, // provisioning_verb
            Option<String>, // location_type
            bool,           // per_market
            bool,           // per_currency
            bool,           // per_counterparty
        )> = sqlx::query_as(
            r#"SELECT
                cbu_id, cbu_name, instrument_class, market, counterparty_name,
                lifecycle_code, lifecycle_name, is_mandatory,
                missing_resource_code, missing_resource_name, provisioning_verb,
                location_type, per_market, per_currency, per_counterparty
               FROM "ob-poc".v_cbu_lifecycle_gaps
               WHERE cbu_id = $1
               ORDER BY instrument_class, lifecycle_code, missing_resource_code"#,
        )
        .bind(cbu_id)
        .fetch_all(pool)
        .await?;

        let result: Vec<serde_json::Value> = gaps
            .into_iter()
            .map(|g| {
                serde_json::json!({
                    "cbu_id": g.0,
                    "cbu_name": g.1,
                    "instrument_class": g.2,
                    "market": g.3,
                    "counterparty_name": g.4,
                    "lifecycle_code": g.5,
                    "lifecycle_name": g.6,
                    "is_mandatory": g.7,
                    "missing_resource_code": g.8,
                    "missing_resource_name": g.9,
                    "provisioning_verb": g.10,
                    "location_type": g.11,
                    "per_market": g.12,
                    "per_currency": g.13,
                    "per_counterparty": g.14
                })
            })
            .collect();

        Ok(dsl_runtime::VerbExecutionOutcome::RecordSet(result))
    }
    fn is_migrated(&self) -> bool {
        true
    }
}


// ============================================================================
// CHECK READINESS OPERATION
// ============================================================================

/// Check if CBU is ready to trade an instrument
///
/// Rationale: Combines gap analysis with readiness decision logic,
/// separating blocking gaps from warnings.
#[register_custom_op]
pub struct LifecycleCheckReadinessOp;

#[async_trait]
impl CustomOperation for LifecycleCheckReadinessOp {
    fn domain(&self) -> &'static str {
        "lifecycle"
    }
    fn verb(&self) -> &'static str {
        "check-readiness"
    }
    fn rationale(&self) -> &'static str {
        "Combines gap analysis with blocking/warning classification"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use uuid::Uuid;

        let cbu_id: Uuid = super::helpers::json_extract_uuid(args, ctx, "cbu-id")?;

        let instrument_class = super::helpers::json_extract_string(args, "instrument-class")?;

        let market: Option<String> = super::helpers::json_extract_string_opt(args, "market");

        // Get gaps for this specific instrument/context
        let gaps: Vec<(String, String, bool, String)> = sqlx::query_as(
            r#"SELECT lifecycle_code, missing_resource_code, is_mandatory, missing_resource_name
               FROM "ob-poc".v_cbu_lifecycle_gaps
               WHERE cbu_id = $1
                 AND instrument_class = $2
                 AND ($3::text IS NULL OR market = $3)"#,
        )
        .bind(cbu_id)
        .bind(&instrument_class)
        .bind(market.as_deref())
        .fetch_all(pool)
        .await?;

        let blocking_gaps: Vec<serde_json::Value> = gaps
            .iter()
            .filter(|g| g.2) // is_mandatory
            .map(|g| {
                serde_json::json!({
                    "lifecycle": g.0,
                    "resource_code": g.1,
                    "resource_name": g.3
                })
            })
            .collect();

        let warnings: Vec<serde_json::Value> = gaps
            .iter()
            .filter(|g| !g.2) // not mandatory
            .map(|g| {
                serde_json::json!({
                    "lifecycle": g.0,
                    "resource_code": g.1,
                    "resource_name": g.3
                })
            })
            .collect();

        Ok(dsl_runtime::VerbExecutionOutcome::Record(serde_json::json!({
            "ready": blocking_gaps.is_empty(),
            "instrument_class": instrument_class,
            "market": market,
            "blocking_gaps": blocking_gaps,
            "warnings": warnings
        })))
    }
    fn is_migrated(&self) -> bool {
        true
    }
}


// ============================================================================
// DISCOVER OPERATION
// ============================================================================

/// Discover all lifecycles and resources for an instrument class
///
/// Rationale: Multi-join query to discover the full lifecycle tree for
/// an instrument class, including ISDA requirements.
#[register_custom_op]
pub struct LifecycleDiscoverOp;

#[async_trait]
impl CustomOperation for LifecycleDiscoverOp {
    fn domain(&self) -> &'static str {
        "lifecycle"
    }
    fn verb(&self) -> &'static str {
        "discover"
    }
    fn rationale(&self) -> &'static str {
        "Complex multi-join query for lifecycle/resource discovery"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let instrument_class = super::helpers::json_extract_string(args, "instrument-class")?;

        let include_optional = super::helpers::json_extract_bool_opt(args, "include-optional")
            .unwrap_or(false);

        // Get lifecycles for instrument class
        let lifecycles: Vec<(String, String, bool, bool)> = sqlx::query_as(
            r#"SELECT l.code, l.name, il.is_mandatory, il.requires_isda
               FROM "ob-poc".lifecycles l
               JOIN "ob-poc".instrument_lifecycles il ON il.lifecycle_id = l.lifecycle_id
               JOIN "ob-poc".instrument_classes ic ON ic.class_id = il.instrument_class_id
               WHERE ic.code = $1
                 AND il.is_active = true
                 AND l.is_active = true
                 AND ($2 OR il.is_mandatory = true)
               ORDER BY il.display_order"#,
        )
        .bind(&instrument_class)
        .bind(include_optional)
        .fetch_all(pool)
        .await?;

        let requires_isda = lifecycles.iter().any(|(_, _, _, r)| *r);

        let mut mandatory = Vec::new();
        let mut optional = Vec::new();

        for (code, name, is_mandatory, _) in &lifecycles {
            // Get resources for this lifecycle
            let resources: Vec<(String, String, bool)> = sqlx::query_as(
                r#"SELECT lrt.code, lrt.name, lrc.is_required
                   FROM "ob-poc".lifecycle_resource_types lrt
                   JOIN "ob-poc".lifecycle_resource_capabilities lrc
                     ON lrc.resource_type_id = lrt.resource_type_id
                   JOIN "ob-poc".lifecycles l ON l.lifecycle_id = lrc.lifecycle_id
                   WHERE l.code = $1
                     AND lrt.is_active = true
                     AND lrc.is_active = true
                   ORDER BY lrc.priority"#,
            )
            .bind(code)
            .fetch_all(pool)
            .await?;

            let entry = serde_json::json!({
                "code": code,
                "name": name,
                "resources": resources.iter().map(|(c, n, r)| {
                    serde_json::json!({"code": c, "name": n, "required": r})
                }).collect::<Vec<_>>()
            });

            if *is_mandatory {
                mandatory.push(entry);
            } else {
                optional.push(entry);
            }
        }

        Ok(dsl_runtime::VerbExecutionOutcome::Record(serde_json::json!({
            "instrument_class": instrument_class,
            "requires_isda": requires_isda,
            "mandatory_lifecycles": mandatory,
            "optional_lifecycles": optional
        })))
    }
    fn is_migrated(&self) -> bool {
        true
    }
}


// ============================================================================
// GENERATE PLAN OPERATION
// ============================================================================

/// Generate DSL provisioning plan for lifecycle gaps
///
/// Rationale: Generates DSL statements from gaps, incorporating user responses
/// to prompts for provider selection.
#[register_custom_op]
pub struct LifecycleGeneratePlanOp;

#[async_trait]
impl CustomOperation for LifecycleGeneratePlanOp {
    fn domain(&self) -> &'static str {
        "lifecycle"
    }
    fn verb(&self) -> &'static str {
        "generate-plan"
    }
    fn rationale(&self) -> &'static str {
        "Generates DSL statements from gap analysis with user response handling"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use uuid::Uuid;

        let cbu_id: Uuid = super::helpers::json_extract_uuid(args, ctx, "cbu-id")?;

        let user_responses: serde_json::Value = args
            .get("user-responses")
            .and_then(|v| if v.is_object() { Some(v.clone()) } else { None })
            .unwrap_or_else(|| serde_json::json!({}));

        // Get gaps
        let gaps: Vec<(
            String,         // instrument_class
            Option<String>, // market
            Option<String>, // counterparty_name
            String,         // lifecycle_code
            bool,           // is_mandatory
            String,         // missing_resource_code
            String,         // missing_resource_name
            Option<String>, // provisioning_verb
            Option<String>, // location_type
            bool,           // per_market
            bool,           // per_currency
            bool,           // per_counterparty
        )> = sqlx::query_as(
            r#"SELECT instrument_class, market, counterparty_name, lifecycle_code,
                      is_mandatory, missing_resource_code, missing_resource_name,
                      provisioning_verb, location_type, per_market, per_currency, per_counterparty
               FROM "ob-poc".v_cbu_lifecycle_gaps
               WHERE cbu_id = $1"#,
        )
        .bind(cbu_id)
        .fetch_all(pool)
        .await?;

        let mut dsl_statements = Vec::new();
        let mut pending_prompts = Vec::new();

        for gap in gaps {
            let resource_code = &gap.5;

            if let Some(verb) = &gap.7 {
                // Check if we have user response for this resource
                let has_response = user_responses.get(resource_code).is_some();

                if has_response {
                    // Generate DSL statement
                    let mut stmt = format!(
                        "({} :cbu-id \"{}\" :resource-type \"{}\"",
                        verb, cbu_id, resource_code
                    );

                    if let Some(market) = &gap.1 {
                        stmt.push_str(&format!(" :market \"{}\"", market));
                    }
                    if let Some(counterparty) = &gap.2 {
                        stmt.push_str(&format!(" :counterparty \"{}\"", counterparty));
                    }

                    // Add user-provided values
                    if let Some(resp) = user_responses.get(resource_code) {
                        if let Some(provider) = resp.get("provider").and_then(|v| v.as_str()) {
                            stmt.push_str(&format!(" :provider \"{}\"", provider));
                        }
                        if let Some(bic) = resp.get("bic").and_then(|v| v.as_str()) {
                            stmt.push_str(&format!(" :provider-bic \"{}\"", bic));
                        }
                    }

                    stmt.push(')');
                    dsl_statements.push(stmt);
                } else {
                    // Add to pending prompts
                    pending_prompts.push(serde_json::json!({
                        "resource_code": resource_code,
                        "resource_name": gap.6,
                        "instrument_class": gap.0,
                        "market": gap.1,
                        "counterparty": gap.2,
                        "location_type": gap.8,
                        "prompt": format!(
                            "For {} in {}: Which provider for {}?",
                            gap.0,
                            gap.1.as_deref().or(gap.2.as_deref()).unwrap_or("your setup"),
                            gap.6
                        )
                    }));
                }
            }
        }

        Ok(dsl_runtime::VerbExecutionOutcome::Record(serde_json::json!({
            "dsl_statements": dsl_statements,
            "pending_prompts": pending_prompts,
            "ready_to_execute": pending_prompts.is_empty()
        })))
    }
    fn is_migrated(&self) -> bool {
        true
    }
}


// ============================================================================
// EXECUTE PLAN OPERATION
// ============================================================================

/// Execute a provisioning plan
///
/// Rationale: Executes generated DSL statements, with dry-run support.
#[register_custom_op]
pub struct LifecycleExecutePlanOp;

#[async_trait]
impl CustomOperation for LifecycleExecutePlanOp {
    fn domain(&self) -> &'static str {
        "lifecycle"
    }
    fn verb(&self) -> &'static str {
        "execute-plan"
    }
    fn rationale(&self) -> &'static str {
        "Executes DSL plan with dry-run support and result aggregation"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let dry_run = super::helpers::json_extract_bool_opt(args, "dry-run").unwrap_or(false);

        let plan = args
            .get("plan")
            .and_then(|v| if v.is_object() { Some(v.clone()) } else { None })
            .ok_or_else(|| anyhow::anyhow!("Missing plan argument"))?;

        let statements = plan
            .get("dsl_statements")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("dsl_statements required in plan"))?;

        let mut results = Vec::new();

        for stmt in statements {
            let dsl = stmt.as_str().unwrap_or("");

            if dry_run {
                results.push(serde_json::json!({
                    "statement": dsl,
                    "status": "would_execute"
                }));
            } else {
                // TODO: Execute DSL via DslExecutor
                // For now, just mark as pending
                results.push(serde_json::json!({
                    "statement": dsl,
                    "status": "pending_execution",
                    "note": "DSL execution integration pending"
                }));
            }
        }

        Ok(dsl_runtime::VerbExecutionOutcome::Record(serde_json::json!({
            "dry_run": dry_run,
            "executed": results.len(),
            "results": results
        })))
    }
    fn is_migrated(&self) -> bool {
        true
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lifecycle_provision_op_metadata() {
        let op = LifecycleProvisionOp;
        assert_eq!(op.domain(), "lifecycle");
        assert_eq!(op.verb(), "provision");
    }

    #[test]
    fn test_lifecycle_analyze_gaps_op_metadata() {
        let op = LifecycleAnalyzeGapsOp;
        assert_eq!(op.domain(), "lifecycle");
        assert_eq!(op.verb(), "analyze-gaps");
    }

    #[test]
    fn test_lifecycle_check_readiness_op_metadata() {
        let op = LifecycleCheckReadinessOp;
        assert_eq!(op.domain(), "lifecycle");
        assert_eq!(op.verb(), "check-readiness");
    }

    #[test]
    fn test_lifecycle_discover_op_metadata() {
        let op = LifecycleDiscoverOp;
        assert_eq!(op.domain(), "lifecycle");
        assert_eq!(op.verb(), "discover");
    }

    #[test]
    fn test_lifecycle_generate_plan_op_metadata() {
        let op = LifecycleGeneratePlanOp;
        assert_eq!(op.domain(), "lifecycle");
        assert_eq!(op.verb(), "generate-plan");
    }

    #[test]
    fn test_lifecycle_execute_plan_op_metadata() {
        let op = LifecycleExecutePlanOp;
        assert_eq!(op.domain(), "lifecycle");
        assert_eq!(op.verb(), "execute-plan");
    }
}

/// Compatibility alias for `service-resource.provision-lifecycle`.
#[register_custom_op]
pub struct ServiceResourceProvisionLifecycleOp;

#[async_trait]
impl CustomOperation for ServiceResourceProvisionLifecycleOp {
    fn domain(&self) -> &'static str {
        "service-resource"
    }
    fn verb(&self) -> &'static str {
        "provision-lifecycle"
    }
    fn rationale(&self) -> &'static str {
        "Compatibility alias for lifecycle.provision"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        LifecycleProvisionOp.execute_json(args, ctx, pool).await
    }
    fn is_migrated(&self) -> bool {
        true
    }
}


/// Compatibility alias for `service-resource.analyze-lifecycle-gaps`.
#[register_custom_op]
pub struct ServiceResourceAnalyzeLifecycleGapsOp;

#[async_trait]
impl CustomOperation for ServiceResourceAnalyzeLifecycleGapsOp {
    fn domain(&self) -> &'static str {
        "service-resource"
    }
    fn verb(&self) -> &'static str {
        "analyze-lifecycle-gaps"
    }
    fn rationale(&self) -> &'static str {
        "Compatibility alias for lifecycle.analyze-gaps"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        LifecycleAnalyzeGapsOp.execute_json(args, ctx, pool).await
    }
    fn is_migrated(&self) -> bool {
        true
    }
}


/// Compatibility alias for `service-resource.check-lifecycle-readiness`.
#[register_custom_op]
pub struct ServiceResourceCheckLifecycleReadinessOp;

#[async_trait]
impl CustomOperation for ServiceResourceCheckLifecycleReadinessOp {
    fn domain(&self) -> &'static str {
        "service-resource"
    }
    fn verb(&self) -> &'static str {
        "check-lifecycle-readiness"
    }
    fn rationale(&self) -> &'static str {
        "Compatibility alias for lifecycle.check-readiness"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        LifecycleCheckReadinessOp
            .execute_json(args, ctx, pool)
            .await
    }
    fn is_migrated(&self) -> bool {
        true
    }
}


/// Compatibility alias for `service-resource.discover-lifecycles`.
#[register_custom_op]
pub struct ServiceResourceDiscoverLifecyclesOp;

#[async_trait]
impl CustomOperation for ServiceResourceDiscoverLifecyclesOp {
    fn domain(&self) -> &'static str {
        "service-resource"
    }
    fn verb(&self) -> &'static str {
        "discover-lifecycles"
    }
    fn rationale(&self) -> &'static str {
        "Compatibility alias for lifecycle.discover"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        LifecycleDiscoverOp.execute_json(args, ctx, pool).await
    }
    fn is_migrated(&self) -> bool {
        true
    }
}


/// Compatibility alias for `service-resource.generate-lifecycle-plan`.
#[register_custom_op]
pub struct ServiceResourceGenerateLifecyclePlanOp;

#[async_trait]
impl CustomOperation for ServiceResourceGenerateLifecyclePlanOp {
    fn domain(&self) -> &'static str {
        "service-resource"
    }
    fn verb(&self) -> &'static str {
        "generate-lifecycle-plan"
    }
    fn rationale(&self) -> &'static str {
        "Compatibility alias for lifecycle.generate-plan"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        LifecycleGeneratePlanOp.execute_json(args, ctx, pool).await
    }
    fn is_migrated(&self) -> bool {
        true
    }
}


/// Compatibility alias for `service-resource.execute-lifecycle-plan`.
#[register_custom_op]
pub struct ServiceResourceExecuteLifecyclePlanOp;

#[async_trait]
impl CustomOperation for ServiceResourceExecuteLifecyclePlanOp {
    fn domain(&self) -> &'static str {
        "service-resource"
    }
    fn verb(&self) -> &'static str {
        "execute-lifecycle-plan"
    }
    fn rationale(&self) -> &'static str {
        "Compatibility alias for lifecycle.execute-plan"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        LifecycleExecutePlanOp.execute_json(args, ctx, pool).await
    }
    fn is_migrated(&self) -> bool {
        true
    }
}
