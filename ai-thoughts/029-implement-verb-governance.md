# 029: Implement Verb Lexicon Governance

> **Status:** TODO — Ready for Claude Code
> **Priority:** HIGH
> **Effort:** ~20-30 hours across 3 phases
> **Depends on:** 028 (design spec)
> **Design doc:** `ai-thoughts/028-verb-lexicon-governance.md`

---

## Overview

This is the implementation plan for 028. Each phase has discrete tasks with file paths, code snippets, and verification steps.

---

## Plan Quality Gate (Audit Checklist)

This section explicitly states what the refactor accomplishes and how drift is prevented. **Auditors should verify these commitments are met before accepting the PR.**

### 1. Metadata Contract: Why It's Mandatory (Not Decoration)

Verb metadata is an **API lifecycle tool**:

| Purpose | How Metadata Enables It |
|---------|-------------------------|
| Deprecation + migration | `status: deprecated` + `replaced_by: <verb>` enables phased removal |
| Structured removal schedules | `removal_version` field → Kubernetes-style deprecation policy |
| Strictness tiers for linting | Buf-style MINIMAL/BASIC/STANDARD → gradual enforcement rollout |
| SemVer expectations | Marking public API deprecated is a versioned event, not casual |

**Enforcement:** Missing required metadata → `cargo x verify-verbs` fails (Phase 2+).

---

### 2. Mechanical Inventory: Kill List

This is the explicit enumeration of verb fate. **No verb escapes classification.**

#### 2a. Verbs Removed Outright

| Domain | Verb | Reason |
|--------|------|--------|
| (none yet) | | Phase 3 may identify candidates after migration |

#### 2b. Verbs Deprecated + replaced_by

| Domain | Verb | Replaced By |
|--------|------|-------------|
| `instruction-profile` | `assign-template` | `trading-profile.add-standing-instruction` |
| `instruction-profile` | `override-field` | `trading-profile.set-ssi-override` |
| `instruction-profile` | `remove-assignment` | `trading-profile.remove-standing-instruction` |
| `instruction-profile` | `bulk-assign` | `trading-profile.import` (with SSI section) |
| `cbu-custody` | `add-instrument` | `trading-profile.add-instrument` |
| `cbu-custody` | `add-market` | `trading-profile.add-market` |
| `cbu-custody` | `add-universe` | `trading-profile.set-universe` |
| `cbu-custody` | `remove-instrument` | `trading-profile.remove-instrument` |
| `trade-gateway` | `assign-gateway` | `trading-profile.add-gateway` |
| `trade-gateway` | `set-routing` | `trading-profile.set-gateway-routing` |
| `settlement-chain` | `assign-chain` | `trading-profile.add-settlement-chain` |
| `pricing-config` | `set-source` | `trading-profile.set-pricing-source` |
| `tax-config` | `set-treatment` | `trading-profile.set-tax-treatment` |
| `corporate-action` | `set-preference` | `trading-profile.ca.set-election-policy` |

#### 2c. Verbs Becoming Internal Projection-Only

| Domain | Verb | Called By |
|--------|------|----------|
| `cbu-custody` | `_write-instrument` | `trading-profile.materialize` |
| `cbu-custody` | `_write-market` | `trading-profile.materialize` |
| `cbu-custody` | `_write-universe` | `trading-profile.materialize` |
| `instruction-profile` | `_write-assignment` | `trading-profile.materialize` |
| `trade-gateway` | `_write-routing` | `trading-profile.materialize` |
| `settlement-chain` | `_write-chain` | `trading-profile.materialize` |
| `pricing-config` | `_write-source` | `trading-profile.materialize` |
| `tax-config` | `_write-treatment` | `trading-profile.materialize` |
| `corporate-action` | `_write-preference` | `trading-profile.materialize` |

**Convention:** Internal projection verbs prefixed with `_` to signal non-public.

#### 2d. Verbs Remaining Reference/Catalog (Unchanged)

| Domain | Verb Pattern | Source of Truth |
|--------|--------------|----------------|
| `refdata.*` | `define-*`, `list-*` | catalog |
| `reference.*` | `define-*`, `list-*` | catalog |
| `admin.*` | `define-*`, `list-*` | catalog |
| `instruction-profile` | `define-template`, `list-templates` | catalog |
| `trade-gateway` | `define-gateway`, `list-gateways` | catalog |
| `settlement-chain` | `define-chain`, `list-chains` | catalog |

#### 2e. Verbs Remaining Diagnostics (Read-Only)

| Domain | Verb Pattern |
|--------|-------------|
| `cbu-custody` | `list-instruments`, `list-markets`, `get-universe` |
| `instruction-profile` | `list-assignments`, `get-assignment` |
| `trade-gateway` | `list-routing`, `get-routing` |
| `settlement-chain` | `list-cbu-chains`, `get-cbu-chain` |
| `pricing-config` | `list-sources`, `get-source` |
| `tax-config` | `list-treatments`, `get-treatment` |
| `corporate-action` | `list-preferences`, `get-preference` |

---

### 3. Hard Enforcement Points

| Enforcement | Mechanism | When Active |
|-------------|-----------|-------------|
| **Loader validation** | Missing required metadata → `Err()` | Phase 2 |
| **Linter MINIMAL** | Required fields present | Phase 1 (warn) |
| **Linter BASIC** | Naming + semantics + deprecation coherence | Phase 2 (fail) |
| **Linter STANDARD** | Single authoring surface, projection rules | Phase 3 (fail) |
| **Runtime flag** | `ALLOW_DEPRECATED_VERBS=false` → block execution | Phase 3 (opt-in) |
| **CI gate** | `cargo x verbs lint --tier standard` in PR checks | Phase 3 |

---

### 4. Instruction-Profile Leak Closure

**Decision: Compat Alias Approach**

Rationale: Faster migration, existing DSL scripts don't break immediately.

| Old Verb | Behavior | New Target |
|----------|----------|------------|
| `instruction-profile.assign-template` | Alias → rewrite to matrix mutation | `trading-profile.add-standing-instruction` |
| `instruction-profile.override-field` | Alias → rewrite to matrix mutation | `trading-profile.set-ssi-override` |
| `instruction-profile.remove-assignment` | Alias → rewrite to matrix mutation | `trading-profile.remove-standing-instruction` |

**Implementation:**
```rust
// In executor, when verb.metadata.status == Deprecated:
// 1. Log warning: "deprecated; use X"
// 2. If alias_handler present, delegate to alias target
// 3. If ALLOW_DEPRECATED_VERBS=false, return Err
```

**Read-only verbs remain:** `list-assignments`, `get-assignment`, `list-templates`, `get-template` stay as `tier: diagnostics` pointing at operational tables.

---

### 5. Materialize Plan/Apply Separation

| Function | Purity | DB Writes | Returns |
|----------|--------|-----------|--------|
| `generate-materialization-plan` | Pure | None | Structured diff (JSON) |
| `apply-materialization-plan` | Transactional | Yes | Row counts + audit ID |
| `materialize` | Orchestrator | Calls both | Convenience wrapper |

**Contract:**
- `generate-materialization-plan` can be called N times with no side effects
- `apply-materialization-plan` is idempotent (same plan → same result)
- `materialize` = `generate` + `apply` in one call (for simple cases)

---

### 6. Tests That Prove the Pivot

| Test | What It Proves | File |
|------|----------------|------|
| `materialize_is_idempotent` | Run twice, second run has zero changes | `tests/materialize_idempotency.rs` |
| `linter_catches_ops_intent` | New verb with `writes_operational + tier:intent` fails | `tests/linter_rules.rs` |
| `ca_policy_materialize` | CA policy in matrix → operational tables populated | `tests/ca_materialize.rs` |
| `ssi_reference_integrity` | SSI refs in matrix → correct assignment rows | `tests/ssi_materialize.rs` |
| `deprecated_verb_warning` | Deprecated verb logs warning, executes | `tests/deprecation.rs` |
| `deprecated_verb_blocked` | With flag, deprecated verb returns Err | `tests/deprecation.rs` |

---

## Domains/Verbs Touched (Summary)

| Domain | Files Modified | Verbs Affected |
|--------|----------------|----------------|
| `instruction-profile` | `custody/instruction-profile.yaml` | 8 verbs (4 deprecated, 4 internal) |
| `cbu-custody` | `custody/cbu-custody.yaml` | 12 verbs (4 deprecated, 4 internal, 4 diagnostics) |
| `trade-gateway` | `custody/trade-gateway.yaml` | 6 verbs (2 deprecated, 2 internal, 2 diagnostics) |
| `settlement-chain` | `custody/settlement-chain.yaml` | 6 verbs (2 deprecated, 2 internal, 2 diagnostics) |
| `pricing-config` | `pricing-config.yaml` | 4 verbs (1 deprecated, 1 internal, 2 diagnostics) |
| `tax-config` | `custody/tax-config.yaml` | 4 verbs (1 deprecated, 1 internal, 2 diagnostics) |
| `corporate-action` | `custody/corporate-action.yaml` | 6 verbs (2 deprecated, 2 internal, 2 diagnostics) |
| `trading-profile` | `trading-profile.yaml` | +15 new verbs (CA policy, plan/apply) |

**Total:** ~46 existing verbs reclassified + 15 new verbs added

---

## Deprecation/Alias Mapping (Machine-Readable)

```yaml
# Can be used to auto-generate alias handlers
deprecation_map:
  - old: instruction-profile.assign-template
    new: trading-profile.add-standing-instruction
    arg_map:
      cbu-id: cbu-id
      template-id: template-id
      # new verb expects these in matrix context
    
  - old: instruction-profile.override-field
    new: trading-profile.set-ssi-override
    arg_map:
      cbu-id: cbu-id
      assignment-id: ssi-id
      field: field
      value: value
      
  - old: cbu-custody.add-instrument
    new: trading-profile.add-instrument
    arg_map:
      cbu-id: cbu-id
      instrument-class: instrument-class
      
  - old: cbu-custody.add-market
    new: trading-profile.add-market
    arg_map:
      cbu-id: cbu-id
      market-code: market-code
      
  - old: trade-gateway.assign-gateway
    new: trading-profile.add-gateway
    arg_map:
      cbu-id: cbu-id
      gateway-id: gateway-id
      
  - old: settlement-chain.assign-chain
    new: trading-profile.add-settlement-chain
    arg_map:
      cbu-id: cbu-id
      chain-id: chain-id
      market-code: market-code
      
  - old: corporate-action.set-preference
    new: trading-profile.ca.set-election-policy
    arg_map:
      cbu-id: cbu-id
      event-type: event-type
      default-option: default-option
```

---

## Phase 1: Schema + Warnings (~8 hours)

### 1.1 Extend VerbMetadata struct

**File:** `rust/crates/dsl-core/src/config/types.rs`

Add new fields to `VerbMetadata`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VerbMetadata {
    // Existing fields
    pub tier: Option<VerbTier>,
    pub source_of_truth: Option<SourceOfTruth>,
    pub scope: Option<VerbScope>,
    pub writes_operational: Option<bool>,
    pub internal: Option<bool>,
    pub noun: Option<String>,
    pub tags: Option<Vec<String>>,
    
    // NEW: Lifecycle fields
    #[serde(default)]
    pub status: VerbStatus,           // active | deprecated
    pub replaced_by: Option<String>,  // canonical verb name
    pub since_version: Option<String>,
    pub removal_version: Option<String>,
    
    // NEW: Behavioral flags
    #[serde(default)]
    pub dangerous: bool,              // for delete on regulated nouns
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum VerbStatus {
    #[default]
    Active,
    Deprecated,
}
```

**Verify:** `cargo build -p dsl-core`

---

### 1.2 Add lint severity levels

**File:** `rust/src/session/verb_tiering_linter.rs`

Add lint tier enum and update linter:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LintTier {
    Minimal,   // Required fields present
    Basic,     // Naming + semantics
    Standard,  // Matrix-first enforcement
}

#[derive(Debug, Clone)]
pub struct LintConfig {
    pub tier: LintTier,
    pub fail_on_warning: bool,
}

impl Default for LintConfig {
    fn default() -> Self {
        Self {
            tier: LintTier::Minimal,
            fail_on_warning: false,
        }
    }
}
```

**Verify:** `cargo build --features database`

---

### 1.3 Implement MINIMAL lint rules

**File:** `rust/src/session/verb_tiering_linter.rs`

Add these rules (warn only in Phase 1):

| Rule ID | Check | Severity |
|---------|-------|----------|
| M001 | `metadata` block present | Warn |
| M002 | `tier` field present and valid | Warn |
| M003 | `source_of_truth` field present | Warn |
| M004 | `scope` field present | Warn |
| M005 | `noun` field present | Warn |
| M006 | If `status: deprecated`, `replaced_by` should be present | Warn |

```rust
fn check_minimal_rules(&self, verb: &VerbDefinition, domain: &str) -> Vec<LintViolation> {
    let mut violations = vec![];
    
    let meta = verb.metadata.as_ref();
    
    if meta.is_none() {
        violations.push(LintViolation {
            rule: "M001",
            severity: Severity::Warning,
            message: format!("{}:{} missing metadata block", domain, verb.name),
        });
        return violations; // Can't check other rules
    }
    
    let meta = meta.unwrap();
    
    if meta.tier.is_none() {
        violations.push(LintViolation {
            rule: "M002",
            severity: Severity::Warning,
            message: format!("{}:{} missing metadata.tier", domain, verb.name),
        });
    }
    
    // ... similar for M003-M006
    
    violations
}
```

**Verify:** `cargo x verbs lint --tier minimal`

---

### 1.4 Add xtask verb-inventory command

**File:** `rust/xtask/src/main.rs` (add subcommand)
**New file:** `rust/xtask/src/verb_inventory.rs`

Generate `docs/verb_inventory.md`:

```rust
pub fn generate_inventory(output_path: &Path) -> Result<()> {
    let verbs = load_all_verbs()?;
    
    let mut md = String::new();
    writeln!(md, "# Verb Inventory\n")?;
    writeln!(md, "> Auto-generated by `cargo x verb-inventory`\n")?;
    writeln!(md, "> Generated: {}\n", Utc::now().format("%Y-%m-%d %H:%M UTC"))?;
    
    // Domain summary table
    writeln!(md, "## Domain Summary\n")?;
    writeln!(md, "| Domain | Verbs | Intent | Projection | Diagnostics | Reference | Composite |")?;
    writeln!(md, "|--------|-------|--------|------------|-------------|-----------|-----------|")?;
    
    for (domain, domain_verbs) in &verbs.by_domain() {
        let counts = count_by_tier(domain_verbs);
        writeln!(md, "| {} | {} | {} | {} | {} | {} | {} |",
            domain, domain_verbs.len(),
            counts.intent, counts.projection, counts.diagnostics,
            counts.reference, counts.composite
        )?;
    }
    
    // Source of truth breakdown
    writeln!(md, "\n## Source of Truth Breakdown\n")?;
    // ...
    
    fs::write(output_path, md)?;
    Ok(())
}
```

**Verify:** `cargo x verb-inventory && cat docs/verb_inventory.md`

---

### 1.5 Update CLAUDE.md header stats automatically

**File:** `rust/xtask/src/verb_inventory.rs`

Add `--update-claude-md` flag:

```rust
pub fn update_claude_md(stats: &VerbStats) -> Result<()> {
    let path = Path::new("CLAUDE.md");
    let content = fs::read_to_string(path)?;
    
    // Update verb count line
    let updated = regex_replace(
        &content,
        r#"> \*\*Verb count:\*\* [^\n]+"#,
        &format!("> **Verb count:** ~{} verbs across {} YAML files", 
            stats.total_verbs, stats.file_count)
    );
    
    fs::write(path, updated)?;
    Ok(())
}
```

**Verify:** `cargo x verb-inventory --update-claude-md && head -10 CLAUDE.md`

---

### Phase 1 Verification Checklist

```bash
# All should pass
cargo build -p dsl-core
cargo build --features database
cargo x verbs lint --tier minimal  # Warnings OK, no errors
cargo x verb-inventory
cat docs/verb_inventory.md | head -30
```

---

## Phase 2: Mandatory Metadata + BASIC Lints (~10 hours)

### 2.1 Make metadata required (hard fail)

**File:** `rust/src/dsl_v2/verb_loader.rs`

Change from warn to error:

```rust
pub fn load_verb_file(path: &Path) -> Result<Vec<VerbDefinition>> {
    let verbs = parse_yaml(path)?;
    
    for verb in &verbs {
        if verb.metadata.is_none() {
            return Err(anyhow!(
                "Verb {}:{} missing required metadata block. See docs/verb-definition-spec.md",
                verb.domain, verb.name
            ));
        }
        
        let meta = verb.metadata.as_ref().unwrap();
        if meta.tier.is_none() {
            return Err(anyhow!(
                "Verb {}:{} missing required metadata.tier",
                verb.domain, verb.name
            ));
        }
        // ... check other required fields
    }
    
    Ok(verbs)
}
```

**Verify:** `cargo x verify-verbs` (should fail if any verb missing metadata)

---

### 2.2 Implement BASIC lint rules

**File:** `rust/src/session/verb_tiering_linter.rs`

| Rule ID | Check | Severity |
|---------|-------|----------|
| B001 | `create-*` verbs must use `operation: insert` (not upsert) | Error |
| B002 | `ensure-*` verbs must use `operation: upsert` | Error |
| B003 | `delete-*` on regulated nouns requires `dangerous: true` | Error |
| B004 | Deprecated verb must have valid `replaced_by` target | Error |
| B005 | `read-*` must be `tier: diagnostics` | Warn |
| B006 | `list-*` must be `tier: diagnostics` | Warn |

```rust
fn check_basic_rules(&self, verb: &VerbDefinition, domain: &str) -> Vec<LintViolation> {
    let mut violations = vec![];
    let meta = verb.metadata.as_ref().unwrap();
    
    // B001: create-* must be insert
    if verb.name.starts_with("create-") {
        if let Some(crud) = &verb.crud {
            if crud.operation != CrudOperation::Insert {
                violations.push(LintViolation {
                    rule: "B001",
                    severity: Severity::Error,
                    message: format!(
                        "{}:{} uses 'create-' prefix but operation is {:?}, expected insert",
                        domain, verb.name, crud.operation
                    ),
                });
            }
        }
    }
    
    // B003: delete on regulated nouns
    if verb.name.starts_with("delete-") {
        let regulated_nouns = ["entity", "cbu", "kyc_case", "investor", "holding"];
        if let Some(noun) = &meta.noun {
            if regulated_nouns.contains(&noun.as_str()) && !meta.dangerous {
                violations.push(LintViolation {
                    rule: "B003",
                    severity: Severity::Error,
                    message: format!(
                        "{}:{} deletes regulated noun '{}' but missing dangerous: true",
                        domain, verb.name, noun
                    ),
                });
            }
        }
    }
    
    // ... B002, B004-B006
    
    violations
}
```

**Verify:** `cargo x verbs lint --tier basic`

---

### 2.3 Tag remaining untagged verbs

Run inventory to find gaps:

```bash
cargo x verb-inventory --show-untagged
```

For each untagged verb, add proper metadata. Priority order:
1. `instruction-profile.yaml` — migrate to deprecated or projection
2. `cbu-custody.yaml` — ensure projection tier
3. `trade-gateway.yaml` — ensure projection tier
4. Remaining files alphabetically

---

### 2.4 Add deprecation warning to executor

**File:** `rust/src/dsl_v2/executor.rs`

```rust
pub async fn execute_verb(&self, verb: &VerbDefinition, args: &OpArgs) -> Result<OpResult> {
    // Check deprecation
    if let Some(meta) = &verb.metadata {
        if meta.status == VerbStatus::Deprecated {
            let replacement = meta.replaced_by.as_deref().unwrap_or("(none specified)");
            warn!(
                "DEPRECATED: {}:{} is deprecated. Use {} instead.",
                verb.domain, verb.name, replacement
            );
            
            // Check runtime flag
            if !self.config.allow_deprecated_verbs {
                return Err(anyhow!(
                    "Deprecated verb {}:{} blocked. Set ALLOW_DEPRECATED_VERBS=true to override.",
                    verb.domain, verb.name
                ));
            }
        }
    }
    
    // Continue with execution...
}
```

**Verify:** Execute a deprecated verb and check logs for warning.

---

### Phase 2 Verification Checklist

```bash
cargo x verify-verbs              # Hard fail on missing metadata
cargo x verbs lint --tier basic   # All BASIC rules pass
cargo x verb-inventory --show-untagged  # Should be empty
```

---

## Phase 3: STANDARD Lints + Matrix Enforcement (~12 hours)

### 3.1 Implement STANDARD lint rules

**File:** `rust/src/session/verb_tiering_linter.rs`

| Rule ID | Check | Severity |
|---------|-------|----------|
| S001 | If `source_of_truth: matrix`, no other verb with same noun can be `tier: intent` | Error |
| S002 | If `writes_operational: true`, must be `tier: projection` or `composite` | Error |
| S003 | If `writes_operational: true` AND `tier: projection`, must be `internal: true` | Error |
| S004 | Operational table writes must trace to `materialize` composite | Warn |
| S005 | `tier: composite` verbs should have plan/apply documentation | Warn |

```rust
fn check_standard_rules(&self, all_verbs: &[VerbDefinition]) -> Vec<LintViolation> {
    let mut violations = vec![];
    
    // S001: Single authoring surface
    let matrix_nouns: HashMap<String, &VerbDefinition> = all_verbs.iter()
        .filter(|v| v.metadata.as_ref()
            .map(|m| m.source_of_truth == Some(SourceOfTruth::Matrix))
            .unwrap_or(false))
        .filter_map(|v| {
            v.metadata.as_ref()?.noun.as_ref().map(|n| (n.clone(), v))
        })
        .collect();
    
    for verb in all_verbs {
        let meta = match &verb.metadata {
            Some(m) => m,
            None => continue,
        };
        
        if meta.tier == Some(VerbTier::Intent) {
            if let Some(noun) = &meta.noun {
                if let Some(matrix_verb) = matrix_nouns.get(noun) {
                    if verb.fqn() != matrix_verb.fqn() {
                        violations.push(LintViolation {
                            rule: "S001",
                            severity: Severity::Error,
                            message: format!(
                                "{}:{} is tier:intent for noun '{}' but {} already owns it via matrix",
                                verb.domain, verb.name, noun, matrix_verb.fqn()
                            ),
                        });
                    }
                }
            }
        }
        
        // S002: writes_operational implies projection/composite
        if meta.writes_operational == Some(true) {
            if !matches!(meta.tier, Some(VerbTier::Projection) | Some(VerbTier::Composite)) {
                violations.push(LintViolation {
                    rule: "S002",
                    severity: Severity::Error,
                    message: format!(
                        "{}:{} has writes_operational:true but tier is {:?}, expected projection or composite",
                        verb.domain, verb.name, meta.tier
                    ),
                });
            }
        }
        
        // S003: projection + writes_operational => internal
        if meta.tier == Some(VerbTier::Projection) 
            && meta.writes_operational == Some(true)
            && meta.internal != Some(true) 
        {
            violations.push(LintViolation {
                rule: "S003",
                severity: Severity::Error,
                message: format!(
                    "{}:{} is projection with writes_operational but missing internal:true",
                    verb.domain, verb.name
                ),
            });
        }
    }
    
    violations
}
```

**Verify:** `cargo x verbs lint --tier standard`

---

### 3.2 Migrate instruction-profile verbs

**File:** `rust/config/verbs/custody/instruction-profile.yaml`

For each assignment/override verb:

```yaml
# BEFORE
assign-template:
  description: "Assign SSI template to CBU"
  behavior: plugin
  handler: assign_ssi_template
  metadata:
    tier: intent  # WRONG - parallel authoring surface
    # ...

# AFTER
assign-template:
  description: "DEPRECATED - Use trading-profile.add-standing-instruction"
  behavior: plugin
  handler: assign_ssi_template_compat
  metadata:
    tier: projection
    status: deprecated
    replaced_by: trading-profile.add-standing-instruction
    internal: true
    writes_operational: true
    # ...
```

Keep read-only verbs as `tier: diagnostics`:
```yaml
list-assignments:
  description: "List SSI assignments for CBU"
  behavior: crud
  metadata:
    tier: diagnostics
    source_of_truth: operational
    # ...
```

---

### 3.3 Add CA policy to matrix schema

**File:** `rust/src/trading/matrix_schema.rs` (or equivalent)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingMatrix {
    // Existing
    pub markets: Vec<MarketConfig>,
    pub instruments: Vec<InstrumentConfig>,
    pub standing_instructions: Vec<SsiConfig>,
    pub gateways: Vec<GatewayConfig>,
    pub settlement_chains: Vec<SettlementChainConfig>,
    
    // NEW
    pub corporate_actions: Option<CorporateActionsPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorporateActionsPolicy {
    pub event_types: Vec<String>,
    pub notification_policy: NotificationPolicy,
    pub election_policy: ElectionPolicy,
    pub cutoff_rules: Vec<CutoffRule>,
    pub proceeds_settlement: ProceedsSettlement,
}
```

**File:** `rust/config/verbs/trading-profile.yaml`

Add CA verbs:

```yaml
ca-enable-event-types:
  description: "Enable corporate action event types for this CBU"
  behavior: plugin
  handler: ca_enable_event_types
  metadata:
    tier: intent
    source_of_truth: matrix
    scope: cbu
    noun: corporate_actions
  args:
    - name: cbu-id
      type: uuid
      required: true
    - name: event-types
      type: json
      required: true
      description: "Array of event type codes"
  returns:
    type: boolean

# ... ca-set-election-policy, ca-set-cutoff-rules, etc.
```

---

### 3.4 Implement plan/apply for materialize

**File:** `rust/src/dsl_v2/custom_ops/trading_profile_ops.rs`

```rust
pub struct GenerateMaterializationPlanOp;

#[async_trait]
impl CustomOp for GenerateMaterializationPlanOp {
    async fn execute(&self, ctx: &OpContext, args: &OpArgs) -> Result<OpResult> {
        let cbu_id = args.get_uuid("cbu-id")?;
        let profile = load_active_profile(ctx.pool(), cbu_id).await?;
        
        let plan = MaterializationPlan::generate(ctx.pool(), &profile).await?;
        
        Ok(OpResult::Json(serde_json::to_value(&plan)?))
    }
}

pub struct ApplyMaterializationPlanOp;

#[async_trait]
impl CustomOp for ApplyMaterializationPlanOp {
    async fn execute(&self, ctx: &OpContext, args: &OpArgs) -> Result<OpResult> {
        let plan_json = args.get_json("plan")?;
        let plan: MaterializationPlan = serde_json::from_value(plan_json)?;
        
        let mut tx = ctx.pool().begin().await?;
        let result = plan.apply(&mut tx).await?;
        tx.commit().await?;
        
        // Audit log
        emit_audit_event(AuditEvent::Materialization {
            cbu_id: plan.cbu_id,
            inserts: result.inserts,
            updates: result.updates,
            deletes: result.deletes,
        });
        
        Ok(OpResult::Json(serde_json::to_value(&result)?))
    }
}
```

**File:** `rust/config/verbs/trading-profile.yaml`

```yaml
generate-materialization-plan:
  description: "Generate a plan showing what materialize would do"
  behavior: plugin
  handler: GenerateMaterializationPlanOp
  metadata:
    tier: diagnostics
    source_of_truth: matrix
    scope: cbu
    noun: trading_matrix
  args:
    - name: cbu-id
      type: uuid
      required: true
  returns:
    type: json

apply-materialization-plan:
  description: "Apply a previously generated materialization plan"
  behavior: plugin
  handler: ApplyMaterializationPlanOp
  metadata:
    tier: composite
    source_of_truth: matrix
    scope: cbu
    noun: trading_matrix
    writes_operational: true
  args:
    - name: plan
      type: json
      required: true
  returns:
    type: json
```

---

### 3.5 Add idempotency test

**File:** `rust/tests/materialize_idempotency.rs`

```rust
#[tokio::test]
#[cfg(feature = "database")]
async fn materialize_is_idempotent() {
    let pool = test_pool().await;
    let cbu_id = setup_test_cbu(&pool).await;
    
    // Build complete matrix
    let matrix = TradingMatrix {
        markets: vec![market("NYSE"), market("LSE")],
        instruments: vec![instrument("EQUITY"), instrument("FIXED_INCOME")],
        standing_instructions: vec![ssi("USD_CASH"), ssi("EUR_CASH")],
        gateways: vec![gateway("FIX_PRIMARY")],
        settlement_chains: vec![chain("US_DTC")],
        corporate_actions: Some(ca_policy()),
    };
    
    // Import and materialize
    import_matrix(&pool, cbu_id, &matrix).await.unwrap();
    let result1 = materialize(&pool, cbu_id).await.unwrap();
    
    // Materialize again
    let result2 = materialize(&pool, cbu_id).await.unwrap();
    
    // Second run should be no-op
    assert_eq!(result2.inserts, 0, "Expected no inserts on second materialize");
    assert_eq!(result2.updates, 0, "Expected no updates on second materialize");
    assert_eq!(result2.deletes, 0, "Expected no deletes on second materialize");
}
```

---

### Phase 3 Verification Checklist

```bash
cargo x verbs lint --tier standard   # All STANDARD rules pass
cargo test materialize_idempotent    # Idempotency test passes
cargo x verb-inventory               # No untagged verbs, no S001 violations
```

---

## Final Acceptance Criteria

- [ ] `VerbMetadata` has all required fields (tier, source_of_truth, scope, noun, status)
- [ ] `cargo x verify-verbs` fails on missing metadata
- [ ] `cargo x verbs lint --tier standard` passes with no errors
- [ ] All 870+ verbs have proper metadata
- [ ] `instruction-profile` assignment verbs are deprecated with `replaced_by`
- [ ] `trading-profile.materialize` has plan/apply variants
- [ ] CA policy verbs exist and are `source_of_truth: matrix`
- [ ] Idempotency test passes
- [ ] `docs/verb_inventory.md` is auto-generated and accurate
- [ ] CLAUDE.md header stats auto-update

---

## Files to Modify (Summary)

| File | Changes |
|------|---------|
| `rust/crates/dsl-core/src/config/types.rs` | Add VerbStatus, lifecycle fields |
| `rust/src/session/verb_tiering_linter.rs` | Add LintTier, MINIMAL/BASIC/STANDARD rules |
| `rust/src/dsl_v2/verb_loader.rs` | Make metadata required |
| `rust/src/dsl_v2/executor.rs` | Add deprecation warning/block |
| `rust/xtask/src/main.rs` | Add verb-inventory subcommand |
| `rust/xtask/src/verb_inventory.rs` | New file - inventory generation |
| `rust/config/verbs/custody/instruction-profile.yaml` | Migrate to deprecated |
| `rust/config/verbs/trading-profile.yaml` | Add CA verbs, plan/apply |
| `rust/src/trading/matrix_schema.rs` | Add CorporateActionsPolicy |
| `rust/src/dsl_v2/custom_ops/trading_profile_ops.rs` | Add plan/apply handlers |
| `rust/tests/materialize_idempotency.rs` | New test file |

---

*This implementation plan is ready for Claude Code execution. Start with Phase 1.*
