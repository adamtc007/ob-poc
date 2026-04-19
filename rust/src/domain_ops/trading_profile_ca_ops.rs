//! Trading Profile Corporate Actions Operations
//!
//! Intent-tier handlers for `trading-profile.ca.*` verbs.
//! These modify the matrix JSONB document (source of truth).
//! Operational table writes happen during `trading-profile.materialize`.

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use serde_json::json;
use uuid::Uuid;

use super::CustomOperation;
use crate::trading_profile::ast_db;
use ob_poc_types::trading_matrix::{
    CaCutoffRule, CaDefaultOption, CaElectionPolicy, CaElector, CaNotificationPolicy,
    CaProceedsSsiMapping, CaProceedsType, TradingMatrixCorporateActions,
};

#[cfg(feature = "database")]
use sqlx::PgPool;

// =============================================================================
// HELPER: Load and save CA section
// =============================================================================

#[cfg(feature = "database")]
async fn load_ca_section(
    pool: &PgPool,
    profile_id: Uuid,
) -> Result<(
    ob_poc_types::trading_matrix::TradingMatrixDocument,
    TradingMatrixCorporateActions,
)> {
    let doc = ast_db::load_document(pool, profile_id).await?;
    let ca = doc.corporate_actions.clone().unwrap_or_default();
    Ok((doc, ca))
}

#[cfg(feature = "database")]
async fn save_ca_section(
    pool: &PgPool,
    profile_id: Uuid,
    mut doc: ob_poc_types::trading_matrix::TradingMatrixDocument,
    ca: TradingMatrixCorporateActions,
) -> Result<ob_poc_types::trading_matrix::TradingMatrixDocument> {
    doc.corporate_actions = Some(ca);
    ast_db::save_document(pool, profile_id, &doc).await?;
    Ok(doc)
}

/// Extract an optional `Decimal` from JSON args (accepts number or string).
#[cfg(feature = "database")]
fn json_extract_decimal_opt(
    args: &serde_json::Value,
    arg_name: &str,
) -> Option<rust_decimal::Decimal> {
    use std::str::FromStr;
    args.get(arg_name).and_then(|v| {
        if let Some(s) = v.as_str() {
            rust_decimal::Decimal::from_str(s).ok()
        } else if let Some(f) = v.as_f64() {
            rust_decimal::Decimal::try_from(f).ok()
        } else if let Some(i) = v.as_i64() {
            Some(rust_decimal::Decimal::from(i))
        } else {
            None
        }
    })
}

// =============================================================================
// CA.ENABLE-EVENT-TYPES
// =============================================================================

/// Enable CA event types for this trading profile
#[register_custom_op]
pub struct TradingProfileCaEnableEventTypesOp;

#[async_trait]
impl CustomOperation for TradingProfileCaEnableEventTypesOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }
    fn verb(&self) -> &'static str {
        "ca.enable-event-types"
    }
    fn rationale(&self) -> &'static str {
        "Modifies matrix JSONB to enable CA event types"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let profile_id: Uuid = super::helpers::json_extract_uuid(args, ctx, "profile-id")?;

        let event_types: Vec<String> =
            super::helpers::json_extract_string_list(args, "event-types")?;

        let (doc, mut ca) = load_ca_section(pool, profile_id).await?;

        // Merge event types (don't duplicate)
        for et in event_types {
            if !ca.enabled_event_types.contains(&et) {
                ca.enabled_event_types.push(et);
            }
        }

        let doc = save_ca_section(pool, profile_id, doc, ca.clone()).await?;

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!({
            "profile_id": profile_id,
            "enabled_count": ca.enabled_event_types.len(),
            "version": doc.version,
        })))
    }
    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// CA.DISABLE-EVENT-TYPES
// =============================================================================

/// Disable CA event types for this trading profile
#[register_custom_op]
pub struct TradingProfileCaDisableEventTypesOp;

#[async_trait]
impl CustomOperation for TradingProfileCaDisableEventTypesOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }
    fn verb(&self) -> &'static str {
        "ca.disable-event-types"
    }
    fn rationale(&self) -> &'static str {
        "Modifies matrix JSONB to disable CA event types"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let profile_id: Uuid = super::helpers::json_extract_uuid(args, ctx, "profile-id")?;

        let event_types: Vec<String> =
            super::helpers::json_extract_string_list(args, "event-types")?;

        let (doc, mut ca) = load_ca_section(pool, profile_id).await?;

        // Remove event types
        ca.enabled_event_types
            .retain(|et| !event_types.contains(et));

        save_ca_section(pool, profile_id, doc, ca).await?;

        Ok(dsl_runtime::VerbExecutionOutcome::Affected(
            event_types.len() as u64,
        ))
    }
    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// CA.SET-NOTIFICATION-POLICY
// =============================================================================

/// Configure CA notification settings
#[register_custom_op]
pub struct TradingProfileCaSetNotificationOp;

#[async_trait]
impl CustomOperation for TradingProfileCaSetNotificationOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }
    fn verb(&self) -> &'static str {
        "ca.set-notification-policy"
    }
    fn rationale(&self) -> &'static str {
        "Modifies matrix JSONB to set CA notification policy"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let profile_id: Uuid = super::helpers::json_extract_uuid(args, ctx, "profile-id")?;

        let channels: Vec<String> = super::helpers::json_extract_string_list(args, "channels")?;

        let sla_hours = super::helpers::json_extract_int_opt(args, "sla-hours").map(|v| v as i32);

        let escalation_contact =
            super::helpers::json_extract_string_opt(args, "escalation-contact");

        let (doc, mut ca) = load_ca_section(pool, profile_id).await?;

        ca.notification_policy = Some(CaNotificationPolicy {
            channels,
            sla_hours,
            escalation_contact,
        });

        save_ca_section(pool, profile_id, doc, ca).await?;

        Ok(dsl_runtime::VerbExecutionOutcome::Affected(1))
    }
    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// CA.SET-ELECTION-POLICY
// =============================================================================

/// Configure who makes CA elections and requirements
#[register_custom_op]
pub struct TradingProfileCaSetElectionOp;

#[async_trait]
impl CustomOperation for TradingProfileCaSetElectionOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }
    fn verb(&self) -> &'static str {
        "ca.set-election-policy"
    }
    fn rationale(&self) -> &'static str {
        "Modifies matrix JSONB to set CA election policy"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let profile_id: Uuid = super::helpers::json_extract_uuid(args, ctx, "profile-id")?;

        let elector_str = super::helpers::json_extract_string(args, "elector")?;

        let elector = match elector_str.as_str() {
            "investment_manager" => CaElector::InvestmentManager,
            "admin" => CaElector::Admin,
            "client" => CaElector::Client,
            _ => return Err(anyhow::anyhow!("Invalid elector value: {}", elector_str)),
        };

        let evidence_required = super::helpers::json_extract_bool_opt(args, "evidence-required")
            .unwrap_or(true);

        let auto_instruct_threshold = json_extract_decimal_opt(args, "auto-instruct-threshold");

        let (doc, mut ca) = load_ca_section(pool, profile_id).await?;

        ca.election_policy = Some(CaElectionPolicy {
            elector,
            evidence_required,
            auto_instruct_threshold,
        });

        save_ca_section(pool, profile_id, doc, ca).await?;

        Ok(dsl_runtime::VerbExecutionOutcome::Affected(1))
    }
    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// CA.SET-DEFAULT-OPTION
// =============================================================================

/// Set default election for specific event type
#[register_custom_op]
pub struct TradingProfileCaSetDefaultOp;

#[async_trait]
impl CustomOperation for TradingProfileCaSetDefaultOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }
    fn verb(&self) -> &'static str {
        "ca.set-default-option"
    }
    fn rationale(&self) -> &'static str {
        "Modifies matrix JSONB to set default CA election"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let profile_id: Uuid = super::helpers::json_extract_uuid(args, ctx, "profile-id")?;

        let event_type = super::helpers::json_extract_string(args, "event-type")?;

        let default_option = super::helpers::json_extract_string(args, "default-option")?;

        let (doc, mut ca) = load_ca_section(pool, profile_id).await?;

        // Update or add default option
        if let Some(existing) = ca
            .default_options
            .iter_mut()
            .find(|o| o.event_type == event_type)
        {
            existing.default_option = default_option;
        } else {
            ca.default_options.push(CaDefaultOption {
                event_type,
                default_option,
            });
        }

        save_ca_section(pool, profile_id, doc, ca).await?;

        Ok(dsl_runtime::VerbExecutionOutcome::Affected(1))
    }
    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// CA.REMOVE-DEFAULT-OPTION
// =============================================================================

/// Remove default election for specific event type
#[register_custom_op]
pub struct TradingProfileCaRemoveDefaultOp;

#[async_trait]
impl CustomOperation for TradingProfileCaRemoveDefaultOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }
    fn verb(&self) -> &'static str {
        "ca.remove-default-option"
    }
    fn rationale(&self) -> &'static str {
        "Modifies matrix JSONB to remove default CA election"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let profile_id: Uuid = super::helpers::json_extract_uuid(args, ctx, "profile-id")?;

        let event_type = super::helpers::json_extract_string(args, "event-type")?;

        let (doc, mut ca) = load_ca_section(pool, profile_id).await?;

        ca.default_options.retain(|o| o.event_type != event_type);

        save_ca_section(pool, profile_id, doc, ca).await?;

        Ok(dsl_runtime::VerbExecutionOutcome::Affected(1))
    }
    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// CA.ADD-CUTOFF-RULE
// =============================================================================

/// Add deadline cutoff rule for market/depository
#[register_custom_op]
pub struct TradingProfileCaAddCutoffOp;

#[async_trait]
impl CustomOperation for TradingProfileCaAddCutoffOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }
    fn verb(&self) -> &'static str {
        "ca.add-cutoff-rule"
    }
    fn rationale(&self) -> &'static str {
        "Modifies matrix JSONB to add CA cutoff rule"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let profile_id: Uuid = super::helpers::json_extract_uuid(args, ctx, "profile-id")?;

        let event_type = super::helpers::json_extract_string_opt(args, "event-type");

        let market_code = super::helpers::json_extract_string_opt(args, "market-code");

        let depository_code = super::helpers::json_extract_string_opt(args, "depository-code");

        let days_before = super::helpers::json_extract_int(args, "days-before")? as i32;

        let warning_days = super::helpers::json_extract_int_opt(args, "warning-days")
            .map(|v| v as i32)
            .unwrap_or(3);

        let escalation_days = super::helpers::json_extract_int_opt(args, "escalation-days")
            .map(|v| v as i32)
            .unwrap_or(1);

        let (doc, mut ca) = load_ca_section(pool, profile_id).await?;

        ca.cutoff_rules.push(CaCutoffRule {
            event_type,
            market_code,
            depository_code,
            days_before,
            warning_days,
            escalation_days,
        });

        save_ca_section(pool, profile_id, doc, ca).await?;

        Ok(dsl_runtime::VerbExecutionOutcome::Affected(1))
    }
    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// CA.REMOVE-CUTOFF-RULE
// =============================================================================

/// Remove cutoff rule by market/depository
#[register_custom_op]
pub struct TradingProfileCaRemoveCutoffOp;

#[async_trait]
impl CustomOperation for TradingProfileCaRemoveCutoffOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }
    fn verb(&self) -> &'static str {
        "ca.remove-cutoff-rule"
    }
    fn rationale(&self) -> &'static str {
        "Modifies matrix JSONB to remove CA cutoff rule"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let profile_id: Uuid = super::helpers::json_extract_uuid(args, ctx, "profile-id")?;

        let market_code = super::helpers::json_extract_string_opt(args, "market-code");

        let depository_code = super::helpers::json_extract_string_opt(args, "depository-code");

        let (doc, mut ca) = load_ca_section(pool, profile_id).await?;

        ca.cutoff_rules.retain(|r| {
            !(r.market_code.as_deref() == market_code.as_deref()
                && r.depository_code.as_deref() == depository_code.as_deref())
        });

        save_ca_section(pool, profile_id, doc, ca).await?;

        Ok(dsl_runtime::VerbExecutionOutcome::Affected(1))
    }
    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// CA.LINK-PROCEEDS-SSI
// =============================================================================

/// Map CA proceeds to settlement instruction
#[register_custom_op]
pub struct TradingProfileCaLinkSsiOp;

#[async_trait]
impl CustomOperation for TradingProfileCaLinkSsiOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }
    fn verb(&self) -> &'static str {
        "ca.link-proceeds-ssi"
    }
    fn rationale(&self) -> &'static str {
        "Modifies matrix JSONB to link CA proceeds to SSI"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let profile_id: Uuid = super::helpers::json_extract_uuid(args, ctx, "profile-id")?;

        let proceeds_type_str = super::helpers::json_extract_string(args, "proceeds-type")?;

        let proceeds_type = match proceeds_type_str.as_str() {
            "cash" => CaProceedsType::Cash,
            "stock" => CaProceedsType::Stock,
            _ => {
                return Err(anyhow::anyhow!(
                    "Invalid proceeds-type: {}",
                    proceeds_type_str
                ));
            }
        };

        let currency = super::helpers::json_extract_string_opt(args, "currency");

        let ssi_reference = super::helpers::json_extract_string(args, "ssi-name")?;

        let (doc, mut ca) = load_ca_section(pool, profile_id).await?;

        // Update or add mapping
        if let Some(existing) = ca.proceeds_ssi_mappings.iter_mut().find(|m| {
            std::mem::discriminant(&m.proceeds_type) == std::mem::discriminant(&proceeds_type)
                && m.currency == currency
        }) {
            existing.ssi_reference = ssi_reference;
        } else {
            ca.proceeds_ssi_mappings.push(CaProceedsSsiMapping {
                proceeds_type,
                currency,
                ssi_reference,
            });
        }

        save_ca_section(pool, profile_id, doc, ca).await?;

        Ok(dsl_runtime::VerbExecutionOutcome::Affected(1))
    }
    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// CA.REMOVE-PROCEEDS-SSI
// =============================================================================

/// Remove CA proceeds SSI mapping
#[register_custom_op]
pub struct TradingProfileCaRemoveSsiOp;

#[async_trait]
impl CustomOperation for TradingProfileCaRemoveSsiOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }
    fn verb(&self) -> &'static str {
        "ca.remove-proceeds-ssi"
    }
    fn rationale(&self) -> &'static str {
        "Modifies matrix JSONB to remove CA proceeds SSI mapping"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let profile_id: Uuid = super::helpers::json_extract_uuid(args, ctx, "profile-id")?;

        let proceeds_type_str = super::helpers::json_extract_string(args, "proceeds-type")?;

        let proceeds_type = match proceeds_type_str.as_str() {
            "cash" => CaProceedsType::Cash,
            "stock" => CaProceedsType::Stock,
            _ => {
                return Err(anyhow::anyhow!(
                    "Invalid proceeds-type: {}",
                    proceeds_type_str
                ));
            }
        };

        let currency = super::helpers::json_extract_string_opt(args, "currency");

        let (doc, mut ca) = load_ca_section(pool, profile_id).await?;

        ca.proceeds_ssi_mappings.retain(|m| {
            !(std::mem::discriminant(&m.proceeds_type) == std::mem::discriminant(&proceeds_type)
                && m.currency.as_deref() == currency.as_deref())
        });

        save_ca_section(pool, profile_id, doc, ca).await?;

        Ok(dsl_runtime::VerbExecutionOutcome::Affected(1))
    }
    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// CA.GET-POLICY
// =============================================================================

/// Get current CA policy configuration from trading profile
#[register_custom_op]
pub struct TradingProfileCaGetPolicyOp;

#[async_trait]
impl CustomOperation for TradingProfileCaGetPolicyOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }
    fn verb(&self) -> &'static str {
        "ca.get-policy"
    }
    fn rationale(&self) -> &'static str {
        "Reads CA policy from matrix JSONB"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let profile_id: Uuid = super::helpers::json_extract_uuid(args, ctx, "profile-id")?;

        let (_, ca) = load_ca_section(pool, profile_id).await?;

        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            serde_json::to_value(&ca)?,
        ))
    }
    fn is_migrated(&self) -> bool {
        true
    }
}
