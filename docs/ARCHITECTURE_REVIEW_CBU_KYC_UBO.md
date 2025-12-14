# Architecture Review: CBU + KYC/UBO Domain

**Reviewer**: Claude Opus 4.5  
**Date**: 2025-12-14  
**Scope**: CBU foundation, KYC/UBO subdomain, DSL grammar alignment

---

## Executive Summary

The architecture is **fundamentally sound** with a well-designed layered approach:
1. **CBU** as the shared anchor for all onboarding domains
2. **Entity** as the polymorphic participant model (proper-person, limited-company, trust, partnership)
3. **KYC Case** + **UBO** as the compliance workflow layer
4. **Observation/Allegation** as the evidence flow model
5. **DSL** with S-expression syntax and EntityRef resolution

However, I've identified **friction points, gaps, and inconsistencies** that should be addressed.

---

## 1. CBU Foundation

### ✅ What's Good

| Aspect | Assessment |
|--------|------------|
| Core CRUD verbs | Complete (create, read, update, delete, list, ensure) |
| Role assignment | Clean junction pattern via `cbu_entity_roles` |
| Product association | `add-product` / `remove-product` handles service_delivery_map |
| Lifecycle status | Schema has CHECK constraint for valid states |
| Evidence tracking | `cbu_evidence` table links documents/attestations |

### ⚠️ Issues Found

#### 1.1 Status Enum Mismatch

**Schema** (`cbus.status` CHECK constraint):
```
DISCOVERED, VALIDATION_PENDING, VALIDATED, UPDATE_PENDING_PROOF, VALIDATION_FAILED
```

**Verb** (`cbu.decide` lifecycle requires_states):
```yaml
- VALIDATION_PENDING
```

**Gap**: The `decide` verb only works from `VALIDATION_PENDING` but doesn't define what states it transitions TO. The verb should declare:
```yaml
lifecycle:
  entity_arg: cbu-id
  requires_states: [VALIDATION_PENDING]
  success_states:
    APPROVED: VALIDATED
    REJECTED: VALIDATION_FAILED
    REFERRED: VALIDATION_PENDING  # stays in pending
```

#### 1.2 Category Enum Duplication

Schema has **two CHECK constraints** with slightly different values:
```sql
CONSTRAINT cbus_category_check CHECK (... 'FAMILY_TRUST' ...)
CONSTRAINT chk_cbu_category CHECK (... 'INTERNAL_TEST' ...)
```
One has `FAMILY_TRUST`, the other has `INTERNAL_TEST`. Both exist on the same column.

**Fix**: Remove one constraint, keep canonical list.

#### 1.3 Missing `evidence.attach` Verb

`cbu_evidence` table exists but no verb to attach evidence to a CBU. Currently only `document.catalog` exists which is entity-scoped, not CBU-scoped.

**Recommendation**: Add `cbu.attach-evidence` verb:
```yaml
attach-evidence:
  description: Attach evidence (document or attestation) to CBU
  behavior: crud
  crud:
    operation: insert
    table: cbu_evidence
    schema: ob-poc
  args:
    - name: cbu-id
      type: uuid
      required: true
    - name: document-id
      type: uuid
      required: false
    - name: attestation-ref
      type: string
      required: false
    - name: evidence-type
      type: string
      required: true
      valid_values: [DOCUMENT, ATTESTATION, SCREENING, REGISTRY_CHECK, MANUAL_VERIFICATION]
```

---

## 2. KYC Case Domain

### ✅ What's Good

| Aspect | Assessment |
|--------|------------|
| Case lifecycle | INTAKE → DISCOVERY → ASSESSMENT → REVIEW → APPROVED/REJECTED |
| Escalation levels | STANDARD → SENIOR_COMPLIANCE → EXECUTIVE → BOARD |
| Assignment | Analyst and reviewer separation |
| Risk rating | LOW → MEDIUM → HIGH → VERY_HIGH → PROHIBITED |
| Emits events | `case.created`, `case.status-changed`, `case.escalated`, etc. |

### ⚠️ Issues Found

#### 2.1 Status Enum Mismatch

**Schema** has additional statuses not in verb `valid_values`:
```sql
'REFER_TO_REGULATOR', 'DO_NOT_ONBOARD'
```

**Verb** (`kyc-case.update-status` and `kyc-case.close`) only allows:
```yaml
- INTAKE, DISCOVERY, ASSESSMENT, REVIEW, APPROVED, REJECTED, BLOCKED, WITHDRAWN, EXPIRED
```

**Fix**: Either add missing statuses to verb or remove from schema.

#### 2.2 Missing Case-Entity Linkage

Cases link to CBU but individual entities within the CBU are tracked via **workstreams** (`kyc.entity_workstreams`). The `entity-workstream.yaml` exists but the flow is:

```
CBU → Case → Workstreams → Entity-specific KYC tasks
```

This is correct but **no verb** to create workstreams from DSL. Only `case-screening.yaml` exists.

**Recommendation**: Verify `entity-workstream.yaml` has complete CRUD:
```yaml
# Should exist:
- create-workstream (for entity within case)
- update-workstream-status
- list-workstreams-for-case
```

#### 2.3 Missing `case.reopen` Verb

Once closed (APPROVED/REJECTED), there's no verb to reopen for remediation. The `case_type` includes `REMEDIATION` but no workflow to transition.

---

## 3. UBO Domain

### ✅ What's Good

| Aspect | Assessment |
|--------|------------|
| Ownership tracking | `ownership_relationships` with DIRECT/INDIRECT/BENEFICIAL |
| UBO registry | `ubo_registry` with verification workflow |
| Calculation plugin | `ubo.calculate` for traversing ownership chains |
| Temporal tracking | `effective_from` / `effective_to` on relationships |

### ⚠️ Issues Found

#### 3.1 Verification Status Mismatch

**Schema** (`ubo_registry.verification_status` CHECK):
```sql
'SUSPECTED', 'PENDING', 'PROVEN', 'VERIFIED', 'FAILED', 'DISPUTED', 'REMOVED'
```

**Verb** (`ubo.verify-ubo` valid_values):
```yaml
- PENDING, VERIFIED, FAILED, DISPUTED
```

**Missing from verb**: `SUSPECTED`, `PROVEN`, `REMOVED`

This is significant because:
- `SUSPECTED` is the initial state from calculation
- `PROVEN` vs `VERIFIED` distinction is unclear
- `REMOVED` is needed for lifecycle

**Recommendation**: Either:
1. Add all statuses to verb, or
2. Create separate verbs: `ubo.mark-suspected`, `ubo.mark-proven`, `ubo.remove`

#### 3.2 Lookup Table Mismatch in `verify-ubo`

```yaml
lookup:
  table: ubo_determinations  # WRONG - table is ubo_registry
  schema: ob-poc
  search_key: ubo_id
```

Should be:
```yaml
lookup:
  table: ubo_registry
  schema: ob-poc
```

#### 3.3 Missing Supersession Flow

`ubo_registry` has `superseded_by` and `superseded_at` columns but no verb to supersede a UBO determination when ownership changes.

**Recommendation**: Add `ubo.supersede`:
```yaml
supersede:
  description: Supersede a UBO determination with a new one
  args:
    - name: ubo-id
      type: uuid
      required: true
    - name: superseded-by
      type: uuid
      required: true
      description: New UBO determination ID
    - name: reason
      type: string
      required: true
```

#### 3.4 Missing `control` Verbs

UBO has two prongs: **ownership** (>25%) and **control** (decision-making power). The schema supports `control_type` but there's no verb to specifically record control relationships separate from ownership.

**Recommendation**: Add `ubo.add-control`:
```yaml
add-control:
  description: Record control relationship (non-ownership based)
  args:
    - name: controller-entity-id
      type: uuid
    - name: controlled-entity-id  
      type: uuid
    - name: control-type
      type: string
      valid_values: [BOARD_SEAT, VOTING_RIGHTS, VETO_POWER, MANAGEMENT_AGREEMENT, OTHER]
```

---

## 4. DSL Grammar Analysis

### 4.1 EBNF vs Tree-Sitter vs Execution

| Layer | File | Status |
|-------|------|--------|
| Formal grammar | `docs/dsl-grammar.ebnf` | **Outdated** - S-expression workflow-centric |
| Parser | `tree-sitter-dsl/grammar.js` | Current - simple S-expr with verbs |
| AST | `rust/src/dsl_v2/ast.rs` | Current - `VerbCall` + `EntityRef` model |

**The EBNF is divergent** from actual implementation:
- EBNF describes `(define-kyc-investigation ...)` workflow blocks
- Actual DSL is flat verb calls: `(cbu.ensure :name "Fund" :jurisdiction "LU")`

**Recommendation**: Either:
1. Update EBNF to match current flat verb syntax, or
2. Implement the workflow blocks described in EBNF (parallel-block, sequential-block)

### 4.2 EntityRef Resolution - Hybrid DSL Friction

The hybrid DSL (human-readable identifiers → UUID resolution) works well but has friction:

#### Friction Point: Inconsistent `entity_type` Values

In verb YAML files, `lookup.entity_type` varies inconsistently:
```yaml
# Some use singular
entity_type: entity
entity_type: cbu
entity_type: role

# Some use descriptive
entity_type: kyc_case
entity_type: document_type

# Some use domain prefix
entity_type: workstream  # should be entity_workstream?
```

**Recommendation**: Establish naming convention:
- Use `snake_case` domain prefix for domain-specific types
- Use singular for core types

#### Friction Point: Missing Resolution for Some Lookups

```yaml
# isda.yaml counterparty - NO LOOKUP
- name: counterparty
  type: uuid
  required: true
  maps_to: counterparty_entity_id
  # Missing lookup! How does user specify counterparty?
```

The Trading Profile solved this with `EntityRef` but direct ISDA verb doesn't have it.

### 4.3 Symbol References (`@name`) - Well Designed

The `:as @symbol` binding pattern allows chaining:
```lisp
(cbu.ensure :name "Apex Fund" :jurisdiction "LU" :as @fund)
(entity.create-proper-person :name "John Smith" :as @owner)
(cbu.assign-role :cbu-id @fund :entity-id @owner :role "DIRECTOR")
```

This works well. The `SymbolRef` resolution in AST is clean.

---

## 5. Schema-to-Verb Gaps

### 5.1 Tables Without Verbs

| Table | Schema | Missing Verbs |
|-------|--------|---------------|
| `cbu_evidence` | ob-poc | attach, verify, list |
| `kyc.case_events` | kyc | add-event (only case-event.yaml has partial) |
| `kyc.red_flags` | kyc | create, resolve, list |
| `ob-poc.entity_addresses` | ob-poc | add-address, list-addresses |
| `custody.subcustodian_network` | custody | add-override, remove-override |

### 5.2 Verbs Without Schema Alignment

| Verb | Issue |
|------|-------|
| `ubo.verify-ubo` | References wrong table (`ubo_determinations` not `ubo_registry`) |
| `kyc-case.update-status` | Missing schema statuses |
| `isda.create` | `counterparty` has no lookup |

---

## 6. Lifecycle Coverage Analysis

### CBU Lifecycle

```
DISCOVERED → VALIDATION_PENDING → VALIDATED
                ↓                      ↓
         VALIDATION_FAILED    UPDATE_PENDING_PROOF
```

**Verb coverage**:
- ❌ No verb to move DISCOVERED → VALIDATION_PENDING
- ✅ `cbu.decide` moves VALIDATION_PENDING → VALIDATED/VALIDATION_FAILED
- ❌ No verb for UPDATE_PENDING_PROOF flow

### KYC Case Lifecycle

```
INTAKE → DISCOVERY → ASSESSMENT → REVIEW → APPROVED/REJECTED
                                    ↓
                               BLOCKED/WITHDRAWN/EXPIRED
```

**Verb coverage**:
- ✅ `kyc-case.create` starts at INTAKE
- ✅ `kyc-case.update-status` transitions through stages
- ✅ `kyc-case.close` terminates
- ❌ No `reopen` verb

### UBO Lifecycle

```
SUSPECTED → PENDING → VERIFIED/FAILED/DISPUTED
              ↓
           PROVEN → REMOVED
```

**Verb coverage**:
- ✅ `ubo.register-ubo` creates entry
- ⚠️ `ubo.verify-ubo` only covers subset of statuses
- ❌ No verb for SUSPECTED → PENDING transition
- ❌ No verb for REMOVED status

---

## 7. Recommendations Summary

### High Priority (Correctness)

1. **Fix `ubo.verify-ubo` lookup table** → `ubo_registry` not `ubo_determinations`
2. **Align status enums** between schema CHECKs and verb valid_values
3. **Add missing `counterparty` lookup** to `isda.create` verb
4. **Remove duplicate cbu_category CHECK constraint**

### Medium Priority (Completeness)

5. **Add `cbu.attach-evidence` verb** for evidence linkage
6. **Add `ubo.supersede` verb** for ownership changes
7. **Add `ubo.add-control` verb** for control prong
8. **Add lifecycle transition verbs** for CBU state machine
9. **Add `kyc-case.reopen` verb** for remediation flow

### Low Priority (Consistency)

10. **Standardize `entity_type` naming** in verb lookups
11. **Update EBNF grammar** to match actual DSL syntax
12. **Add verbs for orphan tables** (cbu_evidence, entity_addresses, etc.)

---

## 8. Next Steps

1. **Quick wins**: Fix the lookup table typo and enum mismatches (items 1-4)
2. **Design session**: UBO lifecycle verbs and control prong
3. **DSL grammar update**: Decide if workflow blocks should be implemented
4. **Comprehensive verb audit**: Generate list of all tables vs all verbs

Would you like me to:
- A) Generate specific YAML patches for the high-priority fixes?
- B) Deep-dive into any specific area?
- C) Continue to Trading Profile / Custody domain review?
