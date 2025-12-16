//! Trading Profile Custom Operations
//!
//! Handles import and materialization of trading profile documents.
//! The trading profile is a single JSONB document that is the source of truth
//! for a CBU's trading configuration.
//!
//! Materialization syncs the document to operational tables:
//! - custody.cbu_instrument_universe
//! - custody.cbu_ssi
//! - custody.ssi_booking_rules (NOTE: specificity_score is GENERATED - never insert it)
//! - custody.isda_agreements
//! - custody.csa_agreements
//! - custody.subcustodian_network

use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

use super::{CustomOperation, ExecutionResult};
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::ExecutionContext;
use crate::trading_profile::{
    resolve::resolve_entity_ref, BookingRule, IsdaAgreementConfig, MaterializationResult,
    StandingInstruction, TradingProfileDocument, TradingProfileImport,
};

#[cfg(feature = "database")]
use sqlx::{PgPool, Row};

// =============================================================================
// IMPORT OPERATION
// =============================================================================

/// Import a trading profile document from file or inline JSON
///
/// Rationale: Requires file I/O, YAML parsing, hash computation, and
/// document validation before storing to database.
pub struct TradingProfileImportOp;

#[async_trait]
impl CustomOperation for TradingProfileImportOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }
    fn verb(&self) -> &'static str {
        "import"
    }
    fn rationale(&self) -> &'static str {
        "Requires file I/O, YAML parsing, hash computation, and validation"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use sha2::{Digest, Sha256};

        // Get CBU ID (required)
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

        // Get file path or inline document
        let file_path = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "file-path")
            .and_then(|a| a.value.as_string());

        // For now, only file-based import is supported
        let file_path = file_path.ok_or_else(|| {
            anyhow::anyhow!("Missing :file-path argument. File-based import is required.")
        })?;

        // Read from file
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| anyhow::anyhow!("Failed to read file {}: {}", file_path, e))?;

        // Parse YAML (works for JSON too)
        let import: TradingProfileImport = serde_yaml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse YAML: {}", e))?;

        let (document, raw_content) = (import.into_document(), content);

        // Compute hash
        let mut hasher = Sha256::new();
        hasher.update(raw_content.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        // Get optional args
        let version: i32 = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "version")
            .and_then(|a| a.value.as_integer())
            .unwrap_or(1) as i32;

        let status_str = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "status")
            .and_then(|a| a.value.as_string())
            .unwrap_or("DRAFT");

        let notes = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "notes")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        // Convert to JSON for storage
        let document_json = serde_json::to_value(&document)?;

        // Insert profile
        let profile_id = Uuid::new_v4();

        sqlx::query(
            r#"INSERT INTO "ob-poc".cbu_trading_profiles
               (profile_id, cbu_id, version, status, document, document_hash, notes, created_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
               ON CONFLICT (cbu_id, version) DO UPDATE SET
                   document = EXCLUDED.document,
                   document_hash = EXCLUDED.document_hash,
                   notes = EXCLUDED.notes"#,
        )
        .bind(profile_id)
        .bind(cbu_id)
        .bind(version)
        .bind(status_str)
        .bind(&document_json)
        .bind(&hash)
        .bind(&notes)
        .execute(pool)
        .await?;

        ctx.bind("profile", profile_id);

        Ok(ExecutionResult::Uuid(profile_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(Uuid::new_v4()))
    }
}

// =============================================================================
// GET ACTIVE OPERATION
// =============================================================================

/// Get the active trading profile for a CBU
pub struct TradingProfileGetActiveOp;

#[async_trait]
impl CustomOperation for TradingProfileGetActiveOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }
    fn verb(&self) -> &'static str {
        "get-active"
    }
    fn rationale(&self) -> &'static str {
        "Custom query to find ACTIVE status profile"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
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

        let row = sqlx::query!(
            r#"SELECT profile_id, version, document, document_hash, created_at, activated_at
               FROM "ob-poc".cbu_trading_profiles
               WHERE cbu_id = $1 AND status = 'ACTIVE'
               LIMIT 1"#,
            cbu_id
        )
        .fetch_optional(pool)
        .await?;

        match row {
            Some(r) => Ok(ExecutionResult::Record(json!({
                "profile_id": r.profile_id,
                "version": r.version,
                "document": r.document,
                "document_hash": r.document_hash,
                "created_at": r.created_at,
                "activated_at": r.activated_at
            }))),
            None => Ok(ExecutionResult::Record(json!({
                "error": "No active trading profile found for CBU"
            }))),
        }
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

// =============================================================================
// ACTIVATE OPERATION
// =============================================================================

/// Activate a trading profile (sets status to ACTIVE, supersedes previous)
pub struct TradingProfileActivateOp;

#[async_trait]
impl CustomOperation for TradingProfileActivateOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }
    fn verb(&self) -> &'static str {
        "activate"
    }
    fn rationale(&self) -> &'static str {
        "Requires transaction to supersede previous active profile"
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

        let activated_by = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "activated-by")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        // Get cbu_id for this profile
        let cbu_id: Uuid = sqlx::query_scalar(
            r#"SELECT cbu_id FROM "ob-poc".cbu_trading_profiles WHERE profile_id = $1"#,
        )
        .bind(profile_id)
        .fetch_one(pool)
        .await?;

        let mut tx = pool.begin().await?;

        // Supersede any existing active profile for this CBU
        sqlx::query(
            r#"UPDATE "ob-poc".cbu_trading_profiles
               SET status = 'SUPERSEDED'
               WHERE cbu_id = $1 AND status = 'ACTIVE'"#,
        )
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;

        // Activate the new profile
        sqlx::query(
            r#"UPDATE "ob-poc".cbu_trading_profiles
               SET status = 'ACTIVE', activated_at = NOW(), activated_by = $2
               WHERE profile_id = $1"#,
        )
        .bind(profile_id)
        .bind(&activated_by)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(ExecutionResult::Record(json!({
            "profile_id": profile_id,
            "status": "ACTIVE",
            "activated_at": chrono::Utc::now()
        })))
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

// =============================================================================
// MATERIALIZE OPERATION
// =============================================================================

/// Materialize trading profile to operational tables
///
/// This is the core operation that syncs the document to:
/// - custody.cbu_instrument_universe
/// - custody.cbu_ssi
/// - custody.ssi_booking_rules (CRITICAL: specificity_score is GENERATED ALWAYS)
/// - custody.isda_agreements
/// - custody.csa_agreements
pub struct TradingProfileMaterializeOp;

#[async_trait]
impl CustomOperation for TradingProfileMaterializeOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }
    fn verb(&self) -> &'static str {
        "materialize"
    }
    fn rationale(&self) -> &'static str {
        "Complex multi-table sync with FK lookups and transaction management"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let start = std::time::Instant::now();

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

        let dry_run = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "dry-run")
            .and_then(|a| a.value.as_boolean())
            .unwrap_or(false);

        let force = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "force")
            .and_then(|a| a.value.as_boolean())
            .unwrap_or(false);

        let sections: Vec<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "sections")
            .and_then(|a| {
                // Convert list of AstNodes to strings
                a.value.as_list().map(|list| {
                    list.iter()
                        .filter_map(|node| node.as_string().map(|s| s.to_string()))
                        .collect()
                })
            })
            .unwrap_or_else(|| vec!["universe".into(), "ssis".into(), "booking_rules".into()]);

        // Load the profile document
        let row = sqlx::query!(
            r#"SELECT cbu_id, document FROM "ob-poc".cbu_trading_profiles WHERE profile_id = $1"#,
            profile_id
        )
        .fetch_one(pool)
        .await?;

        let cbu_id = row.cbu_id;
        let document: TradingProfileDocument = serde_json::from_value(row.document)?;

        let mut result = MaterializationResult {
            profile_id,
            sections_materialized: vec![],
            records_created: HashMap::new(),
            records_updated: HashMap::new(),
            records_deleted: HashMap::new(),
            errors: vec![],
            duration_ms: 0,
        };

        if dry_run {
            // Just return what would be done
            result.sections_materialized = sections.clone();
            // Count records that would be created
            if sections.contains(&"universe".to_string()) {
                let count = document.universe.allowed_markets.len()
                    * document.universe.instrument_classes.len();
                result
                    .records_created
                    .insert("cbu_instrument_universe".to_string(), count as i32);
            }
            if sections.contains(&"ssis".to_string()) {
                let mut ssi_count = 0;
                for ssis in document.standing_instructions.values() {
                    ssi_count += ssis.len();
                }
                result
                    .records_created
                    .insert("cbu_ssi".to_string(), ssi_count as i32);
            }
            if sections.contains(&"booking_rules".to_string()) {
                result.records_created.insert(
                    "ssi_booking_rules".to_string(),
                    document.booking_rules.len() as i32,
                );
            }
            result.duration_ms = start.elapsed().as_millis() as i64;
            return Ok(ExecutionResult::Record(serde_json::to_value(&result)?));
        }

        // Start transaction
        let mut tx = pool.begin().await?;

        // Build lookup caches
        let instrument_class_map = build_instrument_class_map(&mut tx).await?;
        let market_map = build_market_map(&mut tx).await?;

        // Materialize SSIs first (booking rules reference them)
        let mut ssi_name_to_id: HashMap<String, Uuid> = HashMap::new();

        if sections.contains(&"ssis".to_string()) {
            let mut created = 0;
            for (category, ssis) in &document.standing_instructions {
                for ssi in ssis {
                    let ssi_id =
                        materialize_ssi(&mut tx, cbu_id, category, ssi, &market_map, force).await?;
                    ssi_name_to_id.insert(ssi.name.clone(), ssi_id);
                    created += 1;
                }
            }
            result
                .records_created
                .insert("cbu_ssi".to_string(), created);
            result.sections_materialized.push("ssis".to_string());
        } else {
            // Load existing SSI name->id mapping for booking rules
            let rows = sqlx::query!(
                r#"SELECT ssi_id, ssi_name FROM custody.cbu_ssi WHERE cbu_id = $1"#,
                cbu_id
            )
            .fetch_all(&mut *tx)
            .await?;
            for row in rows {
                ssi_name_to_id.insert(row.ssi_name, row.ssi_id);
            }
        }

        // Materialize universe
        if sections.contains(&"universe".to_string()) {
            let created = materialize_universe(
                &mut tx,
                cbu_id,
                &document.universe,
                &instrument_class_map,
                &market_map,
                force,
            )
            .await?;
            result
                .records_created
                .insert("cbu_instrument_universe".to_string(), created);
            result.sections_materialized.push("universe".to_string());
        }

        // Materialize booking rules
        if sections.contains(&"booking_rules".to_string()) {
            let created = materialize_booking_rules(
                &mut tx,
                cbu_id,
                &document.booking_rules,
                &ssi_name_to_id,
                &instrument_class_map,
                &market_map,
                force,
            )
            .await?;
            result
                .records_created
                .insert("ssi_booking_rules".to_string(), created);
            result
                .sections_materialized
                .push("booking_rules".to_string());
        }

        // Materialize ISDA agreements and CSAs
        if sections.contains(&"isda".to_string()) {
            let created = materialize_isda_agreements(
                &mut tx,
                pool,
                cbu_id,
                &document.isda_agreements,
                &ssi_name_to_id,
            )
            .await?;
            result
                .records_created
                .insert("isda_agreements".to_string(), created);
            result.sections_materialized.push("isda".to_string());
        }

        tx.commit().await?;

        result.duration_ms = start.elapsed().as_millis() as i64;

        // Log materialization audit
        sqlx::query(
            r#"INSERT INTO "ob-poc".trading_profile_materializations
               (profile_id, sections_materialized, records_created, records_updated, records_deleted, duration_ms)
               VALUES ($1, $2, $3, $4, $5, $6)"#,
        )
        .bind(profile_id)
        .bind(&result.sections_materialized)
        .bind(serde_json::to_value(&result.records_created)?)
        .bind(serde_json::to_value(&result.records_updated)?)
        .bind(serde_json::to_value(&result.records_deleted)?)
        .bind(result.duration_ms as i32)
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Record(serde_json::to_value(&result)?))
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

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

#[cfg(feature = "database")]
async fn build_instrument_class_map(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<HashMap<String, Uuid>> {
    let rows = sqlx::query!(r#"SELECT class_id, code FROM custody.instrument_classes"#)
        .fetch_all(&mut **tx)
        .await?;

    Ok(rows.into_iter().map(|r| (r.code, r.class_id)).collect())
}

#[cfg(feature = "database")]
async fn build_market_map(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<HashMap<String, Uuid>> {
    let rows = sqlx::query!(r#"SELECT market_id, mic FROM custody.markets"#)
        .fetch_all(&mut **tx)
        .await?;

    Ok(rows.into_iter().map(|r| (r.mic, r.market_id)).collect())
}

#[cfg(feature = "database")]
async fn materialize_ssi(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    cbu_id: Uuid,
    category: &str,
    ssi: &StandingInstruction,
    market_map: &HashMap<String, Uuid>,
    force: bool,
) -> Result<Uuid> {
    let ssi_id = Uuid::new_v4();

    // Determine SSI type from category
    let ssi_type = match category {
        "CUSTODY" => "SECURITIES",
        "OTC_COLLATERAL" => "COLLATERAL",
        "FUND_ACCOUNTING" => "CASH",
        _ => "SECURITIES",
    };

    // Look up market_id if mic is specified
    let market_id = ssi.mic.as_ref().and_then(|m| market_map.get(m)).copied();

    let _conflict_action = if force { "DO UPDATE SET" } else { "DO NOTHING" };

    // Use raw query to handle ON CONFLICT properly
    let query = format!(
        r#"INSERT INTO custody.cbu_ssi
           (ssi_id, cbu_id, ssi_name, ssi_type, market_id,
            safekeeping_account, safekeeping_bic,
            cash_account, cash_account_bic, cash_currency,
            status, effective_date, source)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'ACTIVE', CURRENT_DATE, 'TRADING_PROFILE')
           ON CONFLICT (cbu_id, ssi_name) {}
           RETURNING ssi_id"#,
        if force {
            "DO UPDATE SET
                ssi_type = EXCLUDED.ssi_type,
                market_id = EXCLUDED.market_id,
                safekeeping_account = EXCLUDED.safekeeping_account,
                safekeeping_bic = EXCLUDED.safekeeping_bic,
                cash_account = EXCLUDED.cash_account,
                cash_account_bic = EXCLUDED.cash_account_bic,
                cash_currency = EXCLUDED.cash_currency,
                updated_at = NOW()"
        } else {
            "DO NOTHING"
        }
    );

    tracing::debug!(ssi_name = %ssi.name, "materialize_ssi: inserting SSI");
    let result: Option<(Uuid,)> = sqlx::query_as(&query)
        .bind(ssi_id)
        .bind(cbu_id)
        .bind(&ssi.name)
        .bind(ssi_type)
        .bind(market_id)
        .bind(&ssi.custody_account)
        .bind(&ssi.custody_bic)
        .bind(&ssi.cash_account)
        .bind(&ssi.cash_bic)
        .bind(&ssi.currency)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| {
            tracing::error!(ssi_name = %ssi.name, error = %e, "materialize_ssi: SSI insert failed");
            e
        })?;

    // If DO NOTHING and row exists, fetch existing ID
    if let Some((id,)) = result {
        Ok(id)
    } else {
        let existing: (Uuid,) = sqlx::query_as(
            r#"SELECT ssi_id FROM custody.cbu_ssi WHERE cbu_id = $1 AND ssi_name = $2"#,
        )
        .bind(cbu_id)
        .bind(&ssi.name)
        .fetch_one(&mut **tx)
        .await?;
        Ok(existing.0)
    }
}

#[cfg(feature = "database")]
async fn materialize_universe(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    cbu_id: Uuid,
    universe: &crate::trading_profile::Universe,
    instrument_class_map: &HashMap<String, Uuid>,
    market_map: &HashMap<String, Uuid>,
    _force: bool,
) -> Result<i32> {
    let mut created = 0;

    for market_cfg in &universe.allowed_markets {
        let Some(&market_id) = market_map.get(&market_cfg.mic) else {
            tracing::warn!(mic = %market_cfg.mic, "Market not found in reference data, skipping");
            continue;
        };

        for inst_cfg in &universe.instrument_classes {
            let Some(&class_id) = instrument_class_map.get(&inst_cfg.class_code) else {
                tracing::warn!(code = %inst_cfg.class_code, "Instrument class not found, skipping");
                continue;
            };

            // Build currencies array
            let currencies: Vec<String> = if market_cfg.currencies.is_empty() {
                universe.allowed_currencies.clone()
            } else {
                market_cfg.currencies.clone()
            };

            let settlement_types: Vec<String> = if market_cfg.settlement_types.is_empty() {
                vec!["DVP".to_string()]
            } else {
                market_cfg.settlement_types.clone()
            };

            // Uses partial unique index: cbu_instrument_universe_no_counterparty_key
            // Column list + WHERE clause for partial index inference
            tracing::debug!(mic = %market_cfg.mic, class = %inst_cfg.class_code, "materialize_universe: inserting");
            let result = sqlx::query(
                r#"INSERT INTO custody.cbu_instrument_universe
                   (cbu_id, instrument_class_id, market_id, currencies, settlement_types,
                    is_held, is_traded, effective_date)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, CURRENT_DATE)
                   ON CONFLICT (cbu_id, instrument_class_id, market_id) WHERE counterparty_entity_id IS NULL
                   DO NOTHING"#,
            )
            .bind(cbu_id)
            .bind(class_id)
            .bind(market_id)
            .bind(&currencies)
            .bind(&settlement_types)
            .bind(inst_cfg.is_held)
            .bind(inst_cfg.is_traded)
            .execute(&mut **tx)
            .await
            .map_err(|e| {
                tracing::error!(mic = %market_cfg.mic, class = %inst_cfg.class_code, error = %e, "materialize_universe: insert failed");
                e
            })?;

            if result.rows_affected() > 0 {
                created += 1;
            }
        }
    }

    Ok(created)
}

#[cfg(feature = "database")]
async fn materialize_booking_rules(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    cbu_id: Uuid,
    rules: &[BookingRule],
    ssi_name_to_id: &HashMap<String, Uuid>,
    instrument_class_map: &HashMap<String, Uuid>,
    market_map: &HashMap<String, Uuid>,
    _force: bool,
) -> Result<i32> {
    let mut created = 0;

    for rule in rules {
        // Look up SSI ID from name
        let Some(&ssi_id) = ssi_name_to_id.get(&rule.ssi_ref) else {
            tracing::warn!(ssi_ref = %rule.ssi_ref, rule = %rule.name, "SSI not found for booking rule, skipping");
            continue;
        };

        // Look up instrument_class_id if specified
        let instrument_class_id = rule
            .match_criteria
            .instrument_class
            .as_ref()
            .and_then(|c| instrument_class_map.get(c))
            .copied();

        // Look up market_id if mic specified
        let market_id = rule
            .match_criteria
            .mic
            .as_ref()
            .and_then(|m| market_map.get(m))
            .copied();

        // NOTE: We do NOT insert specificity_score - it's GENERATED ALWAYS
        // Constraint is (cbu_id, priority, rule_name) - unique rule names within a priority tier
        tracing::debug!(rule_name = %rule.name, priority = rule.priority, "materialize_booking_rules: inserting");
        let result = sqlx::query(
            r#"INSERT INTO custody.ssi_booking_rules
               (cbu_id, ssi_id, rule_name, priority,
                instrument_class_id, market_id, currency, settlement_type,
                effective_date)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, CURRENT_DATE)
               ON CONFLICT (cbu_id, priority, rule_name) DO NOTHING"#,
        )
        .bind(cbu_id)
        .bind(ssi_id)
        .bind(&rule.name)
        .bind(rule.priority)
        .bind(instrument_class_id)
        .bind(market_id)
        .bind(&rule.match_criteria.currency)
        .bind(&rule.match_criteria.settlement_type)
        .execute(&mut **tx)
        .await
        .map_err(|e| {
            tracing::error!(rule_name = %rule.name, error = %e, "materialize_booking_rules: insert failed");
            e
        })?;

        if result.rows_affected() > 0 {
            created += 1;
        }
    }

    Ok(created)
}

// =============================================================================
// ISDA/CSA MATERIALIZATION
// =============================================================================

#[cfg(feature = "database")]
async fn materialize_isda_agreements(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    pool: &PgPool,
    cbu_id: Uuid,
    isda_agreements: &[IsdaAgreementConfig],
    ssi_name_to_id: &HashMap<String, Uuid>,
) -> Result<i32> {
    let mut created = 0;

    for isda in isda_agreements {
        // Resolve counterparty EntityRef → entity_id
        let counterparty_entity_id = resolve_entity_ref(pool, &isda.counterparty)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to resolve ISDA counterparty: {}", e))?;

        // Parse dates
        let agreement_date = chrono::NaiveDate::parse_from_str(&isda.agreement_date, "%Y-%m-%d")
            .map_err(|e| {
                anyhow::anyhow!("Invalid agreement_date '{}': {}", isda.agreement_date, e)
            })?;

        let effective_date = isda
            .effective_date
            .as_ref()
            .map(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d"))
            .transpose()
            .map_err(|e| anyhow::anyhow!("Invalid effective_date: {}", e))?
            .unwrap_or(agreement_date);

        // Insert ISDA agreement
        let isda_id = Uuid::new_v4();
        tracing::debug!(
            counterparty = %counterparty_entity_id,
            agreement_date = %agreement_date,
            "materialize_isda_agreements: inserting ISDA"
        );

        let result = sqlx::query(
            r#"INSERT INTO custody.isda_agreements
               (isda_id, cbu_id, counterparty_entity_id, agreement_date, governing_law, effective_date)
               VALUES ($1, $2, $3, $4, $5, $6)
               ON CONFLICT (cbu_id, counterparty_entity_id, agreement_date) DO UPDATE SET
                   governing_law = EXCLUDED.governing_law,
                   updated_at = NOW()
               RETURNING isda_id"#,
        )
        .bind(isda_id)
        .bind(cbu_id)
        .bind(counterparty_entity_id)
        .bind(agreement_date)
        .bind(&isda.governing_law)
        .bind(effective_date)
        .fetch_one(&mut **tx)
        .await?;

        let actual_isda_id: Uuid = result.get("isda_id");
        created += 1;

        // Insert product coverage
        for coverage in &isda.product_coverage {
            // Look up instrument_class_id by asset_class code
            let class_id: Option<Uuid> = sqlx::query_scalar(
                r#"SELECT class_id FROM custody.instrument_classes WHERE code = $1"#,
            )
            .bind(&coverage.asset_class)
            .fetch_optional(&mut **tx)
            .await?;

            if let Some(class_id) = class_id {
                sqlx::query(
                    r#"INSERT INTO custody.isda_product_coverage
                       (isda_id, instrument_class_id)
                       VALUES ($1, $2)
                       ON CONFLICT (isda_id, instrument_class_id) DO NOTHING"#,
                )
                .bind(actual_isda_id)
                .bind(class_id)
                .execute(&mut **tx)
                .await?;
            } else {
                tracing::warn!(
                    asset_class = %coverage.asset_class,
                    "ISDA product coverage: instrument class not found, skipping"
                );
            }
        }

        // Insert CSA if present
        if let Some(ref csa) = isda.csa {
            // Resolve collateral_ssi_id from reference
            let collateral_ssi_id = if let Some(ref ssi_ref) = csa.collateral_ssi_ref {
                // Look up SSI by name from our map
                let ssi_id = ssi_name_to_id.get(ssi_ref).copied();
                if ssi_id.is_none() {
                    tracing::warn!(
                        ssi_ref = %ssi_ref,
                        "CSA collateral_ssi_ref not found in standing_instructions, skipping SSI link"
                    );
                }
                ssi_id
            } else if let Some(ref inline_ssi) = csa.collateral_ssi {
                // Deprecated: inline SSI definition - look up by name
                ssi_name_to_id.get(&inline_ssi.name).copied()
            } else {
                None
            };

            let csa_id = Uuid::new_v4();
            tracing::debug!(
                csa_type = %csa.csa_type,
                collateral_ssi_id = ?collateral_ssi_id,
                "materialize_isda_agreements: inserting CSA"
            );

            sqlx::query(
                r#"INSERT INTO custody.csa_agreements
                   (csa_id, isda_id, csa_type, threshold_amount, threshold_currency,
                    minimum_transfer_amount, rounding_amount, collateral_ssi_id, effective_date)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                   ON CONFLICT (csa_id) DO UPDATE SET
                       csa_type = EXCLUDED.csa_type,
                       threshold_amount = EXCLUDED.threshold_amount,
                       threshold_currency = EXCLUDED.threshold_currency,
                       minimum_transfer_amount = EXCLUDED.minimum_transfer_amount,
                       rounding_amount = EXCLUDED.rounding_amount,
                       collateral_ssi_id = EXCLUDED.collateral_ssi_id,
                       updated_at = NOW()"#,
            )
            .bind(csa_id)
            .bind(actual_isda_id)
            .bind(&csa.csa_type)
            .bind(csa.threshold_amount)
            .bind(&csa.threshold_currency)
            .bind(csa.minimum_transfer_amount)
            .bind(csa.rounding_amount)
            .bind(collateral_ssi_id)
            .bind(effective_date)
            .execute(&mut **tx)
            .await?;
        }
    }

    Ok(created)
}

// =============================================================================
// VALIDATE OPERATION
// =============================================================================

/// Validate a trading profile document without importing
pub struct TradingProfileValidateOp;

#[async_trait]
impl CustomOperation for TradingProfileValidateOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }
    fn verb(&self) -> &'static str {
        "validate"
    }
    fn rationale(&self) -> &'static str {
        "Validates document structure and references without database writes"
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
            .and_then(|a| a.value.as_string());

        // For now, only file-based validation is supported
        let file_path = file_path.ok_or_else(|| {
            anyhow::anyhow!("Missing :file-path argument. File-based validation is required.")
        })?;

        let mut errors: Vec<String> = vec![];
        let mut warnings: Vec<String> = vec![];

        // Parse the document
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| anyhow::anyhow!("Failed to read file {}: {}", file_path, e))?;

        let document: TradingProfileDocument =
            match serde_yaml::from_str::<TradingProfileImport>(&content) {
                Ok(import) => import.into_document(),
                Err(e) => {
                    return Ok(ExecutionResult::Record(json!({
                        "valid": false,
                        "errors": [format!("Parse error: {}", e)],
                        "warnings": []
                    })));
                }
            };

        // Validate markets exist
        let market_codes: Vec<String> = document
            .universe
            .allowed_markets
            .iter()
            .map(|m| m.mic.clone())
            .collect();

        let known_markets: Vec<String> =
            sqlx::query_scalar(r#"SELECT mic FROM custody.markets WHERE mic = ANY($1)"#)
                .bind(&market_codes)
                .fetch_all(pool)
                .await?;

        for market in &market_codes {
            if !known_markets.contains(market) {
                warnings.push(format!("Unknown market MIC: {}", market));
            }
        }

        // Validate instrument classes exist
        let class_codes: Vec<String> = document
            .universe
            .instrument_classes
            .iter()
            .map(|c| c.class_code.clone())
            .collect();

        let known_classes: Vec<String> = sqlx::query_scalar(
            r#"SELECT code FROM custody.instrument_classes WHERE code = ANY($1)"#,
        )
        .bind(&class_codes)
        .fetch_all(pool)
        .await?;

        for class in &class_codes {
            if !known_classes.contains(class) {
                warnings.push(format!("Unknown instrument class: {}", class));
            }
        }

        // Validate booking rules reference defined SSIs
        let ssi_names: Vec<String> = document
            .standing_instructions
            .values()
            .flatten()
            .map(|s| s.name.clone())
            .collect();

        for rule in &document.booking_rules {
            if !ssi_names.contains(&rule.ssi_ref) {
                errors.push(format!(
                    "Booking rule '{}' references undefined SSI '{}'",
                    rule.name, rule.ssi_ref
                ));
            }
        }

        let valid = errors.is_empty();

        Ok(ExecutionResult::Record(json!({
            "valid": valid,
            "errors": errors,
            "warnings": warnings,
            "stats": {
                "markets": market_codes.len(),
                "instrument_classes": class_codes.len(),
                "ssis": ssi_names.len(),
                "booking_rules": document.booking_rules.len()
            }
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(json!({"valid": true})))
    }
}
