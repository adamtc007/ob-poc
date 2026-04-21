//! ManCo / governance-controller verbs (9 plugin verbs spanning
//! `manco` + `ownership` domains) — SemOS-side YAML-first
//! re-implementation of the plugin subset of
//! `rust/config/verbs/manco.yaml` + `rust/config/verbs/ownership.yaml`.
//!
//! Three op shapes:
//! - **Bridge ops** (ownership.bridge.*) — delegate to SQL
//!   `fn_bridge_*` functions that translate data sources
//!   (manco roles, GLEIF fund managers, BODS ownership) into
//!   governance-controller signals.
//! - **Group derivation** (manco.group.*, manco.primary-controller,
//!   manco.control-chain, manco.book.summary) — SQL function
//!   calls over the derived group state.
//! - **Pipeline** (ownership.refresh) — runs all bridges +
//!   control-links + group derivation in one atomic txn.
//!
//! Ports the shared `ob_poc_types::manco_group::*` DTOs
//! (BridgeRolesResult, DeriveGroupsResult, etc.) for
//! consistent JSON shapes.

use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{json_extract_int_opt, json_extract_string_opt, json_extract_uuid, json_extract_uuid_opt};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use ob_poc_types::manco_group::{
    BridgeBodsResult, BridgeGleifResult, BridgeRolesResult, CbuMancoNotFound, CbuMancoResult,
    ComputeControlLinksResult, ControlChainNode, ControlType, ControllerBasis, DeriveGroupsResult,
    GovernanceRefreshResult, GroupCbuEntry, PrimaryGovernanceController,
};

use super::SemOsVerbOp;

fn json_extract_date_opt(args: &Value, arg_name: &str) -> Option<chrono::NaiveDate> {
    json_extract_string_opt(args, arg_name)
        .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
}

// ── ownership.bridge.manco-roles ──────────────────────────────────────────────

pub struct BridgeMancoRoles;

#[async_trait]
impl SemOsVerbOp for BridgeMancoRoles {
    fn fqn(&self) -> &str {
        "ownership.bridge.manco-roles"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let as_of = json_extract_date_opt(args, "as-of");
        let row: (i32, i32) =
            sqlx::query_as(r#"SELECT * FROM "ob-poc".fn_bridge_manco_role_to_board_rights($1)"#)
                .bind(as_of)
                .fetch_one(scope.executor())
                .await?;
        let result = BridgeRolesResult {
            rights_created: row.0,
            rights_updated: row.1,
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

// ── ownership.bridge.gleif-fund-managers ──────────────────────────────────────

pub struct BridgeGleifFundManagers;

#[async_trait]
impl SemOsVerbOp for BridgeGleifFundManagers {
    fn fqn(&self) -> &str {
        "ownership.bridge.gleif-fund-managers"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let as_of = json_extract_date_opt(args, "as-of");
        let row: (i32, i32) = sqlx::query_as(
            r#"SELECT * FROM "ob-poc".fn_bridge_gleif_fund_manager_to_board_rights($1)"#,
        )
        .bind(as_of)
        .fetch_one(scope.executor())
        .await?;
        let result = BridgeGleifResult {
            rights_created: row.0,
            rights_updated: row.1,
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

// ── ownership.bridge.bods-ownership ───────────────────────────────────────────

pub struct BridgeBodsOwnership;

#[async_trait]
impl SemOsVerbOp for BridgeBodsOwnership {
    fn fqn(&self) -> &str {
        "ownership.bridge.bods-ownership"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let as_of = json_extract_date_opt(args, "as-of");
        let row: (i32, i32, i32) =
            sqlx::query_as(r#"SELECT * FROM "ob-poc".fn_bridge_bods_to_holdings($1)"#)
                .bind(as_of)
                .fetch_one(scope.executor())
                .await?;
        let result = BridgeBodsResult {
            holdings_created: row.0,
            holdings_updated: row.1,
            entities_linked: row.2,
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

// ── manco.group.derive ────────────────────────────────────────────────────────

pub struct GroupDerive;

#[async_trait]
impl SemOsVerbOp for GroupDerive {
    fn fqn(&self) -> &str {
        "manco.group.derive"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let as_of = json_extract_date_opt(args, "as-of");
        let row: (i32, i32) = sqlx::query_as(r#"SELECT * FROM "ob-poc".fn_derive_cbu_groups($1)"#)
            .bind(as_of)
            .fetch_one(scope.executor())
            .await?;
        let result = DeriveGroupsResult {
            groups_created: row.0,
            memberships_created: row.1,
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

// ── manco.group.cbus ──────────────────────────────────────────────────────────

pub struct GroupCbus;

#[async_trait]
impl SemOsVerbOp for GroupCbus {
    fn fqn(&self) -> &str {
        "manco.group.cbus"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let manco_entity_id = json_extract_uuid(args, ctx, "manco-entity-id")?;

        type Row = (
            Uuid,
            String,
            String,
            Option<String>,
            Option<Uuid>,
            Option<String>,
            String,
        );

        let rows: Vec<Row> = sqlx::query_as(r#"SELECT * FROM "ob-poc".fn_get_manco_group_cbus($1)"#)
            .bind(manco_entity_id)
            .fetch_all(scope.executor())
            .await?;

        let results: Vec<Value> = rows
            .into_iter()
            .map(|(cbu_id, cbu_name, cbu_category, jurisdiction, fund_entity_id, fund_entity_name, membership_source)| {
                let entry = GroupCbuEntry {
                    cbu_id,
                    cbu_name,
                    cbu_category,
                    jurisdiction,
                    fund_entity_id,
                    fund_entity_name,
                    membership_source,
                };
                serde_json::to_value(entry).unwrap_or_default()
            })
            .collect();

        Ok(VerbExecutionOutcome::RecordSet(results))
    }
}

// ── manco.group.for-cbu ───────────────────────────────────────────────────────

pub struct GroupForCbu;

#[async_trait]
impl SemOsVerbOp for GroupForCbu {
    fn fqn(&self) -> &str {
        "manco.group.for-cbu"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;

        type Row = (Uuid, String, Option<String>, Uuid, String, String, String);

        let row: Option<Row> = sqlx::query_as(r#"SELECT * FROM "ob-poc".fn_get_cbu_manco($1)"#)
            .bind(cbu_id)
            .fetch_optional(scope.executor())
            .await?;

        let value = match row {
            Some((manco_entity_id, manco_name, manco_lei, group_id, group_name, group_type, source)) => {
                serde_json::to_value(CbuMancoResult {
                    manco_entity_id,
                    manco_name,
                    manco_lei,
                    group_id,
                    group_name,
                    group_type,
                    source,
                })?
            }
            None => serde_json::to_value(CbuMancoNotFound {
                message: "No governance controller found for this CBU".to_string(),
            })?,
        };
        Ok(VerbExecutionOutcome::Record(value))
    }
}

// ── manco.primary-controller ──────────────────────────────────────────────────

pub struct PrimaryController;

#[async_trait]
impl SemOsVerbOp for PrimaryController {
    fn fqn(&self) -> &str {
        "manco.primary-controller"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let issuer_entity_id = json_extract_uuid(args, ctx, "issuer-entity-id")?;
        let as_of = json_extract_date_opt(args, "as-of");

        type Row = (
            Uuid,
            Option<Uuid>,
            Option<Uuid>,
            Option<String>,
            Option<i32>,
            Option<Decimal>,
            Option<Decimal>,
            Option<bool>,
            Option<bool>,
        );

        let row: Option<Row> = sqlx::query_as(
            r#"SELECT * FROM "ob-poc".fn_primary_governance_controller($1, $2)"#,
        )
        .bind(issuer_entity_id)
        .bind(as_of)
        .fetch_optional(scope.executor())
        .await?;

        let value = match row {
            Some((_, primary, governance, basis, board_seats, voting_pct, economic_pct, has_control, has_sig)) => {
                serde_json::to_value(PrimaryGovernanceController {
                    issuer_entity_id,
                    primary_controller_entity_id: primary.unwrap_or(Uuid::nil()),
                    governance_controller_entity_id: governance.unwrap_or(Uuid::nil()),
                    basis: basis
                        .as_deref()
                        .map(|s| match s {
                            "BOARD_APPOINTMENT" => ControllerBasis::BoardAppointment,
                            "VOTING_CONTROL" => ControllerBasis::VotingControl,
                            "SIGNIFICANT_INFLUENCE" => ControllerBasis::SignificantInfluence,
                            _ => ControllerBasis::None,
                        })
                        .unwrap_or(ControllerBasis::None),
                    board_seats: board_seats.unwrap_or(0),
                    voting_pct,
                    economic_pct,
                    has_control: has_control.unwrap_or(false),
                    has_significant_influence: has_sig.unwrap_or(false),
                })?
            }
            None => serde_json::to_value(CbuMancoNotFound {
                message: "No governance controller found".to_string(),
            })?,
        };
        Ok(VerbExecutionOutcome::Record(value))
    }
}

// ── manco.control-chain ───────────────────────────────────────────────────────

pub struct ControlChain;

#[async_trait]
impl SemOsVerbOp for ControlChain {
    fn fqn(&self) -> &str {
        "manco.control-chain"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let manco_entity_id = json_extract_uuid(args, ctx, "manco-entity-id")?;
        let max_depth = json_extract_int_opt(args, "max-depth").unwrap_or(5);

        type Row = (
            i32,
            Uuid,
            String,
            Option<String>,
            Option<Uuid>,
            Option<String>,
            Option<String>,
            Option<Decimal>,
            bool,
        );

        let rows: Vec<Row> = sqlx::query_as(
            r#"SELECT * FROM "ob-poc".fn_manco_group_control_chain($1, $2)"#,
        )
        .bind(manco_entity_id)
        .bind(max_depth as i32)
        .fetch_all(scope.executor())
        .await?;

        let results: Vec<Value> = rows
            .into_iter()
            .map(|(depth, entity_id, entity_name, entity_type, controlled_by_id, controlled_by_name, control_type_str, voting_pct, is_ultimate)| {
                let control_type = control_type_str.as_deref().map(|s| match s {
                    "CONTROLLING" => ControlType::Controlling,
                    "SIGNIFICANT_INFLUENCE" => ControlType::SignificantInfluence,
                    "MATERIAL" => ControlType::Material,
                    "NOTIFIABLE" => ControlType::Notifiable,
                    _ => ControlType::Minority,
                });
                serde_json::to_value(ControlChainNode {
                    depth,
                    entity_id,
                    entity_name,
                    entity_type,
                    controlled_by_entity_id: controlled_by_id,
                    controlled_by_name,
                    control_type,
                    voting_pct,
                    is_ultimate_controller: is_ultimate,
                })
                .unwrap_or_default()
            })
            .collect();

        Ok(VerbExecutionOutcome::RecordSet(results))
    }
}

// ── manco.book.summary ────────────────────────────────────────────────────────

pub struct BookSummary;

#[async_trait]
impl SemOsVerbOp for BookSummary {
    fn fqn(&self) -> &str {
        "manco.book.summary"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let manco_entity_id = json_extract_uuid(args, ctx, "manco-entity-id")?;

        // 1. Group info
        let group_row: Option<(Uuid, Uuid, String, Option<String>, String, Option<String>, Option<Uuid>, i64)> =
            sqlx::query_as(
                r#"
                SELECT g.group_id, g.manco_entity_id, g.group_name, g.group_code,
                       g.group_type, g.jurisdiction, g.ultimate_parent_entity_id,
                       COUNT(DISTINCT m.cbu_id) as cbu_count
                FROM "ob-poc".cbu_groups g
                LEFT JOIN "ob-poc".cbu_group_members m ON g.group_id = m.group_id
                    AND m.effective_to IS NULL
                WHERE g.manco_entity_id = $1
                GROUP BY g.group_id
                LIMIT 1
                "#,
            )
            .bind(manco_entity_id)
            .fetch_optional(scope.executor())
            .await?;

        let group_info = match group_row {
            Some((group_id, manco_id, name, code, gtype, jur, up_id, count)) => json!({
                "group_id": group_id,
                "manco_entity_id": manco_id,
                "group_name": name,
                "group_code": code,
                "group_type": gtype,
                "jurisdiction": jur,
                "ultimate_parent_entity_id": up_id,
                "cbu_count": count,
            }),
            None => json!(null),
        };

        // 2. CBUs
        type CbuRow = (
            Uuid,
            String,
            String,
            Option<String>,
            Option<Uuid>,
            Option<String>,
            String,
        );
        let cbu_rows: Vec<CbuRow> =
            sqlx::query_as(r#"SELECT * FROM "ob-poc".fn_get_manco_group_cbus($1)"#)
                .bind(manco_entity_id)
                .fetch_all(scope.executor())
                .await?;

        let cbus: Vec<Value> = cbu_rows
            .into_iter()
            .map(|(cbu_id, cbu_name, cbu_category, jurisdiction, fund_entity_id, fund_entity_name, membership_source)| {
                json!({
                    "cbu_id": cbu_id,
                    "cbu_name": cbu_name,
                    "cbu_category": cbu_category,
                    "jurisdiction": jurisdiction,
                    "fund_entity_id": fund_entity_id,
                    "fund_entity_name": fund_entity_name,
                    "membership_source": membership_source,
                })
            })
            .collect();

        // 3. Control chain (max depth 5)
        type ChainRow = (
            i32,
            Uuid,
            String,
            Option<String>,
            Option<Uuid>,
            Option<String>,
            Option<String>,
            Option<Decimal>,
            bool,
        );
        let chain_rows: Vec<ChainRow> = sqlx::query_as(
            r#"SELECT * FROM "ob-poc".fn_manco_group_control_chain($1, $2)"#,
        )
        .bind(manco_entity_id)
        .bind(5i32)
        .fetch_all(scope.executor())
        .await?;

        let control_chain: Vec<Value> = chain_rows
            .into_iter()
            .map(|(depth, entity_id, entity_name, entity_type, controlled_by_id, controlled_by_name, control_type, voting_pct, is_ultimate)| {
                json!({
                    "depth": depth,
                    "entity_id": entity_id,
                    "entity_name": entity_name,
                    "entity_type": entity_type,
                    "controlled_by_entity_id": controlled_by_id,
                    "controlled_by_name": controlled_by_name,
                    "control_type": control_type,
                    "voting_pct": voting_pct,
                    "is_ultimate_controller": is_ultimate,
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::Record(json!({
            "group": group_info,
            "cbus": cbus,
            "control_chain": control_chain,
        })))
    }
}

// ── ownership.control-links.compute ───────────────────────────────────────────

pub struct ComputeControlLinks;

#[async_trait]
impl SemOsVerbOp for ComputeControlLinks {
    fn fqn(&self) -> &str {
        "ownership.control-links.compute"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let issuer_entity_id = json_extract_uuid_opt(args, ctx, "issuer-entity-id");
        let as_of = json_extract_date_opt(args, "as-of");

        let count: i32 = sqlx::query_scalar(r#"SELECT "ob-poc".fn_compute_control_links($1, $2)"#)
            .bind(issuer_entity_id)
            .bind(as_of)
            .fetch_one(scope.executor())
            .await?;

        let result = ComputeControlLinksResult {
            links_created: count,
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

// ── ownership.refresh ─────────────────────────────────────────────────────────

pub struct Refresh;

#[async_trait]
impl SemOsVerbOp for Refresh {
    fn fqn(&self) -> &str {
        "ownership.refresh"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let as_of = json_extract_date_opt(args, "as-of");

        let manco_row: (i32, i32) =
            sqlx::query_as(r#"SELECT * FROM "ob-poc".fn_bridge_manco_role_to_board_rights($1)"#)
                .bind(as_of)
                .fetch_one(scope.executor())
                .await?;
        let manco_bridge = BridgeRolesResult {
            rights_created: manco_row.0,
            rights_updated: manco_row.1,
        };

        let gleif_row: (i32, i32) = sqlx::query_as(
            r#"SELECT * FROM "ob-poc".fn_bridge_gleif_fund_manager_to_board_rights($1)"#,
        )
        .bind(as_of)
        .fetch_one(scope.executor())
        .await?;
        let gleif_bridge = BridgeGleifResult {
            rights_created: gleif_row.0,
            rights_updated: gleif_row.1,
        };

        let bods_row: (i32, i32, i32) =
            sqlx::query_as(r#"SELECT * FROM "ob-poc".fn_bridge_bods_to_holdings($1)"#)
                .bind(as_of)
                .fetch_one(scope.executor())
                .await?;
        let bods_bridge = BridgeBodsResult {
            holdings_created: bods_row.0,
            holdings_updated: bods_row.1,
            entities_linked: bods_row.2,
        };

        let links_count: i32 =
            sqlx::query_scalar(r#"SELECT "ob-poc".fn_compute_control_links(NULL, $1)"#)
                .bind(as_of)
                .fetch_one(scope.executor())
                .await?;
        let control_links = ComputeControlLinksResult {
            links_created: links_count,
        };

        let groups_row: (i32, i32) =
            sqlx::query_as(r#"SELECT * FROM "ob-poc".fn_derive_cbu_groups($1)"#)
                .bind(as_of)
                .fetch_one(scope.executor())
                .await?;
        let groups = DeriveGroupsResult {
            groups_created: groups_row.0,
            memberships_created: groups_row.1,
        };

        let result = GovernanceRefreshResult {
            manco_bridge,
            gleif_bridge,
            bods_bridge,
            control_links,
            groups,
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}
