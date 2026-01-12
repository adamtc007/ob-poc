//! UBO (Ultimate Beneficial Owner) Analysis Operations
//!
//! Enhanced UBO operations for discovery, chain tracing, snapshots, and comparisons.
//! These extend the basic UBO CRUD operations with complex graph traversal and analysis.

use anyhow::Result;
use async_trait::async_trait;

use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::custom_ops::CustomOperation;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

/// Calculate UBO (Ultimate Beneficial Ownership) chain
///
/// Rationale: Requires recursive graph traversal through ownership chains
/// to identify beneficial owners above the specified threshold.
pub struct UboCalculateOp;

#[async_trait]
impl CustomOperation for UboCalculateOp {
    fn domain(&self) -> &'static str {
        "ubo"
    }
    fn verb(&self) -> &'static str {
        "calculate"
    }
    fn rationale(&self) -> &'static str {
        "Requires recursive graph traversal through ownership hierarchy"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use serde_json::json;
        use uuid::Uuid;

        // Get CBU ID
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

        // Get threshold (default 25%)
        let threshold: f64 = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "threshold")
            .and_then(|a| a.value.as_decimal())
            .map(|d| d.to_string().parse().unwrap_or(25.0))
            .unwrap_or(25.0);

        // First get the primary entity for this CBU
        let cbu_entity = sqlx::query!(
            r#"
            SELECT e.entity_id
            FROM "ob-poc".cbu_entity_roles cer
            JOIN "ob-poc".entities e ON e.entity_id = cer.entity_id
            JOIN "ob-poc".roles r ON r.role_id = cer.role_id
            WHERE cer.cbu_id = $1
            AND r.name IN ('Primary Entity', 'Main Entity', 'Client')
            LIMIT 1
            "#,
            cbu_id
        )
        .fetch_optional(pool)
        .await?;

        let target_entity_id = match cbu_entity {
            Some(row) => row.entity_id,
            None => {
                // No primary entity found, return empty result
                return Ok(ExecutionResult::RecordSet(vec![]));
            }
        };

        // Query ownership structure using recursive CTE through entity_relationships
        let ubos = sqlx::query!(
            r#"
            WITH RECURSIVE ownership_chain AS (
                -- Base case: direct owners of the target entity
                SELECT
                    r.from_entity_id as entity_id,
                    r.percentage as ownership_percent,
                    ARRAY[r.from_entity_id] as path,
                    1 as depth
                FROM "ob-poc".entity_relationships r
                WHERE r.to_entity_id = $1
                AND r.relationship_type = 'ownership'
                AND r.ownership_type IN ('DIRECT', 'BENEFICIAL', 'direct', 'beneficial')
                AND (r.effective_to IS NULL OR r.effective_to > CURRENT_DATE)

                UNION ALL

                -- Recursive case: owners of owners
                SELECT
                    r2.from_entity_id as entity_id,
                    (oc.ownership_percent * r2.percentage / 100)::numeric(5,2) as ownership_percent,
                    oc.path || r2.from_entity_id,
                    oc.depth + 1
                FROM ownership_chain oc
                JOIN "ob-poc".entity_relationships r2 ON r2.to_entity_id = oc.entity_id
                WHERE oc.depth < 10
                AND r2.relationship_type = 'ownership'
                AND NOT r2.from_entity_id = ANY(oc.path)
                AND (r2.effective_to IS NULL OR r2.effective_to > CURRENT_DATE)
            )
            SELECT
                entity_id,
                SUM(ownership_percent) as total_ownership
            FROM ownership_chain
            GROUP BY entity_id
            HAVING SUM(ownership_percent) >= $2
            ORDER BY total_ownership DESC
            "#,
            target_entity_id,
            sqlx::types::BigDecimal::try_from(threshold).ok()
        )
        .fetch_all(pool)
        .await?;

        // Build result
        let ubo_list: Vec<serde_json::Value> = ubos
            .iter()
            .map(|row| {
                json!({
                    "entity_id": row.entity_id,
                    "ownership_percent": row.total_ownership
                })
            })
            .collect();

        Ok(ExecutionResult::RecordSet(ubo_list))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::RecordSet(vec![]))
    }
}

/// Discover potential UBOs from document extraction or registry lookup
///
/// Rationale: Orchestrates document extraction and registry lookups to identify
/// potential beneficial owners, creating preliminary ownership records.
pub struct UboTraceChainsOp;

#[async_trait]
impl CustomOperation for UboTraceChainsOp {
    fn domain(&self) -> &'static str {
        "ubo"
    }
    fn verb(&self) -> &'static str {
        "trace-chains"
    }
    fn rationale(&self) -> &'static str {
        "Calls SQL recursive function to compute ownership chains"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use serde_json::json;
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

        let target_entity_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "target-entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });

        let threshold: f64 = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "threshold")
            .and_then(|a| a.value.as_decimal())
            .map(|d| d.to_string().parse().unwrap_or(25.0))
            .unwrap_or(25.0);

        // Get as-of-date (optional, defaults to today)
        let as_of_date: chrono::NaiveDate = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "as-of-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());

        // Convert f64 to BigDecimal via string
        // Default 25.0 is a valid BigDecimal constant - use from_str_radix for safety
        let threshold_bd: sqlx::types::BigDecimal = threshold
            .to_string()
            .parse()
            .unwrap_or_else(|_| sqlx::types::BigDecimal::from(25));

        // Call the SQL function with as_of_date - includes both ownership AND control relationships
        // Control relationships are UBO extensions per regulatory guidance
        let chains = sqlx::query!(
            r#"SELECT chain_id, ubo_person_id, ubo_name,
                      path_entities, path_names, ownership_percentages,
                      effective_ownership, chain_depth, is_complete,
                      relationship_types, has_control_path
               FROM "ob-poc".compute_ownership_chains($1, $2, 10, $4)
               WHERE effective_ownership >= $3 OR has_control_path = true
               ORDER BY effective_ownership DESC NULLS LAST"#,
            cbu_id,
            target_entity_id,
            threshold_bd,
            as_of_date
        )
        .fetch_all(pool)
        .await?;

        let chain_list: Vec<serde_json::Value> = chains
            .iter()
            .map(|c| {
                json!({
                    "chain_id": c.chain_id,
                    "ubo_person_id": c.ubo_person_id,
                    "ubo_name": c.ubo_name,
                    "path_entities": c.path_entities,
                    "path_names": c.path_names,
                    "ownership_percentages": c.ownership_percentages,
                    "effective_ownership": c.effective_ownership,
                    "chain_depth": c.chain_depth,
                    "is_complete": c.is_complete,
                    "relationship_types": c.relationship_types,
                    "has_control_path": c.has_control_path,
                    "ubo_type": if c.has_control_path == Some(true) && c.effective_ownership.is_none() {
                        "CONTROL"
                    } else if c.has_control_path == Some(true) {
                        "OWNERSHIP_AND_CONTROL"
                    } else {
                        "OWNERSHIP"
                    }
                })
            })
            .collect();

        // Separate ownership-based and control-based chains
        let ownership_chains: Vec<&serde_json::Value> = chain_list
            .iter()
            .filter(|c| c.get("ubo_type").and_then(|v| v.as_str()) != Some("CONTROL"))
            .collect();

        let control_chains: Vec<&serde_json::Value> = chain_list
            .iter()
            .filter(|c| {
                let ubo_type = c.get("ubo_type").and_then(|v| v.as_str());
                ubo_type == Some("CONTROL") || ubo_type == Some("OWNERSHIP_AND_CONTROL")
            })
            .collect();

        let result = json!({
            "cbu_id": cbu_id,
            "target_entity_id": target_entity_id,
            "threshold": threshold,
            "as_of_date": as_of_date.to_string(),
            "chain_count": chain_list.len(),
            "chains": chain_list,
            "ownership_chain_count": ownership_chains.len(),
            "control_chain_count": control_chains.len(),
            "includes_control_relationships": true
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
            "chain_count": 0,
            "chains": []
        })))
    }
}

/// List owners of an entity as of a specific date
///
/// Rationale: Returns ownership relationships for an entity with temporal filtering.
/// This is the temporal-aware version of the CRUD list-owners.
pub struct UboListOwnersOp;

#[async_trait]
impl CustomOperation for UboListOwnersOp {
    fn domain(&self) -> &'static str {
        "ubo"
    }
    fn verb(&self) -> &'static str {
        "list-owners"
    }
    fn rationale(&self) -> &'static str {
        "Lists ownership relationships with temporal filtering"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use serde_json::json;
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

        // Get as-of-date (optional, defaults to today)
        let as_of_date: chrono::NaiveDate = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "as-of-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());

        // Query ownership relationships with temporal filtering
        let owners = sqlx::query!(
            r#"SELECT
                r.relationship_id,
                r.from_entity_id as owner_entity_id,
                e.name as owner_name,
                et.type_code as owner_type,
                r.percentage,
                r.ownership_type,
                r.effective_from,
                r.effective_to,
                r.source
            FROM "ob-poc".entity_relationships r
            JOIN "ob-poc".entities e ON r.from_entity_id = e.entity_id
            JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
            WHERE r.to_entity_id = $1
              AND r.relationship_type = 'ownership'
              AND (r.effective_from IS NULL OR r.effective_from <= $2)
              AND (r.effective_to IS NULL OR r.effective_to >= $2)
            ORDER BY r.percentage DESC NULLS LAST"#,
            entity_id,
            as_of_date
        )
        .fetch_all(pool)
        .await?;

        let owner_list: Vec<serde_json::Value> = owners
            .iter()
            .map(|o| {
                json!({
                    "relationship_id": o.relationship_id,
                    "owner_entity_id": o.owner_entity_id,
                    "owner_name": o.owner_name,
                    "owner_type": o.owner_type,
                    "percentage": o.percentage,
                    "ownership_type": o.ownership_type,
                    "effective_from": o.effective_from,
                    "effective_to": o.effective_to,
                    "source": o.source
                })
            })
            .collect();

        let result = json!({
            "entity_id": entity_id,
            "as_of_date": as_of_date.to_string(),
            "owner_count": owner_list.len(),
            "owners": owner_list
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
            "owner_count": 0,
            "owners": []
        })))
    }
}
