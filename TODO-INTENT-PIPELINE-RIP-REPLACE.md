# Intent Pipeline: Rip & Replace

> **Status:** DRAFT - For GPT/Opus review
> **Date:** 2026-01-27
> **Problem:** Intent extraction is voodoo. 82% confidence is not good enough for regulated operations.
> **Solution:** Replace probabilistic intent guessing with deterministic context + confirmation.

---

## Why Not AI Intent Extraction?

The AI approach **looks** impressive in demos:
- "Just say anything naturally!"
- "It understands context!"
- "82% accuracy!"

But in production with regulated financial operations:
- **82% = 18% wrong** = unacceptable
- Users don't trust it ("did it understand me?")
- Edge cases everywhere
- Debugging nightmares ("why did it do that?")
- Non-deterministic = untestable

**The people selling AI intent extraction aren't the ones dealing with the consequences.**

### Where AI IS Good

| Use Case | Why It Works |
|----------|--------------|
| Speech-to-text | Commodity, good enough, not critical if slightly wrong |
| Summarization | Nice to have, human reviews anyway |
| Suggestions | Optional, user decides to accept or not |
| Research/exploration | Low stakes, discovery mode |

### Where AI Should NOT Be Used

| Use Case | Why It Fails |
|----------|--------------|
| "What did the user mean?" | **Just ask them** |
| "Which entity?" | **Show options, let them pick** |
| "Is this the right verb?" | **Confirm before executing** |
| "Execute this financial operation" | **User must explicitly approve** |

**The smartest thing is knowing when NOT to use AI.**

---

## The Problem

Current flow relies on ML/LLM to "understand" user intent:

```
User: "load the book"
         │
         ▼
   ┌─────────────────┐
   │  INTENT VOODOO  │
   │                 │
   │  Embeddings     │
   │  Semantic search│
   │  LLM extraction │
   │  Confidence: 82%│
   │                 │
   │  Maybe right?   │
   │  Maybe wrong?   │
   │  Who knows!     │
   └─────────────────┘
         │
         ▼
   Hope for the best
```

**Why this fails:**
- 82% confidence = 18% wrong = unacceptable for financial ops
- Typos break embeddings ("allainz" ≠ "allianz" semantically)
- Ambiguous phrases can't be resolved without context
- LLM calls add latency and cost
- Non-deterministic = hard to debug, test, trust

---

## The New Approach

**Don't guess intent. Narrow the space and confirm.**

### Client First - The Key Constraint

**This is the most important insight: establish client scope BEFORE anything else.**

```
Without client context:
  "load the fund" → search ALL funds → thousands → hopeless
  "show kyc cases" → search ALL cases → chaos

With client context (Allianz):
  "load the fund" → search Allianz funds → 12 matches → manageable
  "load lux fund" → search Allianz LU funds → 2 matches → easy
  "show kyc cases" → search Allianz cases → 5 active → done
```

Client scope dramatically reduces the search space. Instead of matching against the entire universe, we're matching against a bounded set. Phonetic matching becomes viable because the candidate pool is small.

### Two Context Questions Upfront

```
┌─────────────────────────────────────────────────────────────┐
│  SESSION START                                              │
│                                                             │
│  1. "Which client are you working on?"    ← ALWAYS FIRST   │
│     → Allianz / BlackRock / Aviva / [search]               │
│     → Sets SCOPE (all entity resolution within this)        │
│     → This is NON-NEGOTIABLE - must be set before work     │
│                                                             │
│  2. "What are you doing?"                                   │
│     → KYC & Onboarding                                      │
│     → CBU Structure & Maintenance                           │
│     → Trading Setup                                         │
│     → Custody & Settlement                                  │
│     → Sets PERSONA (filters available verbs)               │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**Once context is set, input becomes constrained:**
- Entity resolution searches within client scope (not global)
- Verb matching filters to persona's domain
- Ambiguity dramatically reduced
- Phonetic matching actually works (small candidate pool)

### Voice-First, Not Chat-First

Users are SMEs speaking, not typing. Speech-to-text produces phonetic errors:

```
Spoken: "Allianz"
Transcribed: "allainz" / "alianz" / "all E ants"
```

**Solution:** Phonetic matching (dmetaphone) against known values.
- `dmetaphone("allainz")` = `dmetaphone("allianz")` = `ALNS`
- Deterministic, fast, no ML required

### Verb Taxonomy Tree - Semantic Structured Picker

Instead of a flat list of verbs or a chat "what do you want?", the verb taxonomy is a **navigable tree filtered by selection**.

**Selection drives available verbs:**

The verb's arg types determine when it's available:

| Selection Level | Available Verb Args | Example Verbs |
|-----------------|---------------------|---------------|
| Client only | `:client` | `session.load-cluster`, `contract.create`, `cbu.create` |
| Client + CBU | `:cbu-id` | `cbu.assign-role`, `trading-profile.create`, `kyc.create` |
| Client + CBU + Entity | `:entity-id` | `entity.update`, `ubo.discover`, `document.solicit` |

**The tree UI:**

```
┌─────────────────────────────────────────────────────────────┐
│  VERB TREE (filtered by: Allianz > Lux Fund I)             │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ▼ STRUCTURE                                                │
│    ├─● cbu.assign-role      "add entity to this cbu"       │
│    ├─○ cbu.remove-role      "remove entity"                │
│    └─○ cbu.update           "update cbu details"           │
│                                                             │
│  ▼ KYC & COMPLIANCE                                         │
│    ├─● kyc.create           "start kyc case"               │
│    ├─○ kyc.list             "show cases"                   │
│    └─○ ubo.discover         "find owners"                  │
│                                                             │
│  ▼ TRADING                                                  │
│    ├─● trading-profile.create  "setup trading"             │
│    ├─○ isda.create          "add isda agreement"           │
│    └─○ counterparty.add     "add counterparty"             │
│                                                             │
│  ▼ CUSTODY                                                  │
│    ├─● custody.create-account  "open custody account"      │
│    └─○ ssi.create           "add settlement instructions"  │
│                                                             │
│  ▶ DOCUMENTS (collapsed)                                    │
│  ▶ CONTRACTS (collapsed)                                    │
│                                                             │
├─────────────────────────────────────────────────────────────┤
│  ● = ready (prerequisites met)                              │
│  ○ = needs prerequisites (click to see what's missing)     │
└─────────────────────────────────────────────────────────────┘
```

**Pick a node → See what's needed → Fill gaps → Execute:**

```
User clicks: trading-profile.create

┌─────────────────────────────────────────────────────────────┐
│  trading-profile.create                                     │
│  "Create trading profile for Lux Fund I"                   │
│                                                             │
│  Prerequisites:     ✓ CBU exists (Lux Fund I)              │
│                     ✓ Client set (Allianz)                 │
│                                                             │
│  Required args:                                             │
│    :cbu-id         ✓ Lux Fund I (auto-filled from selection)│
│    :products       ? [Select products...]                  │
│                                                             │
│                    [Create] [Cancel]                        │
└─────────────────────────────────────────────────────────────┘
```

**Why tree > list:**
- Semantically grouped (Structure, KYC, Trading, Custody)
- Visually shows what's available vs blocked
- Selection auto-fills args (no redundant questions)
- Collapsible (don't overwhelm with 500 verbs)
- Learnable (users build mental model of domain)

**Why tree > chat:**
- No ambiguity ("what do you mean?")
- No guessing (user explicitly picks)
- Deterministic (same click = same result)
- Discoverable (browse what's possible)

**The tree structure IS the meaning.**

### The DAG is the UI

User states an outcome. System shows what's needed:

```
User: "Subscribe Allianz Lux Fund to custody products"

System analyzes dependencies:
  - CBU "Allianz Lux Fund" → doesn't exist
  - Contract for Allianz + CUSTODY → doesn't exist
  - Client "Allianz" → exists ✓

System shows DAG visually:

         ┌─────────────────┐
         │  Subscribe to   │  ← GOAL
         │  Custody        │
         └────────┬────────┘
                  │
       ┌──────────┴──────────┐
       ▼                     ▼
  ┌─────────┐          ┌──────────┐
  │   CBU   │          │ Contract │
  │ ☐ NEED  │          │ ☐ NEED   │
  └────┬────┘          └────┬─────┘
       │                    │
       └────────┬───────────┘
                ▼
          ┌──────────┐
          │  Client  │
          │ ✓ Allianz│
          └──────────┘

User: "yes, set it up"
System: Executes in dependency order
```

### No LLM in the Hot Path

| Component | Old | New |
|-----------|-----|-----|
| Verb discovery | Embeddings + LLM | Context filter + phonetic match |
| Entity resolution | LLM extraction | Phonetic match within scope |
| Arg extraction | LLM JSON | Verb schema drives questions (only if needed) |
| Disambiguation | Confidence threshold | User confirmation |

LLM only used for:
- Initial speech-to-text (Whisper - commodity)
- Edge cases where structured resolution fails

---

## Architecture Changes

### Current Pipeline (Remove)

```
User Input
    │
    ▼
IntentPipeline.process()
    │
    ├─► Scope Resolution (phonetic) ← KEEP
    │
    ├─► Verb Search (embeddings) ← REPLACE
    │       │
    │       └─► Ambiguity check
    │           └─► Disambiguation UI
    │
    ├─► LLM Arg Extraction ← REMOVE
    │
    └─► DSL Assembly
```

### New Pipeline

```
Session Start
    │
    ├─► "Which client?" → Set scope (phonetic match)
    │
    └─► "What are you doing?" → Set persona (verb filter)
    
User Input (within context)
    │
    ▼
Structured Resolution
    │
    ├─► Parse: VERB + ENTITY(s) pattern
    │
    ├─► Verb: Match against persona's verb set (phonetic)
    │
    ├─► Entity: Match within client scope (phonetic)
    │
    ├─► Validate: Verb args satisfied?
    │       │
    │       ├─► Yes → Stage
    │       │
    │       └─► No → Prompt for missing (voice Q&A)
    │
    └─► Show DAG → User confirms → Execute
```

### Key Components

#### 1. Session Context

```rust
pub struct SessionContext {
    /// Active client group (scope for all entity resolution)
    pub client: Option<ClientGroup>,
    
    /// Active persona (filters available verbs)
    pub persona: Persona,
    
    /// Navigation state
    pub loaded_cbus: HashSet<Uuid>,
}

pub enum Persona {
    KycOnboarding,      // kyc.*, ubo.*, entity.*, gleif.*
    CbuStructure,       // cbu.*, session.*, manco.*
    TradingSetup,       // trading.*, isda.*, counterparty.*
    CustodySettlement,  // custody.*, ssi.*, account.*
    All,                // Power user - no filter
}
```

#### 2. Phonetic Resolver (Unified)

```rust
pub struct PhoneticResolver {
    pool: PgPool,
}

impl PhoneticResolver {
    /// Resolve any input against known values
    pub async fn resolve(
        &self,
        input: &str,
        context: &SessionContext,
        expected_type: EntityType,
    ) -> ResolveOutcome {
        // Generate phonetic codes for input
        // Match against appropriate table based on expected_type
        // Filter by context.client scope
        // Return: Resolved | Candidates | NoMatch
    }
}

pub enum EntityType {
    ClientGroup,
    Cbu,
    Entity,
    Product,
    Jurisdiction,
    Verb,
}
```

#### 3. DAG Analyzer

```rust
pub struct DagAnalyzer {
    verb_registry: VerbRegistry,
}

impl DagAnalyzer {
    /// Given a goal verb+args, compute dependency tree
    pub fn analyze(&self, goal: &VerbCall) -> DependencyDag {
        // Walk verb dependencies
        // Check what exists vs what's needed
        // Return tree with status per node
    }
    
    /// Generate execution plan from DAG
    pub fn plan(&self, dag: &DependencyDag) -> Vec<StagedCommand> {
        // Topological sort
        // Return ordered list of commands
    }
}
```

#### 4. Visual DAG Component (egui)

```rust
pub struct DagView {
    dag: DependencyDag,
    selected_node: Option<NodeId>,
}

impl DagView {
    pub fn ui(&mut self, ui: &mut Ui) -> Option<DagAction> {
        // Render nodes with status colors
        // ✓ green = exists
        // ☐ yellow = staged
        // ✗ red = blocked
        // Show connections
        // Handle click/hover
    }
}
```

---

## Migration Path

### Phase 1: Context Setting
- [ ] Add `SessionContext` with client + persona
- [ ] UI: Client picker at session start
- [ ] UI: Persona selector (or infer from first action)
- [ ] Wire context through to entity resolution

### Phase 2: Phonetic Resolution
- [ ] Unify phonetic matching (already started - scope_resolution.rs)
- [ ] Add phonetic index to all resolvable tables
- [ ] Create `PhoneticResolver` service
- [ ] Replace embedding-based entity lookup

### Phase 3: Verb Resolution
- [ ] Filter verbs by persona
- [ ] Phonetic match verb names (not just patterns)
- [ ] Remove LLM from verb discovery path
- [ ] Keep verb search for "discovery" mode only

### Phase 4: DAG Visualization
- [ ] Implement `DagAnalyzer` 
- [ ] Create `DagView` egui component
- [ ] Show dependencies before execution
- [ ] User confirms or edits

### Phase 5: Remove LLM Hot Path
- [ ] Remove `IntentPipeline.process_as_natural_language()`
- [ ] Remove embedding-based verb search as primary path
- [ ] Keep semantic search for "I don't know what I want" fallback
- [ ] Measure latency improvement

---

## Success Criteria

| Metric | Current | Target |
|--------|---------|--------|
| Confidence on resolution | 82% (guessed) | 100% (confirmed) |
| Latency (voice to staged) | 500-1000ms | <200ms |
| LLM calls per action | 1-2 | 0 (normal path) |
| Typo handling | Fragile (embeddings) | Robust (phonetic) |
| User trust | "Did it understand me?" | "I told it what to do" |

---

## Open Questions

1. **Persona inference** - Should we infer from first action or always ask?

2. **Power user mode** - SMEs who know the DSL want to skip the guardrails. How?

3. **Fallback to LLM** - When structured resolution fails, do we fall back or ask for clarification?

4. **Learning** - How do we capture when phonetic match is wrong to improve?

5. **Multi-intent** - "Load allianz and run kyc" - handle sequentially or parse as compound?

---

## References

- `ai-thoughts/035-repl-session-implementation-plan.md` - Runbook/staging model
- `rust/src/mcp/scope_resolution.rs` - Phonetic matching (already implemented)
- `rust/src/mcp/intent_pipeline.rs` - Current pipeline (to be replaced)
- `rust/src/repl/dag_analyzer.rs` - DAG ordering (exists for execution)
