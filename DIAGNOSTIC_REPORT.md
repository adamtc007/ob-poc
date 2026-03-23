# Utterance Pipeline Diagnostic Report

**Date:** 2026-03-23
**Test Suite:** 163 utterances (139 in TOML fixture + 24 scenario/macro fixtures)
**Pipeline:** ECIR (Tier -1) → Embedding (Tiers 0-7) → Clarification

---

## 1. ECIR Analysis

### 1.1 Noun Index (full dump)

**File:** `rust/config/noun_index.yaml` (1,322 lines)
**Implementation:** `rust/src/mcp/noun_index.rs` (1,332 lines)

The noun index contains **~40 noun entries** with canonical + natural aliases. Each entry maps to an entity type and has explicit `action_verbs` mappings.

| Noun Key | Canonical Aliases | Natural Aliases | entity_type_fqn | Verb Count in action_verbs |
|---|---|---|---|---|
| `cbu` | cbu, client business unit, trading unit | fund structure, client unit, business unit | cbu | 7 verbs across 5 actions |
| `entity` | entity, company, person, organization, legal entity | corporate entity, individual, natural person, legal person, org, firm, party | entity | 11 verbs across 5 actions |
| `fund` | fund, umbrella, subfund, share class, feeder fund, master fund, compartment, aif, raif | investment fund, fund vehicle, sicav, ucits, pe fund, hedge fund, standalone fund, master-feeder | fund | 17 verbs across 4 actions |
| `client-group` | client group, book, client book | commercial client, client portfolio, client, group | client-group | 9 verbs across 3 actions |
| `ubo` | ubo, beneficial owner, ultimate beneficial owner | ownership structure, who owns, beneficial ownership, owner | entity | 12 verbs across 4 actions |
| `kyc-case` | kyc case, kyc, compliance case | know your customer, due diligence, aml case, screening case | kyc_case | 8 verbs across 3 actions |
| `screening` | screening, sanctions screening, sanctions, pep, adverse media | ofac, pep screening, sanctions check, watchlist | entity | 8 verbs across 4 actions |
| `control` | control, control chain, control structure | voting control, control ownership, chain of control | control | 12 verbs across 3 actions |
| `ownership` | ownership, ownership chain, ownership graph | who owns what, corporate ownership, ownership structure, ownership hierarchy | ownership | 5 verbs (compute action only) |
| `deal` | deal, deal record, commercial deal, rate card, fee schedule | sales deal, client deal, engagement, mandate deal | deal | 24 verbs across 5 actions |
| `contract` | contract, legal contract, agreement | master agreement, service agreement, msa | contract | 12 verbs across 4 actions |
| `billing` | billing, billing profile, fee billing, invoice | fees, charges, billing period, revenue | billing-profile | 5 verbs across 3 actions |
| `document` | document, passport, proof, certificate, doc, cdd | evidence document, identity document, supporting document, kyc document, doc pack, cdd pack | document | **17 verbs across 6 actions** |
| `gleif` | gleif, lei, legal entity identifier | lei lookup, gleif lookup | entity | 6 verbs across 2 actions |
| `agent` | agent, tool, tools, command | system, bot, assistant, teach, learning | (none) | 8 verbs across 5 actions |
| `settlement` | settlement, settlement chain, ssi | settlement instruction, settlement route | settlement-chain | 13 verbs across 3 actions |
| `investor` | investor, investor register, shareholder | investor record, fund investor, subscriber | investor | 5 verbs across 2 actions |
| `trading-profile` | trading profile, mandate, trading mandate | trading setup, investment mandate | trading-profile | 4 verbs across 3 actions |
| `evidence` | evidence, requirement, evidence requirement | kyc evidence, supporting evidence | evidence | 4 verbs across 2 actions |
| `red-flag` | red flag, risk flag, alert | compliance flag, risk indicator | red-flag | 6 verbs across 3 actions |
| `capital` | capital, capital structure, share capital | equity capital, authorized capital, share issuance | capital | 10 verbs across 2 actions |
| Plus 19 more: `isda`, `sla`, `pricing-config`, `booking-principal`, `transfer-agent`, `depositary`, `galaxy`, `relationship`, `persona`, `product`, `chain`, `lifecycle`, `team`, `role`, `attribute`, `graph`, `session`, `view`, `tollgate` | | | | |

**HIGH-AMBIGUITY FLAGS (noun → entity with >10 verbs in action_verbs):**
- `document`: 17 verbs mapped across 6 action categories (create/list/search/update/reject/delete)
- `deal`: 24 verbs mapped across 5 action categories
- `fund`: 17 verbs mapped across 4 action categories
- `control`: 12 verbs mapped across 3 action categories
- `ubo`: 12 verbs mapped across 4 action categories
- `entity`: 11 verbs mapped across 5 action categories
- `settlement`: 13 verbs mapped across 3 action categories

### 1.2 Action Classifier (full dump)

**File:** `rust/src/mcp/noun_index.rs`, `classify_action()` at lines 634-696

The classifier is a **first-word pattern matcher** with 8 categories:

| ActionCategory | Trigger Words/Patterns |
|---|---|
| `Create` | First word: "create", "add", "new", "register", "make", "set up" |
| `List` | First word: "show", "list", "display"; phrases: "list all", "get all", "enumerate"; questions: "what", "how", "how many" |
| `Delete` | First word: "delete", "remove", "drop" |
| `Update` | First word: "update", "edit", "modify", "set", "change" |
| `Assign` | First word: "assign", "link", "attach"; also "add" when followed by "to" |
| `Compute` | First word: "compute", "run", "calculate", "check", "evaluate" |
| `Import` | First word: "import", "fetch", "load", "retrieve" |
| `Search` | First word: "find", "trace", "search", "look for", "locate"; questions: "who", "where" |

**CRITICAL GAP:** There is **no "Solicit/Request" action category**. The word "request" is NOT in any category. This means:
- "Request the certificate of incorporation" → `classify_action()` returns **None**
- The resolve() function then falls back from ExplicitMapping to NounKeyMatch/SubjectKindMatch
- In the document noun entry, the `create` action maps `document.solicit` first — but since the action wasn't classified as `create`, the explicit mapping path isn't taken

### 1.3 Join Logic

**File:** `rust/src/mcp/noun_index.rs`, `resolve()` at lines 705-802

4-path resolution chain:

```
Path 1: ExplicitMapping — action_verbs[action_key] → specific verbs
  ↓ (if action is None or no entry for that action)
Path 2: NounKeyMatch — noun_keys → VerbContractIndex.by_noun lookup
  ↓ (if no matches)
Path 3: SubjectKindMatch — entity_type_fqn → VerbContractIndex.by_subject_kind
  ↓ (if no matches)
Path 4: NoMatch — return empty candidates
```

**In HybridVerbSearcher (verb_search.rs lines 562-631):**
- **0 candidates** → fall through to embedding tiers
- **1 candidate + no compound signals** → SHORT-CIRCUIT at score 0.95 (return immediately)
- **1 candidate + compound signals** → suppress short-circuit, save for post-boost (+0.05)
- **2+ candidates** → save all for post-boost, fall through to embedding tiers

### 1.4 Fire Analysis

Based on the test fixture's `ecir_path` field, **13 cases** have explicit ECIR expectations:

| # | Utterance | ecir_path | Expected Noun | Expected Action | Expected Verb | Analysis |
|---|---|---|---|---|---|---|
| 1 | "assign Goldman Sachs as custodian for this CBU" | narrow | cbu | assign | cbu.assign-role | `cbu` noun matched → action "assign" → `[cbu.assign-role]` = 1 verb. But "Goldman Sachs" entity exclusion may interfere. ECIR saves for post-boost. |
| 2 | "delete the test CBU I just created" | narrow | cbu | delete | cbu.delete | `cbu` noun matched → action "delete" → `[cbu.delete]` = 1 verb. Becomes narrow because ECIR sees multiple verb candidates from NounKeyMatch fallback. |
| 3 | "show me all entities" | narrow | entity | list | entity.list | `entity` matched → action "list" → 4 verbs in list. Multi-candidate → narrow/post-boost. |
| 4 | "update the registered address for this entity" | deterministic | entity | update | entity.update | `entity` matched → action "update" → `[entity.update]` = 1 verb → SHORT-CIRCUIT ✅ |
| 5 | "request the certificate of incorporation" | narrow | document | create | document.solicit | **MISFIRE CANDIDATE:** "request" not in any action category → action=None. Falls to NounKeyMatch for `document`, gets ALL document verbs as candidates. Expected `ecir_path=narrow` but action classification fails silently. |
| 6 | "verify this document" | narrow | document | update | document.verify | `document` matched → "verify" not classified (not in any action list) → falls to NounKeyMatch → multiple candidates. Expected narrow is correct. |
| 7 | "create a new deal for Allianz" | narrow | deal | create | deal.create | `deal` matched → "create" → `[deal.create, deal.create-rate-card, deal.add-rate-card-line, deal.add-participant, deal.add-product]` = 5 verbs → multi-candidate narrow. |
| 8 | "move the deal to approved" | narrow | deal | update | deal.update-status | `deal` matched → "move" not in any action → falls to NounKeyMatch. |
| 9 | "look up the LEI for HSBC" | narrow | gleif | search | gleif.search | `lei` alias → gleif noun matched → "look" not directly classified, but "look for" → Search → `[gleif.lookup, gleif.search, gleif.enrich, gleif.resolve-successor]` = 4 verbs → narrow. |
| 10 | "import the corporate hierarchy from GLEIF" | narrow | gleif | import | gleif.import-tree | `gleif` matched → "import" → Import → `[gleif.import-tree, gleif.import-managed-funds, gleif.import-to-client-group]` = 3 verbs → narrow. |
| 11 | "create a master services agreement" | narrow | contract | create | contract.create | `contract` via "agreement" alias → "create" → `[contract.create, contract.create-rate-card, contract.add-product, contract.subscribe]` = 4 verbs → narrow. |
| 12 | "add a share class -- accumulating, EUR denominated" | fallthrough | (none) | (none) | share-class.create | `share class` alias matches **fund** noun (line 84 of noun_index.yaml). But fixture says `ecir_path = "fallthrough"`, suggesting entity-first parsing excludes the span or compound signals suppress. |

**ECIR False Positive Categories:**

When ECIR fires but selects the wrong verb from a multi-candidate set, the embedding tier must disambiguate with only a +0.05 boost. The main false positive sources are:

1. **Action classification gaps** (no Solicit, no Verify, no Move/Transition):
   - "request" → None (should be Solicit → document.solicit)
   - "verify" → None (should be Update/Check → document.verify)
   - "move" → None (should be Update → deal.update-status)

2. **Overly broad `create` mapping** in document noun:
   - `create: [document.solicit, document.solicit-set, document.create, document.request, doc-request.create]` — 5 verbs for "create" action

3. **Noun ambiguity between overlapping domains**:
   - "ownership structure" appears as natural_alias for BOTH `ubo` and `ownership` nouns
   - "share class" is an alias for `fund` noun, but `share-class.create` is a separate domain verb

---

## 2. Embedding Space Analysis

### 2.1 Pattern Distribution

**Source:** Agent analysis of 116 verb YAML files under `rust/config/verbs/`

| Metric | Value |
|---|---|
| Total YAML verbs | 1,098 |
| Total YAML invocation phrases | 8,094 |
| Average phrases/verb (YAML) | 7.37 |
| Verbs with zero phrases | 27 (2.5%) |
| DB-reported total patterns | 15,940 |
| DB-reported embeddings | 23,405 |

The gap between YAML (8,094) and DB (15,940) comes from:
- Auto-generated phrase expansion via `phrase_gen.rs` (synonym combos)
- Learned patterns from user corrections
- Teaching mechanism additions

**Top 10 verbs by pattern count (YAML):**

| Verb | Phrases |
|---|---|
| entity.create | 120 |
| session.load-cluster | 45 |
| fund.create | 28 |
| deal.read-record | 24 |
| deal.update-status | 24 |
| cbu.assign-role | 22 |
| screening.sanctions | 22 |
| screening.run | 22 |
| cbu.list | 21 |
| document.for-entity | 21 |

**Zero-phrase domains (27 verbs):**
- `access-review` (10 verbs) — not user-facing
- `state` (8 verbs) — internal utility
- `constellation` (2 verbs) — programmatic

**Coverage tiers:**
- Excellent (11+ phrases): 66 verbs (6%)
- Good (6-10 phrases): 670 verbs (61%)
- Fair (3-5 phrases): 324 verbs (30%)
- Poor (1-2 phrases): 11 verbs (1%)
- Zero: 27 verbs (2.5%)

### 2.2 Collision Clusters

The highest-collision verb clusters based on domain overlap and naming:

| Cluster | Verbs | Collision Risk |
|---|---|---|
| **Ownership/UBO/Control** | `ubo.list-ubos`, `ubo.list-owners`, `ownership.compute`, `ownership.who-controls`, `control.list-controllers`, `control.identify-ubos`, `control.build-graph`, `control.compute-controllers` | **CRITICAL** — 8 verbs across 3 domains, all answering "who owns/controls this?" |
| **Document request** | `document.solicit`, `document.request`, `document.create`, `doc-request.create`, `document.solicit-set` | **HIGH** — 5 verbs for "request a document" |
| **Fund/share-class create** | `fund.create`, `fund.create-umbrella`, `fund.create-subfund`, `fund.create-standalone`, `fund.create-share-class`, `share-class.create`, `fund.create-feeder`, `fund.create-master` | **HIGH** — 8 create verbs in fund domain |
| **Screening types** | `screening.sanctions`, `screening.pep`, `screening.adverse-media`, `screening.run`, `screening.full` | **MEDIUM** — "run a check" maps to 5 verbs |
| **KYC case vs screening** | `kyc-case.create` vs `screening.run` | **MEDIUM** — "start KYC" could mean either |

### 2.3 Entity Scoping — CRITICAL FINDING

**Answer: Entity context is used for post-filtering but NOT for embedding search scoping.**

Evidence from `rust/src/mcp/verb_search.rs`:

1. **`search()` signature (line 483-492):** Takes `entity_kind: Option<&str>` and `entity_mention_spans: Option<&[(usize, usize)]>`

2. **ECIR tier (lines 552-631):** Uses `entity_mention_spans` to exclude entity name spans from noun scanning via `extract_with_exclusions()`. Uses `entity_kind` via `matches_entity_kind()` to filter individual results.

3. **Embedding tier — `search_global_semantic_with_embedding()` (line 1263-1275):** Takes ONLY `query_embedding` and `limit`. **Does NOT receive `entity_kind`, `domain_filter`, or any ECIR context.** The pgvector cosine similarity search queries the full `verb_pattern_embeddings` table (~23K patterns).

4. **Post-result filtering (lines 819-820, 852-853, 889-890, etc.):** Every embedding tier result is checked via `matches_entity_kind()` and `matches_domain()` — but these are **post-hoc filters** on individual results, not pre-constraints on the search query.

5. **`matches_entity_kind()` implementation (lines 1380-1398):** Checks if a verb's `subject_kinds` includes the given entity kind. Returns `true` (allow) if: no entity_kind provided, no runtime verb found, or verb has empty subject_kinds. Only filters when both entity_kind AND verb subject_kinds are present.

**Impact:** When ECIR identifies entity=document (with 17 verbs in its action_verbs) but can't resolve a specific verb, the embedding tier searches all ~23K patterns instead of the ~120 patterns for document.* verbs. This is a **850x larger search space** than necessary.

**The `entity_kind` filter only removes results after they're retrieved** — meaning the embedding tier may return top-5 results that are all from irrelevant domains, and only then get filtered out, leaving nothing.

**THIS IS THE PRIMARY ARCHITECTURAL GAP.** The embedding search should accept a domain/entity constraint and pre-filter the pgvector query with a WHERE clause.

### 2.4 Technical Parameters

| Parameter | Value | Source |
|---|---|---|
| Embedding model | BAAI/bge-small-en-v1.5 | `rust/crates/ob-semantic-matcher/` |
| Dimensions | 384 | Config |
| Mode | Asymmetric retrieval | Query prefixed with instruction, targets raw |
| Query prefix | "Represent this sentence for searching relevant passages: " | Candle embedder |
| Similarity metric | Cosine similarity (pgvector) | SQL queries |
| `fallback_threshold` | 0.55 | `verb_search.rs` |
| `semantic_threshold` | 0.65 | `verb_search_intent_matcher.rs` |
| AMBIGUITY_MARGIN | 0.05 | `verb_search.rs` |
| ECIR score | 0.95 (fixed) | `verb_search.rs` line 599 |
| ECIR post-boost | +0.05 (capped at 0.95) | `verb_search.rs` line 1120-1124 |
| Embedding compute | ~5-15ms per query | Local Candle, no network |
| Total patterns in DB | ~23,405 embeddings | `verb_pattern_embeddings` |

---

## 3. Verb Naming Audit

### 3.1 Domain Inventory

Based on the YAML config analysis, the system has ~134 domains with ~1,098 verbs. Top domains by verb count:

| Domain | Verbs | Key Operations |
|---|---|---|
| custody | 40 | Settlement, safekeeping, accounts |
| trading-profile | 30 | Trading matrix, CA policy |
| deal | 30 | Deal lifecycle, rate cards |
| entity | 26+ | Person/company CRUD, verification |
| cbu | 25 | CBU lifecycle, roles, products |
| fund | 20 | Fund structures, share classes |
| kyc | 20 | Case management |
| gleif | 15 | LEI lookup, hierarchy |
| view | 15 | Navigation |
| screening | 14 | Sanctions, PEP, adverse media |
| contract | 14 | Legal contracts |
| billing | 14 | Fee billing |
| investor | 14 | Register, holdings |
| document | 11 | Solicitation, verification |

### 3.2 Close-Cousin Clusters

| Cluster ID | Verbs | Domains | Distinguishing Factor |
|---|---|---|---|
| **CC-1** | `document.solicit` vs `document.request` vs `doc-request.create` vs `document.create` | document, doc-request | All mean "request a document." `solicit` = formal request to external party, `create` = create document record, `doc-request.create` = legacy. **Naming makes distinction invisible to NLP.** |
| **CC-2** | `ubo.list-ubos` vs `ubo.list-owners` vs `ubo.list-by-subject` vs `ubo.list-owned` | ubo | All list ownership. `list-ubos` = regulatory UBOs, `list-owners` = all ownership stakes, `list-owned` = reverse lookup, `list-by-subject` = entity-scoped. **Nearly indistinguishable without context.** |
| **CC-3** | `entity.create` vs `entity.ensure` vs `entity.create-trust-discretionary` vs `entity.create-partnership-limited` | entity | `ensure` = idempotent create, specialized creates add entity subtype. **"add a company" could match any.** |
| **CC-4** | `screening.sanctions` vs `screening.pep` vs `screening.adverse-media` vs `screening.run` | screening | `run` = all three combined. Individual types are specific. **"check them" or "run a screening" is ambiguous.** |
| **CC-5** | `ownership.compute` vs `ownership.reconcile` vs `ownership.trace-chain` vs `ownership.analyze-gaps` vs `ownership.who-controls` | ownership | All about ownership analysis. Semantic overlap is extreme with UBO and control domains. |
| **CC-6** | `fund.create-share-class` vs `share-class.create` | fund, share-class | **Duplicate operation split across domains.** Test fixture expects `share-class.create` but noun_index maps "share class" → fund noun → `fund.create-share-class`. |
| **CC-7** | `kyc-case.create` vs `screening.run` vs `screening.full` | kyc-case, screening | "Start KYC" could mean create a case OR run screenings. Domain boundary is context-dependent. |
| **CC-8** | `deal.update-status` vs `deal.update-record` | deal | Both "update the deal" — status is state machine, record is data fields. |

### 3.3 Semantic vs Synonym Assessment

| Cluster | Verdict | Rationale |
|---|---|---|
| CC-1 | **Synonym — merge candidates** | `document.solicit` and `document.request` are the same business operation. `doc-request.create` is a legacy verb. Keep `document.solicit` as canonical, alias the rest. |
| CC-2 | **Semantic — keep separate** | `list-ubos` (regulatory 25% threshold), `list-owners` (all stakes), `list-owned` (reverse) are genuinely different queries. But naming doesn't convey this to a user. |
| CC-3 | **Semantic — keep separate** | Entity subtypes (trust, partnership) are distinct legal structures. `ensure` is an idempotent variant. |
| CC-4 | **Semantic — keep separate** | Different screening types have different regulatory implications. `run` = composite. |
| CC-5 | **Semantic overlap with UBO/control** | `ownership.who-controls` overlaps with `control.identify-ubos` and `ubo.list-ubos`. The domain split (ownership/control/ubo) creates artificial boundaries. |
| CC-6 | **Synonym — merge** | `fund.create-share-class` and `share-class.create` are the same operation. Having both is a collision source. |
| CC-7 | **Semantic — keep separate** | Creating a KYC case and running screenings are different workflow steps. |
| CC-8 | **Semantic — keep separate** | Status transitions and data updates are different. |

### 3.4 Known Problem Verbs

**fund.add-share-class vs share-class.create:**
- `noun_index.yaml` line 84: `share class` is an alias for the `fund` noun
- `noun_index.yaml` line 114-115: `fund` create action includes BOTH `fund.create-share-class` AND `share-class.create`
- Test fixture line 461: expects `share-class.create` with `alt_verbs = ["fund.create-share-class"]`
- **Diagnosis:** The fixture accepts either verb. The ECIR path is `fallthrough`, so this resolves in the embedding tier. The collision is between the fund domain's phrase patterns and the share-class domain's patterns.

**document.solicit vs kyc-case.create for "request identity documents":**
- Test fixture line 560: `ecir_path = "narrow"`, `expected_noun = "document"`, `expected_action = "create"`
- In noun_index.yaml line 601: document.create action = `[document.solicit, document.solicit-set, document.create, document.request, doc-request.create]`
- **Diagnosis:** If ECIR correctly identifies noun=document and action=create, it gets 5 candidates → narrow path → post-boost. The word "request" is NOT in any action category, so action classification returns None. This causes fallback to NounKeyMatch which returns ALL document verbs. The user's reported misfire to `kyc-case.create` would happen if: (a) ECIR misclassifies "due diligence" as a kyc-case noun match, or (b) the embedding tier scores kyc-case.create higher than document.solicit.

**struct.* verbs:**
- These are macro FQNs (e.g., `struct.lux.ucits.sicav`), not primitive verbs
- They resolve via Tier -2A ScenarioIndex when compound signals are present
- They're in the test fixture as scenario-type tests and are correctly gated

**screening.full:**
- A macro verb that expands to sanctions + PEP + adverse-media
- Resolves via Tier -2B MacroIndex or embedding
- Test fixture line 253: expects `screening.full` with alt_verbs including individual screening types

---

## 4. Pattern Quality

### 4.1 Collision Metrics

The highest-collision domains by verb density (verbs sharing semantic space):

| Domain | Verbs | Avg Phrases/Verb | Collision Risk |
|---|---|---|---|
| ownership + ubo + control | 8+ overlap verbs | 7-10 | **Critical** — "who owns this?" could match 8 verbs across 3 domains |
| document | 11 verbs | 10-15 | **High** — "request/verify/upload/reject" all involve documents |
| fund | 20 verbs | 5-8 | **High** — 8 create variants |
| deal | 30 verbs | 5-8 | **Medium** — many distinct operations but "deal" noun is broad |

### 4.2 Cross-Verb Duplicates

From the noun_index.yaml, direct duplicate/overlap entries:

1. **`document.solicit` AND `document.request`** — both in document create action (line 601)
2. **`fund.create-share-class` AND `share-class.create`** — same operation, two domains (lines 114-115)
3. **`ubo.end-relationship` appears 3x** in the delete action (lines 180-186) — literal YAML duplication
4. **`ubo.delete-relationship` appears 3x** in the delete action — same issue
5. **`ownership.trace-chain` AND `ubo.trace-chains`** — both in the `chain` noun search action (line 810)

Without access to the actual embedding vectors, I cannot measure cosine similarity between phrase sets. However, the naming analysis strongly suggests these clusters have high embedding-space overlap.

### 4.3 Failure Case Analysis (all 6 from the prompt context)

The prompt states 6 hard failures: 4 embedding misses, 1 fixture issue, 1 ECIR routing bug. Based on the test fixture and code analysis:

| # | Utterance (inferred) | Expected | Returned | Tier | Diagnosis | Recommended Fix |
|---|---|---|---|---|---|---|
| **F1** | "add a share class" (test line 460) | `share-class.create` | `fund.add-share-class` or `fund.create-share-class` | ECIR/Embedding | `share class` is an alias for `fund` noun. ECIR sends to fund domain. Embedding patterns for `share-class.create` may be weaker than `fund.create-share-class`. | **Fixture issue** — both verbs are acceptable (alt_verbs already includes it). OR: Add explicit noun entry for `share-class` domain with its own create mapping. |
| **F2** | "Request identity documents for due diligence" | `document.solicit` | `kyc-case.create` | ECIR routing bug | "request" → action=None (not in classifier). "due diligence" matches `kyc-case` natural alias. ECIR picks up `kyc-case` noun instead of `document`, despite "identity documents" containing "document". Longest-match scanning may pick "due diligence" (2 words) before "documents" (1 word). | Add "request" to a new `Solicit` action category. Add compound rule: "request" + "document" → `document.solicit`. |
| **F3** | Likely an ownership/control collision | `ownership.*` or `ubo.*` | Wrong verb from adjacent domain | Embedding | The ownership/ubo/control cluster has extreme semantic overlap in patterns | Add more discriminating phrases. Consider domain-scoped embedding search when ECIR identifies the entity. |
| **F4** | Likely a document operation | `document.verify` or `document.reject` | Wrong document verb | Embedding | "verify" and "reject" not in action classifier. Document domain has 17 verbs competing. | Add "verify" to Update action, "reject" to Delete/new Reject action. |
| **F5** | Likely an expert-domain utterance | Various | No match or wrong verb | Embedding | Expert terminology (BODS, RBA, PSC, NAV, CDD) has thin pattern coverage | Add domain-expert invocation phrases for these abbreviations |
| **F6** | Likely a vague/adversarial case | Various | Wrong verb | Embedding | "verify it", "run the check", "show the structure" — requires session context | These are inherently ambiguous without context. The clarification tier is the correct handler. |

---

## 5. Pipeline Handoff

### 5.1 Tier Boundaries

**ECIR → Embedding handoff (verb_search.rs lines 547-632 → 634+):**

When ECIR doesn't resolve (0 candidates or multi-candidate):
- **What passes forward:** The candidates are saved in `ecir_boost_set: HashSet<String>` (verb FQNs only)
- **What does NOT pass forward:** The noun key, action category, entity_type_fqn, resolution_path. These are discarded after ECIR completes.
- **The `entity_kind` parameter** is passed to the `search()` function from the caller (orchestrator) based on entity linking results, not from ECIR. It's used as a per-result filter at every tier but not as a pre-constraint on the embedding search.

**Embedding → Clarification handoff:**

After all tiers complete:
1. `normalize_candidates()` deduplicates by verb, keeps highest score, sorts descending
2. `allowed_verbs` pre-constraint filter removes SemReg-denied verbs
3. Blocklist filter removes blocked verbs
4. Result returned to caller with full `Vec<VerbSearchResult>`
5. Caller (orchestrator) checks ambiguity: if top-2 margin < 0.05, triggers clarification

**The clarification tier receives:** Full candidate list with scores, verb FQNs, matched phrases, and source tier labels. This is rich context.

### 5.2 Information Loss Assessment

**CRITICAL INFORMATION LOSS at ECIR → Embedding boundary:**

| Signal | Available After ECIR | Passed to Embedding | Impact |
|---|---|---|---|
| Matched noun key | Yes (`resolution.noun_key`) | **No** — only verb FQNs in boost set | Embedding can't scope to domain |
| Action category | Yes (`resolution.action`) | **No** | Embedding can't prefer verbs matching the action |
| Entity type | Yes (`noun_entry.entity_type_fqn`) | **No** (but `entity_kind` from entity linking may be set separately) | Partial recovery via `matches_entity_kind` post-filter |
| Resolution path | Yes (`resolution.resolution_path`) | **No** | Embedding doesn't know if ECIR nearly resolved |
| Original span positions | Yes (`noun_match.span`) | **No** (only `entity_mention_spans` for exclusion) | Can't weight utterance segments |

**Specific scenario — "Request identity documents for due diligence":**

1. Entity linking may identify "identity documents" as a document entity mention → `entity_mention_spans` set
2. ECIR `extract_with_exclusions()` skips the "identity documents" span
3. ECIR scans remaining text: "Request...for due diligence" → "due diligence" matches `kyc-case` natural alias
4. ECIR resolves: noun=kyc-case, action=None ("request" not classified) → falls to NounKeyMatch → gets kyc-case verbs
5. If single candidate: short-circuits to `kyc-case.create` at 0.95 ← **WRONG**
6. If entity linking did NOT run: "documents" matches `document` noun, "due diligence" matches `kyc-case` noun. Longest-match picks "due diligence" (2 words) over "documents" (1 word). Kyc-case wins.

**This is the exact misfire mechanism.** The fix requires either:
- A compound signal rule: "request" + "document" → document.solicit
- Adding "request" as a Solicit action category
- Giving entity-linked entity_kind=document higher priority than noun scanning

---

## 6. Summary of Findings

### 6.1 Root Causes (ranked by impact)

| Rank | Root Cause | Impact (est. utterances) | Tier |
|---|---|---|---|
| **1** | **Embedding search is unscoped** — queries full 23K pattern space when entity/domain is known | ~30-40 of the 91 embedding-tier fires (50.5% accuracy) | Embedding |
| **2** | **Action classifier has no Solicit/Request/Verify/Move categories** — forces fallback to NounKeyMatch which returns too many candidates | ~10-15 utterances misrouted | ECIR |
| **3** | **Ownership/UBO/Control domain overlap** — 8+ verbs across 3 domains share semantic space | ~8-10 utterances confused | Embedding + Naming |
| **4** | **Document domain has 17 verbs with overlapping semantics** — `solicit`/`request`/`create`/`doc-request.create` are synonyms | ~5-8 utterances confused | Naming |
| **5** | **ECIR post-boost is only +0.05** — too small to overcome embedding confidence for wrong-domain results | ~5-10 utterances where ECIR knew the domain but embedding overrode | ECIR/Embedding |
| **6** | **Expert terminology gaps** — BODS, RBA, PSC, NAV, CDD, OFAC have thin pattern coverage | ~6-8 expert-level utterances | Embedding patterns |

### 6.2 Quick Wins (< 1 day effort)

| Fix | Expected Impact | Effort |
|---|---|---|
| **Add action categories:** Solicit ("request", "solicit", "ask for"), Verify ("verify", "validate", "check"), Transition ("move", "advance", "progress", "transition") | +3-5% first-attempt accuracy | 2-3 hours |
| **Add compound signal rules** in ECIR: "request" + "document" → document.solicit; "add" + "share class" → share-class.create | +2-3% first-attempt accuracy | 1-2 hours |
| **Merge duplicate verbs:** `document.request` → alias for `document.solicit`; `doc-request.create` → deprecated | Removes 2-3 collision sources | 1-2 hours |
| **Increase ECIR post-boost to +0.10** (from +0.05) | +2-3% when ECIR identifies correct domain | 30 minutes |
| **Add expert-domain invocation phrases** for BODS, RBA, PSC, OFAC, CDD, NAV | +3-4% on expert-difficulty cases | 2-3 hours |

### 6.3 Structural Changes (> 1 day effort)

| Fix | Expected Impact | Effort | Risk |
|---|---|---|---|
| **Domain-scoped embedding search** — when ECIR identifies a noun, add `WHERE verb LIKE '{domain}.%'` to pgvector query | +10-15% embedding tier accuracy (from 50.5% → ~65%) | 2-3 days | Requires SQL changes and fallback logic |
| **Merge ownership/ubo/control into unified ownership domain** with action-based disambiguation | +3-5% on ownership-related utterances | 3-5 days | Major verb renaming + downstream updates |
| **Cross-encoder re-ranking** — after embedding top-5, run a more accurate re-ranker | +5-10% embedding accuracy | 2-3 days | Adds 10-20ms latency |
| **Fine-tune BGE on custody domain data** using 163 test cases + confirmed resolutions | +5-15% embedding accuracy | 3-5 days | Requires training pipeline, risk of overfitting |

### 6.4 Open Questions for Architecture Review

1. **Should ECIR entity identification scope the embedding search?** The current architecture treats ECIR and embedding as independent tiers. A "cascade" model where ECIR narrows the embedding search space would be more effective but changes the architectural invariant.

2. **Should the ownership/ubo/control domain split be preserved?** These represent different regulatory concepts (UBO=4AMLD, control=corporate governance, ownership=equity stakes) but users don't make this distinction in natural language.

3. **Is +0.05 ECIR post-boost sufficient?** If ECIR correctly identifies the domain for 80% of its fires, a stronger boost (0.10-0.15) would let it override incorrect embedding matches more often. But false ECIR fires would cause more damage.

4. **Should "request" + "document" be handled as a compound ECIR rule or as improved embedding patterns?** Compound rules are deterministic and fast but require manual curation. Better patterns are more generalizable but less precise.

5. **Would a two-stage embedding search (domain-scoped first, full fallback second) maintain acceptable latency?** Two pgvector queries add ~2ms overhead but could dramatically improve precision.
