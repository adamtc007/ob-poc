# Session Verb Surface — Call Stack Trace & Issue Diagnostic

**Date:** 1 March 2026  
**Repo:** `ob-poc` commit head  
**Scope:** Server-side verb surface → utterance-to-intent resolution → UI wiring

---

## 1. Complete Call Stack: Utterance → Verb Execution

```
User types in chat UI (ChatPage.tsx or SemOsPage.tsx)
  │
  ├─ handleSend() → sendMutation.mutate(message)
  │    └─ chatApi.sendMessage(sessionId, { message })
  │         └─ POST /api/session/:id/chat
  │
  └─ chat_session() handler (agent_routes.rs:2464)
       │
       ├─ AgentService::process_chat()
       │    │
       │    ├─ [1] "run"/"execute" check → execute staged runbook
       │    ├─ [2] Pending disambiguation check (numeric selection)
       │    └─ [3] orchestrator::handle_utterance(ctx, utterance)
       │
       └─ handle_utterance() (orchestrator.rs:179)
            │
            ├─ Step 1: Entity Linking
            │    └─ LookupService.analyze(utterance, 5)
            │         → dominant_entity_kind, entity_candidates
            │
            ├─ Step 2: SemReg Context Resolution
            │    └─ resolve_sem_reg_verbs(ctx, entity_kind)
            │         ├─ via SemOsClient (DI boundary) if available
            │         │    └─ client.resolve_context(principal, request)
            │         │         └─ CCIR 9-step pipeline:
            │         │              1. Subject match
            │         │              2. Tier filter
            │         │              3. Trust class
            │         │              4. Taxonomy membership
            │         │              5. ABAC policy evaluation ← KEY STEP
            │         │              6. Evidence mode
            │         │              7. Precondition check
            │         │              8. Rank scoring (ViewDef boost)
            │         │              9. Fingerprint computation
            │         └─ → ContextEnvelope {allowed_verbs, pruned_verbs, fingerprint}
            │
            ├─ Step 2.5: Compute SessionVerbSurface
            │    └─ compute_session_verb_surface(ctx) (verb_surface.rs:158)
            │         ├─ Layer 1: RuntimeVerbRegistry (642-653 verbs)
            │         ├─ Layer 2: AgentMode filter (Research vs Governed)
            │         ├─ Layer 3: Workflow phase filter (stage_focus → domain allowlist)
            │         ├─ Layer 4: SemReg CCIR (envelope.is_allowed)
            │         ├─ Layer 5: Lifecycle state (entity_state = None ← DEFERRED)
            │         ├─ Layer 6: Actor gating (passthrough)
            │         ├─ Layer 7: FailPolicy (FailClosed → safe-harbor only)
            │         └─ Layer 8: Rank, group, dual fingerprint
            │              → SessionVerbSurface {verbs, excluded, surface_fingerprint}
            │
            ├─ Step 3: Intent Pipeline (IntentPipeline.process_with_scope)
            │    │
            │    ├─ Pre-constraint: with_allowed_verbs(surface.allowed_fqns())
            │    │
            │    ├─ Stage 0: Scope Resolution (HARD GATE)
            │    │    └─ scope_resolver.resolve() — client name matching
            │    │         ├─ Resolved → return early (no verb search)
            │    │         ├─ Candidates → return picker (no verb search)
            │    │         └─ Unresolved/NotScope → continue to verb discovery
            │    │
            │    └─ process_as_natural_language()
            │         │
            │         ├─ HybridVerbSearcher.search() — 8-TIER SEARCH:
            │         │    ├─ Tier 0: Operator macros (MacroRegistry)  ← HIGHEST PRIORITY
            │         │    ├─ Tier 0.5: Lexicon (LexiconService)
            │         │    │    ├─ Exact label match (score 1.0)
            │         │    │    └─ Token overlap match (score <1.0)
            │         │    ├─ Tier 1: User learned exact (DB)
            │         │    ├─ Tier 2: Global learned exact (in-memory)
            │         │    ├─ Tier 3: User semantic (pgvector, user_phrase_embeddings)
            │         │    ├─ Tier 5: Blocklist collision check
            │         │    ├─ Tier 6: Global semantic (verb_pattern_embeddings)
            │         │    │    └─ BGE-small-en-v1.5, 384-dim, asymmetric mode
            │         │    │    └─ SOURCE: v_verb_intent_patterns VIEW
            │         │    │         ├─ yaml_intent_patterns (from YAML invocation_phrases)
            │         │    │         └─ intent_patterns (learned from feedback)
            │         │    └─ Tier 7: Phonetic fallback (dmetaphone)
            │         │
            │         ├─ POST-FILTER: allowed_verbs.retain() ← SEMREG CONSTRAINT
            │         │
            │         ├─ Ambiguity check (margin < 0.05 → NeedsClarification)
            │         │
            │         └─ LLM DSL generation (if clear match above threshold 0.65)
            │
            ├─ Step 4: Post-filter safety net + AgentMode gating
            │
            └─ Step 5: DSL validation + execution staging
```

---

## 2. Diagnostic: Three Questions Answered

### Q1: Are all DSL verbs defined and visible to the agent?

**Partially. 52.4% coverage.**

| Metric | Count |
|--------|-------|
| Verbs in RuntimeVerbRegistry (YAML parsed) | ~653 |
| Verbs WITH invocation_phrases | 342 (52.4%) |
| Verbs WITHOUT invocation_phrases | 311 (47.6%) |
| Draft phrases files (not yet merged) | 2 files, 2272 lines |
| YAML verb domain files | 37 files |

**What "invisible" means for the 311 verbs without phrases:**

1. No `yaml_intent_patterns` in `dsl_verbs` table
2. No row in `v_verb_intent_patterns` view (the UNION of yaml + learned)
3. No embedding in `verb_pattern_embeddings`
4. **Therefore invisible to Tier 6 semantic search** (the primary discovery path)
5. Only discoverable via: Lexicon exact match (Tier 0.5), or if someone previously taught the system via the learning loop

**Files containing pending phrases:**
- `_invocation_phrases_draft.yaml` (945 lines)
- `_invocation_phrases_extension.yaml` (1327 lines)

These are NOT loaded by the VerbSyncService because they use the `_` prefix convention. They need to be merged into the domain YAML files.

### Q2: Are verbs linked to data (entities)?

**Yes, structurally. But lifecycle filtering is DEFERRED (always `None`).**

Verb-to-entity linkage exists at three levels:

**a) YAML definition level:**
- `produces.produced_type`: e.g., `entity`, `cbu`, `fund`
- `consumes[].consumed_type`: required input entity types
- `lifecycle.entity_arg`: which arg references the entity being transitioned
- `lifecycle.requires_states`: e.g., `["draft", "open"]`
- `lifecycle.transitions_to`: e.g., `"approved"`
- `args[].lookup.entity_type`: entity gateway for arg resolution

**b) SemReg CCIR level:**
- Subject matching (entity-kind constraints on verb contracts)
- Taxonomy membership (fund-vehicle taxonomy, etc.)
- Relationship-based filtering
- ViewDef domain/entity-kind prominence boosts

**c) SessionVerbSurface level:**
- Layer 5 (Lifecycle state): **exists in code but `entity_state` is ALWAYS `None`**
  - `orchestrator.rs:228`: `entity_state: None, // Lifecycle filtering deferred to Phase 3`
  - This means a verb requiring entity state "approved" is never filtered out at surface time
  - It fails only at EXECUTION time, which is the "try → fail → retry" anti-pattern the architecture doc explicitly calls out

### Q3: How are utterance phrases linked to verb invocation?

**Multi-tier search with a critical gap in the SemReg-to-surface wiring.**

The utterance-to-verb pipeline uses 8 search tiers (detailed in call stack above). The chain is:

```
YAML invocation_phrases
  → VerbSyncService.sync_invocation_phrases()
    → dsl_verbs.yaml_intent_patterns column
      → v_verb_intent_patterns VIEW (UNION with learned patterns)
        → populate_embeddings binary
          → verb_pattern_embeddings table (BGE 384-dim vectors)
            → HybridVerbSearcher.search_global_semantic_with_embedding()
              → Tier 6 results
                → allowed_verbs POST-FILTER (SemReg constraint)
                  → Ambiguity check
                    → LLM DSL generation
```

---

## 3. Root Cause Analysis: Why Verbs Can't Be Changed or Executed

### BUG 1: GET `/api/session/:id/verb-surface` always returns ~30 verbs (CRITICAL)

**File:** `agent_routes.rs:4164`

```rust
// Build context — use unavailable envelope since we don't have a live SemReg
// resolution for a GET request.
let envelope = ContextEnvelope::unavailable();  // ← ALWAYS unavailable
let ctx = VerbSurfaceContext {
    ...
    fail_policy: VerbSurfaceFailPolicy::default(), // ← FailClosed
    ...
};
```

The GET endpoint creates `ContextEnvelope::unavailable()`, so SemReg is always "unavailable". With FailClosed (default), `compute_session_verb_surface` reduces to safe-harbor domains only: `session.*`, `view.*`, `agent.*` — about 30 verbs.

**Impact:** The VerbBrowser, if it were to call this endpoint directly, would only show ~30 verbs. Currently, it uses `availableVerbs` from the zustand store which is populated from `ChatResponse.available_verbs`.

### BUG 2: SemOsPage has NO VerbBrowser component

**File:** `SemOsPage.tsx`

The SemOs page renders:
- Left: Chat messages + input
- Right: `SemOsContextPanel` (registry context)

It does NOT render `VerbBrowser`. The verb profiles ARE stored in the zustand store via `setAvailableVerbs()` on chat response, but nothing renders them in the SemOs UI.

### BUG 3: VerbBrowser click → setInputValue, NOT direct execution

**File:** `VerbBrowser.tsx:284`

```typescript
const handleSelectVerb = (verb: VerbProfile) => {
    setInputValue(verb.sexpr);  // Just fills the input box
};
```

When a user clicks a verb in VerbBrowser, it inserts the s-expr signature (e.g., `(kyc.open-case :entity-id <uuid>)`) into the chat input. The user must then press Enter, which submits through the FULL intent pipeline again. The s-expr text then goes through:

1. Scope resolver → not a scope phrase
2. `infer_domain_from_phrase()` → might infer domain
3. `HybridVerbSearcher.search()` → searches ALL tiers for a match

The s-expr format `(verb.fqn :arg value)` is NOT directly parsed as DSL. The comment in orchestrator.rs says: `// NOTE: Direct DSL early-exit (dsl: prefix) was removed in Phase 0B CCIR.`

So the s-expr has to match via semantic search against `invocation_phrases` embeddings, which may not match the s-expr format at all. This is the core UX breakage: **clicking a verb doesn't deterministically execute it.**

### BUG 4: Verb surface only populated AFTER first chat message

Verb profiles in `ChatResponse.available_verbs` are only populated when the user sends a message (inside `chat_session()` handler). On initial session load via `getSession()`, no verbs are returned. The VerbBrowser stays empty until the first chat round-trip completes.

### BUG 5: 47.6% of verbs unreachable via natural language

311 of 653 verbs have empty `invocation_phrases`. Without phrases:
- No embeddings generated → invisible to Tier 6 semantic search
- Only discoverable if:
  - A macro maps to them (Tier 0) — few macros exist
  - The lexicon has their label (Tier 0.5) — partial coverage
  - Someone previously taught the system (Tier 2) — cold start problem

### BUG 6: Workflow phase filter has no effect on verb SET change

When the user selects a workflow (e.g., "KYC"), `session.context.stage_focus` is set to `"semos-kyc"`. This correctly filters domains in `compute_session_verb_surface` Layer 3. However:

- The `ChatResponse.available_verbs` is only re-computed on the NEXT chat message
- There's no mechanism to push an updated verb set to the UI on workflow change
- The VerbBrowser doesn't call `GET /verb-surface` on fingerprint changes (and even if it did, Bug 1 would return only 30 verbs)

---

## 4. Fix Priority Map

| # | Bug | Severity | Fix |
|---|-----|----------|-----|
| 1 | GET verb-surface uses `ContextEnvelope::unavailable()` | **P0** | Call `resolve_sem_reg_verbs()` with session context instead of `unavailable()`. Cache the envelope in session state. |
| 2 | SemOsPage missing VerbBrowser | **P0** | Add `<VerbBrowser />` to SemOsPage right panel |
| 3 | Verb click → setInputValue, not execution | **P0** | Add a direct verb invocation path: `POST /api/session/:id/chat` with `{ message, forced_verb_fqn }` — use `handle_utterance_with_forced_verb()` which already exists |
| 4 | Verbs only populate after first message | **P1** | Call `GET /verb-surface` on session load, or include `available_verbs` in `getSession()` response |
| 5 | 47.6% verbs missing invocation_phrases | **P1** | Merge `_invocation_phrases_draft.yaml` and `_invocation_phrases_extension.yaml` into domain YAML files; run `populate_embeddings` |
| 6 | No verb surface push on workflow change | **P1** | Emit `available_verbs` in the decision-reply response when workflow is selected |

---

## 5. Existing Test Coverage

**Unit tests** (`verb_surface.rs`): 10 tests covering:
- Fingerprint determinism and format
- SI-1 FailClosed safe-harbor behavior
- SI-2 Dual fingerprint distinctness
- SI-3 Multi-reason exclusions
- AgentMode filtering (Research vs Governed)
- Workflow phase filtering
- Progressive narrowing invariant
- SemReg verb filtering
- Convenience methods

**Integration tests** (in `rust/tests/`):
- `verb_search_integration.rs`: Tests HybridVerbSearcher with DB
- `sem_reg_integration.rs`: Tests CCIR pipeline
- `sem_reg_invariants.rs`: Tests governance invariants
- `scope_resolution_integration.rs`: Tests Stage 0 scope resolver
- `repl_v2_*`: Multiple phase tests covering intent pipeline

**Gap:** No integration test covers the full `utterance → verb surface → search → execute` round-trip with a live SemReg. The verb_surface.rs tests use `ContextEnvelope::unavailable()` or `test_with_verbs()` mocks. No test exercises the GET `/verb-surface` endpoint.

---

## 6. Recommended Fix Order (Phase 0: Make It Work)

**Step 1** — Fix Bug 3 first (highest user impact). Wire `VerbBrowser` click to call `POST /api/session/:id/chat` with `forced_verb_fqn` parameter. `handle_utterance_with_forced_verb()` already exists in the orchestrator (line 778) and bypasses the entire search pipeline. The s-expr template with placeholder args can be sent as the message for the LLM to fill in.

**Step 2** — Fix Bug 2. Add `<VerbBrowser />` to SemOsPage. One-line JSX change.

**Step 3** — Fix Bug 1. Replace `ContextEnvelope::unavailable()` in `get_session_verb_surface()` with a real `resolve_sem_reg_verbs()` call using the session's orchestrator context. Use `VerbSurfaceFailPolicy::FailOpen` for the GET endpoint during dev.

**Step 4** — Fix Bug 4. Have the workflow decision reply handler include `available_verbs` in its response.

**Step 5** — Fix Bug 5. Merge the phrase draft files. Run `populate_embeddings`.

→ IMMEDIATELY proceed to implementation if desired. These are all surgical fixes, not architectural changes.
