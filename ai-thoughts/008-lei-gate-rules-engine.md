# LEI Gate Rules Engine

> **Purpose:** Deterministic decision table for LEI (Legal Entity Identifier) requirements across BNY service lines, activities, and jurisdictions.
> **Integration:** Wire into onboarding workflow as compliance gate before service enablement.

---

## Core Concept

LEI is collected **once** at entity onboarding, then **gate rules** enforce additional checks based on:
- **Service line** (custody, execution, prime, collateral, sec_lending)
- **Activity** (execute_trade, otc_derivatives, sft, custody_only)
- **Jurisdiction** (EU, UK, US, AU, HK)
- **Regulatory regime** (MiFIR, EMIR, CFTC, SFTR, ASIC DTR, HKTR)

---

## Gate Types

| Gate Type | Meaning | UX Impact |
|-----------|---------|-----------|
| `PRE_TRADE_HARD_STOP` | Must have LEI before executing trade | Blocks trade execution |
| `PRE_GO_LIVE_HARD_STOP` | Must have LEI before enabling service | Blocks service activation |
| `POST_TRADE_REPORTING_RISK` | Not a formal stop, but breaks reporting | Warning + escalation |
| `BEST_PRACTICE_MASTERING` | Recommended for enterprise data quality | Soft prompt |

---

## Party Types

| Party | Description |
|-------|-------------|
| `CLIENT` | The onboarded entity/fund/CBU legal entity |
| `COUNTERPARTY` | Trading counterparty (where reporting both sides) |
| `ISSUER` | Issuer of securities lent/borrowed/posted as collateral |
| `HEAD_OFFICE` | Head office LEI when acting via branches |

---

## LEI Checks

| Check | Description |
|-------|-------------|
| `lei_present` | Entity has an LEI assigned |
| `lei_active` | LEI status is ACTIVE (not LAPSED/RETIRED) |
| `iso17442_conformant` | LEI conforms to ISO 17442 format |

---

## Regulatory Anchors

| Regime | Jurisdiction | Key Requirement |
|--------|--------------|-----------------|
| **EU MiFIR/RTS 22** | EU | "No LEI, no trade" - hard stop before execution |
| **UK MiFIR** | UK | Same as EU - cannot execute for LEI-eligible client without LEI |
| **EU EMIR** | EU | LEI required for derivative counterparties in reporting |
| **UK EMIR** | UK | Validation rules specify ISO 17442 LEI |
| **US CFTC Part 45** | US | LEI required for swap counterparties, must be ISO 17442 |
| **EU SFTR** | EU | LEI for SFT counterparties + issuers of collateral securities |
| **AU ASIC DTR 2024** | AU | LEI per ISO 17442 where entity has one |
| **HK HKTR** | HK | ISO 20022 + CDE alignment, LEI prioritized |

---

## Decision Matrix

| Service Line | Activity | Jurisdiction | Gate Type | Required LEI |
|--------------|----------|--------------|-----------|--------------|
| Execution | execute_trade | EU | `PRE_TRADE_HARD_STOP` | CLIENT |
| Execution | execute_trade | UK | `PRE_TRADE_HARD_STOP` | CLIENT |
| Prime | otc_derivatives | EU | `PRE_GO_LIVE_HARD_STOP` | CLIENT, COUNTERPARTY |
| Prime | otc_derivatives | UK | `PRE_GO_LIVE_HARD_STOP` | CLIENT, COUNTERPARTY |
| Prime | otc_derivatives | US | `PRE_GO_LIVE_HARD_STOP` | CLIENT, COUNTERPARTY |
| Collateral | otc_derivatives | EU/UK/US | `PRE_GO_LIVE_HARD_STOP` | CLIENT |
| Sec Lending | sft | EU | `PRE_GO_LIVE_HARD_STOP` | CLIENT, COUNTERPARTY, ISSUER |
| Prime | otc_derivatives | AU | `PRE_GO_LIVE_HARD_STOP` | CLIENT, COUNTERPARTY |
| Prime | otc_derivatives | HK | `PRE_GO_LIVE_HARD_STOP` | CLIENT, COUNTERPARTY |
| Custody | custody_only | ANY | `BEST_PRACTICE_MASTERING` | CLIENT (recommended) |
| Any (branch) | any | ANY | `POST_TRADE_REPORTING_RISK` | HEAD_OFFICE |

---

## Rules Configuration (YAML)

```yaml
version: 1

enums:
  gate_type:
    - PRE_TRADE_HARD_STOP
    - PRE_GO_LIVE_HARD_STOP
    - POST_TRADE_REPORTING_RISK
    - BEST_PRACTICE_MASTERING
  
  party:
    - CLIENT
    - COUNTERPARTY
    - ISSUER
    - HEAD_OFFICE
  
  requiredness:
    - REQUIRED
    - REQUIRED_IF_EXISTS
    - RECOMMENDED

rules:
  # === EU/UK MiFIR - No LEI No Trade ===
  - id: EU_MIFIR_NO_LEI_NO_TRADE
    service_lines: [execution, prime]
    activities: [execute_trade]
    jurisdictions: [EU]
    regimes: [EU_MIFIR_RTS22]
    gate_type: PRE_TRADE_HARD_STOP
    lei_requirements:
      - party: CLIENT
        requiredness: REQUIRED
        checks: [lei_present, lei_active]

  - id: UK_MIFIR_NO_LEI_NO_TRADE
    service_lines: [execution, prime]
    activities: [execute_trade]
    jurisdictions: [UK]
    regimes: [UK_MIFIR]
    gate_type: PRE_TRADE_HARD_STOP
    lei_requirements:
      - party: CLIENT
        requiredness: REQUIRED
        checks: [lei_present, lei_active]

  # === EMIR Derivatives Reporting ===
  - id: EU_EMIR_DERIVATIVES
    service_lines: [prime, collateral]
    activities: [otc_derivatives]
    jurisdictions: [EU]
    regimes: [EU_EMIR]
    gate_type: PRE_GO_LIVE_HARD_STOP
    lei_requirements:
      - party: CLIENT
        requiredness: REQUIRED
        checks: [lei_present, lei_active]
      - party: COUNTERPARTY
        requiredness: REQUIRED
        checks: [lei_present]

  - id: UK_EMIR_DERIVATIVES
    service_lines: [prime, collateral]
    activities: [otc_derivatives]
    jurisdictions: [UK]
    regimes: [UK_EMIR]
    gate_type: PRE_GO_LIVE_HARD_STOP
    lei_requirements:
      - party: CLIENT
        requiredness: REQUIRED
        checks: [lei_present, lei_active]
      - party: COUNTERPARTY
        requiredness: REQUIRED
        checks: [lei_present]

  # === US CFTC Swaps ===
  - id: US_CFTC_SWAPS
    service_lines: [prime, collateral]
    activities: [otc_derivatives]
    jurisdictions: [US]
    regimes: [US_CFTC_PART45]
    gate_type: PRE_GO_LIVE_HARD_STOP
    lei_requirements:
      - party: CLIENT
        requiredness: REQUIRED
        checks: [lei_present, lei_active, iso17442_conformant]
      - party: COUNTERPARTY
        requiredness: REQUIRED
        checks: [lei_present, iso17442_conformant]

  # === EU SFTR Securities Lending ===
  - id: EU_SFTR_SFT
    service_lines: [sec_lending, collateral]
    activities: [sft]
    jurisdictions: [EU]
    regimes: [EU_SFTR]
    gate_type: PRE_GO_LIVE_HARD_STOP
    lei_requirements:
      - party: CLIENT
        requiredness: REQUIRED
        checks: [lei_present, lei_active]
      - party: COUNTERPARTY
        requiredness: REQUIRED
        checks: [lei_present]
      - party: ISSUER
        requiredness: REQUIRED_IF_EXISTS
        checks: [lei_present]

  # === Australia ASIC ===
  - id: AU_ASIC_OTC
    service_lines: [prime, collateral]
    activities: [otc_derivatives]
    jurisdictions: [AU]
    regimes: [AU_ASIC_DTR_2024]
    gate_type: PRE_GO_LIVE_HARD_STOP
    lei_requirements:
      - party: CLIENT
        requiredness: REQUIRED
        checks: [lei_present, iso17442_conformant]
      - party: COUNTERPARTY
        requiredness: REQUIRED
        checks: [lei_present, iso17442_conformant]

  # === Hong Kong HKTR ===
  - id: HK_HKTR_OTC
    service_lines: [prime, collateral]
    activities: [otc_derivatives]
    jurisdictions: [HK]
    regimes: [HK_HKTR]
    gate_type: PRE_GO_LIVE_HARD_STOP
    lei_requirements:
      - party: CLIENT
        requiredness: REQUIRED
        checks: [lei_present, iso17442_conformant]
      - party: COUNTERPARTY
        requiredness: REQUIRED
        checks: [lei_present, iso17442_conformant]
      - party: HEAD_OFFICE
        requiredness: RECOMMENDED
        checks: [lei_present]

  # === Custody Default ===
  - id: CUSTODY_MASTERING
    service_lines: [custody]
    activities: [custody_only]
    jurisdictions: [EU, UK, US, AU, HK, OTHER]
    regimes: []
    gate_type: BEST_PRACTICE_MASTERING
    lei_requirements:
      - party: CLIENT
        requiredness: RECOMMENDED
        checks: [lei_present]

  # === Branch Policy ===
  - id: BRANCH_HEAD_OFFICE
    conditions:
      is_branch: true
    gate_type: POST_TRADE_REPORTING_RISK
    lei_requirements:
      - party: HEAD_OFFICE
        requiredness: REQUIRED
        checks: [lei_present]
```

---

## Implementation Notes

### SFTR Issuer LEI Pain Point
ESMA has issued statements about third-country issuer LEI implementation challenges. Support three modes:
- `REQUIRED` - block if missing
- `REQUIRED_IF_EXISTS` - require only if issuer has LEI
- `BEST_EFFORT` - log warning, allow proceed

### LEI Renewal (UK MiFIR)
FCA notes annual renewal requirement. Build:
- LEI renewal status check
- Escalation workflow for lapsed LEIs
- Grace period handling (typically 30 days)

### Evaluation Function Signature

```rust
pub struct LeiGateContext {
    pub service_line: ServiceLine,
    pub activity: Activity,
    pub jurisdiction: Jurisdiction,
    pub regime_flags: HashSet<Regime>,
    pub client_is_lei_eligible: bool,
    pub is_branch: bool,
    pub client_lei: Option<LeiRecord>,
    pub counterparty_lei: Option<LeiRecord>,
    pub issuer_lei: Option<LeiRecord>,
    pub head_office_lei: Option<LeiRecord>,
}

pub struct LeiRecord {
    pub lei: String,
    pub status: LeiStatus,  // ACTIVE, LAPSED, RETIRED
    pub next_renewal: Option<DateTime<Utc>>,
}

pub enum GateFinding {
    Pass,
    HardStop { rule_id: String, message: String, missing: Vec<Party> },
    Warning { rule_id: String, message: String },
    Recommendation { rule_id: String, message: String },
}

/// Evaluate LEI gates for onboarding context
pub fn evaluate_lei_gates(ctx: &LeiGateContext) -> Vec<GateFinding>;
```

---

## Sources

- EU RTS 22 "no LEI, no trade" language
- FCA UK MiFIR LEI pre-execution requirement
- ESMA EMIR guidance "LEI codes should be used"
- UK EMIR validation rules specifying ISO 17442 LEI
- CFTC Part 45 LEI requirement + ISO 17442 conformity
- ESMA SFTR third-country issuer LEI acceptance timeline
- ASIC 2024 rules using LEI per ISO 17442
- HKTR supplementary instructions prioritizing LEI
- HKMA/SFC ISO20022/CDE reform documentation
- ROC consolidated view of regulatory LEI uses
