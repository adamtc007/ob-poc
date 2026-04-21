//! CBU custom operations (9 plugin verbs) — YAML-first re-implementation of
//! `cbu.*` from `rust/config/verbs/cbu.yaml`.
//!
//! Operations for CBU (Client Business Unit) management including
//! creation, structure links, product assignment, inspect, decide,
//! cascade delete, and bulk creation from client groups.
//!
//! # Ops
//!
//! - `cbu.create` — Create CBU with optional entity linking (ASSET_OWNER, MANAGEMENT_COMPANY)
//! - `cbu.link-structure` — Persist parent-child structure link between two CBUs
//! - `cbu.list-structure-links` — List persisted structure links
//! - `cbu.unlink-structure` — Terminate an active structure link
//! - `cbu.add-product` — Link CBU to product and create service delivery entries
//! - `cbu.inspect` — Show full CBU structure with entities, roles, documents, screenings
//! - `cbu.decide` — Record KYC/AML decision for CBU collective state
//! - `cbu.delete-cascade` — Delete CBU and related data with cascade
//! - `cbu.create-from-client-group` — Bulk CBU creation from client group entities

use anyhow::Result;
use async_trait::async_trait;
use chrono::NaiveDate;
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_bool_opt, json_extract_int_opt, json_extract_string, json_extract_string_opt,
    json_extract_uuid, json_extract_uuid_opt,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

// =============================================================================
// Local helpers
// =============================================================================

fn json_extract_uuid_alias(
    args: &Value,
    ctx: &mut VerbExecutionContext,
    keys: &[&str],
) -> Result<Option<Uuid>> {
    for key in keys {
        if args.get(*key).is_some() {
            return Ok(Some(json_extract_uuid(args, ctx, key)?));
        }
    }
    Ok(None)
}

fn json_extract_string_alias(args: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        args.get(*key)
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
    })
}

fn parse_optional_date(value: Option<String>, arg_name: &str) -> Result<Option<NaiveDate>> {
    value
        .map(|raw| {
            NaiveDate::parse_from_str(&raw, "%Y-%m-%d")
                .map_err(|err| anyhow::anyhow!("invalid {} '{}': {}", arg_name, raw, err))
        })
        .transpose()
}

fn normalize_relationship_type(raw: &str) -> String {
    raw.replace('-', "_").to_ascii_uppercase()
}

fn normalize_capital_flow(raw: &str) -> String {
    raw.replace('-', "_").to_ascii_uppercase()
}

// =============================================================================
// cbu.create
// =============================================================================

/// Create a new CBU with optional fund entity linking.
///
/// Idempotency:
/// - If :fund-entity-id is provided, checks if that entity is already linked to ANY CBU
///   as ASSET_OWNER. If so, returns the existing CBU (skipped).
/// - If no :fund-entity-id, uses name+jurisdiction as fallback idempotency key.
pub struct Create;

#[async_trait]
impl SemOsVerbOp for Create {
    fn fqn(&self) -> &str {
        "cbu.create"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
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
            .fetch_optional(scope.executor())
            .await?;

            if let Some((existing_cbu_id, existing_cbu_name)) = existing {
                return Ok(VerbExecutionOutcome::Record(serde_json::json!({
                    "cbu_id": existing_cbu_id,
                    "name": existing_cbu_name,
                    "created": false,
                    "skipped_reason": format!("Entity {} already linked to CBU '{}'", fund_id, existing_cbu_name)
                })));
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
        .fetch_one(scope.executor())
        .await?;

        if let Some(fund_id) = fund_entity_id {
            let asset_owner_role_id: Option<Uuid> = sqlx::query_scalar(
                r#"SELECT role_id FROM "ob-poc".roles WHERE name = 'ASSET_OWNER'"#,
            )
            .fetch_optional(scope.executor())
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
                .execute(scope.executor())
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
            .execute(scope.executor())
            .await?;
        }

        if let Some(manco_id) = manco_entity_id {
            let manco_role_id: Option<Uuid> = sqlx::query_scalar(
                r#"SELECT role_id FROM "ob-poc".roles WHERE name = 'MANAGEMENT_COMPANY'"#,
            )
            .fetch_optional(scope.executor())
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
                .execute(scope.executor())
                .await?;
            }
        }

        let skipped_reason: Option<&str> = if is_new {
            None
        } else {
            Some("CBU with same name+jurisdiction already exists")
        };

        Ok(VerbExecutionOutcome::Record(serde_json::json!({
            "cbu_id": cbu_id,
            "name": name,
            "jurisdiction": jurisdiction,
            "created": is_new,
            "skipped_reason": skipped_reason
        })))
    }
}

// =============================================================================
// cbu.link-structure
// =============================================================================

/// Persist a parent-child structure link between two CBUs.
pub struct LinkStructure;

#[async_trait]
impl SemOsVerbOp for LinkStructure {
    fn fqn(&self) -> &str {
        "cbu.link-structure"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
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
        .fetch_optional(scope.executor())
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
        .fetch_optional(scope.executor())
        .await?;
        if child_exists.is_none() {
            return Err(anyhow::anyhow!(
                "cbu.link-structure: child CBU not found: {}",
                child_cbu_id
            ));
        }

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
        .fetch_optional(scope.executor())
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
            .execute(scope.executor())
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
            .execute(scope.executor())
            .await?;
        }

        Ok(VerbExecutionOutcome::Uuid(child_cbu_id))
    }
}

// =============================================================================
// cbu.list-structure-links
// =============================================================================

/// List persisted structure links for a parent or child CBU.
pub struct ListStructureLinks;

#[async_trait]
impl SemOsVerbOp for ListStructureLinks {
    fn fqn(&self) -> &str {
        "cbu.list-structure-links"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
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

        type LinkRow = (
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
        );

        let rows: Vec<LinkRow> = match (parent_cbu_id, child_cbu_id, status) {
            (Some(parent), Some(child), Some(status)) => {
                sqlx::query_as::<_, LinkRow>(
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
                .fetch_all(scope.executor())
                .await?
            }
            (Some(parent), Some(child), None) => {
                sqlx::query_as::<_, LinkRow>(
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
                .fetch_all(scope.executor())
                .await?
            }
            (Some(parent), None, Some(status)) => {
                sqlx::query_as::<_, LinkRow>(
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
                .fetch_all(scope.executor())
                .await?
            }
            (Some(parent), None, None) => {
                sqlx::query_as::<_, LinkRow>(
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
                .fetch_all(scope.executor())
                .await?
            }
            (None, Some(child), Some(status)) => {
                sqlx::query_as::<_, LinkRow>(
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
                .fetch_all(scope.executor())
                .await?
            }
            (None, Some(child), None) => {
                sqlx::query_as::<_, LinkRow>(
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
                .fetch_all(scope.executor())
                .await?
            }
            (None, None, _) => unreachable!(),
        };

        Ok(VerbExecutionOutcome::RecordSet(
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
}

// =============================================================================
// cbu.unlink-structure
// =============================================================================

/// Terminate an active persisted structure link between CBUs.
pub struct UnlinkStructure;

#[async_trait]
impl SemOsVerbOp for UnlinkStructure {
    fn fqn(&self) -> &str {
        "cbu.unlink-structure"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
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
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Affected(result.rows_affected()))
    }
}

// =============================================================================
// cbu.add-product
// =============================================================================

/// Add a product to a CBU by creating service_delivery_map and cbu_resource_instances entries.
pub struct AddProduct;

#[async_trait]
impl SemOsVerbOp for AddProduct {
    fn fqn(&self) -> &str {
        "cbu.add-product"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let product_name = json_extract_string(args, "product")?;
        let cbu_id_arg = args.get("cbu-id").ok_or_else(|| {
            anyhow::anyhow!("cbu.add-product: Missing required argument :cbu-id")
        })?;

        let (cbu_id, cbu_name): (Uuid, String) = if let Some(str_val) = cbu_id_arg.as_str() {
            if str_val.starts_with('@') {
                let resolved_id = json_extract_uuid(args, ctx, "cbu-id")?;
                sqlx::query_as(
                    r#"SELECT cbu_id, name
                       FROM "ob-poc".cbus
                       WHERE cbu_id = $1
                         AND deleted_at IS NULL"#,
                )
                .bind(resolved_id)
                .fetch_optional(scope.executor())
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!("cbu.add-product: CBU not found with id {}", resolved_id)
                })?
            } else if let Ok(uuid_val) = Uuid::parse_str(str_val) {
                sqlx::query_as(
                    r#"SELECT cbu_id, name
                       FROM "ob-poc".cbus
                       WHERE cbu_id = $1
                         AND deleted_at IS NULL"#,
                )
                .bind(uuid_val)
                .fetch_optional(scope.executor())
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!("cbu.add-product: CBU not found with id {}", uuid_val)
                })?
            } else {
                sqlx::query_as(
                    r#"SELECT cbu_id, name
                       FROM "ob-poc".cbus
                       WHERE LOWER(name) = LOWER($1)
                         AND deleted_at IS NULL"#,
                )
                .bind(str_val)
                .fetch_optional(scope.executor())
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "cbu.add-product: CBU '{}' not found. Use cbu.list to see available CBUs.",
                        str_val
                    )
                })?
            }
        } else {
            let resolved_id = json_extract_uuid(args, ctx, "cbu-id")?;
            sqlx::query_as(
                r#"SELECT cbu_id, name
                   FROM "ob-poc".cbus
                   WHERE cbu_id = $1
                     AND deleted_at IS NULL"#,
            )
            .bind(resolved_id)
            .fetch_optional(scope.executor())
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("cbu.add-product: CBU not found with id {}", resolved_id)
            })?
        };

        let product_row: Option<(Uuid, String, String)> = sqlx::query_as(
            r#"SELECT product_id, name, product_code FROM "ob-poc".products WHERE product_code = $1"#,
        )
        .bind(&product_name)
        .fetch_optional(scope.executor())
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
        .fetch_all(scope.executor())
        .await?;

        if services.is_empty() {
            return Err(anyhow::anyhow!(
                "cbu.add-product: Product '{}' has no services defined in product_services. \
                 Cannot add product without services.",
                product_name
            ));
        }

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
            .execute(scope.executor())
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
        .fetch_all(scope.executor())
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
            .execute(scope.executor())
            .await?;

            if result.rows_affected() > 0 {
                resource_created += 1;
            } else {
                resource_skipped += 1;
            }
        }

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

        Ok(VerbExecutionOutcome::Affected(
            (delivery_created + resource_created) as u64,
        ))
    }
}

// =============================================================================
// cbu.inspect
// =============================================================================

/// Show full CBU structure including entities, roles, documents, screenings.
pub struct Inspect;

#[async_trait]
impl SemOsVerbOp for Inspect {
    fn fqn(&self) -> &str {
        "cbu.inspect"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let as_of_date = json_extract_string_opt(args, "as-of-date")
            .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());

        let cbu: (
            Uuid,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<chrono::DateTime<chrono::Utc>>,
            Option<chrono::DateTime<chrono::Utc>>,
        ) = sqlx::query_as(
            r#"SELECT cbu_id, name, jurisdiction, client_type, cbu_category,
                      nature_purpose, description, created_at, updated_at
               FROM "ob-poc".cbus WHERE cbu_id = $1 AND deleted_at IS NULL"#,
        )
        .bind(cbu_id)
        .fetch_optional(scope.executor())
        .await?
        .ok_or_else(|| anyhow::anyhow!("CBU not found: {}", cbu_id))?;

        let entities: Vec<(Uuid, String, String, Option<String>)> = sqlx::query_as(
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
        )
        .bind(cbu_id)
        .bind(as_of_date)
        .fetch_all(scope.executor())
        .await?;

        let roles: Vec<(Uuid, String)> = sqlx::query_as(
            r#"SELECT cer.entity_id, r.name as role_name
               FROM "ob-poc".cbu_entity_roles cer
               JOIN "ob-poc".roles r ON cer.role_id = r.role_id
               WHERE cer.cbu_id = $1
               AND (cer.effective_from IS NULL OR cer.effective_from <= $2)
               AND (cer.effective_to IS NULL OR cer.effective_to >= $2)
               ORDER BY cer.entity_id, r.name"#,
        )
        .bind(cbu_id)
        .bind(as_of_date)
        .fetch_all(scope.executor())
        .await?;

        let entity_list: Vec<Value> = entities
            .iter()
            .map(|(eid, name, etype, juris)| {
                let entity_roles: Vec<String> = roles
                    .iter()
                    .filter(|(rid, _)| rid == eid)
                    .map(|(_, rn)| rn.clone())
                    .collect();
                serde_json::json!({
                    "entity_id": eid, "name": name,
                    "entity_type": etype, "jurisdiction": juris,
                    "roles": entity_roles
                })
            })
            .collect();

        let documents: Vec<(Uuid, Option<String>, Option<String>, Option<String>, Option<String>)> =
            sqlx::query_as(
                r#"SELECT dc.doc_id, dc.document_name, dt.type_code, dt.display_name, dc.status
                   FROM "ob-poc".document_catalog dc
                   LEFT JOIN "ob-poc".document_types dt ON dc.document_type_id = dt.type_id
                   WHERE dc.cbu_id = $1 ORDER BY dt.type_code"#,
            )
            .bind(cbu_id)
            .fetch_all(scope.executor())
            .await?;

        let doc_list: Vec<Value> = documents
            .iter()
            .map(|(doc_id, document_name, type_code, display_name, status)| {
                serde_json::json!({
                    "doc_id": doc_id,
                    "name": document_name,
                    "type_code": type_code,
                    "type_name": display_name,
                    "status": status
                })
            })
            .collect();

        let screenings: Vec<(Uuid, Uuid, String, String, String, Option<String>)> = sqlx::query_as(
            r#"SELECT s.screening_id, w.entity_id, e.name as entity_name,
                      s.screening_type, s.status, s.result_summary
               FROM "ob-poc".screenings s
               JOIN "ob-poc".entity_workstreams w ON w.workstream_id = s.workstream_id
               JOIN "ob-poc".cases c ON c.case_id = w.case_id
               JOIN "ob-poc".entities e ON e.entity_id = w.entity_id
               WHERE c.cbu_id = $1 AND e.deleted_at IS NULL
               ORDER BY s.screening_type, e.name"#,
        )
        .bind(cbu_id)
        .fetch_all(scope.executor())
        .await?;

        let screening_list: Vec<Value> = screenings
            .iter()
            .map(|(sid, eid, ename, stype, status, result)| {
                serde_json::json!({
                    "screening_id": sid,
                    "entity_id": eid,
                    "entity_name": ename,
                    "screening_type": stype,
                    "status": status,
                    "result": result
                })
            })
            .collect();

        let services: Vec<(Uuid, String, String, String, String)> = sqlx::query_as(
            r#"SELECT sdm.delivery_id, p.name as product_name, p.product_code,
                      s.name as service_name, sdm.delivery_status
               FROM "ob-poc".service_delivery_map sdm
               JOIN "ob-poc".products p ON p.product_id = sdm.product_id
               JOIN "ob-poc".services s ON s.service_id = sdm.service_id
               WHERE sdm.cbu_id = $1 ORDER BY p.name, s.name"#,
        )
        .bind(cbu_id)
        .fetch_all(scope.executor())
        .await?;

        let service_list: Vec<Value> = services
            .iter()
            .map(|(did, pname, pcode, sname, dstatus)| {
                serde_json::json!({
                    "delivery_id": did,
                    "product": pname,
                    "product_code": pcode,
                    "service": sname,
                    "status": dstatus
                })
            })
            .collect();

        let cases: Vec<(
            Uuid,
            String,
            String,
            Option<String>,
            Option<String>,
            chrono::DateTime<chrono::Utc>,
            Option<chrono::DateTime<chrono::Utc>>,
        )> = sqlx::query_as(
            r#"SELECT case_id, status, case_type, risk_rating, escalation_level,
                      opened_at, closed_at
               FROM "ob-poc".cases WHERE cbu_id = $1 ORDER BY opened_at DESC"#,
        )
        .bind(cbu_id)
        .fetch_all(scope.executor())
        .await?;

        let case_list: Vec<Value> = cases
            .iter()
            .map(
                |(cid, status, ctype, risk_rating, escalation_level, opened_at, closed_at)| {
                    serde_json::json!({
                        "case_id": cid,
                        "status": status,
                        "case_type": ctype,
                        "risk_rating": risk_rating,
                        "escalation_level": escalation_level,
                        "opened_at": opened_at.to_rfc3339(),
                        "closed_at": closed_at.map(|t| t.to_rfc3339())
                    })
                },
            )
            .collect();

        let entity_count = entity_list.len();
        let document_count = doc_list.len();
        let screening_count = screening_list.len();
        let service_count = service_list.len();
        let case_count = case_list.len();

        Ok(VerbExecutionOutcome::Record(serde_json::json!({
            "cbu_id": cbu.0,
            "name": cbu.1,
            "jurisdiction": cbu.2,
            "client_type": cbu.3,
            "category": cbu.4,
            "nature_purpose": cbu.5,
            "description": cbu.6,
            "created_at": cbu.7.map(|t| t.to_rfc3339()),
            "updated_at": cbu.8.map(|t| t.to_rfc3339()),
            "as_of_date": as_of_date.to_string(),
            "entities": entity_list,
            "documents": doc_list,
            "screenings": screening_list,
            "services": service_list,
            "kyc_cases": case_list,
            "summary": {
                "entity_count": entity_count,
                "document_count": document_count,
                "screening_count": screening_count,
                "service_count": service_count,
                "case_count": case_count
            }
        })))
    }
}

// =============================================================================
// cbu.decide
// =============================================================================

/// Record KYC/AML decision for CBU collective state.
pub struct Decide;

#[async_trait]
impl SemOsVerbOp for Decide {
    fn fqn(&self) -> &str {
        "cbu.decide"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
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

        let cbu: (String, Option<String>) = sqlx::query_as(
            r#"SELECT name, status
               FROM "ob-poc".cbus
               WHERE cbu_id = $1
                 AND deleted_at IS NULL"#,
        )
        .bind(cbu_id)
        .fetch_optional(scope.executor())
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
                let row: Option<(Uuid,)> = sqlx::query_as(
                    r#"SELECT case_id FROM "ob-poc".cases
                       WHERE cbu_id = $1 AND status NOT IN ('APPROVED', 'REJECTED', 'WITHDRAWN', 'EXPIRED')
                       ORDER BY opened_at DESC LIMIT 1"#,
                )
                .bind(cbu_id)
                .fetch_optional(scope.executor())
                .await?;
                row.ok_or_else(|| anyhow::anyhow!("No active KYC case found for CBU"))?
                    .0
            }
        };

        sqlx::query(
            r#"UPDATE "ob-poc".cbus
               SET status = $1, updated_at = now()
               WHERE cbu_id = $2
                 AND deleted_at IS NULL"#,
        )
        .bind(new_cbu_status)
        .bind(cbu_id)
        .execute(scope.executor())
        .await?;

        let should_close = matches!(decision.as_str(), "APPROVED" | "REJECTED");
        if should_close {
            sqlx::query(
                r#"UPDATE "ob-poc".cases SET status = $1, closed_at = now(), last_activity_at = now() WHERE case_id = $2"#,
            )
            .bind(new_case_status)
            .bind(case_id)
            .execute(scope.executor())
            .await?;
        } else {
            sqlx::query(
                r#"UPDATE "ob-poc".cases SET escalation_level = 'SENIOR_COMPLIANCE', last_activity_at = now() WHERE case_id = $1"#,
            )
            .bind(case_id)
            .execute(scope.executor())
            .await?;
        }

        let snapshot_id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO "ob-poc".case_evaluation_snapshots
               (snapshot_id, case_id, soft_count, escalate_count, hard_stop_count, total_score,
                recommended_action, evaluated_by, decision_made, decision_made_at, decision_made_by, decision_notes)
               VALUES ($1, $2, 0, 0, 0, 0, $3, $4, $3, now(), $4, $5)"#,
        )
        .bind(snapshot_id)
        .bind(case_id)
        .bind(&decision)
        .bind(&decided_by)
        .bind(&rationale)
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Record(serde_json::json!({
            "cbu_id": cbu_id,
            "cbu_name": cbu.0,
            "case_id": case_id,
            "snapshot_id": snapshot_id,
            "decision": decision,
            "previous_status": cbu.1,
            "new_status": new_cbu_status,
            "decided_by": decided_by,
            "rationale": rationale,
            "conditions": conditions,
            "escalation_reason": escalation_reason
        })))
    }
}

// =============================================================================
// cbu.delete-cascade
// =============================================================================

/// Delete a CBU and all related data with cascade.
pub struct DeleteCascade;

#[async_trait]
impl SemOsVerbOp for DeleteCascade {
    fn fqn(&self) -> &str {
        "cbu.delete-cascade"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let delete_entities = json_extract_bool_opt(args, "delete-entities").unwrap_or(true);

        let cbu: (String,) = sqlx::query_as(
            r#"SELECT name FROM "ob-poc".cbus WHERE cbu_id = $1 AND deleted_at IS NULL"#,
        )
        .bind(cbu_id)
        .fetch_optional(scope.executor())
        .await?
        .ok_or_else(|| anyhow::anyhow!("CBU not found: {}", cbu_id))?;

        let cbu_name = cbu.0;
        let mut deleted_counts: HashMap<String, i64> = HashMap::new();

        let result = sqlx::query(
            r#"UPDATE "ob-poc".client_group_entity
               SET cbu_id = NULL, updated_at = NOW()
               WHERE cbu_id = $1"#,
        )
        .bind(cbu_id)
        .execute(scope.executor())
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
        .execute(scope.executor())
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
        .execute(scope.executor())
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
            .fetch_all(scope.executor())
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
            .fetch_one(scope.executor())
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
                .execute(scope.executor())
                .await;
            }

            entities_deleted = exclusive_entities.len() as i64;
        }

        let result = sqlx::query(r#"DELETE FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(scope.executor())
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
        .execute(scope.executor())
        .await?;
        deleted_counts.insert("cbus".to_string(), result.rows_affected() as i64);

        let total_deleted: i64 = deleted_counts.values().sum();

        tracing::info!(
            cbu_id = %cbu_id,
            cbu_name = %cbu_name,
            total_deleted = total_deleted,
            entities_deleted = entities_deleted,
            entities_preserved = entities_preserved,
            "cbu.delete-cascade completed"
        );

        Ok(VerbExecutionOutcome::Record(serde_json::json!({
            "cbu_id": cbu_id,
            "cbu_name": cbu_name,
            "deleted": true,
            "total_records_deleted": total_deleted,
            "entities_deleted": entities_deleted,
            "entities_preserved_shared": entities_preserved,
            "by_table": deleted_counts
        })))
    }
}

// =============================================================================
// cbu.create-from-client-group
// =============================================================================

/// Entity info from client group query - includes GLEIF category and group role for mapping.
#[derive(Debug)]
struct ClientGroupEntity {
    entity_id: Uuid,
    name: String,
    jurisdiction: Option<String>,
    gleif_category: Option<String>,
    group_role: Option<String>,
}

/// Create CBUs from entities in a client group with GLEIF category and role filters.
pub struct CreateFromClientGroup;

#[async_trait]
impl SemOsVerbOp for CreateFromClientGroup {
    fn fqn(&self) -> &str {
        "cbu.create-from-client-group"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
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
            .fetch_all(scope.executor())
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
        let mut entity_info: Vec<Value> = Vec::new();

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
            return Ok(VerbExecutionOutcome::Record(serde_json::json!({
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

        let combined_dsl = dsl_statements.join("\n");

        Ok(VerbExecutionOutcome::Record(serde_json::json!({
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
}
