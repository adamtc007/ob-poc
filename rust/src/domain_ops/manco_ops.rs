//! ManCo / Governance Controller Operations
//!
//! Operations for governance controller computation, group derivation, and data bridges.
//!
//! All operations use typed structs from `ob_poc_types::manco_group` for consistency.

use anyhow::Result;
use async_trait::async_trait;

use super::helpers::{extract_int_opt, extract_string_opt, extract_uuid, extract_uuid_opt};
use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

// Re-export types for consistent use
pub use ob_poc_types::manco_group::{
    BridgeBodsResult, BridgeGleifResult, BridgeRolesResult, CbuMancoNotFound, CbuMancoResult,
    ComputeControlLinksResult, ControlChainNode, ControlType, ControllerBasis, DeriveGroupsResult,
    GroupCbuEntry, PrimaryGovernanceController,
};

#[cfg(feature = "database")]
use sqlx::PgPool;

/// Extract optional date from verb args (as string, parsed to NaiveDate)
fn extract_date_opt(verb_call: &VerbCall, arg_name: &str) -> Option<chrono::NaiveDate> {
    extract_string_opt(verb_call, arg_name)
        .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
}

// =============================================================================
// Bridge Operations (data source → governance signals)
// =============================================================================

/// Bridge MANAGEMENT_COMPANY roles to BOARD_APPOINTMENT special rights
pub struct MancoBridgeRolesOp;

#[async_trait]
impl CustomOperation for MancoBridgeRolesOp {
    fn domain(&self) -> &'static str {
        "manco"
    }
    fn verb(&self) -> &'static str {
        "bridge.manco-roles"
    }
    fn rationale(&self) -> &'static str {
        "Bridges role assignments to special rights for governance controller computation"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let as_of = extract_date_opt(verb_call, "as-of");

        let row: (i32, i32) =
            sqlx::query_as("SELECT * FROM kyc.fn_bridge_manco_role_to_board_rights($1)")
                .bind(as_of)
                .fetch_one(pool)
                .await?;

        let result = BridgeRolesResult {
            rights_created: row.0,
            rights_updated: row.1,
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        let result = BridgeRolesResult {
            rights_created: 0,
            rights_updated: 0,
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }
}

/// Bridge GLEIF IS_FUND_MANAGED_BY relationships to BOARD_APPOINTMENT special rights
pub struct MancoBridgeGleifFundManagersOp;

#[async_trait]
impl CustomOperation for MancoBridgeGleifFundManagersOp {
    fn domain(&self) -> &'static str {
        "manco"
    }
    fn verb(&self) -> &'static str {
        "bridge.gleif-fund-managers"
    }
    fn rationale(&self) -> &'static str {
        "Bridges GLEIF fund manager relationships to special rights for governance controller"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let as_of = extract_date_opt(verb_call, "as-of");

        let row: (i32, i32) =
            sqlx::query_as("SELECT * FROM kyc.fn_bridge_gleif_fund_manager_to_board_rights($1)")
                .bind(as_of)
                .fetch_one(pool)
                .await?;

        let result = BridgeGleifResult {
            rights_created: row.0,
            rights_updated: row.1,
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        let result = BridgeGleifResult {
            rights_created: 0,
            rights_updated: 0,
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }
}

/// Bridge BODS ownership statements to kyc.holdings
pub struct MancoBridgeBodsOwnershipOp;

#[async_trait]
impl CustomOperation for MancoBridgeBodsOwnershipOp {
    fn domain(&self) -> &'static str {
        "manco"
    }
    fn verb(&self) -> &'static str {
        "bridge.bods-ownership"
    }
    fn rationale(&self) -> &'static str {
        "Bridges BODS ownership percentages to holdings for governance controller"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let as_of = extract_date_opt(verb_call, "as-of");

        let row: (i32, i32, i32) =
            sqlx::query_as("SELECT * FROM kyc.fn_bridge_bods_to_holdings($1)")
                .bind(as_of)
                .fetch_one(pool)
                .await?;

        let result = BridgeBodsResult {
            holdings_created: row.0,
            holdings_updated: row.1,
            entities_linked: row.2,
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        let result = BridgeBodsResult {
            holdings_created: 0,
            holdings_updated: 0,
            entities_linked: 0,
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }
}

// =============================================================================
// Group Derivation Operations
// =============================================================================

/// Derive CBU groups from governance controller
pub struct MancoGroupDeriveOp;

#[async_trait]
impl CustomOperation for MancoGroupDeriveOp {
    fn domain(&self) -> &'static str {
        "manco"
    }
    fn verb(&self) -> &'static str {
        "group.derive"
    }
    fn rationale(&self) -> &'static str {
        "Complex group derivation with governance controller signals and fallback logic"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let as_of = extract_date_opt(verb_call, "as-of");

        let row: (i32, i32) = sqlx::query_as(r#"SELECT * FROM "ob-poc".fn_derive_cbu_groups($1)"#)
            .bind(as_of)
            .fetch_one(pool)
            .await?;

        let result = DeriveGroupsResult {
            groups_created: row.0,
            memberships_created: row.1,
        };

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        let result = DeriveGroupsResult {
            groups_created: 0,
            memberships_created: 0,
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }
}

/// Get CBUs for a governance controller group
pub struct MancoGroupCbusOp;

#[async_trait]
impl CustomOperation for MancoGroupCbusOp {
    fn domain(&self) -> &'static str {
        "manco"
    }
    fn verb(&self) -> &'static str {
        "group.cbus"
    }
    fn rationale(&self) -> &'static str {
        "Calls function with entity lookup and returns structured result set"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let manco_entity_id = extract_uuid(verb_call, ctx, "manco-entity-id")?;

        let rows: Vec<(
            uuid::Uuid,
            String,
            String,
            Option<String>,
            Option<uuid::Uuid>,
            Option<String>,
            String,
        )> = sqlx::query_as(r#"SELECT * FROM "ob-poc".fn_get_manco_group_cbus($1)"#)
            .bind(manco_entity_id)
            .fetch_all(pool)
            .await?;

        let results: Vec<serde_json::Value> = rows
            .into_iter()
            .map(
                |(
                    cbu_id,
                    cbu_name,
                    cbu_category,
                    jurisdiction,
                    fund_entity_id,
                    fund_entity_name,
                    membership_source,
                )| {
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
                },
            )
            .collect();

        Ok(ExecutionResult::RecordSet(results))
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

/// Get governance controller for a CBU
pub struct MancoGroupForCbuOp;

#[async_trait]
impl CustomOperation for MancoGroupForCbuOp {
    fn domain(&self) -> &'static str {
        "manco"
    }
    fn verb(&self) -> &'static str {
        "group.for-cbu"
    }
    fn rationale(&self) -> &'static str {
        "Calls function with CBU lookup and returns structured result"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = extract_uuid(verb_call, ctx, "cbu-id")?;

        let row: Option<(
            uuid::Uuid,
            String,
            Option<String>,
            uuid::Uuid,
            String,
            String,
            String,
        )> = sqlx::query_as(r#"SELECT * FROM "ob-poc".fn_get_cbu_manco($1)"#)
            .bind(cbu_id)
            .fetch_optional(pool)
            .await?;

        match row {
            Some((
                manco_entity_id,
                manco_name,
                manco_lei,
                group_id,
                group_name,
                group_type,
                source,
            )) => {
                let result = CbuMancoResult {
                    manco_entity_id,
                    manco_name,
                    manco_lei,
                    group_id,
                    group_name,
                    group_type,
                    source,
                };
                Ok(ExecutionResult::Record(serde_json::to_value(result)?))
            }
            None => {
                let result = CbuMancoNotFound {
                    message: "No governance controller found for this CBU".to_string(),
                };
                Ok(ExecutionResult::Record(serde_json::to_value(result)?))
            }
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        let result = CbuMancoNotFound {
            message: "Database not available".to_string(),
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }
}

/// Get primary governance controller for an issuer
pub struct MancoPrimaryControllerOp;

#[async_trait]
impl CustomOperation for MancoPrimaryControllerOp {
    fn domain(&self) -> &'static str {
        "manco"
    }
    fn verb(&self) -> &'static str {
        "primary-controller"
    }
    fn rationale(&self) -> &'static str {
        "Complex governance controller computation with board rights, voting control, and tie-breaking"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use rust_decimal::Decimal;

        let issuer_entity_id = extract_uuid(verb_call, ctx, "issuer-entity-id")?;
        let as_of = extract_date_opt(verb_call, "as-of");

        let row: Option<(
            uuid::Uuid,         // issuer_entity_id
            Option<uuid::Uuid>, // primary_controller_entity_id
            Option<uuid::Uuid>, // governance_controller_entity_id
            Option<String>,     // basis
            Option<i32>,        // board_seats
            Option<Decimal>,    // voting_pct
            Option<Decimal>,    // economic_pct
            Option<bool>,       // has_control
            Option<bool>,       // has_significant_influence
        )> = sqlx::query_as("SELECT * FROM kyc.fn_primary_governance_controller($1, $2)")
            .bind(issuer_entity_id)
            .bind(as_of)
            .fetch_optional(pool)
            .await?;

        match row {
            Some((
                _,
                primary_controller,
                governance_controller,
                basis,
                board_seats,
                voting_pct,
                economic_pct,
                has_control,
                has_significant_influence,
            )) => {
                let result = PrimaryGovernanceController {
                    issuer_entity_id,
                    primary_controller_entity_id: primary_controller.unwrap_or(uuid::Uuid::nil()),
                    governance_controller_entity_id: governance_controller
                        .unwrap_or(uuid::Uuid::nil()),
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
                    has_significant_influence: has_significant_influence.unwrap_or(false),
                };
                Ok(ExecutionResult::Record(serde_json::to_value(result)?))
            }
            None => {
                let result = CbuMancoNotFound {
                    message: "No governance controller found".to_string(),
                };
                Ok(ExecutionResult::Record(serde_json::to_value(result)?))
            }
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        let result = CbuMancoNotFound {
            message: "Database not available".to_string(),
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }
}

/// Trace control chain upward from a governance controller
pub struct MancoControlChainOp;

#[async_trait]
impl CustomOperation for MancoControlChainOp {
    fn domain(&self) -> &'static str {
        "manco"
    }
    fn verb(&self) -> &'static str {
        "control-chain"
    }
    fn rationale(&self) -> &'static str {
        "Recursive CTE traversal of control chain to find ultimate parent"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use rust_decimal::Decimal;

        let manco_entity_id = extract_uuid(verb_call, ctx, "manco-entity-id")?;
        let max_depth = extract_int_opt(verb_call, "max-depth").unwrap_or(5);

        let rows: Vec<(
            i32,                // depth
            uuid::Uuid,         // entity_id
            String,             // entity_name
            Option<String>,     // entity_type
            Option<uuid::Uuid>, // controlled_by_entity_id
            Option<String>,     // controlled_by_name
            Option<String>,     // control_type
            Option<Decimal>,    // voting_pct
            bool,               // is_ultimate_controller
        )> = sqlx::query_as(r#"SELECT * FROM "ob-poc".fn_manco_group_control_chain($1, $2)"#)
            .bind(manco_entity_id)
            .bind(max_depth as i32)
            .fetch_all(pool)
            .await?;

        let results: Vec<serde_json::Value> = rows
            .into_iter()
            .map(
                |(
                    depth,
                    entity_id,
                    entity_name,
                    entity_type,
                    controlled_by_id,
                    controlled_by_name,
                    control_type_str,
                    voting_pct,
                    is_ultimate,
                )| {
                    let control_type = control_type_str.as_deref().map(|s| match s {
                        "CONTROLLING" => ControlType::Controlling,
                        "SIGNIFICANT_INFLUENCE" => ControlType::SignificantInfluence,
                        "MATERIAL" => ControlType::Material,
                        "NOTIFIABLE" => ControlType::Notifiable,
                        _ => ControlType::Minority,
                    });

                    let node = ControlChainNode {
                        depth,
                        entity_id,
                        entity_name,
                        entity_type,
                        controlled_by_entity_id: controlled_by_id,
                        controlled_by_name,
                        control_type,
                        voting_pct,
                        is_ultimate_controller: is_ultimate,
                    };
                    serde_json::to_value(node).unwrap_or_default()
                },
            )
            .collect();

        Ok(ExecutionResult::RecordSet(results))
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

// =============================================================================
// Ownership Operations
// =============================================================================

/// Compute control links from holdings
pub struct OwnershipComputeControlLinksOp;

#[async_trait]
impl CustomOperation for OwnershipComputeControlLinksOp {
    fn domain(&self) -> &'static str {
        "ownership"
    }
    fn verb(&self) -> &'static str {
        "control-links.compute"
    }
    fn rationale(&self) -> &'static str {
        "Materializes control links from holdings with threshold computation"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let issuer_entity_id = extract_uuid_opt(verb_call, ctx, "issuer-entity-id");
        let as_of = extract_date_opt(verb_call, "as-of");

        let count: i32 = sqlx::query_scalar("SELECT kyc.fn_compute_control_links($1, $2)")
            .bind(issuer_entity_id)
            .bind(as_of)
            .fetch_one(pool)
            .await?;

        let result = ComputeControlLinksResult {
            links_created: count,
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        let result = ComputeControlLinksResult { links_created: 0 };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }
}

// =============================================================================
// Registration Helper
// =============================================================================

use super::CustomOperationRegistry;
use std::sync::Arc;

pub fn register_manco_ops(registry: &mut CustomOperationRegistry) {
    // Bridge operations
    registry.register(Arc::new(MancoBridgeRolesOp));
    registry.register(Arc::new(MancoBridgeGleifFundManagersOp));
    registry.register(Arc::new(MancoBridgeBodsOwnershipOp));

    // Group operations
    registry.register(Arc::new(MancoGroupDeriveOp));
    registry.register(Arc::new(MancoGroupCbusOp));
    registry.register(Arc::new(MancoGroupForCbuOp));

    // Governance controller operations
    registry.register(Arc::new(MancoPrimaryControllerOp));
    registry.register(Arc::new(MancoControlChainOp));

    // Ownership operations
    registry.register(Arc::new(OwnershipComputeControlLinksOp));
}
