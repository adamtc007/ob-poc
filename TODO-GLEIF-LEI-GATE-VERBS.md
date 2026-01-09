# TODO: GLEIF DSL Enhancements - LEI Gate Rules Integration

> **Priority:** MEDIUM - Required for compliance-aware onboarding
> **Depends on:** LEI Gate Rules Engine (see `ai-thoughts/008-lei-gate-rules-engine.md`)
> **Status:** New feature

## Current GLEIF Verbs

| Verb | Purpose |
|------|---------|
| `gleif.enrich` | Fetch LEI data for entity |
| `gleif.import-tree` | Import corporate ownership tree |
| `gleif.refresh` | Update/resync LEI data |
| `gleif.trace-ownership` | Trace UBO chain via GLEIF parents |

---

## New Verbs Required

### 1. `gleif.validate` - LEI Validation Checks

Validate LEI against compliance requirements.

```
(gleif.validate :entity @apex :checks [lei_present lei_active iso17442_conformant])
```

**Parameters:**
- `:entity` - Entity reference to validate
- `:checks` - List of checks to perform

**Checks:**
| Check | Description | Returns |
|-------|-------------|---------|
| `lei_present` | Entity has LEI assigned | bool |
| `lei_active` | LEI status is ACTIVE | bool |
| `lei_lapsed` | LEI status is LAPSED | bool |
| `lei_retired` | LEI status is RETIRED | bool |
| `iso17442_conformant` | LEI matches ISO 17442 format | bool |
| `renewal_due` | LEI renewal due within 30 days | bool |
| `renewal_overdue` | LEI renewal overdue | bool |

**Returns:**
```rust
pub struct LeiValidation {
    pub entity_id: Uuid,
    pub lei: Option<String>,
    pub status: Option<LeiStatus>,
    pub checks: HashMap<String, bool>,
    pub next_renewal: Option<DateTime<Utc>>,
    pub last_updated: Option<DateTime<Utc>>,
    pub issues: Vec<LeiIssue>,
}

pub enum LeiStatus {
    Active,
    Lapsed,
    Retired,
    Pending,
    Unknown,
}

pub struct LeiIssue {
    pub check: String,
    pub severity: Severity,  // Error, Warning, Info
    pub message: String,
}
```

---

### 2. `gleif.check-gate` - LEI Gate Evaluation

Evaluate LEI compliance gate for service enablement.

```
(gleif.check-gate 
  :entity @apex 
  :service_line execution 
  :activity execute_trade 
  :jurisdiction EU)
```

**Parameters:**
- `:entity` - Entity to check
- `:service_line` - custody | execution | prime | collateral | sec_lending
- `:activity` - custody_only | execute_trade | otc_derivatives | sft
- `:jurisdiction` - EU | UK | US | AU | HK | OTHER

**Returns:**
```rust
pub struct GateResult {
    pub passed: bool,
    pub gate_type: GateType,
    pub rule_id: String,
    pub findings: Vec<GateFinding>,
    pub required_parties: Vec<PartyRequirement>,
    pub message: String,
}

pub enum GateType {
    PreTradeHardStop,
    PreGoLiveHardStop,
    PostTradeReportingRisk,
    BestPracticeMastering,
}

pub struct PartyRequirement {
    pub party: Party,  // Client, Counterparty, Issuer, HeadOffice
    pub requiredness: Requiredness,  // Required, RequiredIfExists, Recommended
    pub checks: Vec<String>,
    pub satisfied: bool,
}

pub enum GateFinding {
    Pass,
    HardStop { missing: Vec<Party>, message: String },
    Warning { message: String },
    Recommendation { message: String },
}
```

---

### 3. `gleif.check-renewal` - LEI Renewal Status

Check LEI renewal status and upcoming expirations.

```
(gleif.check-renewal :entity @apex)
(gleif.check-renewal :cbu @client :threshold 30d)
```

**Parameters:**
- `:entity` or `:cbu` - What to check
- `:threshold` - Days ahead to check (default 30)

**Returns:**
```rust
pub struct RenewalStatus {
    pub entity_id: Uuid,
    pub lei: String,
    pub status: LeiStatus,
    pub next_renewal: DateTime<Utc>,
    pub days_until_renewal: i32,
    pub is_overdue: bool,
    pub is_due_soon: bool,  // within threshold
    pub last_renewed: Option<DateTime<Utc>>,
}
```

---

### 4. `gleif.bulk-validate` - Batch LEI Validation

Validate LEIs for all entities in a CBU or list.

```
(gleif.bulk-validate :cbu @client)
(gleif.bulk-validate :entities [@entity1 @entity2 @entity3])
```

**Returns:**
```rust
pub struct BulkValidation {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub missing_lei: Vec<EntityRef>,
    pub lapsed_lei: Vec<EntityRef>,
    pub renewal_due: Vec<EntityRef>,
    pub results: Vec<LeiValidation>,
}
```

---

## DSL Integration

### Service Enablement Workflow

When enabling a service on a CBU, automatically check LEI gates:

```
# User wants to enable execution service
(service.enable :cbu @client :service execution :jurisdiction EU)

# System automatically runs:
# 1. (gleif.check-gate :entity @client.anchor :service_line execution :activity execute_trade :jurisdiction EU)
# 2. If PRE_TRADE_HARD_STOP or PRE_GO_LIVE_HARD_STOP fails -> block enablement
# 3. If POST_TRADE_REPORTING_RISK -> warn but allow
# 4. If BEST_PRACTICE_MASTERING -> soft prompt
```

### Onboarding Checklist Integration

Add LEI gate status to onboarding checklist:

```rust
pub struct OnboardingChecklist {
    // ... existing fields
    pub lei_gates: Vec<LeiGateStatus>,
}

pub struct LeiGateStatus {
    pub service_line: ServiceLine,
    pub jurisdiction: Jurisdiction,
    pub gate_type: GateType,
    pub passed: bool,
    pub blocking: bool,
    pub message: String,
}
```

---

## RAG Metadata (verb_rag_metadata.rs)

Add to `verb_rag_metadata.rs`:

```rust
m.insert(
    "gleif.validate",
    vec![
        "validate lei",
        "check lei status",
        "is lei active",
        "lei compliance check",
        "verify lei",
        "lei validation",
        "check lei conformance",
        "lei status check",
        "is lei valid",
        "lei expiry check",
    ],
);

m.insert(
    "gleif.check-gate",
    vec![
        "check lei gate",
        "lei compliance gate",
        "can we trade",
        "no lei no trade",
        "lei requirement check",
        "mifir lei check",
        "emir lei check",
        "sftr lei check",
        "lei gate evaluation",
        "service enablement check",
        "lei hard stop",
        "pre trade lei check",
    ],
);

m.insert(
    "gleif.check-renewal",
    vec![
        "lei renewal status",
        "when does lei expire",
        "lei expiry date",
        "lei renewal due",
        "check lei expiration",
        "lei lapsed",
        "lei needs renewal",
        "annual lei renewal",
    ],
);

m.insert(
    "gleif.bulk-validate",
    vec![
        "validate all leis",
        "bulk lei check",
        "cbu lei validation",
        "check all entity leis",
        "lei audit",
        "lei compliance scan",
        "lei health check",
    ],
);
```

---

## Files to Modify

| File | Changes |
|------|---------|
| `rust/src/session/verb_rag_metadata.rs` | Add RAG metadata for new verbs |
| `rust/crates/dsl-core/src/grammar.rs` | Add verb definitions |
| `rust/crates/dsl-core/src/parser.rs` | Add parsers |
| `rust/crates/dsl-core/src/ast.rs` | Add AST nodes |
| `rust/src/dsl_v2/executor.rs` | Add execution logic |
| `rust/src/services/gleif_service.rs` | Add validation/gate logic |
| NEW: `rust/src/services/lei_gate_engine.rs` | Gate rules evaluation engine |
| NEW: `data/lei_gate_rules.yaml` | Gate rules configuration |

---

## Acceptance Criteria

- [ ] `gleif.validate` returns comprehensive LEI status for entity
- [ ] `gleif.check-gate` evaluates correct rule based on service/activity/jurisdiction
- [ ] `PRE_TRADE_HARD_STOP` actually blocks trade execution
- [ ] `PRE_GO_LIVE_HARD_STOP` blocks service enablement
- [ ] `gleif.check-renewal` identifies LEIs due for renewal
- [ ] `gleif.bulk-validate` processes all entities in CBU
- [ ] Gate rules loadable from YAML configuration
- [ ] Agent can ask "can we trade equities in EU for this client?" and get accurate answer

---

## Example Agent Interactions

```
User: "Can we enable execution for Allianz in the EU?"

Agent: (gleif.check-gate :entity @allianz.anchor :service_line execution :activity execute_trade :jurisdiction EU)

Response: "PRE_TRADE_HARD_STOP: EU MiFIR requires LEI before execution. 
           Allianz SE has LEI 529900K9WJHPHV2Q2L79 (ACTIVE). 
           ✓ Gate passed - execution can be enabled."
```

```
User: "Check LEI compliance for the whole Allianz group"

Agent: (gleif.bulk-validate :cbu @allianz)

Response: "Validated 47 entities:
           - 42 have active LEIs ✓
           - 3 have lapsed LEIs ⚠️ (Allianz Suisse, Allianz Taiwan, Allianz Morocco)
           - 2 missing LEIs ✗ (special purpose vehicles)
           
           3 LEIs due for renewal within 30 days."
```

---

## Dependencies

- LEI Gate Rules YAML configuration loaded at startup
- GLEIF API integration (existing) for status lookups
- CBU entity membership for bulk operations
