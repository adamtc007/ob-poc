//! Reference Data Bulk Loader Custom Operations
//!
//! Plugin handlers for loading reference data from YAML files into the database.
//! These operations support three modes:
//! - INSERT: Fail on conflict
//! - UPSERT: Update on conflict (default)
//! - REPLACE: Delete all first, then insert

// These structs deserialize from YAML - fields may not be used in code but are required for serde
#![allow(dead_code)]

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;

use ob_poc_macros::register_custom_op;

use crate::domain_ops::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

// ============================================================================
// YAML Data Structures
// ============================================================================

#[derive(Debug, Deserialize)]
struct MarketsYaml {
    markets: Vec<MarketEntry>,
}

#[derive(Debug, Deserialize)]
struct MarketEntry {
    mic: String,
    name: String,
    country_code: String,
    #[serde(default)]
    operating_mic: Option<String>,
    primary_currency: String,
    #[serde(default)]
    supported_currencies: Vec<String>,
    #[serde(default)]
    csd_bic: Option<String>,
    timezone: String,
    #[serde(default)]
    cut_off_time: Option<String>,
    #[serde(default = "default_true")]
    is_active: bool,
    #[serde(default)]
    settlement_cycle: Option<String>,
}

#[derive(Debug, Deserialize)]
struct InstrumentClassesYaml {
    instrument_classes: Vec<InstrumentClassEntry>,
}

#[derive(Debug, Deserialize)]
struct InstrumentClassEntry {
    code: String,
    name: String,
    #[serde(default)]
    parent: Option<String>,
    #[serde(default)]
    cfi_category: Option<String>,
    #[serde(default)]
    cfi_group: Option<String>,
    #[serde(default)]
    cfi_prefixes: Vec<String>,
    #[serde(default)]
    smpg_group: Option<String>,
    #[serde(default)]
    smpg_code: Option<String>,
    #[serde(default)]
    isda_asset_class: Option<String>,
    #[serde(default)]
    isda_base_product: Option<String>,
    #[serde(default)]
    settlement_cycle: Option<String>,
    #[serde(default)]
    requires_isda: bool,
    #[serde(default)]
    requires_collateral: bool,
    #[serde(default)]
    sweep_eligible: bool,
    #[serde(default = "default_true")]
    is_active: bool,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    children: Vec<InstrumentClassChild>,
}

#[derive(Debug, Deserialize)]
struct InstrumentClassChild {
    code: String,
    name: String,
    #[serde(default)]
    cfi_prefixes: Vec<String>,
    #[serde(default)]
    smpg_code: Option<String>,
    #[serde(default)]
    isda_asset_class: Option<String>,
    #[serde(default)]
    isda_base_product: Option<String>,
    #[serde(default)]
    settlement_cycle: Option<String>,
    #[serde(default)]
    requires_isda: bool,
    #[serde(default)]
    requires_collateral: bool,
    #[serde(default)]
    sweep_eligible: bool,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SubcustodianNetworkYaml {
    subcustodian_network: Vec<SubcustodianEntry>,
}

#[derive(Debug, Deserialize)]
struct SubcustodianEntry {
    market_mic: String,
    subcustodian_bic: String,
    subcustodian_name: String,
    #[serde(default)]
    local_agent_bic: Option<String>,
    #[serde(default)]
    local_agent_name: Option<String>,
    #[serde(default)]
    local_agent_account: Option<String>,
    #[serde(default)]
    csd_bic: Option<String>,
    #[serde(default)]
    csd_participant_id: Option<String>,
    #[serde(default)]
    is_direct: bool,
    #[serde(default = "default_true")]
    is_primary: bool,
    #[serde(default)]
    currencies: Vec<String>,
    #[serde(default)]
    effective_date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SlaTemplatesYaml {
    sla_templates: Vec<SlaTemplateEntry>,
}

#[derive(Debug, Deserialize)]
struct SlaTemplateEntry {
    template_code: String,
    name: String,
    #[serde(default)]
    category: Option<String>,
    applies_to_type: String,
    #[serde(default)]
    applies_to_code: Option<String>,
    metric_code: String,
    target_value: f64,
    #[serde(default)]
    warning_threshold: Option<f64>,
    #[serde(default)]
    measurement_unit: Option<String>,
    #[serde(default)]
    measurement_period: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    response_time_hours: Option<f64>,
    #[serde(default)]
    escalation_path: Option<String>,
    #[serde(default)]
    penalty_type: Option<String>,
    #[serde(default)]
    penalty_value: Option<f64>,
    #[serde(default)]
    penalty_unit: Option<String>,
    #[serde(default)]
    regulatory_requirement: bool,
    #[serde(default)]
    regulatory_reference: Option<String>,
    #[serde(default = "default_true")]
    is_active: bool,
}

fn default_true() -> bool {
    true
}

// ============================================================================
// Load Markets Operation
// ============================================================================

#[register_custom_op]
pub struct LoadMarketsOp;

#[async_trait]
impl CustomOperation for LoadMarketsOp {
    fn domain(&self) -> &'static str {
        "refdata"
    }
    fn verb(&self) -> &'static str {
        "load-markets"
    }
    fn rationale(&self) -> &'static str {
        "Bulk load markets from YAML with upsert semantics"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let file_path = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "file-path")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("Missing :file-path argument"))?;

        let mode = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "mode")
            .and_then(|a| a.value.as_string())
            .unwrap_or("UPSERT");

        // Read and parse YAML
        let yaml_content = std::fs::read_to_string(file_path)
            .map_err(|e| anyhow!("Failed to read file {}: {}", file_path, e))?;
        let data: MarketsYaml = serde_yaml::from_str(&yaml_content)
            .map_err(|e| anyhow!("Failed to parse YAML: {}", e))?;

        let mut inserted = 0;
        let mut skipped = 0;

        // Handle REPLACE mode
        if mode == "REPLACE" {
            sqlx::query("DELETE FROM custody.markets")
                .execute(pool)
                .await?;
        }

        for market in &data.markets {
            let result = if mode == "INSERT" {
                sqlx::query(
                    r#"
                    INSERT INTO custody.markets
                    (mic, name, country_code, operating_mic, primary_currency,
                     supported_currencies, csd_bic, timezone, is_active)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                    ON CONFLICT (mic) DO NOTHING
                    "#,
                )
                .bind(&market.mic)
                .bind(&market.name)
                .bind(&market.country_code)
                .bind(&market.operating_mic)
                .bind(&market.primary_currency)
                .bind(&market.supported_currencies)
                .bind(&market.csd_bic)
                .bind(&market.timezone)
                .bind(market.is_active)
                .execute(pool)
                .await?
            } else {
                sqlx::query(
                    r#"
                    INSERT INTO custody.markets
                    (mic, name, country_code, operating_mic, primary_currency,
                     supported_currencies, csd_bic, timezone, is_active)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                    ON CONFLICT (mic) DO UPDATE SET
                        name = EXCLUDED.name,
                        country_code = EXCLUDED.country_code,
                        operating_mic = EXCLUDED.operating_mic,
                        primary_currency = EXCLUDED.primary_currency,
                        supported_currencies = EXCLUDED.supported_currencies,
                        csd_bic = EXCLUDED.csd_bic,
                        timezone = EXCLUDED.timezone,
                        is_active = EXCLUDED.is_active,
                        updated_at = now()
                    "#,
                )
                .bind(&market.mic)
                .bind(&market.name)
                .bind(&market.country_code)
                .bind(&market.operating_mic)
                .bind(&market.primary_currency)
                .bind(&market.supported_currencies)
                .bind(&market.csd_bic)
                .bind(&market.timezone)
                .bind(market.is_active)
                .execute(pool)
                .await?
            };

            if result.rows_affected() > 0 {
                inserted += 1;
            } else {
                skipped += 1;
            }
        }

        Ok(ExecutionResult::Record(json!({
            "status": "success",
            "table": "custody.markets",
            "mode": mode,
            "total": data.markets.len(),
            "inserted": inserted,
            "skipped": skipped
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature not enabled"))
    }
}

// ============================================================================
// Load Instrument Classes Operation
// ============================================================================

#[register_custom_op]
pub struct LoadInstrumentClassesOp;

#[async_trait]
impl CustomOperation for LoadInstrumentClassesOp {
    fn domain(&self) -> &'static str {
        "refdata"
    }
    fn verb(&self) -> &'static str {
        "load-instrument-classes"
    }
    fn rationale(&self) -> &'static str {
        "Bulk load instrument class taxonomy from YAML with parent-child hierarchy"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        let file_path = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "file-path")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("Missing :file-path argument"))?;

        let mode = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "mode")
            .and_then(|a| a.value.as_string())
            .unwrap_or("UPSERT");

        // Read and parse YAML
        let yaml_content = std::fs::read_to_string(file_path)
            .map_err(|e| anyhow!("Failed to read file {}: {}", file_path, e))?;
        let data: InstrumentClassesYaml = serde_yaml::from_str(&yaml_content)
            .map_err(|e| anyhow!("Failed to parse YAML: {}", e))?;

        // Handle REPLACE mode
        if mode == "REPLACE" {
            // Must delete in reverse order due to FK constraints
            sqlx::query("DELETE FROM custody.instrument_classes WHERE parent_class_id IS NOT NULL")
                .execute(pool)
                .await?;
            sqlx::query("DELETE FROM custody.instrument_classes WHERE parent_class_id IS NULL")
                .execute(pool)
                .await?;
        }

        let mut inserted = 0;
        let mut code_to_id: HashMap<String, Uuid> = HashMap::new();

        // First pass: insert parent classes
        for ic in &data.instrument_classes {
            let settlement_cycle = ic
                .settlement_cycle
                .clone()
                .unwrap_or_else(|| "T+2".to_string());
            let cfi_cat = ic.cfi_category.as_ref().and_then(|s| s.chars().next());

            let row: (Uuid,) = sqlx::query_as(
                r#"
                INSERT INTO custody.instrument_classes
                (code, name, default_settlement_cycle, requires_isda, requires_collateral,
                 cfi_category, smpg_group, isda_asset_class, is_active)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                ON CONFLICT (code) DO UPDATE SET
                    name = EXCLUDED.name,
                    default_settlement_cycle = EXCLUDED.default_settlement_cycle,
                    requires_isda = EXCLUDED.requires_isda,
                    requires_collateral = EXCLUDED.requires_collateral,
                    cfi_category = EXCLUDED.cfi_category,
                    smpg_group = EXCLUDED.smpg_group,
                    isda_asset_class = EXCLUDED.isda_asset_class,
                    is_active = EXCLUDED.is_active,
                    updated_at = now()
                RETURNING class_id
                "#,
            )
            .bind(&ic.code)
            .bind(&ic.name)
            .bind(&settlement_cycle)
            .bind(ic.requires_isda)
            .bind(ic.requires_collateral)
            .bind(cfi_cat.map(|c| c.to_string()))
            .bind(&ic.smpg_group)
            .bind(&ic.isda_asset_class)
            .bind(ic.is_active)
            .fetch_one(pool)
            .await?;

            code_to_id.insert(ic.code.clone(), row.0);
            inserted += 1;

            // Second pass: insert children with parent reference
            for child in &ic.children {
                let child_settlement = child
                    .settlement_cycle
                    .clone()
                    .or_else(|| ic.settlement_cycle.clone())
                    .unwrap_or_else(|| "T+2".to_string());

                let child_row: (Uuid,) = sqlx::query_as(
                    r#"
                    INSERT INTO custody.instrument_classes
                    (code, name, default_settlement_cycle, requires_isda, requires_collateral,
                     smpg_group, isda_asset_class, parent_class_id, is_active)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                    ON CONFLICT (code) DO UPDATE SET
                        name = EXCLUDED.name,
                        default_settlement_cycle = EXCLUDED.default_settlement_cycle,
                        requires_isda = EXCLUDED.requires_isda,
                        requires_collateral = EXCLUDED.requires_collateral,
                        smpg_group = EXCLUDED.smpg_group,
                        isda_asset_class = EXCLUDED.isda_asset_class,
                        parent_class_id = EXCLUDED.parent_class_id,
                        is_active = EXCLUDED.is_active,
                        updated_at = now()
                    RETURNING class_id
                    "#,
                )
                .bind(&child.code)
                .bind(&child.name)
                .bind(&child_settlement)
                .bind(child.requires_isda || ic.requires_isda)
                .bind(child.requires_collateral || ic.requires_collateral)
                .bind(child.smpg_code.as_ref().or(ic.smpg_group.as_ref()))
                .bind(
                    child
                        .isda_asset_class
                        .as_ref()
                        .or(ic.isda_asset_class.as_ref()),
                )
                .bind(row.0)
                .bind(true)
                .fetch_one(pool)
                .await?;

                code_to_id.insert(child.code.clone(), child_row.0);
                inserted += 1;
            }
        }

        Ok(ExecutionResult::Record(json!({
            "status": "success",
            "table": "custody.instrument_classes",
            "mode": mode,
            "inserted": inserted
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature not enabled"))
    }
}

// ============================================================================
// Load Subcustodians Operation
// ============================================================================

#[register_custom_op]
pub struct LoadSubcustodiansOp;

#[async_trait]
impl CustomOperation for LoadSubcustodiansOp {
    fn domain(&self) -> &'static str {
        "refdata"
    }
    fn verb(&self) -> &'static str {
        "load-subcustodians"
    }
    fn rationale(&self) -> &'static str {
        "Bulk load subcustodian network from YAML with market FK resolution"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        let file_path = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "file-path")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("Missing :file-path argument"))?;

        let mode = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "mode")
            .and_then(|a| a.value.as_string())
            .unwrap_or("UPSERT");

        // Read and parse YAML
        let yaml_content = std::fs::read_to_string(file_path)
            .map_err(|e| anyhow!("Failed to read file {}: {}", file_path, e))?;
        let data: SubcustodianNetworkYaml = serde_yaml::from_str(&yaml_content)
            .map_err(|e| anyhow!("Failed to parse YAML: {}", e))?;

        // Build market MIC -> ID lookup
        let markets: Vec<(Uuid, String)> =
            sqlx::query_as("SELECT market_id, mic FROM custody.markets")
                .fetch_all(pool)
                .await?;
        let mic_to_id: HashMap<String, Uuid> =
            markets.into_iter().map(|(id, mic)| (mic, id)).collect();

        // Handle REPLACE mode
        if mode == "REPLACE" {
            sqlx::query("DELETE FROM custody.subcustodian_network")
                .execute(pool)
                .await?;
        }

        let mut inserted = 0;
        let mut skipped = 0;
        let mut errors: Vec<String> = Vec::new();

        for entry in &data.subcustodian_network {
            let market_id = match mic_to_id.get(&entry.market_mic) {
                Some(id) => *id,
                None => {
                    errors.push(format!("Market MIC not found: {}", entry.market_mic));
                    skipped += 1;
                    continue;
                }
            };

            // Determine PSET BIC (use csd_bic if available)
            let pset_bic = entry
                .csd_bic
                .clone()
                .unwrap_or_else(|| entry.subcustodian_bic.clone());

            let effective_date = entry
                .effective_date
                .as_ref()
                .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
                .unwrap_or_else(|| chrono::Utc::now().date_naive());

            // Process each currency for this subcustodian
            let currencies = if entry.currencies.is_empty() {
                vec!["USD".to_string()] // Default fallback
            } else {
                entry.currencies.clone()
            };

            for currency in &currencies {
                let result = sqlx::query(
                    r#"
                    INSERT INTO custody.subcustodian_network
                    (market_id, currency, subcustodian_bic, subcustodian_name,
                     local_agent_bic, local_agent_name, local_agent_account,
                     csd_participant_id, place_of_settlement_bic, is_primary,
                     effective_date, is_active)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, true)
                    ON CONFLICT (market_id, currency, subcustodian_bic, effective_date)
                    DO UPDATE SET
                        subcustodian_name = EXCLUDED.subcustodian_name,
                        local_agent_bic = EXCLUDED.local_agent_bic,
                        local_agent_name = EXCLUDED.local_agent_name,
                        local_agent_account = EXCLUDED.local_agent_account,
                        csd_participant_id = EXCLUDED.csd_participant_id,
                        place_of_settlement_bic = EXCLUDED.place_of_settlement_bic,
                        is_primary = EXCLUDED.is_primary,
                        updated_at = now()
                    "#,
                )
                .bind(market_id)
                .bind(currency)
                .bind(&entry.subcustodian_bic)
                .bind(&entry.subcustodian_name)
                .bind(&entry.local_agent_bic)
                .bind(&entry.local_agent_name)
                .bind(&entry.local_agent_account)
                .bind(&entry.csd_participant_id)
                .bind(&pset_bic)
                .bind(entry.is_primary)
                .bind(effective_date)
                .execute(pool)
                .await?;

                if result.rows_affected() > 0 {
                    inserted += 1;
                }
            }
        }

        Ok(ExecutionResult::Record(json!({
            "status": if errors.is_empty() { "success" } else { "partial" },
            "table": "custody.subcustodian_network",
            "mode": mode,
            "inserted": inserted,
            "skipped": skipped,
            "errors": errors
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature not enabled"))
    }
}

// ============================================================================
// Load SLA Templates Operation
// ============================================================================

#[register_custom_op]
pub struct LoadSlaTemplatesOp;

#[async_trait]
impl CustomOperation for LoadSlaTemplatesOp {
    fn domain(&self) -> &'static str {
        "refdata"
    }
    fn verb(&self) -> &'static str {
        "load-sla-templates"
    }
    fn rationale(&self) -> &'static str {
        "Bulk load SLA templates from YAML with metric type FK validation"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use rust_decimal::Decimal;
        use std::str::FromStr;

        let file_path = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "file-path")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("Missing :file-path argument"))?;

        let mode = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "mode")
            .and_then(|a| a.value.as_string())
            .unwrap_or("UPSERT");

        // Read and parse YAML
        let yaml_content = std::fs::read_to_string(file_path)
            .map_err(|e| anyhow!("Failed to read file {}: {}", file_path, e))?;
        let data: SlaTemplatesYaml = serde_yaml::from_str(&yaml_content)
            .map_err(|e| anyhow!("Failed to parse YAML: {}", e))?;

        // First, ensure all required metric types exist
        let required_metrics: std::collections::HashSet<&str> = data
            .sla_templates
            .iter()
            .map(|t| t.metric_code.as_str())
            .collect();

        for metric_code in &required_metrics {
            ensure_metric_type_exists(pool, metric_code).await?;
        }

        // Handle REPLACE mode
        if mode == "REPLACE" {
            sqlx::query(r#"DELETE FROM "ob-poc".sla_templates"#)
                .execute(pool)
                .await?;
        }

        let mut inserted = 0;
        let mut errors: Vec<String> = Vec::new();

        for template in &data.sla_templates {
            let target_value =
                Decimal::from_str(&template.target_value.to_string()).unwrap_or(Decimal::ZERO);
            let warning_threshold = template
                .warning_threshold
                .map(|v| Decimal::from_str(&v.to_string()).unwrap_or(Decimal::ZERO));
            let response_time = template
                .response_time_hours
                .map(|v| Decimal::from_str(&v.to_string()).unwrap_or(Decimal::ZERO));

            let result = sqlx::query(
                r#"
                INSERT INTO "ob-poc".sla_templates
                (template_code, name, description, applies_to_type, applies_to_code,
                 metric_code, target_value, warning_threshold, measurement_period,
                 response_time_hours, escalation_path, regulatory_requirement,
                 regulatory_reference, is_active)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                ON CONFLICT (template_code) DO UPDATE SET
                    name = EXCLUDED.name,
                    description = EXCLUDED.description,
                    applies_to_type = EXCLUDED.applies_to_type,
                    applies_to_code = EXCLUDED.applies_to_code,
                    metric_code = EXCLUDED.metric_code,
                    target_value = EXCLUDED.target_value,
                    warning_threshold = EXCLUDED.warning_threshold,
                    measurement_period = EXCLUDED.measurement_period,
                    response_time_hours = EXCLUDED.response_time_hours,
                    escalation_path = EXCLUDED.escalation_path,
                    regulatory_requirement = EXCLUDED.regulatory_requirement,
                    regulatory_reference = EXCLUDED.regulatory_reference,
                    is_active = EXCLUDED.is_active
                "#,
            )
            .bind(&template.template_code)
            .bind(&template.name)
            .bind(&template.description)
            .bind(&template.applies_to_type)
            .bind(&template.applies_to_code)
            .bind(&template.metric_code)
            .bind(target_value)
            .bind(warning_threshold)
            .bind(template.measurement_period.as_deref().unwrap_or("MONTHLY"))
            .bind(response_time)
            .bind(&template.escalation_path)
            .bind(template.regulatory_requirement)
            .bind(&template.regulatory_reference)
            .bind(template.is_active)
            .execute(pool)
            .await;

            match result {
                Ok(r) if r.rows_affected() > 0 => inserted += 1,
                Ok(_) => {}
                Err(e) => errors.push(format!("{}: {}", template.template_code, e)),
            }
        }

        Ok(ExecutionResult::Record(json!({
            "status": if errors.is_empty() { "success" } else { "partial" },
            "table": "ob-poc.sla_templates",
            "mode": mode,
            "total": data.sla_templates.len(),
            "inserted": inserted,
            "errors": errors
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature not enabled"))
    }
}

/// Ensure a metric type exists, creating it if necessary
#[cfg(feature = "database")]
async fn ensure_metric_type_exists(pool: &PgPool, metric_code: &str) -> Result<()> {
    // Check if exists
    let exists: Option<(String,)> = sqlx::query_as(
        r#"SELECT metric_code FROM "ob-poc".sla_metric_types WHERE metric_code = $1"#,
    )
    .bind(metric_code)
    .fetch_optional(pool)
    .await?;

    if exists.is_none() {
        // Generate a human-readable name from the code
        let name = metric_code
            .replace('_', " ")
            .split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    Some(c) => {
                        c.to_uppercase().collect::<String>()
                            + chars.as_str().to_lowercase().as_str()
                    }
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        // Infer metric_category from code
        // Valid: TIMELINESS, ACCURACY, AVAILABILITY, VOLUME, QUALITY
        let code_upper = metric_code.to_uppercase();
        let metric_category = if code_upper.contains("TIME")
            || code_upper.contains("LATENCY")
            || code_upper.contains("DURATION")
            || code_upper.contains("RESPONSE")
            || code_upper.contains("TURNAROUND")
            || code_upper.contains("CUTOFF")
            || code_upper.contains("DEADLINE")
        {
            "TIMELINESS"
        } else if code_upper.contains("RATE")
            || code_upper.contains("ACCURACY")
            || code_upper.contains("ERROR")
            || code_upper.contains("STP")
            || code_upper.contains("MATCH")
            || code_upper.contains("FAIL")
            || code_upper.contains("BREAK")
        {
            "ACCURACY"
        } else if code_upper.contains("UPTIME")
            || code_upper.contains("AVAILABILITY")
            || code_upper.contains("ONLINE")
        {
            "AVAILABILITY"
        } else if code_upper.contains("VOLUME")
            || code_upper.contains("COUNT")
            || code_upper.contains("CAPACITY")
            || code_upper.contains("THROUGHPUT")
        {
            "VOLUME"
        } else {
            // Default to QUALITY for other metrics
            "QUALITY"
        };

        // Infer unit from code
        // Valid: PERCENT, HOURS, MINUTES, SECONDS, COUNT, CURRENCY, BASIS_POINTS
        let unit = if code_upper.contains("RATE")
            || code_upper.contains("PERCENT")
            || code_upper.contains("STP")
            || code_upper.contains("ACCURACY")
            || code_upper.contains("UPTIME")
            || code_upper.contains("AVAILABILITY")
        {
            "PERCENT"
        } else if code_upper.contains("TIME") && code_upper.contains("HOUR") {
            "HOURS"
        } else if code_upper.contains("TIME") && code_upper.contains("MIN") {
            "MINUTES"
        } else if code_upper.contains("LATENCY") || code_upper.contains("SECOND") {
            "SECONDS"
        } else if code_upper.contains("COUNT")
            || code_upper.contains("VOLUME")
            || code_upper.contains("CAPACITY")
        {
            "COUNT"
        } else if code_upper.contains("COST") || code_upper.contains("FEE") {
            "CURRENCY"
        } else if code_upper.contains("BPS") || code_upper.contains("BASIS") {
            "BASIS_POINTS"
        } else if metric_category == "TIMELINESS" {
            "HOURS" // Default for timeliness
        } else if metric_category == "ACCURACY" || metric_category == "AVAILABILITY" {
            "PERCENT" // Default for accuracy/availability
        } else if metric_category == "VOLUME" {
            "COUNT" // Default for volume
        } else {
            "PERCENT" // Fallback default
        };

        // Determine if higher is better
        let higher_is_better = !code_upper.contains("ERROR")
            && !code_upper.contains("FAIL")
            && !code_upper.contains("BREAK")
            && !code_upper.contains("LATENCY")
            && !code_upper.contains("TIME"); // For time metrics, lower is usually better

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".sla_metric_types
            (metric_code, name, description, metric_category, unit, higher_is_better, is_active)
            VALUES ($1, $2, $3, $4, $5, $6, true)
            ON CONFLICT (metric_code) DO NOTHING
            "#,
        )
        .bind(metric_code)
        .bind(&name)
        .bind(format!("Auto-generated metric type for {}", metric_code))
        .bind(metric_category)
        .bind(unit)
        .bind(higher_is_better)
        .execute(pool)
        .await?;
    }

    Ok(())
}

// ============================================================================
// Load All Reference Data Operation
// ============================================================================

#[register_custom_op]
pub struct LoadAllRefdataOp;

#[async_trait]
impl CustomOperation for LoadAllRefdataOp {
    fn domain(&self) -> &'static str {
        "refdata"
    }
    fn verb(&self) -> &'static str {
        "load-all"
    }
    fn rationale(&self) -> &'static str {
        "Orchestrate loading all reference data in dependency order"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let directory = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "directory")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("Missing :directory argument"))?;

        let mode = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "mode")
            .and_then(|a| a.value.as_string())
            .unwrap_or("UPSERT");

        let dir_path = Path::new(directory);
        let mut results = serde_json::Map::new();

        // Helper to extract JSON from ExecutionResult
        fn result_to_json(result: ExecutionResult) -> serde_json::Value {
            match result {
                ExecutionResult::Record(v) => v,
                ExecutionResult::RecordSet(v) => serde_json::Value::Array(v),
                ExecutionResult::Uuid(u) => serde_json::Value::String(u.to_string()),
                ExecutionResult::Affected(n) => serde_json::Value::Number(n.into()),
                ExecutionResult::Void => serde_json::Value::Null,
                _ => serde_json::Value::Null,
            }
        }

        // Load in dependency order: markets -> instrument_classes -> subcustodians -> sla_templates

        // 1. Markets (no dependencies)
        let markets_path = dir_path.join("markets.yaml");
        if markets_path.exists() {
            let markets_call =
                create_load_call("load-markets", &markets_path.to_string_lossy(), mode);
            let result = LoadMarketsOp.execute(&markets_call, ctx, pool).await?;
            results.insert("markets".to_string(), result_to_json(result));
        }

        // 2. Instrument classes (no dependencies)
        let ic_path = dir_path.join("instrument_classes.yaml");
        if ic_path.exists() {
            let ic_call =
                create_load_call("load-instrument-classes", &ic_path.to_string_lossy(), mode);
            let result = LoadInstrumentClassesOp.execute(&ic_call, ctx, pool).await?;
            results.insert("instrument_classes".to_string(), result_to_json(result));
        }

        // 3. Subcustodians (depends on markets)
        let sub_path = dir_path.join("subcustodian_network.yaml");
        if sub_path.exists() {
            let sub_call =
                create_load_call("load-subcustodians", &sub_path.to_string_lossy(), mode);
            let result = LoadSubcustodiansOp.execute(&sub_call, ctx, pool).await?;
            results.insert("subcustodian_network".to_string(), result_to_json(result));
        }

        // 4. SLA templates (depends on metric types - auto-created)
        let sla_path = dir_path.join("sla_templates.yaml");
        if sla_path.exists() {
            let sla_call =
                create_load_call("load-sla-templates", &sla_path.to_string_lossy(), mode);
            let result = LoadSlaTemplatesOp.execute(&sla_call, ctx, pool).await?;
            results.insert("sla_templates".to_string(), result_to_json(result));
        }

        Ok(ExecutionResult::Record(json!({
            "status": "success",
            "directory": directory,
            "mode": mode,
            "results": results
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature not enabled"))
    }
}

/// Create a VerbCall for internal use
fn create_load_call(verb: &str, file_path: &str, mode: &str) -> VerbCall {
    use crate::dsl_v2::ast::{Argument, AstNode, Literal, Span};

    VerbCall {
        domain: "refdata".to_string(),
        verb: verb.to_string(),
        arguments: vec![
            Argument {
                key: "file-path".to_string(),
                value: AstNode::Literal(Literal::String(file_path.to_string())),
                span: Span { start: 0, end: 0 },
            },
            Argument {
                key: "mode".to_string(),
                value: AstNode::Literal(Literal::String(mode.to_string())),
                span: Span { start: 0, end: 0 },
            },
        ],
        binding: None,
        span: Span { start: 0, end: 0 },
    }
}

// ============================================================================
// Registration
// ============================================================================

/// Get all refdata loader operations for registration
pub fn get_refdata_operations() -> Vec<Box<dyn CustomOperation>> {
    vec![
        Box::new(LoadMarketsOp),
        Box::new(LoadInstrumentClassesOp),
        Box::new(LoadSubcustodiansOp),
        Box::new(LoadSlaTemplatesOp),
        Box::new(LoadAllRefdataOp),
    ]
}
