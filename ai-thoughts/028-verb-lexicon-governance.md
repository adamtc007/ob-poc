# 028: Verb Lexicon Governance Upgrade

> **Status:** TODO — Ready for implementation
> **Priority:** HIGH — Stops architectural drift, enables safe refactoring
> **Effort:** ~40-60 hours across 3 phases
> **Depends on:** 027 (trading matrix pivot - ✅ DONE)

---

## Problem Statement

With 870+ verbs across multiple domains, the lexicon is susceptible to regression:
- Multiple "sources of truth" for the same intent (matrix vs ops-table authoring)
- Inconsistent semantics (`create` vs `ensure`, `delete` vs `end`/`close`)
- Composites that mutate state without being obvious/auditable
- Docs that drift away from what agents actually see

**The fix:** Treat verbs like a public API—every verb declares what it is, and tooling enforces it.

This follows mature API ecosystem patterns:
- OpenAPI supports explicit deprecation and extension fields
- Kubernetes pairs deprecation with formal policy + migration guide
- SemVer formalizes API surface introduction and retirement

---

## A. Required Verb Metadata Schema

### A1. Extend verb YAML schema (required fields)

Standardize these fields under `metadata:` for every verb:

```yaml
metadata:
  # Required
  tier: reference | intent | projection | diagnostics | composite
  source_of_truth: matrix | operational | catalog | entity | workflow | external | register | session | document
  scope: global | cbu | entity | case
  noun: string                    # e.g., trading_matrix, ssi, gateway, settlement_chain
  
  # Lifecycle (required for deprecation flow)
  status: active | deprecated     # default: active
  replaced_by: string             # optional, canonical verb name
  since_version: string           # optional, e.g., "0.9.0"
  removal_version: string         # optional, e.g., "1.0.0"
  
  # Behavioral flags
  writes_operational: boolean     # true if writes to operational/projected tables
  internal: boolean               # true if not for direct user invocation
  dangerous: boolean              # true for destructive ops on regulated nouns
  
  # Documentation
  tags: [string]                  # free-form categorization
```

### A2. Make metadata mandatory via loader validation

Update verb loader so missing required metadata fails `cargo x verify-verbs`:
- **Phase 1:** Warn mode (log missing fields, don't fail)
- **Phase 2:** Hard fail (block build if required fields missing)

### A3. Lint categories (like Buf)

Implement linter strictness tiers for gradual rollout:

| Tier | Rules |
|------|-------|
| `MINIMAL` | Required metadata present + valid enums |
| `BASIC` | Naming conventions + create/ensure semantics + deprecation coherence |
| `STANDARD` | Matrix pivot rules + composite audit requirements + no alternate authoring |

---

## B. Matrix-First Pivot Linter Rules

### B1. "Single authoring surface" rule

If `source_of_truth: matrix`, no other verb may be `tier: intent` for the same `noun + scope` combination unless explicitly a compat alias.

**Enforcement targets:**
Any verb writing `custody.cbu_*` (or equivalent per-CBU operational tables) must be:
- `tier: projection` (or `composite` if it's materialize)
- `source_of_truth: operational`
- `writes_operational: true`
- Either `internal: true` or only callable through `trading-profile.materialize`

### B2. "Projection-only writes" rule

These domain writes must be `tier: projection` + `internal: true`:

| Domain | Affected Tables |
|--------|-----------------|
| `cbu-custody.*` | cbu_instruments, cbu_markets, cbu_universe |
| `instruction-profile.*` | SSI assignments, field overrides |
| `trade-gateway.*` | CBU routing, connectivity |
| `settlement-chain.*` | CBU chains, preferences |
| `pricing-config.*` | CBU pricing sources |
| `tax-config.*` | CBU tax treatment |
| `corporate-action.*` | CBU CA preferences |

### B3. "One commit path" rule

Operational state written through exactly one composite entry:
- `trading-profile.materialize` (or `trading-matrix.materialize`)

Sub-steps allowed but must be `internal: true` and called only by materialize.

---

## C. Deprecation + Aliasing

### C1. Deprecation semantics in YAML

```yaml
my-deprecated-verb:
  description: "..."
  metadata:
    status: deprecated
    replaced_by: trading-profile.add-standing-instruction
    removal_version: "1.0.0"
```

### C2. Alias implementation in executor (optional but recommended)

Allow verb to alias another:
- Same args (or defined arg mapping)
- Emit warning in execution log: `"deprecated; use X"`
- Can be globally disabled via config

### C3. "Fail on deprecated" execution mode

Runtime flag (env/config):
```bash
ALLOW_DEPRECATED_VERBS=true|false  # Default: true until migration done
```

---

## D. Migration: instruction-profile to projection/compat

### D1. Classify current verbs

| Verb Type | New Classification |
|-----------|-------------------|
| Template library verbs | `tier: reference`, `source_of_truth: catalog` |
| Assignment/override verbs | `status: deprecated` with alias OR `tier: projection` + `internal: true` |

### D2. Migration mechanics

**Option A — Compat alias (preferred for speed):**
```yaml
instruction-profile.assign-template:
  metadata:
    status: deprecated
    replaced_by: trading-profile.add-standing-instruction
  # Executor translates to matrix mutation
```

**Option B — Hard cutover:**
- Block direct assignment writes immediately
- Require matrix authoring + materialize for all new changes
- Keep `instruction-profile.list-*` as read-only inspection

### D3. Backsliding prevention linter rule

```
IF domain == instruction-profile AND writes_operational == true:
  REQUIRE tier: projection OR (status: deprecated AND replaced_by: present)
```

---

## E. Corporate Actions: Matrix Axis (not SSI subsystem)

### E1. Define CA intent in matrix schema

Add `corporate_actions` section to matrix document:

```yaml
corporate_actions:
  event_types:
    - DIVIDEND_CASH
    - STOCK_SPLIT
    - RIGHTS_ISSUE
    # ...
  notification_policy:
    sla_hours: 24
    channels: [email, portal]
  election_policy:
    require_evidence: true
    default_option: CASH
  cutoff_rules:
    - market: NYSE
      days_before: 2
    - depository: DTCC
      days_before: 1
  proceeds_settlement:
    cash_proceeds_ssi: $ssi_cash_usd
    stock_proceeds_ssi: $ssi_securities
```

### E2. Matrix verbs for CA intent

```
trading-profile.ca.enable-event-types
trading-profile.ca.set-election-policy
trading-profile.ca.set-default-option
trading-profile.ca.set-cutoff-rules
trading-profile.ca.link-proceeds-ssi
```

### E3. Materialize CA to operational tables

During `trading-profile.materialize`:
- Generate/update `custody.cbu_ca_*` tables
- Direct CA preference CRUD becomes `tier: projection` or `status: deprecated`

---

## F. Plan/Apply Materialization

### F1. `generate-materialization-plan`

Returns structured diff:
```json
{
  "tables": {
    "custody.cbu_instruments": {
      "insert": 12,
      "update": 3,
      "delete": 0
    },
    "custody.cbu_ssi_assignments": {
      "insert": 8,
      "update": 0,
      "delete": 2
    }
  },
  "missing_references": [
    {"type": "template", "id": "ssi-template-xyz"}
  ],
  "warnings": [
    "Gateway GW-123 not found; routing will be incomplete"
  ]
}
```

### F2. `apply-materialization-plan`

- Apply diff in single transaction
- Emit audit summary to `audit.materialize_events`
- Return success/failure + row counts

**Why:** Composites become explainable, testable, resumable (better for agents and debugging).

---

## G. Naming + Semantics Normalization

### G1. `create` vs `ensure`

| Verb | Semantics | Linter Rule |
|------|-----------|-------------|
| `create` | Fail if exists | Must have `behavior: crud` + `operation: insert` |
| `ensure` | Idempotent/upsert | Must have `behavior: crud` + `operation: upsert` |

Deprecate `create-*` in favor of `ensure-*` where idempotency is appropriate.

### G2. `delete` vs `end`/`close`

| Verb | Use Case | Linter Rule |
|------|----------|-------------|
| `delete` | Non-regulated, hard delete | Require `metadata.dangerous: true` |
| `end` / `close` | Regulated nouns, soft close | Preferred for audit trail |
| `supersede` | Version replacement | For documents, profiles |

### G3. `read`/`list`/`find`/`query`

| Verb Pattern | Semantics |
|--------------|-----------|
| `read` | By primary key |
| `get` | By natural key (name, code) |
| `list` | Enumeration with filters |
| `find` / `lookup` | Resolution/heuristic match |
| `query` | Recordset/batch helper |

---

## H. Docs + Agent Manual Refresh

### H1. Auto-generate domain inventory

Add xtask/CLI that writes:
- `docs/verb_inventory.md` — domain → verb count, tier breakdown, source breakdown
- Optionally refresh `CLAUDE.md` header stats

```bash
cargo x verb-inventory --output docs/verb_inventory.md
cargo x verb-inventory --update-claude-md
```

### H2. Add "Truth Hierarchy" section to docs

Document explicitly:

```
1. Matrix Document (intent) — what the user authored
   ↓
2. Materialize (projection) — deterministic transformation
   ↓
3. Operational Tables (execution state) — derived, not authored
   ↓
4. Inspection Verbs (read-only) — diagnostics, queries
```

---

## I. Test Requirements

### I1. Determinism / Idempotency test

```rust
#[test]
fn materialize_is_idempotent() {
    // Build matrix with: universe + SSIs + gateway + settlement + booking + CA
    let matrix = build_test_matrix();
    
    // Run materialize twice
    let result1 = materialize(&matrix);
    let result2 = materialize(&matrix);
    
    // Assert second run produces no changes
    assert_eq!(result2.inserts, 0);
    assert_eq!(result2.updates, 0);
    assert_eq!(result2.deletes, 0);
}
```

### I2. "No alternate authoring surfaces" test

**Static (linter):**
- Fail any new verb that writes operational tables as `tier: intent`

**Runtime (optional):**
- Flag to fail if deprecated verbs are executed

---

## J. Rollout Plan

### Phase 1 — Introduce schema + warnings (~1 week)

- [ ] Metadata optional but recommended
- [ ] Linter in WARN mode for MINIMAL rules
- [ ] Generate initial `verb_inventory.md`
- [ ] Update CLAUDE.md with governance section

### Phase 2 — Mandatory metadata + BASIC lints (~2 weeks)

- [ ] Loader requires metadata (hard fail)
- [ ] Linter fails BASIC rules
- [ ] Tag all remaining verbs with proper metadata
- [ ] Deprecate first batch of duplicate verbs

### Phase 3 — STANDARD lints + matrix enforcement (~2 weeks)

- [ ] Forbid alternate authoring surfaces
- [ ] Require `replaced_by` on deprecated verbs
- [ ] Enable "fail on deprecated" in CI
- [ ] Complete instruction-profile migration
- [ ] Add CA policy to matrix schema

---

## Acceptance Criteria

- [ ] Every verb YAML has required `metadata.*` fields
- [ ] `instruction-profile` assignment/override no longer represents a parallel intent surface
- [ ] Corporate actions policy is authored in matrix and projected by materialize
- [ ] Materialize has deterministic audit output (plan/apply)
- [ ] Linter prevents any new drift back into "ops as truth"
- [ ] Docs reflect true counts and truth hierarchy
- [ ] `cargo x verify-verbs` enforces all STANDARD rules

---

## Related Documents

- `027-trading-matrix-canonical-pivot.md` — Types + linter + tagging (✅ DONE)
- `docs/verb-definition-spec.md` — YAML structure reference
- `docs/strategy-patterns.md` §2 — LLM→DSL pattern

---

*This document follows Kubernetes-style deprecation cadence: mark deprecated, publish migration guidance, then remove later.*
