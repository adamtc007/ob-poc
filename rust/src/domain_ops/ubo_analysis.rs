//! UBO (Ultimate Beneficial Owner) Analysis Operations
//!
//! Enhanced UBO operations for discovery, chain tracing, snapshots, and comparisons.
//! These extend the basic UBO CRUD operations with complex graph traversal and analysis.

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;

use crate::domain_ops::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

/// Calculate UBO (Ultimate Beneficial Ownership) chain
///
/// Rationale: Requires recursive graph traversal through ownership chains
/// to identify beneficial owners above the specified threshold.
#[register_custom_op]
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

/// List entities owned by one or more entities
///
/// Rationale: Returns ownership relationships where entities are the owners.
/// Supports Pattern B entity scope via :entity-ids argument.
#[register_custom_op]
pub struct UboListOwnedOp;

#[async_trait]
impl CustomOperation for UboListOwnedOp {
    fn domain(&self) -> &'static str {
        "ubo"
    }
    fn verb(&self) -> &'static str {
        "list-owned"
    }
    fn rationale(&self) -> &'static str {
        "Lists entities owned by entities with Pattern B scope support"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use serde_json::json;
        use sqlx::Row;
        use uuid::Uuid;

        // Check for entity-ids (Pattern B scope rewrite) first
        let entity_ids: Vec<Uuid> = if let Some(arg) =
            verb_call.arguments.iter().find(|a| a.key == "entity-ids")
        {
            // Extract UUIDs from list
            arg.value
                .as_list()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| {
                            item.as_uuid()
                                .or_else(|| item.as_string().and_then(|s| Uuid::parse_str(s).ok()))
                        })
                        .collect()
                })
                .unwrap_or_default()
        } else if let Some(arg) = verb_call.arguments.iter().find(|a| a.key == "entity-id") {
            // Single entity-id fallback
            let entity_id = if let Some(name) = arg.value.as_symbol() {
                ctx.resolve(name)
            } else {
                arg.value.as_uuid()
            };
            entity_id.map(|id| vec![id]).unwrap_or_default()
        } else {
            return Err(anyhow::anyhow!("Missing entity-id or entity-ids argument"));
        };

        if entity_ids.is_empty() {
            return Ok(ExecutionResult::RecordSet(vec![]));
        }

        // Query ownership relationships where entity_ids are the owners (from_entity_id)
        let owned = sqlx::query(
            r#"SELECT
                r.relationship_id,
                r.from_entity_id as owner_entity_id,
                r.to_entity_id as owned_entity_id,
                e.name as owned_name,
                et.type_code as owned_type,
                r.percentage,
                r.ownership_type,
                r.effective_from,
                r.effective_to
            FROM "ob-poc".entity_relationships r
            JOIN "ob-poc".entities e ON r.to_entity_id = e.entity_id
            JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
            WHERE r.from_entity_id = ANY($1)
              AND r.relationship_type = 'ownership'
              AND (r.effective_to IS NULL OR r.effective_to >= CURRENT_DATE)
            ORDER BY r.from_entity_id, r.percentage DESC NULLS LAST"#,
        )
        .bind(&entity_ids)
        .fetch_all(pool)
        .await?;

        let owned_list: Vec<serde_json::Value> = owned
            .iter()
            .map(|o| {
                json!({
                    "relationship_id": o.get::<Uuid, _>("relationship_id"),
                    "owner_entity_id": o.get::<Uuid, _>("owner_entity_id"),
                    "owned_entity_id": o.get::<Uuid, _>("owned_entity_id"),
                    "owned_name": o.get::<Option<String>, _>("owned_name"),
                    "owned_type": o.get::<Option<String>, _>("owned_type"),
                    "percentage": o.get::<Option<sqlx::types::BigDecimal>, _>("percentage"),
                    "ownership_type": o.get::<Option<String>, _>("ownership_type"),
                    "effective_from": o.get::<Option<chrono::NaiveDate>, _>("effective_from"),
                    "effective_to": o.get::<Option<chrono::NaiveDate>, _>("effective_to")
                })
            })
            .collect();

        Ok(ExecutionResult::RecordSet(owned_list))
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

/// Trace UBO ownership chains for entities
///
/// Rationale: Orchestrates document extraction and registry lookups to identify
/// potential beneficial owners, creating preliminary ownership records.
/// Supports Pattern B entity scope via :entity-ids argument.
#[register_custom_op]
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
        "Calls SQL recursive function to compute ownership chains with Pattern B scope support"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use serde_json::json;
        use sqlx::Row;
        use uuid::Uuid;

        // Check for entity-ids (Pattern B scope rewrite) first
        let entity_ids: Option<Vec<Uuid>> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "entity-ids")
            .and_then(|arg| {
                arg.value.as_list().map(|items| {
                    items
                        .iter()
                        .filter_map(|item| {
                            item.as_uuid()
                                .or_else(|| item.as_string().and_then(|s| Uuid::parse_str(s).ok()))
                        })
                        .collect()
                })
            });

        // If entity-ids provided, use those directly (Pattern B mode)
        if let Some(ids) = entity_ids {
            if ids.is_empty() {
                return Ok(ExecutionResult::RecordSet(vec![]));
            }

            let threshold: f64 = verb_call
                .arguments
                .iter()
                .find(|a| a.key == "threshold")
                .and_then(|a| a.value.as_decimal())
                .map(|d| d.to_string().parse().unwrap_or(25.0))
                .unwrap_or(25.0);

            let as_of_date: chrono::NaiveDate = verb_call
                .arguments
                .iter()
                .find(|a| a.key == "as-of-date")
                .and_then(|a| a.value.as_string())
                .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
                .unwrap_or_else(|| chrono::Utc::now().date_naive());

            let threshold_bd: sqlx::types::BigDecimal = threshold
                .to_string()
                .parse()
                .unwrap_or_else(|_| sqlx::types::BigDecimal::from(25));

            // Query for all entity_ids in scope
            let mut all_chains: Vec<serde_json::Value> = Vec::new();
            for entity_id in &ids {
                // Get the CBU for this entity (if any) to call compute_ownership_chains
                let cbu_row = sqlx::query(
                    r#"SELECT cbu_id FROM "ob-poc".cbu_entity_roles WHERE entity_id = $1 LIMIT 1"#,
                )
                .bind(entity_id)
                .fetch_optional(pool)
                .await?;

                if let Some(row) = cbu_row {
                    let cbu_id: Uuid = row.get("cbu_id");
                    let chains = sqlx::query(
                        r#"SELECT chain_id, ubo_person_id, ubo_name,
                                  path_entities, path_names, ownership_percentages,
                                  effective_ownership, chain_depth, is_complete,
                                  relationship_types, has_control_path
                           FROM "ob-poc".compute_ownership_chains($1, $2, 10, $4)
                           WHERE effective_ownership >= $3 OR has_control_path = true
                           ORDER BY effective_ownership DESC NULLS LAST"#,
                    )
                    .bind(cbu_id)
                    .bind(Some(*entity_id))
                    .bind(threshold_bd.clone())
                    .bind(as_of_date)
                    .fetch_all(pool)
                    .await?;

                    for c in chains {
                        let has_control: Option<bool> = c.get("has_control_path");
                        let eff_ownership: Option<sqlx::types::BigDecimal> =
                            c.get("effective_ownership");
                        let ubo_type = if has_control == Some(true) && eff_ownership.is_none() {
                            "CONTROL"
                        } else if has_control == Some(true) {
                            "OWNERSHIP_AND_CONTROL"
                        } else {
                            "OWNERSHIP"
                        };
                        all_chains.push(json!({
                            "source_entity_id": entity_id,
                            "cbu_id": cbu_id,
                            "chain_id": c.get::<Option<i32>, _>("chain_id"),
                            "ubo_person_id": c.get::<Option<Uuid>, _>("ubo_person_id"),
                            "ubo_name": c.get::<Option<String>, _>("ubo_name"),
                            "path_entities": c.get::<Option<Vec<Uuid>>, _>("path_entities"),
                            "path_names": c.get::<Option<Vec<String>>, _>("path_names"),
                            "ownership_percentages": c.get::<Option<Vec<sqlx::types::BigDecimal>>, _>("ownership_percentages"),
                            "effective_ownership": eff_ownership,
                            "chain_depth": c.get::<Option<i32>, _>("chain_depth"),
                            "is_complete": c.get::<Option<bool>, _>("is_complete"),
                            "relationship_types": c.get::<Option<Vec<String>>, _>("relationship_types"),
                            "has_control_path": has_control,
                            "ubo_type": ubo_type
                        }));
                    }
                }
            }

            return Ok(ExecutionResult::RecordSet(all_chains));
        }

        // Fallback: traditional cbu-id based trace
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
            .ok_or_else(|| anyhow::anyhow!("Missing cbu-id or entity-ids argument"))?;

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

/// List owners of one or more entities as of a specific date
///
/// Rationale: Returns ownership relationships for entities with temporal filtering.
/// Supports Pattern B entity scope via :entity-ids argument.
#[register_custom_op]
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
        "Lists ownership relationships with temporal filtering and Pattern B scope support"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use serde_json::json;
        use sqlx::Row;
        use uuid::Uuid;

        // Check for entity-ids (Pattern B scope rewrite) first
        let entity_ids: Vec<Uuid> = if let Some(arg) =
            verb_call.arguments.iter().find(|a| a.key == "entity-ids")
        {
            // Extract UUIDs from list
            arg.value
                .as_list()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| {
                            item.as_uuid()
                                .or_else(|| item.as_string().and_then(|s| Uuid::parse_str(s).ok()))
                        })
                        .collect()
                })
                .unwrap_or_default()
        } else if let Some(arg) = verb_call.arguments.iter().find(|a| a.key == "entity-id") {
            // Single entity-id fallback
            let entity_id = if let Some(name) = arg.value.as_symbol() {
                ctx.resolve(name)
            } else {
                arg.value.as_uuid()
            };
            entity_id.map(|id| vec![id]).unwrap_or_default()
        } else {
            return Err(anyhow::anyhow!("Missing entity-id or entity-ids argument"));
        };

        if entity_ids.is_empty() {
            return Ok(ExecutionResult::RecordSet(vec![]));
        }

        // Get as-of-date (optional, defaults to today)
        let as_of_date: chrono::NaiveDate = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "as-of-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());

        // Query ownership relationships with temporal filtering - use runtime query for entity_ids array
        let owners = sqlx::query(
            r#"SELECT
                r.relationship_id,
                r.to_entity_id as subject_entity_id,
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
            WHERE r.to_entity_id = ANY($1)
              AND r.relationship_type = 'ownership'
              AND (r.effective_from IS NULL OR r.effective_from <= $2)
              AND (r.effective_to IS NULL OR r.effective_to >= $2)
            ORDER BY r.to_entity_id, r.percentage DESC NULLS LAST"#,
        )
        .bind(&entity_ids)
        .bind(as_of_date)
        .fetch_all(pool)
        .await?;

        let owner_list: Vec<serde_json::Value> = owners
            .iter()
            .map(|o| {
                json!({
                    "relationship_id": o.get::<Uuid, _>("relationship_id"),
                    "subject_entity_id": o.get::<Uuid, _>("subject_entity_id"),
                    "owner_entity_id": o.get::<Uuid, _>("owner_entity_id"),
                    "owner_name": o.get::<Option<String>, _>("owner_name"),
                    "owner_type": o.get::<Option<String>, _>("owner_type"),
                    "percentage": o.get::<Option<sqlx::types::BigDecimal>, _>("percentage"),
                    "ownership_type": o.get::<Option<String>, _>("ownership_type"),
                    "effective_from": o.get::<Option<chrono::NaiveDate>, _>("effective_from"),
                    "effective_to": o.get::<Option<chrono::NaiveDate>, _>("effective_to"),
                    "source": o.get::<Option<String>, _>("source")
                })
            })
            .collect();

        Ok(ExecutionResult::RecordSet(owner_list))
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
