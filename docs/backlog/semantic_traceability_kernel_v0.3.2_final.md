# Vision & Scope: The Semantic Traceability Kernel v0.3.2
**Subtitle:** Full-Path Traceability from Natural Language Utterance to REPL DSL Execution

**Status:** Working Draft — Architecture Paper  
**Repo:** `ob-poc-main`  
**Author:** Adam  
**Date:** March 2026

---

## 1. Vision Statement

Every utterance that enters the system produces a **Trace Record** — a structured, persisted artifact that captures the complete transformation path from raw English words to a deterministic DSL command executed against the REPL. The trace is not a log. It is a first-class diagnostic object that answers three questions at any point in time:

1. **What did the user mean?** (Linguistic decomposition)
2. **What was the system allowed to do?** (SemOS constellation-aware state-gating)
3. **What did the system actually do?** (REPL execution outcome)

When these three answers align, the system works. When they diverge, the trace tells you exactly where and why.

**v0.3.1 addition:** The system also answers a fourth question: **What was the user most likely trying to do, given the operational situation they were in?** The constellation's composite state is not just a legality guard — it is a pattern-matching signal that sharply narrows plausible intent before final DSL resolution, but only after Phase 1 linguistic decomposition and Phase 2 legality recovery have established the structural and legal foundations. The shape of the constellation is the picture on the jigsaw box.

---

## 2. The Pipeline: Six Phases, One Trace

The pipeline has six phases, each producing a trace segment that chains into the next. Each phase declares formal invariants: required inputs, output guarantees, and boundary contracts that downstream phases must respect.

```
Utterance
  │
  ▼
┌──────────────────────────────────────────────────────┐
│ Phase 0: Plane Classification                        │
│ Component: observation_plane + intent_polarity        │
│ Output: { plane, polarity, domain_hints[] }           │
│ Invariant: ALWAYS produces output; never gates.       │
└──────────────────────┬───────────────────────────────┘
                       │
                       ▼
┌──────────────────────────────────────────────────────┐
│ Phase 1: Linguistic Decomposition                     │
│ Component: NLCI utterance parser                      │
│ Output: { verb_phrases[], noun_phrases[],             │
│           quantifiers[], referential_bindings[] }     │
│ Invariant: Emits ONLY linguistic structures.          │
│            Never emits domain verbs or entity IDs.    │
└──────────────────────┬───────────────────────────────┘
                       │
                       ▼
┌──────────────────────────────────────────────────────┐
│ Phase 2: Constellation Recovery & State Legality      │
│ Component: NounIndex + Constellation Engine +         │
│            StateGraph + Verb Taxonomy                 │
│ Output: { constellation_snapshot, legal_verb_set[],   │
│           situation_signature, verb_taxonomy_tags[] }  │
│ Invariant: FIRST and ONLY authoritative source of     │
│            legal verb scope for concrete instances.    │
│            Downstream phases may only prune, never add.│
│            Classifies every legal verb as entity-verb  │
│            or state-verb with cross-entity couplings.  │
└──────────────────────┬───────────────────────────────┘
                       │
                       ▼
┌──────────────────────────────────────────────────────┐
│ Phase 3: ECIR Candidate Narrowing                     │
│ Component: Entity-Centric Intent Resolution +         │
│            Constellation Pattern Matcher               │
│ Output: { candidates[1..15], scores[],                │
│           pattern_match_contribution }                 │
│ Invariant: May ONLY prune from Phase 2's legal set.   │
│            May NEVER introduce verbs absent from       │
│            Phase 2 output.                             │
│            Constellation pattern match is a filter     │
│            dimension alongside plane + action category.│
└──────────────────────┬───────────────────────────────┘
                       │
                       ▼
┌──────────────────────────────────────────────────────┐
│ Phase 4: DSL Resolution                               │
│ Component: verb_concepts + embedding similarity       │
│ Output: { resolved_verb, confidence, dsl_command }    │
│ Invariant: May rank and select ONLY from Phase 3 set. │
│            Escape-hatch fallback is formally traced.   │
└──────────────────────┬───────────────────────────────┘
                       │
                       ▼
┌──────────────────────────────────────────────────────┐
│ Phase 5: REPL Execution                               │
│ Component: DSL REPL engine                            │
│ Output: { execution_result, side_effects[] }          │
│ Invariant: Executes ONLY the DSL command from Phase 4.│
└──────────────────────┘
```

**Phase boundary law:** Each phase consumes the output of its predecessor and may only narrow, never widen. Phase 2 is the authoritative ceiling. Phase 3 prunes from Phase 2. Phase 4 selects from Phase 3. If Phase 4 ever needs to go outside the ECIR candidate set, it must invoke a formally traced **Fallback Escape Hatch** that records why the narrowed set was insufficient. This ensures hardening cannot silently erode.

---

## 3. Phase Specifications

### Phase 0 — Plane Classification

**What it does:** Determines the fundamental nature of the utterance before any verb or entity parsing. Three dimensions, all cheap and deterministic.

| Dimension | Values | Example |
|-----------|--------|---------|
| **Plane** | Observation, Mutation, Query, Meta | "Show me the CBU" → Observation |
| **Polarity** | Constructive, Destructive, Neutral | "Archive the account" → Destructive |
| **Domain Hints** | 0..N domain tags from lexical signals | "KYC screening" → `[kyc, screening]` |

**Why it's Phase 0:** Plane classification alone hit 96.0% accuracy. A correct plane immediately eliminates 60–70% of the verb surface. An Observation utterance never evaluates Mutation verbs. This prunes before anything expensive happens.

**Repo component:** `observation_plane` classifier + `intent_polarity` classifier (Sage Skeleton, Phase 1/2 Codex execution).

**Invariants:**
- ALWAYS produces output. Low confidence is carried forward as a signal, not a gate.
- Output is best-effort and advisory. No downstream phase treats plane classification as authoritative for legality — only Phase 2 (state-gating) is authoritative.

**Trace output — `PlaneTrace`:**

```rust
struct PlaneTrace {
    plane: Plane,                    // Observation | Mutation | Query | Meta
    plane_confidence: f32,           // 0.0–1.0
    polarity: Polarity,              // Constructive | Destructive | Neutral
    polarity_confidence: f32,
    domain_hints: Vec<DomainHint>,   // lexically extracted domain tags
    lexical_signals: Vec<LexicalSignal>, // which words triggered which classifications
}
```

**Halt condition:** None.

---

### Phase 1 — Linguistic Decomposition

**What it does:** Decomposes the utterance into structured linguistic components — verb phrases (transformations), noun phrases (entity references), quantifiers (parameters, filters, constraints), and referential bindings (pronouns, implicit context references).

**Critical invariant:** Phase 1 operates on linguistic structure only. It NEVER emits domain verbs, entity IDs, or SemOS concepts. "Freeze the account" produces `verb_phrase: "freeze"`, `noun_phrase: "the account"`. It does NOT produce `cbu.suspend` or `entity_id: cbu-7742`. That mapping belongs to Phase 2 and Phase 4 respectively.

**Referential binding detection (new in v0.3):** Phase 1 now explicitly detects and classifies referential expressions:

| Pattern | Classification | Example |
|---------|---------------|---------|
| Explicit named entity | `Direct` | "the Acme Corp custody account" |
| Pronoun reference | `Pronominal` | "freeze it" |
| Implicit contextual | `Implicit` | "onboard the usual SICAV" |
| Filter/scope expression | `Filtered` | "all CBUs under this deal" |
| Deictic (conversation-relative) | `Deictic` | "that one we discussed" |

If the referential binding is anything other than `Direct`, the trace records what the parser could and could not resolve. This feeds the new halt reasons `MissingReferentialContext` and `InsufficientScopeBinding` in Phase 2.

**Repo component:** NLCI Semantic IR layer (NLCI Architecture v1.2 paper).

**Trace output — `LinguisticTrace`:**

```rust
struct LinguisticTrace {
    raw_utterance: String,
    verb_phrases: Vec<VerbPhrase>,
    noun_phrases: Vec<NounPhrase>,
    quantifiers: Vec<Quantifier>,
    referential_bindings: Vec<ReferentialBinding>,
    parse_method: ParseMethod,         // Rule-based | LLM-assisted | Hybrid
    token_map: Vec<TokenAttribution>,  // provenance: input tokens → output components
}

struct ReferentialBinding {
    noun_phrase_index: usize,          // which noun_phrase this qualifies
    binding_type: BindingType,         // Direct | Pronominal | Implicit | Filtered | Deictic
    resolved_antecedent: Option<String>, // if session context resolves the pronoun
    scope_expression: Option<String>,   // for filtered bindings: "all X under Y"
    confidence: f32,
}

struct TokenAttribution {
    source_span: (usize, usize),  // character offsets in raw_utterance
    target: AttributionTarget,     // which output field this token contributed to
    confidence: f32,
}
```

**Halt condition:** `HaltReason::NoParsableIntent` — parser cannot extract any verb phrase.

---

### Phase 2 — Constellation Recovery & State Legality

This is the most architecturally significant phase in the pipeline and the one most materially changed from v0.2. The v0.2 paper treated this as "look up one entity, check its FSM state." That was wrong. The actual SemOS model is: **given a UUID or noun-phrase resolution, recover the full constellation, assess composite state across all linked entities, and determine verb legality from the constellation's collective position.**

#### 2a. What is Constellation Recovery?

A constellation is the complete graph of entities linked to a target entity instance. When you resolve "the Acme Corp custody account" to `cbu-7742`, the constellation engine doesn't just look up CBU 7742's own lifecycle state — it recovers the entire entity neighbourhood:

```
                    ┌─────────────┐
                    │  Deal D-401  │ ← parent commercial agreement
                    │  EXECUTING   │
                    └──────┬──────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
        ┌─────┴─────┐ ┌───┴────┐ ┌────┴──────┐
        │ CBU-7742   │ │CBU-7743│ │ CBU-7744  │
        │ ACTIVE     │ │ DRAFT  │ │ VALIDATED │
        └─────┬──────┘ └────────┘ └───────────┘
              │
    ┌─────────┼──────────┬───────────────┐
    │         │          │               │
┌───┴───┐ ┌──┴───┐ ┌────┴────┐  ┌──────┴──────┐
│UBO-91 │ │KYC-34│ │SI-2201  │  │TP-8870      │
│VERIFIED│ │OPEN  │ │APPROVED │  │PENDING_VALID│
└───────┘ └──────┘ └─────────┘  └─────────────┘
```

**This matters because verb legality is not just a function of the target entity's state — it is a function of the constellation's composite state.** Examples:

| Utterance | Target entity state | Constellation constraint | Verdict |
|-----------|-------------------|-------------------------|---------|
| "Terminate CBU-7742" | ACTIVE (legal for terminate) | KYC-34 is OPEN — cannot terminate while KYC case is unresolved | **Blocked by constellation** |
| "Close deal D-401" | EXECUTING (legal for close) | CBU-7743 is still DRAFT — cannot close deal with incomplete CBUs | **Blocked by constellation** |
| "Approve service intent SI-2201" | PENDING (legal for approve) | Parent CBU-7742 is ACTIVE, UBO-91 is VERIFIED — all prerequisites met | **Allowed by constellation** |

Without constellation recovery, Phase 2 would see "ACTIVE + terminate = legal" and pass it through. With constellation recovery, Phase 2 sees the full picture and correctly gates.

#### 2b. UUID Instance Recovery

The constellation engine's primary entry point is a UUID. When the system has a concrete entity instance (from NounIndex resolution, from session context, or from a prior verb's output), it can recover the full constellation in one operation:

```rust
/// Given any entity UUID, recover its full constellation context
fn recover_constellation(entity_id: &EntityId) -> ConstellationSnapshot {
    // 1. Identify entity type from UUID registry
    // 2. Load entity's own state from its lifecycle table
    // 3. Walk cbu_structure_links (with status filter) to find linked entities
    // 4. For each linked entity, load its state
    // 5. Build the constellation graph
    // 6. Compute composite state predicates
    // 7. Compute situation signature (v0.3.1)
    // 8. Classify all legal verbs by taxonomy (v0.3.1)
}
```

This is the bridge between singleton DSL verbs (which operate on individual entities) and the constellation model (which understands the relationships between them). A singleton verb like `cbu.terminate` targets one CBU, but the constellation snapshot tells Phase 2 whether that verb is safe to execute given everything else connected to that CBU.

**The `cbu_structure_links` table** (with the `status` column and partial unique index from the cross-border constellation map remediation) is the physical backbone of this recovery. The hydration direction convention determines which links are walked: parent-to-child for scope queries ("everything under this deal"), child-to-parent for prerequisite queries ("is this CBU's parent deal still active?").

#### 2c. Constellation-Aware Verb Legality

The valid verb set for a given entity instance is the **intersection** of:

1. **Entity-level FSM verbs** — what the entity's own state allows (e.g., CBU in ACTIVE state allows `suspend`, `terminate`, `annotate`, etc.)
2. **Constellation predicates** — cross-entity state conditions that must hold (e.g., "no open KYC cases" for terminate, "all child CBUs validated" for deal close)
3. **Tollgate requirements** — mandatory checkpoints that must be passed before certain state transitions

```rust
struct ConstellationLegalityCheck {
    entity_fsm_verbs: Vec<VerbId>,           // from entity's own state
    constellation_blocks: Vec<ConstellationBlock>, // verbs blocked by linked entity states
    legal_verb_set: Vec<VerbId>,             // entity_fsm_verbs MINUS blocked verbs
}

struct ConstellationBlock {
    blocked_verb: VerbId,
    blocking_entity: EntityId,
    blocking_entity_type: String,
    blocking_state: StateNode,
    predicate: String,               // human-readable: "KYC case must be CLOSED"
    resolution_hint: Option<String>,  // "Close or withdraw KYC-34 first"
}
```

**This is the mechanism that makes the clarification dialogue genuinely useful.** Instead of:

> "You can't terminate this CBU right now."

The system says:

> "You asked to terminate CBU-7742, but KYC case KYC-34 is still OPEN. You'll need to close or withdraw the KYC case first. Alternatively, you can suspend the CBU while the KYC case is resolved."

The constellation block carries enough information to generate that message directly.

#### 2d. The Entity-Verb / State-Verb Duality (new in v0.3.1)

SemOS has two fundamentally different categories of verb that the paper must distinguish. This duality is not a cosmetic taxonomy — it changes how verbs are narrowed, scored, and surfaced to the user.

**Entity verbs** are operations that are valid regardless of the target entity's lifecycle state. They are intrinsic to the entity type, not to its position in the FSM:

| Entity Verb | Entity Type | Available in ALL states |
|-------------|-------------|----------------------|
| `cbu.get-status` | CBU | Yes — you can always look |
| `cbu.annotate` | CBU | Yes — annotations don't change state |
| `cbu.list-history` | CBU | Yes — history is always readable |
| `kyc.get-summary` | KYC Case | Yes |
| `entity.get-metadata` | Any | Yes |

**State verbs** are operations that are meaningful only at specific lifecycle positions. They exist because the entity is in a particular state and cease to be relevant when the entity transitions:

| State Verb | Entity Type | Valid States | Invalid States |
|------------|-------------|--------------|----------------|
| `cbu.validate` | CBU | PENDING_VALIDATION | All others |
| `cbu.suspend` | CBU | ACTIVE | DRAFT, TERMINATED |
| `kyc.close` | KYC Case | OPEN, ESCALATED | CLOSED, WITHDRAWN |
| `tp.validate` | Trading Profile | PENDING_VALIDATION | ACTIVE, DEACTIVATED |
| `tollgate.check` | Tollgate | OPEN | PASSED, FAILED |

**The critical extension: cross-entity state-verb coupling.** Some state verbs on one entity are only operationally meaningful when a *different* entity is in a specific state. This coupling doesn't affect legality (which is Phase 2's FSM + constellation predicate check) — it affects *plausibility*, which is Phase 3's job.

| State Verb | Target Entity | Operationally Meaningful When | Reason |
|------------|--------------|-------------------------------|--------|
| `tp.validate` | Trading Profile | Parent CBU is VALIDATED or ACTIVE | No point validating a TP if CBU hasn't been validated yet |
| `kyc.assign-analyst` | KYC Case | Parent CBU is at least DRAFT | KYC case exists because of the CBU |
| `si.approve` | Service Intent | UBO is VERIFIED for parent CBU | Approving service without verified UBO is premature |
| `tollgate.check` | Tollgate | All prerequisite entities in required states | Tollgate is a checkpoint that reads constellation |

This coupling is not a legality constraint — `tp.validate` might be technically legal regardless of the CBU's state. But it's operationally implausible. A user standing in front of a constellation where the CBU is in DRAFT is not trying to validate a trading profile. The constellation's composite state tells you that.

**Verb taxonomy tags in Phase 2 output:**

Phase 2 now classifies every verb in the legal set:

```rust
struct VerbTaxonomyEntry {
    verb_id: VerbId,
    category: VerbCategory,                  // EntityVerb | StateVerb
    state_coupling: Option<StateCoupling>,    // for state verbs: which state(s) make this meaningful
    cross_entity_couplings: Vec<CrossEntityCoupling>, // operational plausibility from constellation
    plausibility_score: f32,                  // 0.0–1.0, derived from constellation position
}

enum VerbCategory {
    /// Always available for this entity type, regardless of state
    EntityVerb,
    /// Only meaningful in specific lifecycle states
    StateVerb {
        valid_states: Vec<StateNode>,
        current_state_match: bool,  // is the entity currently in a valid state?
    },
}

struct CrossEntityCoupling {
    coupled_entity_type: String,     // e.g., "cbu"
    coupled_entity_id: EntityId,
    required_states: Vec<StateNode>, // states that make this verb operationally plausible
    actual_state: StateNode,         // current state of the coupled entity
    coupling_satisfied: bool,        // does actual_state match any required_state?
}
```

This taxonomy flows from Phase 2 into Phase 3, where it becomes a first-class filter dimension (see §3 Phase 3 and §5 Constellation Pattern Matching).

#### 2e. Situation Signatures (new in v0.3.1)

The constellation's composite state — the specific combination of entity types, lifecycle states, and relationships — forms a **situation signature**. This is a fingerprint of the operational moment.

```rust
struct SituationSignature {
    /// Canonical sorted representation of entity-type:state pairs
    /// e.g., "cbu:ACTIVE|kyc:OPEN|si:APPROVED|tp:PENDING_VALIDATION|ubo:VERIFIED"
    canonical_form: String,

    /// Hash of canonical_form for fast lookup
    signature_hash: u64,

    /// Human-readable operational label (from known signature catalogue)
    situation_label: Option<String>,

    /// Which entity types are present in the constellation
    entity_types_present: Vec<String>,

    /// Which entity types are EXPECTED but missing (from constellation template)
    entity_types_missing: Vec<String>,

    /// Derived operational phase
    operational_phase: Option<OperationalPhase>,
}

enum OperationalPhase {
    EarlyOnboarding,       // CBU in DRAFT/DISCOVERED, most children missing or early
    MidOnboarding,         // CBU advancing, KYC/UBO in progress
    KYCBlocked,            // CBU progressing but KYC case(s) stalled
    PreActivation,         // CBU VALIDATED, dependencies completing
    Active,                // CBU ACTIVE, operational steady-state
    UnderReview,           // Active but KYC or screening re-opened
    WindDown,              // Termination in progress, dependencies being closed
    Terminated,            // CBU TERMINATED, constellation frozen
    Disputed,              // Any entity in dispute/escalated state
}
```

The situation signature is computed during constellation recovery and attached to the Phase 2 output. It flows into Phase 3 as a pattern-matching dimension and into the trace for Loop 3 analytics.

**Why this matters:** The canonical form is a lookup key. If the system has seen this exact signature before (or a signature within edit-distance-1), it has historical trace data showing what users typically do in this situation. That is the jigsaw picture — the shape tells you which pieces are plausible.

#### 2f. Multi-Entity and Filtered Targets

The v0.2 paper assumed one utterance → one entity. Real operational utterances frequently target multiple entities or use filter expressions:

| Pattern | Example | Resolution Model |
|---------|---------|-----------------|
| **Single entity** | "freeze CBU-7742" | Direct UUID resolution |
| **Ambiguous entity** | "freeze the Allianz account" | Multiple matches → disambiguation |
| **Filtered set** | "suspend all CBUs under deal D-401" | Scope query via constellation links |
| **Cross-entity** | "re-open onboarding for the Irish fund but leave KYC untouched" | Two entities, two intents, one utterance |
| **Implicit reference** | "freeze it" | Pronominal → session context lookup |
| **Macro subject** | "onboard a UCITS SICAV" | Template entity → expands to many concrete entities |

Phase 2 must handle all of these. The trace captures which resolution path was taken:

```rust
enum EntityResolutionMode {
    SingleDirect { entity_id: EntityId },
    Disambiguated { candidates: Vec<EntityId>, selected: EntityId, method: String },
    FilteredSet { scope_root: EntityId, filter: String, matched: Vec<EntityId> },
    CrossEntity { targets: Vec<(EntityId, Vec<VerbPhrase>)> },  // entity + its specific verbs
    ReferentialRecovery { antecedent_source: String, resolved: EntityId },
    MacroSubject { template: String, expansion: Vec<EntityId> },
}
```

**For filtered sets**, the constellation snapshot covers all matched entities. Verb legality is checked per-entity, and the trace records which entities passed and which were blocked:

```rust
struct FilteredSetLegality {
    filter_expression: String,
    matched_entities: Vec<EntityId>,
    per_entity_legality: Vec<(EntityId, ConstellationLegalityCheck)>,
    fully_legal: Vec<EntityId>,       // passed all checks
    blocked: Vec<(EntityId, Vec<ConstellationBlock>)>,  // blocked + reasons
}
```

#### 2g. Full Phase 2 Trace

```rust
struct EntityTrace {
    // Resolution
    resolution_mode: EntityResolutionMode,
    resolved_entities: Vec<ResolvedEntity>,
    ambiguous_entities: Vec<AmbiguousEntityResolution>,
    unresolved_nouns: Vec<UnresolvedNoun>,

    // Constellation
    constellation_snapshot: ConstellationSnapshot,
    constellation_recovery_time_ms: u32,

    // Situation (v0.3.1)
    situation_signature: SituationSignature,

    // Verb taxonomy (v0.3.1)
    verb_taxonomy: Vec<VerbTaxonomyEntry>,

    // Legality
    legality: ConstellationLegalityCheck,  // or FilteredSetLegality for sets
    legal_verb_set: Vec<VerbId>,           // the AUTHORITATIVE ceiling for Phase 3+
}

struct ConstellationSnapshot {
    root_entity: EntityId,
    root_state: StateNode,
    linked_entities: Vec<LinkedEntity>,
    structure_links: Vec<StructureLink>,    // from cbu_structure_links
    snapshot_ts: DateTime<Utc>,
    constellation_version: String,          // which constellation template was used
}

struct LinkedEntity {
    entity_id: EntityId,
    entity_type: String,        // "ubo", "kyc-case", "service-intent", etc.
    current_state: StateNode,
    relationship: String,       // "child", "parent", "associated", "prerequisite"
    link_status: String,        // from cbu_structure_links.status
}

struct AmbiguousEntityResolution {
    noun_phrase: String,
    candidates: Vec<EntityCandidate>,
    disambiguation_method: Option<String>,  // how ambiguity was (or wasn't) resolved
}

struct UnresolvedNoun {
    noun_phrase: String,
    binding_type: BindingType,              // from Phase 1 referential binding
    resolution_attempts: Vec<ResolutionAttempt>,
    failure_reason: NounResolutionFailure,
}
```

**Halt conditions (expanded from v0.2):**

| Halt Reason | Trigger | Diagnostic Value |
|------------|---------|-----------------|
| `NoEntityFound` | No noun phrase resolves to any entity | Noun index gap |
| `AmbiguousEntity` | Multiple entities match, no disambiguation signal | UX: "which one?" |
| `StateConflict` | Target entity state has zero overlap with verb phrases | Entity FSM constraint |
| `ConstellationBlock` | Entity state is legal but constellation predicate fails | Cross-entity dependency |
| `MissingReferentialContext` | Pronoun/implicit reference with no session antecedent | Conversational context gap |
| `InsufficientScopeBinding` | Filter expression targets entities but scope root is unresolvable | Scope ambiguity |

---

### Phase 3 — ECIR Candidate Narrowing

**What it does:** Takes the verb phrases (Phase 1), the legal verb set from Phase 2's constellation-aware legality check, the verb taxonomy (entity-verb/state-verb classification), the situation signature, and the plane/polarity (Phase 0), and narrows to 1–15 candidates using deterministic logic. No embeddings, no LLM.

**Boundary contract:**
- **Input:** Phase 2's `legal_verb_set` (authoritative ceiling), Phase 2's `verb_taxonomy` and `situation_signature`, Phase 0's plane/polarity, Phase 1's verb phrases and action semantics.
- **Output:** A strict subset of Phase 2's legal_verb_set. Phase 3 may NEVER introduce verbs absent from Phase 2's output.
- **Output nature:** Ranked, complete within the legal set, deterministic.

**The narrowing funnel (updated for v0.3.1):**

```
Phase 2 legal_verb_set:                ~20–40 (state + constellation gated)
After plane filter:                    ~15    (Observation removes Mutations)
After verb taxonomy filter:            ~10    (state-verb coupling removes implausible verbs)
After constellation pattern filter:    ~5–8   (situation signature eliminates off-situation verbs)
After action-category filter:          1–15   (8-category classifier from ECIR)
```

The funnel now has four filter stages instead of two. The two new stages — verb taxonomy filter and constellation pattern filter — are the v0.3.1 additions. They sit between the existing plane filter and the action-category filter.

**Verb taxonomy filter (new in v0.3.1):**

Uses the `VerbTaxonomyEntry` classification from Phase 2. For each verb in the legal set:

- If the verb is a **state verb** and its `cross_entity_couplings` are all unsatisfied (the coupled entities are in states that make this verb operationally implausible), **demote** it. It remains in the candidate set but with a reduced ranking score.
- If the verb is an **entity verb**, it passes through unaffected — entity verbs are always operationally plausible.

Note: this is a soft ranking filter, not a hard gate. State verbs with unsatisfied couplings are demoted, not eliminated. A user might have a legitimate reason to validate a trading profile even though the parent CBU is still in DRAFT. But the ranking demotion means that if there's a better-coupled verb that also matches the linguistic intent, it will score higher.

**Constellation pattern filter (new in v0.3.1):**

Uses the `SituationSignature` from Phase 2. Two mechanisms:

1. **Static pattern catalogue:** A hand-curated mapping from known situation signatures to high-probability verb sets. Example: signature `cbu:ACTIVE|kyc:OPEN|...` → likely verbs: `kyc.assign-analyst, kyc.escalate, kyc.close, cbu.get-status, cbu.annotate`. Verbs that appear in the catalogue for this signature get a ranking boost; verbs that don't appear get a demotion.

2. **Learned pattern matching (Loop 3 output):** Historical trace data is aggregated by situation signature to produce empirical verb-frequency distributions. If 78% of utterances with this signature resolved to `kyc.close` or `kyc.escalate`, those verbs get an empirical boost. This is the self-optimising aspect — see §10 Loop 3.

```rust
struct ConstellationPatternMatch {
    signature: SituationSignature,
    catalogue_matches: Vec<CatalogueMatch>,
    learned_matches: Vec<LearnedMatch>,
    pattern_boost_applied: Vec<(VerbId, f32)>,   // verb + boost delta
    pattern_demote_applied: Vec<(VerbId, f32)>,  // verb + demotion delta
}

struct CatalogueMatch {
    catalogue_entry_id: String,
    matched_signature: String,
    match_type: SignatureMatchType,   // Exact | EditDistance1 | PartialOverlap
    suggested_verbs: Vec<VerbId>,
}

struct LearnedMatch {
    signature_hash: u64,
    historical_trace_count: usize,    // how many traces with this signature
    verb_frequency: Vec<(VerbId, f32)>, // empirical probability distribution
    confidence: f32,                    // higher with more traces
}

enum SignatureMatchType {
    /// Exact canonical form match
    Exact,
    /// One entity-state pair differs (e.g., same constellation but KYC moved from OPEN to ESCALATED)
    EditDistance1,
    /// Same entity types present but multiple state differences
    PartialOverlap { overlap_ratio: f32 },
}
```

**Trace output — `ECIRTrace` (updated for v0.3.1):**

```rust
struct ECIRTrace {
    legal_set_in: Vec<VerbId>,                // from Phase 2 (authoritative ceiling)
    after_plane_filter: Vec<VerbId>,
    after_taxonomy_filter: Vec<VerbId>,       // v0.3.1: after entity/state-verb filtering
    after_pattern_filter: Vec<VerbId>,        // v0.3.1: after constellation pattern matching
    after_action_filter: Vec<VerbId>,         // final 1–15 candidates
    action_category: ActionCategory,           // which of 8 categories matched
    filter_chain: Vec<FilterStep>,             // ordered record of each narrowing step
    deterministic_resolution: Option<VerbId>,  // if narrowed to exactly 1

    // v0.3.1 additions
    verb_taxonomy_applied: Vec<VerbTaxonomyEntry>,  // taxonomy tags used for filtering
    pattern_match: Option<ConstellationPatternMatch>, // constellation pattern match details
}

struct FilterStep {
    filter_name: &'static str,
    input_count: usize,
    output_count: usize,
    eliminated: Vec<VerbId>,  // what got pruned (capped for storage)
    filter_type: FilterType,  // HardGate | SoftRanking
}
```

#### Phase 3 Contract Refinement: Prune vs Rank vs Demote (new in v0.3.2)

Phase 3 contains both hard gates (which eliminate candidates) and soft filters (which reorder candidates). The paper must distinguish these precisely, because conflating them allows implementation to silently weaken the narrowing guarantee.

**Three operations within Phase 3:**

| Operation | Mechanism | Effect on Candidate Set | Trace Record |
|-----------|-----------|------------------------|-------------|
| **Eliminate** | Hard gate — verb removed from candidate set entirely | Set shrinks | `eliminated_candidates[]` |
| **Demote** | Soft ranking — verb stays in set but ranking score reduced | Set unchanged, order changes | `demoted_candidates[]` with demotion delta |
| **Retain** | Verb passes all filters with no score adjustment | Set unchanged | `retained_candidates[]` |

**Which filters use which operations:**

| Filter | Operation | Rationale |
|--------|-----------|-----------|
| Plane filter | **Eliminate** | An Observation utterance cannot be a Mutation verb. Hard gate, no exceptions. |
| Verb taxonomy filter | **Demote** | State-verb with unsatisfied coupling is implausible but not impossible. User may have a legitimate reason. |
| Constellation pattern filter | **Demote** | Low-frequency verb for this situation is unlikely but not illegal. Ranking adjustment only. |
| Action-category filter | **Eliminate** | If the linguistic action category doesn't match, the verb is structurally wrong. Hard gate. |

**Phase 3 output contract:**

```rust
struct Phase3Output {
    /// Verbs eliminated by hard gates — removed from consideration entirely
    eliminated_candidates: Vec<EliminatedCandidate>,

    /// Verbs retained but with reduced ranking — still in candidate set
    demoted_candidates: Vec<DemotedCandidate>,

    /// Verbs that passed all filters with no score adjustment
    retained_candidates: Vec<VerbId>,

    /// The final bounded set handed to Phase 4
    /// Invariant: phase4_candidate_set ⊆ Phase 2 legal_verb_set
    /// Invariant: phase4_candidate_set = retained_candidates ∪ demoted_candidates
    /// Invariant: eliminated_candidates ∩ phase4_candidate_set = ∅
    phase4_candidate_set: Vec<ScoredCandidate>,
}

struct EliminatedCandidate {
    verb_id: VerbId,
    eliminated_by: &'static str,  // which hard gate
    reason: String,
}

struct DemotedCandidate {
    verb_id: VerbId,
    demoted_by: &'static str,     // which soft filter
    original_score: f32,
    demoted_score: f32,
    demotion_reason: String,
}

struct ScoredCandidate {
    verb_id: VerbId,
    composite_score: f32,         // ranking score after all filters
    was_demoted: bool,
}
```

**Non-negotiable invariant:** `phase4_candidate_set` must be a subset of Phase 2's `legal_verb_set`, except when Phase 4 invokes the formally traced Fallback Escape Hatch. This is verified at the Phase 3 → Phase 4 boundary; a violation is a bug, not a configuration issue.

**Halt condition:** `HaltReason::NoViableVerb` — candidate set narrows to zero. This is the most valuable trace for DSL Discovery: the user wanted something, the entity and constellation were valid, but no verb in the legal set matches the linguistic intent. Concrete DSL gap.

**Fast path:** If ECIR narrows to exactly one candidate, Phase 4 is skipped. Target: 40% of utterances resolve deterministically here. With constellation pattern matching, the target increases to **55%** — the situation signature frequently eliminates enough candidates that only one remains before the action-category filter even fires.

---

### Phase 4 — DSL Resolution

**What it does:** Takes the ECIR candidate set (1–15 verbs) and selects the best match using embedding similarity, verb_concepts.yaml enrichment, and — only when needed — an LLM disambiguation call.

**Boundary contract:**
- **Input:** Phase 3's candidate set (strict subset of Phase 2's legal set).
- **Output:** At most one resolved verb from the Phase 3 set, OR a formally traced fallback.
- **Forbidden:** Phase 4 may NOT select a verb outside the Phase 3 candidate set unless it invokes the **Fallback Escape Hatch**, which is a formally traced exception that records why the narrowed set was insufficient. This prevents hardening erosion.

**Resolution strategy (ordered by cost):**

1. **Exact match** — utterance verb phrase matches a verb's invocation phrase → resolve
2. **verb_concepts.yaml match** — utterance maps to a concept → resolve
3. **Embedding similarity** — BGE distance against candidate set → resolve if top score clears threshold
4. **LLM disambiguation** — present top-3 candidates to LLM with context → resolve

**Fallback Escape Hatch:**

If all four strategies fail against the Phase 3 candidate set, Phase 4 may widen back to the Phase 2 legal set (NOT the full verb surface) and retry strategies 3–4. This is formally traced:

```rust
struct FallbackEscapeHatch {
    reason: String,                        // why Phase 3 set was insufficient
    reason_code: FallbackReasonCode,       // v0.3.2: typed reason for metrics
    source_phase: u8,                      // v0.3.2: which phase's narrowing was insufficient (always 3)
    widened_to: Vec<VerbId>,               // Phase 2 legal set
    resolution_from_widened: Option<VerbId>,
    widened_strategy: ResolutionStrategy,
}

enum FallbackReasonCode {
    ActionCategoryOverPrune,    // ECIR 8-category classifier too aggressive
    TaxonomyOverDemotion,       // state-verb coupling demoted the correct verb below threshold
    PatternMismatch,            // constellation pattern filter demoted the correct verb
    ConceptCoverageGap,         // verb_concepts.yaml missing mapping
    EmbeddingCollision,         // embedding distances too close in narrowed set
    Unknown,
}
```

**Fallback is an anti-pattern, not a feature (v0.3.2).** The escape hatch exists because ECIR's action-category classifier may occasionally over-prune when the user's phrasing doesn't match the expected action semantics. But fallback is explicitly an operational exception, not healthy runtime behaviour. It is governed by an error-budget model:

| Metric | Threshold | Response |
|--------|-----------|----------|
| Fallback rate < 5% of traces | Normal | Monitor, no action |
| Fallback rate 5–10% | Warning | Review ECIR categories and verb_concepts.yaml coverage |
| Fallback rate > 10% | Defect | Mandatory remediation — ECIR is structurally under-discriminating |
| Fallback rate > 20% | Critical | Phase 3 is not providing value — the pipeline is effectively two-phase |

Every fallback invocation is traced and countable. The `fallback_invoked` flag is a hoisted column in the trace schema (§13), indexed for trending. Rising fallback frequency is evidence of ECIR over-pruning, weak concept coverage, or inadequate constellation pattern data — it is a defect signal, not a normal operational path.

#### Legality vs. Confidence: Separate Models

The v0.2 paper mixed legality and confidence into one `ConfidenceVector`. That was wrong. They are fundamentally different:

**Legality** (binary gates — pass or reject, no ranking):
- `state_validity`: Is this verb legal in the entity's current FSM state? (Phase 2)
- `constellation_clear`: Are all constellation predicates satisfied? (Phase 2)

**Confidence** (continuous scores — used for ranking and threshold comparison):
- `embedding_distance`: How close is the utterance to the verb's embedding?
- `plane_confidence`: How confident was Phase 0's plane classification?
- `domain_match_score`: How well did domain hints align?
- `concept_match`: Did verb_concepts.yaml contribute?
- `pattern_match_boost`: How much did constellation pattern matching contribute? (v0.3.1)

**Confirmation triggers** (thresholds that determine UX behaviour):

| Plane | Polarity | Threshold | Behaviour |
|-------|----------|-----------|-----------|
| Observation / Query | Any | 0.65 | Execute immediately |
| Mutation | Constructive | 0.75 | Execute, note in trace |
| Mutation | Neutral | 0.80 | Confirm before execution |
| Mutation | Destructive | 0.92 | Always confirm, show consequences |

**Audit-only signals** (recorded but never influence resolution):
- `parse_method`: Whether Phase 1 used rule-based, LLM-assisted, or hybrid parsing
- `resolution_strategy`: Which strategy resolved the verb (for analytics, not gating)

Legality gates are evaluated in Phase 2 and are never overridable by confidence. High embedding similarity cannot override state illegality. This is non-negotiable.

**Trace output — `DSLTrace`:**

```rust
struct DSLTrace {
    candidates_in: Vec<VerbId>,            // from Phase 3
    resolution_strategy: ResolutionStrategy,
    resolved_verb: Option<ResolvedVerb>,
    alternative_verbs: Vec<ScoredVerb>,    // runners-up with scores
    confidence: ConfidenceScores,          // ranking scores only
    dsl_command: Option<String>,           // the actual DSL string
    requires_confirmation: bool,
    confirmation_reason: Option<String>,   // which threshold triggered
    fallback_escape: Option<FallbackEscapeHatch>,
}

struct ConfidenceScores {
    embedding_distance: f32,
    plane_confidence: f32,        // carried from Phase 0
    domain_match_score: f32,
    concept_match: bool,
    pattern_match_boost: f32,     // v0.3.1: constellation pattern contribution
}

enum ResolutionStrategy {
    ExactMatch,
    ConceptMatch { concept_id: String },
    EmbeddingSimilarity { distance: f32 },
    LLMDisambiguation { model: String, prompt_hash: String },
    FallbackWidened { escape: FallbackEscapeHatch },
    Failed,
}
```

**Halt conditions:**
- `BelowConfidenceThreshold` — no candidate cleared threshold → clarification dialogue with top-3
- `AmbiguousResolution` — two candidates within epsilon → disambiguation dialogue

---

### Phase 5 — REPL Execution

**What it does:** Submits the resolved DSL command to the REPL engine, records the outcome, and closes the trace.

**Invariant:** Executes ONLY the DSL command produced by Phase 4. No reinterpretation, no implicit widening.

**Trace output — `ExecutionTrace`:**

```rust
struct ExecutionTrace {
    dsl_command: String,
    execution_start: DateTime<Utc>,
    execution_end: DateTime<Utc>,
    outcome: ExecutionOutcome,
    side_effects: Vec<SideEffect>,
    post_constellation_snapshot: Option<ConstellationSnapshot>, // state AFTER execution
    repl_session_id: SessionId,
}

enum ExecutionOutcome {
    Success { result: serde_json::Value },
    ValidationError { field: String, reason: String },
    StateTransitionError { from: StateNode, to: StateNode, reason: String },
    ConstellationViolation { block: ConstellationBlock }, // runtime catch
    InternalError { code: String, message: String },
}

struct SideEffect {
    entity_id: EntityId,
    field_or_state: String,
    before: serde_json::Value,
    after: serde_json::Value,
}
```

**Post-execution constellation snapshot:** After a successful state-changing verb, the system takes a second constellation snapshot. Diffing the pre-execution snapshot (Phase 2) against the post-execution snapshot (Phase 5) gives you the full impact map of the verb: which entities changed state, which links were created or severed, which new verbs became legal or illegal as a result.

**Post-hoc correction annotation:** If the user subsequently corrects the action, the trace records the correction and links it to the original. This closes the feedback loop.

---

## 4. The Composite Trace Record

All six phase traces compose into a single `UtteranceTrace`:

```rust
struct UtteranceTrace {
    // ─── Identity ───
    trace_id: Uuid,
    utterance_id: Uuid,
    session_id: SessionId,
    trace_kind: TraceKind,       // Original | ClarificationPrompt |
                                 // ClarificationResponse | ResumedExecution
    parent_trace_id: Option<Uuid>, // for clarification chains
    timestamp: DateTime<Utc>,

    // ─── Version Pins (for replay forensics) ───
    surface_versions: SurfaceVersions,

    // ─── Raw Input ───
    raw_utterance: String,

    // ─── Phase Traces ───
    plane: PlaneTrace,                       // Phase 0 — always present
    linguistic: Option<LinguisticTrace>,      // Phase 1
    entity: Option<EntityTrace>,             // Phase 2
    ecir: Option<ECIRTrace>,                 // Phase 3
    dsl: Option<DSLTrace>,                   // Phase 4
    execution: Option<ExecutionTrace>,        // Phase 5

    // ─── Terminal State ───
    outcome: TraceOutcome,
    halt_reason: Option<HaltReason>,

    // ─── Macro Context ───
    macro_context: Option<MacroContext>,

    // ─── Post-hoc ───
    user_correction: Option<UserCorrection>,
}
```

### 4a. Trace Kinds and Clarification Lineage

Clarification is not a single event — it is a conversational tree. The v0.2 model linked traces via `parent_trace_id` but didn't distinguish trace kinds. In v0.3, every trace has an explicit kind:

```rust
enum TraceKind {
    /// First utterance in a resolution attempt
    Original,
    /// System-generated clarification question
    ClarificationPrompt,
    /// User's response to a clarification
    ClarificationResponse,
    /// Execution that resumes after clarification resolves
    ResumedExecution,
}
```

A clarification conversation produces a tree:

```
Original (trace-001)           "freeze the Allianz account"
  ├─ ClarificationPrompt       "Which account? CBU-7742 (LU SICAV) or CBU-8891 (IE ICAV)?"
  │   └─ ClarificationResponse "the Luxembourg one"
  │       └─ ResumedExecution   → cbu.suspend cbu-7742
  └─ (alternative branch if user had said "the Irish one")
```

Each node in the tree is a full `UtteranceTrace` with its own phase outputs. The `ClarificationResponse` trace runs Phases 0–5 on the response text, using the `parent_trace_id` to carry forward context from the original trace. This makes clarification behaviour fully analyzable: you can query how often clarification leads to successful resolution, which halt reasons trigger the most clarification, and whether the clarification dialogue actually helps.

### 4b. Version Pinning

Replay quality depends on knowing exactly which surfaces were active at original resolution time. Every trace pins its versions:

```rust
struct SurfaceVersions {
    verb_surface_version: String,          // hash of verb registry at trace time
    concept_registry_version: String,      // hash of verb_concepts.yaml
    entity_fsm_version: String,            // hash of entity FSM definitions
    constellation_template_version: String, // which constellation maps were active
    embedding_model_version: String,       // BGE model identifier
    threshold_policy_version: String,      // hash of threshold config
    parser_version: String,                // NLCI parser version
    macro_compiler_version: String,        // macro expansion compiler version
    pattern_catalogue_version: String,     // v0.3.1: hash of situation pattern catalogue
}
```

When replay disagrees with history, the version pins tell you exactly what changed. Without them, replay is diagnostic but not forensically strong.

### 4c. Outcome and Halt Reasons

```rust
enum TraceOutcome {
    ExecutedSuccessfully,
    ExecutedWithCorrection,     // system executed, user then corrected
    HaltedAtPhase { phase: u8, reason: HaltReason },
    ClarificationTriggered,
    NoMatch,                    // complete miss — DSL gap candidate
}

enum HaltReason {
    // Phase 1
    NoParsableIntent,

    // Phase 2 — Entity resolution
    NoEntityFound,
    AmbiguousEntity { candidates: Vec<String> },
    MissingReferentialContext { binding_type: BindingType, context: String },
    InsufficientScopeBinding { filter: String, reason: String },

    // Phase 2 — State legality
    StateConflict {
        entity: String, state: String,
        requested: String, valid: Vec<String>,
    },
    ConstellationBlock {
        target_entity: String,
        blocking_entity: String,
        blocking_state: String,
        predicate: String,
        resolution_hint: Option<String>,
    },

    // Phase 3
    NoViableVerb,

    // Phase 4
    BelowConfidenceThreshold { best_score: f32, threshold: f32 },
    AmbiguousResolution { candidates: Vec<String>, scores: Vec<f32> },

    // User
    UserCancelled,

    // Cross-entity execution (v0.3.2)
    ExclusionViolation {
        excluded_entity: String,
        violated_by_verb: String,
        exclusion_source: String,  // the original user phrase
    },

    // DAG execution (v0.3.2 final)
    DagOrderingConflict {
        user_stated_first: String,        // verb the user wanted first
        user_stated_second: String,       // verb the user wanted second
        dag1_required_first: String,      // verb DAG 1 requires first
        prerequisite_edge: String,        // the DAG 1 edge that mandates the order
        suggested_correction: String,
    },
    ExclusionMakesPlanInfeasible {
        excluded_entity: String,
        required_by_prerequisite: String, // the DAG 1 edge that requires action on excluded entity
        explanation: String,
    },
    MidPlanConstellationBlock {
        completed_nodes: Vec<String>,     // verbs that already executed
        blocked_node: String,             // verb that failed re-check
        blocking_entity: String,
        blocking_state: String,
        predicate: String,
    },
}
```

---

## 5. Constellation Pattern Matching as a Resolution Accelerator (new in v0.3.1)

The previous sections describe what constellation pattern matching *does* within the pipeline. This section explains *why* it is architecturally significant and *how* the taxonomies compound to form the jigsaw-picture effect.

### 5a. The Jigsaw Analogy

Consider a jigsaw puzzle. If you have no picture on the box, every piece could go anywhere — you are doing trial-and-error matching (this is embedding similarity over the full verb surface). If you have the picture on the box, you don't try pieces randomly; you look at the area you're working on, identify the colour and shape pattern, and reach for pieces that fit that region.

The constellation's composite state is the picture on the box. Each entity's lifecycle state is a piece. The constellation topology — which entities exist, how they're linked, what states they're in — tells you which region of the verb surface you're working in. You don't need to try all 1,123 verbs; you need to try the 5–10 that belong in this region.

### 5b. The Compounding Taxonomy Stack

The resolution power comes from the compounding of multiple taxonomies. Each taxonomy is a filter dimension. Individually, each removes some candidates. In combination, they compound multiplicatively:

```
Full verb surface:                           ~1,123 verbs

  × Plane filter (Phase 0):                 → ~350  (Observation removes Mutations)
  × Entity-type filter (Phase 2):           → ~40   (only verbs for this entity)
  × State-legality filter (Phase 2):        → ~25   (only verbs legal in current state)
  × Constellation predicate filter (Phase 2): → ~20   (only verbs unblocked by constellation)
  × Verb taxonomy filter (Phase 3):         → ~12   (state-verb coupling demotion)
  × Constellation pattern filter (Phase 3): → ~5–8  (situation signature match)
  × Action-category filter (Phase 3):       → 1–5   (linguistic action classification)

Final candidate set for Phase 4:             1–5 verbs (from 1,123)
```

That is a 200:1 to 1000:1 reduction ratio before any embedding comparison or LLM call. The reduction is not from one clever filter — it is from the compounding of seven orthogonal taxonomies. Each taxonomy contributes a modest reduction (2x–5x), but they multiply.

### 5c. The Seven Taxonomies

| # | Taxonomy | Source | Phase | Filter Type | Typical Reduction |
|---|----------|--------|-------|-------------|-------------------|
| 1 | **Plane** | Observation / Mutation / Query / Meta | 0 | Hard gate | 2–3x |
| 2 | **Polarity** | Constructive / Destructive / Neutral | 0 | Soft ranking | 1.5x |
| 3 | **Entity type** | Which entity is the target | 2 | Hard gate | 25–30x |
| 4 | **State legality** | Entity FSM valid verbs | 2 | Hard gate | 1.5–2x |
| 5 | **Constellation predicates** | Cross-entity blocking rules | 2 | Hard gate | 1.2–1.5x |
| 6 | **Verb taxonomy** | Entity-verb vs state-verb + cross-entity coupling | 3 | Soft ranking | 1.5–2x |
| 7 | **Situation signature** | Constellation pattern match (static + learned) | 3 | Soft ranking | 2–3x |

Hard gates eliminate outright. Soft rankings demote. The combination of gates + rankings means that by the time Phase 3 hands candidates to Phase 4, the candidate set is not just small — it is heavily pre-ranked, so the top candidate is usually correct and embedding similarity is confirming rather than discovering.

### 5d. Concrete Example: The Compounding Effect

**Constellation state:**
```
CBU-7742: ACTIVE
KYC-34:   OPEN
UBO-91:   VERIFIED
SI-2201:  APPROVED
TP-8870:  PENDING_VALIDATION
```

**Situation signature:** `cbu:ACTIVE|kyc:OPEN|si:APPROVED|tp:PENDING_VALIDATION|ubo:VERIFIED`
**Operational phase:** `KYCBlocked` (CBU is active but KYC stalled)

**User says:** "What's holding things up?"

This is a vague utterance. Without constellation context, the system has no idea what "things" refers to or what "holding up" means in domain terms. With the constellation:

1. **Phase 0:** Plane = Query/Observation, Polarity = Neutral → eliminates all Mutation verbs
2. **Phase 2:** Entity resolved via session context. Legal verb set = all observation verbs for CBU + linked entities (~25 verbs)
3. **Phase 3 — Verb taxonomy:** Entity verbs (get-status, get-summary) pass through. State verbs for entities NOT in a blocked/waiting state are demoted (SI-2201 is APPROVED, nothing to check there). State verbs for entities IN a waiting/problematic state are boosted (KYC-34 is OPEN, TP-8870 is PENDING_VALIDATION).
4. **Phase 3 — Constellation pattern:** Signature matches `KYCBlocked` operational phase. Historical traces for this signature show 64% of queries resolve to `kyc.get-status` or `cbu.get-status-with-blockers`. These get boosted.
5. **Phase 3 — Action category:** "holding up" maps to Diagnostic/Status category.

**Result:** From 1,123 verbs, narrowed to: `cbu.get-status-with-blockers`, `kyc.get-status`, `cbu.get-status`. The top candidate is the one that specifically shows blocking dependencies — exactly what the user meant by "what's holding things up." Resolved without embedding similarity, without LLM, purely from taxonomy compounding.

### 5e. The Self-Reinforcing Quality

Each successful resolution adds to the trace corpus. Each trace carries its situation signature. As the corpus grows, the learned pattern data gets stronger for common signatures and starts covering rare signatures. This means:

- **Common operational situations** (mid-onboarding, KYC-blocked, pre-activation) develop very accurate verb-frequency distributions quickly
- **Rare situations** (cross-border hedge fund with multiple feeders and a disputed UBO) start with static catalogue entries and gradually build empirical data
- **Novel situations** (signature never seen before) fall back to the static catalogue and the existing ECIR filters, then the first few traces bootstrap the learned data

The system gets better at resolving the situations it sees most often, while maintaining baseline capability for situations it hasn't seen. This is Loop 3 (see §10).

---

## 6. The Constellation DAG Hierarchy (new in v0.3.2)

The paper uses "DAG" in three different contexts without naming or distinguishing them. This section makes the hierarchy explicit, because the three DAGs have a derivation relationship that governs how the constellation model connects to execution — and conflating them will cause implementation drift.

### 6a. Three DAGs, One Source of Truth

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                 │
│   DAG 1: Constellation Dependency DAG (Structural)              │
│   ─────────────────────────────────────────────────             │
│   The SemOS constellation template.                             │
│   Defines entity types, dependency edges, prerequisite          │
│   direction, and topological ordering constraints.              │
│   Lives in: SemOS constellation metadata                        │
│   Hydrated by: cbu_structure_links (live instances)             │
│   Scope: ALL constellation-aware behaviour derives from this.   │
│                                                                 │
│              ┌─────────────┐                                    │
│              │    Deal      │                                    │
│              └──────┬──────┘                                    │
│                     │ parent-of                                  │
│              ┌──────┴──────┐                                    │
│              │     CBU      │                                    │
│              └──────┬──────┘                                    │
│           ┌─────┬───┴───┬──────┐                                │
│           │     │       │      │                                 │
│         ┌─┴─┐ ┌─┴─┐  ┌──┴──┐ ┌┴──┐                             │
│         │UBO│ │KYC│  │ SI  │ │TP │                              │
│         └───┘ └───┘  └──┬──┘ └───┘                              │
│                         │                                       │
│                      ┌──┴──┐                                    │
│                      │ TG  │ (tollgate — depends on all above)  │
│                      └─────┘                                    │
│                                                                 │
│   Edges encode:                                                 │
│   • prerequisite_of(CBU, Deal) — CBU cannot exist without Deal  │
│   • prerequisite_of(KYC, CBU) — KYC case is scoped to a CBU    │
│   • prerequisite_of(TG, [KYC,UBO,SI,TP]) — tollgate reads all  │
│   • hydration_direction — parent→child for scope queries,       │
│     child→parent for prerequisite queries                       │
│                                                                 │
└──────────────────────────┬──────────────────────────────────────┘
                           │
              ┌────────────┴────────────┐
              │                         │
              ▼                         ▼
┌──────────────────────────┐  ┌──────────────────────────┐
│                          │  │                          │
│  DAG 2: Macro Expansion  │  │  DAG 3: Cross-Entity     │
│  DAG (Compile-time)      │  │  Plan DAG (Runtime)      │
│  ────────────────────    │  │  ────────────────────    │
│  Produced by: macro      │  │  Produced by: cross-     │
│  expansion compiler      │  │  entity plan compiler    │
│  When: macro resolution  │  │  When: multi-entity      │
│  (M1–M18 and beyond)     │  │  utterance resolution    │
│                          │  │  (§9)                    │
│  Example: M1 "onboard    │  │                          │
│  a UCITS SICAV" →        │  │  Example: "close KYC-34  │
│  cbu.create              │  │  then terminate           │
│    → cbu.set-domicile    │  │  CBU-7742" →             │
│    → ubo.discover        │  │  kyc.close kyc-34        │
│    → kyc.open            │  │    → cbu.terminate 7742  │
│    → si.create           │  │                          │
│    → tp.create           │  │  Edges inferred from     │
│    → tollgate.check      │  │  DAG 1 prerequisites     │
│                          │  │  + explicit user ordering │
│  Edges derived from      │  │  + exclusion predicates   │
│  DAG 1 dependency order  │  │                          │
│                          │  │                          │
└──────────────────────────┘  └──────────────────────────┘
```

### 6b. DAG 1 — The Constellation Dependency DAG

This is the structural backbone. It lives in SemOS constellation template metadata and defines, for each constellation topology (e.g., "LU UCITS SICAV," "IE ICAV," "US 40 Act Fund"):

1. **Which entity types participate** — Deal, CBU, UBO, KYC, SI, TP, Tollgate
2. **Dependency edges between entity types** — directed prerequisite relationships
3. **Cardinality constraints** — one Deal to many CBUs, one CBU to many KYC cases, etc.
4. **Hydration direction** — parent-to-child for scope queries, child-to-parent for prerequisite queries
5. **State propagation rules** — which state transitions on one entity type affect which others

The constellation template for a given jurisdiction (from the 17 constellation maps: LU, IE, UK, US, cross-border) is a static DAG of entity types. When hydrated with live instances via `cbu_structure_links`, it becomes a concrete DAG of entity instances with live states — the `ConstellationSnapshot` from Phase 2.

```rust
/// DAG 1: The constellation template (static, per-jurisdiction)
struct ConstellationTemplate {
    template_id: String,                        // e.g., "lu-ucits-sicav"
    jurisdiction: String,
    entity_type_nodes: Vec<EntityTypeNode>,
    dependency_edges: Vec<DependencyEdge>,
    state_propagation_rules: Vec<StatePropagationRule>,
}

struct EntityTypeNode {
    entity_type: String,                        // "cbu", "kyc-case", etc.
    cardinality: Cardinality,                   // One | ZeroOrOne | OneOrMany | ZeroOrMany
    required: bool,                             // must this entity type be present?
}

struct DependencyEdge {
    from_type: String,                          // prerequisite (parent)
    to_type: String,                            // dependent (child)
    relationship: String,                       // "parent-of", "scoped-to", "gates"
    hydration_direction: HydrationDirection,
    prerequisite_semantics: PrerequisiteKind,
}

enum PrerequisiteKind {
    /// Child cannot be CREATED until parent exists
    ExistencePrerequisite,
    /// Child cannot advance past a state until parent reaches a state
    StatePrerequisite { parent_min_state: StateNode, child_gate_state: StateNode },
    /// Child cannot be TERMINATED until parent is terminated or child is resolved
    TerminationPrerequisite,
    /// Tollgate reads state of all prerequisites before allowing passage
    TollgatePrerequisite,
}

enum HydrationDirection {
    /// Walk parent → child: "find all entities under this Deal"
    ParentToChild,
    /// Walk child → parent: "find the Deal that owns this CBU"
    ChildToParent,
    /// Walk both directions: constellation recovery always builds the full graph
    Bidirectional,
}

struct StatePropagationRule {
    trigger_entity_type: String,
    trigger_transition: (StateNode, StateNode),  // from → to
    affected_entity_type: String,
    effect: PropagationEffect,
}

enum PropagationEffect {
    /// Downstream entity's legal verb set changes
    VerbSetInvalidation,
    /// Downstream entity should be re-checked for constellation blocks
    BlockReEvaluation,
    /// Downstream entity transitions automatically (e.g., cascade termination)
    CascadeTransition { to_state: StateNode },
    /// Advisory only — no automatic action, but trace records the propagation
    AdvisoryNotification,
}
```

**Key principle: the constellation template IS the schema.** It is not a runtime optimisation or an analytics model. It is the authoritative definition of how entities relate within a jurisdiction's fund structure. Everything else — macro expansion ordering, cross-entity plan dependency inference, constellation predicate evaluation, situation signature computation — reads from this one DAG.

**Acyclicity guarantee (non-negotiable):** A constellation template must be a directed acyclic graph by construction. Every downstream consumer — topological sort for macro compilation, prerequisite inference for cross-entity plans, state propagation for post-execution re-evaluation — depends on acyclicity for correctness. A cycle in DAG 1 would make topological sorting impossible, cause infinite prerequisite chains, and break constellation predicate evaluation.

Enforcement:

```rust
/// Validates that a constellation template is acyclic.
/// Called at template publish time — a template with a cycle is rejected
/// before it can be consumed by any pipeline component.
fn validate_template_acyclicity(template: &ConstellationTemplate) -> Result<(), CycleError> {
    // Standard Kahn's algorithm or DFS-based cycle detection
    // on template.dependency_edges
    // Returns Err(CycleError { edge_path }) if a cycle is found
}

struct CycleError {
    /// The edges that form the cycle, in order
    cycle_path: Vec<DependencyEdge>,
}
```

This validation runs at template publish time, not at runtime. A template that passes validation is guaranteed acyclic for all consumers. If a new prerequisite edge is added to an existing template and that edge introduces a cycle, the publish is rejected with the specific cycle path reported. This is a schema-level constraint, not a runtime check.

### 6c. DAG 2 — The Macro Expansion DAG (derived from DAG 1)

When the macro compiler processes M1 ("onboard a UCITS SICAV"), it reads the constellation template for LU UCITS SICAVs and produces an ordered sequence of singleton DSL verbs. The ordering is derived from DAG 1's dependency edges:

```
DAG 1 says: CBU depends on Deal, UBO depends on CBU, KYC depends on CBU, ...
Therefore: cbu.create must precede ubo.discover, which must precede kyc.open, ...
```

The macro compiler performs a topological sort of DAG 1's dependency edges, maps each entity-type node to its creation/setup verbs, and emits the compiled runbook. The resulting DAG 2 has:

- **Nodes:** individual singleton DSL verb invocations
- **Edges:** "must execute before" derived from DAG 1 prerequisite edges + verb-internal ordering (e.g., `cbu.create` before `cbu.set-domicile` because you can't set a domicile on a CBU that doesn't exist yet)
- **Scope:** the entire lifecycle of a macro from first verb to last

**Derivation invariant:** Every dependency edge in DAG 2 must trace back to either (a) a DAG 1 prerequisite edge or (b) an intra-entity verb ordering rule. If a DAG 2 edge exists that has no DAG 1 justification and no intra-entity justification, it is an implementation bug — the macro compiler has invented an ordering constraint that the constellation template doesn't support.

### 6d. DAG 3 — The Cross-Entity Plan DAG (derived from DAG 1)

When the cross-entity plan compiler (§9) processes an utterance like "close KYC-34 then terminate CBU-7742," it constructs an execution plan. If the user provides explicit ordering ("close X **then** terminate Y"), that ordering becomes an edge. But when ordering is implicit, the compiler infers it from DAG 1:

```
DAG 1 says: KYC is a child/prerequisite of CBU
DAG 1 says: CBU termination has a TerminationPrerequisite on KYC (KYC must be resolved)
Therefore: kyc.close must precede cbu.terminate
```

DAG 3 is more dynamic than DAG 2. It is compiled at runtime from a specific utterance against specific live entity instances. It may include:

- **Explicit edges** from user ordering ("close X then terminate Y")
- **Inferred edges** from DAG 1 prerequisite semantics
- **Exclusion constraints** from negative predicates ("leave KYC untouched")
- **Constellation re-check gates** between nodes (mid-plan constellation re-evaluation)

**Derivation invariant:** Same as DAG 2 — every inferred edge must trace to a DAG 1 prerequisite. Explicit user-ordered edges override DAG 1 ordering only when they are consistent (strengthening the order is fine; contradicting it triggers a clarification dialogue).

**User ordering conflict rule (non-negotiable):** If a user states an ordering that contradicts a DAG 1 prerequisite edge, the system must refuse the plan and trigger clarification — it may never silently reshape the plan to accommodate the contradiction. The user said "terminate CBU-7742 then close KYC-34," but DAG 1 says KYC must be resolved before CBU termination. The system does not reorder silently. It says:

> *"You asked to terminate CBU-7742 before closing KYC-34, but KYC cases must be resolved before their parent CBU can be terminated. Would you like to: (a) reverse the order — close KYC-34 first, then terminate CBU-7742, or (b) cancel?"*

The trace records `HaltReason::DagOrderingConflict` with the user-stated order, the DAG 1 prerequisite that contradicts it, and the suggested correction. Silent plan reshaping is an implementation bug — it hides a misunderstanding that the user needs to resolve.

### 6e. DAG 3 Runtime Execution Law (new in v0.3.2 final)

DAG 3 is a runtime object, unlike DAG 1 (static metadata) and DAG 2 (compile-time artifact). Its execution semantics need explicit specification because this is where safety-critical behaviour happens — verbs fire, states change, constellations shift.

**Mutability model: compile-once, re-check-per-node.**

DAG 3 is immutable once compiled. The node set, edge set, and exclusion predicates are fixed at compilation time and do not change during execution. However, the *legality assessment* of each node is re-evaluated before that node fires. This means:

1. **Plan compilation** — the cross-entity plan compiler produces the full DAG 3 (nodes, edges, exclusions). This is a snapshot of intent.
2. **Node execution** — before each node fires, Phase 2 constellation legality is re-checked against the *current* constellation state (which may have changed due to earlier nodes in the plan).
3. **Re-check outcome:**
   - **Still legal** → execute the node, record in child trace, proceed to next node
   - **Newly blocked** → halt the plan at this node with `HaltReason::MidPlanConstellationBlock`. The completed nodes remain committed. The remaining nodes are abandoned. The trace records partial completion.
   - **Newly illegal (state conflict)** → same as blocked. The plan does not attempt to recompile or re-route.

**Why immutable-plan + per-node-recheck?** The alternative — recompiling the plan after each node — would allow the plan to silently change shape mid-execution. A user who approved "close KYC then terminate CBU" should not discover that the system recompiled into "close KYC then suspend CBU" because a state propagation made termination illegal mid-flight. Immutable plans preserve the user's intent; per-node re-checks preserve safety. When the two conflict, safety wins and the plan halts with an explanation.

**What triggers a re-check failure:**

| Earlier node caused... | Effect on later node | Result |
|----------------------|---------------------|--------|
| State transition on linked entity | Constellation predicate now blocks later verb | `MidPlanConstellationBlock` |
| State propagation cascade | Later node's entity transitioned automatically | `MidPlanConstellationBlock` (entity is no longer in expected state) |
| Side effect created new entity | New entity introduces new constellation predicates | Re-check includes new entity; may block or pass |
| No state change (observation verb) | No effect | Always passes |

**Trace structure for partial plan execution:**

```rust
struct PartialPlanExecution {
    plan_dag: CrossEntityPlan,           // the immutable compiled plan
    completed_nodes: Vec<Uuid>,          // child trace IDs of nodes that executed
    halted_at_node: Option<usize>,       // node index where re-check failed
    halt_reason: Option<HaltReason>,
    remaining_nodes: Vec<usize>,         // node indices not executed
    constellation_at_halt: ConstellationSnapshot, // state when plan halted
}
```

### 6f. Exclusion Predicate Materialisation in DAG 3 (new in v0.3.2 final)

§9 defines exclusion predicates conceptually ("leave KYC untouched"). This section specifies how they materialise in DAG 3's structure.

**Exclusions are not nodes.** An exclusion does not create a "do nothing" node in the DAG. Exclusions are constraints checked at two compile-time validation points and one runtime enforcement point:

**Compile-time validation 1 — Node rejection:**
When the plan compiler generates DAG 3 nodes, any node that would mutate an excluded entity is rejected before the plan is finalised:

```rust
// During plan compilation:
for node in &candidate_nodes {
    for exclusion in &exclusion_predicates {
        if exclusion.matches(node.entity_id, node.verb_id) {
            // This node violates an exclusion — reject it from the plan
            rejected_nodes.push(RejectedNode {
                node: node.clone(),
                exclusion: exclusion.clone(),
                reason: "Excluded by user constraint",
            });
        }
    }
}
```

If rejecting the node makes the plan infeasible (e.g., the user said "terminate CBU but leave KYC untouched," but DAG 1 says KYC must be closed before CBU termination), the plan compilation fails with `HaltReason::ExclusionMakesPlanInfeasible`:

> *"You asked to terminate CBU-7742 but leave KYC-34 untouched. However, KYC cases must be resolved before their parent CBU can be terminated. These constraints are contradictory. Would you like to: (a) remove the KYC exclusion and close KYC-34 first, or (b) cancel?"*

**Compile-time validation 2 — Side-effect prediction:**
The plan compiler checks whether any *allowed* node could trigger a state propagation (via DAG 1's `StatePropagationRule`) that would affect an excluded entity. If a propagation cascade could reach an excluded entity:

- If the propagation effect is `AdvisoryNotification` → allow (no state change)
- If the propagation effect is `CascadeTransition` or `VerbSetInvalidation` → warn the user at plan presentation time: "Executing this plan may indirectly affect KYC-34 through state propagation. Proceed?"
- If the propagation effect is certain (not conditional) → reject the plan as infeasible

**Runtime enforcement — Post-node exclusion check:**
After each node executes and the constellation is re-evaluated, the system checks whether any excluded entity's state has changed. If it has:

```rust
// After node execution, before proceeding to next node:
for exclusion in &plan.exclusion_predicates {
    let current_state = get_entity_state(exclusion.target_entity());
    let pre_plan_state = plan.initial_constellation.get_state(exclusion.target_entity());
    if current_state != pre_plan_state {
        // Exclusion violated by side effect
        halt with HaltReason::ExclusionViolation {
            excluded_entity: exclusion.target_entity().to_string(),
            violated_by_verb: last_executed_node.verb_id.to_string(),
            exclusion_source: exclusion.source_phrase.clone(),
            previous_state: pre_plan_state,
            new_state: current_state,
        };
    }
}
```

This three-point enforcement model means exclusions are checked at plan compilation (can we even build this plan?), at plan presentation (will side effects violate exclusions?), and at runtime (did side effects actually violate exclusions?). The trace records which check caught the violation.

### 6g. How DAG 1 Feeds the Pipeline

DAG 1 is consumed at multiple points in the trace pipeline:

| Consumer | What it reads from DAG 1 | Purpose |
|----------|--------------------------|---------|
| **Phase 2 — Constellation Recovery** | Entity type nodes + dependency edges | Build the constellation graph from a UUID |
| **Phase 2 — Constellation Predicates** | State prerequisite edges + termination prerequisites | Determine which verbs are blocked by linked entity states |
| **Phase 2 — Situation Signature** | Expected entity types + dependency topology | Compute operational phase from constellation shape |
| **Phase 2 — Verb Taxonomy** | State propagation rules + prerequisite semantics | Classify cross-entity couplings for state-verbs |
| **Phase 3 — Pattern Matching** | Constellation topology | Match situation signatures against known patterns |
| **Phase 5 — Post-execution** | State propagation rules | Determine which linked entities need re-evaluation after state change |
| **§8 — Macro Compiler** | Dependency edges (topological sort) | Produce DAG 2 execution ordering |
| **§9 — Cross-Entity Plan Compiler** | Prerequisite semantics | Infer DAG 3 dependency edges |
| **§9 — Exclusion Enforcement** | Dependency edges | Validate that exclusions don't create impossible plans |

**Single source of truth principle:** If two consumers of DAG 1 disagree about ordering (e.g., the macro compiler thinks UBO must come before KYC, but the cross-entity plan compiler thinks the reverse), the bug is in the consumer, not in DAG 1. The constellation template is authoritative.

### 6h. Constellation Template Versioning

Because DAG 1 is the foundation of so many pipeline components, it must be version-pinned alongside the other surfaces:

```rust
// Already in SurfaceVersions (§4b), now explicitly justified:
struct SurfaceVersions {
    // ... existing fields ...
    constellation_template_version: String, // hash of the constellation template DAG
}
```

When a constellation template changes (new entity types added, prerequisite edges modified, state propagation rules updated), the replay engine must be able to detect whether the change affects historical trace outcomes. The template version pin enables this: replay runs the historical utterance against the new template and diffs not just the resolved verb but also the constellation snapshot, the legal verb set, and the inferred DAG edges.

### 6i. Trace Fields for DAG Provenance

The trace should record which DAG edges were active during resolution, so that diagnostic queries can answer "why was this verb ordered before that one?" and "which constellation template edge justified this blocking predicate?"

```rust
struct DagProvenance {
    /// Which constellation template was used
    template_id: String,
    template_version: String,

    /// Which dependency edges were traversed during constellation recovery
    traversed_edges: Vec<TraversedEdge>,

    /// Which state propagation rules fired (if any, post-execution)
    propagation_rules_fired: Vec<StatePropagationRule>,
}

struct TraversedEdge {
    from_type: String,
    to_type: String,
    relationship: String,
    direction: HydrationDirection,
    /// The live instances this edge connected
    from_instance: EntityId,
    to_instance: EntityId,
}
```

This is added to `EntityTrace` (Phase 2) as an optional field:

```rust
struct EntityTrace {
    // ... existing fields ...

    // DAG provenance (v0.3.2)
    dag_provenance: Option<DagProvenance>,
}
```

And to `MacroContext` and `CrossEntityPlan` to record which DAG 1 edges justified their DAG 2/3 orderings:

```rust
struct MacroContext {
    // ... existing fields ...
    dag1_edges_used: Vec<TraversedEdge>,  // v0.3.2: which template edges produced this ordering
}
```

### 6j. Worked Example — One Prerequisite Edge, Three DAGs (new in v0.3.2 final)

This example traces a single DAG 1 prerequisite edge — **CBU → KYC (TerminationPrerequisite)** — through all three DAGs to show how the derivation hierarchy works end to end.

**DAG 1 — The constellation template edge:**

```
Template: "lu-ucits-sicav"
Edge: {
    from_type: "kyc-case",
    to_type: "cbu",
    relationship: "scoped-to",
    prerequisite_semantics: TerminationPrerequisite,
    // Meaning: a CBU cannot be terminated while any of its KYC cases are unresolved
}
```

This edge exists in the static template metadata. It says nothing about specific CBUs or KYC cases — it is a type-level rule. It was validated as acyclic when the template was published.

**DAG 2 — The same edge during onboarding (macro compilation):**

User says: "Onboard a UCITS SICAV for Acme Corp." This triggers macro M1. The macro compiler reads the `lu-ucits-sicav` template, topologically sorts the dependency edges, and produces DAG 2:

```
DAG 2 nodes (excerpt):
  Node 3: kyc.open (creates KYC case for the new CBU)
  Node 7: tollgate.check (reads KYC status among other things)

DAG 2 edge:
  Node 3 → Node 7 (kyc.open must execute before tollgate.check)
  Justified by: DAG 1 edge "kyc-case scoped-to cbu" +
                DAG 1 edge "tollgate gates kyc-case" (TollgatePrerequisite)
```

The DAG 2 edge doesn't reference the TerminationPrerequisite directly — during onboarding, the relevant prerequisite semantics are ExistencePrerequisite (KYC can't exist without CBU) and TollgatePrerequisite (tollgate reads KYC). But the same DAG 1 edge (`kyc-case → cbu`) is the source. The macro trace records `dag1_edges_used` including this edge.

**DAG 3 — The same edge during termination (runtime plan):**

Six months later, the user says: "Close KYC-34 then terminate CBU-7742."

The cross-entity plan compiler builds DAG 3:

```
DAG 3 nodes:
  Node 0: kyc.close kyc-34
  Node 1: cbu.terminate cbu-7742

DAG 3 edge:
  Node 0 → Node 1 (kyc.close must execute before cbu.terminate)
  Justified by: DAG 1 edge "kyc-case → cbu" with TerminationPrerequisite
  Also reinforced by: explicit user ordering ("close ... then terminate")
```

In this case, the user's explicit ordering is **consistent** with DAG 1 — they said "close KYC then terminate CBU," which is exactly the order DAG 1 requires. The plan compiles successfully.

**DAG 3 — The same edge when the user contradicts it:**

Now imagine the user says: "Terminate CBU-7742, then close KYC-34."

The cross-entity plan compiler detects a conflict:

```
User-stated order: cbu.terminate → kyc.close
DAG 1 requires:    kyc.close → cbu.terminate (TerminationPrerequisite)
Conflict: user order contradicts DAG 1 prerequisite
```

Plan compilation halts. `HaltReason::DagOrderingConflict`:

> *"You asked to terminate CBU-7742 before closing KYC-34, but KYC cases must be resolved before their parent CBU can be terminated. Would you like to reverse the order?"*

The trace records the DAG 1 edge that caused the conflict, the user's stated order, and the suggested correction.

**DAG 3 — The same edge with an exclusion that makes the plan infeasible:**

User says: "Terminate CBU-7742 but leave KYC-34 untouched."

The plan compiler attempts to build DAG 3:
- Node 0: `cbu.terminate cbu-7742`
- Exclusion: KYC-34 must not be mutated

But DAG 1's TerminationPrerequisite says KYC must be resolved before CBU termination. KYC-34 is currently OPEN. The only way to satisfy the prerequisite is to close or withdraw KYC-34 — which the exclusion forbids.

`HaltReason::ExclusionMakesPlanInfeasible`:

> *"You asked to terminate CBU-7742 but leave KYC-34 untouched. However, KYC-34 is OPEN, and all KYC cases must be resolved before their parent CBU can be terminated. These constraints are contradictory. Would you like to: (a) remove the KYC exclusion and close KYC-34 first, or (b) cancel?"*

**One edge, four scenarios.** The same DAG 1 prerequisite edge produces:
1. A DAG 2 ordering edge during onboarding
2. A consistent DAG 3 ordering edge during termination
3. A `DagOrderingConflict` halt when the user contradicts it
4. An `ExclusionMakesPlanInfeasible` halt when an exclusion makes it unsatisfiable

All four scenarios trace back to the same template edge. All four are recorded in the trace with DAG provenance pointing at that edge. That is the derivation hierarchy in action.

---

## 7. Constellation Integration with Singleton DSL Verbs

This section addresses the architectural relationship between the constellation model and the individual DSL verbs that the REPL actually executes.

### The Core Tension

DSL verbs are singletons. `cbu.suspend` operates on one CBU. `kyc.close` operates on one KYC case. The REPL engine doesn't understand constellations — it understands individual entity operations.

But business operations are inherently multi-entity. "Onboard a UCITS SICAV" involves creating a CBU, discovering UBOs, opening KYC cases, defining service intents, setting up trading profiles, and passing tollgates. "Terminate a custody relationship" means checking that all linked KYC cases are resolved, all trading profiles are deactivated, and the parent deal's other CBUs aren't affected.

### How It Resolves

The constellation model operates at three levels, each feeding into the trace pipeline:

**Level 1 — Pre-execution constellation check (Phase 2):**
Before any singleton verb executes, the constellation snapshot is consulted for blocking predicates. This is the state-gating layer described in Phase 2. The verb itself is still a singleton, but its execution is governed by the constellation.

**Level 2 — Macro expansion into singleton verb sequences (Phase 5):**
Multi-entity operations are macros that expand into ordered sequences of singleton verbs. The macro expansion compiler (M1–M18 for CBU structures) produces a compiled runbook — a DAG of singleton verbs with dependency edges. Each verb in the DAG executes independently against the REPL, but the DAG structure ensures correct ordering.

```
Macro: "onboard a UCITS SICAV" (M1)
Expands to:
  cbu.create → cbu.set-domicile → cbu.set-fund-type →
  ubo.discover → ubo.verify →
  kyc.open → kyc.assign-analyst →
  si.create → si.define-scope →
  tp.create → tp.set-instrument-universe →
  tollgate.create → tollgate.check
```

Each singleton verb in this sequence gets its own child trace (linked by `correlation_id`). The parent trace records the macro identity. The audit trail records only the expanded verbs.

**Level 3 — Post-execution constellation update (Phase 5):**
After a singleton verb changes an entity's state, the constellation may need re-evaluation. A `cbu.suspend` on CBU-7742 might make verbs on linked service intents illegal (can't approve a service intent for a suspended CBU). The post-execution constellation snapshot captures this propagation, and subsequent verbs in a macro's DAG are re-checked against the updated constellation.

### How the Verb Taxonomy Bridges Constellation and Singleton Verbs (v0.3.1)

The entity-verb / state-verb duality (§3 Phase 2d) is the mechanism that connects the constellation's multi-entity awareness to the singleton verb execution model:

1. **Entity verbs** are immune to constellation state. `cbu.get-status` works regardless of what KYC or UBO entities are doing. The constellation has no influence on their resolution — they pass through Phase 3's taxonomy filter untouched.

2. **State verbs with no cross-entity coupling** are gated by their own entity's FSM but not by the constellation. `cbu.validate` requires CBU to be in PENDING_VALIDATION — it doesn't care about linked entity states.

3. **State verbs with cross-entity coupling** are where the constellation shapes resolution. `tp.validate` is technically legal when TP is in PENDING_VALIDATION, but it's operationally meaningful only when the parent CBU is at least VALIDATED. The constellation snapshot provides the coupled entity state; the verb taxonomy classifies the coupling; Phase 3 uses the coupling to rank.

This three-tier classification means the constellation's influence on resolution is graduated, not binary. It ranges from "no influence" (entity verbs) through "soft influence" (coupled state verbs, ranking adjustment) to "hard influence" (constellation predicates blocking execution in Phase 2).

### Constellation-Aware Trace Fields

The constellation surfaces in the trace at these points:

| Trace Phase | Constellation Data | Purpose |
|-------------|-------------------|---------|
| Phase 2 `EntityTrace` | `constellation_snapshot` | Pre-execution state of all linked entities |
| Phase 2 `EntityTrace` | `constellation_blocks` | Verbs blocked by linked entity states |
| Phase 2 `EntityTrace` | `situation_signature` | Operational fingerprint for pattern matching (v0.3.1) |
| Phase 2 `EntityTrace` | `verb_taxonomy` | Entity-verb/state-verb classification (v0.3.1) |
| Phase 3 `ECIRTrace` | `pattern_match` | Constellation pattern match details (v0.3.1) |
| Phase 5 `ExecutionTrace` | `post_constellation_snapshot` | Post-execution state — what changed |
| Macro `MacroContext` | `child_traces[]` | Per-verb traces within macro expansion |
| Halt `ConstellationBlock` | blocking entity + predicate | Why a verb was refused |

---

## 8. Macro Expansion and DAG Correlation

When a user says "onboard a UCITS SICAV," the utterance resolves to a macro that expands into 20–40 compiled verbs via the macro expansion compiler.

```rust
struct MacroContext {
    macro_id: String,                     // e.g., "M1"
    macro_name: String,                   // e.g., "cbu.create-lu-sicav"
    expansion_dag: Vec<DagNode>,          // the compiled runbook as a DAG
    child_traces: Vec<Uuid>,              // trace_ids of individual verb executions
    correlation_id: Uuid,                 // groups all child traces under one utterance
    constellation_at_expansion: ConstellationSnapshot, // state when macro was compiled
}

struct DagNode {
    verb_id: VerbId,
    depends_on: Vec<VerbId>,              // DAG edges
    execution_order: usize,               // topological sort position
    child_trace_id: Option<Uuid>,         // links to child UtteranceTrace
    pre_check: Option<ConstellationLegalityCheck>, // re-checked before each verb
}
```

**Key invariant:** Audit trail records expanded verbs, never macro names. But the trace records both — macro identity (for human diagnosis) and expanded verbs (for execution audit). The `correlation_id` links them.

**Mid-macro constellation re-check:** After each verb in the DAG executes, the constellation is re-evaluated before the next verb fires. If a mid-macro verb causes a constellation block for a subsequent verb, the DAG halts at that point with a partial execution trace. This prevents a macro from blindly executing through a state that has become illegal mid-flight.

---

## 9. Cross-Entity Execution Model (new in v0.3.2)

The paper recognises multi-entity and cross-entity utterances (§3 Phase 2f), but until now did not define how those utterances become executable trace structures. This section closes that gap.

### The Problem

When a user says "re-open onboarding for the Irish fund but leave KYC untouched," the system must:
1. Resolve two entities (the Irish fund CBU + its KYC case)
2. Associate specific verbs with specific entities (re-open → CBU, leave untouched → KYC)
3. Handle a **negative constraint** ("leave KYC untouched" = explicit exclusion)
4. Determine execution ordering and dependency structure
5. Produce a trace structure that captures all of this

Without a defined execution model, implementation will improvise in exactly the part of the system where safety matters most.

### Three Execution Shapes

Every utterance that passes Phase 2 is classified into one of three execution shapes. The shape determines how the utterance maps to traces and REPL commands.

```rust
enum ExecutionShape {
    /// One entity, one verb, one trace
    SingletonTarget {
        entity_id: EntityId,
        verb_id: VerbId,
        dsl_command: String,
    },

    /// One verb applied to multiple entities independently (fan-out)
    /// Each entity gets its own trace; all share a correlation_id
    /// No dependency ordering between entities — they execute in parallel or any order
    BatchTarget {
        entities: Vec<EntityId>,
        verb_id: VerbId,                    // same verb applied to all
        per_entity_legality: Vec<(EntityId, ConstellationLegalityCheck)>,
        exclusions: Vec<BatchExclusion>,    // entities explicitly excluded
        correlation_id: Uuid,
    },

    /// Multiple entities, potentially different verbs, with dependency ordering
    /// Compiled into a DAG; each node gets its own child trace
    CrossEntityPlan {
        plan_nodes: Vec<PlanNode>,
        dependency_edges: Vec<(usize, usize)>,  // (from_node, to_node)
        exclusion_predicates: Vec<ExclusionPredicate>,
        correlation_id: Uuid,
    },
}
```

### When Each Shape Applies

| Utterance Pattern | Execution Shape | Example |
|-------------------|----------------|---------|
| "Freeze CBU-7742" | `SingletonTarget` | One entity, one verb |
| "Suspend all CBUs under deal D-401" | `BatchTarget` | Filter → multiple entities, same verb |
| "Re-open onboarding for the Irish fund but leave KYC untouched" | `CrossEntityPlan` | Multiple entities, different verbs, negative constraint |
| "Close KYC-34 then terminate CBU-7742" | `CrossEntityPlan` | Explicit ordering across entities |
| "Onboard a UCITS SICAV" | Macro expansion (§8) | Template → compiled runbook DAG |

### Negative Constraints and Exclusion Predicates

Cross-entity utterances frequently include negative constraints: "but leave X untouched," "don't change Y," "keep Z as-is." These are not verbs — they are explicit exclusions that constrain which entities the plan may modify.

```rust
struct ExclusionPredicate {
    /// The entity (or entity type) that must not be modified
    excluded_entity: ExclusionTarget,
    /// The linguistic source of the exclusion
    source_phrase: String,          // "leave KYC untouched"
    /// What the exclusion prevents
    excluded_operations: ExcludedOps,
}

enum ExclusionTarget {
    SpecificEntity(EntityId),       // "leave KYC-34 untouched"
    EntityType(String),             // "leave all KYC cases untouched"
    EntityByRelation(String),       // "leave the parent deal untouched"
}

enum ExcludedOps {
    AllMutations,                   // "leave X untouched" = no state changes
    SpecificVerbs(Vec<VerbId>),     // "don't close X" = specific verb excluded
    StateTransitions,               // "keep X in current state"
}
```

**Enforcement:** Exclusion predicates are checked at two points:
1. **Plan compilation** — any plan node that would mutate an excluded entity is rejected before execution
2. **Mid-execution re-check** — if a side effect of an allowed verb propagates a state change to an excluded entity, the plan halts with `HaltReason::ExclusionViolation`

### BatchTarget: Fan-Out Execution

For `BatchTarget`, the same verb is applied to multiple entities. Each entity gets its own child trace. Entities that fail the constellation legality check are separated into a blocked set with per-entity reasons:

```rust
struct BatchExclusion {
    entity_id: EntityId,
    reason: BatchExclusionReason,
}

enum BatchExclusionReason {
    ConstellationBlocked(ConstellationBlock),
    ExplicitlyExcluded(ExclusionPredicate),
    StateLegality(String),
}
```

**Trace structure:** One parent trace (with `macro_context` recording the batch) and N child traces (one per entity that passed legality). Blocked entities are recorded in the parent trace but do not produce child traces.

### CrossEntityPlan: DAG Compilation

For `CrossEntityPlan`, the system compiles the utterance into a DAG of plan nodes with dependency edges:

```rust
struct PlanNode {
    node_id: usize,
    entity_id: EntityId,
    verb_id: VerbId,
    dsl_command: String,
    pre_check: ConstellationLegalityCheck,
}
```

**Compilation rules:**
1. If the utterance specifies explicit ordering ("close X **then** terminate Y"), the ordering becomes a dependency edge
2. If no explicit ordering is given, the system infers dependencies from constellation predicates (if terminating CBU requires KYC to be closed, `kyc.close` must precede `cbu.terminate`)
3. If no dependency exists, nodes are independent and may execute in any order
4. Exclusion predicates are validated against all plan nodes at compilation time

**Trace structure:** Same as macro expansion — one parent trace with `correlation_id`, child traces per node, constellation re-check between each node.

### Execution Shape in the Trace

The execution shape is recorded in the composite trace and hoisted for indexing:

```rust
// Added to UtteranceTrace
struct UtteranceTrace {
    // ... existing fields ...
    execution_shape: Option<ExecutionShapeKind>,  // v0.3.2
    exclusion_predicates: Vec<ExclusionPredicate>, // v0.3.2
}

enum ExecutionShapeKind {
    Singleton,
    Batch { entity_count: usize, blocked_count: usize },
    CrossEntityPlan { node_count: usize, edge_count: usize },
    MacroExpansion { macro_id: String },
}
```

### New Halt Reason

```rust
// Added to HaltReason enum
ExclusionViolation {
    excluded_entity: String,
    violated_by_verb: String,
    exclusion_source: String,  // the original user phrase
},
```

---

## 10. The Three Feedback Loops

### Loop 1 — DSL Discovery (Developer)

**Input:** Traces where `outcome == NoMatch` or `halt_reason == NoViableVerb`.

**Output:** Gap entries in `macro_verb_corrections.yaml` format with `GAP` codes:

```yaml
- original: ~
  corrected: "cbu.freeze"
  code: GAP
  source_trace: "trace_id:abc123"
  evidence:
    utterance: "freeze the CBU"
    entity: "cbu"
    entity_state: "ACTIVE"
    constellation_clear: true
    ecir_narrowed_to: 0
    nearest_verb: "cbu.suspend"
    embedding_distance: 0.34
```

**Destination:** Constellation remediation pipeline — same format, same review process, same Codex execution path.

**New in v0.3 — constellation gap detection:** Traces where `halt_reason == ConstellationBlock` but the user's intent was clearly valid (e.g., the user knew the KYC case was open and wanted to terminate anyway, suggesting a missing "force-terminate" verb or a "terminate-with-cascade" macro). These generate gap entries with code `CONSTELLATION_GAP` — suggesting that the verb surface needs a constellation-aware variant.

### Loop 2 — User Clarification (Runtime)

Clarification prompts are generated from trace data:

| Halt Reason | Clarification Pattern |
|-------------|----------------------|
| `StateConflict` | "You asked to {verb} this {entity}, but it's in {state}. Available: {valid_verbs}." |
| `ConstellationBlock` | "You asked to {verb} {entity}, but {blocking_entity} is {blocking_state}. {resolution_hint}" |
| `AmbiguousEntity` | "Which one? {candidates with distinguishing attributes}" |
| `BelowConfidenceThreshold` | "Did you mean: (1) {verb_a}, (2) {verb_b}, (3) {verb_c}?" |
| `AmbiguousResolution` | "Your request matches {verb_a} and {verb_b} equally. Which?" |
| `MissingReferentialContext` | "I'm not sure what '{pronoun}' refers to. Can you specify?" |
| `InsufficientScopeBinding` | "'{filter}' could match {n} entities. Can you narrow the scope?" |

Each clarification response creates a new trace (kind: `ClarificationResponse`) linked to the original, forming the analysable tree described in §4a.

### Loop 3 — Operational Pattern Learning (new in v0.3.1)

**Non-negotiable boundary (v0.3.2):** Pattern learning may boost, demote, or suggest; it may never authorise, legalise, or block independently of Phase 2 legality. Loop 3 output influences plausibility ranking in Phase 3. It never modifies the legal verb set, never alters state transitions, and never creates or enforces policy. The Phase 2 legality contract is the sole authority on what is permitted. Loop 3 is an optimisation layer, not a governance layer. Any implementation that allows learned patterns to override Phase 2 legality is a bug.

**Input:** All completed traces (both successful and corrected), grouped by situation signature.

**What it produces:** Empirical verb-frequency distributions per situation signature, cross-entity coupling validation data, and operational pathway models.

**This is the loop the v0.3 paper was missing.** Loop 1 discovers DSL gaps. Loop 2 clarifies ambiguity at runtime. Loop 3 teaches the system *what users actually do* in specific operational situations, making future resolution faster and more accurate.

#### 3a. Verb-Frequency Distributions

For each situation signature that has accumulated sufficient traces (threshold: 10+ traces), compute:

```rust
struct SituationVerbDistribution {
    signature_hash: u64,
    canonical_form: String,
    total_traces: usize,
    verb_frequencies: Vec<VerbFrequency>,
    last_updated: DateTime<Utc>,
}

struct VerbFrequency {
    verb_id: VerbId,
    count: usize,
    frequency: f32,           // count / total_traces
    avg_confidence: f32,      // average Phase 4 confidence when this verb resolved
    correction_rate: f32,     // how often users corrected after this verb executed
}
```

**Example:** For signature `cbu:ACTIVE|kyc:OPEN|si:APPROVED|tp:PENDING_VALIDATION|ubo:VERIFIED` with 87 historical traces:

```
kyc.get-status:        28 traces (32.2%) — avg confidence 0.91, correction rate 0.02
kyc.escalate:          19 traces (21.8%) — avg confidence 0.84, correction rate 0.05
kyc.assign-analyst:    14 traces (16.1%) — avg confidence 0.88, correction rate 0.03
cbu.get-status:        12 traces (13.8%) — avg confidence 0.94, correction rate 0.01
tp.validate:            8 traces  (9.2%) — avg confidence 0.79, correction rate 0.12
cbu.annotate:           6 traces  (6.9%) — avg confidence 0.92, correction rate 0.00
```

This distribution tells Phase 3: in this situation, the user is most likely querying or escalating KYC. `tp.validate` happens sometimes but has a high correction rate (users often do it prematurely), so it should be ranked lower than its frequency alone would suggest.

#### 3b. Cross-Entity Coupling Validation

Loop 3 validates the hand-coded cross-entity coupling rules (from §3 Phase 2d) against empirical data. If the coupling says "`tp.validate` is operationally meaningful only when parent CBU is VALIDATED or ACTIVE" but traces show that users successfully and without correction execute `tp.validate` when CBU is in DRAFT, then the coupling rule is wrong and should be relaxed.

```rust
struct CouplingValidationReport {
    coupling_rule: CrossEntityCoupling,
    traces_where_coupling_satisfied: usize,
    traces_where_coupling_violated: usize,
    correction_rate_when_satisfied: f32,
    correction_rate_when_violated: f32,
    recommendation: CouplingRecommendation,
}

enum CouplingRecommendation {
    /// Empirical data confirms the coupling — keep it
    Confirmed,
    /// Users routinely violate the coupling successfully — relax it
    RelaxCoupling,
    /// Users violate and then correct — tighten it to a hard gate
    TightenToGate,
    /// Insufficient data to validate
    InsufficientData,
}
```

This closes the gap between hand-coded domain knowledge and observed operational reality. The coupling taxonomy isn't static — it evolves based on what users actually do.

#### 3c. Operational Pathway Models

Beyond individual verb frequencies, Loop 3 can detect common **sequences** of verbs within a constellation lifecycle. By analysing traces grouped by `correlation_id` (macro traces) and by session + constellation (sequential user actions), the system builds pathway models:

```rust
struct OperationalPathway {
    starting_signature: String,       // constellation state at pathway start
    ending_signature: String,         // constellation state at pathway end
    verb_sequence: Vec<VerbId>,       // the sequence of verbs observed
    frequency: usize,                 // how many times this pathway appeared
    avg_duration: Duration,           // how long the pathway typically takes
    is_macro_derived: bool,           // did this pathway come from a macro expansion?
}
```

**Why this matters:** Pathway models enable predictive prompting. If the user has just executed `kyc.close` and the constellation now matches a signature where the most common next action is `tollgate.check`, the system can proactively suggest: "KYC case closed. Ready to run the tollgate check?" This turns Sage from a reactive command interpreter into a proactive operational assistant — the constellation pattern tells it what's likely next.

#### 3d. Loop 3 Data Flow

```
Completed traces (all outcomes)
        │
        ▼
┌───────────────────────────┐
│ Aggregate by situation    │
│ signature                 │
└───────────┬───────────────┘
            │
    ┌───────┼──────────┬──────────────────┐
    │       │          │                  │
    ▼       ▼          ▼                  ▼
 Verb     Coupling   Pathway          Signature
 frequency validation models           catalogue
 distrib.  reports                     enrichment
    │       │          │                  │
    ▼       ▼          ▼                  ▼
 Phase 3   Phase 2    Sage             Phase 3
 pattern   coupling   predictive       static
 filter    taxonomy   prompting        catalogue
           updates                     updates
```

**Cadence:** Loop 3 runs as a batch process (daily or on-demand), not in the hot path. It produces updated verb-frequency distributions and coupling validation reports that are loaded into the Phase 3 pattern matcher at startup. The trace corpus is the training data; the pattern catalogue is the model; Phase 3 is the inference engine.

---

## 11. The No-Match Path

| Scenario | Halt Reason | Trace Outcome | Action |
|----------|------------|---------------|--------|
| Valid domain, missing verb | `NoViableVerb` | `NoMatch` | DSL gap → Loop 1 |
| Valid domain, wrong entity type | `NoEntityFound` | `NoMatch` | Entity model gap |
| Valid domain, constellation blocks | `ConstellationBlock` | `HaltedAtPhase(2)` | Constellation gap → Loop 1 |
| Missing referential context | `MissingReferentialContext` | `ClarificationTriggered` | Session context gap |
| Incomplete scope binding | `InsufficientScopeBinding` | `ClarificationTriggered` | Scope UX |
| Outside domain entirely | `NoParsableIntent` | `NoMatch` | Sage conversational response |
| Sage/Coder ambiguity | `BelowConfidenceThreshold` (floor) | `ClarificationTriggered` | "Do something or ask a question?" |

---

## 12. Replay and Regression

### Replay Protocol

1. Load historical `UtteranceTrace` records
2. Re-execute Phases 0–4 against the *current* verb surface, constellation templates, entity FSMs, **and pattern catalogue**
3. Compare resolved verbs, using version pins to identify what changed
4. Produce regression report

```rust
struct ReplayResult {
    trace_id: Uuid,
    original_versions: SurfaceVersions,
    replay_versions: SurfaceVersions,
    version_deltas: Vec<String>,             // which versions changed
    original_resolution: Option<VerbId>,
    replayed_resolution: Option<VerbId>,
    verdict: ReplayVerdict,
}

enum ReplayVerdict {
    Unchanged,
    ImprovedResolution,                       // was NoMatch, now resolves
    DegradedResolution,                       // was resolved, now NoMatch or different
    ChangedResolution { from: VerbId, to: VerbId },
    ConstellationDivergence,                  // same verb, but constellation checks differ
    PatternInfluencedChange,                  // v0.3.1: verb changed due to updated pattern data

    // v0.3.2: narrowing behaviour drift verdicts
    FallbackNewlyRequired,                    // originally resolved without fallback, now needs it
    FallbackNoLongerRequired,                 // originally needed fallback, now resolves normally
    PatternInfluenceIntroduced,               // pattern filter now contributes where it didn't before
    PatternInfluenceRemoved,                  // pattern filter no longer contributes where it did
    CandidateSetExpandedUnexpectedly,         // Phase 3 output is larger than original (weaker narrowing)
    CandidateSetContractedUnexpectedly,       // Phase 3 output is smaller — may indicate over-pruning
}
```

**v0.3.2 addition — narrowing behaviour drift detection:**

The original replay verdicts focused on the final resolved verb: did it change? The v0.3.2 verdicts add a second layer: did the *narrowing behaviour* change, even if the final verb didn't? This catches drift before it becomes a resolution failure.

```rust
struct ReplayNarrowingDiff {
    original_phase3_size: usize,
    replayed_phase3_size: usize,
    original_fallback_invoked: bool,
    replayed_fallback_invoked: bool,
    original_pattern_applied: bool,
    replayed_pattern_applied: bool,
    narrowing_drift: NarrowingDrift,
}

enum NarrowingDrift {
    Stable,                               // same narrowing behaviour
    Weakened { expansion_ratio: f32 },     // candidate set grew — narrowing less effective
    Strengthened { contraction_ratio: f32 }, // candidate set shrank — narrowing more effective
    FallbackRegressed,                     // didn't need fallback before, needs it now
    FallbackImproved,                      // needed fallback before, doesn't now
}
```

**Regression gate (updated for v0.3.2):** In addition to blocking on `DegradedResolution` and unexpected `ChangedResolution`, the replay gate now also flags `FallbackNewlyRequired` as a warning and `CandidateSetExpandedUnexpectedly` as an investigation trigger. These don't block shipment, but they indicate that ECIR's narrowing power is eroding and should be reviewed before the pattern becomes a resolution failure.

**Constellation replay caveat:** Constellation predicates depend on live entity states, which change over time. Replay of Phase 2 uses the `constellation_snapshot` from the original trace (not live state) for deterministic comparison. The `ConstellationDivergence` verdict catches cases where the same verb resolves but the constellation check would now give a different answer due to changed entity relationships.

**Pattern replay caveat (v0.3.1):** Pattern catalogue updates (from Loop 3) can change Phase 3 rankings, potentially changing which verb wins in Phase 4. The `PatternInfluencedChange` verdict flags these separately from verb-surface changes. Pattern-influenced regressions are expected during early learning and should be monitored for stabilisation rather than blocked outright.

---

## 13. Storage Schema

### Canonical Columns (hoisted from JSONB for indexing and operations)

```sql
CREATE TABLE utterance_traces (
    -- ─── Identity ───
    trace_id            UUID PRIMARY KEY,
    utterance_id        UUID NOT NULL,
    session_id          UUID NOT NULL,
    correlation_id      UUID,
    parent_trace_id     UUID REFERENCES utterance_traces(trace_id),
    trace_kind          TEXT NOT NULL,     -- Original | ClarificationPrompt | etc.
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- ─── Raw Input ───
    raw_utterance       TEXT NOT NULL,

    -- ─── Hoisted Canonical Fields (for indexing, dashboards, replay) ───
    resolved_verb       TEXT,              -- NULL if unresolved
    entity_id           TEXT,              -- primary target entity UUID
    entity_type         TEXT,              -- "cbu", "kyc-case", etc.
    entity_state        TEXT,              -- FSM state at resolution time
    outcome             TEXT NOT NULL,     -- ExecutedSuccessfully | NoMatch | etc.
    halt_phase          SMALLINT,          -- 0–5, NULL if completed
    halt_reason_code    TEXT,              -- enum discriminant as text
    resolution_strategy TEXT,              -- ExactMatch | EmbeddingSimilarity | etc.
    requires_confirmation BOOLEAN DEFAULT false,
    constellation_clear BOOLEAN,           -- all constellation predicates passed?
    situation_signature TEXT,              -- v0.3.1: canonical constellation fingerprint
    operational_phase   TEXT,              -- v0.3.1: derived operational phase label

    -- ─── Phase 3 Narrowing Summary (v0.3.2: hoisted for replay diffs and ECIR tuning) ───
    phase2_legal_set_size    SMALLINT,     -- cardinality of Phase 2 legal verb set
    phase3_candidate_set_size SMALLINT,    -- cardinality of Phase 3 output set
    phase4_candidate_set_size SMALLINT,    -- cardinality of Phase 4 input (= Phase 3 output minus any Phase 3 fast-path)
    pattern_influence_applied BOOLEAN DEFAULT false, -- did constellation pattern filter affect ranking?
    fallback_invoked    BOOLEAN DEFAULT false,        -- did Phase 4 invoke the escape hatch?
    fallback_reason_code TEXT,            -- FallbackReasonCode enum if fallback_invoked
    execution_shape_kind TEXT,            -- v0.3.2: Singleton | Batch | CrossEntityPlan | MacroExpansion

    -- ─── Version Pins ───
    surface_versions    JSONB NOT NULL,

    -- ─── Full Phase Payloads (JSONB for flexibility) ───
    plane_trace         JSONB NOT NULL,
    linguistic_trace    JSONB,
    entity_trace        JSONB,
    ecir_trace          JSONB,
    dsl_trace           JSONB,
    execution_trace     JSONB,

    -- ─── Halt Detail ───
    halt_reason         JSONB,

    -- ─── Post-hoc ───
    user_correction     JSONB,
    corrected_at        TIMESTAMPTZ
);

-- ─── Operational Indexes ───
CREATE INDEX idx_traces_outcome        ON utterance_traces (outcome);
CREATE INDEX idx_traces_no_match       ON utterance_traces (outcome) WHERE outcome = 'NoMatch';
CREATE INDEX idx_traces_halt           ON utterance_traces (halt_phase, halt_reason_code)
                                       WHERE halt_phase IS NOT NULL;
CREATE INDEX idx_traces_verb           ON utterance_traces (resolved_verb)
                                       WHERE resolved_verb IS NOT NULL;
CREATE INDEX idx_traces_entity         ON utterance_traces (entity_type, entity_id)
                                       WHERE entity_id IS NOT NULL;
CREATE INDEX idx_traces_correlation    ON utterance_traces (correlation_id)
                                       WHERE correlation_id IS NOT NULL;
CREATE INDEX idx_traces_session        ON utterance_traces (session_id, created_at DESC);
CREATE INDEX idx_traces_constellation  ON utterance_traces (constellation_clear)
                                       WHERE constellation_clear = false;
CREATE INDEX idx_traces_kind           ON utterance_traces (trace_kind, parent_trace_id)
                                       WHERE trace_kind != 'Original';
-- v0.3.1 additions
CREATE INDEX idx_traces_situation      ON utterance_traces (situation_signature)
                                       WHERE situation_signature IS NOT NULL;
CREATE INDEX idx_traces_op_phase       ON utterance_traces (operational_phase, resolved_verb)
                                       WHERE operational_phase IS NOT NULL;
-- v0.3.2 additions
CREATE INDEX idx_traces_fallback       ON utterance_traces (fallback_invoked, fallback_reason_code)
                                       WHERE fallback_invoked = true;
CREATE INDEX idx_traces_pattern        ON utterance_traces (pattern_influence_applied, situation_signature)
                                       WHERE pattern_influence_applied = true;
CREATE INDEX idx_traces_exec_shape     ON utterance_traces (execution_shape_kind)
                                       WHERE execution_shape_kind IS NOT NULL;
CREATE INDEX idx_traces_narrowing      ON utterance_traces (phase2_legal_set_size, phase3_candidate_set_size)
                                       WHERE phase2_legal_set_size IS NOT NULL;

-- ─── Loop 3 Aggregation Table (v0.3.1) ───
CREATE TABLE situation_verb_distributions (
    signature_hash      BIGINT PRIMARY KEY,
    canonical_form      TEXT NOT NULL,
    operational_phase   TEXT,
    total_traces        INTEGER NOT NULL DEFAULT 0,
    verb_frequencies    JSONB NOT NULL,     -- array of {verb_id, count, frequency, correction_rate}
    pathway_data        JSONB,              -- common verb sequences from this signature
    coupling_validations JSONB,             -- coupling rule validation results
    last_updated        TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_svd_phase ON situation_verb_distributions (operational_phase);
```

---

## 14. Modernisation Calculus (Revised)

| Capability | Standard Function-Calling Agent | SemOS + Traceability Kernel |
|------------|--------------------------------|------------------------------|
| Function dispatch | Maps intent to function by name/description | 6-phase pipeline with per-phase trace |
| State awareness | None — calls function regardless of entity state | FSM gating: entity-level AND constellation-level |
| Cross-entity safety | None — each call is independent | Constellation predicates block verbs when linked entities are in incompatible states |
| Situational awareness | None — each call evaluated independently | Constellation pattern matching: situation signature narrows candidates before linguistic analysis (v0.3.1) |
| Verb taxonomy | Flat function list | Entity-verbs vs state-verbs with cross-entity coupling classification (v0.3.1) |
| Failure diagnosis | HTTP error code after the fact | Trace shows which phase failed, why, and what the system tried |
| Verb discovery | Manual — developer adds functions | NoMatch + NoViableVerb traces generate gap reports automatically |
| Regression safety | None | Replay engine with version-pinned traces diffs every historical resolution |
| Confidence model | Single scalar threshold | Legality (binary gates) separated from confidence (ranking scores) |
| Multi-entity operations | Not supported | Macro expansion to DAG of singleton verbs with per-verb constellation re-check |
| Clarification | Generic "I don't understand" | Structured tree with halt-reason-specific prompts and constellation block explanations |
| Entity relationships | Not modelled | Full constellation recovery from UUID, cross-entity state awareness |
| Self-improvement | None | Three feedback loops: DSL discovery, user clarification, operational pattern learning (v0.3.1) |
| Predictive prompting | None | Pathway models suggest likely next actions based on constellation state (v0.3.1) |
| Structural dependency model | Implicit in code, not inspectable | Constellation Template DAG (DAG 1) is the single source of truth; macro ordering and plan inference are derived and auditable (v0.3.2) |
| Cross-entity execution | Manual orchestration | Three formal execution shapes — Singleton, Batch, CrossEntityPlan — with exclusion predicates and DAG-derived ordering (v0.3.2) |
| Narrowing contract | Undefined — model picks from full function set | Phase 3 prune/rank/demote semantics with fallback error-budget monitoring (v0.3.2) |

---

## 15. Worked Example — Constellation-Blocked Utterance

**Utterance:** "Terminate the Acme Corp custody account"

**Phase 0 — Plane:**
```json
{ "plane": "Mutation", "plane_confidence": 0.96,
  "polarity": "Destructive", "polarity_confidence": 0.93,
  "domain_hints": ["custody", "account"] }
```

**Phase 1 — Linguistic:**
```json
{ "verb_phrases": [{"phrase": "terminate", "span": [0, 9]}],
  "noun_phrases": [{"phrase": "Acme Corp custody account", "span": [14, 42]}],
  "referential_bindings": [{"binding_type": "Direct", "confidence": 0.97}] }
```

**Phase 2 — Constellation Recovery:**

Entity resolved: `cbu-7742` (ACTIVE).

Constellation snapshot recovered:
```json
{ "root_entity": "cbu-7742", "root_state": "ACTIVE",
  "linked_entities": [
    { "entity_id": "ubo-91",  "type": "ubo",      "state": "VERIFIED" },
    { "entity_id": "kyc-34",  "type": "kyc-case",  "state": "OPEN" },
    { "entity_id": "si-2201", "type": "service-intent", "state": "APPROVED" },
    { "entity_id": "tp-8870", "type": "trading-profile", "state": "PENDING_VALIDATION" }
  ] }
```

Situation signature: `cbu:ACTIVE|kyc:OPEN|si:APPROVED|tp:PENDING_VALIDATION|ubo:VERIFIED`
Operational phase: `KYCBlocked`

Constellation legality check:
```json
{ "entity_fsm_verbs_for_terminate": true,
  "constellation_blocks": [{
    "blocked_verb": "cbu.terminate",
    "blocking_entity": "kyc-34",
    "blocking_state": "OPEN",
    "predicate": "All KYC cases must be CLOSED or WITHDRAWN before CBU termination",
    "resolution_hint": "Close or withdraw KYC case KYC-34 first"
  }, {
    "blocked_verb": "cbu.terminate",
    "blocking_entity": "tp-8870",
    "blocking_state": "PENDING_VALIDATION",
    "predicate": "All trading profiles must be DEACTIVATED before CBU termination",
    "resolution_hint": "Deactivate trading profile TP-8870 first"
  }] }
```

**→ HALT at Phase 2** — `ConstellationBlock`

**Clarification dialogue generated from trace:**

> *"You asked to terminate CBU-7742 (Acme Corp), but two linked entities are blocking this:*
>
> *1. KYC case KYC-34 is OPEN — all KYC cases must be closed or withdrawn before termination.*
> *2. Trading profile TP-8870 is PENDING_VALIDATION — all trading profiles must be deactivated first.*
>
> *Would you like to: (a) close KYC-34 and deactivate TP-8870 first, (b) suspend the CBU instead, or (c) cancel?"*

If the user selects (a), the system can generate a macro-like sequence: `kyc.close kyc-34` → `tp.deactivate tp-8870` → `cbu.terminate cbu-7742`, each verb getting its own child trace linked by correlation_id, with constellation re-checks between each step.

---

## 16. Worked Example — End-to-End Success with Pattern Matching (updated in v0.3.1)

**Utterance:** "What's the status?"

**Context:** User is in a session focused on CBU-7742. The constellation is the same as the previous example (KYCBlocked situation).

**Phase 0:** `{ plane: Observation, polarity: Neutral }` — eliminates all Mutation verbs.

**Phase 1:** `{ verb_phrases: ["status"], noun_phrases: [], referential_bindings: [{ binding_type: "Implicit" }] }`

Note: no explicit noun phrase. The parser detects an implicit referential binding — the user expects the system to know what they're asking about.

**Phase 2:** Entity resolved via session context → `cbu-7742` (ACTIVE). Constellation recovered. Situation signature: `cbu:ACTIVE|kyc:OPEN|si:APPROVED|tp:PENDING_VALIDATION|ubo:VERIFIED`, operational phase: `KYCBlocked`. No constellation blocks (Observation verbs are never gated).

Legal verb set: `[cbu.get-status, cbu.get-summary, cbu.get-detail, cbu.list-history, cbu.get-status-with-blockers, kyc.get-status, ubo.get-status, si.get-status, tp.get-status]`

Verb taxonomy: all are entity-verbs (observation), no state-verb filtering applies.

**Phase 3 — ECIR with pattern matching:**

- Plane filter: all pass (already observation-only)
- Verb taxonomy filter: all pass (entity-verbs, no coupling)
- **Constellation pattern filter:** Signature matches `KYCBlocked`. Historical distribution (87 traces) shows that when users ask "status" in this operational phase, they want `cbu.get-status-with-blockers` (41%) or `kyc.get-status` (29%) or `cbu.get-status` (18%). Boosted: `cbu.get-status-with-blockers` (+0.15), `kyc.get-status` (+0.08).
- Action-category filter: "status" → Status/Query category → confirms `get-status` family

Narrowed to: `[cbu.get-status-with-blockers]` — deterministic resolution via pattern match + action category. Phase 4 skipped.

**Phase 5 — Execution:**
```json
{ "dsl_command": "cbu.get-status-with-blockers cbu-7742",
  "outcome": "Success",
  "side_effects": [] }
```

**Trace outcome:** `ExecutedSuccessfully` — the system correctly inferred that a vague "what's the status?" in a KYCBlocked constellation means "show me what's blocking progress", not just "show me the CBU state." That inference came from the constellation pattern, not from the linguistics alone.

---

## 17. Component Map

| Phase | Component | Repo / Artifact | Status |
|-------|-----------|-----------------|--------|
| 0 | Plane Classification | `observation_plane`, `intent_polarity` | Implemented (96.0% / 84.1%) |
| 0 | Domain Hints | Lexical domain extractor | Implemented (64.8%) |
| 1 | Linguistic Decomposition | NLCI Semantic IR | Implemented |
| 1 | Referential Binding Detection | NLCI parser extension | **New — not yet implemented** |
| 2 | Entity Resolution | NounIndex taxonomy | Designed (ECIR spec) |
| 2 | Constellation Recovery | Constellation Engine + `cbu_structure_links` | Partially implemented |
| 2 | State Legality | StateGraph + entity FSMs | CBU complete, 6 entities remaining |
| 2 | Constellation Predicates | Cross-entity blocking rules | **New — needs formal predicate set** |
| 2 | Verb Taxonomy Classification | Entity-verb / state-verb + coupling | **New — not yet implemented** (v0.3.1) |
| 2 | Situation Signature Computation | Constellation fingerprinting | **New — not yet implemented** (v0.3.1) |
| 3 | ECIR Narrowing | Entity-Centric Intent Resolution | Designed, partially implemented |
| 3 | Action Classifier | 8-category classifier | Designed (ECIR spec) |
| 3 | Verb Taxonomy Filter | State-verb coupling demotion | **New — not yet implemented** (v0.3.1) |
| 3 | Constellation Pattern Matcher | Static catalogue + learned distributions | **New — not yet implemented** (v0.3.1) |
| 4 | Concept Matching | `verb_concepts.yaml` | 3.6% coverage — major gap |
| 4 | Embedding Resolution | BGE embedding index | Implemented |
| 4 | Threshold Policy | Asymmetric confidence model | Implemented |
| 4 | Fallback Escape Hatch | Widened resolution with trace | **New — not yet implemented** |
| 5 | REPL Execution | DSL REPL engine | Implemented |
| 5 | Post-execution constellation diff | Snapshot comparison | **New — not yet implemented** |
| — | Macro Expansion | Macro compiler (M1–M18) | Implemented |
| — | Mid-macro constellation re-check | DAG halt on constellation block | **New — not yet implemented** |
| — | Trace Persistence | `utterance_traces` table | **Not yet implemented** |
| — | Replay Engine | Version-pinned trace replay + diff | **Not yet implemented** |
| — | Gap Report Generator | → `macro_verb_corrections.yaml` | **Not yet implemented** |
| — | Clarification Lineage | Trace tree with TraceKind | **Not yet implemented** |
| — | Loop 3 Aggregation Engine | Situation verb distributions + coupling validation | **New — not yet implemented** (v0.3.1) |
| — | Static Pattern Catalogue | Hand-curated situation → verb mappings | **New — not yet implemented** (v0.3.1) |
| — | Pathway Model Builder | Verb sequence detection per constellation lifecycle | **New — not yet implemented** (v0.3.1) |
| — | Phase 3 Contract Enforcement | Prune/rank/demote verification at Phase 3→4 boundary | **New — not yet implemented** (v0.3.2) |
| — | Fallback Budget Monitor | Error-budget tracking for escape hatch frequency | **New — not yet implemented** (v0.3.2) |
| — | Cross-Entity Plan Compiler | BatchTarget / CrossEntityPlan DAG compilation | **New — not yet implemented** (v0.3.2) |
| — | Exclusion Predicate Engine | Negative constraint parsing and enforcement | **New — not yet implemented** (v0.3.2) |
| — | Replay Narrowing Drift Detector | Phase 3 behaviour change detection across replays | **New — not yet implemented** (v0.3.2) |
| — | Constellation Template DAG (DAG 1) | SemOS constellation metadata — entity types, dependency edges, state propagation | Partially defined (17 constellation maps exist, formal `ConstellationTemplate` struct not yet implemented) (v0.3.2) |
| — | DAG Provenance Tracing | Record which DAG 1 edges justified DAG 2/3 orderings | **New — not yet implemented** (v0.3.2) |
| — | DAG 1 Acyclicity Validator | Cycle detection at template publish time | **New — not yet implemented** (v0.3.2 final) |
| — | DAG 3 Runtime Re-check Engine | Per-node constellation re-evaluation with immutable plan | **New — not yet implemented** (v0.3.2 final) |
| — | DAG Ordering Conflict Detector | User-stated vs DAG 1 prerequisite contradiction detection | **New — not yet implemented** (v0.3.2 final) |
| — | Exclusion Feasibility Checker | Compile-time validation that exclusions don't make plans impossible | **New — not yet implemented** (v0.3.2 final) |

---

## 18. Next Steps

1. **Define constellation predicates formally** — For each entity type, enumerate the cross-entity blocking rules. Start with CBU (most linked entity). Format: `{ verb, blocking_entity_type, blocking_states[], predicate_text, resolution_hint }`. These are SemOS metadata, not application code.

2. **Implement constellation recovery from UUID** — Given any entity UUID, walk `cbu_structure_links` (with status filter and hydration direction convention), load linked entity states, return `ConstellationSnapshot`. This is the foundation for Phase 2.

3. **Build verb taxonomy classification** (v0.3.1) — For each of the ~1,123 verbs, classify as entity-verb or state-verb. For state-verbs, document valid states and cross-entity couplings. Start with CBU domain (highest verb count). This is a SemOS metadata enrichment task.

4. **Build situation signature computation** (v0.3.1) — Implement `SituationSignature` derivation from `ConstellationSnapshot`. Define the `OperationalPhase` mapping rules. Build a seed static pattern catalogue from the 18 macro constellation maps (M1–M18) — each macro already describes a constellation-to-verb-sequence mapping.

5. **Implement `utterance_traces` table** — Schema from §13 with hoisted canonical columns including `situation_signature` and `operational_phase`. JSONB for phase payloads, typed columns for operational fields.

6. **Instrument Phases 0–4** — Emit trace segments at each phase boundary. Phase 5 instrumentation partially exists in the REPL.

7. **Build referential binding detection** — Extend NLCI parser to classify pronoun/implicit/filtered/deictic references. Critical for the new halt reasons.

8. **Close `verb_concepts.yaml` gap** — 3.6% coverage is the single largest hole in Phase 4.

9. **Build replay engine** — Version-pinned replay against current surfaces, regression diff, gate for verb surface changes. Include `PatternInfluencedChange` verdict for Loop 3 monitoring.

10. **Wire Loop 1 to constellation remediation pipeline** — GAP and CONSTELLATION_GAP entries in `macro_verb_corrections.yaml` format.

11. **Build Loop 3 aggregation engine** (v0.3.1) — Batch process: group traces by situation signature, compute verb-frequency distributions, validate coupling rules, detect operational pathways. Output feeds the Phase 3 pattern matcher.

12. **Seed the pattern catalogue from macro maps** (v0.3.1) — The 18 CBU structure macros (M1–M18) already define constellation-to-verb-sequence mappings. Invert these into situation signatures with expected verb sets. This gives Loop 3 a warm start before any trace data accumulates.

13. **Implement Phase 3 contract enforcement** (v0.3.2) — Build the `Phase3Output` struct with explicit eliminated/demoted/retained partitioning. Add verification at the Phase 3→4 boundary that `phase4_candidate_set ⊆ Phase 2 legal_verb_set`. Violation is a bug, not a configuration issue.

14. **Build fallback budget monitoring** (v0.3.2) — Instrument fallback invocations with `fallback_invoked` flag and `FallbackReasonCode`. Build a dashboard query that trends fallback rate over time. Set alerting thresholds: 5% warning, 10% defect, 20% critical.

15. **Build cross-entity plan compiler** (v0.3.2) — Implement `ExecutionShape` classification (Singleton/Batch/CrossEntityPlan). Build exclusion predicate parser for negative constraints. Wire into Phase 2's `EntityResolutionMode::CrossEntity` variant.

16. **Extend replay with narrowing drift detection** (v0.3.2) — Add `ReplayNarrowingDiff` to replay output. Flag `FallbackNewlyRequired` and `CandidateSetExpandedUnexpectedly` as investigation triggers alongside existing regression verdicts.

17. **Formalise Constellation Template DAG (DAG 1)** (v0.3.2) — Implement the `ConstellationTemplate` struct with `EntityTypeNode`, `DependencyEdge`, `PrerequisiteKind`, and `StatePropagationRule`. Seed from the 17 existing constellation maps (LU, IE, UK, US, cross-border). This becomes the single source of truth from which macro ordering, cross-entity plan inference, constellation predicate evaluation, and situation signature computation all derive.

18. **Wire DAG provenance into traces** (v0.3.2) — Add `DagProvenance` to `EntityTrace` (Phase 2) and `dag1_edges_used` to `MacroContext`. Every inferred dependency edge in DAG 2 or DAG 3 must reference the DAG 1 edge that justifies it. This makes ordering decisions auditable.

19. **Validate macro compiler against DAG 1** (v0.3.2) — Retrofit existing M1–M18 macro expansions to verify that every DAG 2 dependency edge traces to a DAG 1 prerequisite edge or an intra-entity verb ordering rule. Any edge that has no DAG 1 justification is a candidate for removal or for a new DAG 1 edge to be added.

20. **Implement DAG 1 acyclicity validation** (v0.3.2 final) — Add `validate_template_acyclicity()` to the constellation template publish pipeline. Reject any template where a new prerequisite edge introduces a cycle. Report the specific cycle path. This is a schema-level constraint, not a runtime check.

21. **Build DAG 3 runtime re-check engine** (v0.3.2 final) — Implement the compile-once, re-check-per-node execution model. DAG 3 is immutable after compilation; each node is re-checked against current constellation state before firing. Plan halts on `MidPlanConstellationBlock` with partial execution trace.

22. **Build DAG ordering conflict detector** (v0.3.2 final) — When the cross-entity plan compiler receives user-stated ordering, validate it against DAG 1 prerequisite edges. Contradictions trigger `DagOrderingConflict` halt with suggested correction. Silent plan reshaping is forbidden.

23. **Build exclusion feasibility checker** (v0.3.2 final) — Three-point enforcement: compile-time node rejection, compile-time side-effect prediction via state propagation rules, runtime post-node exclusion check. `ExclusionMakesPlanInfeasible` when exclusions contradict DAG 1 prerequisites.

---

*"While others are trying to make LLMs smarter, we are making the execution environment more rigid and the intent path more visible."*

*The v0.3.2 final refinement: the constellation template is not a diagram — it is the schema. The three DAGs are not three designs — they are one source of truth and two derived projections. Plans are immutable once compiled but re-checked for safety before every node fires. User ordering that contradicts the structural DAG is refused, not silently reshaped. Exclusions that make plans impossible are caught at compile time, not discovered at runtime. The system is now hard to misunderstand, hard to mis-implement, and hard to break.*
