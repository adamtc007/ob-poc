# 032: Corporate Actions Integration

> **Status:** TODO — For Claude Code
> **Priority:** MEDIUM (Phase 5 of trading matrix pivot)
> **Depends on:** 029 verb governance complete
> **Design source:** `ai-thoughts/027-trading-matrix-canonical-pivot.md` Phase 5

---

## Context

Corporate Actions (CA) is the last major section to integrate into the trading matrix. The YAML verbs exist (`custody/corporate-action.yaml`) but:
- DB tables don't exist
- Rust types not in `trading_matrix.rs`
- No `trading-profile.ca.*` intent verbs
- Materialize doesn't handle CA section

---

## ISO 15022/20022 Compliance

This implementation follows **ISO 15022** (SWIFT MT564-568 messages) and **ISO 20022** corporate action standards used by:
- DTCC (US)
- Euroclear/Clearstream (EU)
- All major global custodians

### Event Type Coverage (53 CAEV codes)

| Category | Count | Codes |
|----------|-------|-------|
| **Income** | 8 | DVCA, DVSE, DVOP, INTR, CAPD, CAPG, DRIP, PINK |
| **Reorganization** | 11 | MRGR, SPLF, SPLR, BONU, EXOF, CONS, CONV, PARI, REDO, DECR, SOFF |
| **Voluntary** | 7 | RHTS, RHDI, TEND, BIDS, BPUT, EXWA, NOOF |
| **Redemption** | 6 | REDM, MCAL, PCAL, PRED, DRAW, PDEF |
| **Meetings/Info** | 6 | OMET, XMET, BMET, CMET, INFO, DSCL |
| **Credit/Default** | 4 | DFLT, CREV, BRUP, LIQU |
| **Other** | 11 | ATTI, CERT, CHAN, DETI, DRCA, PPMT, REMK, TREC, WTRC, ACCU, CAPI, OTHR |

### Key Standards References

- **SWIFT MT564**: Corporate Action Notification
- **SWIFT MT565**: Corporate Action Instruction  
- **SWIFT MT566**: Corporate Action Confirmation
- **SWIFT MT567**: Corporate Action Status/Processing Advice
- **SMPG Global Market Practice**: Event/option code combinations
- **ISITC US Market Practice**: DTCC-specific guidelines

---

## Architecture Decisions (Answered)

### Q1: Where does CA live in the matrix?

**Both field + category:**

```rust
// Field for typed access
pub struct TradingMatrixDocument {
    // ... existing
    pub corporate_actions: Option<TradingMatrixCorporateActions>,
}

// Node variant for tree visualization  
pub enum TradingMatrixNodeType {
    // ... existing
    CorporateActionsPolicy(CorporateActionsPolicyNode),
}
```

### Q2: What tables does materialize write?

| Table | Schema | Written By |
|-------|--------|------------|
| `ca_event_types` | custody | `corporate-action.define-event-type` (reference, seeded) |
| `cbu_ca_preferences` | custody | `corporate-action.set-preferences` (projection) |
| `cbu_ca_instruction_windows` | custody | `corporate-action.set-instruction-window` (projection) |
| `cbu_ca_ssi_mappings` | custody | `corporate-action.link-ca-ssi` (projection) |

### Q3: Verb delegation pattern?

```
trading-profile.ca.set-election-policy  (intent → matrix JSONB)
                    │
                    ▼
         TradingMatrixDocument.corporate_actions updated
                    │
                    │ (user calls materialize)
                    ▼
         trading-profile.materialize
                    │
                    ▼
         corporate-action.set-preferences  (projection → DB)
```

**Intent verbs write matrix. Materialize calls projection verbs.**

### Q4: Materialize pipeline?

1. Read `matrix.corporate_actions`
2. Query current `cbu_ca_preferences` from DB
3. Compute diff (insert/update/delete)
4. Call internal projection verbs for each change
5. Repeat for instruction windows and SSI mappings

---

## Implementation Tasks

### Phase 1: Database Schema (~2h)

Create `migrations/021_corporate_actions.sql`:

```sql
-- ============================================================================
-- Migration 021: Corporate Actions Schema
-- ============================================================================

-- Reference catalog: CA event types (global)
CREATE TABLE IF NOT EXISTS custody.ca_event_types (
    event_type_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_code TEXT NOT NULL UNIQUE,
    event_name TEXT NOT NULL,
    category TEXT NOT NULL CHECK (category IN (
        'INCOME', 'REORGANIZATION', 'VOLUNTARY', 'MANDATORY', 'INFORMATION'
    )),
    is_elective BOOLEAN NOT NULL DEFAULT false,
    default_election TEXT CHECK (default_election IN (
        'CASH', 'STOCK', 'ROLLOVER', 'LAPSE', 'DECLINE', 'NO_ACTION'
    )),
    iso_event_code TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- CBU-specific CA preferences
CREATE TABLE IF NOT EXISTS custody.cbu_ca_preferences (
    preference_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    event_type_id UUID NOT NULL REFERENCES custody.ca_event_types(event_type_id),
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    processing_mode TEXT NOT NULL CHECK (processing_mode IN (
        'AUTO_INSTRUCT', 'MANUAL', 'DEFAULT_ONLY', 'THRESHOLD'
    )),
    default_election TEXT,
    threshold_value NUMERIC(18,4),
    threshold_currency TEXT,
    notification_email TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(cbu_id, event_type_id, instrument_class_id)
);

-- Instruction windows (deadline rules)
CREATE TABLE IF NOT EXISTS custody.cbu_ca_instruction_windows (
    window_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    event_type_id UUID NOT NULL REFERENCES custody.ca_event_types(event_type_id),
    market_id UUID REFERENCES custody.markets(market_id),
    cutoff_days_before INTEGER NOT NULL,
    warning_days INTEGER DEFAULT 3,
    escalation_days INTEGER DEFAULT 1,
    escalation_contact TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(cbu_id, event_type_id, market_id)
);

-- CA proceeds SSI mapping
CREATE TABLE IF NOT EXISTS custody.cbu_ca_ssi_mappings (
    mapping_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    event_type_id UUID NOT NULL REFERENCES custody.ca_event_types(event_type_id),
    currency TEXT NOT NULL,
    proceeds_type TEXT NOT NULL CHECK (proceeds_type IN ('CASH', 'STOCK')),
    ssi_id UUID NOT NULL REFERENCES custody.standing_settlement_instructions(ssi_id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(cbu_id, event_type_id, currency, proceeds_type)
);

-- Indexes
CREATE INDEX idx_ca_preferences_cbu ON custody.cbu_ca_preferences(cbu_id);
CREATE INDEX idx_ca_windows_cbu ON custody.cbu_ca_instruction_windows(cbu_id);
CREATE INDEX idx_ca_ssi_cbu ON custody.cbu_ca_ssi_mappings(cbu_id);

-- ============================================================================
-- ISO 15022 Corporate Action Event Types (CAEV)
-- Complete reference catalog per SWIFT/DTCC/SMPG standards
-- ============================================================================

-- INCOME EVENTS
INSERT INTO custody.ca_event_types (event_code, event_name, category, is_elective, default_election, iso_event_code) VALUES
    ('DVCA', 'Cash Dividend', 'INCOME', false, 'CASH', 'DVCA'),
    ('DVSE', 'Stock Dividend', 'INCOME', false, 'STOCK', 'DVSE'),
    ('DVOP', 'Dividend Option', 'INCOME', true, 'CASH', 'DVOP'),
    ('INTR', 'Interest Payment', 'INCOME', false, 'CASH', 'INTR'),
    ('CAPD', 'Capital Distribution', 'INCOME', false, 'CASH', 'CAPD'),
    ('CAPG', 'Capital Gains Distribution', 'INCOME', false, 'CASH', 'CAPG'),
    ('DRIP', 'Dividend Reinvestment Plan', 'INCOME', true, 'STOCK', 'DRIP'),
    ('PINK', 'Interest Payment in Kind', 'INCOME', false, 'STOCK', 'PINK')
ON CONFLICT (event_code) DO NOTHING;

-- REORGANIZATION EVENTS
INSERT INTO custody.ca_event_types (event_code, event_name, category, is_elective, default_election, iso_event_code) VALUES
    ('MRGR', 'Merger', 'REORGANIZATION', false, NULL, 'MRGR'),
    ('SPLF', 'Stock Split (Forward)', 'REORGANIZATION', false, NULL, 'SPLF'),
    ('SPLR', 'Reverse Stock Split', 'REORGANIZATION', false, NULL, 'SPLR'),
    ('BONU', 'Bonus Issue/Capitalisation Issue', 'REORGANIZATION', false, 'STOCK', 'BONU'),
    ('EXOF', 'Exchange Offer', 'REORGANIZATION', true, 'DECLINE', 'EXOF'),
    ('CONS', 'Consent', 'REORGANIZATION', true, NULL, 'CONS'),
    ('CONV', 'Conversion', 'REORGANIZATION', true, 'STOCK', 'CONV'),
    ('PARI', 'Pari-Passu', 'REORGANIZATION', false, NULL, 'PARI'),
    ('REDO', 'Redenomination', 'REORGANIZATION', false, NULL, 'REDO'),
    ('DECR', 'Decrease in Value', 'REORGANIZATION', false, NULL, 'DECR'),
    ('SOFF', 'Spin-Off', 'REORGANIZATION', false, 'STOCK', 'SOFF')
ON CONFLICT (event_code) DO NOTHING;

-- VOLUNTARY EVENTS
INSERT INTO custody.ca_event_types (event_code, event_name, category, is_elective, default_election, iso_event_code) VALUES
    ('RHTS', 'Rights Issue', 'VOLUNTARY', true, 'LAPSE', 'RHTS'),
    ('RHDI', 'Rights Distribution', 'VOLUNTARY', false, NULL, 'RHDI'),
    ('TEND', 'Tender/Takeover Offer', 'VOLUNTARY', true, 'DECLINE', 'TEND'),
    ('BIDS', 'Repurchase Offer/Issuer Bid', 'VOLUNTARY', true, 'CASH', 'BIDS'),
    ('BPUT', 'Put Redemption', 'VOLUNTARY', true, 'CASH', 'BPUT'),
    ('EXWA', 'Exercise of Warrants', 'VOLUNTARY', true, NULL, 'EXWA'),
    ('NOOF', 'Non-Official Offer', 'VOLUNTARY', true, 'DECLINE', 'NOOF')
ON CONFLICT (event_code) DO NOTHING;

-- REDEMPTION EVENTS
INSERT INTO custody.ca_event_types (event_code, event_name, category, is_elective, default_election, iso_event_code) VALUES
    ('REDM', 'Final Maturity/Redemption', 'MANDATORY', false, 'CASH', 'REDM'),
    ('MCAL', 'Full Call/Early Redemption', 'MANDATORY', false, 'CASH', 'MCAL'),
    ('PCAL', 'Partial Redemption (Nominal Reduction)', 'MANDATORY', false, 'CASH', 'PCAL'),
    ('PRED', 'Partial Redemption (No Nominal Change)', 'MANDATORY', false, 'CASH', 'PRED'),
    ('DRAW', 'Drawing', 'MANDATORY', false, 'CASH', 'DRAW'),
    ('PDEF', 'Prerefunding', 'MANDATORY', false, NULL, 'PDEF')
ON CONFLICT (event_code) DO NOTHING;

-- MEETINGS & INFORMATION EVENTS
INSERT INTO custody.ca_event_types (event_code, event_name, category, is_elective, default_election, iso_event_code) VALUES
    ('OMET', 'Ordinary General Meeting', 'INFORMATION', false, NULL, 'OMET'),
    ('XMET', 'Extraordinary General Meeting', 'INFORMATION', false, NULL, 'XMET'),
    ('BMET', 'Bondholder Meeting', 'INFORMATION', false, NULL, 'BMET'),
    ('CMET', 'Court Meeting', 'INFORMATION', false, NULL, 'CMET'),
    ('INFO', 'Information Only', 'INFORMATION', false, NULL, 'INFO'),
    ('DSCL', 'Disclosure', 'INFORMATION', false, NULL, 'DSCL')
ON CONFLICT (event_code) DO NOTHING;

-- CREDIT/DEFAULT EVENTS
INSERT INTO custody.ca_event_types (event_code, event_name, category, is_elective, default_election, iso_event_code) VALUES
    ('DFLT', 'Bond Default', 'MANDATORY', false, NULL, 'DFLT'),
    ('CREV', 'Credit Event', 'MANDATORY', false, NULL, 'CREV'),
    ('BRUP', 'Bankruptcy', 'MANDATORY', false, NULL, 'BRUP'),
    ('LIQU', 'Liquidation', 'MANDATORY', false, 'CASH', 'LIQU')
ON CONFLICT (event_code) DO NOTHING;

-- OTHER EVENTS
INSERT INTO custody.ca_event_types (event_code, event_name, category, is_elective, default_election, iso_event_code) VALUES
    ('ATTI', 'Attachment', 'MANDATORY', false, NULL, 'ATTI'),
    ('CERT', 'Certification of Beneficial Ownership', 'VOLUNTARY', true, NULL, 'CERT'),
    ('CHAN', 'Change (Name/Domicile/etc)', 'MANDATORY', false, NULL, 'CHAN'),
    ('DETI', 'Detachment of Warrants', 'MANDATORY', false, NULL, 'DETI'),
    ('DRCA', 'Non-Eligible Securities Cash Distribution', 'MANDATORY', false, 'CASH', 'DRCA'),
    ('PPMT', 'Installment Call', 'MANDATORY', false, 'CASH', 'PPMT'),
    ('REMK', 'Remarketing Agreement', 'VOLUNTARY', true, NULL, 'REMK'),
    ('TREC', 'Tax Reclaim', 'VOLUNTARY', true, 'CASH', 'TREC'),
    ('WTRC', 'Withholding Tax Relief Certification', 'VOLUNTARY', true, NULL, 'WTRC'),
    ('ACCU', 'Accumulation', 'MANDATORY', false, NULL, 'ACCU'),
    ('CAPI', 'Capitalisation', 'MANDATORY', false, NULL, 'CAPI'),
    ('OTHR', 'Other (Unclassified)', 'INFORMATION', false, NULL, 'OTHR')
ON CONFLICT (event_code) DO NOTHING;

COMMENT ON TABLE custody.ca_event_types IS 'Reference catalog of corporate action event types';
COMMENT ON TABLE custody.cbu_ca_preferences IS 'CBU-specific CA processing preferences (written by materialize)';
COMMENT ON TABLE custody.cbu_ca_instruction_windows IS 'CBU deadline/cutoff rules for CA instructions';
COMMENT ON TABLE custody.cbu_ca_ssi_mappings IS 'Which SSI receives CA proceeds (cash/stock) per currency';
```

**Verification:**
```bash
psql -d data_designer -f migrations/021_corporate_actions.sql
psql -d data_designer -c "\dt custody.cbu_ca*"
psql -d data_designer -c "SELECT * FROM custody.ca_event_types"
```

---

### Phase 2: Rust Types (~3h)

#### 2.1 Add to `ob-poc-types/src/trading_matrix.rs`

```rust
// ============================================================================
// CORPORATE ACTIONS TYPES
// ============================================================================

/// CA notification policy
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CaNotificationPolicy {
    pub channels: Vec<String>,  // email, portal, swift
    pub sla_hours: Option<i32>,
    pub escalation_contact: Option<String>,
}

/// CA election policy  
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CaElectionPolicy {
    pub elector: CaElector,
    pub evidence_required: bool,
    pub auto_instruct_threshold: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CaElector {
    #[default]
    InvestmentManager,
    Admin,
    Client,
}

/// Cutoff rule for specific market/depository
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaCutoffRule {
    pub market_code: Option<String>,
    pub depository_code: Option<String>,
    pub days_before: i32,
    pub warning_days: i32,
    pub escalation_days: i32,
}

/// Proceeds SSI mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaProceedsSsiMapping {
    pub proceeds_type: CaProceedsType,
    pub currency: Option<String>,
    pub ssi_reference: String,  // SSI name or ID
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaProceedsType {
    Cash,
    Stock,
}

/// Per-event-type default option override
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaDefaultOption {
    pub event_type: String,
    pub default_option: String,  // CASH, STOCK, ROLLOVER, LAPSE, DECLINE
}

/// Corporate Actions section of trading matrix
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TradingMatrixCorporateActions {
    pub enabled_event_types: Vec<String>,
    pub notification_policy: Option<CaNotificationPolicy>,
    pub election_policy: Option<CaElectionPolicy>,
    pub default_options: Vec<CaDefaultOption>,
    pub cutoff_rules: Vec<CaCutoffRule>,
    pub proceeds_ssi_mappings: Vec<CaProceedsSsiMapping>,
}
```

#### 2.2 Add field to TradingMatrixDocument

```rust
pub struct TradingMatrixDocument {
    // ... existing fields
    
    /// Corporate actions configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corporate_actions: Option<TradingMatrixCorporateActions>,
}
```

#### 2.3 Add node type variant

```rust
pub enum TradingMatrixNodeType {
    // ... existing variants
    
    /// Corporate actions policy node
    CorporateActionsPolicy {
        enabled_count: usize,
        has_custom_elections: bool,
        has_cutoff_rules: bool,
    },
    
    /// Individual CA event type configuration
    CaEventTypeConfig {
        event_code: String,
        event_name: String,
        processing_mode: String,
        default_option: Option<String>,
    },
}
```

**Verification:**
```bash
cargo build -p ob-poc-types
```

---

### Phase 3: Intent Verbs (~2h)

Add to `rust/config/verbs/trading-profile.yaml`:

```yaml
      # =========================================================================
      # CORPORATE ACTIONS POLICY (Intent - writes matrix JSONB)
      # =========================================================================

      ca.enable-event-types:
        description: "Enable CA event types for this trading profile"
        behavior: plugin
        handler: TradingProfileCaEnableEventTypesOp
        metadata:
          tier: intent
          source_of_truth: matrix
          scope: cbu
          noun: corporate_actions
          tags: [authoring, ca-policy]
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
          - name: event-types
            type: string_list
            required: true
            description: "Event codes: DVCA, DVOP, RHTS, TEND, etc."
        returns:
          type: record
          fields:
            - name: enabled_count
              type: integer

      ca.set-notification-policy:
        description: "Configure CA notification settings"
        behavior: plugin
        handler: TradingProfileCaSetNotificationOp
        metadata:
          tier: intent
          source_of_truth: matrix
          scope: cbu
          noun: corporate_actions
          tags: [authoring, ca-policy]
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
          - name: channels
            type: string_list
            required: true
            description: "Notification channels: email, portal, swift"
          - name: sla-hours
            type: integer
            required: false
            default: 24
          - name: escalation-contact
            type: string
            required: false
        returns:
          type: affected

      ca.set-election-policy:
        description: "Configure who makes CA elections and requirements"
        behavior: plugin
        handler: TradingProfileCaSetElectionOp
        metadata:
          tier: intent
          source_of_truth: matrix
          scope: cbu
          noun: corporate_actions
          tags: [authoring, ca-policy]
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
          - name: elector
            type: string
            required: true
            description: "Who elects: investment_manager, admin, client"
          - name: evidence-required
            type: boolean
            required: false
            default: true
          - name: auto-instruct-threshold
            type: decimal
            required: false
            description: "Value below which auto-instruct applies"
        returns:
          type: affected

      ca.set-default-option:
        description: "Set default election for specific event type"
        behavior: plugin
        handler: TradingProfileCaSetDefaultOp
        metadata:
          tier: intent
          source_of_truth: matrix
          scope: cbu
          noun: corporate_actions
          tags: [authoring, ca-policy]
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
          - name: event-type
            type: string
            required: true
            description: "Event code: DVOP, RHTS, TEND, etc."
          - name: default-option
            type: string
            required: true
            description: "CASH, STOCK, ROLLOVER, LAPSE, DECLINE"
        returns:
          type: affected

      ca.add-cutoff-rule:
        description: "Add deadline cutoff rule for market/depository"
        behavior: plugin
        handler: TradingProfileCaAddCutoffOp
        metadata:
          tier: intent
          source_of_truth: matrix
          scope: cbu
          noun: corporate_actions
          tags: [authoring, ca-policy]
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
          - name: market-code
            type: string
            required: false
            description: "MIC code (e.g., XNYS)"
          - name: depository-code
            type: string
            required: false
            description: "Depository (e.g., DTCC, CREST)"
          - name: days-before
            type: integer
            required: true
            description: "Days before market deadline"
          - name: warning-days
            type: integer
            required: false
            default: 3
          - name: escalation-days
            type: integer
            required: false
            default: 1
        returns:
          type: affected

      ca.link-proceeds-ssi:
        description: "Map CA proceeds to settlement instruction"
        behavior: plugin
        handler: TradingProfileCaLinkSsiOp
        metadata:
          tier: intent
          source_of_truth: matrix
          scope: cbu
          noun: corporate_actions
          tags: [authoring, ca-policy]
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
          - name: proceeds-type
            type: string
            required: true
            description: "cash or stock"
          - name: currency
            type: string
            required: false
            description: "Currency code (if currency-specific)"
          - name: ssi-name
            type: string
            required: true
            description: "SSI name or reference"
        returns:
          type: affected
```

---

### Phase 4: Plugin Handlers (~4h)

Create `rust/src/dsl_v2/custom_ops/trading_profile_ca_ops.rs`:

```rust
//! Corporate Actions policy handlers for trading-profile.ca.* verbs
//! 
//! These are INTENT tier handlers that mutate the matrix JSONB document.
//! They do NOT write to operational tables - that's done by materialize.

use crate::dsl_v2::{CustomOp, OpContext, OpArgs, OpResult};
use ob_poc_types::trading_matrix::{
    TradingMatrixCorporateActions, CaNotificationPolicy, CaElectionPolicy,
    CaDefaultOption, CaCutoffRule, CaProceedsSsiMapping, CaElector, CaProceedsType,
};

pub struct TradingProfileCaEnableEventTypesOp;
pub struct TradingProfileCaSetNotificationOp;
pub struct TradingProfileCaSetElectionOp;
pub struct TradingProfileCaSetDefaultOp;
pub struct TradingProfileCaAddCutoffOp;
pub struct TradingProfileCaLinkSsiOp;

#[async_trait]
impl CustomOp for TradingProfileCaEnableEventTypesOp {
    async fn execute(&self, ctx: &OpContext, args: &OpArgs) -> Result<OpResult> {
        let cbu_id = args.get_uuid("cbu-id")?;
        let event_types = args.get_string_list("event-types")?;
        
        // Load or create matrix document
        let mut matrix = ctx.load_trading_matrix(cbu_id).await?;
        
        // Ensure CA section exists
        let ca = matrix.corporate_actions.get_or_insert_with(Default::default);
        
        // Merge event types (don't duplicate)
        for et in event_types {
            if !ca.enabled_event_types.contains(&et) {
                ca.enabled_event_types.push(et);
            }
        }
        
        // Save matrix
        ctx.save_trading_matrix(cbu_id, &matrix).await?;
        
        Ok(OpResult::record(json!({
            "enabled_count": ca.enabled_event_types.len()
        })))
    }
}

// ... implement other handlers similarly
```

Register in `custom_ops/mod.rs`:
```rust
mod trading_profile_ca_ops;
pub use trading_profile_ca_ops::*;
```

---

### Phase 5: Materialize Integration (~3h)

Add to `trading_profile_ops.rs` materialize handler:

```rust
async fn materialize_corporate_actions(
    ctx: &OpContext,
    matrix: &TradingMatrixDocument,
    cbu_id: Uuid,
) -> Result<SectionDiff> {
    let Some(ca) = &matrix.corporate_actions else {
        return Ok(SectionDiff::empty("corporate_actions"));
    };
    
    let mut diff = SectionDiff::new("corporate_actions");
    
    // 1. Materialize preferences for each enabled event type
    for event_code in &ca.enabled_event_types {
        let processing_mode = determine_processing_mode(ca, event_code);
        let default_option = find_default_option(ca, event_code);
        
        // Call internal projection verb
        let result = ctx.execute_internal_verb(
            "corporate-action.set-preferences",
            json!({
                "cbu-id": cbu_id,
                "event-type": event_code,
                "processing-mode": processing_mode,
                "default-election": default_option,
            })
        ).await?;
        
        diff.add_upsert("cbu_ca_preferences", result);
    }
    
    // 2. Materialize instruction windows
    for rule in &ca.cutoff_rules {
        let result = ctx.execute_internal_verb(
            "corporate-action.set-instruction-window",
            json!({
                "cbu-id": cbu_id,
                "event-type": rule.event_type.as_deref().unwrap_or("*"),
                "market": rule.market_code,
                "cutoff-days-before": rule.days_before,
                "warning-days": rule.warning_days,
                "escalation-days": rule.escalation_days,
            })
        ).await?;
        
        diff.add_upsert("cbu_ca_instruction_windows", result);
    }
    
    // 3. Materialize SSI mappings
    for mapping in &ca.proceeds_ssi_mappings {
        let result = ctx.execute_internal_verb(
            "corporate-action.link-ca-ssi",
            json!({
                "cbu-id": cbu_id,
                "proceeds-type": mapping.proceeds_type,
                "currency": mapping.currency,
                "ssi-name": mapping.ssi_reference,
            })
        ).await?;
        
        diff.add_upsert("cbu_ca_ssi_mappings", result);
    }
    
    Ok(diff)
}
```

---

### Phase 6: Verify & Test (~2h)

```bash
# 1. Apply migration
psql -d data_designer -f migrations/021_corporate_actions.sql

# 2. Regenerate SQLx cache
cd rust && cargo sqlx prepare --workspace

# 3. Build
cargo build

# 4. Verify verbs load
cargo x verify-verbs

# 5. Test CA verbs
cargo test --features database ca

# 6. Manual integration test
./target/debug/dsl_cli execute -e '
trading-profile.ca.enable-event-types cbu-id="Test CBU" event-types=["DVCA", "DVOP", "RHTS"]
trading-profile.ca.set-election-policy cbu-id="Test CBU" elector="investment_manager" evidence-required=true
trading-profile.ca.set-default-option cbu-id="Test CBU" event-type="DVOP" default-option="CASH"
trading-profile.materialize cbu-id="Test CBU"
corporate-action.list-preferences cbu-id="Test CBU"
'
```

---

## Files to Create/Modify

| File | Action |
|------|--------|
| `migrations/021_corporate_actions.sql` | CREATE |
| `ob-poc-types/src/trading_matrix.rs` | ADD CA types |
| `config/verbs/trading-profile.yaml` | ADD ca.* verbs |
| `src/dsl_v2/custom_ops/trading_profile_ca_ops.rs` | CREATE |
| `src/dsl_v2/custom_ops/mod.rs` | REGISTER handlers |
| `src/dsl_v2/custom_ops/trading_profile_ops.rs` | ADD CA to materialize |

---

## Acceptance Criteria

- [ ] Migration creates all CA tables
- [ ] Seed data populates common event types
- [ ] `trading-profile.ca.*` verbs exist and load
- [ ] CA policy writes to matrix JSONB (not operational tables)
- [ ] Materialize reads CA from matrix, writes to operational tables
- [ ] `corporate-action.list-*` shows materialized data
- [ ] Idempotency: materialize twice produces no diff on second run

---

## Estimated Effort

| Phase | Hours |
|-------|-------|
| DB Schema | 2h |
| Rust Types | 3h |
| Intent Verbs | 2h |
| Plugin Handlers | 4h |
| Materialize Integration | 3h |
| Verify & Test | 2h |
| **Total** | **~16h** |
