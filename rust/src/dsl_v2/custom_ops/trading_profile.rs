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
    ast_db, document_ops, resolve::resolve_entity_ref, BookingRule, IsdaAgreementConfig,
    MaterializationResult, StandingInstruction, TradingProfileDocument, TradingProfileImport,
};
use ob_poc_types::trading_matrix::{
    categories, BookingMatchCriteria, TradingMatrixNodeId, TradingMatrixOp,
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

        // Build reference maps for materialization
        let mut refs = ReferenceMaps::new();
        refs.instrument_class_map = build_instrument_class_map(&mut tx).await?;
        refs.market_map = build_market_map(&mut tx).await?;

        let opts = MaterializationOptions { force };

        // Materialize SSIs first (booking rules reference them)
        if sections.contains(&"ssis".to_string()) {
            // Collect all SSI names from the incoming matrix
            let incoming_ssi_names: std::collections::HashSet<String> = document
                .standing_instructions
                .values()
                .flat_map(|ssis| ssis.iter().map(|s| s.name.clone()))
                .collect();

            // Delete orphaned SSIs (exist in DB but not in matrix)
            let existing_ssis: Vec<(Uuid, String)> = sqlx::query_as(
                r#"SELECT ssi_id, ssi_name FROM custody.cbu_ssi
                   WHERE cbu_id = $1 AND source = 'TRADING_PROFILE'"#,
            )
            .bind(cbu_id)
            .fetch_all(&mut *tx)
            .await?;

            let mut deleted = 0;
            for (ssi_id, ssi_name) in existing_ssis {
                if !incoming_ssi_names.contains(&ssi_name) {
                    tracing::info!(
                        ssi_id = %ssi_id,
                        ssi_name = %ssi_name,
                        "materialize: deleting orphaned SSI"
                    );
                    // Delete booking rules referencing this SSI first
                    sqlx::query("DELETE FROM custody.ssi_booking_rules WHERE ssi_id = $1")
                        .bind(ssi_id)
                        .execute(&mut *tx)
                        .await?;
                    // Delete the SSI
                    sqlx::query("DELETE FROM custody.cbu_ssi WHERE ssi_id = $1")
                        .bind(ssi_id)
                        .execute(&mut *tx)
                        .await?;
                    deleted += 1;
                }
            }
            if deleted > 0 {
                result
                    .records_deleted
                    .insert("cbu_ssi".to_string(), deleted);
            }

            // Now upsert SSIs from the matrix
            let mut created = 0;
            for (category, ssis) in &document.standing_instructions {
                for ssi in ssis {
                    let ssi_id =
                        materialize_ssi(&mut tx, cbu_id, category, ssi, &refs, &opts).await?;
                    refs.ssi_name_to_id.insert(ssi.name.clone(), ssi_id);
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
                refs.ssi_name_to_id.insert(row.ssi_name, row.ssi_id);
            }
        }

        // Materialize universe
        if sections.contains(&"universe".to_string()) {
            let created =
                materialize_universe(&mut tx, cbu_id, &document.universe, &refs, &opts).await?;
            result
                .records_created
                .insert("cbu_instrument_universe".to_string(), created);
            result.sections_materialized.push("universe".to_string());
        }

        // Materialize booking rules
        if sections.contains(&"booking_rules".to_string()) {
            let created =
                materialize_booking_rules(&mut tx, cbu_id, &document.booking_rules, &refs, &opts)
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
                &refs.ssi_name_to_id,
            )
            .await?;
            result
                .records_created
                .insert("isda_agreements".to_string(), created);
            result.sections_materialized.push("isda".to_string());
        }

        // Materialize Corporate Actions preferences
        if sections.contains(&"corporate_actions".to_string()) {
            if let Some(ref ca) = document.corporate_actions {
                let created =
                    materialize_corporate_actions(&mut tx, cbu_id, ca, &refs.ssi_name_to_id)
                        .await?;
                result
                    .records_created
                    .insert("cbu_ca_preferences".to_string(), created);
                result
                    .sections_materialized
                    .push("corporate_actions".to_string());
            }
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
// MATERIALIZATION HELPERS
// =============================================================================

/// Reference maps for materializing trading profile elements
///
/// These maps resolve symbolic names (SSI name, instrument class code, MIC)
/// to database UUIDs during materialization.
#[cfg(feature = "database")]
struct ReferenceMaps {
    /// SSI name -> ssi_id
    ssi_name_to_id: HashMap<String, Uuid>,
    /// Instrument class code -> class_id
    instrument_class_map: HashMap<String, Uuid>,
    /// Market MIC -> market_id
    market_map: HashMap<String, Uuid>,
}

#[cfg(feature = "database")]
impl ReferenceMaps {
    fn new() -> Self {
        Self {
            ssi_name_to_id: HashMap::new(),
            instrument_class_map: HashMap::new(),
            market_map: HashMap::new(),
        }
    }
}

/// Options controlling materialization behavior
#[cfg(feature = "database")]
#[derive(Debug, Clone, Default)]
struct MaterializationOptions {
    /// Force overwrite existing records (vs skip on conflict)
    force: bool,
}

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
    refs: &ReferenceMaps,
    opts: &MaterializationOptions,
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
    let market_id = ssi
        .mic
        .as_ref()
        .and_then(|m| refs.market_map.get(m))
        .copied();

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
        if opts.force {
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
    refs: &ReferenceMaps,
    _opts: &MaterializationOptions,
) -> Result<i32> {
    let mut created = 0;

    for market_cfg in &universe.allowed_markets {
        let Some(&market_id) = refs.market_map.get(&market_cfg.mic) else {
            tracing::warn!(mic = %market_cfg.mic, "Market not found in reference data, skipping");
            continue;
        };

        for inst_cfg in &universe.instrument_classes {
            let Some(&class_id) = refs.instrument_class_map.get(&inst_cfg.class_code) else {
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

            // Uses natural key unique constraint: (cbu_id, instrument_class_id, market_id, counterparty_key)
            // counterparty_key defaults to nil UUID for non-OTC entries
            tracing::debug!(mic = %market_cfg.mic, class = %inst_cfg.class_code, "materialize_universe: inserting");
            let nil_uuid = Uuid::nil();
            let result = sqlx::query(
                r#"INSERT INTO custody.cbu_instrument_universe
                   (cbu_id, instrument_class_id, market_id, currencies, settlement_types,
                    is_held, is_traded, effective_date, counterparty_key)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, CURRENT_DATE, $8)
                   ON CONFLICT (cbu_id, instrument_class_id, market_id, counterparty_key)
                   DO UPDATE SET
                       currencies = EXCLUDED.currencies,
                       settlement_types = EXCLUDED.settlement_types,
                       is_held = EXCLUDED.is_held,
                       is_traded = EXCLUDED.is_traded"#,
            )
            .bind(cbu_id)
            .bind(class_id)
            .bind(market_id)
            .bind(&currencies)
            .bind(&settlement_types)
            .bind(inst_cfg.is_held)
            .bind(inst_cfg.is_traded)
            .bind(nil_uuid)
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
    refs: &ReferenceMaps,
    _opts: &MaterializationOptions,
) -> Result<i32> {
    let mut created = 0;

    for rule in rules {
        // Look up SSI ID from name
        let Some(&ssi_id) = refs.ssi_name_to_id.get(&rule.ssi_ref) else {
            tracing::warn!(ssi_ref = %rule.ssi_ref, rule = %rule.name, "SSI not found for booking rule, skipping");
            continue;
        };

        // Look up instrument_class_id if specified
        let instrument_class_id = rule
            .match_criteria
            .instrument_class
            .as_ref()
            .and_then(|c| refs.instrument_class_map.get(c))
            .copied();

        // Look up market_id if mic specified
        let market_id = rule
            .match_criteria
            .mic
            .as_ref()
            .and_then(|m| refs.market_map.get(m))
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

    // =========================================================================
    // ORPHAN CLEANUP: Delete ISDAs that are no longer in the matrix
    // =========================================================================

    // Build set of (counterparty_entity_id, agreement_date) keys from incoming matrix
    let mut incoming_keys: Vec<(Uuid, chrono::NaiveDate)> = Vec::new();
    for isda in isda_agreements {
        if let Ok(counterparty_id) = resolve_entity_ref(pool, &isda.counterparty).await {
            if let Ok(date) = chrono::NaiveDate::parse_from_str(&isda.agreement_date, "%Y-%m-%d") {
                incoming_keys.push((counterparty_id, date));
            }
        }
    }

    // Get existing ISDA IDs for this CBU
    let existing: Vec<(Uuid, Uuid, chrono::NaiveDate)> = sqlx::query_as(
        r#"SELECT isda_id, counterparty_entity_id, agreement_date
           FROM custody.isda_agreements WHERE cbu_id = $1"#,
    )
    .bind(cbu_id)
    .fetch_all(&mut **tx)
    .await?;

    // Find orphans (exist in DB but not in incoming matrix)
    for (isda_id, counterparty_id, agreement_date) in existing {
        let key = (counterparty_id, agreement_date);
        if !incoming_keys.contains(&key) {
            tracing::info!(
                isda_id = %isda_id,
                counterparty = %counterparty_id,
                "materialize_isda_agreements: deleting orphaned ISDA"
            );

            // Delete CSAs first (FK constraint)
            sqlx::query("DELETE FROM custody.csa_agreements WHERE isda_id = $1")
                .bind(isda_id)
                .execute(&mut **tx)
                .await?;

            // Delete product coverage (FK constraint)
            sqlx::query("DELETE FROM custody.isda_product_coverage WHERE isda_id = $1")
                .bind(isda_id)
                .execute(&mut **tx)
                .await?;

            // Delete ISDA
            sqlx::query("DELETE FROM custody.isda_agreements WHERE isda_id = $1")
                .bind(isda_id)
                .execute(&mut **tx)
                .await?;
        }
    }

    // =========================================================================
    // UPSERT: Insert or update ISDAs from the matrix
    // =========================================================================

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
                   ON CONFLICT (isda_id, csa_type) DO UPDATE SET
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
// CORPORATE ACTIONS MATERIALIZATION
// =============================================================================

#[cfg(feature = "database")]
async fn materialize_corporate_actions(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    cbu_id: Uuid,
    ca: &ob_poc_types::trading_matrix::TradingMatrixCorporateActions,
    ssi_name_to_id: &HashMap<String, Uuid>,
) -> Result<i32> {
    let mut created = 0;

    // Look up event_type_ids from enabled event codes
    let mut event_code_to_id: HashMap<String, Uuid> = HashMap::new();
    for event_code in &ca.enabled_event_types {
        let row: Option<(Uuid,)> = sqlx::query_as(
            r#"SELECT event_type_id FROM custody.ca_event_types WHERE event_code = $1"#,
        )
        .bind(event_code)
        .fetch_optional(&mut **tx)
        .await?;

        if let Some((event_type_id,)) = row {
            event_code_to_id.insert(event_code.clone(), event_type_id);
        } else {
            tracing::warn!(event_code = %event_code, "CA event type not found in reference catalog, skipping");
        }
    }

    // Determine processing mode from election policy
    let processing_mode = match ca.election_policy.as_ref().map(|e| &e.elector) {
        Some(ob_poc_types::trading_matrix::CaElector::InvestmentManager) => "AUTO_INSTRUCT",
        Some(ob_poc_types::trading_matrix::CaElector::Admin) => "MANUAL",
        Some(ob_poc_types::trading_matrix::CaElector::Client) => "MANUAL",
        None => "DEFAULT_ONLY",
    };

    // Materialize preferences for each enabled event type
    for (event_code, event_type_id) in &event_code_to_id {
        // Find default option if specified
        let default_election = ca
            .default_options
            .iter()
            .find(|o| &o.event_type == event_code)
            .map(|o| o.default_option.clone());

        tracing::debug!(
            event_code = %event_code,
            processing_mode = %processing_mode,
            "materialize_corporate_actions: inserting preference"
        );

        let result = sqlx::query(
            r#"INSERT INTO custody.cbu_ca_preferences
               (cbu_id, event_type_id, processing_mode, default_election, created_at)
               VALUES ($1, $2, $3, $4, NOW())
               ON CONFLICT (cbu_id, event_type_id, instrument_class_id)
               DO UPDATE SET
                   processing_mode = EXCLUDED.processing_mode,
                   default_election = EXCLUDED.default_election,
                   updated_at = NOW()"#,
        )
        .bind(cbu_id)
        .bind(event_type_id)
        .bind(processing_mode)
        .bind(&default_election)
        .execute(&mut **tx)
        .await?;

        if result.rows_affected() > 0 {
            created += 1;
        }
    }

    // Materialize instruction windows (cutoff rules)
    for rule in &ca.cutoff_rules {
        // Find event_type_id if event-specific
        let event_type_id = rule
            .event_type
            .as_ref()
            .and_then(|et| event_code_to_id.get(et))
            .copied();

        // Look up market_id if market-specific
        let market_id: Option<Uuid> = if let Some(ref mic) = rule.market_code {
            sqlx::query_scalar(r#"SELECT market_id FROM custody.markets WHERE mic = $1"#)
                .bind(mic)
                .fetch_optional(&mut **tx)
                .await?
        } else {
            None
        };

        tracing::debug!(
            market_code = ?rule.market_code,
            days_before = rule.days_before,
            "materialize_corporate_actions: inserting instruction window"
        );

        sqlx::query(
            r#"INSERT INTO custody.cbu_ca_instruction_windows
               (cbu_id, event_type_id, market_id, cutoff_days_before, warning_days, escalation_days, created_at)
               VALUES ($1, $2, $3, $4, $5, $6, NOW())
               ON CONFLICT (cbu_id, event_type_id, market_id)
               DO UPDATE SET
                   cutoff_days_before = EXCLUDED.cutoff_days_before,
                   warning_days = EXCLUDED.warning_days,
                   escalation_days = EXCLUDED.escalation_days,
                   updated_at = NOW()"#,
        )
        .bind(cbu_id)
        .bind(event_type_id)
        .bind(market_id)
        .bind(rule.days_before)
        .bind(rule.warning_days)
        .bind(rule.escalation_days)
        .execute(&mut **tx)
        .await?;
    }

    // Materialize SSI mappings for CA proceeds
    for mapping in &ca.proceeds_ssi_mappings {
        // Look up SSI ID from name
        let Some(&ssi_id) = ssi_name_to_id.get(&mapping.ssi_reference) else {
            tracing::warn!(
                ssi_ref = %mapping.ssi_reference,
                "CA proceeds SSI not found in standing_instructions, skipping"
            );
            continue;
        };

        let proceeds_type = match mapping.proceeds_type {
            ob_poc_types::trading_matrix::CaProceedsType::Cash => "CASH",
            ob_poc_types::trading_matrix::CaProceedsType::Stock => "STOCK",
        };

        let currency = mapping.currency.as_deref().unwrap_or("*");

        tracing::debug!(
            proceeds_type = %proceeds_type,
            currency = %currency,
            ssi_ref = %mapping.ssi_reference,
            "materialize_corporate_actions: inserting SSI mapping"
        );

        // Note: event_type_id is NULL for global mappings
        sqlx::query(
            r#"INSERT INTO custody.cbu_ca_ssi_mappings
               (cbu_id, currency, proceeds_type, ssi_id, created_at)
               VALUES ($1, $2, $3, $4, NOW())
               ON CONFLICT (cbu_id, event_type_id, currency, proceeds_type)
               DO UPDATE SET
                   ssi_id = EXCLUDED.ssi_id,
                   updated_at = NOW()"#,
        )
        .bind(cbu_id)
        .bind(currency)
        .bind(proceeds_type)
        .bind(ssi_id)
        .execute(&mut **tx)
        .await?;
    }

    Ok(created)
}

// =============================================================================
// DOCUMENT CONSTRUCTION OPERATIONS
// =============================================================================

/// Create a new draft trading profile for a CBU
pub struct TradingProfileCreateDraftOp;

#[async_trait]
impl CustomOperation for TradingProfileCreateDraftOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }
    fn verb(&self) -> &'static str {
        "create-draft"
    }
    fn rationale(&self) -> &'static str {
        "Creates new DRAFT profile document with optional cloning from existing"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::trading_profile::ast_db;

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

        let notes = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "notes")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let (profile_id, _doc) = ast_db::create_draft(pool, cbu_id, notes)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create draft: {}", e))?;

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

/// Add instrument class to trading profile universe
pub struct TradingProfileAddInstrumentClassOp;

#[async_trait]
impl CustomOperation for TradingProfileAddInstrumentClassOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }
    fn verb(&self) -> &'static str {
        "add-instrument-class"
    }
    fn rationale(&self) -> &'static str {
        "Modifies JSONB document to add instrument class"
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

        let class_code = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "class-code")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing class-code argument"))?;

        let cfi_prefix = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "cfi-prefixes")
            .and_then(|a| {
                a.value.as_list().and_then(|list| {
                    list.first()
                        .and_then(|node| node.as_string().map(|s| s.to_string()))
                })
            });

        // Determine if OTC based on class code (IRS, FX, etc. are OTC)
        let is_otc = matches!(
            class_code.as_str(),
            "OTC_IRS" | "OTC_FX" | "OTC_CREDIT" | "OTC_EQUITY" | "OTC_COMMODITY"
        ) || class_code.starts_with("OTC_");

        // Apply operation to AST and save
        let doc = ast_db::apply_and_save(
            pool,
            profile_id,
            TradingMatrixOp::AddInstrumentClass {
                class_code: class_code.clone(),
                cfi_prefix,
                is_otc,
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to add instrument class: {}", e))?;

        Ok(ExecutionResult::Record(json!({
            "profile_id": profile_id,
            "class_code": class_code,
            "version": doc.version,
            "status": format!("{:?}", doc.status),
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

/// Remove instrument class from trading profile universe
pub struct TradingProfileRemoveInstrumentClassOp;

#[async_trait]
impl CustomOperation for TradingProfileRemoveInstrumentClassOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }
    fn verb(&self) -> &'static str {
        "remove-instrument-class"
    }
    fn rationale(&self) -> &'static str {
        "Modifies JSONB document to remove instrument class"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::trading_profile::ast_db;
        use ob_poc_types::trading_matrix::{categories, TradingMatrixNodeId, TradingMatrixOp};

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

        let class_code = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "class-code")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing class-code argument"))?;

        // Build node ID: _Trading Universe / {class_code}
        let node_id = TradingMatrixNodeId::category(categories::UNIVERSE).child(&class_code);

        let doc = ast_db::apply_and_save(pool, profile_id, TradingMatrixOp::RemoveNode { node_id })
            .await
            .map_err(|e| anyhow::anyhow!("Failed to remove instrument class: {}", e))?;

        Ok(ExecutionResult::Record(json!({
            "profile_id": profile_id,
            "removed": class_code,
            "version": doc.version,
            "status": format!("{:?}", doc.status),
        })))
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

/// Add market to trading profile universe under an instrument class
pub struct TradingProfileAddMarketOp;

#[async_trait]
impl CustomOperation for TradingProfileAddMarketOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }
    fn verb(&self) -> &'static str {
        "add-market"
    }
    fn rationale(&self) -> &'static str {
        "Modifies JSONB document to add market under instrument class"
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

        let instrument_class = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "instrument-class")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing instrument-class argument"))?;

        let mic = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "mic")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing mic argument"))?;

        // Get market name from argument or look up from reference data
        let market_name = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "market-name")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let country_code = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "country-code")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        // Look up market metadata from reference data if not provided
        let (resolved_name, resolved_country) =
            if let (Some(name), Some(code)) = (market_name.clone(), country_code.clone()) {
                (name, code)
            } else {
                let row =
                    sqlx::query(r#"SELECT name, country_code FROM custody.markets WHERE mic = $1"#)
                        .bind(&mic)
                        .fetch_optional(pool)
                        .await?;

                match row {
                    Some(r) => (
                        market_name.unwrap_or_else(|| r.get::<String, _>("name")),
                        country_code.unwrap_or_else(|| r.get::<String, _>("country_code")),
                    ),
                    None => (
                        market_name.unwrap_or_else(|| mic.clone()),
                        country_code.unwrap_or_else(|| "XX".to_string()),
                    ),
                }
            };

        // Apply operation to AST and save
        let doc = ast_db::apply_and_save(
            pool,
            profile_id,
            TradingMatrixOp::AddMarket {
                parent_class: instrument_class.clone(),
                mic: mic.clone(),
                market_name: resolved_name,
                country_code: resolved_country,
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to add market: {}", e))?;

        Ok(ExecutionResult::Record(json!({
            "profile_id": profile_id,
            "instrument_class": instrument_class,
            "mic": mic,
            "version": doc.version,
            "status": format!("{:?}", doc.status),
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

/// Remove market from trading profile universe
pub struct TradingProfileRemoveMarketOp;

#[async_trait]
impl CustomOperation for TradingProfileRemoveMarketOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }
    fn verb(&self) -> &'static str {
        "remove-market"
    }
    fn rationale(&self) -> &'static str {
        "Modifies JSONB document to remove market"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::trading_profile::ast_db;
        use ob_poc_types::trading_matrix::{categories, TradingMatrixNodeId, TradingMatrixOp};

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

        let instrument_class = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "instrument-class")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing instrument-class argument"))?;

        let mic = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "mic")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing mic argument"))?;

        // Build node ID: _Trading Universe / {instrument_class} / {mic}
        let node_id = TradingMatrixNodeId::category(categories::UNIVERSE)
            .child(&instrument_class)
            .child(&mic);

        let doc = ast_db::apply_and_save(pool, profile_id, TradingMatrixOp::RemoveNode { node_id })
            .await
            .map_err(|e| anyhow::anyhow!("Failed to remove market: {}", e))?;

        Ok(ExecutionResult::Record(json!({
            "profile_id": profile_id,
            "instrument_class": instrument_class,
            "removed": mic,
            "version": doc.version,
            "status": format!("{:?}", doc.status),
        })))
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

/// Add standing instruction to trading profile
pub struct TradingProfileAddSsiOp;

#[async_trait]
impl CustomOperation for TradingProfileAddSsiOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }
    fn verb(&self) -> &'static str {
        "add-standing-instruction"
    }
    fn rationale(&self) -> &'static str {
        "Modifies JSONB document to add SSI"
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

        let ssi_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "ssi-type")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing ssi-type argument"))?;

        let ssi_name = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "ssi-name")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing ssi-name argument"))?;

        // Generate ssi_id from type and name
        let ssi_id = format!("{}:{}", ssi_type, ssi_name);

        let safekeeping_account = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "safekeeping-account")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let safekeeping_bic = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "safekeeping-bic")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let cash_account = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "cash-account")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let cash_bic = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "cash-bic")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let cash_currency = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "cash-currency")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let pset_bic = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "pset-bic")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let doc = ast_db::apply_and_save(
            pool,
            profile_id,
            TradingMatrixOp::AddSsi {
                ssi_id: ssi_id.clone(),
                ssi_name: ssi_name.clone(),
                ssi_type,
                safekeeping_account,
                safekeeping_bic,
                cash_account,
                cash_bic,
                cash_currency,
                pset_bic,
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to add SSI: {}", e))?;

        Ok(ExecutionResult::Record(json!({
            "profile_id": profile_id,
            "ssi_id": ssi_id,
            "ssi_name": ssi_name,
            "version": doc.version,
            "status": format!("{:?}", doc.status),
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

/// Remove standing instruction from trading profile
pub struct TradingProfileRemoveSsiOp;

#[async_trait]
impl CustomOperation for TradingProfileRemoveSsiOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }
    fn verb(&self) -> &'static str {
        "remove-standing-instruction"
    }
    fn rationale(&self) -> &'static str {
        "Modifies JSONB document to remove SSI"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::trading_profile::ast_db;
        use ob_poc_types::trading_matrix::{categories, TradingMatrixNodeId, TradingMatrixOp};

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

        let ssi_name = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "ssi-name")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing ssi-name argument"))?;

        // Build node ID: _Standing Settlement Instructions / {ssi_name}
        let node_id = TradingMatrixNodeId::category(categories::SSI).child(&ssi_name);

        let doc = ast_db::apply_and_save(pool, profile_id, TradingMatrixOp::RemoveNode { node_id })
            .await
            .map_err(|e| anyhow::anyhow!("Failed to remove SSI: {}", e))?;

        Ok(ExecutionResult::Record(json!({
            "profile_id": profile_id,
            "removed": ssi_name,
            "version": doc.version,
            "status": format!("{:?}", doc.status),
        })))
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

/// Add booking rule to trading profile
pub struct TradingProfileAddBookingRuleOp;

#[async_trait]
impl CustomOperation for TradingProfileAddBookingRuleOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }
    fn verb(&self) -> &'static str {
        "add-booking-rule"
    }
    fn rationale(&self) -> &'static str {
        "Modifies JSONB document to add booking rule"
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

        let rule_name = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "rule-name")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing rule-name argument"))?;

        let priority = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "priority")
            .and_then(|a| a.value.as_integer())
            .ok_or_else(|| anyhow::anyhow!("Missing priority argument"))?
            as i32;

        let ssi_ref = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "ssi-ref")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing ssi-ref argument"))?;

        // Generate rule_id from ssi_ref and rule_name
        let rule_id = format!("{}:{}", ssi_ref, rule_name);

        // Build match criteria
        let match_criteria = BookingMatchCriteria {
            instrument_class: verb_call
                .arguments
                .iter()
                .find(|a| a.key == "match-instrument-class")
                .and_then(|a| a.value.as_string())
                .map(|s| s.to_string()),
            security_type: verb_call
                .arguments
                .iter()
                .find(|a| a.key == "match-security-type")
                .and_then(|a| a.value.as_string())
                .map(|s| s.to_string()),
            mic: verb_call
                .arguments
                .iter()
                .find(|a| a.key == "match-mic")
                .and_then(|a| a.value.as_string())
                .map(|s| s.to_string()),
            currency: verb_call
                .arguments
                .iter()
                .find(|a| a.key == "match-currency")
                .and_then(|a| a.value.as_string())
                .map(|s| s.to_string()),
            settlement_type: verb_call
                .arguments
                .iter()
                .find(|a| a.key == "match-settlement-type")
                .and_then(|a| a.value.as_string())
                .map(|s| s.to_string()),
            counterparty_entity_id: verb_call
                .arguments
                .iter()
                .find(|a| a.key == "match-counterparty-id")
                .and_then(|a| a.value.as_string())
                .map(|s| s.to_string()),
        };

        let doc = ast_db::apply_and_save(
            pool,
            profile_id,
            TradingMatrixOp::AddBookingRule {
                ssi_ref: ssi_ref.clone(),
                rule_id: rule_id.clone(),
                rule_name: rule_name.clone(),
                priority,
                match_criteria,
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to add booking rule: {}", e))?;

        Ok(ExecutionResult::Record(json!({
            "profile_id": profile_id,
            "rule_id": rule_id,
            "rule_name": rule_name,
            "ssi_ref": ssi_ref,
            "priority": priority,
            "version": doc.version,
            "status": format!("{:?}", doc.status),
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

/// Remove booking rule from trading profile
pub struct TradingProfileRemoveBookingRuleOp;

#[async_trait]
impl CustomOperation for TradingProfileRemoveBookingRuleOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }
    fn verb(&self) -> &'static str {
        "remove-booking-rule"
    }
    fn rationale(&self) -> &'static str {
        "Modifies JSONB document to remove booking rule"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::trading_profile::ast_db;
        use ob_poc_types::trading_matrix::{categories, TradingMatrixNodeId, TradingMatrixOp};

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

        let ssi_ref = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "ssi-ref")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing ssi-ref argument"))?;

        let rule_id = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "rule-id")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing rule-id argument"))?;

        // Build node ID: _Standing Settlement Instructions / {ssi_ref} / {rule_id}
        let node_id = TradingMatrixNodeId::category(categories::SSI)
            .child(&ssi_ref)
            .child(&rule_id);

        let doc = ast_db::apply_and_save(pool, profile_id, TradingMatrixOp::RemoveNode { node_id })
            .await
            .map_err(|e| anyhow::anyhow!("Failed to remove booking rule: {}", e))?;

        Ok(ExecutionResult::Record(json!({
            "profile_id": profile_id,
            "ssi_ref": ssi_ref,
            "removed": rule_id,
            "version": doc.version,
            "status": format!("{:?}", doc.status),
        })))
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
// ISDA/CSA CONSTRUCTION OPERATIONS (Phase 2)
// =============================================================================

/// Add ISDA configuration to a trading profile document
pub struct TradingProfileAddIsdaConfigOp;

#[async_trait]
impl CustomOperation for TradingProfileAddIsdaConfigOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }

    fn verb(&self) -> &'static str {
        "add-isda-config"
    }

    fn rationale(&self) -> &'static str {
        "Adds ISDA master agreement configuration to the document"
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

        let counterparty_entity_id = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "counterparty-entity-id")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing counterparty-entity-id argument"))?;

        let counterparty_name = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "counterparty-name")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing counterparty-name argument"))?;

        let counterparty_lei = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "counterparty-lei")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let governing_law = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "governing-law")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let agreement_date = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "agreement-date")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        // Generate isda_id from counterparty name
        let isda_id = counterparty_name.clone();

        let doc = ast_db::apply_and_save(
            pool,
            profile_id,
            TradingMatrixOp::AddIsda {
                isda_id: isda_id.clone(),
                counterparty_entity_id: counterparty_entity_id.clone(),
                counterparty_name: counterparty_name.clone(),
                counterparty_lei,
                governing_law,
                agreement_date,
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to add ISDA config: {}", e))?;

        Ok(ExecutionResult::Record(json!({
            "profile_id": profile_id,
            "isda_id": isda_id,
            "counterparty_name": counterparty_name,
            "counterparty_entity_id": counterparty_entity_id,
            "version": doc.version,
            "status": format!("{:?}", doc.status),
        })))
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

/// Add product coverage to an ISDA agreement
pub struct TradingProfileAddIsdaCoverageOp;

#[async_trait]
impl CustomOperation for TradingProfileAddIsdaCoverageOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }

    fn verb(&self) -> &'static str {
        "add-isda-coverage"
    }

    fn rationale(&self) -> &'static str {
        "Adds product coverage to an ISDA master agreement"
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

        let isda_ref = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "isda-ref")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing isda-ref argument"))?;

        let asset_class = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "asset-class")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing asset-class argument"))?;

        let base_products = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "base-products")
            .and_then(|a| {
                a.value.as_list().map(|list| {
                    list.iter()
                        .filter_map(|node| node.as_string().map(|s| s.to_string()))
                        .collect()
                })
            })
            .unwrap_or_default();

        // Generate coverage_id from isda_ref and asset_class
        let coverage_id = format!("{}:{}", isda_ref, asset_class);

        let doc = ast_db::apply_and_save(
            pool,
            profile_id,
            TradingMatrixOp::AddProductCoverage {
                isda_ref: isda_ref.clone(),
                coverage_id: coverage_id.clone(),
                asset_class: asset_class.clone(),
                base_products,
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to add ISDA coverage: {}", e))?;

        Ok(ExecutionResult::Record(json!({
            "profile_id": profile_id,
            "isda_ref": isda_ref,
            "coverage_id": coverage_id,
            "asset_class": asset_class,
            "version": doc.version,
            "status": format!("{:?}", doc.status),
        })))
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

/// Add CSA configuration to an ISDA agreement
pub struct TradingProfileAddCsaConfigOp;

#[async_trait]
impl CustomOperation for TradingProfileAddCsaConfigOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }

    fn verb(&self) -> &'static str {
        "add-csa-config"
    }

    fn rationale(&self) -> &'static str {
        "Adds Credit Support Annex configuration to an ISDA agreement"
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

        let isda_ref = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "isda-ref")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing isda-ref argument"))?;

        let csa_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "csa-type")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing csa-type argument"))?;

        let threshold_currency = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "threshold-currency")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let threshold_amount = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "threshold-amount")
            .and_then(|a| a.value.as_decimal())
            .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0));

        let minimum_transfer_amount = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "minimum-transfer-amount")
            .and_then(|a| a.value.as_decimal())
            .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0));

        let collateral_ssi_ref = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "collateral-ssi-ref")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        // Generate csa_id from isda_ref and csa_type
        let csa_id = format!("{}:{}", isda_ref, csa_type);

        let doc = ast_db::apply_and_save(
            pool,
            profile_id,
            TradingMatrixOp::AddCsa {
                isda_ref: isda_ref.clone(),
                csa_id: csa_id.clone(),
                csa_type: csa_type.clone(),
                threshold_currency,
                threshold_amount,
                minimum_transfer_amount,
                collateral_ssi_ref,
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to add CSA config: {}", e))?;

        Ok(ExecutionResult::Record(json!({
            "profile_id": profile_id,
            "isda_ref": isda_ref,
            "csa_id": csa_id,
            "csa_type": csa_type,
            "version": doc.version,
            "status": format!("{:?}", doc.status),
        })))
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

/// Add eligible collateral to a CSA
pub struct TradingProfileAddCsaCollateralOp;

#[async_trait]
impl CustomOperation for TradingProfileAddCsaCollateralOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }

    fn verb(&self) -> &'static str {
        "add-csa-collateral"
    }

    fn rationale(&self) -> &'static str {
        "Adds eligible collateral type to a Credit Support Annex"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use rust_decimal::prelude::ToPrimitive;

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

        let counterparty_ref = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "counterparty-ref")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing counterparty-ref argument"))?;

        let collateral_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "collateral-type")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing collateral-type argument"))?;

        let currencies: Option<Vec<String>> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "currencies")
            .and_then(|a| {
                a.value.as_list().map(|list| {
                    list.iter()
                        .filter_map(|node| node.as_string().map(|s| s.to_string()))
                        .collect()
                })
            });

        let _issuers: Option<Vec<String>> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "issuers")
            .and_then(|a| {
                a.value.as_list().map(|list| {
                    list.iter()
                        .filter_map(|node| node.as_string().map(|s| s.to_string()))
                        .collect()
                })
            });

        let _min_rating = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "min-rating")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let haircut_pct = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "haircut-pct")
            .and_then(|a| a.value.as_decimal())
            .and_then(|d| d.to_f64());

        // CSA type defaults to "VM" (Variation Margin) if not specified
        let csa_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "csa-type")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "VM".to_string());

        // Generate a collateral ID from type and optional currency
        let collateral_id = if let Some(ref currs) = currencies {
            format!(
                "{}:{}",
                collateral_type,
                currs.first().unwrap_or(&"ANY".to_string())
            )
        } else {
            collateral_type.clone()
        };

        // Apply operation to AST and save
        let doc = ast_db::apply_and_save(
            pool,
            profile_id,
            TradingMatrixOp::AddCsaEligibleCollateral {
                isda_ref: counterparty_ref.clone(),
                csa_ref: csa_type.clone(),
                collateral_id,
                collateral_type,
                currency: currencies.as_ref().and_then(|c| c.first().cloned()),
                haircut_pct,
                concentration_limit_pct: None,
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to add CSA collateral: {}", e))?;

        Ok(ExecutionResult::Record(json!({
            "profile_id": profile_id,
            "counterparty_ref": counterparty_ref,
            "csa_type": csa_type,
            "version": doc.version,
            "status": format!("{:?}", doc.status),
        })))
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

/// Link CSA to collateral SSI
pub struct TradingProfileLinkCsaSsiOp;

#[async_trait]
impl CustomOperation for TradingProfileLinkCsaSsiOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }

    fn verb(&self) -> &'static str {
        "link-csa-ssi"
    }

    fn rationale(&self) -> &'static str {
        "Links a CSA to a collateral SSI for margin transfers"
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

        let counterparty_ref = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "counterparty-ref")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing counterparty-ref argument"))?;

        let ssi_name = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "ssi-name")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing ssi-name argument"))?;

        // CSA type defaults to "VM" (Variation Margin) if not specified
        let csa_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "csa-type")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "VM".to_string());

        // Apply operation to AST and save
        let doc = ast_db::apply_and_save(
            pool,
            profile_id,
            TradingMatrixOp::LinkCsaSsi {
                isda_ref: counterparty_ref.clone(),
                csa_ref: csa_type.clone(),
                ssi_ref: ssi_name.clone(),
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to link CSA SSI: {}", e))?;

        Ok(ExecutionResult::Record(json!({
            "profile_id": profile_id,
            "counterparty_ref": counterparty_ref,
            "csa_type": csa_type,
            "ssi_name": ssi_name,
            "version": doc.version,
            "status": format!("{:?}", doc.status),
        })))
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
// IM MANDATE OPERATIONS (Phase 3)
// =============================================================================

/// Add Investment Manager mandate to trading profile
pub struct TradingProfileAddImMandateOp;

#[async_trait]
impl CustomOperation for TradingProfileAddImMandateOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }

    fn verb(&self) -> &'static str {
        "add-im-mandate"
    }

    fn rationale(&self) -> &'static str {
        "Adds Investment Manager mandate configuration to the document"
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

        let manager_ref = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "manager-ref")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing manager-ref argument"))?;

        let manager_ref_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "manager-ref-type")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "NAME".to_string());

        let priority = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "priority")
            .and_then(|a| a.value.as_integer())
            .map(|i| i as i32)
            .unwrap_or(50);

        let role = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "role")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "INVESTMENT_MANAGER".to_string());

        let _scope_all = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "scope-all")
            .and_then(|a| a.value.as_boolean())
            .unwrap_or(true);

        let scope_mics = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "scope-mics")
            .and_then(|a| {
                a.value.as_list().map(|list| {
                    list.iter()
                        .filter_map(|node| node.as_string().map(|s| s.to_string()))
                        .collect()
                })
            });

        let scope_instrument_classes = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "scope-instrument-classes")
            .and_then(|a| {
                a.value.as_list().map(|list| {
                    list.iter()
                        .filter_map(|node| node.as_string().map(|s| s.to_string()))
                        .collect()
                })
            });

        let _instruction_method = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "instruction-method")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let can_trade = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "can-trade")
            .and_then(|a| a.value.as_boolean())
            .unwrap_or(true);

        let can_settle = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "can-settle")
            .and_then(|a| a.value.as_boolean())
            .unwrap_or(true);

        // Generate a unique mandate ID
        let mandate_id = Uuid::new_v4().to_string();

        // Resolve manager entity if needed - for now use manager_ref as both ID and name
        // In a fuller implementation, we'd look up the entity by LEI/BIC/UUID/NAME
        let manager_entity_id = if manager_ref_type == "UUID" {
            manager_ref.clone()
        } else {
            // Use manager_ref as the entity ID for now
            manager_ref.clone()
        };

        // Extract manager LEI if provided
        let manager_lei = if manager_ref_type == "LEI" {
            Some(manager_ref.clone())
        } else {
            None
        };

        // Apply operation to AST and save
        let doc = ast_db::apply_and_save(
            pool,
            profile_id,
            TradingMatrixOp::AddImMandate {
                manager_id: mandate_id,
                manager_entity_id,
                manager_name: manager_ref.clone(),
                manager_lei,
                priority,
                role: role.clone(),
                can_trade,
                can_settle,
                scope_instrument_classes: scope_instrument_classes.unwrap_or_default(),
                scope_markets: scope_mics.unwrap_or_default(),
                scope_currencies: vec![], // Not in current API
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to add IM mandate: {}", e))?;

        Ok(ExecutionResult::Record(json!({
            "profile_id": profile_id,
            "manager_ref": manager_ref,
            "role": role,
            "priority": priority,
            "version": doc.version,
            "status": format!("{:?}", doc.status),
        })))
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

/// Update Investment Manager scope
pub struct TradingProfileUpdateImScopeOp;

#[async_trait]
impl CustomOperation for TradingProfileUpdateImScopeOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }

    fn verb(&self) -> &'static str {
        "update-im-scope"
    }

    fn rationale(&self) -> &'static str {
        "Updates the scope of an existing Investment Manager mandate"
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

        let manager_ref = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "manager-ref")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing manager-ref argument"))?;

        let _scope_all = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "scope-all")
            .and_then(|a| a.value.as_boolean());

        let scope_mics = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "scope-mics")
            .and_then(|a| {
                a.value.as_list().map(|list| {
                    list.iter()
                        .filter_map(|node| node.as_string().map(|s| s.to_string()))
                        .collect()
                })
            });

        let scope_instrument_classes = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "scope-instrument-classes")
            .and_then(|a| {
                a.value.as_list().map(|list| {
                    list.iter()
                        .filter_map(|node| node.as_string().map(|s| s.to_string()))
                        .collect()
                })
            });

        // Apply operation to AST and save
        let doc = ast_db::apply_and_save(
            pool,
            profile_id,
            TradingMatrixOp::UpdateImScope {
                manager_ref: manager_ref.clone(),
                scope_instrument_classes,
                scope_markets: scope_mics,
                scope_currencies: None, // Not in current API
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to update IM scope: {}", e))?;

        Ok(ExecutionResult::Record(json!({
            "profile_id": profile_id,
            "manager_ref": manager_ref,
            "version": doc.version,
            "status": format!("{:?}", doc.status),
        })))
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

/// Remove Investment Manager mandate
pub struct TradingProfileRemoveImMandateOp;

#[async_trait]
impl CustomOperation for TradingProfileRemoveImMandateOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }

    fn verb(&self) -> &'static str {
        "remove-im-mandate"
    }

    fn rationale(&self) -> &'static str {
        "Removes an Investment Manager mandate from the document"
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

        let manager_ref = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "manager-ref")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing manager-ref argument"))?;

        // Build node ID: _Managers / {manager_ref}
        let node_id = TradingMatrixNodeId::category(categories::MANAGERS).child(&manager_ref);

        // Apply operation to AST and save
        let doc = ast_db::apply_and_save(pool, profile_id, TradingMatrixOp::RemoveNode { node_id })
            .await
            .map_err(|e| anyhow::anyhow!("Failed to remove IM mandate: {}", e))?;

        Ok(ExecutionResult::Record(json!({
            "profile_id": profile_id,
            "removed": manager_ref,
            "version": doc.version,
            "status": format!("{:?}", doc.status),
        })))
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
// SETTLEMENT CONFIG OPERATIONS (Phase 3)
// =============================================================================

/// Set base currency for the trading profile
/// Verb: trading-profile.set-base-currency
pub struct TradingProfileSetBaseCurrencyOp;

#[async_trait]
impl CustomOperation for TradingProfileSetBaseCurrencyOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }

    fn verb(&self) -> &'static str {
        "set-base-currency"
    }

    fn rationale(&self) -> &'static str {
        "Sets the base currency for the trading profile document"
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

        let currency = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "currency")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing currency argument"))?;

        // Apply operation to AST and save
        let doc = ast_db::apply_and_save(
            pool,
            profile_id,
            TradingMatrixOp::SetBaseCurrency {
                currency: currency.clone(),
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to set base currency: {}", e))?;

        Ok(ExecutionResult::Record(json!({
            "profile_id": profile_id,
            "base_currency": currency,
            "version": doc.version,
            "status": format!("{:?}", doc.status),
        })))
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

/// Add allowed currency to the trading profile
/// Verb: trading-profile.add-allowed-currency
pub struct TradingProfileAddAllowedCurrencyOp;

#[async_trait]
impl CustomOperation for TradingProfileAddAllowedCurrencyOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }

    fn verb(&self) -> &'static str {
        "add-allowed-currency"
    }

    fn rationale(&self) -> &'static str {
        "Adds an allowed currency to the trading profile document"
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

        let currency = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "currency")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing currency argument"))?;

        // Apply operation to AST and save
        let doc = ast_db::apply_and_save(
            pool,
            profile_id,
            TradingMatrixOp::AddAllowedCurrency {
                currency: currency.clone(),
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to add allowed currency: {}", e))?;

        Ok(ExecutionResult::Record(json!({
            "profile_id": profile_id,
            "currency": currency,
            "version": doc.version,
            "status": format!("{:?}", doc.status),
        })))
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
// SYNC OPERATIONS (Phase 4)
// =============================================================================

/// Compare document with operational tables to show differences
/// Verb: trading-profile.diff
pub struct TradingProfileDiffOp;

#[async_trait]
impl CustomOperation for TradingProfileDiffOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }

    fn verb(&self) -> &'static str {
        "diff"
    }

    fn rationale(&self) -> &'static str {
        "Compares document with operational tables to identify sync differences"
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

        let result = document_ops::diff_document_vs_operational(pool, profile_id)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to diff document: {}", e))?;

        Ok(ExecutionResult::Record(serde_json::to_value(&result)?))
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
// PHASE 5: VALIDATION OPERATIONS
// =============================================================================

/// Validate that booking rules cover all universe combinations
pub struct TradingProfileValidateCoverageOp;

#[async_trait]
impl CustomOperation for TradingProfileValidateCoverageOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }
    fn verb(&self) -> &'static str {
        "validate-universe-coverage"
    }
    fn rationale(&self) -> &'static str {
        "Validates that booking rules cover all market/instrument/currency combinations in the universe"
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
            .ok_or_else(|| anyhow::anyhow!("profile-id is required"))?;

        let result = document_ops::validate_coverage(pool, profile_id).await?;

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(
            json!({"is_valid": true, "coverage_percentage": 100.0}),
        ))
    }
}

/// Validate that a profile is ready for go-live
pub struct TradingProfileValidateGoLiveReadyOp;

#[async_trait]
impl CustomOperation for TradingProfileValidateGoLiveReadyOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }
    fn verb(&self) -> &'static str {
        "validate-go-live-ready"
    }
    fn rationale(&self) -> &'static str {
        "Validates that a trading profile has all required components for production use"
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
            .ok_or_else(|| anyhow::anyhow!("profile-id is required"))?;

        let result = document_ops::validate_go_live_ready(pool, profile_id).await?;

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(json!({"is_ready": true})))
    }
}

// =============================================================================
// PHASE 6: Document Lifecycle Operations
// =============================================================================

/// Submit a draft profile for review
/// Transitions: Draft → PendingReview
pub struct TradingProfileSubmitOp;

#[async_trait]
impl CustomOperation for TradingProfileSubmitOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }

    fn verb(&self) -> &'static str {
        "submit"
    }

    fn rationale(&self) -> &'static str {
        "Submits a draft trading profile for review. Validates the profile is ready before transitioning from Draft to PendingReview status."
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
            .ok_or_else(|| anyhow::anyhow!("profile-id is required"))?;

        let submitted_by: Option<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "submitted-by")
            .and_then(|a| a.value.as_string().map(|s| s.to_string()));

        let notes: Option<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "notes")
            .and_then(|a| a.value.as_string().map(|s| s.to_string()));

        let result = document_ops::submit_for_review(pool, profile_id, submitted_by, notes).await?;

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(json!({"status": "submitted"})))
    }
}

/// Approve a profile pending review
/// Transitions: PendingReview → Active
pub struct TradingProfileApproveOp;

#[async_trait]
impl CustomOperation for TradingProfileApproveOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }

    fn verb(&self) -> &'static str {
        "approve"
    }

    fn rationale(&self) -> &'static str {
        "Approves a trading profile pending review, transitioning it to Active status. Any previously active profile for the same CBU is superseded."
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
            .ok_or_else(|| anyhow::anyhow!("profile-id is required"))?;

        let approved_by: Option<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "approved-by")
            .and_then(|a| a.value.as_string().map(|s| s.to_string()));

        let notes: Option<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "notes")
            .and_then(|a| a.value.as_string().map(|s| s.to_string()));

        // First, approve the profile (transitions PENDING_REVIEW -> ACTIVE)
        let approve_result =
            document_ops::approve_profile(pool, profile_id, approved_by, notes).await?;

        // After approval, automatically materialize the document to operational tables
        // This ensures the Trading Matrix API can read the activated configuration
        let row = sqlx::query!(
            r#"SELECT cbu_id, document FROM "ob-poc".cbu_trading_profiles WHERE profile_id = $1"#,
            profile_id
        )
        .fetch_one(pool)
        .await?;

        let cbu_id = row.cbu_id;
        let document: TradingProfileDocument = serde_json::from_value(row.document)?;

        // Start transaction for materialization
        let mut tx = pool.begin().await?;

        // Build reference maps for materialization
        let mut refs = ReferenceMaps::new();
        refs.instrument_class_map = build_instrument_class_map(&mut tx).await?;
        refs.market_map = build_market_map(&mut tx).await?;

        let opts = MaterializationOptions { force: true };

        // Materialize SSIs first (booking rules reference them)
        for (category, ssis) in &document.standing_instructions {
            for ssi in ssis {
                let ssi_id = materialize_ssi(&mut tx, cbu_id, category, ssi, &refs, &opts).await?;
                refs.ssi_name_to_id.insert(ssi.name.clone(), ssi_id);
            }
        }

        // Materialize universe
        materialize_universe(&mut tx, cbu_id, &document.universe, &refs, &opts).await?;

        // Materialize booking rules
        materialize_booking_rules(&mut tx, cbu_id, &document.booking_rules, &refs, &opts).await?;

        // Materialize ISDA agreements if present
        if !document.isda_agreements.is_empty() {
            materialize_isda_agreements(
                &mut tx,
                pool,
                cbu_id,
                &document.isda_agreements,
                &refs.ssi_name_to_id,
            )
            .await?;
        }

        // Commit transaction
        tx.commit().await?;

        Ok(ExecutionResult::Record(serde_json::to_value(
            approve_result,
        )?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(json!({"status": "approved"})))
    }
}

/// Reject a profile pending review
/// Transitions: PendingReview → Draft
pub struct TradingProfileRejectOp;

#[async_trait]
impl CustomOperation for TradingProfileRejectOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }

    fn verb(&self) -> &'static str {
        "reject"
    }

    fn rationale(&self) -> &'static str {
        "Rejects a trading profile pending review, transitioning it back to Draft status with a rejection reason for remediation."
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
            .ok_or_else(|| anyhow::anyhow!("profile-id is required"))?;

        let rejection_reason: String = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "reason")
            .and_then(|a| a.value.as_string().map(|s| s.to_string()))
            .ok_or_else(|| anyhow::anyhow!("reason is required"))?;

        let rejected_by: Option<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "rejected-by")
            .and_then(|a| a.value.as_string().map(|s| s.to_string()));

        let result =
            document_ops::reject_profile(pool, profile_id, rejection_reason, rejected_by).await?;

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(json!({"status": "rejected"})))
    }
}

/// Archive an active or superseded profile
/// Transitions: Active|Superseded → Archived
pub struct TradingProfileArchiveOp;

#[async_trait]
impl CustomOperation for TradingProfileArchiveOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }

    fn verb(&self) -> &'static str {
        "archive"
    }

    fn rationale(&self) -> &'static str {
        "Archives an active or superseded trading profile, removing it from operational use while preserving the audit trail."
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
            .ok_or_else(|| anyhow::anyhow!("profile-id is required"))?;

        let archived_by: Option<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "archived-by")
            .and_then(|a| a.value.as_string().map(|s| s.to_string()));

        let notes: Option<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "notes")
            .and_then(|a| a.value.as_string().map(|s| s.to_string()));

        let result = document_ops::archive_profile(pool, profile_id, archived_by, notes).await?;

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(json!({"status": "archived"})))
    }
}

// =============================================================================
// CLONE OPERATION
// =============================================================================

/// Clone a trading profile to another CBU
///
/// Creates a new DRAFT profile for the target CBU with the document content
/// from the source profile. Useful for:
/// - Setting up new funds with similar trading configuration
/// - Creating templates from production profiles
/// - Migrating config during fund family restructuring
///
/// DSL: (trading-profile.clone-to :profile-id @source :target-cbu-id @target-cbu)
pub struct TradingProfileCloneToOp;

#[async_trait]
impl CustomOperation for TradingProfileCloneToOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }

    fn verb(&self) -> &'static str {
        "clone-to"
    }

    fn rationale(&self) -> &'static str {
        "Clones a trading profile document to another CBU, creating a new DRAFT profile that can be customized before activation."
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Source profile ID (required)
        let source_profile_id: Uuid = verb_call
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
            .ok_or_else(|| anyhow::anyhow!("profile-id is required"))?;

        // Target CBU ID (required)
        let target_cbu_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "target-cbu-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("target-cbu-id is required"))?;

        // Optional: notes
        let notes: Option<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "notes")
            .and_then(|a| a.value.as_string().map(|s| s.to_string()));

        // Use ast_db::clone_to_draft for new AST-based document format
        let (profile_id, doc) =
            ast_db::clone_to_draft(pool, source_profile_id, target_cbu_id, notes)
                .await
                .map_err(|e| anyhow::anyhow!("Clone failed: {}", e))?;

        // Bind target profile ID if :as binding specified
        if let Some(binding_name) = verb_call.binding.as_ref() {
            ctx.bind(binding_name, profile_id);
        }

        Ok(ExecutionResult::Record(json!({
            "profile_id": profile_id,
            "cbu_id": target_cbu_id,
            "cbu_name": doc.cbu_name,
            "status": "DRAFT",
            "version": doc.version
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(json!({"status": "cloned"})))
    }
}

// =============================================================================
// CREATE NEW VERSION OPERATION
// =============================================================================

/// Create a new draft version from the current ACTIVE profile
/// Used when modifications are needed to a live trading matrix
pub struct TradingProfileCreateNewVersionOp;

#[async_trait]
impl CustomOperation for TradingProfileCreateNewVersionOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }

    fn verb(&self) -> &'static str {
        "create-new-version"
    }

    fn rationale(&self) -> &'static str {
        "Creates a new DRAFT version from the current ACTIVE profile. Enforces that only one working version exists at a time."
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
            .ok_or_else(|| anyhow::anyhow!("cbu-id is required"))?;

        let created_by: Option<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "created-by")
            .and_then(|a| a.value.as_string().map(|s| s.to_string()));

        let notes: Option<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "notes")
            .and_then(|a| a.value.as_string().map(|s| s.to_string()));

        let result = document_ops::create_new_version(pool, cbu_id, created_by, notes).await?;

        // Bind new profile ID if :as binding specified
        if let Some(binding_name) = verb_call.binding.as_ref() {
            ctx.bind(binding_name, result.new_profile_id);
        }

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(
            json!({"status": "new_version_created"}),
        ))
    }
}
