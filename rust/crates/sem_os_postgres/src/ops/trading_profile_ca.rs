//! Trading profile corporate-actions verbs (11 plugin verbs) —
//! YAML-first re-implementation of `trading-profile.ca.*` from
//! `rust/config/verbs/trading-profile.yaml`.
//!
//! Persistence is delegated to the host via the
//! [`TradingProfileDocument`] service trait — these ops modify the
//! trading matrix JSONB document (source of truth). Operational table
//! writes happen at `trading-profile.materialize` time, not here.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_bool_opt, json_extract_int, json_extract_int_opt, json_extract_string,
    json_extract_string_list, json_extract_string_opt, json_extract_uuid,
};
use dsl_runtime::service_traits::TradingProfileDocument;
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};
use ob_poc_types::trading_matrix::{
    CaCutoffRule, CaDefaultOption, CaElectionPolicy, CaElector, CaNotificationPolicy,
    CaProceedsSsiMapping, CaProceedsType, TradingMatrixCorporateActions, TradingMatrixDocument,
};

use super::SemOsVerbOp;

async fn load_ca_section(
    docs: &dyn TradingProfileDocument,
    profile_id: Uuid,
) -> Result<(TradingMatrixDocument, TradingMatrixCorporateActions)> {
    let doc = docs.load_document(profile_id).await?;
    let ca = doc.corporate_actions.clone().unwrap_or_default();
    Ok((doc, ca))
}

async fn save_ca_section(
    docs: &dyn TradingProfileDocument,
    profile_id: Uuid,
    mut doc: TradingMatrixDocument,
    ca: TradingMatrixCorporateActions,
) -> Result<TradingMatrixDocument> {
    doc.corporate_actions = Some(ca);
    docs.save_document(profile_id, &doc).await?;
    Ok(doc)
}

fn json_extract_decimal_opt(args: &Value, arg_name: &str) -> Option<rust_decimal::Decimal> {
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

pub struct EnableEventTypes;

#[async_trait]
impl SemOsVerbOp for EnableEventTypes {
    fn fqn(&self) -> &str {
        "trading-profile.ca.enable-event-types"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let profile_id = json_extract_uuid(args, ctx, "profile-id")?;
        let event_types = json_extract_string_list(args, "event-types")?;
        let docs = ctx.service::<dyn TradingProfileDocument>()?;
        let (doc, mut ca) = load_ca_section(docs.as_ref(), profile_id).await?;
        for et in event_types {
            if !ca.enabled_event_types.contains(&et) {
                ca.enabled_event_types.push(et);
            }
        }
        let doc = save_ca_section(docs.as_ref(), profile_id, doc, ca.clone()).await?;
        Ok(VerbExecutionOutcome::Record(json!({
            "profile_id": profile_id,
            "enabled_count": ca.enabled_event_types.len(),
            "version": doc.version,
        })))
    }
}

pub struct DisableEventTypes;

#[async_trait]
impl SemOsVerbOp for DisableEventTypes {
    fn fqn(&self) -> &str {
        "trading-profile.ca.disable-event-types"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let profile_id = json_extract_uuid(args, ctx, "profile-id")?;
        let event_types = json_extract_string_list(args, "event-types")?;
        let docs = ctx.service::<dyn TradingProfileDocument>()?;
        let (doc, mut ca) = load_ca_section(docs.as_ref(), profile_id).await?;
        ca.enabled_event_types
            .retain(|et| !event_types.contains(et));
        save_ca_section(docs.as_ref(), profile_id, doc, ca).await?;
        Ok(VerbExecutionOutcome::Affected(event_types.len() as u64))
    }
}

pub struct SetNotificationPolicy;

#[async_trait]
impl SemOsVerbOp for SetNotificationPolicy {
    fn fqn(&self) -> &str {
        "trading-profile.ca.set-notification-policy"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let profile_id = json_extract_uuid(args, ctx, "profile-id")?;
        let channels = json_extract_string_list(args, "channels")?;
        let sla_hours = json_extract_int_opt(args, "sla-hours").map(|v| v as i32);
        let escalation_contact = json_extract_string_opt(args, "escalation-contact");
        let docs = ctx.service::<dyn TradingProfileDocument>()?;
        let (doc, mut ca) = load_ca_section(docs.as_ref(), profile_id).await?;
        ca.notification_policy = Some(CaNotificationPolicy {
            channels,
            sla_hours,
            escalation_contact,
        });
        save_ca_section(docs.as_ref(), profile_id, doc, ca).await?;
        Ok(VerbExecutionOutcome::Affected(1))
    }
}

pub struct SetElectionPolicy;

#[async_trait]
impl SemOsVerbOp for SetElectionPolicy {
    fn fqn(&self) -> &str {
        "trading-profile.ca.set-election-policy"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let profile_id = json_extract_uuid(args, ctx, "profile-id")?;
        let elector_str = json_extract_string(args, "elector")?;
        let elector = match elector_str.as_str() {
            "investment_manager" => CaElector::InvestmentManager,
            "admin" => CaElector::Admin,
            "client" => CaElector::Client,
            _ => return Err(anyhow!("Invalid elector value: {}", elector_str)),
        };
        let evidence_required = json_extract_bool_opt(args, "evidence-required").unwrap_or(true);
        let auto_instruct_threshold = json_extract_decimal_opt(args, "auto-instruct-threshold");
        let docs = ctx.service::<dyn TradingProfileDocument>()?;
        let (doc, mut ca) = load_ca_section(docs.as_ref(), profile_id).await?;
        ca.election_policy = Some(CaElectionPolicy {
            elector,
            evidence_required,
            auto_instruct_threshold,
        });
        save_ca_section(docs.as_ref(), profile_id, doc, ca).await?;
        Ok(VerbExecutionOutcome::Affected(1))
    }
}

pub struct SetDefaultOption;

#[async_trait]
impl SemOsVerbOp for SetDefaultOption {
    fn fqn(&self) -> &str {
        "trading-profile.ca.set-default-option"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let profile_id = json_extract_uuid(args, ctx, "profile-id")?;
        let event_type = json_extract_string(args, "event-type")?;
        let default_option = json_extract_string(args, "default-option")?;
        let docs = ctx.service::<dyn TradingProfileDocument>()?;
        let (doc, mut ca) = load_ca_section(docs.as_ref(), profile_id).await?;
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
        save_ca_section(docs.as_ref(), profile_id, doc, ca).await?;
        Ok(VerbExecutionOutcome::Affected(1))
    }
}

pub struct RemoveDefaultOption;

#[async_trait]
impl SemOsVerbOp for RemoveDefaultOption {
    fn fqn(&self) -> &str {
        "trading-profile.ca.remove-default-option"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let profile_id = json_extract_uuid(args, ctx, "profile-id")?;
        let event_type = json_extract_string(args, "event-type")?;
        let docs = ctx.service::<dyn TradingProfileDocument>()?;
        let (doc, mut ca) = load_ca_section(docs.as_ref(), profile_id).await?;
        ca.default_options.retain(|o| o.event_type != event_type);
        save_ca_section(docs.as_ref(), profile_id, doc, ca).await?;
        Ok(VerbExecutionOutcome::Affected(1))
    }
}

pub struct AddCutoffRule;

#[async_trait]
impl SemOsVerbOp for AddCutoffRule {
    fn fqn(&self) -> &str {
        "trading-profile.ca.add-cutoff-rule"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let profile_id = json_extract_uuid(args, ctx, "profile-id")?;
        let event_type = json_extract_string_opt(args, "event-type");
        let market_code = json_extract_string_opt(args, "market-code");
        let depository_code = json_extract_string_opt(args, "depository-code");
        let days_before = json_extract_int(args, "days-before")? as i32;
        let warning_days = json_extract_int_opt(args, "warning-days")
            .map(|v| v as i32)
            .unwrap_or(3);
        let escalation_days = json_extract_int_opt(args, "escalation-days")
            .map(|v| v as i32)
            .unwrap_or(1);
        let docs = ctx.service::<dyn TradingProfileDocument>()?;
        let (doc, mut ca) = load_ca_section(docs.as_ref(), profile_id).await?;
        ca.cutoff_rules.push(CaCutoffRule {
            event_type,
            market_code,
            depository_code,
            days_before,
            warning_days,
            escalation_days,
        });
        save_ca_section(docs.as_ref(), profile_id, doc, ca).await?;
        Ok(VerbExecutionOutcome::Affected(1))
    }
}

pub struct RemoveCutoffRule;

#[async_trait]
impl SemOsVerbOp for RemoveCutoffRule {
    fn fqn(&self) -> &str {
        "trading-profile.ca.remove-cutoff-rule"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let profile_id = json_extract_uuid(args, ctx, "profile-id")?;
        let market_code = json_extract_string_opt(args, "market-code");
        let depository_code = json_extract_string_opt(args, "depository-code");
        let docs = ctx.service::<dyn TradingProfileDocument>()?;
        let (doc, mut ca) = load_ca_section(docs.as_ref(), profile_id).await?;
        ca.cutoff_rules.retain(|r| {
            !(r.market_code.as_deref() == market_code.as_deref()
                && r.depository_code.as_deref() == depository_code.as_deref())
        });
        save_ca_section(docs.as_ref(), profile_id, doc, ca).await?;
        Ok(VerbExecutionOutcome::Affected(1))
    }
}

pub struct LinkProceedsSsi;

#[async_trait]
impl SemOsVerbOp for LinkProceedsSsi {
    fn fqn(&self) -> &str {
        "trading-profile.ca.link-proceeds-ssi"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let profile_id = json_extract_uuid(args, ctx, "profile-id")?;
        let proceeds_type_str = json_extract_string(args, "proceeds-type")?;
        let proceeds_type = match proceeds_type_str.as_str() {
            "cash" => CaProceedsType::Cash,
            "stock" => CaProceedsType::Stock,
            _ => return Err(anyhow!("Invalid proceeds-type: {}", proceeds_type_str)),
        };
        let currency = json_extract_string_opt(args, "currency");
        let ssi_reference = json_extract_string(args, "ssi-name")?;
        let docs = ctx.service::<dyn TradingProfileDocument>()?;
        let (doc, mut ca) = load_ca_section(docs.as_ref(), profile_id).await?;
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
        save_ca_section(docs.as_ref(), profile_id, doc, ca).await?;
        Ok(VerbExecutionOutcome::Affected(1))
    }
}

pub struct RemoveProceedsSsi;

#[async_trait]
impl SemOsVerbOp for RemoveProceedsSsi {
    fn fqn(&self) -> &str {
        "trading-profile.ca.remove-proceeds-ssi"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let profile_id = json_extract_uuid(args, ctx, "profile-id")?;
        let proceeds_type_str = json_extract_string(args, "proceeds-type")?;
        let proceeds_type = match proceeds_type_str.as_str() {
            "cash" => CaProceedsType::Cash,
            "stock" => CaProceedsType::Stock,
            _ => return Err(anyhow!("Invalid proceeds-type: {}", proceeds_type_str)),
        };
        let currency = json_extract_string_opt(args, "currency");
        let docs = ctx.service::<dyn TradingProfileDocument>()?;
        let (doc, mut ca) = load_ca_section(docs.as_ref(), profile_id).await?;
        ca.proceeds_ssi_mappings.retain(|m| {
            !(std::mem::discriminant(&m.proceeds_type) == std::mem::discriminant(&proceeds_type)
                && m.currency.as_deref() == currency.as_deref())
        });
        save_ca_section(docs.as_ref(), profile_id, doc, ca).await?;
        Ok(VerbExecutionOutcome::Affected(1))
    }
}

pub struct GetPolicy;

#[async_trait]
impl SemOsVerbOp for GetPolicy {
    fn fqn(&self) -> &str {
        "trading-profile.ca.get-policy"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let profile_id = json_extract_uuid(args, ctx, "profile-id")?;
        let docs = ctx.service::<dyn TradingProfileDocument>()?;
        let (_, ca) = load_ca_section(docs.as_ref(), profile_id).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(&ca)?))
    }
}
