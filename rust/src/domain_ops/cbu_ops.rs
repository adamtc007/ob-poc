//! CBU custom operations
//!
//! Operations for CBU (Client Business Unit) management including
//! product assignment, show, decide, and cascade delete.

use anyhow::Result;
use async_trait::async_trait;
use governed_query_proc::governed_query;
use dsl_runtime_macros::register_custom_op;

use super::helpers::{
    extract_bool_opt, extract_int_opt, extract_string_opt, get_required_uuid,
    json_extract_bool_opt, json_extract_int_opt, json_extract_string, json_extract_string_opt,
    json_extract_uuid, json_extract_uuid_opt,
};
use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use chrono::NaiveDate;
#[cfg(feature = "database")]
use sqlx::PgPool;
#[cfg(feature = "database")]
use uuid::Uuid;

#[cfg(feature = "database")]
fn extract_uuid_alias(verb_call: &VerbCall, ctx: &ExecutionContext, keys: &[&str]) -> Option<Uuid> {
    verb_call.arguments.iter().find_map(|arg| {
        if !keys.iter().any(|key| arg.key == *key) {
            return None;
        }
        if let Some(symbol) = arg.value.as_symbol() {
            return ctx.resolve(symbol);
        }
        if let Some(uuid) = arg.value.as_uuid() {
            return Some(uuid);
        }
        arg.value
            .as_string()
            .and_then(|value| Uuid::parse_str(value).ok())
    })
}

#[cfg(feature = "database")]
fn extract_string_alias(verb_call: &VerbCall, keys: &[&str]) -> Option<String> {
    verb_call.arguments.iter().find_map(|arg| {
        if keys.iter().any(|key| arg.key == *key) {
            return arg.value.as_string().map(ToOwned::to_owned);
        }
        None
    })
}

#[cfg(feature = "database")]
fn json_extract_uuid_alias(
    args: &serde_json::Value,
    ctx: &mut dsl_runtime::VerbExecutionContext,
    keys: &[&str],
) -> Result<Option<Uuid>> {
    for key in keys {
        if args.get(*key).is_some() {
            return Ok(Some(json_extract_uuid(args, ctx, key)?));
        }
    }
    Ok(None)
}

#[cfg(feature = "database")]
fn json_extract_string_alias(args: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        args.get(*key)
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
    })
}

#[cfg(feature = "database")]
fn parse_optional_date(value: Option<String>, arg_name: &str) -> Result<Option<NaiveDate>> {
    value
        .map(|raw| {
            NaiveDate::parse_from_str(&raw, "%Y-%m-%d")
                .map_err(|err| anyhow::anyhow!("invalid {} '{}': {}", arg_name, raw, err))
        })
        .transpose()
}

#[cfg(feature = "database")]
fn normalize_relationship_type(raw: &str) -> String {
    raw.replace('-', "_").to_ascii_uppercase()
}

#[cfg(feature = "database")]
fn normalize_capital_flow(raw: &str) -> String {
    raw.replace('-', "_").to_ascii_uppercase()
}

// ============================================================================
// CBU Create (with entity-based idempotency)
// ============================================================================

/// Create a new CBU with optional fund entity linking
///
/// Idempotency:
/// - If :fund-entity-id is provided, checks if that entity is already linked to ANY CBU
///   as ASSET_OWNER. If so, returns the existing CBU (skipped).
/// - If no :fund-entity-id, uses name+jurisdiction as fallback idempotency key.
///
/// Entity Linking:
/// - If :fund-entity-id provided, links entity to CBU with ASSET_OWNER role
/// - If :manco-entity-id provided, links entity with MANAGEMENT_COMPANY role
#[register_custom_op]
pub struct CbuCreateOp;

#[async_trait]
impl CustomOperation for CbuCreateOp {
    fn domain(&self) -> &'static str {
        "cbu"
    }
    fn verb(&self) -> &'static str {
        "create"
    }
    fn rationale(&self) -> &'static str {
        "Entity-based idempotency: skips if fund entity already on a CBU"
    }

    #[cfg(feature = "database")]
    #[governed_query(verb = "cbu.create", skip_principal_check = true)]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let name = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "name")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("cbu.create: Missing required argument :name"))?;

        let jurisdiction = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "jurisdiction")
            .and_then(|a| a.value.as_string());

        let fund_entity_id = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "fund-entity-id")
            .and_then(|a| a.value.as_uuid());

        let manco_entity_id = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "manco-entity-id")
            .and_then(|a| a.value.as_uuid());

        let client_type =
            extract_string_opt(verb_call, "client-type").unwrap_or_else(|| "FUND".to_string());

        let nature_purpose = extract_string_opt(verb_call, "nature-purpose");
        let description = extract_string_opt(verb_call, "description");
        let commercial_client_entity_id = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "commercial-client-entity-id")
            .and_then(|a| a.value.as_uuid());

        // =====================================================================
        // Step 1: Entity-based idempotency check (if fund-entity-id provided)
        // =====================================================================
        if let Some(fund_id) = fund_entity_id {
            // Check if this entity is already linked to a CBU as ASSET_OWNER
            let existing: Option<(Uuid, String)> = sqlx::query_as(
                r#"
                SELECT c.cbu_id, c.name
                FROM "ob-poc".cbu_entity_roles cer
                JOIN "ob-poc".cbus c ON c.cbu_id = cer.cbu_id
                JOIN "ob-poc".roles r ON r.role_id = cer.role_id
                WHERE cer.entity_id = $1
                  AND c.deleted_at IS NULL
                  AND r.name = 'ASSET_OWNER'
                  AND (cer.effective_to IS NULL OR cer.effective_to > CURRENT_DATE)
                LIMIT 1
                "#,
            )
            .bind(fund_id)
            .fetch_optional(pool)
            .await?;

            if let Some((existing_cbu_id, existing_cbu_name)) = existing {
                // Entity already on a CBU - return existing, don't create
                return Ok(ExecutionResult::Record(serde_json::json!({
                    "cbu_id": existing_cbu_id,
                    "name": existing_cbu_name,
                    "created": false,
                    "skipped_reason": format!("Entity {} already linked to CBU '{}'", fund_id, existing_cbu_name)
                })));
            }
        }

        // =====================================================================
        // Step 2: Create CBU (upsert by name+jurisdiction as fallback)
        // =====================================================================
        let (cbu_id, is_new): (Uuid, bool) = sqlx::query_as(
            r#"
            INSERT INTO "ob-poc".cbus (name, jurisdiction, client_type, nature_purpose, description, commercial_client_entity_id)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (name, jurisdiction)
            DO UPDATE SET updated_at = NOW()
            RETURNING cbu_id, (xmax = 0) as is_insert
            "#,
        )
        .bind(name)
        .bind(jurisdiction)
        .bind(&client_type)
        .bind(&nature_purpose)
        .bind(&description)
        .bind(commercial_client_entity_id)
        .fetch_one(pool)
        .await?;

        // =====================================================================
        // Step 3: Link fund entity as ASSET_OWNER (if provided and CBU is new)
        // =====================================================================
        if let Some(fund_id) = fund_entity_id {
            // Get ASSET_OWNER role_id
            let asset_owner_role_id: Option<Uuid> = sqlx::query_scalar(
                r#"SELECT role_id FROM "ob-poc".roles WHERE name = 'ASSET_OWNER'"#,
            )
            .fetch_optional(pool)
            .await?;

            if let Some(role_id) = asset_owner_role_id {
                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id)
                    VALUES ($1, $2, $3)
                    ON CONFLICT (cbu_id, entity_id, role_id) DO NOTHING
                    "#,
                )
                .bind(cbu_id)
                .bind(fund_id)
                .bind(role_id)
                .execute(pool)
                .await?;
            }

            // =====================================================================
            // Step 3b: Link CBU to client_group_entity (if entity is in a group)
            // =====================================================================
            // This enables fast lookup via session.load-cluster without tree walking.
            // Sets cbu_id on client_group_entity for direct CBU → group mapping.
            sqlx::query(
                r#"
                UPDATE "ob-poc".client_group_entity
                SET cbu_id = $1, updated_at = NOW()
                WHERE entity_id = $2
                  AND membership_type NOT IN ('historical', 'rejected')
                  AND cbu_id IS NULL
                "#,
            )
            .bind(cbu_id)
            .bind(fund_id)
            .execute(pool)
            .await?;
        }

        // =====================================================================
        // Step 4: Link manco entity as MANAGEMENT_COMPANY (if provided)
        // =====================================================================
        if let Some(manco_id) = manco_entity_id {
            let manco_role_id: Option<Uuid> = sqlx::query_scalar(
                r#"SELECT role_id FROM "ob-poc".roles WHERE name = 'MANAGEMENT_COMPANY'"#,
            )
            .fetch_optional(pool)
            .await?;

            if let Some(role_id) = manco_role_id {
                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id)
                    VALUES ($1, $2, $3)
                    ON CONFLICT (cbu_id, entity_id, role_id) DO NOTHING
                    "#,
                )
                .bind(cbu_id)
                .bind(manco_id)
                .bind(role_id)
                .execute(pool)
                .await?;
            }
        }

        // Bind result to context if :as provided
        if let Some(binding) = &verb_call.binding {
            ctx.bind(binding, cbu_id);
        }

        let skipped_reason: Option<&str> = if is_new {
            None
        } else {
            Some("CBU with same name+jurisdiction already exists")
        };

        Ok(ExecutionResult::Record(serde_json::json!({
            "cbu_id": cbu_id,
            "name": name,
            "jurisdiction": jurisdiction,
            "created": is_new,
            "skipped_reason": skipped_reason
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "error": "Database feature required for cbu.create"
        })))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let name = json_extract_string(args, "name")?;
        let jurisdiction = json_extract_string_opt(args, "jurisdiction");
        let fund_entity_id = json_extract_uuid_opt(args, ctx, "fund-entity-id");
        let manco_entity_id = json_extract_uuid_opt(args, ctx, "manco-entity-id");
        let client_type =
            json_extract_string_opt(args, "client-type").unwrap_or_else(|| "FUND".to_string());
        let nature_purpose = json_extract_string_opt(args, "nature-purpose");
        let description = json_extract_string_opt(args, "description");
        let commercial_client_entity_id =
            json_extract_uuid_opt(args, ctx, "commercial-client-entity-id");

        if let Some(fund_id) = fund_entity_id {
            let existing: Option<(Uuid, String)> = sqlx::query_as(
                r#"
                SELECT c.cbu_id, c.name
                FROM "ob-poc".cbu_entity_roles cer
                JOIN "ob-poc".cbus c ON c.cbu_id = cer.cbu_id
                JOIN "ob-poc".roles r ON r.role_id = cer.role_id
                WHERE cer.entity_id = $1
                  AND c.deleted_at IS NULL
                  AND r.name = 'ASSET_OWNER'
                  AND (cer.effective_to IS NULL OR cer.effective_to > CURRENT_DATE)
                LIMIT 1
                "#,
            )
            .bind(fund_id)
            .fetch_optional(pool)
            .await?;

            if let Some((existing_cbu_id, existing_cbu_name)) = existing {
                return Ok(dsl_runtime::VerbExecutionOutcome::Record(
                    serde_json::json!({
                        "cbu_id": existing_cbu_id,
                        "name": existing_cbu_name,
                        "created": false,
                        "skipped_reason": format!("Entity {} already linked to CBU '{}'", fund_id, existing_cbu_name)
                    }),
                ));
            }
        }

        let (cbu_id, is_new): (Uuid, bool) = sqlx::query_as(
            r#"
            INSERT INTO "ob-poc".cbus (name, jurisdiction, client_type, nature_purpose, description, commercial_client_entity_id)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (name, jurisdiction)
            DO UPDATE SET updated_at = NOW()
            RETURNING cbu_id, (xmax = 0) as is_insert
            "#,
        )
        .bind(&name)
        .bind(&jurisdiction)
        .bind(&client_type)
        .bind(&nature_purpose)
        .bind(&description)
        .bind(commercial_client_entity_id)
        .fetch_one(pool)
        .await?;

        if let Some(fund_id) = fund_entity_id {
            let asset_owner_role_id: Option<Uuid> = sqlx::query_scalar(
                r#"SELECT role_id FROM "ob-poc".roles WHERE name = 'ASSET_OWNER'"#,
            )
            .fetch_optional(pool)
            .await?;

            if let Some(role_id) = asset_owner_role_id {
                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id)
                    VALUES ($1, $2, $3)
                    ON CONFLICT (cbu_id, entity_id, role_id) DO NOTHING
                    "#,
                )
                .bind(cbu_id)
                .bind(fund_id)
                .bind(role_id)
                .execute(pool)
                .await?;
            }

            sqlx::query(
                r#"
                UPDATE "ob-poc".client_group_entity
                SET cbu_id = $1, updated_at = NOW()
                WHERE entity_id = $2
                  AND membership_type NOT IN ('historical', 'rejected')
                  AND cbu_id IS NULL
                "#,
            )
            .bind(cbu_id)
            .bind(fund_id)
            .execute(pool)
            .await?;
        }

        if let Some(manco_id) = manco_entity_id {
            let manco_role_id: Option<Uuid> = sqlx::query_scalar(
                r#"SELECT role_id FROM "ob-poc".roles WHERE name = 'MANAGEMENT_COMPANY'"#,
            )
            .fetch_optional(pool)
            .await?;

            if let Some(role_id) = manco_role_id {
                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id)
                    VALUES ($1, $2, $3)
                    ON CONFLICT (cbu_id, entity_id, role_id) DO NOTHING
                    "#,
                )
                .bind(cbu_id)
                .bind(manco_id)
                .bind(role_id)
                .execute(pool)
                .await?;
            }
        }

        let skipped_reason: Option<&str> = if is_new {
            None
        } else {
            Some("CBU with same name+jurisdiction already exists")
        };

        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            serde_json::json!({
                "cbu_id": cbu_id,
                "name": name,
                "jurisdiction": jurisdiction,
                "created": is_new,
                "skipped_reason": skipped_reason
            }),
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ============================================================================
// CBU Structure Links
// ============================================================================

/// Persist a parent-child structure link between two CBUs.
#[register_custom_op]
pub struct CbuLinkStructureOp;

#[async_trait]
impl CustomOperation for CbuLinkStructureOp {
    fn domain(&self) -> &'static str {
        "cbu"
    }

    fn verb(&self) -> &'static str {
        "link-structure"
    }

    fn rationale(&self) -> &'static str {
        "Persists cross-border and parallel-fund structure relationships between CBUs"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let parent_cbu_id = extract_uuid_alias(verb_call, ctx, &["parent-cbu-id", "parent_cbu_id"])
            .ok_or_else(|| {
                anyhow::anyhow!("cbu.link-structure: missing required argument :parent-cbu-id")
            })?;
        let child_cbu_id = extract_uuid_alias(verb_call, ctx, &["child-cbu-id", "child_cbu_id"])
            .ok_or_else(|| {
                anyhow::anyhow!("cbu.link-structure: missing required argument :child-cbu-id")
            })?;
        let relationship_type_raw =
            extract_string_alias(verb_call, &["relationship-type", "relationship_type"])
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "cbu.link-structure: missing required argument :relationship-type"
                    )
                })?;
        let relationship_type = normalize_relationship_type(&relationship_type_raw);
        let qualifier = extract_string_alias(verb_call, &["qualifier"]);
        let relationship_selector = extract_string_alias(
            verb_call,
            &["relationship-selector", "relationship_selector"],
        )
        .or_else(|| {
            qualifier.as_ref().map(|value| {
                format!(
                    "{}:{}",
                    relationship_type_raw.to_ascii_lowercase(),
                    value.to_ascii_lowercase()
                )
            })
        })
        .unwrap_or_else(|| relationship_type_raw.to_ascii_lowercase());
        let capital_flow_raw = extract_string_alias(verb_call, &["capital-flow", "capital_flow"]);
        let capital_flow = capital_flow_raw.as_deref().map(normalize_capital_flow);
        let effective_from = parse_optional_date(
            extract_string_alias(verb_call, &["effective-from", "effective_from"]),
            "effective-from",
        )?;
        let effective_to = parse_optional_date(
            extract_string_alias(verb_call, &["effective-to", "effective_to"]),
            "effective-to",
        )?;

        const ALLOWED_RELATIONSHIP_TYPES: &[&str] = &[
            "FEEDER",
            "PARALLEL",
            "AGGREGATOR",
            "MASTER",
            "CO_INVEST_VEHICLE",
        ];
        if !ALLOWED_RELATIONSHIP_TYPES.contains(&relationship_type.as_str()) {
            return Err(anyhow::anyhow!(
                "cbu.link-structure: unsupported relationship-type '{}'",
                relationship_type_raw
            ));
        }
        if let Some(flow) = capital_flow.as_deref() {
            const ALLOWED_CAPITAL_FLOWS: &[&str] = &["UPSTREAM", "DOWNSTREAM", "CO_INVEST"];
            if !ALLOWED_CAPITAL_FLOWS.contains(&flow) {
                return Err(anyhow::anyhow!(
                    "cbu.link-structure: unsupported capital-flow '{}'",
                    capital_flow_raw.unwrap_or_default()
                ));
            }
        }

        let parent_exists: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT cbu_id FROM "ob-poc".cbus WHERE cbu_id = $1 AND deleted_at IS NULL"#,
        )
        .bind(parent_cbu_id)
        .fetch_optional(pool)
        .await?;
        if parent_exists.is_none() {
            return Err(anyhow::anyhow!(
                "cbu.link-structure: parent CBU not found: {}",
                parent_cbu_id
            ));
        }

        let child_exists: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT cbu_id FROM "ob-poc".cbus WHERE cbu_id = $1 AND deleted_at IS NULL"#,
        )
        .bind(child_cbu_id)
        .fetch_optional(pool)
        .await?;
        if child_exists.is_none() {
            return Err(anyhow::anyhow!(
                "cbu.link-structure: child CBU not found: {}",
                child_cbu_id
            ));
        }

        let mut tx = pool.begin().await?;

        let existing_link_id: Option<Uuid> = sqlx::query_scalar(
            r#"
            SELECT link_id
            FROM "ob-poc".cbu_structure_links
            WHERE parent_cbu_id = $1
              AND child_cbu_id = $2
              AND relationship_type = $3
              AND status = 'ACTIVE'
            LIMIT 1
            "#,
        )
        .bind(parent_cbu_id)
        .bind(child_cbu_id)
        .bind(&relationship_type)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(link_id) = existing_link_id {
            sqlx::query(
                r#"
                UPDATE "ob-poc".cbu_structure_links
                SET relationship_selector = $2,
                    capital_flow = $3,
                    effective_from = $4,
                    effective_to = $5,
                    status = 'ACTIVE',
                    updated_at = NOW()
                WHERE link_id = $1
                "#,
            )
            .bind(link_id)
            .bind(&relationship_selector)
            .bind(&capital_flow)
            .bind(effective_from)
            .bind(effective_to)
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".cbu_structure_links (
                    parent_cbu_id,
                    child_cbu_id,
                    relationship_type,
                    relationship_selector,
                    status,
                    capital_flow,
                    effective_from,
                    effective_to
                )
                VALUES ($1, $2, $3, $4, 'ACTIVE', $5, $6, $7)
                "#,
            )
            .bind(parent_cbu_id)
            .bind(child_cbu_id)
            .bind(&relationship_type)
            .bind(&relationship_selector)
            .bind(&capital_flow)
            .bind(effective_from)
            .bind(effective_to)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        if let Some(binding) = &verb_call.binding {
            ctx.bind(binding, child_cbu_id);
        }

        Ok(ExecutionResult::Uuid(child_cbu_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "error": "Database feature required for cbu.link-structure"
        })))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let parent_cbu_id =
            json_extract_uuid_alias(args, ctx, &["parent-cbu-id", "parent_cbu_id"])?.ok_or_else(
                || anyhow::anyhow!("cbu.link-structure: missing required argument :parent-cbu-id"),
            )?;
        let child_cbu_id = json_extract_uuid_alias(args, ctx, &["child-cbu-id", "child_cbu_id"])?
            .ok_or_else(|| {
            anyhow::anyhow!("cbu.link-structure: missing required argument :child-cbu-id")
        })?;
        let relationship_type_raw =
            json_extract_string_alias(args, &["relationship-type", "relationship_type"])
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "cbu.link-structure: missing required argument :relationship-type"
                    )
                })?;
        let relationship_type = normalize_relationship_type(&relationship_type_raw);
        let qualifier = json_extract_string_alias(args, &["qualifier"]);
        let relationship_selector =
            json_extract_string_alias(args, &["relationship-selector", "relationship_selector"])
                .or_else(|| {
                    qualifier.as_ref().map(|value| {
                        format!(
                            "{}:{}",
                            relationship_type_raw.to_ascii_lowercase(),
                            value.to_ascii_lowercase()
                        )
                    })
                })
                .unwrap_or_else(|| relationship_type_raw.to_ascii_lowercase());
        let capital_flow_raw = json_extract_string_alias(args, &["capital-flow", "capital_flow"]);
        let capital_flow = capital_flow_raw.as_deref().map(normalize_capital_flow);
        let effective_from = parse_optional_date(
            json_extract_string_alias(args, &["effective-from", "effective_from"]),
            "effective-from",
        )?;
        let effective_to = parse_optional_date(
            json_extract_string_alias(args, &["effective-to", "effective_to"]),
            "effective-to",
        )?;

        const ALLOWED_RELATIONSHIP_TYPES: &[&str] = &[
            "FEEDER",
            "PARALLEL",
            "AGGREGATOR",
            "MASTER",
            "CO_INVEST_VEHICLE",
        ];
        if !ALLOWED_RELATIONSHIP_TYPES.contains(&relationship_type.as_str()) {
            return Err(anyhow::anyhow!(
                "cbu.link-structure: unsupported relationship-type '{}'",
                relationship_type_raw
            ));
        }
        if let Some(flow) = capital_flow.as_deref() {
            const ALLOWED_CAPITAL_FLOWS: &[&str] = &["UPSTREAM", "DOWNSTREAM", "CO_INVEST"];
            if !ALLOWED_CAPITAL_FLOWS.contains(&flow) {
                return Err(anyhow::anyhow!(
                    "cbu.link-structure: unsupported capital-flow '{}'",
                    capital_flow_raw.unwrap_or_default()
                ));
            }
        }

        let parent_exists: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT cbu_id FROM "ob-poc".cbus WHERE cbu_id = $1 AND deleted_at IS NULL"#,
        )
        .bind(parent_cbu_id)
        .fetch_optional(pool)
        .await?;
        if parent_exists.is_none() {
            return Err(anyhow::anyhow!(
                "cbu.link-structure: parent CBU not found: {}",
                parent_cbu_id
            ));
        }

        let child_exists: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT cbu_id FROM "ob-poc".cbus WHERE cbu_id = $1 AND deleted_at IS NULL"#,
        )
        .bind(child_cbu_id)
        .fetch_optional(pool)
        .await?;
        if child_exists.is_none() {
            return Err(anyhow::anyhow!(
                "cbu.link-structure: child CBU not found: {}",
                child_cbu_id
            ));
        }

        let mut tx = pool.begin().await?;

        let existing_link_id: Option<Uuid> = sqlx::query_scalar(
            r#"
            SELECT link_id
            FROM "ob-poc".cbu_structure_links
            WHERE parent_cbu_id = $1
              AND child_cbu_id = $2
              AND relationship_type = $3
              AND status = 'ACTIVE'
            LIMIT 1
            "#,
        )
        .bind(parent_cbu_id)
        .bind(child_cbu_id)
        .bind(&relationship_type)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(link_id) = existing_link_id {
            sqlx::query(
                r#"
                UPDATE "ob-poc".cbu_structure_links
                SET relationship_selector = $2,
                    capital_flow = $3,
                    effective_from = $4,
                    effective_to = $5,
                    status = 'ACTIVE',
                    updated_at = NOW()
                WHERE link_id = $1
                "#,
            )
            .bind(link_id)
            .bind(&relationship_selector)
            .bind(&capital_flow)
            .bind(effective_from)
            .bind(effective_to)
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".cbu_structure_links (
                    parent_cbu_id,
                    child_cbu_id,
                    relationship_type,
                    relationship_selector,
                    status,
                    capital_flow,
                    effective_from,
                    effective_to
                )
                VALUES ($1, $2, $3, $4, 'ACTIVE', $5, $6, $7)
                "#,
            )
            .bind(parent_cbu_id)
            .bind(child_cbu_id)
            .bind(&relationship_type)
            .bind(&relationship_selector)
            .bind(&capital_flow)
            .bind(effective_from)
            .bind(effective_to)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        Ok(dsl_runtime::VerbExecutionOutcome::Uuid(
            child_cbu_id,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// List persisted structure links for a parent or child CBU.
#[register_custom_op]
pub struct CbuListStructureLinksOp;

#[async_trait]
impl CustomOperation for CbuListStructureLinksOp {
    fn domain(&self) -> &'static str {
        "cbu"
    }

    fn verb(&self) -> &'static str {
        "list-structure-links"
    }

    fn rationale(&self) -> &'static str {
        "Reads persisted cross-border structure relationships for diagnostics and macros"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let parent_cbu_id = extract_uuid_alias(verb_call, ctx, &["parent-cbu-id", "parent_cbu_id"]);
        let child_cbu_id = extract_uuid_alias(verb_call, ctx, &["child-cbu-id", "child_cbu_id"]);
        let cbu_id = extract_uuid_alias(verb_call, ctx, &["cbu-id", "cbu_id"]);
        let direction = extract_string_alias(verb_call, &["direction"])
            .unwrap_or_else(|| String::from("parent"))
            .to_ascii_lowercase();
        let status = extract_string_alias(verb_call, &["status"]);

        if !matches!(direction.as_str(), "parent" | "child") {
            return Err(anyhow::anyhow!(
                "cbu.list-structure-links: direction must be 'parent' or 'child'"
            ));
        }

        let (parent_cbu_id, child_cbu_id) = match (parent_cbu_id, child_cbu_id, cbu_id) {
            (Some(parent), child, _) => (Some(parent), child),
            (None, Some(child), _) => (None, Some(child)),
            (None, None, Some(cbu_id)) if direction == "child" => (None, Some(cbu_id)),
            (None, None, Some(cbu_id)) => (Some(cbu_id), None),
            (None, None, None) => (None, None),
        };

        if parent_cbu_id.is_none() && child_cbu_id.is_none() {
            return Err(anyhow::anyhow!(
                "cbu.list-structure-links: one of :cbu-id, :parent-cbu-id or :child-cbu-id is required"
            ));
        }

        let rows = match (parent_cbu_id, child_cbu_id, status) {
            (Some(parent), Some(child), Some(status)) => {
                sqlx::query_as::<
                    _,
                    (
                        Uuid,
                        Uuid,
                        String,
                        Uuid,
                        String,
                        String,
                        String,
                        String,
                        Option<String>,
                        Option<NaiveDate>,
                        Option<NaiveDate>,
                    ),
                >(
                    r#"
                    SELECT
                        l.link_id,
                        l.parent_cbu_id,
                        p.name,
                        l.child_cbu_id,
                        c.name,
                        l.relationship_type,
                        l.relationship_selector,
                        l.status,
                        l.capital_flow,
                        l.effective_from,
                        l.effective_to
                    FROM "ob-poc".cbu_structure_links l
                    JOIN "ob-poc".cbus p ON p.cbu_id = l.parent_cbu_id
                    JOIN "ob-poc".cbus c ON c.cbu_id = l.child_cbu_id
                    WHERE l.parent_cbu_id = $1
                      AND l.child_cbu_id = $2
                      AND l.status = $3
                    ORDER BY l.created_at DESC
                    "#,
                )
                .bind(parent)
                .bind(child)
                .bind(status.to_ascii_uppercase())
                .fetch_all(pool)
                .await?
            }
            (Some(parent), Some(child), None) => {
                sqlx::query_as::<
                    _,
                    (
                        Uuid,
                        Uuid,
                        String,
                        Uuid,
                        String,
                        String,
                        String,
                        String,
                        Option<String>,
                        Option<NaiveDate>,
                        Option<NaiveDate>,
                    ),
                >(
                    r#"
                    SELECT
                        l.link_id,
                        l.parent_cbu_id,
                        p.name,
                        l.child_cbu_id,
                        c.name,
                        l.relationship_type,
                        l.relationship_selector,
                        l.status,
                        l.capital_flow,
                        l.effective_from,
                        l.effective_to
                    FROM "ob-poc".cbu_structure_links l
                    JOIN "ob-poc".cbus p ON p.cbu_id = l.parent_cbu_id
                    JOIN "ob-poc".cbus c ON c.cbu_id = l.child_cbu_id
                    WHERE l.parent_cbu_id = $1
                      AND l.child_cbu_id = $2
                    ORDER BY l.created_at DESC
                    "#,
                )
                .bind(parent)
                .bind(child)
                .fetch_all(pool)
                .await?
            }
            (Some(parent), None, Some(status)) => {
                sqlx::query_as::<
                    _,
                    (
                        Uuid,
                        Uuid,
                        String,
                        Uuid,
                        String,
                        String,
                        String,
                        String,
                        Option<String>,
                        Option<NaiveDate>,
                        Option<NaiveDate>,
                    ),
                >(
                    r#"
                    SELECT
                        l.link_id,
                        l.parent_cbu_id,
                        p.name,
                        l.child_cbu_id,
                        c.name,
                        l.relationship_type,
                        l.relationship_selector,
                        l.status,
                        l.capital_flow,
                        l.effective_from,
                        l.effective_to
                    FROM "ob-poc".cbu_structure_links l
                    JOIN "ob-poc".cbus p ON p.cbu_id = l.parent_cbu_id
                    JOIN "ob-poc".cbus c ON c.cbu_id = l.child_cbu_id
                    WHERE l.parent_cbu_id = $1
                      AND l.status = $2
                    ORDER BY l.created_at DESC
                    "#,
                )
                .bind(parent)
                .bind(status.to_ascii_uppercase())
                .fetch_all(pool)
                .await?
            }
            (Some(parent), None, None) => {
                sqlx::query_as::<
                    _,
                    (
                        Uuid,
                        Uuid,
                        String,
                        Uuid,
                        String,
                        String,
                        String,
                        String,
                        Option<String>,
                        Option<NaiveDate>,
                        Option<NaiveDate>,
                    ),
                >(
                    r#"
                    SELECT
                        l.link_id,
                        l.parent_cbu_id,
                        p.name,
                        l.child_cbu_id,
                        c.name,
                        l.relationship_type,
                        l.relationship_selector,
                        l.status,
                        l.capital_flow,
                        l.effective_from,
                        l.effective_to
                    FROM "ob-poc".cbu_structure_links l
                    JOIN "ob-poc".cbus p ON p.cbu_id = l.parent_cbu_id
                    JOIN "ob-poc".cbus c ON c.cbu_id = l.child_cbu_id
                    WHERE l.parent_cbu_id = $1
                    ORDER BY l.created_at DESC
                    "#,
                )
                .bind(parent)
                .fetch_all(pool)
                .await?
            }
            (None, Some(child), Some(status)) => {
                sqlx::query_as::<
                    _,
                    (
                        Uuid,
                        Uuid,
                        String,
                        Uuid,
                        String,
                        String,
                        String,
                        String,
                        Option<String>,
                        Option<NaiveDate>,
                        Option<NaiveDate>,
                    ),
                >(
                    r#"
                    SELECT
                        l.link_id,
                        l.parent_cbu_id,
                        p.name,
                        l.child_cbu_id,
                        c.name,
                        l.relationship_type,
                        l.relationship_selector,
                        l.status,
                        l.capital_flow,
                        l.effective_from,
                        l.effective_to
                    FROM "ob-poc".cbu_structure_links l
                    JOIN "ob-poc".cbus p ON p.cbu_id = l.parent_cbu_id
                    JOIN "ob-poc".cbus c ON c.cbu_id = l.child_cbu_id
                    WHERE l.child_cbu_id = $1
                      AND l.status = $2
                    ORDER BY l.created_at DESC
                    "#,
                )
                .bind(child)
                .bind(status.to_ascii_uppercase())
                .fetch_all(pool)
                .await?
            }
            (None, Some(child), None) => {
                sqlx::query_as::<
                    _,
                    (
                        Uuid,
                        Uuid,
                        String,
                        Uuid,
                        String,
                        String,
                        String,
                        String,
                        Option<String>,
                        Option<NaiveDate>,
                        Option<NaiveDate>,
                    ),
                >(
                    r#"
                    SELECT
                        l.link_id,
                        l.parent_cbu_id,
                        p.name,
                        l.child_cbu_id,
                        c.name,
                        l.relationship_type,
                        l.relationship_selector,
                        l.status,
                        l.capital_flow,
                        l.effective_from,
                        l.effective_to
                    FROM "ob-poc".cbu_structure_links l
                    JOIN "ob-poc".cbus p ON p.cbu_id = l.parent_cbu_id
                    JOIN "ob-poc".cbus c ON c.cbu_id = l.child_cbu_id
                    WHERE l.child_cbu_id = $1
                    ORDER BY l.created_at DESC
                    "#,
                )
                .bind(child)
                .fetch_all(pool)
                .await?
            }
            (None, None, _) => unreachable!(),
        };

        Ok(ExecutionResult::RecordSet(
            rows.into_iter()
                .map(
                    |(
                        link_id,
                        parent_cbu_id,
                        parent_name,
                        child_cbu_id,
                        child_name,
                        relationship_type,
                        relationship_selector,
                        status,
                        capital_flow,
                        effective_from,
                        effective_to,
                    )| {
                        serde_json::json!({
                            "link_id": link_id,
                            "parent_cbu_id": parent_cbu_id,
                            "parent_name": parent_name,
                            "child_cbu_id": child_cbu_id,
                            "child_name": child_name,
                            "relationship_type": relationship_type,
                            "relationship_selector": relationship_selector,
                            "status": status,
                            "capital_flow": capital_flow,
                            "effective_from": effective_from.map(|value| value.to_string()),
                            "effective_to": effective_to.map(|value| value.to_string()),
                        })
                    },
                )
                .collect(),
        ))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "error": "Database feature required for cbu.list-structure-links"
        })))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let parent_cbu_id =
            json_extract_uuid_alias(args, ctx, &["parent-cbu-id", "parent_cbu_id"])?;
        let child_cbu_id = json_extract_uuid_alias(args, ctx, &["child-cbu-id", "child_cbu_id"])?;
        let cbu_id = json_extract_uuid_alias(args, ctx, &["cbu-id", "cbu_id"])?;
        let direction = json_extract_string_alias(args, &["direction"])
            .unwrap_or_else(|| String::from("parent"))
            .to_ascii_lowercase();
        let status = json_extract_string_alias(args, &["status"]);

        if !matches!(direction.as_str(), "parent" | "child") {
            return Err(anyhow::anyhow!(
                "cbu.list-structure-links: direction must be 'parent' or 'child'"
            ));
        }

        let (parent_cbu_id, child_cbu_id) = match (parent_cbu_id, child_cbu_id, cbu_id) {
            (Some(parent), child, _) => (Some(parent), child),
            (None, Some(child), _) => (None, Some(child)),
            (None, None, Some(cbu_id)) if direction == "child" => (None, Some(cbu_id)),
            (None, None, Some(cbu_id)) => (Some(cbu_id), None),
            (None, None, None) => (None, None),
        };

        if parent_cbu_id.is_none() && child_cbu_id.is_none() {
            return Err(anyhow::anyhow!(
                "cbu.list-structure-links: one of :cbu-id, :parent-cbu-id or :child-cbu-id is required"
            ));
        }

        let rows = match (parent_cbu_id, child_cbu_id, status) {
            (Some(parent), Some(child), Some(status)) => {
                sqlx::query_as::<
                    _,
                    (
                        Uuid,
                        Uuid,
                        String,
                        Uuid,
                        String,
                        String,
                        String,
                        String,
                        Option<String>,
                        Option<NaiveDate>,
                        Option<NaiveDate>,
                    ),
                >(
                    r#"
                    SELECT
                        l.link_id,
                        l.parent_cbu_id,
                        p.name,
                        l.child_cbu_id,
                        c.name,
                        l.relationship_type,
                        l.relationship_selector,
                        l.status,
                        l.capital_flow,
                        l.effective_from,
                        l.effective_to
                    FROM "ob-poc".cbu_structure_links l
                    JOIN "ob-poc".cbus p ON p.cbu_id = l.parent_cbu_id
                    JOIN "ob-poc".cbus c ON c.cbu_id = l.child_cbu_id
                    WHERE l.parent_cbu_id = $1
                      AND l.child_cbu_id = $2
                      AND l.status = $3
                    ORDER BY l.created_at DESC
                    "#,
                )
                .bind(parent)
                .bind(child)
                .bind(status.to_ascii_uppercase())
                .fetch_all(pool)
                .await?
            }
            (Some(parent), Some(child), None) => {
                sqlx::query_as::<
                    _,
                    (
                        Uuid,
                        Uuid,
                        String,
                        Uuid,
                        String,
                        String,
                        String,
                        String,
                        Option<String>,
                        Option<NaiveDate>,
                        Option<NaiveDate>,
                    ),
                >(
                    r#"
                    SELECT
                        l.link_id,
                        l.parent_cbu_id,
                        p.name,
                        l.child_cbu_id,
                        c.name,
                        l.relationship_type,
                        l.relationship_selector,
                        l.status,
                        l.capital_flow,
                        l.effective_from,
                        l.effective_to
                    FROM "ob-poc".cbu_structure_links l
                    JOIN "ob-poc".cbus p ON p.cbu_id = l.parent_cbu_id
                    JOIN "ob-poc".cbus c ON c.cbu_id = l.child_cbu_id
                    WHERE l.parent_cbu_id = $1
                      AND l.child_cbu_id = $2
                    ORDER BY l.created_at DESC
                    "#,
                )
                .bind(parent)
                .bind(child)
                .fetch_all(pool)
                .await?
            }
            (Some(parent), None, Some(status)) => {
                sqlx::query_as::<
                    _,
                    (
                        Uuid,
                        Uuid,
                        String,
                        Uuid,
                        String,
                        String,
                        String,
                        String,
                        Option<String>,
                        Option<NaiveDate>,
                        Option<NaiveDate>,
                    ),
                >(
                    r#"
                    SELECT
                        l.link_id,
                        l.parent_cbu_id,
                        p.name,
                        l.child_cbu_id,
                        c.name,
                        l.relationship_type,
                        l.relationship_selector,
                        l.status,
                        l.capital_flow,
                        l.effective_from,
                        l.effective_to
                    FROM "ob-poc".cbu_structure_links l
                    JOIN "ob-poc".cbus p ON p.cbu_id = l.parent_cbu_id
                    JOIN "ob-poc".cbus c ON c.cbu_id = l.child_cbu_id
                    WHERE l.parent_cbu_id = $1
                      AND l.status = $2
                    ORDER BY l.created_at DESC
                    "#,
                )
                .bind(parent)
                .bind(status.to_ascii_uppercase())
                .fetch_all(pool)
                .await?
            }
            (Some(parent), None, None) => {
                sqlx::query_as::<
                    _,
                    (
                        Uuid,
                        Uuid,
                        String,
                        Uuid,
                        String,
                        String,
                        String,
                        String,
                        Option<String>,
                        Option<NaiveDate>,
                        Option<NaiveDate>,
                    ),
                >(
                    r#"
                    SELECT
                        l.link_id,
                        l.parent_cbu_id,
                        p.name,
                        l.child_cbu_id,
                        c.name,
                        l.relationship_type,
                        l.relationship_selector,
                        l.status,
                        l.capital_flow,
                        l.effective_from,
                        l.effective_to
                    FROM "ob-poc".cbu_structure_links l
                    JOIN "ob-poc".cbus p ON p.cbu_id = l.parent_cbu_id
                    JOIN "ob-poc".cbus c ON c.cbu_id = l.child_cbu_id
                    WHERE l.parent_cbu_id = $1
                    ORDER BY l.created_at DESC
                    "#,
                )
                .bind(parent)
                .fetch_all(pool)
                .await?
            }
            (None, Some(child), Some(status)) => {
                sqlx::query_as::<
                    _,
                    (
                        Uuid,
                        Uuid,
                        String,
                        Uuid,
                        String,
                        String,
                        String,
                        String,
                        Option<String>,
                        Option<NaiveDate>,
                        Option<NaiveDate>,
                    ),
                >(
                    r#"
                    SELECT
                        l.link_id,
                        l.parent_cbu_id,
                        p.name,
                        l.child_cbu_id,
                        c.name,
                        l.relationship_type,
                        l.relationship_selector,
                        l.status,
                        l.capital_flow,
                        l.effective_from,
                        l.effective_to
                    FROM "ob-poc".cbu_structure_links l
                    JOIN "ob-poc".cbus p ON p.cbu_id = l.parent_cbu_id
                    JOIN "ob-poc".cbus c ON c.cbu_id = l.child_cbu_id
                    WHERE l.child_cbu_id = $1
                      AND l.status = $2
                    ORDER BY l.created_at DESC
                    "#,
                )
                .bind(child)
                .bind(status.to_ascii_uppercase())
                .fetch_all(pool)
                .await?
            }
            (None, Some(child), None) => {
                sqlx::query_as::<
                    _,
                    (
                        Uuid,
                        Uuid,
                        String,
                        Uuid,
                        String,
                        String,
                        String,
                        String,
                        Option<String>,
                        Option<NaiveDate>,
                        Option<NaiveDate>,
                    ),
                >(
                    r#"
                    SELECT
                        l.link_id,
                        l.parent_cbu_id,
                        p.name,
                        l.child_cbu_id,
                        c.name,
                        l.relationship_type,
                        l.relationship_selector,
                        l.status,
                        l.capital_flow,
                        l.effective_from,
                        l.effective_to
                    FROM "ob-poc".cbu_structure_links l
                    JOIN "ob-poc".cbus p ON p.cbu_id = l.parent_cbu_id
                    JOIN "ob-poc".cbus c ON c.cbu_id = l.child_cbu_id
                    WHERE l.child_cbu_id = $1
                    ORDER BY l.created_at DESC
                    "#,
                )
                .bind(child)
                .fetch_all(pool)
                .await?
            }
            (None, None, _) => unreachable!(),
        };

        Ok(dsl_runtime::VerbExecutionOutcome::RecordSet(
            rows.into_iter()
                .map(
                    |(
                        link_id,
                        parent_cbu_id,
                        parent_name,
                        child_cbu_id,
                        child_name,
                        relationship_type,
                        relationship_selector,
                        status,
                        capital_flow,
                        effective_from,
                        effective_to,
                    )| {
                        serde_json::json!({
                            "link_id": link_id,
                            "parent_cbu_id": parent_cbu_id,
                            "parent_name": parent_name,
                            "child_cbu_id": child_cbu_id,
                            "child_name": child_name,
                            "relationship_type": relationship_type,
                            "relationship_selector": relationship_selector,
                            "status": status,
                            "capital_flow": capital_flow,
                            "effective_from": effective_from.map(|value| value.to_string()),
                            "effective_to": effective_to.map(|value| value.to_string()),
                        })
                    },
                )
                .collect(),
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Terminate an active persisted structure link between CBUs.
#[register_custom_op]
pub struct CbuUnlinkStructureOp;

#[async_trait]
impl CustomOperation for CbuUnlinkStructureOp {
    fn domain(&self) -> &'static str {
        "cbu"
    }

    fn verb(&self) -> &'static str {
        "unlink-structure"
    }

    fn rationale(&self) -> &'static str {
        "Terminates an active cross-border structure relationship without deleting history"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let link_id = get_required_uuid(verb_call, "link-id")?;
        let reason = extract_string_alias(verb_call, &["reason"]).ok_or_else(|| {
            anyhow::anyhow!("cbu.unlink-structure: missing required argument :reason")
        })?;

        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".cbu_structure_links
            SET status = 'TERMINATED',
                terminated_at = NOW(),
                terminated_reason = $2,
                updated_at = NOW()
            WHERE link_id = $1
              AND status = 'ACTIVE'
            "#,
        )
        .bind(link_id)
        .bind(reason)
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Affected(result.rows_affected()))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "error": "Database feature required for cbu.unlink-structure"
        })))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let link_id = json_extract_uuid(args, ctx, "link-id")?;
        let reason = json_extract_string(args, "reason")?;

        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".cbu_structure_links
            SET status = 'TERMINATED',
                terminated_at = NOW(),
                terminated_reason = $2,
                updated_at = NOW()
            WHERE link_id = $1
              AND status = 'ACTIVE'
            "#,
        )
        .bind(link_id)
        .bind(reason)
        .execute(pool)
        .await?;

        Ok(dsl_runtime::VerbExecutionOutcome::Affected(
            result.rows_affected(),
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

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
                    r#"SELECT cbu_id, name
                       FROM "ob-poc".cbus
                       WHERE cbu_id = $1
                         AND deleted_at IS NULL"#,
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
                    r#"SELECT cbu_id, name
                       FROM "ob-poc".cbus
                       WHERE cbu_id = $1
                         AND deleted_at IS NULL"#,
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
                        r#"SELECT cbu_id, name
                           FROM "ob-poc".cbus
                           WHERE cbu_id = $1
                             AND deleted_at IS NULL"#,
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
                        r#"SELECT cbu_id, name
                           FROM "ob-poc".cbus
                           WHERE LOWER(name) = LOWER($1)
                             AND deleted_at IS NULL"#,
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

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use uuid::Uuid;

        let product_name = json_extract_string(args, "product")?;
        let cbu_id_arg = args
            .get("cbu-id")
            .ok_or_else(|| anyhow::anyhow!("cbu.add-product: Missing required argument :cbu-id"))?;

        let (cbu_id, cbu_name): (Uuid, String) = if let Some(str_val) = cbu_id_arg.as_str() {
            if str_val.starts_with('@') {
                let resolved_id = json_extract_uuid(args, ctx, "cbu-id")?;
                let row: (Uuid, String) = sqlx::query_as(
                    r#"SELECT cbu_id, name
                       FROM "ob-poc".cbus
                       WHERE cbu_id = $1
                         AND deleted_at IS NULL"#,
                )
                .bind(resolved_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!("cbu.add-product: CBU not found with id {}", resolved_id)
                })?;
                row
            } else if let Ok(uuid_val) = Uuid::parse_str(str_val) {
                let row: (Uuid, String) = sqlx::query_as(
                    r#"SELECT cbu_id, name
                       FROM "ob-poc".cbus
                       WHERE cbu_id = $1
                         AND deleted_at IS NULL"#,
                )
                .bind(uuid_val)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!("cbu.add-product: CBU not found with id {}", uuid_val)
                })?;
                row
            } else {
                let row: (Uuid, String) = sqlx::query_as(
                    r#"SELECT cbu_id, name
                       FROM "ob-poc".cbus
                       WHERE LOWER(name) = LOWER($1)
                         AND deleted_at IS NULL"#,
                )
                .bind(str_val)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "cbu.add-product: CBU '{}' not found. Use cbu.list to see available CBUs.",
                        str_val
                    )
                })?;
                row
            }
        } else {
            let resolved_id = json_extract_uuid(args, ctx, "cbu-id")?;
            let row: (Uuid, String) = sqlx::query_as(
                r#"SELECT cbu_id, name
                   FROM "ob-poc".cbus
                   WHERE cbu_id = $1
                     AND deleted_at IS NULL"#,
            )
            .bind(resolved_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("cbu.add-product: CBU not found with id {}", resolved_id)
            })?;
            row
        };

        let product_row: Option<(Uuid, String, String)> = sqlx::query_as(
            r#"SELECT product_id, name, product_code FROM "ob-poc".products WHERE product_code = $1"#,
        )
        .bind(&product_name)
        .fetch_optional(pool)
        .await?;

        let product = product_row.ok_or_else(|| {
            anyhow::anyhow!(
                "cbu.add-product: Product '{}' not found. Use product codes: CUSTODY, FUND_ACCOUNTING, TRANSFER_AGENCY, MIDDLE_OFFICE, COLLATERAL_MGMT, MARKETS_FX, ALTS",
                product_name
            )
        })?;

        let product_id = product.0;

        let services: Vec<(Uuid, String)> = sqlx::query_as(
            r#"SELECT ps.service_id, s.name as service_name
               FROM "ob-poc".product_services ps
               JOIN "ob-poc".services s ON ps.service_id = s.service_id
               WHERE ps.product_id = $1
               ORDER BY s.name"#,
        )
        .bind(product_id)
        .fetch_all(pool)
        .await?;

        if services.is_empty() {
            return Err(anyhow::anyhow!(
                "cbu.add-product: Product '{}' has no services defined in product_services. \
                 Cannot add product without services.",
                product_name
            ));
        }

        let mut tx = pool.begin().await?;
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
            .bind(svc.0)
            .execute(&mut *tx)
            .await?;

            if result.rows_affected() > 0 {
                delivery_created += 1;
            } else {
                delivery_skipped += 1;
            }
        }

        let mut resource_created: i64 = 0;
        let mut resource_skipped: i64 = 0;
        let service_resources: Vec<(Uuid, Uuid, Option<String>, String)> = sqlx::query_as(
            r#"SELECT src.service_id, src.resource_id, srt.resource_code, srt.name as resource_name
               FROM "ob-poc".service_resource_capabilities src
               JOIN "ob-poc".service_resource_types srt ON src.resource_id = srt.resource_id
               WHERE src.service_id IN (
                   SELECT service_id FROM "ob-poc".product_services WHERE product_id = $1
               )
               AND src.is_active = true
               ORDER BY src.service_id, srt.name"#,
        )
        .bind(product_id)
        .fetch_all(&mut *tx)
        .await?;

        for sr in &service_resources {
            let instance_id = Uuid::new_v4();
            let instance_url = format!(
                "urn:ob-poc:{}:{}:{}",
                cbu_name.to_lowercase().replace(' ', "-"),
                sr.2.as_deref().unwrap_or("unknown"),
                &instance_id.to_string()[..8]
            );

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
            .bind(sr.0)
            .bind(sr.1)
            .bind(&instance_url)
            .bind(&sr.3)
            .execute(&mut *tx)
            .await?;

            if result.rows_affected() > 0 {
                resource_created += 1;
            } else {
                resource_skipped += 1;
            }
        }

        tx.commit().await?;

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

        Ok(dsl_runtime::VerbExecutionOutcome::Affected(
            (delivery_created + resource_created) as u64,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
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
pub struct CbuInspectOp;

#[async_trait]
impl CustomOperation for CbuInspectOp {
    fn domain(&self) -> &'static str {
        "cbu"
    }
    fn verb(&self) -> &'static str {
        "inspect"
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
        use super::helpers::{extract_string_opt, extract_uuid};
        let cbu_id = extract_uuid(verb_call, ctx, "cbu-id")?;
        let as_of_date = extract_string_opt(verb_call, "as-of-date")
            .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let result = cbu_inspect_impl(cbu_id, as_of_date, pool).await?;
        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "error": "Database required for cbu.inspect"
        })))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_string_opt, json_extract_uuid};
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let as_of_date = json_extract_string_opt(args, "as-of-date")
            .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let result = cbu_inspect_impl(cbu_id, as_of_date, pool).await?;
        Ok(dsl_runtime::VerbExecutionOutcome::Record(result))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Shared implementation for cbu.inspect — called by both execute() and execute_json().
#[cfg(feature = "database")]
async fn cbu_inspect_impl(
    cbu_id: Uuid,
    as_of_date: NaiveDate,
    pool: &PgPool,
) -> Result<serde_json::Value> {
    let cbu = sqlx::query!(
        r#"SELECT cbu_id, name, jurisdiction, client_type, cbu_category,
                  nature_purpose, description, created_at, updated_at
           FROM "ob-poc".cbus WHERE cbu_id = $1 AND deleted_at IS NULL"#,
        cbu_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow::anyhow!("CBU not found: {}", cbu_id))?;

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
           WHERE cer.cbu_id = $1 AND e.deleted_at IS NULL
             AND (cer.effective_from IS NULL OR cer.effective_from <= $2)
             AND (cer.effective_to IS NULL OR cer.effective_to >= $2)
           ORDER BY e.name"#,
        cbu_id, as_of_date
    )
    .fetch_all(pool)
    .await?;

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

    let entity_list: Vec<serde_json::Value> = entities
        .iter()
        .map(|e| {
            let entity_roles: Vec<String> = roles
                .iter()
                .filter(|r| r.entity_id == e.entity_id)
                .map(|r| r.role_name.clone())
                .collect();
            serde_json::json!({
                "entity_id": e.entity_id, "name": e.name,
                "entity_type": e.entity_type, "jurisdiction": e.jurisdiction,
                "roles": entity_roles
            })
        })
        .collect();

    let documents = sqlx::query!(
        r#"SELECT dc.doc_id, dc.document_name, dt.type_code, dt.display_name, dc.status
           FROM "ob-poc".document_catalog dc
           LEFT JOIN "ob-poc".document_types dt ON dc.document_type_id = dt.type_id
           WHERE dc.cbu_id = $1 ORDER BY dt.type_code"#,
        cbu_id
    )
    .fetch_all(pool)
    .await?;

    let doc_list: Vec<serde_json::Value> = documents
        .iter()
        .map(|d| {
            serde_json::json!({"doc_id": d.doc_id, "name": d.document_name,
            "type_code": d.type_code, "type_name": d.display_name, "status": d.status})
        })
        .collect();

    let screenings = sqlx::query!(
        r#"SELECT s.screening_id, w.entity_id, e.name as entity_name,
                  s.screening_type, s.status, s.result_summary
           FROM "ob-poc".screenings s
           JOIN "ob-poc".entity_workstreams w ON w.workstream_id = s.workstream_id
           JOIN "ob-poc".cases c ON c.case_id = w.case_id
           JOIN "ob-poc".entities e ON e.entity_id = w.entity_id
           WHERE c.cbu_id = $1 AND e.deleted_at IS NULL
           ORDER BY s.screening_type, e.name"#,
        cbu_id
    )
    .fetch_all(pool)
    .await?;

    let screening_list: Vec<serde_json::Value> = screenings
        .iter()
        .map(|s| {
            serde_json::json!({"screening_id": s.screening_id, "entity_id": s.entity_id,
            "entity_name": s.entity_name, "screening_type": s.screening_type,
            "status": s.status, "result": s.result_summary})
        })
        .collect();

    let services = sqlx::query!(
        r#"SELECT sdm.delivery_id, p.name as product_name, p.product_code,
                  s.name as service_name, sdm.delivery_status
           FROM "ob-poc".service_delivery_map sdm
           JOIN "ob-poc".products p ON p.product_id = sdm.product_id
           JOIN "ob-poc".services s ON s.service_id = sdm.service_id
           WHERE sdm.cbu_id = $1 ORDER BY p.name, s.name"#,
        cbu_id
    )
    .fetch_all(pool)
    .await?;

    let service_list: Vec<serde_json::Value> = services
        .iter()
        .map(|s| {
            serde_json::json!({"delivery_id": s.delivery_id, "product": s.product_name,
            "product_code": s.product_code, "service": s.service_name, "status": s.delivery_status})
        })
        .collect();

    let cases = sqlx::query!(
        r#"SELECT case_id, status, case_type, risk_rating, escalation_level,
                  opened_at, closed_at
           FROM "ob-poc".cases WHERE cbu_id = $1 ORDER BY opened_at DESC"#,
        cbu_id
    )
    .fetch_all(pool)
    .await?;

    let case_list: Vec<serde_json::Value> = cases
        .iter()
        .map(|c| {
            serde_json::json!({"case_id": c.case_id, "status": c.status,
            "case_type": c.case_type, "risk_rating": c.risk_rating,
            "escalation_level": c.escalation_level,
            "opened_at": c.opened_at.to_rfc3339(),
            "closed_at": c.closed_at.map(|t| t.to_rfc3339())})
        })
        .collect();

    Ok(serde_json::json!({
        "cbu_id": cbu.cbu_id, "name": cbu.name,
        "jurisdiction": cbu.jurisdiction, "client_type": cbu.client_type,
        "category": cbu.cbu_category, "nature_purpose": cbu.nature_purpose,
        "description": cbu.description,
        "created_at": cbu.created_at.map(|t| t.to_rfc3339()),
        "updated_at": cbu.updated_at.map(|t| t.to_rfc3339()),
        "as_of_date": as_of_date.to_string(),
        "entities": entity_list, "documents": doc_list,
        "screenings": screening_list, "services": service_list,
        "kyc_cases": case_list,
        "summary": {
            "entity_count": entity_list.len(), "document_count": doc_list.len(),
            "screening_count": screening_list.len(), "service_count": service_list.len(),
            "case_count": case_list.len()
        }
    }))
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
            r#"SELECT name, status
               FROM "ob-poc".cbus
               WHERE cbu_id = $1
                 AND deleted_at IS NULL"#,
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
                    r#"SELECT case_id FROM "ob-poc".cases
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
            r#"UPDATE "ob-poc".cbus
               SET status = $1, updated_at = now()
               WHERE cbu_id = $2
                 AND deleted_at IS NULL"#,
            new_cbu_status,
            cbu_id
        )
        .execute(&mut *tx)
        .await?;

        // 2. Update case status
        let should_close = matches!(decision, "APPROVED" | "REJECTED");
        if should_close {
            sqlx::query!(
                r#"UPDATE "ob-poc".cases SET status = $1, closed_at = now(), last_activity_at = now() WHERE case_id = $2"#,
                new_case_status,
                case_id
            )
            .execute(&mut *tx)
            .await?;
        } else {
            // REFERRED - update escalation level
            sqlx::query!(
                r#"UPDATE "ob-poc".cases SET escalation_level = 'SENIOR_COMPLIANCE', last_activity_at = now() WHERE case_id = $1"#,
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

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let decision = json_extract_string(args, "decision")?;
        let decided_by = json_extract_string(args, "decided-by")?;
        let rationale = json_extract_string(args, "rationale")?;
        let case_id = if args.get("case-id").is_some() {
            Some(json_extract_uuid(args, ctx, "case-id")?)
        } else {
            None
        };
        let conditions = json_extract_string_opt(args, "conditions");
        let escalation_reason = json_extract_string_opt(args, "escalation-reason");

        if decision == "REFERRED" && escalation_reason.is_none() {
            return Err(anyhow::anyhow!(
                "escalation-reason is required when decision is REFERRED"
            ));
        }

        let cbu = sqlx::query!(
            r#"SELECT name, status
               FROM "ob-poc".cbus
               WHERE cbu_id = $1
                 AND deleted_at IS NULL"#,
            cbu_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("CBU not found: {}", cbu_id))?;

        let new_cbu_status = match decision.as_str() {
            "APPROVED" => "VALIDATED",
            "REJECTED" => "VALIDATION_FAILED",
            "REFERRED" => "VALIDATION_PENDING",
            _ => return Err(anyhow::anyhow!("Invalid decision: {}", decision)),
        };

        let new_case_status = match decision.as_str() {
            "APPROVED" => "APPROVED",
            "REJECTED" => "REJECTED",
            "REFERRED" => "REVIEW",
            _ => "REVIEW",
        };

        let case_id = match case_id {
            Some(id) => id,
            None => {
                let row = sqlx::query!(
                    r#"SELECT case_id FROM "ob-poc".cases
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

        let mut tx = pool.begin().await?;

        sqlx::query!(
            r#"UPDATE "ob-poc".cbus
               SET status = $1, updated_at = now()
               WHERE cbu_id = $2
                 AND deleted_at IS NULL"#,
            new_cbu_status,
            cbu_id
        )
        .execute(&mut *tx)
        .await?;

        let should_close = matches!(decision.as_str(), "APPROVED" | "REJECTED");
        if should_close {
            sqlx::query!(
                r#"UPDATE "ob-poc".cases SET status = $1, closed_at = now(), last_activity_at = now() WHERE case_id = $2"#,
                new_case_status,
                case_id
            )
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query!(
                r#"UPDATE "ob-poc".cases SET escalation_level = 'SENIOR_COMPLIANCE', last_activity_at = now() WHERE case_id = $1"#,
                case_id
            )
            .execute(&mut *tx)
            .await?;
        }

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

        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            serde_json::json!({
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
            }),
        ))
    }

    fn is_migrated(&self) -> bool {
        true
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
            r#"SELECT name FROM "ob-poc".cbus WHERE cbu_id = $1 AND deleted_at IS NULL"#,
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

        // The historical multi-table hard-delete cascade has drifted too far from
        // the live schema to remain safe. Tier 3 replaces it with a soft-delete
        // flow that detaches the CBU from active runtime paths while preserving
        // root records for audit and recovery.

        let result = sqlx::query(
            r#"UPDATE "ob-poc".client_group_entity
               SET cbu_id = NULL, updated_at = NOW()
               WHERE cbu_id = $1"#,
        )
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;
        deleted_counts.insert(
            "client_group_entity_unlinked".to_string(),
            result.rows_affected() as i64,
        );

        let result = sqlx::query(
            r#"DELETE FROM "ob-poc".cbu_group_members
               WHERE cbu_id = $1"#,
        )
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;
        deleted_counts.insert(
            "cbu_group_members".to_string(),
            result.rows_affected() as i64,
        );

        let result = sqlx::query(
            r#"DELETE FROM "ob-poc".cbu_structure_links
               WHERE parent_cbu_id = $1 OR child_cbu_id = $1"#,
        )
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;
        deleted_counts.insert(
            "cbu_structure_links".to_string(),
            result.rows_affected() as i64,
        );

        let mut entities_deleted: i64 = 0;
        let mut entities_preserved: i64 = 0;

        if delete_entities {
            // Soft-delete entities that are only reachable through this still-active CBU.
            let exclusive_entities: Vec<Uuid> = sqlx::query_scalar(
                r#"SELECT DISTINCT cer.entity_id
                   FROM "ob-poc".cbu_entity_roles cer
                   WHERE cer.cbu_id = $1
                     AND NOT EXISTS (
                         SELECT 1
                         FROM "ob-poc".cbu_entity_roles other
                         JOIN "ob-poc".cbus c ON c.cbu_id = other.cbu_id
                         WHERE other.entity_id = cer.entity_id
                           AND other.cbu_id <> $1
                           AND c.deleted_at IS NULL
                     )"#,
            )
            .bind(cbu_id)
            .fetch_all(&mut *tx)
            .await?;

            let shared_count: Option<i64> = sqlx::query_scalar(
                r#"SELECT COUNT(DISTINCT cer.entity_id)::bigint
                   FROM "ob-poc".cbu_entity_roles cer
                   WHERE cer.cbu_id = $1
                     AND EXISTS (
                         SELECT 1
                         FROM "ob-poc".cbu_entity_roles other
                         JOIN "ob-poc".cbus c ON c.cbu_id = other.cbu_id
                         WHERE other.entity_id = cer.entity_id
                           AND other.cbu_id <> $1
                           AND c.deleted_at IS NULL
                     )"#,
            )
            .bind(cbu_id)
            .fetch_one(&mut *tx)
            .await?;
            entities_preserved = shared_count.unwrap_or(0);

            for entity_id in &exclusive_entities {
                let _ = sqlx::query(
                    r#"UPDATE "ob-poc".entities
                       SET deleted_at = NOW(), updated_at = NOW()
                       WHERE entity_id = $1
                         AND deleted_at IS NULL"#,
                )
                .bind(entity_id)
                .execute(&mut *tx)
                .await;
            }

            entities_deleted = exclusive_entities.len() as i64;
        }

        let result = sqlx::query(r#"DELETE FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&mut *tx)
            .await?;
        deleted_counts.insert(
            "cbu_entity_roles".to_string(),
            result.rows_affected() as i64,
        );

        let result = sqlx::query(
            r#"UPDATE "ob-poc".cbus
               SET deleted_at = NOW(), updated_at = NOW()
               WHERE cbu_id = $1
                 AND deleted_at IS NULL"#,
        )
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

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let delete_entities = json_extract_bool_opt(args, "delete-entities").unwrap_or(true);

        let cbu = sqlx::query!(
            r#"SELECT name FROM "ob-poc".cbus WHERE cbu_id = $1 AND deleted_at IS NULL"#,
            cbu_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("CBU not found: {}", cbu_id))?;

        let cbu_name = cbu.name;
        let mut deleted_counts: std::collections::HashMap<String, i64> =
            std::collections::HashMap::new();
        let mut tx = pool.begin().await?;

        let result = sqlx::query(
            r#"UPDATE "ob-poc".client_group_entity
               SET cbu_id = NULL, updated_at = NOW()
               WHERE cbu_id = $1"#,
        )
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;
        deleted_counts.insert(
            "client_group_entity_unlinked".to_string(),
            result.rows_affected() as i64,
        );

        let result = sqlx::query(
            r#"DELETE FROM "ob-poc".cbu_group_members
               WHERE cbu_id = $1"#,
        )
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;
        deleted_counts.insert(
            "cbu_group_members".to_string(),
            result.rows_affected() as i64,
        );

        let result = sqlx::query(
            r#"DELETE FROM "ob-poc".cbu_structure_links
               WHERE parent_cbu_id = $1 OR child_cbu_id = $1"#,
        )
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;
        deleted_counts.insert(
            "cbu_structure_links".to_string(),
            result.rows_affected() as i64,
        );

        let mut entities_deleted: i64 = 0;
        let mut entities_preserved: i64 = 0;

        if delete_entities {
            let exclusive_entities: Vec<Uuid> = sqlx::query_scalar(
                r#"SELECT DISTINCT cer.entity_id
                   FROM "ob-poc".cbu_entity_roles cer
                   WHERE cer.cbu_id = $1
                     AND NOT EXISTS (
                         SELECT 1
                         FROM "ob-poc".cbu_entity_roles other
                         JOIN "ob-poc".cbus c ON c.cbu_id = other.cbu_id
                         WHERE other.entity_id = cer.entity_id
                           AND other.cbu_id <> $1
                           AND c.deleted_at IS NULL
                     )"#,
            )
            .bind(cbu_id)
            .fetch_all(&mut *tx)
            .await?;

            let shared_count: Option<i64> = sqlx::query_scalar(
                r#"SELECT COUNT(DISTINCT cer.entity_id)::bigint
                   FROM "ob-poc".cbu_entity_roles cer
                   WHERE cer.cbu_id = $1
                     AND EXISTS (
                         SELECT 1
                         FROM "ob-poc".cbu_entity_roles other
                         JOIN "ob-poc".cbus c ON c.cbu_id = other.cbu_id
                         WHERE other.entity_id = cer.entity_id
                           AND other.cbu_id <> $1
                           AND c.deleted_at IS NULL
                     )"#,
            )
            .bind(cbu_id)
            .fetch_one(&mut *tx)
            .await?;
            entities_preserved = shared_count.unwrap_or(0);

            for entity_id in &exclusive_entities {
                let _ = sqlx::query(
                    r#"UPDATE "ob-poc".entities
                       SET deleted_at = NOW(), updated_at = NOW()
                       WHERE entity_id = $1
                         AND deleted_at IS NULL"#,
                )
                .bind(entity_id)
                .execute(&mut *tx)
                .await;
            }

            entities_deleted = exclusive_entities.len() as i64;
        }

        let result = sqlx::query(r#"DELETE FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&mut *tx)
            .await?;
        deleted_counts.insert(
            "cbu_entity_roles".to_string(),
            result.rows_affected() as i64,
        );

        let result = sqlx::query(
            r#"UPDATE "ob-poc".cbus
               SET deleted_at = NOW(), updated_at = NOW()
               WHERE cbu_id = $1
                 AND deleted_at IS NULL"#,
        )
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;
        deleted_counts.insert("cbus".to_string(), result.rows_affected() as i64);

        tx.commit().await?;

        let total_deleted: i64 = deleted_counts.values().sum();

        tracing::info!(
            cbu_id = %cbu_id,
            cbu_name = %cbu_name,
            total_deleted = total_deleted,
            entities_deleted = entities_deleted,
            entities_preserved = entities_preserved,
            "cbu.delete-cascade completed"
        );

        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            serde_json::json!({
                "cbu_id": cbu_id,
                "cbu_name": cbu_name,
                "deleted": true,
                "total_records_deleted": total_deleted,
                "entities_deleted": entities_deleted,
                "entities_preserved_shared": entities_preserved,
                "by_table": deleted_counts
            }),
        ))
    }

    fn is_migrated(&self) -> bool {
        true
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
        // Note: im-entity-id removed - can be added to cbu.create if needed
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
              AND e.deleted_at IS NULL
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

        // Generate DSL statements for each entity (don't execute directly)
        // This allows the batch to be staged in the runbook for user review
        let mut dsl_statements: Vec<String> = Vec::new();
        let mut entity_info: Vec<serde_json::Value> = Vec::new();

        for ent in &entities {
            let jurisdiction = ent.jurisdiction.as_deref().unwrap_or(&default_jurisdiction);

            // Build the cbu.create DSL statement with :fund-entity-id for idempotency
            let mut dsl = format!(
                "(cbu.create :name \"{}\" :jurisdiction \"{}\" :fund-entity-id \"{}\"",
                ent.name.replace('\"', "\\\""), // Escape quotes in name
                jurisdiction,
                ent.entity_id
            );

            // Add manco if provided
            if let Some(manco_id) = manco_entity_id {
                dsl.push_str(&format!(" :manco-entity-id \"{}\"", manco_id));
            }

            dsl.push(')');
            dsl_statements.push(dsl.clone());

            entity_info.push(serde_json::json!({
                "entity_id": ent.entity_id,
                "name": ent.name,
                "jurisdiction": jurisdiction,
                "gleif_category": ent.gleif_category,
                "group_role": ent.group_role,
                "dsl": dsl,
            }));
        }

        if dry_run {
            // Dry run: just return what would be generated
            return Ok(ExecutionResult::Record(serde_json::json!({
                "dry_run": true,
                "group_id": group_id,
                "gleif_category": gleif_category,
                "role_filter": role_filter,
                "jurisdiction_filter": jurisdiction_filter,
                "entities_found": entities.len(),
                "entities": entity_info,
                "dsl_batch": dsl_statements,
            })));
        }

        // Return DSL batch for staging (macro behavior)
        // The caller (agent) should stage these in the runbook
        let combined_dsl = dsl_statements.join("\n");

        Ok(ExecutionResult::Record(serde_json::json!({
            "group_id": group_id,
            "gleif_category": gleif_category,
            "role_filter": role_filter,
            "jurisdiction_filter": jurisdiction_filter,
            "entities_found": entities.len(),
            "dsl_batch": dsl_statements,
            "combined_dsl": combined_dsl,
            "message": format!("Generated {} cbu.create statements. Stage in runbook and say 'run' to execute.", entities.len())
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

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let group_id = json_extract_uuid(args, ctx, "group-id")?;
        let gleif_category = json_extract_string_opt(args, "gleif-category");
        let role_filter = json_extract_string_opt(args, "role-filter");
        let jurisdiction_filter = json_extract_string_opt(args, "jurisdiction-filter");
        let default_jurisdiction = json_extract_string_opt(args, "default-jurisdiction")
            .unwrap_or_else(|| "LU".to_string());
        let manco_entity_id = if args.get("manco-entity-id").is_some() {
            Some(json_extract_uuid(args, ctx, "manco-entity-id")?)
        } else {
            None
        };
        let limit = json_extract_int_opt(args, "limit").unwrap_or(100) as i64;
        let dry_run = json_extract_bool_opt(args, "dry-run").unwrap_or(false);

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
              AND e.deleted_at IS NULL
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

        let mut dsl_statements: Vec<String> = Vec::new();
        let mut entity_info: Vec<serde_json::Value> = Vec::new();

        for ent in &entities {
            let jurisdiction = ent.jurisdiction.as_deref().unwrap_or(&default_jurisdiction);
            let mut dsl = format!(
                "(cbu.create :name \"{}\" :jurisdiction \"{}\" :fund-entity-id \"{}\"",
                ent.name.replace('\"', "\\\""),
                jurisdiction,
                ent.entity_id
            );

            if let Some(manco_id) = manco_entity_id {
                dsl.push_str(&format!(" :manco-entity-id \"{}\"", manco_id));
            }

            dsl.push(')');
            dsl_statements.push(dsl.clone());

            entity_info.push(serde_json::json!({
                "entity_id": ent.entity_id,
                "name": ent.name,
                "jurisdiction": jurisdiction,
                "gleif_category": ent.gleif_category,
                "group_role": ent.group_role,
                "dsl": dsl,
            }));
        }

        if dry_run {
            return Ok(dsl_runtime::VerbExecutionOutcome::Record(
                serde_json::json!({
                    "dry_run": true,
                    "group_id": group_id,
                    "gleif_category": gleif_category,
                    "role_filter": role_filter,
                    "jurisdiction_filter": jurisdiction_filter,
                    "entities_found": entities.len(),
                    "entities": entity_info,
                    "dsl_batch": dsl_statements,
                }),
            ));
        }

        let combined_dsl = dsl_statements.join("\n");

        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            serde_json::json!({
                "group_id": group_id,
                "gleif_category": gleif_category,
                "role_filter": role_filter,
                "jurisdiction_filter": jurisdiction_filter,
                "entities_found": entities.len(),
                "dsl_batch": dsl_statements,
                "combined_dsl": combined_dsl,
                "message": format!("Generated {} cbu.create statements. Stage in runbook and say 'run' to execute.", entities.len())
            }),
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// Note: legacy role-mapping helpers were removed.
// Role assignment is now handled by cbu.create plugin via :fund-entity-id arg
