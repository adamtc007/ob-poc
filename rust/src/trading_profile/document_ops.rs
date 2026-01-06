//! Document-level operations for TradingProfileDocument
//!
//! These handlers modify the JSONB document directly, not operational tables.
//! The document is the source of truth; operational tables are materialized from it.

use anyhow::Result;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use super::types::*;

/// Error types for document operations
#[derive(Debug, thiserror::Error)]
pub enum DocumentOpError {
    #[error("Profile not found: {0}")]
    ProfileNotFound(Uuid),

    #[error("{item} already exists: {key}")]
    AlreadyExists { item: &'static str, key: String },

    #[error("{item} not found: {key}")]
    NotFound { item: &'static str, key: String },

    #[error("Invalid reference: {0}")]
    InvalidReference(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Profile is not in DRAFT status, cannot modify")]
    NotDraft,
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Get profile document as parsed TradingProfileDocument
pub async fn get_and_parse_profile(
    pool: &PgPool,
    profile_id: Uuid,
) -> Result<TradingProfileDocument, DocumentOpError> {
    let row = sqlx::query!(
        r#"SELECT document FROM "ob-poc".cbu_trading_profiles WHERE profile_id = $1"#,
        profile_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or(DocumentOpError::ProfileNotFound(profile_id))?;

    let doc: TradingProfileDocument = serde_json::from_value(row.document)?;
    Ok(doc)
}

/// Get profile status
pub async fn get_profile_status(
    pool: &PgPool,
    profile_id: Uuid,
) -> Result<String, DocumentOpError> {
    let row = sqlx::query!(
        r#"SELECT status FROM "ob-poc".cbu_trading_profiles WHERE profile_id = $1"#,
        profile_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or(DocumentOpError::ProfileNotFound(profile_id))?;

    Ok(row.status)
}

/// Get CBU ID for a profile
pub async fn get_profile_cbu_id(pool: &PgPool, profile_id: Uuid) -> Result<Uuid, DocumentOpError> {
    let row = sqlx::query!(
        r#"SELECT cbu_id FROM "ob-poc".cbu_trading_profiles WHERE profile_id = $1"#,
        profile_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or(DocumentOpError::ProfileNotFound(profile_id))?;

    Ok(row.cbu_id)
}

/// Update profile document in database
pub async fn update_profile_document(
    pool: &PgPool,
    profile_id: Uuid,
    doc: &TradingProfileDocument,
) -> Result<(), DocumentOpError> {
    let doc_json = serde_json::to_value(doc)?;
    let hash = compute_document_hash(doc);

    sqlx::query!(
        r#"UPDATE "ob-poc".cbu_trading_profiles
           SET document = $2,
               document_hash = $3
           WHERE profile_id = $1"#,
        profile_id,
        doc_json,
        hash
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Compute SHA256 hash of document for change detection
pub fn compute_document_hash(doc: &TradingProfileDocument) -> String {
    use sha2::{Digest, Sha256};
    let json = serde_json::to_string(doc).unwrap_or_default();
    let hash = Sha256::digest(json.as_bytes());
    format!("{:x}", hash)
}

/// Ensure profile is in DRAFT status before modifying
async fn ensure_draft(pool: &PgPool, profile_id: Uuid) -> Result<(), DocumentOpError> {
    let status = get_profile_status(pool, profile_id).await?;
    if status != "DRAFT" {
        return Err(DocumentOpError::NotDraft);
    }
    Ok(())
}

// =============================================================================
// DOCUMENT LIFECYCLE
// =============================================================================

/// Create a new draft trading profile for a CBU
pub async fn create_draft_profile(
    pool: &PgPool,
    cbu_id: Uuid,
    base_currency: String,
    copy_from_profile: Option<Uuid>,
    notes: Option<String>,
) -> Result<Uuid, DocumentOpError> {
    // If copying from existing profile, use that document
    let doc = if let Some(source_id) = copy_from_profile {
        let mut source_doc = get_and_parse_profile(pool, source_id).await?;
        // Update base currency if different
        source_doc.universe.base_currency = base_currency;
        source_doc
    } else {
        // Create empty document with just base currency
        TradingProfileDocument {
            universe: Universe {
                base_currency,
                allowed_currencies: vec![],
                allowed_markets: vec![],
                instrument_classes: vec![],
            },
            investment_managers: vec![],
            isda_agreements: vec![],
            settlement_config: None,
            booking_rules: vec![],
            standing_instructions: std::collections::HashMap::new(),
            pricing_matrix: vec![],
            valuation_config: None,
            constraints: None,
            metadata: Some(ProfileMetadata {
                source: Some("DSL".to_string()),
                source_ref: None,
                created_by: None,
                notes,
                regulatory_framework: None,
            }),
        }
    };

    // Get next version number for this CBU
    let version = sqlx::query_scalar!(
        r#"SELECT COALESCE(MAX(version), 0) + 1 as "version!"
           FROM "ob-poc".cbu_trading_profiles
           WHERE cbu_id = $1"#,
        cbu_id
    )
    .fetch_one(pool)
    .await?;

    let profile_id = Uuid::new_v4();
    let doc_json = serde_json::to_value(&doc)?;
    let hash = compute_document_hash(&doc);

    sqlx::query!(
        r#"INSERT INTO "ob-poc".cbu_trading_profiles
           (profile_id, cbu_id, version, status, document, document_hash, created_at)
           VALUES ($1, $2, $3, 'DRAFT', $4, $5, now())"#,
        profile_id,
        cbu_id,
        version as i32,
        doc_json,
        hash
    )
    .execute(pool)
    .await?;

    Ok(profile_id)
}

// =============================================================================
// UNIVERSE OPERATIONS
// =============================================================================

/// Add instrument class to profile universe
pub async fn add_instrument_class(
    pool: &PgPool,
    profile_id: Uuid,
    class_code: String,
    cfi_prefixes: Option<Vec<String>>,
    isda_asset_classes: Option<Vec<String>>,
    is_held: bool,
    is_traded: bool,
) -> Result<Value, DocumentOpError> {
    ensure_draft(pool, profile_id).await?;

    let mut doc = get_and_parse_profile(pool, profile_id).await?;

    // Check if class already exists
    if doc
        .universe
        .instrument_classes
        .iter()
        .any(|c| c.class_code == class_code)
    {
        return Err(DocumentOpError::AlreadyExists {
            item: "instrument_class",
            key: class_code,
        });
    }

    // Add new class
    doc.universe.instrument_classes.push(InstrumentClassConfig {
        class_code,
        cfi_prefixes: cfi_prefixes.unwrap_or_default(),
        isda_asset_classes: isda_asset_classes.unwrap_or_default(),
        is_held,
        is_traded,
    });

    update_profile_document(pool, profile_id, &doc).await?;

    Ok(serde_json::to_value(&doc.universe)?)
}

/// Remove instrument class from profile universe
pub async fn remove_instrument_class(
    pool: &PgPool,
    profile_id: Uuid,
    class_code: String,
) -> Result<i32, DocumentOpError> {
    ensure_draft(pool, profile_id).await?;

    let mut doc = get_and_parse_profile(pool, profile_id).await?;

    let original_len = doc.universe.instrument_classes.len();
    doc.universe
        .instrument_classes
        .retain(|c| c.class_code != class_code);

    if doc.universe.instrument_classes.len() == original_len {
        return Err(DocumentOpError::NotFound {
            item: "instrument_class",
            key: class_code,
        });
    }

    update_profile_document(pool, profile_id, &doc).await?;

    Ok(1)
}

/// Add market to profile universe
pub async fn add_market(
    pool: &PgPool,
    profile_id: Uuid,
    mic: String,
    currencies: Vec<String>,
    settlement_types: Option<Vec<String>>,
) -> Result<Value, DocumentOpError> {
    ensure_draft(pool, profile_id).await?;

    let mut doc = get_and_parse_profile(pool, profile_id).await?;

    // Check if market already exists - if so, update it
    if let Some(market) = doc
        .universe
        .allowed_markets
        .iter_mut()
        .find(|m| m.mic == mic)
    {
        market.currencies = currencies;
        if let Some(st) = settlement_types {
            market.settlement_types = st;
        }
    } else {
        // Add new market
        doc.universe.allowed_markets.push(MarketConfig {
            mic,
            currencies,
            settlement_types: settlement_types.unwrap_or_else(|| vec!["DVP".to_string()]),
        });
    }

    update_profile_document(pool, profile_id, &doc).await?;

    Ok(serde_json::to_value(&doc.universe)?)
}

/// Remove market from profile universe
pub async fn remove_market(
    pool: &PgPool,
    profile_id: Uuid,
    mic: String,
) -> Result<i32, DocumentOpError> {
    ensure_draft(pool, profile_id).await?;

    let mut doc = get_and_parse_profile(pool, profile_id).await?;

    let original_len = doc.universe.allowed_markets.len();
    doc.universe.allowed_markets.retain(|m| m.mic != mic);

    if doc.universe.allowed_markets.len() == original_len {
        return Err(DocumentOpError::NotFound {
            item: "market",
            key: mic,
        });
    }

    update_profile_document(pool, profile_id, &doc).await?;

    Ok(1)
}

/// Set base currency
pub async fn set_base_currency(
    pool: &PgPool,
    profile_id: Uuid,
    currency: String,
) -> Result<i32, DocumentOpError> {
    ensure_draft(pool, profile_id).await?;

    let mut doc = get_and_parse_profile(pool, profile_id).await?;
    doc.universe.base_currency = currency;
    update_profile_document(pool, profile_id, &doc).await?;

    Ok(1)
}

/// Add allowed currency
pub async fn add_allowed_currency(
    pool: &PgPool,
    profile_id: Uuid,
    currency: String,
) -> Result<i32, DocumentOpError> {
    ensure_draft(pool, profile_id).await?;

    let mut doc = get_and_parse_profile(pool, profile_id).await?;

    if !doc.universe.allowed_currencies.contains(&currency) {
        doc.universe.allowed_currencies.push(currency);
        update_profile_document(pool, profile_id, &doc).await?;
    }

    Ok(1)
}

// =============================================================================
// STANDING INSTRUCTIONS
// =============================================================================

/// Add standing instruction to profile
pub async fn add_standing_instruction(
    pool: &PgPool,
    profile_id: Uuid,
    category: String,
    name: String,
    mic: Option<String>,
    currency: Option<String>,
    custody_account: Option<String>,
    custody_bic: Option<String>,
    cash_account: Option<String>,
    cash_bic: Option<String>,
    settlement_model: Option<String>,
    cutoff_time: Option<String>,
    cutoff_timezone: Option<String>,
) -> Result<i32, DocumentOpError> {
    ensure_draft(pool, profile_id).await?;

    let mut doc = get_and_parse_profile(pool, profile_id).await?;

    let ssi = StandingInstruction {
        name: name.clone(),
        mic,
        currency,
        custody_account,
        custody_bic,
        cash_account,
        cash_bic,
        settlement_model,
        cutoff: if cutoff_time.is_some() && cutoff_timezone.is_some() {
            Some(CutoffConfig {
                time: cutoff_time.unwrap(),
                timezone: cutoff_timezone.unwrap(),
            })
        } else {
            None
        },
        counterparty: None,
        counterparty_lei: None,
        provider_ref: None,
        channel: None,
        reporting_frequency: None,
    };

    // Get or create the category
    let category_list = doc
        .standing_instructions
        .entry(category.clone())
        .or_insert_with(Vec::new);

    // Check if SSI with this name already exists in category
    if category_list.iter().any(|s| s.name == name) {
        return Err(DocumentOpError::AlreadyExists {
            item: "standing_instruction",
            key: format!("{}/{}", category, name),
        });
    }

    category_list.push(ssi);
    update_profile_document(pool, profile_id, &doc).await?;

    Ok(1)
}

/// Remove standing instruction from profile
pub async fn remove_standing_instruction(
    pool: &PgPool,
    profile_id: Uuid,
    category: String,
    name: String,
) -> Result<i32, DocumentOpError> {
    ensure_draft(pool, profile_id).await?;

    let mut doc = get_and_parse_profile(pool, profile_id).await?;

    let Some(category_list) = doc.standing_instructions.get_mut(&category) else {
        return Err(DocumentOpError::NotFound {
            item: "standing_instruction_category",
            key: category,
        });
    };

    let original_len = category_list.len();
    category_list.retain(|s| s.name != name);

    if category_list.len() == original_len {
        return Err(DocumentOpError::NotFound {
            item: "standing_instruction",
            key: format!("{}/{}", category, name),
        });
    }

    update_profile_document(pool, profile_id, &doc).await?;

    Ok(1)
}

// =============================================================================
// BOOKING RULES
// =============================================================================

/// Add booking rule to profile
pub async fn add_booking_rule(
    pool: &PgPool,
    profile_id: Uuid,
    name: String,
    priority: i32,
    ssi_ref: String,
    match_counterparty_ref: Option<String>,
    match_counterparty_ref_type: Option<String>,
    match_instrument_class: Option<String>,
    match_security_type: Option<String>,
    match_mic: Option<String>,
    match_currency: Option<String>,
    match_settlement_type: Option<String>,
) -> Result<i32, DocumentOpError> {
    ensure_draft(pool, profile_id).await?;

    let mut doc = get_and_parse_profile(pool, profile_id).await?;

    // Check if rule with this name already exists
    if doc.booking_rules.iter().any(|r| r.name == name) {
        return Err(DocumentOpError::AlreadyExists {
            item: "booking_rule",
            key: name,
        });
    }

    let counterparty = match (match_counterparty_ref, match_counterparty_ref_type) {
        (Some(value), Some(ref_type)) => Some(EntityRef {
            ref_type: match ref_type.as_str() {
                "LEI" => EntityRefType::Lei,
                "BIC" => EntityRefType::Bic,
                "UUID" => EntityRefType::Uuid,
                _ => EntityRefType::Name,
            },
            value,
        }),
        _ => None,
    };

    let rule = BookingRule {
        name,
        priority,
        match_criteria: BookingMatch {
            counterparty,
            instrument_class: match_instrument_class,
            security_type: match_security_type,
            mic: match_mic,
            currency: match_currency,
            settlement_type: match_settlement_type,
        },
        ssi_ref,
    };

    doc.booking_rules.push(rule);

    // Sort by priority
    doc.booking_rules.sort_by_key(|r| r.priority);

    update_profile_document(pool, profile_id, &doc).await?;

    Ok(1)
}

/// Remove booking rule from profile
pub async fn remove_booking_rule(
    pool: &PgPool,
    profile_id: Uuid,
    name: String,
) -> Result<i32, DocumentOpError> {
    ensure_draft(pool, profile_id).await?;

    let mut doc = get_and_parse_profile(pool, profile_id).await?;

    let original_len = doc.booking_rules.len();
    doc.booking_rules.retain(|r| r.name != name);

    if doc.booking_rules.len() == original_len {
        return Err(DocumentOpError::NotFound {
            item: "booking_rule",
            key: name,
        });
    }

    update_profile_document(pool, profile_id, &doc).await?;

    Ok(1)
}

// =============================================================================
// ISDA AGREEMENTS
// =============================================================================

/// Add ISDA agreement configuration to profile
pub async fn add_isda_config(
    pool: &PgPool,
    profile_id: Uuid,
    counterparty_ref: String,
    counterparty_ref_type: String,
    agreement_date: String,
    governing_law: String,
    effective_date: Option<String>,
) -> Result<Value, DocumentOpError> {
    ensure_draft(pool, profile_id).await?;

    let mut doc = get_and_parse_profile(pool, profile_id).await?;

    let ref_type = match counterparty_ref_type.as_str() {
        "LEI" => EntityRefType::Lei,
        "BIC" => EntityRefType::Bic,
        "UUID" => EntityRefType::Uuid,
        _ => EntityRefType::Name,
    };

    // Check if ISDA with this counterparty already exists
    if doc
        .isda_agreements
        .iter()
        .any(|i| i.counterparty.value == counterparty_ref && i.counterparty.ref_type == ref_type)
    {
        return Err(DocumentOpError::AlreadyExists {
            item: "isda_agreement",
            key: counterparty_ref,
        });
    }

    let isda = IsdaAgreementConfig {
        counterparty: EntityRef {
            ref_type,
            value: counterparty_ref,
        },
        agreement_date,
        governing_law,
        effective_date,
        product_coverage: vec![],
        csa: None,
    };

    doc.isda_agreements.push(isda);
    update_profile_document(pool, profile_id, &doc).await?;

    Ok(serde_json::to_value(&doc.isda_agreements)?)
}

/// Add product coverage to ISDA
pub async fn add_isda_product_coverage(
    pool: &PgPool,
    profile_id: Uuid,
    counterparty_ref: String,
    asset_class: String,
    base_products: Option<Vec<String>>,
) -> Result<i32, DocumentOpError> {
    ensure_draft(pool, profile_id).await?;

    let mut doc = get_and_parse_profile(pool, profile_id).await?;

    let isda = doc
        .isda_agreements
        .iter_mut()
        .find(|i| i.counterparty.value == counterparty_ref)
        .ok_or_else(|| DocumentOpError::NotFound {
            item: "isda_agreement",
            key: counterparty_ref,
        })?;

    // Add or update coverage for this asset class
    if let Some(existing) = isda
        .product_coverage
        .iter_mut()
        .find(|c| c.asset_class == asset_class)
    {
        if let Some(products) = base_products {
            existing.base_products = products;
        }
    } else {
        isda.product_coverage.push(ProductCoverage {
            asset_class,
            base_products: base_products.unwrap_or_default(),
        });
    }

    update_profile_document(pool, profile_id, &doc).await?;

    Ok(1)
}

/// Add CSA configuration to ISDA
pub async fn add_csa_config(
    pool: &PgPool,
    profile_id: Uuid,
    counterparty_ref: String,
    csa_type: String,
    threshold_amount: Option<i64>,
    threshold_currency: Option<String>,
    mta: Option<i64>,
    rounding: Option<i64>,
    valuation_time: Option<String>,
    valuation_timezone: Option<String>,
    settlement_days: Option<i32>,
) -> Result<Value, DocumentOpError> {
    ensure_draft(pool, profile_id).await?;

    let mut doc = get_and_parse_profile(pool, profile_id).await?;

    let isda = doc
        .isda_agreements
        .iter_mut()
        .find(|i| i.counterparty.value == counterparty_ref)
        .ok_or_else(|| DocumentOpError::NotFound {
            item: "isda_agreement",
            key: counterparty_ref,
        })?;

    let new_csa = CsaConfig {
        csa_type,
        threshold_amount,
        threshold_currency,
        minimum_transfer_amount: mta,
        rounding_amount: rounding,
        eligible_collateral: vec![],
        initial_margin: None,
        collateral_ssi_ref: None,
        collateral_ssi: None,
        valuation_time,
        valuation_timezone,
        notification_time: None,
        settlement_days,
        dispute_resolution: None,
    };
    let csa_value = serde_json::to_value(&new_csa)?;
    isda.csa = Some(new_csa);

    update_profile_document(pool, profile_id, &doc).await?;

    Ok(csa_value)
}

/// Add eligible collateral to CSA
pub async fn add_csa_eligible_collateral(
    pool: &PgPool,
    profile_id: Uuid,
    counterparty_ref: String,
    collateral_type: String,
    currencies: Option<Vec<String>>,
    issuers: Option<Vec<String>>,
    min_rating: Option<String>,
    haircut_pct: f64,
) -> Result<i32, DocumentOpError> {
    ensure_draft(pool, profile_id).await?;

    let mut doc = get_and_parse_profile(pool, profile_id).await?;

    let isda = doc
        .isda_agreements
        .iter_mut()
        .find(|i| i.counterparty.value == counterparty_ref)
        .ok_or_else(|| DocumentOpError::NotFound {
            item: "isda_agreement",
            key: counterparty_ref.clone(),
        })?;

    let csa = isda.csa.as_mut().ok_or_else(|| DocumentOpError::NotFound {
        item: "csa",
        key: counterparty_ref,
    })?;

    csa.eligible_collateral.push(EligibleCollateral {
        collateral_type,
        currencies: currencies.unwrap_or_default(),
        issuers: issuers.unwrap_or_default(),
        min_rating,
        haircut_pct: Some(haircut_pct),
    });

    update_profile_document(pool, profile_id, &doc).await?;

    Ok(1)
}

/// Link CSA to collateral SSI
pub async fn link_csa_ssi(
    pool: &PgPool,
    profile_id: Uuid,
    counterparty_ref: String,
    ssi_name: String,
) -> Result<i32, DocumentOpError> {
    ensure_draft(pool, profile_id).await?;

    let mut doc = get_and_parse_profile(pool, profile_id).await?;

    // Validate SSI exists in OTC_COLLATERAL category
    let collateral_ssis = doc.standing_instructions.get("OTC_COLLATERAL");
    if !collateral_ssis
        .map(|ssis| ssis.iter().any(|s| s.name == ssi_name))
        .unwrap_or(false)
    {
        return Err(DocumentOpError::InvalidReference(format!(
            "SSI '{}' not found in OTC_COLLATERAL category",
            ssi_name
        )));
    }

    let isda = doc
        .isda_agreements
        .iter_mut()
        .find(|i| i.counterparty.value == counterparty_ref)
        .ok_or_else(|| DocumentOpError::NotFound {
            item: "isda_agreement",
            key: counterparty_ref.clone(),
        })?;

    let csa = isda.csa.as_mut().ok_or_else(|| DocumentOpError::NotFound {
        item: "csa",
        key: counterparty_ref,
    })?;

    csa.collateral_ssi_ref = Some(ssi_name);

    update_profile_document(pool, profile_id, &doc).await?;

    Ok(1)
}

// =============================================================================
// INVESTMENT MANAGERS
// =============================================================================

/// Add investment manager mandate to profile
pub async fn add_im_mandate(
    pool: &PgPool,
    profile_id: Uuid,
    manager_ref: String,
    manager_ref_type: String,
    priority: i32,
    role: String,
    scope_all: bool,
    scope_mics: Option<Vec<String>>,
    scope_instrument_classes: Option<Vec<String>>,
    instruction_method: Option<String>,
    can_trade: bool,
    can_settle: bool,
) -> Result<Value, DocumentOpError> {
    ensure_draft(pool, profile_id).await?;

    let mut doc = get_and_parse_profile(pool, profile_id).await?;

    let ref_type = match manager_ref_type.as_str() {
        "LEI" => EntityRefType::Lei,
        "BIC" => EntityRefType::Bic,
        "UUID" => EntityRefType::Uuid,
        _ => EntityRefType::Name,
    };

    // Check if mandate for this manager already exists
    if doc
        .investment_managers
        .iter()
        .any(|m| m.manager.value == manager_ref)
    {
        return Err(DocumentOpError::AlreadyExists {
            item: "im_mandate",
            key: manager_ref,
        });
    }

    let mandate = InvestmentManagerMandate {
        priority,
        manager: EntityRef {
            ref_type,
            value: manager_ref,
        },
        role,
        scope: ManagerScope {
            all: scope_all,
            mics: scope_mics.unwrap_or_default(),
            instrument_classes: scope_instrument_classes.unwrap_or_default(),
        },
        instruction_method,
        can_trade,
        can_settle,
    };

    doc.investment_managers.push(mandate);

    // Sort by priority
    doc.investment_managers.sort_by_key(|m| m.priority);

    update_profile_document(pool, profile_id, &doc).await?;

    Ok(serde_json::to_value(&doc.investment_managers)?)
}

/// Update IM scope
pub async fn update_im_scope(
    pool: &PgPool,
    profile_id: Uuid,
    manager_ref: String,
    scope_all: Option<bool>,
    scope_mics: Option<Vec<String>>,
    scope_instrument_classes: Option<Vec<String>>,
) -> Result<i32, DocumentOpError> {
    ensure_draft(pool, profile_id).await?;

    let mut doc = get_and_parse_profile(pool, profile_id).await?;

    let mandate = doc
        .investment_managers
        .iter_mut()
        .find(|m| m.manager.value == manager_ref)
        .ok_or_else(|| DocumentOpError::NotFound {
            item: "im_mandate",
            key: manager_ref,
        })?;

    if let Some(all) = scope_all {
        mandate.scope.all = all;
    }
    if let Some(mics) = scope_mics {
        mandate.scope.mics = mics;
    }
    if let Some(classes) = scope_instrument_classes {
        mandate.scope.instrument_classes = classes;
    }

    update_profile_document(pool, profile_id, &doc).await?;

    Ok(1)
}

/// Remove IM mandate
pub async fn remove_im_mandate(
    pool: &PgPool,
    profile_id: Uuid,
    manager_ref: String,
) -> Result<i32, DocumentOpError> {
    ensure_draft(pool, profile_id).await?;

    let mut doc = get_and_parse_profile(pool, profile_id).await?;

    let original_len = doc.investment_managers.len();
    doc.investment_managers
        .retain(|m| m.manager.value != manager_ref);

    if doc.investment_managers.len() == original_len {
        return Err(DocumentOpError::NotFound {
            item: "im_mandate",
            key: manager_ref,
        });
    }

    update_profile_document(pool, profile_id, &doc).await?;

    Ok(1)
}

// =============================================================================
// SYNC OPERATIONS (Phase 4)
// =============================================================================

/// Diff result comparing document to operational tables
#[derive(Debug, Clone, serde::Serialize)]
pub struct DiffResult {
    pub profile_id: Uuid,
    pub cbu_id: Uuid,
    /// Items in document but not in operational tables
    pub document_only: DiffSection,
    /// Items in operational tables but not in document
    pub operational_only: DiffSection,
    /// Items that differ between document and operational tables
    pub differences: DiffSection,
    /// Summary counts
    pub summary: DiffSummary,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct DiffSection {
    pub ssis: Vec<String>,
    pub booking_rules: Vec<String>,
    pub universe_entries: Vec<String>,
    pub isda_agreements: Vec<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct DiffSummary {
    pub document_only_count: i32,
    pub operational_only_count: i32,
    pub differences_count: i32,
    pub in_sync: bool,
}

/// Compare document with operational tables
pub async fn diff_document_vs_operational(
    pool: &PgPool,
    profile_id: Uuid,
) -> Result<DiffResult, DocumentOpError> {
    let cbu_id = get_profile_cbu_id(pool, profile_id).await?;
    let doc = get_and_parse_profile(pool, profile_id).await?;

    let mut document_only = DiffSection::default();
    let mut operational_only = DiffSection::default();
    let differences = DiffSection::default();

    // Compare SSIs
    let doc_ssi_names: std::collections::HashSet<String> = doc
        .standing_instructions
        .values()
        .flatten()
        .map(|s| s.name.clone())
        .collect();

    let op_ssis = sqlx::query!(
        r#"SELECT ssi_name FROM custody.cbu_ssi WHERE cbu_id = $1"#,
        cbu_id
    )
    .fetch_all(pool)
    .await?;

    let op_ssi_names: std::collections::HashSet<String> =
        op_ssis.iter().map(|r| r.ssi_name.clone()).collect();

    for name in &doc_ssi_names {
        if !op_ssi_names.contains(name) {
            document_only.ssis.push(name.clone());
        }
    }
    for name in &op_ssi_names {
        if !doc_ssi_names.contains(name) {
            operational_only.ssis.push(name.clone());
        }
    }

    // Compare booking rules
    let doc_rule_names: std::collections::HashSet<String> =
        doc.booking_rules.iter().map(|r| r.name.clone()).collect();

    let op_rules = sqlx::query!(
        r#"SELECT rule_name FROM custody.ssi_booking_rules WHERE cbu_id = $1"#,
        cbu_id
    )
    .fetch_all(pool)
    .await?;

    let op_rule_names: std::collections::HashSet<String> =
        op_rules.iter().map(|r| r.rule_name.clone()).collect();

    for name in &doc_rule_names {
        if !op_rule_names.contains(name) {
            document_only.booking_rules.push(name.clone());
        }
    }
    for name in &op_rule_names {
        if !doc_rule_names.contains(name) {
            operational_only.booking_rules.push(name.clone());
        }
    }

    // Compare universe entries (by market MIC)
    let doc_mics: std::collections::HashSet<String> = doc
        .universe
        .allowed_markets
        .iter()
        .map(|m| m.mic.clone())
        .collect();

    let op_universe = sqlx::query!(
        r#"SELECT m.mic
           FROM custody.cbu_instrument_universe u
           JOIN custody.markets m ON u.market_id = m.market_id
           WHERE u.cbu_id = $1"#,
        cbu_id
    )
    .fetch_all(pool)
    .await?;

    let op_mics: std::collections::HashSet<String> =
        op_universe.iter().map(|r| r.mic.clone()).collect();

    for mic in &doc_mics {
        if !op_mics.contains(mic) {
            document_only.universe_entries.push(mic.clone());
        }
    }
    for mic in &op_mics {
        if !doc_mics.contains(mic) {
            operational_only.universe_entries.push(mic.clone());
        }
    }

    // Calculate summary
    let document_only_count = document_only.ssis.len() as i32
        + document_only.booking_rules.len() as i32
        + document_only.universe_entries.len() as i32
        + document_only.isda_agreements.len() as i32;

    let operational_only_count = operational_only.ssis.len() as i32
        + operational_only.booking_rules.len() as i32
        + operational_only.universe_entries.len() as i32
        + operational_only.isda_agreements.len() as i32;

    let differences_count = differences.ssis.len() as i32
        + differences.booking_rules.len() as i32
        + differences.universe_entries.len() as i32
        + differences.isda_agreements.len() as i32;

    let in_sync = document_only_count == 0 && operational_only_count == 0 && differences_count == 0;

    Ok(DiffResult {
        profile_id,
        cbu_id,
        document_only,
        operational_only,
        differences,
        summary: DiffSummary {
            document_only_count,
            operational_only_count,
            differences_count,
            in_sync,
        },
    })
}

/// Sync result from operational tables to document
#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncFromOperationalResult {
    pub profile_id: Uuid,
    pub ssis_added: i32,
    pub booking_rules_added: i32,
    pub universe_entries_added: i32,
    pub total_synced: i32,
}

/// Sync from operational tables to document (reverse sync)
/// This reads operational tables and adds missing items to the document
pub async fn sync_from_operational(
    pool: &PgPool,
    profile_id: Uuid,
    sections: Vec<String>,
) -> Result<SyncFromOperationalResult, DocumentOpError> {
    ensure_draft(pool, profile_id).await?;

    let cbu_id = get_profile_cbu_id(pool, profile_id).await?;
    let mut doc = get_and_parse_profile(pool, profile_id).await?;

    let mut ssis_added = 0;
    let mut booking_rules_added = 0;
    let mut universe_entries_added = 0;

    // Sync SSIs from operational tables
    if sections.is_empty() || sections.contains(&"ssis".to_string()) {
        let op_ssis = sqlx::query!(
            r#"SELECT ssi_name, ssi_type, safekeeping_account, safekeeping_bic,
                      cash_account, cash_account_bic, cash_currency, pset_bic
               FROM custody.cbu_ssi
               WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_all(pool)
        .await?;

        let doc_ssi_names: std::collections::HashSet<String> = doc
            .standing_instructions
            .values()
            .flatten()
            .map(|s| s.name.clone())
            .collect();

        for op_ssi in op_ssis {
            if !doc_ssi_names.contains(&op_ssi.ssi_name) {
                let ssi = StandingInstruction {
                    name: op_ssi.ssi_name,
                    mic: None,
                    currency: op_ssi.cash_currency,
                    counterparty: None,
                    counterparty_lei: None,
                    custody_account: op_ssi.safekeeping_account,
                    custody_bic: op_ssi.safekeeping_bic,
                    cash_account: op_ssi.cash_account,
                    cash_bic: op_ssi.cash_account_bic,
                    settlement_model: None,
                    cutoff: None,
                    provider_ref: None,
                    channel: None,
                    reporting_frequency: None,
                };
                // Add to appropriate category based on type
                let category = op_ssi.ssi_type.clone();
                doc.standing_instructions
                    .entry(category)
                    .or_default()
                    .push(ssi);
                ssis_added += 1;
            }
        }
    }

    // Sync booking rules from operational tables
    if sections.is_empty() || sections.contains(&"booking_rules".to_string()) {
        let op_rules = sqlx::query!(
            r#"SELECT r.rule_name, r.priority, s.ssi_name,
                      ic.code as instrument_class,
                      m.mic as market,
                      r.currency, r.settlement_type
               FROM custody.ssi_booking_rules r
               JOIN custody.cbu_ssi s ON r.ssi_id = s.ssi_id
               LEFT JOIN custody.instrument_classes ic ON r.instrument_class_id = ic.class_id
               LEFT JOIN custody.markets m ON r.market_id = m.market_id
               WHERE r.cbu_id = $1"#,
            cbu_id
        )
        .fetch_all(pool)
        .await?;

        let doc_rule_names: std::collections::HashSet<String> =
            doc.booking_rules.iter().map(|r| r.name.clone()).collect();

        for op_rule in op_rules {
            if !doc_rule_names.contains(&op_rule.rule_name) {
                let rule = BookingRule {
                    name: op_rule.rule_name,
                    priority: op_rule.priority,
                    match_criteria: BookingMatch {
                        mic: Some(op_rule.market),
                        instrument_class: Some(op_rule.instrument_class),
                        currency: op_rule.currency,
                        settlement_type: op_rule.settlement_type,
                        counterparty: None,
                        security_type: None,
                    },
                    ssi_ref: op_rule.ssi_name,
                };
                doc.booking_rules.push(rule);
                booking_rules_added += 1;
            }
        }
    }

    // Sync universe entries from operational tables
    if sections.is_empty() || sections.contains(&"universe".to_string()) {
        let op_universe = sqlx::query!(
            r#"SELECT m.mic, u.currencies, u.settlement_types
               FROM custody.cbu_instrument_universe u
               JOIN custody.markets m ON u.market_id = m.market_id
               WHERE u.cbu_id = $1"#,
            cbu_id
        )
        .fetch_all(pool)
        .await?;

        let doc_mics: std::collections::HashSet<String> = doc
            .universe
            .allowed_markets
            .iter()
            .map(|m| m.mic.clone())
            .collect();

        for op_entry in op_universe {
            if !doc_mics.contains(&op_entry.mic) {
                let market = MarketConfig {
                    mic: op_entry.mic,
                    currencies: op_entry.currencies,
                    settlement_types: op_entry.settlement_types.unwrap_or_default(),
                };
                doc.universe.allowed_markets.push(market);
                universe_entries_added += 1;
            }
        }
    }

    // Save updated document
    if ssis_added > 0 || booking_rules_added > 0 || universe_entries_added > 0 {
        update_profile_document(pool, profile_id, &doc).await?;
    }

    let total_synced = ssis_added + booking_rules_added + universe_entries_added;

    Ok(SyncFromOperationalResult {
        profile_id,
        ssis_added,
        booking_rules_added,
        universe_entries_added,
        total_synced,
    })
}

// =============================================================================
// VALIDATION OPERATIONS (Phase 5)
// =============================================================================

/// Validation issue severity
#[derive(Debug, Clone, serde::Serialize)]
pub enum ValidationSeverity {
    Error,
    Warning,
    Info,
}

/// A single validation issue
#[derive(Debug, Clone, serde::Serialize)]
pub struct ValidationIssue {
    pub severity: ValidationSeverity,
    pub category: String,
    pub message: String,
    pub path: Option<String>,
}

/// Result of validate-coverage operation
#[derive(Debug, Clone, serde::Serialize)]
pub struct CoverageValidationResult {
    pub profile_id: Uuid,
    pub is_valid: bool,
    pub coverage_percentage: f64,
    pub issues: Vec<ValidationIssue>,
    pub uncovered_combinations: Vec<UncoveredCombination>,
}

/// An uncovered market/instrument/currency combination
#[derive(Debug, Clone, serde::Serialize)]
pub struct UncoveredCombination {
    pub mic: String,
    pub instrument_class: Option<String>,
    pub currency: String,
    pub settlement_type: String,
}

/// Result of validate-go-live-ready operation
#[derive(Debug, Clone, serde::Serialize)]
pub struct GoLiveValidationResult {
    pub profile_id: Uuid,
    pub is_ready: bool,
    pub issues: Vec<ValidationIssue>,
    pub checklist: GoLiveChecklist,
}

/// Go-live readiness checklist
#[derive(Debug, Clone, serde::Serialize)]
pub struct GoLiveChecklist {
    pub has_base_currency: bool,
    pub has_allowed_markets: bool,
    pub has_instrument_classes: bool,
    pub has_ssis: bool,
    pub has_booking_rules: bool,
    pub all_ssis_referenced: bool,
    pub all_markets_covered: bool,
    pub has_isda_if_otc: bool,
}

/// Validate that booking rules cover all universe combinations
pub async fn validate_coverage(
    pool: &PgPool,
    profile_id: Uuid,
) -> Result<CoverageValidationResult, DocumentOpError> {
    let doc = get_and_parse_profile(pool, profile_id).await?;

    let mut issues = Vec::new();
    let mut uncovered = Vec::new();
    let mut total_combinations = 0;
    let mut covered_combinations = 0;

    // Build all possible combinations from universe
    for market in &doc.universe.allowed_markets {
        for currency in &market.currencies {
            for settlement_type in &market.settlement_types {
                for instrument_class in &doc.universe.instrument_classes {
                    total_combinations += 1;

                    // Check if any booking rule matches this combination
                    let is_covered = doc.booking_rules.iter().any(|rule| {
                        let mic_matches = rule
                            .match_criteria
                            .mic
                            .as_ref()
                            .map(|m| m == &market.mic)
                            .unwrap_or(true); // None means wildcard
                        let currency_matches = rule
                            .match_criteria
                            .currency
                            .as_ref()
                            .map(|c| c == currency)
                            .unwrap_or(true);
                        let settlement_matches = rule
                            .match_criteria
                            .settlement_type
                            .as_ref()
                            .map(|s| s == settlement_type)
                            .unwrap_or(true);
                        let instrument_matches = rule
                            .match_criteria
                            .instrument_class
                            .as_ref()
                            .map(|i| i == &instrument_class.class_code)
                            .unwrap_or(true);

                        mic_matches && currency_matches && settlement_matches && instrument_matches
                    });

                    if is_covered {
                        covered_combinations += 1;
                    } else {
                        uncovered.push(UncoveredCombination {
                            mic: market.mic.clone(),
                            instrument_class: Some(instrument_class.class_code.clone()),
                            currency: currency.clone(),
                            settlement_type: settlement_type.clone(),
                        });
                    }
                }
            }
        }
    }

    let coverage_percentage = if total_combinations > 0 {
        (covered_combinations as f64 / total_combinations as f64) * 100.0
    } else {
        100.0 // No combinations = fully covered (vacuously true)
    };

    if !uncovered.is_empty() {
        issues.push(ValidationIssue {
            severity: ValidationSeverity::Error,
            category: "coverage".to_string(),
            message: format!(
                "{} of {} combinations are not covered by booking rules",
                uncovered.len(),
                total_combinations
            ),
            path: Some("booking_rules".to_string()),
        });
    }

    let is_valid = uncovered.is_empty();

    Ok(CoverageValidationResult {
        profile_id,
        is_valid,
        coverage_percentage,
        issues,
        uncovered_combinations: uncovered,
    })
}

/// Validate that a profile is ready for go-live
pub async fn validate_go_live_ready(
    pool: &PgPool,
    profile_id: Uuid,
) -> Result<GoLiveValidationResult, DocumentOpError> {
    let doc = get_and_parse_profile(pool, profile_id).await?;

    let mut issues = Vec::new();

    // Check base currency
    let has_base_currency = !doc.universe.base_currency.is_empty();
    if !has_base_currency {
        issues.push(ValidationIssue {
            severity: ValidationSeverity::Error,
            category: "universe".to_string(),
            message: "Base currency is not set".to_string(),
            path: Some("universe.base_currency".to_string()),
        });
    }

    // Check allowed markets
    let has_allowed_markets = !doc.universe.allowed_markets.is_empty();
    if !has_allowed_markets {
        issues.push(ValidationIssue {
            severity: ValidationSeverity::Error,
            category: "universe".to_string(),
            message: "No allowed markets defined".to_string(),
            path: Some("universe.allowed_markets".to_string()),
        });
    }

    // Check instrument classes
    let has_instrument_classes = !doc.universe.instrument_classes.is_empty();
    if !has_instrument_classes {
        issues.push(ValidationIssue {
            severity: ValidationSeverity::Error,
            category: "universe".to_string(),
            message: "No instrument classes defined".to_string(),
            path: Some("universe.instrument_classes".to_string()),
        });
    }

    // Check SSIs exist
    let all_ssis: Vec<&StandingInstruction> =
        doc.standing_instructions.values().flatten().collect();
    let has_ssis = !all_ssis.is_empty();
    if !has_ssis {
        issues.push(ValidationIssue {
            severity: ValidationSeverity::Error,
            category: "ssi".to_string(),
            message: "No standing settlement instructions defined".to_string(),
            path: Some("standing_instructions".to_string()),
        });
    }

    // Check booking rules exist
    let has_booking_rules = !doc.booking_rules.is_empty();
    if !has_booking_rules {
        issues.push(ValidationIssue {
            severity: ValidationSeverity::Error,
            category: "booking".to_string(),
            message: "No booking rules defined".to_string(),
            path: Some("booking_rules".to_string()),
        });
    }

    // Check that all SSI references in booking rules are valid
    let ssi_names: std::collections::HashSet<String> =
        all_ssis.iter().map(|s| s.name.clone()).collect();
    let mut all_ssis_referenced = true;
    for rule in &doc.booking_rules {
        if !ssi_names.contains(&rule.ssi_ref) {
            all_ssis_referenced = false;
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Error,
                category: "booking".to_string(),
                message: format!(
                    "Booking rule '{}' references unknown SSI '{}'",
                    rule.name, rule.ssi_ref
                ),
                path: Some(format!("booking_rules[{}].ssi_ref", rule.name)),
            });
        }
    }

    // Check coverage
    let coverage_result = validate_coverage(pool, profile_id).await?;
    let all_markets_covered = coverage_result.is_valid;
    if !all_markets_covered {
        issues.extend(coverage_result.issues);
    }

    // Check ISDA if OTC instruments are present
    let has_otc =
        doc.universe.instrument_classes.iter().any(|ic| {
            ic.class_code.starts_with("OTC") || ic.isda_asset_classes.iter().any(|_| true)
        });
    let has_isda = !doc.isda_agreements.is_empty();
    let has_isda_if_otc = !has_otc || has_isda;
    if has_otc && !has_isda {
        issues.push(ValidationIssue {
            severity: ValidationSeverity::Warning,
            category: "isda".to_string(),
            message: "OTC instruments defined but no ISDA agreements configured".to_string(),
            path: Some("isda_agreements".to_string()),
        });
    }

    let checklist = GoLiveChecklist {
        has_base_currency,
        has_allowed_markets,
        has_instrument_classes,
        has_ssis,
        has_booking_rules,
        all_ssis_referenced,
        all_markets_covered,
        has_isda_if_otc,
    };

    // Profile is ready only if no errors (warnings are ok)
    let is_ready = issues
        .iter()
        .all(|i| !matches!(i.severity, ValidationSeverity::Error));

    Ok(GoLiveValidationResult {
        profile_id,
        is_ready,
        issues,
        checklist,
    })
}

// =============================================================================
// PHASE 6: Document Lifecycle Operations
// =============================================================================

/// Result of a lifecycle transition
#[derive(Debug, Clone, serde::Serialize)]
pub struct LifecycleTransitionResult {
    pub profile_id: Uuid,
    pub previous_status: String,
    pub new_status: String,
    pub transitioned_at: chrono::DateTime<chrono::Utc>,
    pub transitioned_by: Option<String>,
    pub notes: Option<String>,
}

/// Result of a rejection
#[derive(Debug, Clone, serde::Serialize)]
pub struct RejectionResult {
    pub profile_id: Uuid,
    pub previous_status: String,
    pub new_status: String,
    pub rejection_reason: String,
    pub rejected_at: chrono::DateTime<chrono::Utc>,
    pub rejected_by: Option<String>,
}

/// Result of creating a new version
#[derive(Debug, Clone, serde::Serialize)]
pub struct NewVersionResult {
    pub new_profile_id: Uuid,
    pub source_profile_id: Uuid,
    pub cbu_id: Uuid,
    pub new_version: i32,
    pub source_version: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub created_by: Option<String>,
}

/// Create a new draft version from the current ACTIVE profile
/// Used when modifications are needed to a live trading matrix
pub async fn create_new_version(
    pool: &PgPool,
    cbu_id: Uuid,
    created_by: Option<String>,
    notes: Option<String>,
) -> Result<NewVersionResult, DocumentOpError> {
    // Check for existing working version (DRAFT/VALIDATED/PENDING_REVIEW)
    let existing_working = sqlx::query_scalar!(
        r#"
        SELECT profile_id FROM "ob-poc".cbu_trading_profiles
        WHERE cbu_id = $1 AND status IN ('DRAFT', 'VALIDATED', 'PENDING_REVIEW')
        "#,
        cbu_id
    )
    .fetch_optional(pool)
    .await
    .map_err(DocumentOpError::Database)?;

    if existing_working.is_some() {
        return Err(DocumentOpError::InvalidReference(
            "Cannot create new version: a working version (DRAFT/VALIDATED/PENDING_REVIEW) already exists. Complete or archive it first.".to_string()
        ));
    }

    // Get the current ACTIVE profile
    let active_row = sqlx::query!(
        r#"
        SELECT profile_id, version, document, document_hash
        FROM "ob-poc".cbu_trading_profiles
        WHERE cbu_id = $1 AND status = 'ACTIVE'
        "#,
        cbu_id
    )
    .fetch_optional(pool)
    .await
    .map_err(DocumentOpError::Database)?
    .ok_or_else(|| DocumentOpError::InvalidReference(
        "Cannot create new version: no ACTIVE profile exists for this CBU. Use create-draft instead.".to_string()
    ))?;

    let source_profile_id = active_row.profile_id;
    let source_version = active_row.version;
    let new_version = source_version + 1;
    let document = active_row.document;
    let document_hash = active_row.document_hash;

    // Create new profile row with incremented version
    let now = chrono::Utc::now();
    let new_profile_id = sqlx::query_scalar!(
        r#"
        INSERT INTO "ob-poc".cbu_trading_profiles
        (cbu_id, version, status, document, document_hash, created_by, created_at, notes)
        VALUES ($1, $2, 'DRAFT', $3, $4, $5, $6, $7)
        RETURNING profile_id
        "#,
        cbu_id,
        new_version,
        document,
        document_hash,
        created_by,
        now,
        notes
    )
    .fetch_one(pool)
    .await
    .map_err(DocumentOpError::Database)?;

    Ok(NewVersionResult {
        new_profile_id,
        source_profile_id,
        cbu_id,
        new_version,
        source_version,
        created_at: now,
        created_by,
    })
}

/// Validate a draft profile (ops team cleanup complete)
/// Transitions: DRAFT → VALIDATED
/// Runs structural validation to ensure the profile is ready for client review
pub async fn validate_profile(
    pool: &PgPool,
    profile_id: Uuid,
    validated_by: Option<String>,
    notes: Option<String>,
) -> Result<LifecycleTransitionResult, DocumentOpError> {
    // Get current profile status
    let row = sqlx::query!(
        r#"
        SELECT status, cbu_id
        FROM "ob-poc".cbu_trading_profiles
        WHERE profile_id = $1
        "#,
        profile_id
    )
    .fetch_optional(pool)
    .await
    .map_err(DocumentOpError::Database)?
    .ok_or_else(|| DocumentOpError::ProfileNotFound(profile_id))?;

    let current_status = row.status.clone();

    // Must be in DRAFT status
    if current_status != "DRAFT" {
        return Err(DocumentOpError::InvalidReference(format!(
            "Cannot validate profile: expected status DRAFT, found {}",
            current_status
        )));
    }

    // Run validation to ensure profile structure is sound
    let validation_result = validate_go_live_ready(pool, profile_id).await?;

    // Collect errors and warnings
    let errors: Vec<String> = validation_result
        .issues
        .iter()
        .filter(|i| matches!(i.severity, ValidationSeverity::Error))
        .map(|i| i.message.clone())
        .collect();

    let _warnings: Vec<String> = validation_result
        .issues
        .iter()
        .filter(|i| matches!(i.severity, ValidationSeverity::Warning))
        .map(|i| i.message.clone())
        .collect();

    // Block transition if there are errors
    if !errors.is_empty() {
        return Err(DocumentOpError::InvalidReference(format!(
            "Cannot validate profile: {} error(s) found: {}",
            errors.len(),
            errors.join("; ")
        )));
    }

    // Update status to VALIDATED
    let now = chrono::Utc::now();
    sqlx::query!(
        r#"
        UPDATE "ob-poc".cbu_trading_profiles
        SET status = $1, notes = COALESCE($2, notes)
        WHERE profile_id = $3
        "#,
        "VALIDATED",
        notes,
        profile_id
    )
    .execute(pool)
    .await
    .map_err(DocumentOpError::Database)?;

    Ok(LifecycleTransitionResult {
        profile_id,
        previous_status: current_status,
        new_status: "VALIDATED".to_string(),
        transitioned_at: now,
        transitioned_by: validated_by,
        notes,
    })
}

/// Submit a validated profile for client review
/// Transitions: VALIDATED → PENDING_REVIEW
pub async fn submit_for_review(
    pool: &PgPool,
    profile_id: Uuid,
    submitted_by: Option<String>,
    notes: Option<String>,
) -> Result<LifecycleTransitionResult, DocumentOpError> {
    // First, get current profile status
    let row = sqlx::query!(
        r#"
        SELECT status, cbu_id
        FROM "ob-poc".cbu_trading_profiles
        WHERE profile_id = $1
        "#,
        profile_id
    )
    .fetch_optional(pool)
    .await
    .map_err(DocumentOpError::Database)?
    .ok_or_else(|| DocumentOpError::ProfileNotFound(profile_id))?;

    let current_status = row.status.clone();

    // Must be in VALIDATED status (ops team has already validated)
    if current_status != "VALIDATED" {
        return Err(DocumentOpError::InvalidReference(format!(
            "Cannot submit profile for review: expected status VALIDATED, found {}. Use trading-profile.validate first.",
            current_status
        )));
    }

    // Update status to PENDING_REVIEW with timestamp
    let now = chrono::Utc::now();
    sqlx::query!(
        r#"
        UPDATE "ob-poc".cbu_trading_profiles
        SET status = $1, submitted_at = $2, submitted_by = $3, notes = COALESCE($4, notes)
        WHERE profile_id = $5
        "#,
        "PENDING_REVIEW",
        now,
        submitted_by,
        notes,
        profile_id
    )
    .execute(pool)
    .await
    .map_err(DocumentOpError::Database)?;

    Ok(LifecycleTransitionResult {
        profile_id,
        previous_status: current_status,
        new_status: "PENDING_REVIEW".to_string(),
        transitioned_at: now,
        transitioned_by: submitted_by,
        notes,
    })
}

/// Approve a profile pending review
/// Transitions: PENDING_REVIEW → ACTIVE
/// Also supersedes any currently active profile for the same CBU
pub async fn approve_profile(
    pool: &PgPool,
    profile_id: Uuid,
    approved_by: Option<String>,
    notes: Option<String>,
) -> Result<LifecycleTransitionResult, DocumentOpError> {
    // Get current profile status and CBU
    let row = sqlx::query!(
        r#"
        SELECT status, cbu_id
        FROM "ob-poc".cbu_trading_profiles
        WHERE profile_id = $1
        "#,
        profile_id
    )
    .fetch_optional(pool)
    .await
    .map_err(DocumentOpError::Database)?
    .ok_or_else(|| DocumentOpError::ProfileNotFound(profile_id))?;

    let current_status = row.status.clone();
    let cbu_id = row.cbu_id;

    // Validate current status is PENDING_REVIEW
    if current_status != "PENDING_REVIEW" {
        return Err(DocumentOpError::InvalidReference(format!(
            "Cannot approve profile: current status is {}, expected PENDING_REVIEW",
            current_status
        )));
    }

    let now = chrono::Utc::now();

    // Get the version number of the new profile
    let new_version: i32 = sqlx::query_scalar!(
        r#"SELECT version FROM "ob-poc".cbu_trading_profiles WHERE profile_id = $1"#,
        profile_id
    )
    .fetch_one(pool)
    .await
    .map_err(DocumentOpError::Database)?;

    // Supersede any currently active profile for this CBU
    sqlx::query!(
        r#"
        UPDATE "ob-poc".cbu_trading_profiles
        SET status = $1, superseded_at = $2, superseded_by_version = $3
        WHERE cbu_id = $4
          AND status = 'ACTIVE'
          AND profile_id != $5
        "#,
        "SUPERSEDED",
        now,
        new_version,
        cbu_id,
        profile_id
    )
    .execute(pool)
    .await
    .map_err(DocumentOpError::Database)?;

    // Update this profile to ACTIVE
    sqlx::query!(
        r#"
        UPDATE "ob-poc".cbu_trading_profiles
        SET status = $1,
            activated_at = $2,
            activated_by = $3
        WHERE profile_id = $4
        "#,
        "ACTIVE",
        now,
        approved_by,
        profile_id
    )
    .execute(pool)
    .await
    .map_err(DocumentOpError::Database)?;

    Ok(LifecycleTransitionResult {
        profile_id,
        previous_status: current_status,
        new_status: "ACTIVE".to_string(),
        transitioned_at: now,
        transitioned_by: approved_by,
        notes,
    })
}

/// Reject a profile pending review
/// Transitions: PENDING_REVIEW → DRAFT
pub async fn reject_profile(
    pool: &PgPool,
    profile_id: Uuid,
    rejection_reason: String,
    rejected_by: Option<String>,
) -> Result<RejectionResult, DocumentOpError> {
    // Get current profile status
    let row = sqlx::query!(
        r#"
        SELECT status, cbu_id
        FROM "ob-poc".cbu_trading_profiles
        WHERE profile_id = $1
        "#,
        profile_id
    )
    .fetch_optional(pool)
    .await
    .map_err(DocumentOpError::Database)?
    .ok_or_else(|| DocumentOpError::ProfileNotFound(profile_id))?;

    let current_status = row.status.clone();

    // Validate current status is PENDING_REVIEW
    if current_status != "PENDING_REVIEW" {
        return Err(DocumentOpError::InvalidReference(format!(
            "Cannot reject profile: current status is {}, expected PENDING_REVIEW",
            current_status
        )));
    }

    let now = chrono::Utc::now();

    // Update status back to DRAFT with rejection info
    sqlx::query!(
        r#"
        UPDATE "ob-poc".cbu_trading_profiles
        SET status = $1, rejected_at = $2, rejected_by = $3, rejection_reason = $4,
            validated_at = NULL, validated_by = NULL,
            submitted_at = NULL, submitted_by = NULL
        WHERE profile_id = $5
        "#,
        "DRAFT",
        now,
        rejected_by,
        rejection_reason,
        profile_id
    )
    .execute(pool)
    .await
    .map_err(DocumentOpError::Database)?;

    Ok(RejectionResult {
        profile_id,
        previous_status: current_status,
        new_status: "DRAFT".to_string(),
        rejection_reason,
        rejected_at: now,
        rejected_by,
    })
}

// =============================================================================
// CLONE OPERATIONS
// =============================================================================

/// Result of cloning a profile to another CBU
#[derive(Debug, Clone, serde::Serialize)]
pub struct CloneResult {
    pub source_profile_id: Uuid,
    pub source_cbu_id: Uuid,
    pub target_profile_id: Uuid,
    pub target_cbu_id: Uuid,
    pub target_version: i32,
    pub cloned_at: chrono::DateTime<chrono::Utc>,
    pub cloned_by: Option<String>,
    pub sections_cloned: ClonedSections,
}

/// Summary of what was cloned
#[derive(Debug, Clone, serde::Serialize)]
pub struct ClonedSections {
    pub universe: bool,
    pub investment_managers: bool,
    pub isda_agreements: bool,
    pub booking_rules: bool,
    pub standing_instructions: bool,
    pub pricing_matrix: bool,
    pub valuation_config: bool,
    pub constraints: bool,
    pub settlement_config: bool,
}

/// Clone a trading profile from one CBU to another
///
/// Creates a new DRAFT profile for the target CBU with the document content
/// from the source profile. The target profile can then be customized before
/// activation.
///
/// Use cases:
/// - Setting up a new fund with similar trading configuration
/// - Creating templates from an existing production profile
/// - Migrating trading config when restructuring fund families
pub async fn clone_to_cbu(
    pool: &PgPool,
    source_profile_id: Uuid,
    target_cbu_id: Uuid,
    cloned_by: Option<String>,
    adapt_base_currency: Option<String>,
    include_isda: bool,
    notes: Option<String>,
) -> Result<CloneResult, DocumentOpError> {
    // Get source profile
    let source_row = sqlx::query!(
        r#"SELECT cbu_id, document FROM "ob-poc".cbu_trading_profiles WHERE profile_id = $1"#,
        source_profile_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or(DocumentOpError::ProfileNotFound(source_profile_id))?;

    let source_cbu_id = source_row.cbu_id;
    let mut doc: TradingProfileDocument = serde_json::from_value(source_row.document.clone())?;

    // Prevent cloning to the same CBU (use create-draft with copy_from_profile instead)
    if source_cbu_id == target_cbu_id {
        return Err(DocumentOpError::InvalidReference(
            "Cannot clone to the same CBU. Use create-draft with copy_from_profile instead."
                .to_string(),
        ));
    }

    // IDEMPOTENCY: Check if target CBU already has a working version (DRAFT/VALIDATED/PENDING_REVIEW)
    // If so, return the existing profile to avoid constraint violation
    let existing_working = sqlx::query!(
        r#"SELECT profile_id, version, status, created_at, created_by, document
           FROM "ob-poc".cbu_trading_profiles
           WHERE cbu_id = $1
           AND status IN ('DRAFT', 'VALIDATED', 'PENDING_REVIEW')
           LIMIT 1"#,
        target_cbu_id
    )
    .fetch_optional(pool)
    .await?;

    if let Some(existing_row) = existing_working {
        // Already has a working version - return it (idempotent)
        let existing_doc: TradingProfileDocument = serde_json::from_value(existing_row.document)?;
        return Ok(CloneResult {
            source_profile_id,
            source_cbu_id,
            target_profile_id: existing_row.profile_id,
            target_cbu_id,
            target_version: existing_row.version,
            cloned_at: existing_row.created_at,
            cloned_by: existing_row.created_by,
            sections_cloned: ClonedSections {
                universe: true,
                investment_managers: !existing_doc.investment_managers.is_empty(),
                isda_agreements: !existing_doc.isda_agreements.is_empty(),
                booking_rules: !existing_doc.booking_rules.is_empty(),
                standing_instructions: !existing_doc.standing_instructions.is_empty(),
                pricing_matrix: !existing_doc.pricing_matrix.is_empty(),
                valuation_config: existing_doc.valuation_config.is_some(),
                constraints: existing_doc.constraints.is_some(),
                settlement_config: existing_doc.settlement_config.is_some(),
            },
        });
    }

    // Adapt base currency if specified
    if let Some(currency) = adapt_base_currency {
        doc.universe.base_currency = currency;
    }

    // Optionally exclude ISDA agreements (they are counterparty-specific)
    let isda_cloned = if include_isda {
        !doc.isda_agreements.is_empty()
    } else {
        doc.isda_agreements.clear();
        false
    };

    // Update metadata to reflect clone source
    let clone_note = format!(
        "Cloned from profile {} (CBU {})",
        source_profile_id, source_cbu_id
    );
    if let Some(ref mut metadata) = doc.metadata {
        metadata.source = Some("CLONE".to_string());
        metadata.source_ref = Some(source_profile_id.to_string());
        metadata.notes = Some(match &notes {
            Some(n) => format!("{}. {}", clone_note, n),
            None => clone_note,
        });
    } else {
        doc.metadata = Some(ProfileMetadata {
            source: Some("CLONE".to_string()),
            source_ref: Some(source_profile_id.to_string()),
            created_by: cloned_by.clone(),
            notes: Some(match &notes {
                Some(n) => format!("{}. {}", clone_note, n),
                None => clone_note,
            }),
            regulatory_framework: None,
        });
    }

    // Get next version number for target CBU
    let version = sqlx::query_scalar!(
        r#"SELECT COALESCE(MAX(version), 0) + 1 as "version!"
           FROM "ob-poc".cbu_trading_profiles
           WHERE cbu_id = $1"#,
        target_cbu_id
    )
    .fetch_one(pool)
    .await?;

    let target_profile_id = Uuid::new_v4();
    let doc_json = serde_json::to_value(&doc)?;
    let hash = compute_document_hash(&doc);
    let now = chrono::Utc::now();

    sqlx::query!(
        r#"INSERT INTO "ob-poc".cbu_trading_profiles
           (profile_id, cbu_id, version, status, document, document_hash, created_at, created_by)
           VALUES ($1, $2, $3, 'DRAFT', $4, $5, $6, $7)"#,
        target_profile_id,
        target_cbu_id,
        version as i32,
        doc_json,
        hash,
        now,
        cloned_by.as_deref()
    )
    .execute(pool)
    .await?;

    Ok(CloneResult {
        source_profile_id,
        source_cbu_id,
        target_profile_id,
        target_cbu_id,
        target_version: version as i32,
        cloned_at: now,
        cloned_by,
        sections_cloned: ClonedSections {
            universe: true,
            investment_managers: !doc.investment_managers.is_empty(),
            isda_agreements: isda_cloned,
            booking_rules: !doc.booking_rules.is_empty(),
            standing_instructions: !doc.standing_instructions.is_empty(),
            pricing_matrix: !doc.pricing_matrix.is_empty(),
            valuation_config: doc.valuation_config.is_some(),
            constraints: doc.constraints.is_some(),
            settlement_config: doc.settlement_config.is_some(),
        },
    })
}

/// Archive an active or superseded profile
/// Transitions: ACTIVE|SUPERSEDED → ARCHIVED
pub async fn archive_profile(
    pool: &PgPool,
    profile_id: Uuid,
    archived_by: Option<String>,
    notes: Option<String>,
) -> Result<LifecycleTransitionResult, DocumentOpError> {
    // Get current profile status
    let row = sqlx::query!(
        r#"
        SELECT status, cbu_id
        FROM "ob-poc".cbu_trading_profiles
        WHERE profile_id = $1
        "#,
        profile_id
    )
    .fetch_optional(pool)
    .await
    .map_err(DocumentOpError::Database)?
    .ok_or_else(|| DocumentOpError::ProfileNotFound(profile_id))?;

    let current_status = row.status.clone();

    // Validate current status is ACTIVE or SUPERSEDED
    if current_status != "ACTIVE" && current_status != "SUPERSEDED" {
        return Err(DocumentOpError::InvalidReference(format!(
            "Cannot archive profile: current status is {}, expected ACTIVE or SUPERSEDED",
            current_status
        )));
    }

    let now = chrono::Utc::now();

    // Update status to ARCHIVED
    sqlx::query!(
        r#"
        UPDATE "ob-poc".cbu_trading_profiles
        SET status = $1
        WHERE profile_id = $2
        "#,
        "ARCHIVED",
        profile_id
    )
    .execute(pool)
    .await
    .map_err(DocumentOpError::Database)?;

    Ok(LifecycleTransitionResult {
        profile_id,
        previous_status: current_status,
        new_status: "ARCHIVED".to_string(),
        transitioned_at: now,
        transitioned_by: archived_by,
        notes,
    })
}
