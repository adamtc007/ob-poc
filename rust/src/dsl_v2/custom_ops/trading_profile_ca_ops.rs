//! Trading Profile Corporate Actions Operations
//!
//! Intent-tier handlers for `trading-profile.ca.*` verbs.
//! These modify the matrix JSONB document (source of truth).
//! Operational table writes happen during `trading-profile.materialize`.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;
use uuid::Uuid;

use super::{CustomOperation, ExecutionResult};
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::ExecutionContext;
use crate::trading_profile::ast_db;
use ob_poc_types::trading_matrix::{
    CaCutoffRule, CaDefaultOption, CaElectionPolicy, CaElector, CaNotificationPolicy,
    CaProceedsSsiMapping, CaProceedsType, TradingMatrixCorporateActions,
};

#[cfg(feature = "database")]
use rust_decimal::Decimal;
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

// =============================================================================
// CA.ENABLE-EVENT-TYPES
// =============================================================================

/// Enable CA event types for this trading profile
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let profile_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "profile-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing profile-id argument"))?;

        let event_types: Vec<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "event-types")
            .and_then(|a| {
                a.value.as_list().map(|list| {
                    list.iter()
                        .filter_map(|node| node.as_string().map(|s| s.to_string()))
                        .collect()
                })
            })
            .ok_or_else(|| anyhow::anyhow!("Missing event-types argument"))?;

        let (doc, mut ca) = load_ca_section(pool, profile_id).await?;

        // Merge event types (don't duplicate)
        for et in event_types {
            if !ca.enabled_event_types.contains(&et) {
                ca.enabled_event_types.push(et);
            }
        }

        let doc = save_ca_section(pool, profile_id, doc, ca.clone()).await?;

        Ok(ExecutionResult::Record(json!({
            "profile_id": profile_id,
            "enabled_count": ca.enabled_event_types.len(),
            "version": doc.version,
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(json!({"enabled_count": 0})))
    }
}

// =============================================================================
// CA.DISABLE-EVENT-TYPES
// =============================================================================

/// Disable CA event types for this trading profile
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let profile_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "profile-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing profile-id argument"))?;

        let event_types: Vec<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "event-types")
            .and_then(|a| {
                a.value.as_list().map(|list| {
                    list.iter()
                        .filter_map(|node| node.as_string().map(|s| s.to_string()))
                        .collect()
                })
            })
            .ok_or_else(|| anyhow::anyhow!("Missing event-types argument"))?;

        let (doc, mut ca) = load_ca_section(pool, profile_id).await?;

        // Remove event types
        ca.enabled_event_types
            .retain(|et| !event_types.contains(et));

        save_ca_section(pool, profile_id, doc, ca).await?;

        Ok(ExecutionResult::Affected(event_types.len() as u64))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Affected(0))
    }
}

// =============================================================================
// CA.SET-NOTIFICATION-POLICY
// =============================================================================

/// Configure CA notification settings
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let profile_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "profile-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing profile-id argument"))?;

        let channels: Vec<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "channels")
            .and_then(|a| {
                a.value.as_list().map(|list| {
                    list.iter()
                        .filter_map(|node| node.as_string().map(|s| s.to_string()))
                        .collect()
                })
            })
            .ok_or_else(|| anyhow::anyhow!("Missing channels argument"))?;

        let sla_hours = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "sla-hours")
            .and_then(|a| a.value.as_integer())
            .map(|v| v as i32);

        let escalation_contact = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "escalation-contact")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let (doc, mut ca) = load_ca_section(pool, profile_id).await?;

        ca.notification_policy = Some(CaNotificationPolicy {
            channels,
            sla_hours,
            escalation_contact,
        });

        save_ca_section(pool, profile_id, doc, ca).await?;

        Ok(ExecutionResult::Affected(1))
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

// =============================================================================
// CA.SET-ELECTION-POLICY
// =============================================================================

/// Configure who makes CA elections and requirements
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let profile_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "profile-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing profile-id argument"))?;

        let elector_str = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "elector")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing elector argument"))?;

        let elector = match elector_str {
            "investment_manager" => CaElector::InvestmentManager,
            "admin" => CaElector::Admin,
            "client" => CaElector::Client,
            _ => return Err(anyhow::anyhow!("Invalid elector value: {}", elector_str)),
        };

        let evidence_required = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "evidence-required")
            .and_then(|a| a.value.as_boolean())
            .unwrap_or(true);

        let auto_instruct_threshold = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "auto-instruct-threshold")
            .and_then(|a| a.value.as_decimal())
            .map(Decimal::from);

        let (doc, mut ca) = load_ca_section(pool, profile_id).await?;

        ca.election_policy = Some(CaElectionPolicy {
            elector,
            evidence_required,
            auto_instruct_threshold,
        });

        save_ca_section(pool, profile_id, doc, ca).await?;

        Ok(ExecutionResult::Affected(1))
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

// =============================================================================
// CA.SET-DEFAULT-OPTION
// =============================================================================

/// Set default election for specific event type
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let profile_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "profile-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing profile-id argument"))?;

        let event_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "event-type")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing event-type argument"))?;

        let default_option = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "default-option")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing default-option argument"))?;

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

        Ok(ExecutionResult::Affected(1))
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

// =============================================================================
// CA.REMOVE-DEFAULT-OPTION
// =============================================================================

/// Remove default election for specific event type
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let profile_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "profile-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing profile-id argument"))?;

        let event_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "event-type")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing event-type argument"))?;

        let (doc, mut ca) = load_ca_section(pool, profile_id).await?;

        ca.default_options.retain(|o| o.event_type != event_type);

        save_ca_section(pool, profile_id, doc, ca).await?;

        Ok(ExecutionResult::Affected(1))
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

// =============================================================================
// CA.ADD-CUTOFF-RULE
// =============================================================================

/// Add deadline cutoff rule for market/depository
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let profile_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "profile-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing profile-id argument"))?;

        let event_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "event-type")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let market_code = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "market-code")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let depository_code = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "depository-code")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let days_before = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "days-before")
            .and_then(|a| a.value.as_integer())
            .map(|v| v as i32)
            .ok_or_else(|| anyhow::anyhow!("Missing days-before argument"))?;

        let warning_days = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "warning-days")
            .and_then(|a| a.value.as_integer())
            .map(|v| v as i32)
            .unwrap_or(3);

        let escalation_days = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "escalation-days")
            .and_then(|a| a.value.as_integer())
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

        Ok(ExecutionResult::Affected(1))
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

// =============================================================================
// CA.REMOVE-CUTOFF-RULE
// =============================================================================

/// Remove cutoff rule by market/depository
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let profile_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "profile-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing profile-id argument"))?;

        let market_code = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "market-code")
            .and_then(|a| a.value.as_string());

        let depository_code = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "depository-code")
            .and_then(|a| a.value.as_string());

        let (doc, mut ca) = load_ca_section(pool, profile_id).await?;

        ca.cutoff_rules.retain(|r| {
            !(r.market_code.as_deref() == market_code
                && r.depository_code.as_deref() == depository_code)
        });

        save_ca_section(pool, profile_id, doc, ca).await?;

        Ok(ExecutionResult::Affected(1))
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

// =============================================================================
// CA.LINK-PROCEEDS-SSI
// =============================================================================

/// Map CA proceeds to settlement instruction
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let profile_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "profile-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing profile-id argument"))?;

        let proceeds_type_str = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "proceeds-type")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing proceeds-type argument"))?;

        let proceeds_type = match proceeds_type_str {
            "cash" => CaProceedsType::Cash,
            "stock" => CaProceedsType::Stock,
            _ => {
                return Err(anyhow::anyhow!(
                    "Invalid proceeds-type: {}",
                    proceeds_type_str
                ))
            }
        };

        let currency = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "currency")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let ssi_reference = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "ssi-name")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing ssi-name argument"))?;

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

        Ok(ExecutionResult::Affected(1))
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

// =============================================================================
// CA.REMOVE-PROCEEDS-SSI
// =============================================================================

/// Remove CA proceeds SSI mapping
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let profile_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "profile-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing profile-id argument"))?;

        let proceeds_type_str = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "proceeds-type")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing proceeds-type argument"))?;

        let proceeds_type = match proceeds_type_str {
            "cash" => CaProceedsType::Cash,
            "stock" => CaProceedsType::Stock,
            _ => {
                return Err(anyhow::anyhow!(
                    "Invalid proceeds-type: {}",
                    proceeds_type_str
                ))
            }
        };

        let currency = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "currency")
            .and_then(|a| a.value.as_string());

        let (doc, mut ca) = load_ca_section(pool, profile_id).await?;

        ca.proceeds_ssi_mappings.retain(|m| {
            !(std::mem::discriminant(&m.proceeds_type) == std::mem::discriminant(&proceeds_type)
                && m.currency.as_deref() == currency)
        });

        save_ca_section(pool, profile_id, doc, ca).await?;

        Ok(ExecutionResult::Affected(1))
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

// =============================================================================
// CA.GET-POLICY
// =============================================================================

/// Get current CA policy configuration from trading profile
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let profile_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "profile-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing profile-id argument"))?;

        let (_, ca) = load_ca_section(pool, profile_id).await?;

        Ok(ExecutionResult::Record(serde_json::to_value(&ca)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(json!({})))
    }
}
