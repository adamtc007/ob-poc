# CBU Trading Profile Implementation (Complete)

## Overview

The **CBU Trading Profile** is the canonical document that captures a Client Business Unit's complete trading identity:

1. **What they trade** (universe: instrument classes, markets, currencies)
2. **Who manages it** (investment managers - default + asset class overrides)
3. **OTC derivatives framework** (ISDA agreements, CSA collateral terms, product coverage)
4. **How they instruct** (routing rules, Omgeo/CTM matching, messaging channels)
5. **Settlement infrastructure** (subcustodian network, SSIs, booking rules)
6. **How holdings are priced** (pricing matrix for fund accounting)

### Design Principles

1. **Document-first**: The trading profile is a versioned JSON document - the single source of truth
2. **Conversational assembly**: Built incrementally via agent chat ("who are your OTC counterparties?")
3. **Materialization**: When activated, syncs to operational tables for runtime queries
4. **Idempotent**: Hash-based change detection - no-op if unchanged
5. **Auditable**: Full version history with diffs

---

## Existing Schema (Already Built)

The custody schema already has excellent infrastructure. The Trading Profile will **configure and materialize to** these tables:

### OTC Derivatives
| Table | Purpose |
|-------|---------|
| `custody.isda_agreements` | Master agreements with counterparties |
| `custody.csa_agreements` | Collateral terms (threshold, MTA, eligible collateral) |
| `custody.isda_product_coverage` | Which OTC products each ISDA covers |
| `custody.isda_product_taxonomy` | ISDA taxonomy (asset class/base/sub product, UPI) |

### Settlement Infrastructure
| Table | Purpose |
|-------|---------|
| `custody.entity_settlement_identity` | BIC, LEI, ALERT/CTM participant IDs |
| `custody.entity_ssi` | Counterparty SSIs (from ALERT network) |
| `custody.cbu_ssi` | CBU's own SSIs |
| `custody.ssi_booking_rules` | ALERT-style matching (priority + specificity score) |
| `custody.subcustodian_network` | Subcustodian chain per market/currency |
| `custody.instruction_paths` | Routing paths for instruction types |
| `custody.instruction_types` | MT540/541/542/543/54x definitions |

### Universe & Trading
| Table | Purpose |
|-------|---------|
| `custody.cbu_instrument_universe` | What the CBU trades |
| `custody.instrument_classes` | Instrument classification |
| `custody.markets` | MIC registry |
| `custody.security_types` | SMPG/ALERT security types |

---

## Trading Profile Document Structure

### Complete JSON Schema

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "CBU Trading Profile - Complete",
  "type": "object",
  "required": ["universe"],
  "properties": {
    "universe": { "$ref": "#/definitions/Universe" },
    "investment_managers": { "type": "array", "items": { "$ref": "#/definitions/IMAssignment" } },
    "isda_agreements": { "type": "array", "items": { "$ref": "#/definitions/ISDAConfig" } },
    "instruction_routing": { "type": "array", "items": { "$ref": "#/definitions/RoutingRule" } },
    "settlement_config": { "$ref": "#/definitions/SettlementConfig" },
    "pricing_matrix": { "type": "array", "items": { "$ref": "#/definitions/PricingRule" } },
    "valuation_config": { "$ref": "#/definitions/ValuationConfig" },
    "standing_instructions": { "$ref": "#/definitions/StandingInstructions" },
    "constraints": { "$ref": "#/definitions/TradingConstraints" },
    "metadata": { "type": "object" }
  }
}
```

### New Sections for ISDA/CSA

```yaml
# =============================================================================
# ISDA AGREEMENTS: OTC Derivatives Legal Framework
# =============================================================================
isda_agreements:
  # Agreement with Goldman Sachs for rates derivatives
  - counterparty:
      type: LEI
      value: "W22LROWP2IHZNBB6K528"  # Goldman Sachs
    agreement_date: "2020-03-15"
    governing_law: "ENGLISH"
    
    # Products covered by this ISDA
    product_coverage:
      - asset_class: RATES
        base_products: [SWAP, SWAPTION, CAP_FLOOR]
      - asset_class: FX
        base_products: [FORWARD, OPTION, SWAP]
    
    # CSA (Credit Support Annex) terms
    csa:
      csa_type: "VM"  # Variation Margin
      threshold_amount: 0
      threshold_currency: USD
      minimum_transfer_amount: 500000
      rounding_amount: 10000
      
      # Eligible collateral
      eligible_collateral:
        - type: CASH
          currencies: [USD, EUR, GBP]
          haircut_pct: 0
        - type: GOVT_BOND
          issuers: [US, DE, GB]
          min_rating: AA
          haircut_pct: 2.0
      
      # Collateral SSI
      collateral_ssi:
        custody_account: "COLL-GS-001"
        custody_bic: "IRVTUS3N"
        cash_account: "CASH-COLL-GS-001"
        cash_bic: "IRVTUS3N"
      
      # Timing
      valuation_time: "16:00"
      valuation_timezone: "America/New_York"
      notification_time: "18:00"
      settlement_days: 1
      
      # Dispute resolution
      dispute_resolution: "CALCULATION_AGENT"
    
    effective_date: "2020-04-01"

  # Agreement with JP Morgan for credit derivatives
  - counterparty:
      type: LEI
      value: "8IE5DZWZ7BX5LA5DJB03"  # JP Morgan
    agreement_date: "2019-06-01"
    governing_law: "NEW_YORK"
    
    product_coverage:
      - asset_class: CREDIT
        base_products: [CDS, CDX, TRANCHE]
    
    csa:
      csa_type: "VM_IM"  # Both variation and initial margin
      threshold_amount: 0
      minimum_transfer_amount: 1000000
      
      # Initial margin specifics
      initial_margin:
        calculation_method: "ISDA_SIMM"
        posting_frequency: DAILY
        segregation_required: true
        custodian:
          type: LEI
          value: "HPFHU0OQ28E4N0NFVK49"  # BNY as IM custodian
      
      eligible_collateral:
        - type: CASH
          currencies: [USD]
          haircut_pct: 0
```

### New Section for Settlement Configuration

```yaml
# =============================================================================
# SETTLEMENT CONFIG: Omgeo/CTM, Subcustodians, Matching Rules
# =============================================================================
settlement_config:
  # Trade matching platform configuration
  matching_platforms:
    - platform: CTM  # Omgeo Central Trade Matching
      participant_id: "ALLIANZGI-CTM-001"
      enabled_markets: [XNYS, XNAS, XLON, XETR]
      matching_rules:
        auto_match: true
        tolerance_price_pct: 0.01
        tolerance_quantity: 0
        auto_affirm_threshold_usd: 10000000
      
    - platform: ALERT
      participant_id: "ALLIANZGI-ALERT-001"
      enabled_markets: [XHKG, XTKS, XASX]
      matching_rules:
        auto_match: false
        enrichment_sources: [SUBCUST_NETWORK, CLIENT_SSI]
  
  # Entity settlement identities (our BICs/LEIs for different roles)
  settlement_identities:
    - role: PRINCIPAL  # As principal/fund
      bic: "ALLIGILA"
      lei: "5493001KJTIIGC8Y1R12"
      
    - role: AGENT  # As agent for third parties
      bic: "ALLIGILM"
      lei: "5493001KJTIIGC8Y1R12"
  
  # Subcustodian network preferences
  subcustodian_network:
    # Override default BNY subcustodians for specific markets
    - market: XHKG
      currency: HKD
      subcustodian:
        bic: "HSBCHKHH"
        name: "HSBC Hong Kong"
        local_agent_account: "HK-LOCAL-001"
      place_of_settlement: "CCASCHKX"  # CCASS
      is_primary: true
      
    - market: XTKS
      currency: JPY
      subcustodian:
        bic: "MABORJPJ"
        name: "Mizuho Japan"
        local_agent_account: "JP-LOCAL-001"
      place_of_settlement: "JASDECJP"
      is_primary: true
      
    - market: XETR
      currency: EUR
      subcustodian:
        bic: "DEUTDEFF"
        name: "Deutsche Bank Germany"
      place_of_settlement: "DAABORDC"  # Clearstream
      is_primary: true
  
  # Default instruction enrichment chain
  enrichment_chain:
    - source: CLIENT_SSI      # First: check CBU's own SSIs
    - source: SUBCUST_NETWORK # Then: use subcustodian network
    - source: COUNTERPARTY_SSI # Then: counterparty's published SSIs
    - source: ALERT_NETWORK    # Finally: ALERT network lookup

  # Instruction type preferences
  instruction_preferences:
    - instruction_type: DELIVERY_FREE
      swift_msg: MT542
      iso20022_msg: sese.023
      auto_release: false
      requires_approval: true
      
    - instruction_type: RECEIPT_VS_PAYMENT
      swift_msg: MT541
      iso20022_msg: sese.023
      auto_release: true
      requires_approval: false
      
    - instruction_type: DELIVERY_VS_PAYMENT
      swift_msg: MT543
      iso20022_msg: sese.023
      auto_release: true
      requires_approval: false
```

### Enhanced Booking Rules Section

```yaml
# =============================================================================
# BOOKING RULES: ALERT-Style SSI Selection
# =============================================================================
# Rules are matched by priority (lower = higher priority)
# Within same priority, specificity_score determines winner
# NULL fields = wildcard (matches any)

booking_rules:
  # Specific counterparty rule (highest specificity)
  - name: "Goldman OTC via prime broker"
    priority: 10
    match:
      counterparty:
        type: LEI
        value: "W22LROWP2IHZNBB6K528"
      instrument_class: OTC_DERIVATIVE
      isda_asset_class: RATES
    ssi_ref: GS_OTC_SSI
    
  # Market + instrument specific
  - name: "German equities via Clearstream"
    priority: 20
    match:
      market: XETR
      instrument_class: EQUITY
      currency: EUR
    ssi_ref: DE_EQUITY_SSI
    
  # Market + currency (broader)
  - name: "All EUR securities in Germany"
    priority: 30
    match:
      market: XETR
      currency: EUR
    ssi_ref: DE_EUR_SSI
    
  # Instrument class default
  - name: "All government bonds"
    priority: 40
    match:
      instrument_class: GOVT_BOND
    ssi_ref: BOND_DEFAULT_SSI
    
  # Catch-all fallback
  - name: "Default SSI"
    priority: 100
    match: {}  # Matches everything
    ssi_ref: DEFAULT_SSI
```

---

## Rust Types (Extended)

### ISDA/CSA Types

```rust
// rust/src/trading_profile/types.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ISDAConfig {
    pub counterparty: EntityRef,
    pub agreement_date: NaiveDate,
    pub governing_law: GoverningLaw,
    pub product_coverage: Vec<ProductCoverage>,
    pub csa: Option<CSAConfig>,
    pub effective_date: NaiveDate,
    pub termination_date: Option<NaiveDate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GoverningLaw {
    #[serde(rename = "ENGLISH")]
    English,
    #[serde(rename = "NEW_YORK")]
    NewYork,
    #[serde(rename = "JAPANESE")]
    Japanese,
    #[serde(rename = "GERMAN")]
    German,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductCoverage {
    pub asset_class: String,  // RATES, FX, CREDIT, EQUITY, COMMODITY
    pub base_products: Vec<String>,
    pub sub_products: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CSAConfig {
    pub csa_type: CSAType,
    pub threshold_amount: Decimal,
    pub threshold_currency: String,
    pub minimum_transfer_amount: Decimal,
    pub rounding_amount: Option<Decimal>,
    pub eligible_collateral: Vec<EligibleCollateral>,
    pub collateral_ssi: SSI,
    pub valuation_time: String,
    pub valuation_timezone: String,
    pub notification_time: Option<String>,
    pub settlement_days: i32,
    pub initial_margin: Option<InitialMarginConfig>,
    pub dispute_resolution: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CSAType {
    #[serde(rename = "VM")]
    VariationMargin,
    #[serde(rename = "VM_IM")]
    VariationAndInitialMargin,
    #[serde(rename = "LEGACY")]
    Legacy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EligibleCollateral {
    #[serde(rename = "type")]
    pub collateral_type: CollateralType,
    pub currencies: Option<Vec<String>>,
    pub issuers: Option<Vec<String>>,
    pub min_rating: Option<String>,
    pub haircut_pct: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitialMarginConfig {
    pub calculation_method: String,  // ISDA_SIMM, GRID, SCHEDULE
    pub posting_frequency: Frequency,
    pub segregation_required: bool,
    pub custodian: Option<EntityRef>,
}
```

### Settlement Config Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementConfig {
    pub matching_platforms: Vec<MatchingPlatformConfig>,
    pub settlement_identities: Vec<SettlementIdentity>,
    pub subcustodian_network: Vec<SubcustodianOverride>,
    pub enrichment_chain: Vec<EnrichmentSource>,
    pub instruction_preferences: Vec<InstructionPreference>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchingPlatformConfig {
    pub platform: MatchingPlatform,
    pub participant_id: String,
    pub enabled_markets: Vec<String>,
    pub matching_rules: MatchingRules,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchingPlatform {
    CTM,      // Omgeo Central Trade Matching
    ALERT,    // SWIFT ALERT
    TradeSuite,
    TradeNeXus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchingRules {
    pub auto_match: bool,
    pub tolerance_price_pct: Option<Decimal>,
    pub tolerance_quantity: Option<i64>,
    pub auto_affirm_threshold_usd: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementIdentity {
    pub role: SettlementRole,
    pub bic: String,
    pub lei: Option<String>,
    pub alert_participant_id: Option<String>,
    pub ctm_participant_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubcustodianOverride {
    pub market: String,
    pub currency: String,
    pub subcustodian: SubcustodianInfo,
    pub place_of_settlement: String,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubcustodianInfo {
    pub bic: String,
    pub name: String,
    pub local_agent_account: Option<String>,
    pub local_agent_bic: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EnrichmentSource {
    #[serde(rename = "CLIENT_SSI")]
    ClientSSI,
    #[serde(rename = "SUBCUST_NETWORK")]
    SubcustodianNetwork,
    #[serde(rename = "COUNTERPARTY_SSI")]
    CounterpartySSI,
    #[serde(rename = "ALERT_NETWORK")]
    AlertNetwork,
}
```

### Booking Rules Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookingRule {
    pub name: String,
    pub priority: i32,
    #[serde(rename = "match")]
    pub match_criteria: BookingMatchCriteria,
    pub ssi_ref: String,
    pub effective_date: Option<NaiveDate>,
    pub expiry_date: Option<NaiveDate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookingMatchCriteria {
    pub counterparty: Option<EntityRef>,
    pub instrument_class: Option<String>,
    pub security_type: Option<String>,
    pub market: Option<String>,
    pub currency: Option<String>,
    pub settlement_type: Option<String>,
    pub isda_asset_class: Option<String>,
    pub isda_base_product: Option<String>,
}

impl BookingMatchCriteria {
    /// Calculate ALERT-style specificity score
    pub fn specificity_score(&self) -> i32 {
        let mut score = 0;
        if self.counterparty.is_some() { score += 32; }
        if self.instrument_class.is_some() { score += 16; }
        if self.security_type.is_some() { score += 8; }
        if self.market.is_some() { score += 4; }
        if self.currency.is_some() { score += 2; }
        if self.settlement_type.is_some() { score += 1; }
        score
    }
}
```

---

## Extended Verbs

### ISDA/CSA Verbs

```yaml
# In trading-profile.yaml

      # === ISDA Agreement Management ===
      add-isda:
        description: Add ISDA master agreement to profile
        behavior: plugin
        handler: trading_profile_add_isda
        args:
          - name: profile-id
            type: uuid
            required: true
          - name: counterparty
            type: uuid
            required: true
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: search_name
              primary_key: entity_id
          - name: agreement-date
            type: date
            required: true
          - name: governing-law
            type: string
            required: true
            valid_values: [ENGLISH, NEW_YORK, JAPANESE, GERMAN]
          - name: effective-date
            type: date
            required: true
        returns:
          type: record

      add-isda-coverage:
        description: Add product coverage to an ISDA
        behavior: plugin
        handler: trading_profile_add_isda_coverage
        args:
          - name: profile-id
            type: uuid
            required: true
          - name: counterparty
            type: uuid
            required: true
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: search_name
              primary_key: entity_id
          - name: asset-class
            type: string
            required: true
            valid_values: [RATES, FX, CREDIT, EQUITY, COMMODITY]
          - name: base-products
            type: string_list
            required: true
        returns:
          type: record

      set-csa:
        description: Configure CSA terms for an ISDA
        behavior: plugin
        handler: trading_profile_set_csa
        args:
          - name: profile-id
            type: uuid
            required: true
          - name: counterparty
            type: uuid
            required: true
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: search_name
              primary_key: entity_id
          - name: csa-type
            type: string
            required: true
            valid_values: [VM, VM_IM, LEGACY]
          - name: threshold-amount
            type: decimal
            required: true
          - name: threshold-currency
            type: string
            required: true
          - name: mta
            type: decimal
            required: true
            description: Minimum Transfer Amount
          - name: rounding
            type: decimal
            required: false
          - name: valuation-time
            type: string
            required: true
          - name: timezone
            type: string
            required: false
            default: "America/New_York"
          - name: settlement-days
            type: integer
            required: false
            default: 1
        returns:
          type: record

      add-eligible-collateral:
        description: Add eligible collateral type to CSA
        behavior: plugin
        handler: trading_profile_add_eligible_collateral
        args:
          - name: profile-id
            type: uuid
            required: true
          - name: counterparty
            type: uuid
            required: true
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
          - name: min-rating
            type: string
            required: false
          - name: haircut-pct
            type: decimal
            required: true
        returns:
          type: record
```

### Settlement Config Verbs

```yaml
      # === Settlement Configuration ===
      set-matching-platform:
        description: Configure trade matching platform
        behavior: plugin
        handler: trading_profile_set_matching_platform
        args:
          - name: profile-id
            type: uuid
            required: true
          - name: platform
            type: string
            required: true
            valid_values: [CTM, ALERT, TradeSuite, TradeNeXus]
          - name: participant-id
            type: string
            required: true
          - name: markets
            type: string_list
            required: true
          - name: auto-match
            type: boolean
            required: false
            default: true
          - name: price-tolerance-pct
            type: decimal
            required: false
          - name: auto-affirm-threshold
            type: decimal
            required: false
        returns:
          type: record

      add-settlement-identity:
        description: Add settlement identity (BIC/LEI)
        behavior: plugin
        handler: trading_profile_add_settlement_identity
        args:
          - name: profile-id
            type: uuid
            required: true
          - name: role
            type: string
            required: true
            valid_values: [PRINCIPAL, AGENT, CUSTODIAN]
          - name: bic
            type: string
            required: true
          - name: lei
            type: string
            required: false
          - name: alert-id
            type: string
            required: false
          - name: ctm-id
            type: string
            required: false
        returns:
          type: record

      set-subcustodian:
        description: Override subcustodian for a market
        behavior: plugin
        handler: trading_profile_set_subcustodian
        args:
          - name: profile-id
            type: uuid
            required: true
          - name: market
            type: string
            required: true
          - name: currency
            type: string
            required: true
          - name: subcustodian-bic
            type: string
            required: true
          - name: subcustodian-name
            type: string
            required: true
          - name: local-agent-account
            type: string
            required: false
          - name: pset-bic
            type: string
            required: true
            description: Place of settlement BIC (CSD)
          - name: is-primary
            type: boolean
            required: false
            default: true
        returns:
          type: record

      add-booking-rule:
        description: Add ALERT-style booking rule
        behavior: plugin
        handler: trading_profile_add_booking_rule
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
          - name: counterparty
            type: uuid
            required: false
          - name: instrument-class
            type: string
            required: false
          - name: security-type
            type: string
            required: false
          - name: market
            type: string
            required: false
          - name: currency
            type: string
            required: false
          - name: settlement-type
            type: string
            required: false
          - name: isda-asset-class
            type: string
            required: false
        returns:
          type: record

      # === Resolution ===
      resolve-booking:
        description: Find matching booking rule (ALERT-style)
        behavior: plugin
        handler: trading_profile_resolve_booking
        args:
          - name: cbu-id
            type: uuid
            required: true
          - name: counterparty
            type: uuid
            required: false
          - name: instrument-class
            type: string
            required: false
          - name: security-type
            type: string
            required: false
          - name: market
            type: string
            required: false
          - name: currency
            type: string
            required: false
          - name: settlement-type
            type: string
            required: false
        returns:
          type: record

      resolve-isda:
        description: Find applicable ISDA for OTC trade
        behavior: plugin
        handler: trading_profile_resolve_isda
        args:
          - name: cbu-id
            type: uuid
            required: true
          - name: counterparty
            type: uuid
            required: true
          - name: asset-class
            type: string
            required: true
          - name: base-product
            type: string
            required: false
        returns:
          type: record
```

---

## Materialization Mapping

```rust
// rust/src/trading_profile/materialize.rs

pub async fn materialize(
    pool: &PgPool, 
    cbu_id: Uuid, 
    doc: &TradingProfileDoc
) -> Result<MaterializeResult> {
    let mut result = MaterializeResult::default();
    
    // Transaction for atomic sync
    let mut tx = pool.begin().await?;
    
    // 1. Universe → custody.cbu_instrument_universe
    result.universe_rows = sync_universe(&mut tx, cbu_id, &doc.universe).await?;
    
    // 2. Investment managers → ob-poc.investment_manager_mandates
    result.im_mandates = sync_investment_managers(&mut tx, cbu_id, &doc.investment_managers).await?;
    
    // 3. ISDA/CSA → custody.isda_agreements + custody.csa_agreements
    result.isda_agreements = sync_isda_agreements(&mut tx, cbu_id, &doc.isda_agreements).await?;
    
    // 4. Settlement identities → custody.entity_settlement_identity
    if let Some(ref sc) = doc.settlement_config {
        result.settlement_identities = sync_settlement_identities(&mut tx, cbu_id, &sc.settlement_identities).await?;
    }
    
    // 5. Subcustodian network → custody.subcustodian_network
    if let Some(ref sc) = doc.settlement_config {
        result.subcustodian_rows = sync_subcustodian_network(&mut tx, cbu_id, &sc.subcustodian_network).await?;
    }
    
    // 6. SSIs → custody.cbu_ssi
    result.ssi_rows = sync_standing_instructions(&mut tx, cbu_id, &doc.standing_instructions).await?;
    
    // 7. Booking rules → custody.ssi_booking_rules
    result.booking_rules = sync_booking_rules(&mut tx, cbu_id, &doc.booking_rules).await?;
    
    // 8. Pricing matrix → ob-poc.pricing_source_hierarchy
    result.pricing_rules = sync_pricing_matrix(&mut tx, cbu_id, &doc.pricing_matrix).await?;
    
    // 9. Valuation config → ob-poc.valuation_schedule
    if let Some(ref vc) = doc.valuation_config {
        result.valuation_schedules = sync_valuation_config(&mut tx, cbu_id, vc).await?;
    }
    
    tx.commit().await?;
    Ok(result)
}

async fn sync_isda_agreements(
    tx: &mut Transaction<'_, Postgres>,
    cbu_id: Uuid,
    isda_configs: &[ISDAConfig],
) -> Result<usize> {
    // Delete existing (full replace strategy for simplicity)
    sqlx::query("DELETE FROM custody.isda_agreements WHERE cbu_id = $1")
        .bind(cbu_id)
        .execute(&mut **tx)
        .await?;
    
    for isda in isda_configs {
        // Resolve counterparty entity
        let counterparty_id = resolve_entity_ref(tx, &isda.counterparty).await?;
        
        // Insert ISDA
        let isda_id: Uuid = sqlx::query_scalar(r#"
            INSERT INTO custody.isda_agreements 
            (cbu_id, counterparty_entity_id, agreement_date, governing_law, effective_date, termination_date)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING isda_id
        "#)
        .bind(cbu_id)
        .bind(counterparty_id)
        .bind(isda.agreement_date)
        .bind(&isda.governing_law.to_string())
        .bind(isda.effective_date)
        .bind(isda.termination_date)
        .fetch_one(&mut **tx)
        .await?;
        
        // Insert product coverage
        for coverage in &isda.product_coverage {
            for base_product in &coverage.base_products {
                sqlx::query(r#"
                    INSERT INTO custody.isda_product_coverage 
                    (isda_id, instrument_class_id, isda_taxonomy_id)
                    SELECT $1, ic.class_id, ipt.taxonomy_id
                    FROM custody.instrument_classes ic
                    LEFT JOIN custody.isda_product_taxonomy ipt 
                        ON ipt.asset_class = $2 AND ipt.base_product = $3
                    WHERE ic.isda_asset_class = $2
                "#)
                .bind(isda_id)
                .bind(&coverage.asset_class)
                .bind(base_product)
                .execute(&mut **tx)
                .await?;
            }
        }
        
        // Insert CSA if present
        if let Some(ref csa) = isda.csa {
            // Resolve collateral SSI
            let coll_ssi_id = create_or_get_ssi(tx, cbu_id, &csa.collateral_ssi).await?;
            
            sqlx::query(r#"
                INSERT INTO custody.csa_agreements 
                (isda_id, csa_type, threshold_amount, threshold_currency, 
                 minimum_transfer_amount, rounding_amount, collateral_ssi_id, effective_date)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#)
            .bind(isda_id)
            .bind(&csa.csa_type.to_string())
            .bind(&csa.threshold_amount)
            .bind(&csa.threshold_currency)
            .bind(&csa.minimum_transfer_amount)
            .bind(&csa.rounding_amount)
            .bind(coll_ssi_id)
            .bind(isda.effective_date)
            .execute(&mut **tx)
            .await?;
        }
    }
    
    Ok(isda_configs.len())
}
```

---

## Conversational Assembly Example

```clojure
;; Agent: "Do you trade OTC derivatives?"
;; User: "Yes, rates and FX with Goldman, credit with JP Morgan"

(trading-profile.add-isda
  :profile-id @profile
  :counterparty @goldman_sachs
  :agreement-date "2020-03-15"
  :governing-law "ENGLISH"
  :effective-date "2020-04-01")

(trading-profile.add-isda-coverage
  :profile-id @profile
  :counterparty @goldman_sachs
  :asset-class "RATES"
  :base-products ["SWAP" "SWAPTION" "CAP_FLOOR"])

(trading-profile.add-isda-coverage
  :profile-id @profile
  :counterparty @goldman_sachs
  :asset-class "FX"
  :base-products ["FORWARD" "OPTION" "SWAP"])

;; Agent: "What are the CSA terms with Goldman?"
;; User: "Zero threshold, $500k MTA, we can post cash in USD/EUR/GBP"

(trading-profile.set-csa
  :profile-id @profile
  :counterparty @goldman_sachs
  :csa-type "VM"
  :threshold-amount 0
  :threshold-currency "USD"
  :mta 500000
  :rounding 10000
  :valuation-time "16:00"
  :timezone "America/New_York")

(trading-profile.add-eligible-collateral
  :profile-id @profile
  :counterparty @goldman_sachs
  :collateral-type "CASH"
  :currencies ["USD" "EUR" "GBP"]
  :haircut-pct 0)

;; Agent: "How do you match trades?"
;; User: "CTM for US and Europe, ALERT for Asia"

(trading-profile.set-matching-platform
  :profile-id @profile
  :platform "CTM"
  :participant-id "ALLIANZGI-CTM-001"
  :markets ["XNYS" "XNAS" "XLON" "XETR"]
  :auto-match true
  :auto-affirm-threshold 10000000)

(trading-profile.set-matching-platform
  :profile-id @profile
  :platform "ALERT"
  :participant-id "ALLIANZGI-ALERT-001"
  :markets ["XHKG" "XTKS" "XASX"]
  :auto-match false)

;; Agent: "Who are your subcustodians in Asia?"
;; User: "HSBC in Hong Kong, Mizuho in Japan"

(trading-profile.set-subcustodian
  :profile-id @profile
  :market "XHKG"
  :currency "HKD"
  :subcustodian-bic "HSBCHKHH"
  :subcustodian-name "HSBC Hong Kong"
  :pset-bic "CCASCHKX")

(trading-profile.set-subcustodian
  :profile-id @profile
  :market "XTKS"
  :currency "JPY"
  :subcustodian-bic "MABORJPJ"
  :subcustodian-name "Mizuho Japan"
  :pset-bic "JASDECJP")
```

---

## Implementation Checklist (Updated)

### Phase 1: Core (Days 1-3)
- [ ] Migration for `cbu_trading_profiles` table
- [ ] JSON schema (full version with ISDA/CSA/Settlement)
- [ ] Rust types (all sections)
- [ ] Validation module (schema + semantic + reference checks)
- [ ] Canonicalization + hashing

### Phase 2: Document Verbs (Days 4-5)
- [ ] create/update/get/activate/retire
- [ ] import (YAML/JSON file)
- [ ] diff (version comparison)

### Phase 3: Assembly Verbs (Days 6-8)
- [ ] Universe: set-universe, add-market, add-instrument-class
- [ ] IM: assign-im
- [ ] ISDA/CSA: add-isda, add-isda-coverage, set-csa, add-eligible-collateral
- [ ] Settlement: set-matching-platform, add-settlement-identity, set-subcustodian
- [ ] Routing: add-routing-rule, add-booking-rule
- [ ] Pricing: add-pricing-rule, set-valuation

### Phase 4: Resolution (Days 9-10)
- [ ] resolve-im
- [ ] resolve-isda
- [ ] resolve-booking (ALERT-style)
- [ ] resolve-route
- [ ] resolve-pricing

### Phase 5: Materialization (Days 11-12)
- [ ] Sync to custody.cbu_instrument_universe
- [ ] Sync to custody.isda_agreements + csa_agreements
- [ ] Sync to custody.entity_settlement_identity
- [ ] Sync to custody.subcustodian_network
- [ ] Sync to custody.cbu_ssi
- [ ] Sync to custody.ssi_booking_rules
- [ ] Sync to ob-poc.investment_manager_mandates
- [ ] Sync to ob-poc.pricing_source_hierarchy
- [ ] Sync to ob-poc.valuation_schedule

### Phase 6: CLI & Tests (Days 13-14)
- [ ] xtask commands: tp-validate, tp-emit-dsl, tp-apply
- [ ] Unit tests (all modules)
- [ ] Integration tests (materialization)
- [ ] Seed files (realistic examples)

---

## Seed Example (Complete)

See: `rust/config/seed/trading_profiles/allianzgi_complete.yaml`
