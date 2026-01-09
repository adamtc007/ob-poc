# OB-POC Architecture Review: CBU, KYC/UBO, and DSL

**Date:** December 14, 2025  
**Scope:** CBU shared construct, KYC/UBO subdomain, DSL grammar alignment  
**Focus:** Friction, gaps, overlaps, lifecycle coverage

---

## Executive Summary

The architecture is **solid and well-structured** with a clear separation between:
- **Schema** (Postgres with proper FK constraints)
- **Verb definitions** (YAML-driven, declarative)
- **Generic executor** (CRUD operations from config)
- **Custom handlers** (complex business logic as plugins)

Key findings:
1. **CBU is well-designed** as the shared construct - clean junction tables, proper evidence trails
2. **KYC/UBO domain is comprehensive** but has some verb gaps for complete lifecycle coverage
3. **DSL grammar vs verb YAML** - the EBNF is more S-expression focused (Clojure-style), while actual parsing uses a simpler keyword-argument syntax. The EBNF may be aspirational documentation rather than implemented grammar
4. **EntityRef resolution** is well-architected in the AST with `resolved_key` pattern

---

## Part 1: CBU Architecture Review

### 1.1 Schema Structure ✅ Well Designed

```
ob-poc.cbus (core)
├── cbu_entity_roles (junction: CBU ↔ Entity ↔ Role)
├── cbu_evidence (documents/attestations)
├── cbu_trading_profiles (JSONB document store)
├── cbu_resource_instances (service provisioning)
└── cbu_change_log / cbu_creation_log (audit)

kyc.cases (CBU-scoped KYC workflow)
├── kyc.entity_workstreams (per-entity work items)
├── kyc.doc_requests
├── kyc.red_flags
├── kyc.screenings
└── kyc.approval_requests
```

**Strengths:**
- `cbu_id` is consistent FK across all related tables
- `cbu_entity_roles` properly uses a junction table with role lookup
- Status machine in `cbus.status` with CHECK constraint: `DISCOVERED → VALIDATION_PENDING → VALIDATED`
- Flexible JSONB contexts: `risk_context`, `onboarding_context`, `semantic_context`

### 1.2 CBU Verb Coverage ✅ Complete

| Lifecycle Stage | Verbs | Status |
|----------------|-------|--------|
| Create | `cbu.create`, `cbu.ensure` | ✅ |
| Read | `cbu.read`, `cbu.list`, `cbu.show` | ✅ |
| Update | `cbu.update`, `cbu.set-category` | ✅ |
| Delete | `cbu.delete` | ✅ |
| Roles | `cbu.assign-role`, `cbu.remove-role`, `cbu.parties` | ✅ |
| Products | `cbu.add-product`, `cbu.remove-product` | ✅ |
| Validation | `cbu.check-invariants` | ✅ |
| Decision | `cbu.decide` | ✅ |

**Note:** `cbu.decide` has proper lifecycle guard:
```yaml
lifecycle:
  entity_arg: cbu-id
  requires_states:
    - VALIDATION_PENDING
```

### 1.3 CBU Gaps/Recommendations

#### Gap 1: Missing `cbu.transition-status` verb
Currently status changes happen via `cbu.update :status "..."` but there's no dedicated verb that enforces valid transitions.

**Recommendation:** Add `cbu.advance` verb with transition validation:
```yaml
advance:
  description: Advance CBU to next lifecycle state
  behavior: plugin
  handler: CbuAdvanceOp
  args:
    - name: cbu-id
      type: uuid
      required: true
    - name: target-status
      type: string
      required: true
      valid_values: [VALIDATION_PENDING, VALIDATED]
  lifecycle:
    entity_arg: cbu-id
    valid_transitions:
      DISCOVERED: [VALIDATION_PENDING]
      VALIDATION_PENDING: [VALIDATED, VALIDATION_FAILED]
```

#### Gap 2: No `cbu.attach-evidence` convenience verb
Evidence must be inserted directly into `cbu_evidence`. A verb would be cleaner.

#### Gap 3: `commercial_client_entity_id` FK not validated
The column exists but no verb enforces that the referenced entity is actually a valid commercial client type.

---

## Part 2: KYC/UBO Domain Review

### 2.1 KYC Case Lifecycle ✅ Well Covered

**Schema status machine:**
```sql
CONSTRAINT chk_case_status CHECK (status IN (
  'INTAKE', 'DISCOVERY', 'ASSESSMENT', 'REVIEW',
  'APPROVED', 'REJECTED', 'BLOCKED', 'WITHDRAWN',
  'EXPIRED', 'REFER_TO_REGULATOR', 'DO_NOT_ONBOARD'
))
```

**Verb coverage:**
| Action | Verb | Status |
|--------|------|--------|
| Create case | `kyc-case.create` | ✅ |
| Update status | `kyc-case.update-status` | ✅ |
| Escalate | `kyc-case.escalate` | ✅ |
| Assign | `kyc-case.assign` | ✅ |
| Set risk | `kyc-case.set-risk-rating` | ✅ |
| Close | `kyc-case.close` | ✅ |
| Read | `kyc-case.read` | ✅ |
| List by CBU | `kyc-case.list-by-cbu` | ✅ |

### 2.2 Entity Workstream ✅ Well Covered

**Schema status machine:**
```sql
CONSTRAINT chk_workstream_status CHECK (status IN (
  'PENDING', 'COLLECT', 'VERIFY', 'SCREEN', 'ASSESS',
  'COMPLETE', 'BLOCKED', 'ENHANCED_DD', 'REFERRED', 'PROHIBITED'
))
```

**Verb coverage:**
| Action | Verb | Status |
|--------|------|--------|
| Create | `entity-workstream.create` | ✅ |
| Update status | `entity-workstream.update-status` | ✅ |
| Block | `entity-workstream.block` | ✅ |
| Complete | `entity-workstream.complete` | ✅ |
| Set enhanced DD | `entity-workstream.set-enhanced-dd` | ✅ |
| Mark as UBO | `entity-workstream.set-ubo` | ✅ |
| List by case | `entity-workstream.list-by-case` | ✅ |
| Read | `entity-workstream.read` | ✅ |

### 2.3 UBO Domain ⚠️ Has Gaps

**Schema: `ob-poc.ownership_relationships`**
- Tracks who owns what with percentages
- Supports DIRECT, INDIRECT, BENEFICIAL types
- Has temporal validity (`effective_from`, `effective_to`)

**Schema: `ob-poc.ubo_registry`**
- Records UBO determinations with verification status
- Links to `workstream_id` for KYC integration
- Supports audit trail (`evidence_doc_ids`, `proof_method`)

**Verb coverage:**

| Action | Verb | Status |
|--------|------|--------|
| Add ownership | `ubo.add-ownership` | ✅ |
| Update ownership | `ubo.update-ownership` | ✅ |
| End ownership | `ubo.end-ownership` | ✅ |
| List owners | `ubo.list-owners` | ✅ |
| List owned | `ubo.list-owned` | ✅ |
| Register UBO | `ubo.register-ubo` | ✅ |
| Verify UBO | `ubo.verify-ubo` | ⚠️ Table mismatch |
| List UBOs | `ubo.list-ubos` | ✅ |
| List by subject | `ubo.list-by-subject` | ✅ |
| Calculate | `ubo.calculate` | ✅ Plugin |

#### UBO Issue 1: `verify-ubo` lookup table mismatch ⚠️

```yaml
# Current - WRONG
- name: ubo-id
  lookup:
    table: ubo_determinations  # ❌ Table doesn't exist!
    
# Should be
- name: ubo-id
  lookup:
    table: ubo_registry  # ✅ Correct table
```

#### UBO Issue 2: Missing snapshot verbs

Schema has `ubo_snapshots` and `ubo_snapshot_comparisons` but no verbs to:
- Create snapshot (`ubo.snapshot`)
- Compare snapshots (`ubo.compare-snapshots`)

**Recommendation:** Add snapshot verbs:
```yaml
snapshot:
  description: Create point-in-time UBO snapshot for a CBU
  behavior: plugin
  handler: UboSnapshotOp
  args:
    - name: cbu-id
      type: uuid
      required: true
    - name: reason
      type: string
      required: false
      valid_values: [PERIODIC_REVIEW, EVENT_DRIVEN, INITIAL]
  returns:
    type: uuid
    name: snapshot_id

compare:
  description: Compare two UBO snapshots
  behavior: plugin
  handler: UboCompareOp
  args:
    - name: baseline-snapshot-id
      type: uuid
      required: true
    - name: current-snapshot-id
      type: uuid
      required: true
  returns:
    type: record
```

#### UBO Issue 3: Missing `ubo.remove` verb

Schema has `closed_at`, `closed_reason`, `removal_reason` but no verb to close/remove a UBO determination.

---

## Part 3: Entity Domain Review

### 3.1 Entity Type Pattern ✅ Excellent

The entity system uses a **discriminated union pattern**:

```
ob-poc.entities (base)
├── entity_type_id → entity_types
│
├── entity_proper_persons (natural persons)
├── entity_limited_companies (companies)
├── entity_trusts (trusts)
├── entity_partnerships (partnerships)
├── entity_funds (funds)
└── entity_manco (management companies)
```

**Verb pattern is consistent:**
- `entity.create-{type}` / `entity.ensure-{type}` for each subtype
- Extension table populated via `entity_create` / `entity_upsert` CRUD operation

### 3.2 Entity Gap: Missing fund/manco verbs

Only these entity types have explicit verbs:
- `limited-company` ✅
- `proper-person` ✅  
- `trust-discretionary` ✅
- `partnership-limited` ✅

**Missing:**
- `entity.create-fund` / `entity.ensure-fund`
- `entity.create-manco` / `entity.ensure-manco`

The `dynamic_verbs` pattern in YAML suggests these could be auto-generated, but it's not clear if that's implemented.

---

## Part 4: DSL Grammar Analysis

### 4.1 EBNF Grammar (Aligned with Parser)

**The EBNF grammar (`docs/dsl-grammar.ebnf`) now matches the NOM parser implementation (updated 2025-01-09).**

**DSL syntax:**
```clojure
(cbu.ensure :name "Apex Fund" :jurisdiction "LU" :as @fund)
(entity.create-limited-company :name "Apex Holdings" :cbu-id @fund :as @company)
(cbu.assign-role :cbu-id @fund :entity-id @company :role "DIRECTOR")
```

**Grammar structure:**
| Element | Syntax | Example |
|---------|--------|---------|
| Verb call | `(domain.verb :args... [:as @symbol])` | `(cbu.ensure :name "X")` |
| Keyword arg | `:key value` | `:jurisdiction "LU"` |
| Binding | `:as @symbol` | `:as @fund` |
| Symbol ref | `@name` | `@fund`, `@entity` |
| Values | strings, numbers, bools, nil, lists, maps | `"text"`, `42`, `true`, `[1 2]` |

### 4.2 Key Grammar Files

| File | Purpose |
|------|---------|
| `docs/dsl-grammar.ebnf` | Formal EBNF grammar specification |
| `rust/crates/dsl-core/src/parser.rs` | NOM combinator parser implementation |
| `rust/crates/dsl-core/src/ast.rs` | AST types (Program, VerbCall, AstNode) |
| `rust/config/verbs/*.yaml` | Verb definitions (drives semantic validation) |

**Note:** The parser is grammar-driven but verb-agnostic. Verb validation happens in the semantic phase using `RuntimeVerbRegistry` loaded from YAML.

### 4.3 EntityRef Resolution ✅ Well Designed

The AST's `EntityRef` pattern is excellent:
```rust
EntityRef {
    entity_type: String,    // From YAML lookup.entity_type
    search_column: String,  // From YAML lookup.search_key  
    value: String,          // User's input
    resolved_key: Option<String>,  // Populated during validation
    span: Span,
}
```

**Pipeline is clear:**
```
Parser → Raw AST (strings)
       ↓
YAML Enrichment → EntityRefs identified
       ↓
Validator → EntityRefs resolved (DB lookup)
       ↓
Executor → Uses resolved UUIDs
```

---

## Part 5: Cross-Cutting Issues

### 5.1 Naming Inconsistencies

| Location | Name | Issue |
|----------|------|-------|
| Verb YAML | `market` | Used in some places |
| Types.rs | `mic` | ISO 10383 code |
| Seed YAML | `market: XETR` | Mixing conventions |

**Recommendation:** Standardize on `mic` (Market Identifier Code) everywhere, with `market` as alias in serde.

✅ **Already done in types.rs:**
```rust
#[serde(alias = "market")]
pub mic: Option<String>,
```

### 5.2 Trading Profile Integration Gap

The Trading Profile has excellent types but the verb YAML shows handlers as strings:
```yaml
handler: import_trading_profile
handler: materialize_trading_profile
```

**Question:** Are these handlers implemented in `custom_ops/trading_profile.rs`? Need to verify.

### 5.3 Event Emission Pattern

Many verbs have `emits_event`:
```yaml
emits_event: case.created
emits_event: workstream.status-changed
```

**Question:** Is there an event bus/subscriber implementation, or is this aspirational?

---

## Part 6: Recommended Fixes (Priority Order)

### High Priority (Blocking Issues)

1. **Fix `ubo.verify-ubo` lookup table** - Change from `ubo_determinations` to `ubo_registry`

2. **Add missing fund/manco entity verbs** - Either implement dynamic_verbs or add explicit verbs

### Medium Priority (Completeness)

3. **Add UBO snapshot verbs** - `ubo.snapshot`, `ubo.compare`

4. **Add `ubo.remove` verb** - For closing UBO determinations

5. **Add `cbu.attach-evidence` verb** - Convenience for evidence tracking

### Low Priority (Polish)

6. **Reconcile EBNF with actual grammar** - Documentation accuracy

7. **Add `cbu.advance` lifecycle verb** - Enforce valid state transitions

8. **Verify event emission implementation** - Document or implement

---

## Summary: State of the Architecture

| Domain | Schema | Verbs | Types | Handlers | Overall |
|--------|--------|-------|-------|----------|---------|
| CBU | ✅ | ✅ | ✅ | ✅ | **Ready** |
| KYC Case | ✅ | ✅ | ✅ | ✅ | **Ready** |
| Entity Workstream | ✅ | ✅ | ✅ | ✅ | **Ready** |
| UBO | ✅ | ⚠️ | ✅ | ✅ | **Minor fixes** |
| Entity | ✅ | ⚠️ | ✅ | ✅ | **Minor gaps** |
| Trading Profile | ✅ | ✅ | ✅ | ⚠️ | **Verify handlers** |
| DSL Parser | N/A | N/A | ✅ | ✅ | **Ready** |

**Overall Assessment:** The architecture is **production-ready** for the KYC/UBO lifecycle with minor verb fixes. The DSL execution pipeline (parse → enrich → validate → execute) is well-designed and the generic CRUD executor enables rapid verb addition.
