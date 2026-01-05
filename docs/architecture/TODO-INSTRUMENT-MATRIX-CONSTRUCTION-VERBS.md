# TODO: Instrument Matrix Construction Verbs

## Executive Summary

The current DSL has verbs for **importing** complete trading profile documents, but lacks verbs for **constructing** them incrementally. This is a critical gap for agentic onboarding.

**Current state**: Document-First (migration pattern)
```
Human creates YAML file → import → materialize → operational tables
```

**Required state**: Construction-First (agentic pattern)
```
DSL verbs build components → system assembles document → version/activate → materialize
```

Without construction verbs, the LLM/agent cannot help build instrument matrices - it can only import pre-built documents.

---

## The Core Problem

### Two Data Models, One-Way Bridge

```
┌─────────────────────────────────────────────────────────────────────┐
│                    DOCUMENT MODEL                                   │
│              "ob-poc".cbu_trading_profiles.document                 │
│                        (JSONB blob)                                 │
│                                                                     │
│   TradingProfileDocument {                                          │
│       universe: { allowed_markets, instrument_classes }             │
│       investment_managers: [...]                                    │
│       isda_agreements: [...]                                        │
│       settlement_config: {...}                                      │
│       booking_rules: [...]                                          │
│       standing_instructions: {...}                                  │
│   }                                                                 │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                           │ trading-profile.materialize
                           │ (ONE WAY - no reverse)
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    OPERATIONAL MODEL                                │
│                     (custody.* tables)                              │
│                                                                     │
│   cbu_instrument_universe    ← cbu-custody.add-universe             │
│   cbu_ssi                    ← cbu-custody.create-ssi               │
│   ssi_booking_rules          ← cbu-custody.create-booking-rule      │
│   isda_agreements            ← isda.create                          │
│   csa_agreements             ← isda.add-csa                         │
│   cbu_im_assignments         ← investment-manager.assign            │
└─────────────────────────────────────────────────────────────────────┘
```

**Problem**: 
- Operational verbs write to custody.* tables directly
- Document verbs only import/export complete documents
- No verbs to incrementally BUILD the document
- No verbs to SYNC operational changes back to document

### Why This Matters for Agentic Onboarding

Agent conversation:
```
User: "Allianz Lux needs to trade US equities on NYSE"

Agent: I'll add that to their instrument matrix.
       (cbu-custody.add-universe :cbu-id @allianz :instrument-class EQUITY :market XNYS :currencies [USD])
       
       ✅ Added to operational tables
       ❌ Trading profile document NOT updated
       ❌ Document shows stale data
       ❌ export-full-matrix returns wrong info
       ❌ No audit trail at document level
```

The agent CAN add to operational tables but CANNOT update the source-of-truth document.

---

## Missing Verb Categories

### Category 1: Document Construction Verbs

These verbs should modify `cbu_trading_profiles.document` JSONB directly.

#### 1.1 Universe Section

```yaml
# trading-profile.yaml additions

add-instrument-class:
  description: Add instrument class to trading profile universe
  behavior: plugin
  handler: add_instrument_class_to_profile
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: class-code
      type: string
      required: true
      description: "Instrument class code (EQUITY, FIXED_INCOME, IRS, etc.)"
    - name: cfi-prefixes
      type: string_list
      required: false
      description: "CFI code prefixes for matching"
    - name: isda-asset-classes
      type: string_list
      required: false
      description: "ISDA taxonomy codes if OTC"
    - name: is-held
      type: boolean
      required: false
      default: true
    - name: is-traded
      type: boolean
      required: false
      default: true
  returns:
    type: record
    description: "Updated universe section"

remove-instrument-class:
  description: Remove instrument class from trading profile universe
  behavior: plugin
  handler: remove_instrument_class_from_profile
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: class-code
      type: string
      required: true
  returns:
    type: affected

add-market:
  description: Add market to trading profile universe
  behavior: plugin
  handler: add_market_to_profile
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: mic
      type: string
      required: true
      description: "Market Identifier Code (ISO 10383)"
    - name: currencies
      type: string_list
      required: true
      description: "Currencies traded on this market"
    - name: settlement-types
      type: string_list
      required: false
      default: [DVP]
  returns:
    type: record

remove-market:
  description: Remove market from trading profile universe
  behavior: plugin
  handler: remove_market_from_profile
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: mic
      type: string
      required: true
  returns:
    type: affected

set-base-currency:
  description: Set base currency for trading profile
  behavior: plugin
  handler: set_profile_base_currency
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: currency
      type: string
      required: true
  returns:
    type: affected

add-allowed-currency:
  description: Add currency to allowed currencies list
  behavior: plugin
  handler: add_allowed_currency
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: currency
      type: string
      required: true
  returns:
    type: affected
```

#### 1.2 Investment Manager Section

```yaml
add-im-mandate:
  description: Add investment manager mandate to trading profile
  behavior: plugin
  handler: add_im_mandate_to_profile
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: manager-ref
      type: string
      required: true
      description: "Manager reference (LEI, BIC, or name)"
    - name: manager-ref-type
      type: string
      required: true
      valid_values: [LEI, BIC, NAME, UUID]
    - name: priority
      type: integer
      required: true
    - name: role
      type: string
      required: false
      default: INVESTMENT_MANAGER
    - name: scope-all
      type: boolean
      required: false
      default: false
    - name: scope-mics
      type: string_list
      required: false
      description: "Markets this IM can trade (empty = all if scope-all)"
    - name: scope-instrument-classes
      type: string_list
      required: false
    - name: instruction-method
      type: string
      required: true
      valid_values: [SWIFT, CTM, FIX, API, ALERT, MANUAL]
    - name: can-trade
      type: boolean
      required: false
      default: true
    - name: can-settle
      type: boolean
      required: false
      default: true
  returns:
    type: record

update-im-scope:
  description: Update scope for existing IM mandate in profile
  behavior: plugin
  handler: update_im_scope_in_profile
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: manager-ref
      type: string
      required: true
    - name: scope-all
      type: boolean
      required: false
    - name: scope-mics
      type: string_list
      required: false
    - name: scope-instrument-classes
      type: string_list
      required: false
  returns:
    type: affected

remove-im-mandate:
  description: Remove investment manager mandate from profile
  behavior: plugin
  handler: remove_im_mandate_from_profile
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: manager-ref
      type: string
      required: true
  returns:
    type: affected
```

#### 1.3 ISDA/CSA Section

```yaml
add-isda-config:
  description: Add ISDA agreement configuration to trading profile
  behavior: plugin
  handler: add_isda_config_to_profile
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: counterparty-ref
      type: string
      required: true
      description: "Counterparty LEI or name"
    - name: counterparty-ref-type
      type: string
      required: true
      valid_values: [LEI, BIC, NAME, UUID]
    - name: agreement-date
      type: date
      required: true
    - name: governing-law
      type: string
      required: true
      valid_values: [NY, ENGLISH]
    - name: effective-date
      type: date
      required: false
  returns:
    type: record

add-isda-product-coverage:
  description: Add product coverage to ISDA config in profile
  behavior: plugin
  handler: add_isda_coverage_to_profile
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: counterparty-ref
      type: string
      required: true
      description: "Identifies which ISDA in the profile"
    - name: asset-class
      type: string
      required: true
      description: "ISDA asset class (RATES, CREDIT, FX, EQUITY, COMMODITY)"
    - name: base-products
      type: string_list
      required: false
      description: "Specific products (IRS, CDS, FX_FORWARD, etc.)"
  returns:
    type: affected

add-csa-config:
  description: Add CSA configuration to ISDA in trading profile
  behavior: plugin
  handler: add_csa_config_to_profile
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: counterparty-ref
      type: string
      required: true
      description: "Identifies which ISDA"
    - name: csa-type
      type: string
      required: true
      valid_values: [VM, VM_IM]
    - name: threshold-amount
      type: decimal
      required: false
    - name: threshold-currency
      type: string
      required: false
    - name: mta
      type: decimal
      required: false
      description: "Minimum Transfer Amount"
    - name: rounding
      type: decimal
      required: false
    - name: valuation-time
      type: string
      required: false
      description: "e.g., 16:00"
    - name: valuation-timezone
      type: string
      required: false
      description: "e.g., Europe/London"
    - name: settlement-days
      type: integer
      required: false
      default: 1
  returns:
    type: record

add-csa-eligible-collateral:
  description: Add eligible collateral to CSA in trading profile
  behavior: plugin
  handler: add_csa_collateral_to_profile
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: counterparty-ref
      type: string
      required: true
    - name: csa-type
      type: string
      required: true
      valid_values: [VM, VM_IM]
    - name: collateral-type
      type: string
      required: true
      valid_values: [CASH, GOVT_BOND, CORP_BOND, EQUITY, MONEY_MARKET]
    - name: currencies
      type: string_list
      required: false
    - name: issuers
      type: string_list
      required: false
      description: "Allowed issuer countries or entities"
    - name: min-rating
      type: string
      required: false
      description: "Minimum credit rating (e.g., A-)"
    - name: haircut-pct
      type: decimal
      required: true
      description: "Haircut percentage (e.g., 2.0 for 2%)"
  returns:
    type: affected

add-csa-initial-margin:
  description: Configure Initial Margin requirements on CSA
  behavior: plugin
  handler: add_csa_im_config_to_profile
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: counterparty-ref
      type: string
      required: true
    - name: calculation-method
      type: string
      required: true
      valid_values: [SIMM, GRID, SCHEDULE]
    - name: posting-frequency
      type: string
      required: false
      valid_values: [DAILY, WEEKLY]
    - name: segregation-required
      type: boolean
      required: true
      default: true
    - name: custodian-ref
      type: string
      required: false
      description: "Custodian for segregated IM (LEI/BIC)"
  returns:
    type: affected

link-csa-ssi:
  description: Link CSA to collateral SSI in standing instructions
  behavior: plugin
  handler: link_csa_ssi_in_profile
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: counterparty-ref
      type: string
      required: true
    - name: csa-type
      type: string
      required: true
    - name: ssi-name
      type: string
      required: true
      description: "SSI name from standing_instructions.OTC_COLLATERAL section"
  returns:
    type: affected
```

#### 1.4 Settlement Config Section

```yaml
add-subcustodian:
  description: Add subcustodian to settlement config in profile
  behavior: plugin
  handler: add_subcustodian_to_profile
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: mic
      type: string
      required: true
      description: "Market this subcustodian serves"
    - name: currency
      type: string
      required: true
    - name: subcustodian-bic
      type: string
      required: true
    - name: subcustodian-name
      type: string
      required: false
    - name: local-agent-account
      type: string
      required: false
    - name: place-of-settlement
      type: string
      required: false
    - name: is-primary
      type: boolean
      required: false
      default: true
  returns:
    type: affected

add-matching-platform:
  description: Add matching platform config to profile
  behavior: plugin
  handler: add_matching_platform_to_profile
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: platform
      type: string
      required: true
      valid_values: [CTM, ALERT, TLM]
    - name: participant-id
      type: string
      required: true
    - name: enabled-mics
      type: string_list
      required: true
      description: "Markets where this platform is enabled"
    - name: auto-match
      type: boolean
      required: false
      default: true
    - name: auto-affirm-threshold-usd
      type: decimal
      required: false
  returns:
    type: affected

add-settlement-identity:
  description: Add settlement identity to profile
  behavior: plugin
  handler: add_settlement_identity_to_profile
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: role
      type: string
      required: true
      valid_values: [CUSTODIAN, BROKER, COUNTERPARTY]
    - name: bic
      type: string
      required: false
    - name: lei
      type: string
      required: false
    - name: alert-participant-id
      type: string
      required: false
    - name: ctm-participant-id
      type: string
      required: false
  returns:
    type: affected
```

#### 1.5 Standing Instructions Section

```yaml
add-standing-instruction:
  description: Add SSI to trading profile standing instructions section
  behavior: plugin
  handler: add_ssi_to_profile
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: category
      type: string
      required: true
      valid_values: [SECURITIES, CASH, OTC_COLLATERAL, FUND_ACCOUNTING]
    - name: name
      type: string
      required: true
      description: "Unique SSI name within category"
    - name: mic
      type: string
      required: false
    - name: currency
      type: string
      required: false
    - name: custody-account
      type: string
      required: false
    - name: custody-bic
      type: string
      required: false
    - name: cash-account
      type: string
      required: false
    - name: cash-bic
      type: string
      required: false
    - name: settlement-model
      type: string
      required: false
      valid_values: [DVP, FOP, RVP]
    - name: cutoff-time
      type: string
      required: false
    - name: cutoff-timezone
      type: string
      required: false
  returns:
    type: affected

remove-standing-instruction:
  description: Remove SSI from trading profile
  behavior: plugin
  handler: remove_ssi_from_profile
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: category
      type: string
      required: true
    - name: name
      type: string
      required: true
  returns:
    type: affected
```

#### 1.6 Booking Rules Section

```yaml
add-booking-rule:
  description: Add booking rule to trading profile
  behavior: plugin
  handler: add_booking_rule_to_profile
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: name
      type: string
      required: true
    - name: priority
      type: integer
      required: true
    - name: ssi-ref
      type: string
      required: true
      description: "Reference to SSI name in standing_instructions"
    - name: match-counterparty-ref
      type: string
      required: false
    - name: match-instrument-class
      type: string
      required: false
    - name: match-security-type
      type: string
      required: false
    - name: match-mic
      type: string
      required: false
    - name: match-currency
      type: string
      required: false
    - name: match-settlement-type
      type: string
      required: false
  returns:
    type: affected

remove-booking-rule:
  description: Remove booking rule from trading profile
  behavior: plugin
  handler: remove_booking_rule_from_profile
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: name
      type: string
      required: true
  returns:
    type: affected
```

---

### Category 2: Document Lifecycle Verbs

```yaml
create-draft:
  description: Create new draft trading profile for CBU
  behavior: plugin
  handler: create_draft_trading_profile
  args:
    - name: cbu-id
      type: uuid
      required: true
      lookup:
        table: cbus
        entity_type: cbu
        schema: ob-poc
        search_key: name
        primary_key: cbu_id
    - name: base-currency
      type: string
      required: true
      default: USD
    - name: copy-from-profile
      type: uuid
      required: false
      description: "Clone from existing profile"
    - name: notes
      type: string
      required: false
  returns:
    type: uuid
    name: profile_id
    capture: true

increment-version:
  description: Create new version from existing profile
  behavior: plugin
  handler: increment_profile_version
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: notes
      type: string
      required: false
  returns:
    type: uuid
    name: new_profile_id
    capture: true

submit-for-review:
  description: Submit draft profile for review
  behavior: plugin
  handler: submit_profile_for_review
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: submitted-by
      type: string
      required: false
  returns:
    type: affected

approve:
  description: Approve profile (changes status to ACTIVE)
  behavior: plugin
  handler: approve_trading_profile
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: approved-by
      type: string
      required: false
  returns:
    type: affected

reject:
  description: Reject profile (returns to DRAFT)
  behavior: plugin
  handler: reject_trading_profile
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: rejected-by
      type: string
      required: false
    - name: reason
      type: string
      required: true
  returns:
    type: affected
```

---

### Category 3: Document-Operational Sync Verbs

```yaml
sync-to-operational:
  description: Materialize specific sections to operational tables
  behavior: plugin
  handler: sync_profile_to_operational
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: sections
      type: string_list
      required: false
      default: [all]
      valid_values:
        - all
        - universe
        - ssis
        - booking_rules
        - isda
        - subcustodians
        - im_assignments
    - name: mode
      type: string
      required: false
      default: MERGE
      valid_values:
        - MERGE       # Add/update, don't delete
        - REPLACE     # Delete and recreate
        - DIFF_ONLY   # Show what would change
  returns:
    type: record
    description: "Sync result with counts"

sync-from-operational:
  description: Rebuild document sections from operational tables
  behavior: plugin
  handler: sync_operational_to_profile
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: sections
      type: string_list
      required: false
      default: [all]
    - name: mode
      type: string
      required: false
      default: MERGE
      valid_values:
        - MERGE
        - REPLACE
        - DIFF_ONLY
  returns:
    type: record

diff-document-vs-operational:
  description: Show differences between document and operational tables
  behavior: plugin
  handler: diff_profile_vs_operational
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: sections
      type: string_list
      required: false
      default: [all]
  returns:
    type: record
    description: "{ section: { in_document_only: [...], in_operational_only: [...], different: [...] } }"
```

---

### Category 4: Validation Verbs

```yaml
validate-universe-coverage:
  description: Validate all universe entries have required SSIs
  behavior: plugin
  handler: validate_universe_coverage
  args:
    - name: profile-id
      type: uuid
      required: true
  returns:
    type: record
    description: "{ complete: bool, gaps: [{ class, market, missing: [SSI, BOOKING_RULE, ...] }] }"

validate-im-scope:
  description: Validate IM scopes are subset of universe
  behavior: plugin
  handler: validate_im_scope
  args:
    - name: profile-id
      type: uuid
      required: true
  returns:
    type: record
    description: "{ valid: bool, issues: [{ manager, invalid_mics: [...], invalid_classes: [...] }] }"

validate-isda-coverage:
  description: Validate OTC classes have ISDA coverage
  behavior: plugin
  handler: validate_isda_coverage
  args:
    - name: profile-id
      type: uuid
      required: true
  returns:
    type: record
    description: "{ complete: bool, uncovered_otc: [{ class, counterparty }] }"

validate-csa-ssi-refs:
  description: Validate all CSA ssi_refs point to valid SSIs
  behavior: plugin
  handler: validate_csa_ssi_refs
  args:
    - name: profile-id
      type: uuid
      required: true
  returns:
    type: record
    description: "{ valid: bool, invalid_refs: [{ csa, ref, error }] }"

validate-booking-rule-ssi-refs:
  description: Validate all booking rule ssi_refs point to valid SSIs
  behavior: plugin
  handler: validate_booking_rule_ssi_refs
  args:
    - name: profile-id
      type: uuid
      required: true
  returns:
    type: record

validate-go-live-ready:
  description: Full validation for production readiness
  behavior: plugin
  handler: validate_profile_go_live_ready
  args:
    - name: profile-id
      type: uuid
      required: true
    - name: strictness
      type: string
      required: false
      default: STRICT
      valid_values: [STRICT, STANDARD, PERMISSIVE]
  returns:
    type: record
    description: "{ ready: bool, blockers: [...], warnings: [...], coverage_pct: float }"
```

---

## Plugin Handler Implementations Required

### File: `rust/src/trading_profile/document_ops.rs` (NEW)

```rust
//! Document-level operations for TradingProfileDocument
//! 
//! These handlers modify the JSONB document directly, not operational tables.

use sqlx::PgPool;
use uuid::Uuid;
use serde_json::Value;

use super::types::*;

/// Add instrument class to profile.document.universe.instrument_classes
pub async fn add_instrument_class_to_profile(
    pool: &PgPool,
    profile_id: Uuid,
    class_code: String,
    cfi_prefixes: Option<Vec<String>>,
    isda_asset_classes: Option<Vec<String>>,
    is_held: bool,
    is_traded: bool,
) -> Result<Value, DocumentOpError> {
    // 1. Fetch current document
    let doc = get_profile_document(pool, profile_id).await?;
    
    // 2. Parse into TradingProfileDocument
    let mut profile: TradingProfileDocument = serde_json::from_value(doc)?;
    
    // 3. Check if class already exists
    if profile.universe.instrument_classes.iter().any(|c| c.class_code == class_code) {
        return Err(DocumentOpError::AlreadyExists {
            item: "instrument_class",
            key: class_code,
        });
    }
    
    // 4. Add new class
    profile.universe.instrument_classes.push(InstrumentClassConfig {
        class_code,
        cfi_prefixes: cfi_prefixes.unwrap_or_default(),
        isda_asset_classes: isda_asset_classes.unwrap_or_default(),
        is_held,
        is_traded,
    });
    
    // 5. Update document in DB
    update_profile_document(pool, profile_id, &profile).await?;
    
    // 6. Return updated universe section
    Ok(serde_json::to_value(&profile.universe)?)
}

/// Add market to profile.document.universe.allowed_markets
pub async fn add_market_to_profile(
    pool: &PgPool,
    profile_id: Uuid,
    mic: String,
    currencies: Vec<String>,
    settlement_types: Option<Vec<String>>,
) -> Result<Value, DocumentOpError> {
    let mut profile = get_and_parse_profile(pool, profile_id).await?;
    
    // Check if market exists
    if profile.universe.allowed_markets.iter().any(|m| m.mic == mic) {
        // Update existing
        if let Some(market) = profile.universe.allowed_markets.iter_mut().find(|m| m.mic == mic) {
            market.currencies = currencies;
            if let Some(st) = settlement_types {
                market.settlement_types = st;
            }
        }
    } else {
        // Add new
        profile.universe.allowed_markets.push(MarketConfig {
            mic,
            currencies,
            settlement_types: settlement_types.unwrap_or_else(|| vec!["DVP".to_string()]),
        });
    }
    
    update_profile_document(pool, profile_id, &profile).await?;
    Ok(serde_json::to_value(&profile.universe)?)
}

// ... similar implementations for all document ops
```

### File: `rust/src/trading_profile/materialize.rs` (NEW)

```rust
//! Materialize trading profile document to operational tables

use sqlx::PgPool;
use uuid::Uuid;

use super::types::*;
use super::resolve::resolve_entity_ref;

pub struct MaterializeResult {
    pub universe_created: usize,
    pub ssis_created: usize,
    pub booking_rules_created: usize,
    pub isdas_created: usize,
    pub csas_created: usize,
    pub errors: Vec<MaterializeError>,
}

pub async fn materialize_trading_profile(
    pool: &PgPool,
    profile_id: Uuid,
    sections: Option<Vec<String>>,
    dry_run: bool,
    force: bool,
) -> Result<MaterializeResult, MaterializeError> {
    let profile = get_and_parse_profile(pool, profile_id).await?;
    let cbu_id = get_profile_cbu_id(pool, profile_id).await?;
    
    let sections = sections.unwrap_or_else(|| vec!["all".to_string()]);
    let all = sections.contains(&"all".to_string());
    
    let mut result = MaterializeResult::default();
    
    // Begin transaction
    let mut tx = pool.begin().await?;
    
    if all || sections.contains(&"universe".to_string()) {
        result.universe_created = materialize_universe(&mut tx, cbu_id, &profile.universe, force).await?;
    }
    
    if all || sections.contains(&"ssis".to_string()) {
        result.ssis_created = materialize_ssis(&mut tx, cbu_id, &profile.standing_instructions, force).await?;
    }
    
    if all || sections.contains(&"booking_rules".to_string()) {
        result.booking_rules_created = materialize_booking_rules(&mut tx, cbu_id, &profile.booking_rules, force).await?;
    }
    
    if all || sections.contains(&"isda".to_string()) {
        let (isdas, csas) = materialize_isda_agreements(&mut tx, cbu_id, &profile.isda_agreements, force).await?;
        result.isdas_created = isdas;
        result.csas_created = csas;
    }
    
    if dry_run {
        tx.rollback().await?;
    } else {
        // Update materialization status
        sqlx::query!(
            r#"UPDATE "ob-poc".cbu_trading_profiles 
               SET materialization_status = 'COMPLETE',
                   materialized_at = now(),
                   materialization_hash = $2
               WHERE profile_id = $1"#,
            profile_id,
            compute_document_hash(&profile)
        )
        .execute(&mut *tx)
        .await?;
        
        tx.commit().await?;
    }
    
    Ok(result)
}

async fn materialize_universe(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    cbu_id: Uuid,
    universe: &Universe,
    force: bool,
) -> Result<usize, MaterializeError> {
    let mut count = 0;
    
    // For each instrument class × market combination
    for class in &universe.instrument_classes {
        let class_id = lookup_instrument_class(&mut **tx, &class.class_code).await?;
        
        if universe.allowed_markets.is_empty() {
            // Class with no specific market (OTC typically)
            count += upsert_universe_entry(
                tx, cbu_id, class_id, None, 
                &universe.allowed_currencies, 
                class.is_held, class.is_traded,
                force
            ).await?;
        } else {
            for market in &universe.allowed_markets {
                let market_id = lookup_market(&mut **tx, &market.mic).await?;
                count += upsert_universe_entry(
                    tx, cbu_id, class_id, Some(market_id),
                    &market.currencies,
                    class.is_held, class.is_traded,
                    force
                ).await?;
            }
        }
    }
    
    Ok(count)
}

// ... similar for other sections
```

### File: `rust/src/trading_profile/sync.rs` (NEW)

```rust
//! Bidirectional sync between document and operational tables

use sqlx::PgPool;
use uuid::Uuid;

pub struct SyncDiff {
    pub in_document_only: Vec<String>,
    pub in_operational_only: Vec<String>,
    pub different: Vec<DiffItem>,
}

pub struct DiffItem {
    pub key: String,
    pub document_value: serde_json::Value,
    pub operational_value: serde_json::Value,
}

/// Diff document universe vs cbu_instrument_universe table
pub async fn diff_universe(
    pool: &PgPool,
    profile_id: Uuid,
) -> Result<SyncDiff, SyncError> {
    let profile = get_and_parse_profile(pool, profile_id).await?;
    let cbu_id = get_profile_cbu_id(pool, profile_id).await?;
    
    // Get document entries as set of (class_code, mic, currencies)
    let doc_entries = extract_universe_keys(&profile.universe);
    
    // Get operational entries
    let op_entries = sqlx::query!(
        r#"SELECT ic.code, m.mic, u.currencies
           FROM custody.cbu_instrument_universe u
           JOIN custody.instrument_classes ic ON ic.class_id = u.instrument_class_id
           LEFT JOIN custody.markets m ON m.market_id = u.market_id
           WHERE u.cbu_id = $1"#,
        cbu_id
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| (r.code, r.mic, r.currencies))
    .collect::<HashSet<_>>();
    
    // Compute diff
    let in_doc_only = doc_entries.difference(&op_entries).cloned().collect();
    let in_op_only = op_entries.difference(&doc_entries).cloned().collect();
    
    Ok(SyncDiff {
        in_document_only: in_doc_only,
        in_operational_only: in_op_only,
        different: vec![], // TODO: compare matching entries
    })
}

/// Rebuild document from operational tables
pub async fn sync_operational_to_document(
    pool: &PgPool,
    profile_id: Uuid,
    sections: Vec<String>,
) -> Result<(), SyncError> {
    let mut profile = get_and_parse_profile(pool, profile_id).await?;
    let cbu_id = get_profile_cbu_id(pool, profile_id).await?;
    
    if sections.contains(&"universe".to_string()) || sections.contains(&"all".to_string()) {
        profile.universe = rebuild_universe_from_tables(pool, cbu_id).await?;
    }
    
    if sections.contains(&"ssis".to_string()) || sections.contains(&"all".to_string()) {
        profile.standing_instructions = rebuild_ssis_from_tables(pool, cbu_id).await?;
    }
    
    // ... other sections
    
    update_profile_document(pool, profile_id, &profile).await?;
    Ok(())
}
```

---

## Example Agent Workflow

With these verbs, an agent can incrementally construct an instrument matrix:

```
# 1. Create draft profile
(trading-profile.create-draft :cbu-id @allianz :base-currency EUR :as @profile)

# 2. Add instrument classes
(trading-profile.add-instrument-class :profile-id @profile :class-code EQUITY :is-held true :is-traded true)
(trading-profile.add-instrument-class :profile-id @profile :class-code FIXED_INCOME :is-held true :is-traded true)
(trading-profile.add-instrument-class :profile-id @profile :class-code IRS :isda-asset-classes [RATES] :is-held false :is-traded true)

# 3. Add markets
(trading-profile.add-market :profile-id @profile :mic XNYS :currencies [USD])
(trading-profile.add-market :profile-id @profile :mic XLON :currencies [GBP, USD])
(trading-profile.add-market :profile-id @profile :mic XFRA :currencies [EUR])

# 4. Add ISDA for OTC
(trading-profile.add-isda-config :profile-id @profile 
    :counterparty-ref "549300TRUWO2CD2G5692" :counterparty-ref-type LEI
    :agreement-date 2024-01-15 :governing-law NY)

(trading-profile.add-isda-product-coverage :profile-id @profile
    :counterparty-ref "549300TRUWO2CD2G5692"
    :asset-class RATES :base-products [IRS, BASIS_SWAP])

(trading-profile.add-csa-config :profile-id @profile
    :counterparty-ref "549300TRUWO2CD2G5692"
    :csa-type VM_IM :threshold-amount 10000000 :threshold-currency USD)

(trading-profile.add-csa-eligible-collateral :profile-id @profile
    :counterparty-ref "549300TRUWO2CD2G5692" :csa-type VM_IM
    :collateral-type CASH :currencies [USD, EUR] :haircut-pct 0)

(trading-profile.add-csa-eligible-collateral :profile-id @profile
    :counterparty-ref "549300TRUWO2CD2G5692" :csa-type VM_IM
    :collateral-type GOVT_BOND :currencies [USD] :issuers [US] :haircut-pct 2.0)

# 5. Add Investment Manager
(trading-profile.add-im-mandate :profile-id @profile
    :manager-ref "BLKIUS33" :manager-ref-type BIC
    :priority 1 :scope-all true :instruction-method CTM
    :can-trade true :can-settle true)

# 6. Add SSIs
(trading-profile.add-standing-instruction :profile-id @profile
    :category SECURITIES :name "XNYS-USD-PRIMARY"
    :mic XNYS :currency USD
    :custody-account "12345" :custody-bic "IRVTUS3N")

# 7. Add Booking Rules
(trading-profile.add-booking-rule :profile-id @profile
    :name "US-EQUITY-DEFAULT" :priority 100
    :ssi-ref "XNYS-USD-PRIMARY"
    :match-instrument-class EQUITY :match-mic XNYS)

# 8. Validate
(trading-profile.validate-go-live-ready :profile-id @profile :strictness STANDARD)

# 9. If ready, submit for review
(trading-profile.submit-for-review :profile-id @profile)

# 10. After approval, materialize to operational tables
(trading-profile.sync-to-operational :profile-id @profile :sections [all] :mode REPLACE)
```

---

## Implementation Priority

### Phase 1: Core Document Construction (High)
1. `create-draft`
2. `add-instrument-class` / `remove-instrument-class`
3. `add-market` / `remove-market`
4. `add-standing-instruction` / `remove-standing-instruction`
5. `add-booking-rule` / `remove-booking-rule`

### Phase 2: ISDA/CSA Construction (High)
1. `add-isda-config`
2. `add-isda-product-coverage`
3. `add-csa-config`
4. `add-csa-eligible-collateral`
5. `add-csa-initial-margin`
6. `link-csa-ssi`

### Phase 3: IM & Settlement (Medium)
1. `add-im-mandate`
2. `update-im-scope`
3. `add-subcustodian`
4. `add-matching-platform`
5. `add-settlement-identity`

### Phase 4: Sync & Validation (Medium)
1. `sync-to-operational`
2. `sync-from-operational`
3. `diff-document-vs-operational`
4. `validate-go-live-ready`

### Phase 5: Lifecycle (Lower)
1. `increment-version`
2. `submit-for-review`
3. `approve` / `reject`

---

## Files to Create/Modify

| File | Action | Purpose |
|------|--------|---------|
| `rust/config/verbs/trading-profile.yaml` | Modify | Add construction verbs |
| `rust/src/trading_profile/document_ops.rs` | Create | Document modification handlers |
| `rust/src/trading_profile/materialize.rs` | Create | Document → operational sync |
| `rust/src/trading_profile/sync.rs` | Create | Bidirectional sync |
| `rust/src/trading_profile/validate_profile.rs` | Create | Full profile validation |
| `rust/src/trading_profile/mod.rs` | Modify | Export new modules |
| `rust/src/dsl_v2/plugins/trading_profile.rs` | Create | Plugin handler registration |

---

## Dependencies

- Existing `TradingProfileDocument` types ✅
- Entity resolution (`resolve.rs`) ✅
- Instrument class / market lookup tables ✅
- ISDA/CSA tables ✅

---

## Summary

**Current Gap**: DSL can import complete documents but cannot construct them incrementally.

**Solution**: Add ~40 new verbs for document-level operations that modify JSONB directly.

**Outcome**: Agent can build instrument matrices step-by-step through conversation, with full audit trail at document level, bidirectional sync to operational tables, and validation before go-live.
