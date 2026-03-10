# TODO: SemTaxonomy — Rip and Replace Sage/Coder Pipeline

## Classification: RIP AND REPLACE
## Target: Replace the Sage → OutcomeIntent → Coder → DSL pipeline
## Preserve: DSL execution engine, REPL, verb registry, governance layer, snapshot layer
## Delete: OutcomeIntent struct, multi-stage classification chain, Coder verb-resolution logic

---

## CONTEXT: What This Is and Why

### The Problem

The current Sage → Coder → DSL pipeline achieves **8.21% end-to-end accuracy** (11/134 utterances). The legacy pipeline scores 43.28%. The cause is multiplicative semantic loss across 5+ lossy stages — each stage compresses the user's intent into a lossy intermediate before verb selection. The math: `0.6^5 ≈ 0.078`, almost exactly the observed result. Incremental tuning (declash, domain hints, richer payloads) produced only +0.75pp improvement. The pipeline **shape** is the problem, not the individual stage quality.

### The Fix

Replace the multi-stage classify-compress-resolve chain with:

1. **Sage uses SemOS as a read-only research tool** — queries the registry, resolves entities, inspects current state, discovers available verbs. All grounded in SemOS data, not LLM inference.
2. **Single LLM composition step** — receives the raw utterance (never compressed) + SemOS-grounded context + small verb surface with full contracts. Composes multi-statement runbooks from known tools.

Two stages. Stage 1 is deterministic and grounded (r ≈ 0.98). Stage 2 is constrained LLM composition (r ≈ 0.75). Expected: `0.98 × 0.75 = 0.735` — from 8.21% to ~73%.

### The Rip Boundary

**Delete:** `OutcomeIntent` struct, Sage preclassification (plane/polarity/domain → OutcomeIntent), Coder verb-resolution (OutcomeIntent → verb), Coder argument-assembly (verb + OutcomeIntent → args), serve/delegate routing depending on OutcomeIntent.

**Keep:** DSL execution engine, REPL, verb registry (add 8 new verbs), governance layer, snapshot layer, ABAC, all 653 existing verb implementations.

### Architecture: How It Works

```
User: "BNP"
         │
         ▼
Sage calls discovery.cascade-research("BNP")
         │
         ├─ Layer 1: fuzzy entity search across all types
         │   → BNP Paribas SA (company), BNP Securities (company),
         │     BNP Fund Admin (client-group), BNP Luxembourg SICAV (cbu)
         │
         ├─ Layer 2: activity context per top hit
         │   → BNP Paribas SA: onboarding 65% (blocked on UBO), deal #4521 pending
         │   → BNP Securities: KYC review 40% complete
         │
         ├─ Layer 3: relationships for active hits
         │   → BNP Paribas SA: 3 UBO owners (1 unverified), 2 sub-funds
         │
         └─ Layer 4: intent hints (deterministic rules, not LLM)
             → continue_onboarding (high), progress_deal (medium)

Sage presents grounded context to user.
User: "the UBO issue on BNP Paribas"

Sage calls discovery.entity-context + discovery.available-actions
  → verb surface: 5 verbs (list-ubo-chain, verify-beneficial-owner, etc.)
  → current UBO state: 3 owners, 1 unverified

User: "verify Klaus Weber"
  → Single-verb fast path: onboarding.verify-beneficial-owner
  → Args bound from context: entity=BNP Paribas SA, owner=Klaus Weber
  → Write → confirm → execute

User: "actually do the full check — verify Klaus, generate the UBO report, flag gaps"
  → Multi-verb LLM composition (one call):
  → Runbook: 3 statements from the 5-verb surface
  → Validated against verb contracts → confirmed → executed
```

---

## DISCOVERY DOMAIN: 8 NEW VERBS

All read-only. All in a new `discovery` domain. Registry grows from 653 to 661.

### Verb 1: discovery.search-entities

**Purpose:** Fuzzy cross-type entity search. Cold-start verb.

```yaml
verb: discovery.search-entities
domain: discovery
polarity: read

parameters:
  query:
    type: string
    required: true
    description: >
      Free-text search term. Matched against entity name, aliases,
      LEI codes, registration numbers, known abbreviations.
      Fuzzy matching (Levenshtein, trigram, prefix).
    examples: ["BNP", "Luxembourg SICAV", "Mueller", "KYC-2024-0891"]
  entity_types:
    type: array[EntityType]
    required: false
    default: null  # all types
    allowed_values: [company, partnership, cbu_group, cbu, sub_fund,
                     client_group, fund_structure, beneficial_owner,
                     deal, engagement]
  max_results:
    type: integer
    required: false
    default: 10
    min: 1
    max: 50
  include_inactive:
    type: boolean
    required: false
    default: false
    description: Include entities with no active engagements.

returns:
  type: SearchResultSet
  fields:
    total_matches: integer
    results: array[EntitySearchHit]

EntitySearchHit:
  entity_id: uuid
  entity_type: EntityType  # company, partnership, cbu, etc.
  name: string
  aliases: array[string]
  match_score: float  # 0.0-1.0
  match_field: string  # "name", "alias", "lei", "registration_number"
  summary:
    jurisdiction: string | null
    status: enum[active, inactive, pending, archived]
    has_active_engagement: boolean
    primary_domain: string | null
    last_activity: datetime | null
```

**Implementation notes:**
- Query path: `entities` table + alias table + GLEIF data.
- Use PostgreSQL `pg_trgm` for trigram similarity on name/alias fields.
- Prefix match and exact match on LEI/registration codes.
- Rank by match_score descending. Boost entities with has_active_engagement=true.
- <200ms target on production-scale data.

---

### Verb 2: discovery.entity-context

**Purpose:** Activity state map for a resolved entity. What's in flight across all domains.

```yaml
verb: discovery.entity-context
domain: discovery
polarity: read

parameters:
  entity_id:
    type: uuid
    required: true
  include_completed:
    type: boolean
    required: false
    default: false

returns:
  type: EntityContext
  fields:
    entity_id: uuid
    entity_type: EntityType
    name: string
    activities: array[ActivityState]
    signals: ActivitySignals

ActivityState:
  domain: DomainId
  activity_type: enum[onboarding, deal, kyc_review, fund_setup,
                       client_group_mgmt, compliance_review, reporting]
  phase: string  # domain-specific phase name
  status: enum[not_started, in_progress, blocked, pending_review,
               complete, archived]
  completion_pct: float | null  # 0.0-1.0
  blockers: array[string]  # human-readable
  last_activity: datetime
  last_actor: string | null
  linked_entities: array[LinkedEntityRef]

LinkedEntityRef:
  entity_id: uuid
  entity_type: EntityType
  name: string
  relationship: string  # "parent", "deal_counterparty", "custodian", etc.

ActivitySignals:
  has_active_onboarding: boolean
  has_active_deal: boolean
  has_active_kyc: boolean
  has_incomplete_ubo: boolean
  has_pending_documentation: boolean
  days_since_last_activity: integer | null
  stale: boolean  # true if no activity in >30 days
```

**Implementation notes:**
- Query engagement/activity tables across all domains for the entity.
- `signals` object is computed server-side from activity state — derived, not stored.
- Deterministic: same entity state → same signals, always.

---

### Verb 3: discovery.entity-relationships

**Purpose:** Relationship graph traversal for a resolved entity.

```yaml
verb: discovery.entity-relationships
domain: discovery
polarity: read

parameters:
  entity_id:
    type: uuid
    required: true
  relationship_types:
    type: array[RelationshipType]
    required: false
    default: null  # all types
    allowed_values: [ownership, ubo_chain, fund_structure, deal_link,
                     onboarding_link, client_group, service_provider,
                     regulatory]
  max_depth:
    type: integer
    required: false
    default: 2
    min: 1
    max: 5

returns:
  type: RelationshipGraph
  fields:
    entity_id: uuid
    entity_type: EntityType
    name: string
    relationships: array[Relationship]
    summary: RelationshipSummary

Relationship:
  relationship_type: RelationshipType
  direction: enum[outbound, inbound]
  target:
    entity_id: uuid
    entity_type: EntityType
    name: string
  depth: integer  # 1=direct, 2+=transitive
  metadata: map[string, string]
    # Examples per type:
    # ownership: {percentage: "42%", verified: "true"}
    # fund_structure: {role: "sub-fund", domicile: "Luxembourg"}
    # deal_link: {deal_phase: "commercial_terms", deal_id: "..."}
    # service_provider: {role: "custodian", active: "true"}

RelationshipSummary:
  total_relationships: integer
  ownership_chain_depth: integer | null
  ubo_count: integer | null
  ubo_verified_count: integer | null
  sub_fund_count: integer | null
  active_deal_count: integer
  active_onboarding_count: integer
  client_groups: array[string]
```

**Implementation notes:**
- Backed by existing relationship/ownership tables + AffinityGraph.
- Traversal MUST terminate at max_depth. No cycles.
- Relationship metadata is typed per relationship type.

---

### Verb 4: discovery.cascade-research

**Purpose:** Composite verb. Fires full Layer 1-3 cascade in one call. Cold-start verb.

```yaml
verb: discovery.cascade-research
domain: discovery
polarity: read

parameters:
  query:
    type: string
    required: true
    description: Free-text entity search term.
  top_n:
    type: integer
    required: false
    default: 5
    min: 1
    max: 10
  include_relationships:
    type: boolean
    required: false
    default: true

returns:
  type: CascadeResult
  fields:
    query: string
    total_entity_matches: integer
    entities: array[ResearchedEntity]

ResearchedEntity:
  # Layer 1 — search hit
  entity_id: uuid
  entity_type: EntityType
  name: string
  aliases: array[string]
  match_score: float
  match_field: string

  # Layer 2 — activity context (null if entity had no activities)
  context: EntityContext | null

  # Layer 3 — relationships (null if not requested or no relationships)
  relationships: RelationshipGraph | null

  # Layer 4 — derived signals and intent hints
  signals: ActivitySignals
  likely_intents: array[IntentHint]

IntentHint:
  intent: string  # see intent derivation rules below
  confidence: enum[high, medium, low]
  reason: string  # human-readable explanation
```

**Cascade execution:**
1. Call `search-entities` with the query → entity hits
2. For top_n hits: call `entity-context` for each (parallelize)
3. For hits with active engagements AND include_relationships=true: call `entity-relationships` (parallelize)
4. Compute `likely_intents` from `signals` using deterministic rules (below)
5. Return assembled `CascadeResult`

**Layer 4 intent derivation rules (deterministic — not LLM):**

| Activity Pattern | Intent | Confidence |
|-----------------|--------|------------|
| Onboarding in_progress + has blockers | `continue_onboarding` | high |
| Deal active + no onboarding linked | `start_onboarding` | high |
| KYC in_progress + incomplete checks | `continue_kyc` | high |
| Onboarding complete + no KYC started | `initiate_kyc` | medium |
| has_incomplete_ubo = true | `verify_ubo` | high |
| has_pending_documentation = true | `upload_documents` | medium |
| All activities complete | `review_report` | low |
| No activities + entity exists | `check_status` | low |
| Entity not found (no hits) | `create_entity` | medium |
| Multiple entities match, different types | `disambiguate` | N/A |

These are deterministic pattern matches on the `ActivitySignals` booleans. Same state → same intents, always. Sage presents them as options, not decisions.

**Performance:** <500ms for top_n=5. Layers 2+3 execute in parallel per entity.

---

### Verb 5: discovery.available-actions

**Purpose:** Verb surface discovery. What can Sage compose from?

```yaml
verb: discovery.available-actions
domain: discovery
polarity: read

parameters:
  domain:
    type: DomainId
    required: true
  entity_type:
    type: EntityType
    required: true
    description: Only verbs whose subject_kinds include this type are returned.
  aspect:
    type: string
    required: false
    description: phase_tag filter. If omitted, return all grouped by phase_tag.
  polarity:
    type: enum[read, write, all]
    required: false
    default: all

returns:
  type: ActionSurface
  fields:
    domain: DomainId
    entity_type: EntityType
    total_verbs: integer
    groups: array[ActionGroup]

ActionGroup:
  aspect: string  # phase_tag value
  verbs: array[VerbSummary]

VerbSummary:
  verb_id: VerbId  # e.g. "onboarding.verify-beneficial-owner"
  name: string  # action-encoded name
  description: string
  polarity: enum[read, write]
  parameters: array[ParamSummary]
  preconditions: array[string] | null
  governance_status: enum[active, gated, pending]

ParamSummary:
  name: string
  type: string
  required: boolean
  description: string
```

**Implementation notes:**
- Pure in-memory registry lookup. Filter by domain + subject_kinds contains entity_type + optional polarity.
- Group results by phase_tag.
- Apply ABAC: exclude verbs the current user cannot execute.
- This is how Sage discovers the constrained verb surface for composition.

---

### Verb 6: discovery.verb-detail

**Purpose:** Full contract for a specific verb. Used during composition.

```yaml
verb: discovery.verb-detail
domain: discovery
polarity: read

parameters:
  verb_id:
    type: VerbId
    required: true
    description: Fully qualified verb ID (e.g. "onboarding.verify-beneficial-owner")

returns:
  type: VerbContract
  fields:
    verb_id: VerbId
    domain: DomainId
    name: string
    description: string
    polarity: enum[read, write]
    subject_kinds: array[EntityType]
    phase_tags: array[string]
    parameters: array[ParameterSpec]
    preconditions: array[Precondition] | null
    postconditions: array[string] | null
    governance:
      status: enum[active, gated, pending, deprecated]
      required_roles: array[string]
      abac_policy: string | null

ParameterSpec:
  name: string
  type: string  # "uuid", "string", "enum[a,b,c]", "array[uuid]", etc.
  required: boolean
  default: any | null
  description: string
  validation: string | null  # regex, range, enum values
```

---

### Verb 7: discovery.inspect-data

**Purpose:** Read-only data snapshot for entity + domain + aspect.

```yaml
verb: discovery.inspect-data
domain: discovery
polarity: read

parameters:
  entity_id:
    type: uuid
    required: true
  domain:
    type: DomainId
    required: true
  aspect:
    type: string
    required: false
    description: phase_tag. If omitted, summary across all aspects.
    examples: ["ownership", "documentation", "structure", "compliance"]
  depth:
    type: enum[summary, detail]
    required: false
    default: summary

returns:
  type: DataSnapshot
  fields:
    entity_id: uuid
    entity_type: EntityType
    domain: DomainId
    aspect: string | null
    snapshot_at: datetime
    data: map[string, any]  # polymorphic by domain+aspect
    summary:
      record_count: integer
      complete_count: integer
      incomplete_count: integer
      blocked_count: integer
      last_modified: datetime | null
      notable_gaps: array[string]
```

**Implementation notes:**
- Routes through governed query layer. ABAC enforced.
- Returns immutable snapshot (timestamped). Reads serve from cache; writes force refresh.

---

### Verb 8: discovery.search-data

**Purpose:** Fuzzy search within a domain's data for an entity.

```yaml
verb: discovery.search-data
domain: discovery
polarity: read

parameters:
  entity_id:
    type: uuid
    required: true
  domain:
    type: DomainId
    required: true
  query:
    type: string
    required: true
    description: Fuzzy search against field values, descriptions, status labels, notes.
    examples: ["pending", "Luxembourg", "unverified", "missing"]
  aspect:
    type: string
    required: false
  max_results:
    type: integer
    required: false
    default: 20

returns:
  type: DataSearchResults
  fields:
    total_matches: integer
    results: array[DataSearchHit]

DataSearchHit:
  record_type: string
  record_id: uuid
  match_field: string
  match_value: string
  match_score: float
  context: map[string, string]
    # e.g. {document_type: "KYC Form", status: "pending", entity_name: "BNP Paribas SA"}
```

---

## CORE STRUCTS

These are the Rust structs that form the new pipeline. They replace OutcomeIntent and the Sage → Coder handoff.

### SageSession

```rust
/// The session state. All fields trace to a SemOS query result or user input.
/// No inferred/assumed fields. Each field records provenance.
pub struct SageSession {
    pub session_id: Uuid,
    pub started_at: DateTime<Utc>,

    // Accumulated SemOS-grounded context
    pub cascade_result: Option<CascadeResult>,         // from cascade-research
    pub active_entity: Option<ResolvedEntity>,          // user-confirmed focus
    pub domain_scope: Vec<DomainId>,                    // from entity-context
    pub aspect: Option<String>,                         // from user refinement
    pub verb_surface: Vec<VerbContractSummary>,         // from available-actions
    pub entity_state: Option<EntityContext>,             // from entity-context
    pub data_snapshots: HashMap<String, DataSnapshot>,  // from inspect-data, keyed by aspect

    // Conversation state
    pub utterance_history: Vec<Arc<str>>,                // all user inputs, raw, ordered
    pub research_cache: HashMap<String, serde_json::Value>, // SemOS query results, keyed by query

    // Signals
    pub likely_intents: Vec<IntentHint>,                // from cascade Layer 4
}
```

### CompositionRequest

```rust
/// Input to the single LLM composition step.
/// raw_utterance is NEVER compressed, NEVER summarised.
pub struct CompositionRequest {
    pub raw_utterance: Arc<str>,
    pub context: CompositionContext,
}

pub struct CompositionContext {
    pub entity: ResolvedEntity,                     // confirmed entity
    pub domain_scope: Vec<DomainId>,                // active domains
    pub aspect: Option<String>,                     // focused aspect if any
    pub entity_state: EntityContext,                 // current state from SemOS
    pub verb_surface: Vec<VerbContractSummary>,     // with full param schemas
    pub session_history: Vec<Arc<str>>,             // prior utterances
    pub intent_hints: Vec<IntentHint>,              // from cascade Layer 4
}
```

### ComposedRunbook

```rust
/// Output of composition. Validated before presentation to user.
pub struct ComposedRunbook {
    pub steps: Vec<DslStatement>,
    pub explanation: String,           // human-readable plan
    pub requires_confirmation: bool,   // true if any step is a write
}

pub struct DslStatement {
    pub verb_id: VerbId,               // must exist in verb_surface
    pub args: BTreeMap<String, ArgValue>,
    pub polarity: Polarity,
}
```

---

## LLM COMPOSITION PROMPT STRUCTURE

The single LLM call for multi-verb composition uses this prompt structure:

### System Prompt

```
You are a runbook composer for a custody banking compliance system.
You compose sequences of DSL statements from a FIXED set of available verbs.

RULES:
1. You may ONLY use verbs from the provided verb surface. No other verbs exist.
2. Every argument must match the verb's parameter schema (type, required, validation).
3. Respect verb preconditions — if verb B requires output from verb A, A must come first.
4. For each step, provide: verb_id, args (as key-value pairs), and a brief explanation.
5. Output ONLY a JSON array of steps. No prose, no markdown, no commentary.

AVAILABLE VERBS:
{verb_surface formatted as JSON array of VerbContractSummary}

ENTITY CONTEXT:
{entity_state formatted as JSON — current state, activity data, relationships}

DOMAIN SCOPE: {domain_scope}
FOCUSED ASPECT: {aspect or "none"}
INTENT HINTS: {likely_intents}
```

### User Message

```
{raw_utterance}
```

### Expected Output Format

```json
[
  {
    "verb_id": "onboarding.verify-beneficial-owner",
    "args": {"entity_id": "...", "beneficial_owner_id": "..."},
    "explanation": "Verify Klaus Weber identity and AML screening"
  },
  {
    "verb_id": "onboarding.generate-ubo-report",
    "args": {"entity_id": "..."},
    "explanation": "Generate UBO report for BNP Paribas SA"
  }
]
```

### Parsing

```rust
// Parse LLM output. Strip markdown fences if present. Parse as JSON array.
fn parse_composition_output(raw: &str) -> Result<Vec<RawStep>, CompositionError> {
    let clean = raw
        .trim()
        .strip_prefix("```json").unwrap_or(raw)
        .strip_suffix("```").unwrap_or(raw)
        .trim();
    serde_json::from_str::<Vec<RawStep>>(clean)
        .map_err(|e| CompositionError::ParseFailure(e.to_string()))
}
```

---

## CODEBASE ORIENTATION

### Where to Find Things (investigate in Phase 0.3)

These are likely locations based on the project structure. Phase 0.3 confirms with grep.

- **OutcomeIntent struct**: grep for `OutcomeIntent` — this is the primary rip target
- **Sage preclassification**: the code that produces OutcomeIntent from a raw utterance (plane/polarity/domain classification)
- **Coder verb resolution**: the code that takes OutcomeIntent and selects a verb from the registry
- **Coder argument assembly**: the code that takes a verb + OutcomeIntent and produces args
- **Serve/delegate routing**: any routing logic that reads OutcomeIntent fields to decide behavior
- **Verb registry**: YAML files defining the 653 verbs. In-memory registry loaded at startup
- **Verb contracts**: the VerbContractBody struct and related types
- **Session input path**: the entry point where user utterances arrive — this is where the new pipeline hooks in
- **Test corpus**: the 134-utterance harness. Look for test files referencing utterance accuracy
- **Governed query layer**: the query path that enforces ABAC and governance
- **Entity tables**: PostgreSQL schema — `entities` table with 113 incoming FKs
- **Snapshot layer**: immutable snapshot infrastructure

### Grep Commands for Phase 0.3

```bash
# Primary rip targets
grep -rn "OutcomeIntent" --include="*.rs" .
grep -rn "outcome_intent" --include="*.rs" .

# Sage classification
grep -rn "preclassif\|PreClassif\|sage_classify\|SageClassif" --include="*.rs" .
grep -rn "observation_plane\|ObservationPlane" --include="*.rs" .
grep -rn "intent_polarity\|IntentPolarity" --include="*.rs" .

# Coder resolution
grep -rn "resolve_verb\|verb_resolution\|CoderResolv" --include="*.rs" .
grep -rn "assemble_args\|argument_assembly" --include="*.rs" .

# Session entry point
grep -rn "session_input\|SessionInput\|user_utterance\|handle_utterance" --include="*.rs" .

# Verb registry
find . -name "*.yaml" -path "*/verb*" -o -name "*.yml" -path "*/verb*"
grep -rn "VerbContractBody\|VerbRegistry\|verb_registry" --include="*.rs" .

# Test corpus
find . -name "*utterance*" -o -name "*corpus*" -o -name "*harness*" | head -20
grep -rn "134\|test_corpus\|golden_corpus" --include="*.rs" .

# Entity/query infrastructure
grep -rn "governed_query\|GovernedQuery" --include="*.rs" .
grep -rn "pg_trgm\|trigram" --include="*.rs" --include="*.sql" .
```

---

## PHASE 0 — Audit and Rip Preparation
## Progress: 0% → 10%

### 0.1 SemOS Query Surface Audit

Confirm the existing infrastructure can back the 8 discovery verbs.

- [ ] **Entity resolution backing**: Verify `entities` table supports fuzzy search. Check for `pg_trgm` extension. Check alias table. Confirm GLEIF integration resolves by LEI, name, alias. If `pg_trgm` is not installed, add it (`CREATE EXTENSION IF NOT EXISTS pg_trgm`).
- [ ] **Activity state backing**: Identify tables/views holding engagement state per entity. Map which tables answer: "is onboarding in progress?", "is KYC active?", "is there a linked deal?". Document the query path for each activity type in `ActivityState`.
- [ ] **Relationship backing**: Confirm relationship/ownership tables support traversal with depth control. Verify AffinityGraph is available for verb↔entity lookups. Verify UBO chain data is queryable.
- [ ] **Verb registry backing**: Confirm in-memory registry supports filtering by `subject_kinds` + domain + polarity. Confirm verb contracts are retrievable with full parameter schemas.
- [ ] **Governed query layer**: Confirm read-only queries route through ABAC. Confirm snapshot layer can serve `inspect-data` responses.
- [ ] **Output**: Gap list. Document each gap with a fix plan (schema change, new index, new query, extension install).

**E-invariant**: Every one of the 8 discovery verbs must have a confirmed backing implementation path. If any verb has no viable backing, STOP and document the gap before proceeding.

→ **IMMEDIATELY proceed to 0.2.**

### 0.2 Verb Surface Size Validation

- [ ] For each of the 37 domains × relevant entity types, count verbs in scope (filtered by `subject_kinds` match).
- [ ] Report: median, p90, max verb surface sizes per domain+entity_type.
- [ ] Flag any combination with >30 verbs in scope.
- [ ] Audit `phase_tags` population: what % of 653 verbs have non-empty phase_tags? Are tags consistent enough to group by aspect?
- [ ] If phase_tags are sparse (>30% empty), identify which domains are affected and plan backfill strategy (mechanical derivation from verb name prefixes: "list-" → status, "create-" → mutation, "verify-" → compliance).

**Output**: `verb_surface_report.md` with surface sizes and phase_tag coverage.

→ **IMMEDIATELY proceed to 0.3.**

### 0.3 Identify Rip Targets

Map the code that will be deleted. Use the grep commands above.

- [ ] Locate `OutcomeIntent` struct and ALL references. List every file, line number.
- [ ] Locate Sage preclassification stage. List entry points and call sites.
- [ ] Locate Coder verb-resolution logic. List entry points and call sites.
- [ ] Locate Coder argument-assembly logic. List entry points and call sites.
- [ ] Locate serve/delegate routing depending on OutcomeIntent.
- [ ] Locate Sage → Coder handoff interface.
- [ ] **Do NOT delete anything yet.**
- [ ] Produce `rip_manifest.md`: every file and struct to be removed or replaced, with line numbers and dependency notes (what breaks when this is removed).

**E-invariant**: `grep -r "OutcomeIntent"` results must EXACTLY match `rip_manifest.md`. Zero orphaned references.

→ **IMMEDIATELY proceed to Phase 1. Phase 0 complete. Progress: 10%.**

---

## PHASE 1 — Discovery Domain: Verb Implementations
## Progress: 10% → 40%

### 1.1 Discovery Domain Scaffolding

- [ ] Create `discovery` domain YAML in the verb registry directory.
- [ ] Define domain metadata: description="Sage research surface for SemOS context framing", polarity_constraint=read_only.
- [ ] Register all 8 verb stubs with full contracts (parameter schemas and return types as specified in the DISCOVERY DOMAIN section above).
- [ ] `cargo build` succeeds. Registry loads with 661 verbs.
- [ ] All 8 discovery verbs pass registry validation (contract completeness, parameter type checks).

**E-invariant**: Registry loads 661 verbs. All 8 discovery verbs visible with complete contracts. `cargo test` passes.

→ **IMMEDIATELY proceed to 1.2.**

### 1.2 Implement search-entities

- [ ] Fuzzy entity search across all entity types.
- [ ] Query: `entities` table + alias table + GLEIF data.
- [ ] Matching: `pg_trgm` trigram similarity on name/alias, prefix match, exact match on LEI/registration.
- [ ] Return `SearchResultSet` per schema above.
- [ ] `include_inactive` filter: default excludes entities with no active engagements.
- [ ] Results ranked by match_score descending, active-engagement entities boosted.
- [ ] Tests:
  - [ ] "BNP" returns hits across multiple entity types.
  - [ ] LEI search returns exact match with score=1.0.
  - [ ] Empty query returns error, not all entities.
  - [ ] `include_inactive=false` excludes stale entities.
- [ ] Performance: <200ms on production-scale data.

**E-invariant**: Returns typed, scored results across entity types. At least 3 entity types in test results for common multi-type names.

→ **IMMEDIATELY proceed to 1.3.**

### 1.3 Implement entity-context

- [ ] Activity state map for resolved entity across all domains.
- [ ] Query engagement/activity tables for: onboarding, deal, kyc_review, fund_setup, client_group_mgmt, compliance_review, reporting.
- [ ] Per activity: phase, status enum, completion_pct, blockers, last_activity, last_actor, linked_entities.
- [ ] Compute `ActivitySignals` server-side (derived, not stored).
- [ ] Tests:
  - [ ] Entity with active onboarding + deal returns both with correct phases.
  - [ ] Entity with no activities returns empty array, stale=true.
  - [ ] `include_completed=true` includes archived activities.
  - [ ] Signals are deterministic: same state → same signals on every call.

**E-invariant**: Signals are deterministic. Same entity state → same ActivitySignals, always.

→ **IMMEDIATELY proceed to 1.4.**

### 1.4 Implement entity-relationships

- [ ] Relationship graph traversal with depth control.
- [ ] All 8 relationship types supported.
- [ ] Traversal terminates at max_depth. No infinite cycles.
- [ ] Each relationship carries direction, target entity, depth, typed metadata.
- [ ] Summary computed: totals, chain depths, counts.
- [ ] Tests:
  - [ ] UBO chain returns ownership relationships with percentages at correct depths.
  - [ ] Fund structure returns SICAV → sub-fund hierarchy.
  - [ ] max_depth=1 returns only direct relationships.
  - [ ] Cyclic ownership structures terminate correctly.

**E-invariant**: Traversal terminates at max_depth. No cycles. Metadata correctly typed per relationship type.

→ **IMMEDIATELY proceed to 1.5.**

### 1.5 Implement cascade-research

- [ ] Composite: orchestrate Layers 1-3 per cascade logic above.
- [ ] Layer 1: search-entities with query.
- [ ] Layer 2: entity-context for top_n hits (parallelize).
- [ ] Layer 3: entity-relationships for active hits if include_relationships=true (parallelize).
- [ ] Layer 4: compute likely_intents from ActivitySignals using the deterministic rule table (see DISCOVERY DOMAIN section above).
- [ ] Return CascadeResult with full entity array.
- [ ] Tests:
  - [ ] "BNP" cascade returns multiple types with activity state, relationships, intent hints.
  - [ ] Intent hints match the rule table deterministically.
  - [ ] top_n=1 cascades only the top hit.
  - [ ] include_relationships=false skips Layer 3.
- [ ] Performance: <500ms for top_n=5.

**E-invariant**: CascadeResult complete. Every entity has Layer 1. Top-N have Layer 2. Active engagement entities have Layer 3. likely_intents deterministic.

→ **IMMEDIATELY proceed to 1.6.**

### 1.6 Implement available-actions, verb-detail, inspect-data, search-data

- [ ] **available-actions**: In-memory registry lookup. Filter by domain + subject_kinds + polarity. Group by phase_tag. ABAC filter applied. Returns ActionSurface per schema.
- [ ] **verb-detail**: Full contract lookup by verb_id. Returns VerbContract per schema.
- [ ] **inspect-data**: Governed query layer. Domain + entity_id + aspect → DataSnapshot. Immutable, timestamped. ABAC enforced.
- [ ] **search-data**: Fuzzy search within domain data. `pg_trgm` on text fields. Returns DataSearchResults per schema.
- [ ] Tests for each verb individually.
- [ ] Verify ABAC enforcement: user without access to a domain gets filtered results.

**E-invariant**: available-actions returns ABAC-filtered verbs. inspect-data returns immutable timestamps. All four are read-only — verify no write operations in query paths.

→ **IMMEDIATELY proceed to Phase 2. Phase 1 complete. Progress: 40%.**

---

## PHASE 2 — Sage Session Manager
## Progress: 40% → 60%

### 2.1 SageSession Implementation

- [ ] Implement `SageSession` struct per CORE STRUCTS section above.
- [ ] All fields have provenance tracking: each records which SemOS query or user exchange set it.
- [ ] Implement `SageSession::context_summary()` → structured summary for composition.
- [ ] Implement `SageSession::is_ready_for_composition()` → true when active_entity + domain_scope + verb_surface are populated.

**E-invariant**: Every field in SageSession traces to a SemOS query result or user input. No field populated by LLM inference.

→ **IMMEDIATELY proceed to 2.2.**

### 2.2 Session Lifecycle

- [ ] **Open**: New session. First user input triggers cascade-research. Results populate session.
- [ ] **Accumulate**: Subsequent inputs trigger focused queries (entity-context, inspect-data, available-actions). Results cached.
- [ ] **Focus**: User confirms entity + aspect → active_entity and domain_scope lock. verb_surface populated via available-actions.
- [ ] **Shift**: User names different entity or domain → re-query SemOS, update session. Previous context kept in history.
- [ ] **Persist**: Session state survives across interactions. Returning user: restore from cached session.
- [ ] **Refresh**: Before write composition, re-query entity-context for current state. Reads serve from cache.
- [ ] Tests:
  - [ ] Session opens → cascade fires → context populated.
  - [ ] User confirms entity → focus locks correctly.
  - [ ] User shifts context → session updates, history preserved.
  - [ ] Write composition triggers state refresh.

**E-invariant**: Session state always consistent with last SemOS queries. No stale-data writes.

→ **IMMEDIATELY proceed to 2.3.**

### 2.3 Sage Response Generation

- [ ] All Sage responses generated from SemOS query results, not LLM world knowledge.
- [ ] **Cold start**: Format cascade-research results → entity list with activity summaries and intent hints.
- [ ] **Refinement**: Format focused query results → current-state displays.
- [ ] **Available actions**: Format available-actions grouped by aspect. Use action-encoded verb names.
- [ ] **Disambiguation**: When multiple entities match, present list with type + activity state.
- [ ] **Discoverability**: "What can I do here?" → available-actions for current context.
- [ ] Tests:
  - [ ] Every factual claim in a Sage response maps to a field in research_cache.
  - [ ] No hallucinated entity state.

**E-invariant**: No Sage response contains claims about entity state that don't trace to a SemOS query result in the session.

→ **IMMEDIATELY proceed to Phase 3. Phase 2 complete. Progress: 60%.**

---

## PHASE 3 — Composition Engine + The Rip
## Progress: 60% → 85%

### 3.1 CompositionRequest and Context Assembly

- [ ] Implement `CompositionRequest` and `CompositionContext` per CORE STRUCTS section.
- [ ] `CompositionContext` assembled from `SageSession` fields.
- [ ] `verb_surface` includes full contracts with parameter schemas.

→ **IMMEDIATELY proceed to 3.2.**

### 3.2 Single-Verb Fast Path

- [ ] For simple outcomes: keyword/embedding match of raw_utterance against verb_surface names + descriptions.
- [ ] If single candidate matches above threshold → skip LLM. Assemble args from session context.
- [ ] Fast path reads execute immediately (no confirmation). Fast path writes confirm.
- [ ] Tests:
  - [ ] "list the UBO chain" against a surface containing list-ubo-chain → fast path hit.
  - [ ] Ambiguous input → falls through to LLM composition.
  - [ ] Multi-verb input → falls through to LLM composition.

**E-invariant**: Fast path fires only when exactly one verb matches above threshold. Ambiguous/multi-verb always falls through.

→ **IMMEDIATELY proceed to 3.3.**

### 3.3 Multi-Verb LLM Composition

- [ ] Single LLM call per the LLM COMPOSITION PROMPT STRUCTURE section above.
- [ ] System prompt includes: verb_surface as JSON, entity_state as JSON, domain_scope, aspect, intent_hints.
- [ ] User message is raw_utterance (unchanged, uncompressed).
- [ ] Parse output as JSON array of steps (verb_id + args + explanation).
- [ ] Handle markdown fences in output (strip ```json ... ```).
- [ ] If parse fails: retry once with clarification prompt ("Please output only a JSON array of steps."). If still fails: ask user to refine.
- [ ] **One LLM call. Maximum two on parse failure. Never more.**

**E-invariant**: Composition uses ≤2 LLM calls. Output contains ONLY verbs from verb_surface. No verb outside the surface appears.

→ **IMMEDIATELY proceed to 3.4.**

### 3.4 Runbook Validation

- [ ] Validate every step before presenting to user:
  - [ ] Verb exists in verb_surface (not just in registry — in the SESSION's surface)
  - [ ] All required args present
  - [ ] Arg types match contract parameter schemas
  - [ ] Preconditions satisfied by step ordering
  - [ ] Entity references resolve to known entities in session
- [ ] Validation failure → show user what's wrong, ask for refinement. Do NOT silently fix.
- [ ] Confirmation UX: reads execute without confirmation. Any write → show full runbook, require confirmation.

**E-invariant**: No runbook reaches REPL without passing validation. Every verb, arg, and precondition checked.

→ **IMMEDIATELY proceed to 3.5.**

### 3.5 Delete the Old Pipeline (THE RIP)

Using `rip_manifest.md` from Phase 0.3:

- [ ] Delete `OutcomeIntent` struct and all associated types.
- [ ] Delete Sage preclassification stage.
- [ ] Delete Coder verb-resolution logic.
- [ ] Delete Coder argument-assembly logic.
- [ ] Delete/rewire serve/delegate routing that depended on OutcomeIntent.
- [ ] Delete Sage → Coder handoff interface.
- [ ] Rewire session input: user utterance → SageSession → discovery verbs → composition → runbook → REPL.
- [ ] Verify:
  - [ ] `grep -r "OutcomeIntent" --include="*.rs" .` returns **zero** results.
  - [ ] `cargo build` succeeds with zero errors.
  - [ ] `cargo clippy` passes with zero warnings.
  - [ ] All existing non-pipeline tests still pass (execution layer, governance, ABAC, snapshot).

**E-invariant**: OutcomeIntent no longer exists. Build and lint clean. No regressions in execution layer.

→ **IMMEDIATELY proceed to Phase 4. Phase 3 complete. Progress: 85%.**

---

## PHASE 4 — End-to-End Validation
## Progress: 85% → 100%

### 4.1 Harness: 134-Utterance Corpus

- [ ] Update harness to run through new pipeline: SageSession → cascade → composition → validation.
- [ ] Per utterance, measure:
  - [ ] Correct verb(s) in composed runbook (exact match vs expected)
  - [ ] Correct args bound
  - [ ] Fast path vs LLM composition
  - [ ] Number of SemOS queries fired
  - [ ] Latency: total wall time utterance → validated runbook
- [ ] Report: end-to-end accuracy, fast-path %, median queries, median latency.

**KILL METRIC: If accuracy < 43.28% (legacy pipeline), STOP. Do not continue. Review architecture.**

**E-invariant**: Accuracy > 43.28%. Target > 65%.

→ **IMMEDIATELY proceed to 4.2.**

### 4.2 Power User Path

- [ ] Identify corpus utterances with sufficient signal for single-shot resolution.
- [ ] Measure: % resolving in ≤2 exchanges.
- [ ] Target: >40% single-shot.

→ **IMMEDIATELY proceed to 4.3.**

### 4.3 Performance

- [ ] Benchmark cascade-research at top_n=5 on production-scale data.
- [ ] Target: <500ms.
- [ ] If slow: profile. Parallelize Layers 2+3. Check for N+1 queries.

→ **IMMEDIATELY proceed to 4.4.**

### 4.4 Regression Suite

- [ ] All read-only operations from old pipeline still work.
- [ ] Write confirmation UX intact.
- [ ] ABAC enforced on discovery queries.
- [ ] Governance gating applied to composed runbooks.
- [ ] Audit logging covers discovery queries.

**E-invariant**: Zero regressions in execution-layer behavior.

→ **Phase 4 complete. Progress: 100%.**

---

## SUCCESS CRITERIA

| Metric | Current | Target | Stretch | Kill |
|--------|---------|--------|---------|------|
| End-to-end accuracy (134 corpus) | 8.21% | >65% | >80% | <43.28% |
| Single-shot resolution (power user) | N/A | >40% | >60% | — |
| Median exchanges to composition | N/A | ≤3 | ≤2 | — |
| Cascade latency (top_n=5) | N/A | <500ms | <200ms | >2000ms |
| LLM calls per outcome | Multiple | 1 | 0 (fast path) | — |
| Registry size after | 653 | 661 | — | — |
| OutcomeIntent references | Many | 0 | 0 | >0 |

---

## PHASE SUMMARY

| Phase | What | Progress | Key Deliverable |
|-------|------|----------|-----------------|
| 0 | Audit + rip prep | 0→10% | Gap list + verb surface report + rip_manifest.md |
| 1 | 8 discovery verbs | 10→40% | Discovery domain with all verbs passing tests |
| 2 | SageSession manager | 40→60% | Session lifecycle with SemOS-grounded context |
| 3 | Composition + THE RIP | 60→85% | LLM composer + runbook validation + OutcomeIntent deleted |
| 4 | Validation | 85→100% | Harness passing, performance validated, zero regressions |
