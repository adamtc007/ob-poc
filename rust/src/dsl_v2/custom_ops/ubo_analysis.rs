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

/// Discover potential UBOs from document extraction or registry lookup
///
/// Rationale: Orchestrates document extraction and registry lookups to identify
/// potential beneficial owners, creating preliminary ownership records.
pub struct UboDiscoverOwnerOp;

#[async_trait]
impl CustomOperation for UboDiscoverOwnerOp {
    fn domain(&self) -> &'static str {
        "ubo"
    }
    fn verb(&self) -> &'static str {
        "discover-owner"
    }
    fn rationale(&self) -> &'static str {
        "Orchestrates multiple data sources to discover potential UBOs"
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
            .find(|a| a.key.matches("cbu-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing cbu-id argument"))?;

        let entity_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("entity-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing entity-id argument"))?;

        let source_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("source-type"))
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing source-type argument"))?;

        let source_ref = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("source-ref"))
            .and_then(|a| a.value.as_string());

        // Look for ownership information based on source type
        let discovered_owners: Vec<serde_json::Value> = match source_type {
            "DOCUMENT" => {
                // Look for ownership observations extracted from documents
                let obs = sqlx::query!(
                    r#"SELECT ao.observation_id, ao.value_json, ao.confidence,
                              dc.doc_id, dt.type_code as doc_type
                       FROM "ob-poc".attribute_observations ao
                       JOIN "ob-poc".attribute_registry ar ON ar.uuid = ao.attribute_id
                       LEFT JOIN "ob-poc".document_catalog dc ON dc.doc_id = ao.source_document_id
                       LEFT JOIN "ob-poc".document_types dt ON dt.type_id = dc.document_type_id
                       WHERE ao.entity_id = $1
                       AND ar.id LIKE '%ownership%'
                       AND ao.status = 'ACTIVE'"#,
                    entity_id
                )
                .fetch_all(pool)
                .await?;

                obs.iter()
                    .filter_map(|o| {
                        o.value_json.as_ref().map(|v| {
                            json!({
                                "source": "DOCUMENT",
                                "observation_id": o.observation_id,
                                "document_id": o.doc_id,
                                "document_type": o.doc_type,
                                "ownership_data": v,
                                "confidence": o.confidence
                            })
                        })
                    })
                    .collect()
            }
            "REGISTRY" => {
                // Look for existing ownership relationships
                let rels = sqlx::query!(
                    r#"SELECT or2.ownership_id, or2.owner_entity_id, e.name as owner_name,
                              or2.ownership_percent, or2.ownership_type
                       FROM "ob-poc".ownership_relationships or2
                       JOIN "ob-poc".entities e ON e.entity_id = or2.owner_entity_id
                       WHERE or2.owned_entity_id = $1
                       AND (or2.effective_to IS NULL OR or2.effective_to > CURRENT_DATE)"#,
                    entity_id
                )
                .fetch_all(pool)
                .await?;

                rels.iter()
                    .map(|r| {
                        json!({
                            "source": "REGISTRY",
                            "ownership_id": r.ownership_id,
                            "owner_entity_id": r.owner_entity_id,
                            "owner_name": r.owner_name,
                            "ownership_percent": r.ownership_percent,
                            "ownership_type": r.ownership_type
                        })
                    })
                    .collect()
            }
            "SCREENING" => {
                // Look for screening results that mention ownership
                let screenings = sqlx::query!(
                    r#"SELECT s.screening_id, s.result_data
                       FROM kyc.screenings s
                       JOIN kyc.entity_workstreams w ON w.workstream_id = s.workstream_id
                       WHERE w.entity_id = $1
                       AND s.status IN ('CLEAR', 'HIT_CONFIRMED')
                       AND s.result_data IS NOT NULL"#,
                    entity_id
                )
                .fetch_all(pool)
                .await?;

                screenings
                    .iter()
                    .filter_map(|s| {
                        s.result_data.as_ref().map(|data| {
                            json!({
                                "source": "SCREENING",
                                "screening_id": s.screening_id,
                                "data": data
                            })
                        })
                    })
                    .collect()
            }
            _ => vec![],
        };

        let result = json!({
            "cbu_id": cbu_id,
            "entity_id": entity_id,
            "source_type": source_type,
            "source_ref": source_ref,
            "discovered_count": discovered_owners.len(),
            "discovered_owners": discovered_owners
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
            "discovered_count": 0,
            "discovered_owners": []
        })))
    }
}

/// Trace all ownership chains to natural persons for a CBU
///
/// Rationale: Uses the SQL function compute_ownership_chains() to traverse
/// the ownership graph and identify all beneficial owners.
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
            .find(|a| a.key.matches("cbu-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing cbu-id argument"))?;

        let target_entity_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("target-entity-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });

        let threshold: f64 = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("threshold"))
            .and_then(|a| a.value.as_decimal())
            .map(|d| d.to_string().parse().unwrap_or(25.0))
            .unwrap_or(25.0);

        // Convert f64 to BigDecimal via string
        let threshold_bd: sqlx::types::BigDecimal = threshold
            .to_string()
            .parse()
            .unwrap_or_else(|_| "25.0".parse().unwrap());

        // Call the SQL function
        let chains = sqlx::query!(
            r#"SELECT chain_id, ubo_person_id, ubo_name,
                      path_entities, path_names, ownership_percentages,
                      effective_ownership, chain_depth, is_complete
               FROM "ob-poc".compute_ownership_chains($1, $2, 10)
               WHERE effective_ownership >= $3
               ORDER BY effective_ownership DESC"#,
            cbu_id,
            target_entity_id,
            threshold_bd
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
                    "is_complete": c.is_complete
                })
            })
            .collect();

        let result = json!({
            "cbu_id": cbu_id,
            "target_entity_id": target_entity_id,
            "threshold": threshold,
            "chain_count": chain_list.len(),
            "chains": chain_list
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

/// Infer ownership chain from known relationships
///
/// Rationale: Starting from a given entity, traces upward through ownership
/// to find the ultimate owner(s).
pub struct UboInferChainOp;

#[async_trait]
impl CustomOperation for UboInferChainOp {
    fn domain(&self) -> &'static str {
        "ubo"
    }
    fn verb(&self) -> &'static str {
        "infer-chain"
    }
    fn rationale(&self) -> &'static str {
        "Traces ownership chain upward from a starting entity"
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
            .find(|a| a.key.matches("cbu-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing cbu-id argument"))?;

        let start_entity_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("start-entity-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing start-entity-id argument"))?;

        let max_depth: i32 = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("max-depth"))
            .and_then(|a| a.value.as_integer())
            .map(|i| i as i32)
            .unwrap_or(10);

        // Recursive CTE to trace upward (cbu_id not used in query but kept for context)
        let _ = cbu_id; // Acknowledge cbu_id even though not used in this particular query
        let chain = sqlx::query!(
            r#"WITH RECURSIVE upward_chain AS (
                -- Base: start entity
                SELECT
                    $1::uuid as entity_id,
                    e.name::text as entity_name,
                    et.type_code::text as entity_type,
                    ARRAY[$1::uuid] as path,
                    ARRAY[e.name::text] as names,
                    1 as depth
                FROM "ob-poc".entities e
                JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
                WHERE e.entity_id = $1

                UNION ALL

                -- Recursive: find owners
                SELECT
                    orel.owner_entity_id,
                    e.name::text,
                    et.type_code::text,
                    uc.path || orel.owner_entity_id,
                    uc.names || e.name::text,
                    uc.depth + 1
                FROM upward_chain uc
                JOIN "ob-poc".ownership_relationships orel ON orel.owned_entity_id = uc.entity_id
                JOIN "ob-poc".entities e ON e.entity_id = orel.owner_entity_id
                JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
                WHERE uc.depth < $2
                AND NOT orel.owner_entity_id = ANY(uc.path)
                AND (orel.effective_to IS NULL OR orel.effective_to > CURRENT_DATE)
            )
            SELECT entity_id, entity_name, entity_type, path, names, depth
            FROM upward_chain
            ORDER BY depth"#,
            start_entity_id,
            max_depth
        )
        .fetch_all(pool)
        .await?;

        let chain_steps: Vec<serde_json::Value> = chain
            .iter()
            .map(|c| {
                json!({
                    "entity_id": c.entity_id,
                    "entity_name": c.entity_name,
                    "entity_type": c.entity_type,
                    "depth": c.depth,
                    "is_person": c.entity_type.as_ref().map(|t| t == "proper_person").unwrap_or(false)
                })
            })
            .collect();

        // Find terminal nodes (persons or entities with no further ownership)
        let terminals: Vec<&serde_json::Value> = chain_steps
            .iter()
            .filter(|c| {
                c.get("is_person")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            })
            .collect();

        let result = json!({
            "cbu_id": cbu_id,
            "start_entity_id": start_entity_id,
            "max_depth": max_depth,
            "chain_length": chain_steps.len(),
            "chain": chain_steps,
            "terminal_ubos": terminals
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
            "chain_length": 0,
            "chain": [],
            "terminal_ubos": []
        })))
    }
}

/// Check if UBO determination is complete for a CBU
///
/// Rationale: Calls the SQL function to validate UBO completeness.
pub struct UboCheckCompletenessOp;

#[async_trait]
impl CustomOperation for UboCheckCompletenessOp {
    fn domain(&self) -> &'static str {
        "ubo"
    }
    fn verb(&self) -> &'static str {
        "check-completeness"
    }
    fn rationale(&self) -> &'static str {
        "Validates UBO determination completeness using SQL function"
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
            .find(|a| a.key.matches("cbu-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing cbu-id argument"))?;

        let threshold: f64 = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("threshold"))
            .and_then(|a| a.value.as_decimal())
            .map(|d| d.to_string().parse().unwrap_or(25.0))
            .unwrap_or(25.0);

        // Convert f64 to BigDecimal via string
        let threshold_bd: sqlx::types::BigDecimal = threshold
            .to_string()
            .parse()
            .unwrap_or_else(|_| "25.0".parse().unwrap());

        // Call the SQL function
        let result = sqlx::query!(
            r#"SELECT is_complete, total_identified_ownership, gap_percentage,
                      missing_chains, ubos_above_threshold, issues
               FROM "ob-poc".check_ubo_completeness($1, $2)"#,
            cbu_id,
            threshold_bd
        )
        .fetch_optional(pool)
        .await?;

        let output = match result {
            Some(r) => json!({
                "cbu_id": cbu_id,
                "threshold": threshold,
                "is_complete": r.is_complete,
                "total_identified_ownership": r.total_identified_ownership,
                "gap_percentage": r.gap_percentage,
                "missing_chains": r.missing_chains,
                "ubos_above_threshold": r.ubos_above_threshold,
                "issues": r.issues
            }),
            None => json!({
                "cbu_id": cbu_id,
                "threshold": threshold,
                "is_complete": false,
                "total_identified_ownership": 0,
                "gap_percentage": 100,
                "missing_chains": 0,
                "ubos_above_threshold": 0,
                "issues": []
            }),
        };

        Ok(ExecutionResult::Record(output))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "is_complete": false,
            "issues": []
        })))
    }
}

/// Supersede a UBO record with a newer determination
///
/// Rationale: Manages UBO versioning by linking old and new records.
pub struct UboSupersedeOp;

#[async_trait]
impl CustomOperation for UboSupersedeOp {
    fn domain(&self) -> &'static str {
        "ubo"
    }
    fn verb(&self) -> &'static str {
        "supersede-ubo"
    }
    fn rationale(&self) -> &'static str {
        "Manages UBO record versioning and supersession chain"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        let old_ubo_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("old-ubo-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing old-ubo-id argument"))?;

        let new_ubo_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("new-ubo-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing new-ubo-id argument"))?;

        // Update old record to point to new
        let rows = sqlx::query!(
            r#"UPDATE "ob-poc".ubo_registry
               SET superseded_by = $2, superseded_at = NOW(), updated_at = NOW()
               WHERE ubo_id = $1 AND superseded_at IS NULL"#,
            old_ubo_id,
            new_ubo_id
        )
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Affected(rows.rows_affected()))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Affected(1))
    }
}

/// Capture a point-in-time snapshot of UBO state for a CBU
///
/// Rationale: Calls the SQL function to capture complete UBO state.
pub struct UboSnapshotCbuOp;

#[async_trait]
impl CustomOperation for UboSnapshotCbuOp {
    fn domain(&self) -> &'static str {
        "ubo"
    }
    fn verb(&self) -> &'static str {
        "snapshot-cbu"
    }
    fn rationale(&self) -> &'static str {
        "Captures complete UBO state using SQL function"
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
            .find(|a| a.key.matches("cbu-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing cbu-id argument"))?;

        let case_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("case-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });

        let snapshot_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("snapshot-type"))
            .and_then(|a| a.value.as_string())
            .unwrap_or("MANUAL");

        let reason = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("reason"))
            .and_then(|a| a.value.as_string());

        // Call the SQL function
        let snapshot_id: Uuid =
            sqlx::query_scalar(r#"SELECT "ob-poc".capture_ubo_snapshot($1, $2, $3, $4, NULL)"#)
                .bind(cbu_id)
                .bind(case_id)
                .bind(snapshot_type)
                .bind(reason)
                .fetch_one(pool)
                .await?;

        ctx.bind("snapshot", snapshot_id);

        Ok(ExecutionResult::Uuid(snapshot_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(uuid::Uuid::new_v4()))
    }
}

/// Compare two UBO snapshots to detect changes
///
/// Rationale: Complex comparison logic between two snapshot states.
pub struct UboCompareSnapshotOp;

#[async_trait]
impl CustomOperation for UboCompareSnapshotOp {
    fn domain(&self) -> &'static str {
        "ubo"
    }
    fn verb(&self) -> &'static str {
        "compare-snapshot"
    }
    fn rationale(&self) -> &'static str {
        "Compares two snapshots and records differences"
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
            .find(|a| a.key.matches("cbu-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing cbu-id argument"))?;

        let baseline_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("baseline-snapshot-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing baseline-snapshot-id argument"))?;

        let current_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key.matches("current-snapshot-id"))
            .and_then(|a| {
                if let Some(name) = a.value.as_reference() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing current-snapshot-id argument"))?;

        // Get both snapshots
        let baseline = sqlx::query!(
            r#"SELECT ubos, ownership_chains, control_relationships, total_identified_ownership
               FROM "ob-poc".ubo_snapshots WHERE snapshot_id = $1"#,
            baseline_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Baseline snapshot not found"))?;

        let current = sqlx::query!(
            r#"SELECT ubos, ownership_chains, control_relationships, total_identified_ownership
               FROM "ob-poc".ubo_snapshots WHERE snapshot_id = $1"#,
            current_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Current snapshot not found"))?;

        // Compare UBOs
        let baseline_ubo_ids: std::collections::HashSet<String> = baseline
            .ubos
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|u| u.get("ubo_person_id").and_then(|v| v.as_str()))
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();

        let current_ubo_ids: std::collections::HashSet<String> = current
            .ubos
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|u| u.get("ubo_person_id").and_then(|v| v.as_str()))
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();

        let added: Vec<&String> = current_ubo_ids.difference(&baseline_ubo_ids).collect();
        let removed: Vec<&String> = baseline_ubo_ids.difference(&current_ubo_ids).collect();

        let has_changes = !added.is_empty() || !removed.is_empty();

        let change_summary = json!({
            "ubos_added": added.len(),
            "ubos_removed": removed.len(),
            "ownership_change": current.total_identified_ownership != baseline.total_identified_ownership
        });

        // Record comparison
        let comparison_id = Uuid::new_v4();
        sqlx::query!(
            r#"INSERT INTO "ob-poc".ubo_snapshot_comparisons
               (comparison_id, cbu_id, baseline_snapshot_id, current_snapshot_id,
                has_changes, change_summary, added_ubos, removed_ubos)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
            comparison_id,
            cbu_id,
            baseline_id,
            current_id,
            has_changes,
            change_summary,
            json!(added),
            json!(removed)
        )
        .execute(pool)
        .await?;

        let result = json!({
            "comparison_id": comparison_id,
            "cbu_id": cbu_id,
            "baseline_snapshot_id": baseline_id,
            "current_snapshot_id": current_id,
            "has_changes": has_changes,
            "change_summary": change_summary,
            "added_ubos": added,
            "removed_ubos": removed,
            "baseline_ownership": baseline.total_identified_ownership,
            "current_ownership": current.total_identified_ownership
        });

        ctx.bind("comparison", comparison_id);

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "has_changes": false,
            "change_summary": {}
        })))
    }
}
