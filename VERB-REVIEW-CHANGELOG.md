# Verb Review Changelog

> Generated: 2026-02-07
> Covers: Phases VA, VB, VC of TODO-1-VERB-ATLAS-REVIEW.md

---

## Phase VA: Atlas Tooling

### New Command: `cargo x verbs atlas`

Produces four output files in `rust/docs/generated/`:

| File | Purpose |
|------|---------|
| `verb_atlas.md` | Human-readable full verb table (959 verbs, 4856 phrases) |
| `verb_atlas.json` | Machine-readable for downstream tooling |
| `verb_findings.md` | Problems grouped by severity |
| `verb_phrase_collisions.md` | Collision report |

### Lint Checks Implemented (11)

| Check | Severity | Rule |
|-------|----------|------|
| GHOST_VERB | ERROR | `tier: intent` + 0 invocation phrases + not template_only |
| COLLISION | ERROR (same-domain) / WARN (cross-domain) | 2+ verbs share normalized phrase |
| NEAR_COLLISION | WARN | Jaccard token similarity > 0.80 |
| MISSING_TIER | ERROR | No `metadata.tier` field |
| MISSING_EXEC_MODE | WARN | No `execution_mode` field |
| MISSING_PRECONDITIONS | INFO | Intent verb with lifecycle but no precondition_checks |
| CONTROL_IN_PACK | ERROR | Control-plane verb in business pack allowed_verbs |
| ORPHAN_CONCEPT | WARN | verb_concepts.yaml entry maps to nonexistent verb |
| MISSING_CONCEPT | WARN | Intent verb not in verb_concepts.yaml |
| NO_HANDLER | WARN | Plugin verb with no handler found |
| DEAD_VERB | ERROR | No handler + no pack + no tests |

### CI Integration

- `cargo x verbs atlas --lint-only` exits non-zero on ERRORs
- Wired into `cargo x pre-commit` as a build gate

---

## Phase VB: Data Quality Fixes

### MISSING_TIER (10 fixed)

Added `metadata.tier: intent` to 10 ownership.* verbs in `rust/config/verbs/ownership.yaml`:

- `ownership.snapshot.list`
- `ownership.snapshot.get`
- `ownership.right.add-to-class`
- `ownership.right.add-to-holder`
- `ownership.right.end`
- `ownership.right.list-for-issuer`
- `ownership.right.list-for-holder`
- `ownership.reconcile.findings`
- `ownership.reconcile.resolve-finding`
- `ownership.reconcile.list-runs`

### DEAD_VERB (1 deleted)

- `deal.supersede-rate-card` — no handler, no pack membership, no tests. Removed from `rust/config/verbs/deal.yaml`.

### CONTROL_IN_PACK (2 resolved)

- `session.load-galaxy` and `session.load-cbu` were flagged as control-plane verbs in business packs
- Resolution: exempted `session.load-*` scoping verbs from the lint rule (they legitimately appear in business packs to set session scope)

### COLLISION (112 same-domain pairs resolved)

**Root cause analysis:** The phrase_gen auto-generation system produced collisions via its action-synonym x domain-noun cartesian product.

**Fixes in `phrase_gen.rs`:**

| Change | Before | After |
|--------|--------|-------|
| `list` synonyms | "show", "get all" | "show all", "list all" |
| `read` synonyms | "get", "show" | "get", "fetch", "view", "retrieve" |
| `isda` nouns | "agreement", "contract" | "isda agreement", "isda contract" |
| `product` nouns | "service" removed | "offering" only |
| `contract` nouns | "agreement" removed | "legal contract" |
| `role` nouns | "position" removed | "entity role" |
| `holding` nouns | "position" removed | "investment holding" |
| `view` nouns | "display", "visualization" removed | "viewport" only |
| `drill` synonyms | "zoom in", "enter" | "go into", "dig into" |
| `surface` synonyms | "zoom out" | "ascend" |

**Manual phrase fixes in verb YAML:**

| Verb | Change |
|------|--------|
| `runbook.abort` | "clear runbook" -> "clear all staged commands" |
| `view.zoom-out` | "go back up" -> "widen the view" |
| `view.zoom-in` | "show more detail" -> "zoom in for detail" |
| `investor-role.set` | Removed "mark as nominee/FoF/end investor" (belong to convenience verbs) |
| `docs-bundle.list-applied` | "show document bundles" -> "show applied document bundles" |
| `docs-bundle.list-available` | "show document bundles" -> "show available document bundles" |
| `session.set-client` | Removed "work on", "switch to"; added "set client context" |
| `session.load-system` | "switch to" -> "switch to cbu" |
| `session.filter-jurisdiction` | "filter to" -> "filter to jurisdiction" |
| `trading-profile.approve` | "activate trading profile" -> "approve trading profile activation" |

**Severity model:** Cross-domain collisions downgraded to WARN (pack/scope scoring separates them). Only same-domain collisions remain ERROR.

### ORPHAN_CONCEPT (14 fixed)

Updated `rust/config/lexicon/verb_concepts.yaml`:

| Old Key | New Key | Reason |
|---------|---------|--------|
| `cbu.get` | `cbu.read` | Verb renamed |
| `session.load-cbu` | `session.load-system` | Verb renamed |
| `entity.create` | `entity.create-proper-person` | Verb renamed |
| `entity.get` | `entity.read` | Verb renamed |
| `ubo.discover` | `ubo.list-ubos` | Verb renamed |
| `kyc.open-case` | `kyc-case.create` | Domain restructured |
| `kyc.list-cases` | `kyc-case.list-by-cbu` | Domain restructured |
| `trading-profile.create` | `trading-profile.create-draft` | Verb renamed |
| `trading-profile.list` | `trading-profile.list-versions` | Verb renamed |
| `cbu-role.assign` | `cbu.role.assign` | FQN changed |
| `cbu-role.list` | `cbu.role.list` | FQN changed |
| `gleif.lookup` | `gleif.lookup-by-isin` | Verb renamed |
| `scope.commit` | (removed) | No verb exists |
| `scope.narrow` | (removed) | No verb exists |

### MISSING_PRECONDITIONS (25 fixed)

Added `precondition_checks` to all intent verbs with lifecycle blocks:

| Domain | Verbs | Precondition |
|--------|-------|--------------|
| `cbu` | `decide` | `requires_scope:cbu` |
| `pricing-config` | `set-valuation-schedule`, `set-fallback-chain`, `set-stale-policy`, `set-nav-threshold` | `requires_scope:cbu` |
| `settlement-chain` | `create-chain`, `deactivate-chain`, `set-location-preference`, `set-cross-border`, `deactivate-cross-border` | `requires_scope:cbu` |
| `settlement-chain` | `add-hop`, `remove-hop` | `requires_scope:cbu` + `requires_prior:settlement-chain.create-chain` |
| `settlement-chain` | `define-location` | `reviewed:none` (reference data, no scope needed) |
| `tax-config` | `set-tax-status`, `set-reclaim-config`, `deactivate-reclaim`, `set-reporting` | `requires_scope:cbu` |
| `tax-config` | `set-treaty-rate` | `requires_prior:tax-config.define-jurisdiction` |
| `tax-config` | `define-jurisdiction` | `reviewed:none` (reference data) |
| `trade-gateway` | `enable-gateway`, `activate-gateway`, `suspend-gateway`, `add-routing-rule`, `set-fallback` | `requires_scope:cbu` |
| `trade-gateway` | `define-gateway` | `reviewed:none` (reference data) |

---

## Phase VC: Re-embed + Validate + CI Gate

### Embedding Regeneration

- `cargo x verbs compile` synced YAML to DB: 2 added, 3 updated, 136 stale removed
- `populate_embeddings --force` re-embedded 6,017 patterns in 65.9s
- Total: 14,673 patterns with embeddings, 1,093 unique verbs, 99.8% coverage

### CI Gate

- `cargo x verbs atlas --lint-only` wired into `cargo x pre-commit`
- Fails build on any ERROR finding
- WARNs logged but don't fail

### Living Documents

Atlas outputs committed to `rust/docs/generated/`:
- Updated on every `cargo x verbs atlas` run
- Machine-readable JSON for downstream tooling

---

## Final Metrics

| Metric | Before | After |
|--------|--------|-------|
| ERRORs | 243 | 0 |
| WARNs | — | 709 (MISSING_CONCEPT: 626, COLLISION cross-domain: 82, NEAR_COLLISION: 1) |
| INFOs | 25 | 0 |
| Verbs in atlas | 959 | 959 |
| Phrases | ~4800 | 4,856 |
| Embedded patterns | ~14,600 | 14,673 |
| Coverage | ~99% | 99.8% |

### Remaining WARNs (documented, not blocking)

- **MISSING_CONCEPT (626):** Intent verbs without lexicon entries. The lexicon is supplementary — verb YAML `invocation_phrases` is the canonical intent surface (V-4). These can be populated incrementally.
- **COLLISION cross-domain (82):** Phrases shared across domains (e.g., "list products" in both cbu and trading-profile). These are correctly separated by pack/scope scoring. Documented as ACCEPTED.
- **NEAR_COLLISION (1):** Single near-collision pair within acceptable margin.
