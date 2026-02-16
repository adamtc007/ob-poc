# Wire `process_utterance()` — Implementation TODO

**Companion to:** `PACKS_MACROS_RUNBOOK_ARCH_PAPER_v0.5.md` (spec)  
**Depends on:** `PACKS_MACROS_RUNBOOK_TODO.md` phases P0–P5 (completed)  
**Consumer:** Claude Code  
**Date:** 2026‑02‑15

---

## Context: What exists, what's missing

### Already implemented (P0–P5 completion)

| Component | Location | Status |
|---|---|---|
| `CompiledRunbook`, `CompiledRunbookId`, `RunbookStatus` | `runbook/types.rs` | ✅ |
| `OrchestratorResponse` (3 variants) | `runbook/types.rs` | ✅ |
| `ReplayEnvelope`, `MacroExpansionAudit` | `runbook/types.rs` | ✅ |
| `execute_runbook()` with entity UUID pre-lock set | `runbook/mod.rs` | ✅ |
| `compile_verb()` — DSL → compiled runbook | `runbook/mod.rs` | ✅ |
| `VerbClassifier` (primitive / macro / unknown) | `plan_builder/` | ✅ |
| `PackConstraintGate` (post-expansion validation) | `plan_builder/` | ✅ |
| `PackManager` (lifecycle, event projection, intersection) | pack manager module | ✅ |
| Macro expansion (compiler semantics) | `dsl_v2/macros/` | ✅ |
| `process_utterance()` **stub** | `runbook/mod.rs:93` | ⚠️ Stub only |

### Already implemented (pre-existing pipeline)

| Component | Location | Status |
|---|---|---|
| `IntentService` (6 phases) | `mcp/` or `api/` | ✅ Working |
| `HybridVerbSearcher` (multi-channel evidence) | `mcp/verb_search.rs` | ✅ Working |
| `ArgExtractor` (LLM-based) | `mcp/` | ✅ Working |
| `ScopeResolver` (client group fuzzy match) | `mcp/` or `api/` | ✅ Working |
| `SentenceGenerator` (template-based, no LLM) | `mcp/` | ✅ Working |
| `RuntimeRegistry` (verb defs, arg schemas) | shared | ✅ Working |
| Entity resolution (`EntityArgResolver`) | shared | ✅ Working |
| Session state as runbook fold | session module | ✅ Architectural decision |

### The gap

`process_utterance()` is a stub that reserves the API surface (→ SPEC §11.2). The existing `IntentService` phases produce intermediate outcomes (`ScopeOutcome`, `VerbMatchOutcome`, `ArgExtractionOutcome`) but don't feed into the typed `OrchestratorResponse` contract or the compilation pipeline.

**This TODO wires them together.**

---

## Architecture of the wiring

```
process_utterance(session_id, utterance)
  │
  ├─ Phase 0: ScopeResolver
  │    └─ if no scope set → return ClarificationRequest("Which client group?")
  │    └─ if scope phrase → resolve, set scope, return ClarificationRequest("What do you want to do?")
  │    └─ if scope already set → continue
  │
  ├─ Phase 1: PackManager.effective_constraints()
  │    └─ read active pack constraints (may be unconstrained)
  │
  ├─ Phase 2: HybridVerbSearcher.search() (pack-constrained)
  │    └─ filter candidates by pack allowed_verbs if constrained
  │    └─ returns ranked VerbSearchResult[] with evidence
  │
  ├─ Phase 3: VerbClassifier.classify(top_verb)
  │    ├─ Primitive → Phase 4a
  │    ├─ Macro → Phase 4b
  │    └─ Unknown → return ClarificationRequest("I didn't understand...")
  │
  ├─ Phase 4a (primitive): ArgExtractor.extract_args(verb, utterance, session)
  │    └─ if missing required args → return ClarificationRequest
  │    └─ if complete → assemble single DSL statement → Phase 5
  │
  ├─ Phase 4b (macro): extract args from MacroSchema
  │    └─ if missing required params → return ClarificationRequest
  │    └─ if complete → expand_macro(schema, params, session_snapshot)
  │    └─ expanded DSL steps → Phase 5
  │
  ├─ Phase 5: PackConstraintGate.check(candidate_verbs, effective_constraints)
  │    └─ if rejected → return ConstraintViolation
  │    └─ if passed → Phase 6
  │
  ├─ Phase 6: Validate / Lint / SemReg / DAG-toposort
  │    └─ if invalid → return CompilationError (surfaced as agent message)
  │    └─ if valid → Phase 7
  │
  ├─ Phase 7: Persist CompiledRunbook + ReplayEnvelope
  │    └─ assign monotonic runbook_version
  │    └─ store envelope (session cursor, entity bindings, external lookups, macro audit)
  │    └─ return OrchestratorResponse::Compiled { id, version, envelope_refs, preview }
  │
  └─ (caller then optionally calls execute_runbook with the compiled_runbook_id)
```

→ SPEC §5.2 (happy path), §5.3 (rejection paths), §11.2 (surfaces), §11.3 (response variants)

---

## Phase W0 — Scaffolding: `ProcessUtteranceService`

**Goal:** Create the orchestrator struct that owns the dependencies and replaces the stub.

### W0.1 — Define the service

Create or extend the appropriate module (locate where `process_utterance` stub lives):

```rust
/// The top-of-funnel orchestrator.
/// Owns all dependencies needed to go from utterance → OrchestratorResponse.
/// → SPEC §11.2
pub struct ProcessUtteranceService {
    // Pre-existing components (inject, don't reconstruct)
    scope_resolver: Arc<ScopeResolver>,
    verb_searcher: Arc<HybridVerbSearcher>,
    arg_extractor: Arc<ArgExtractor>,
    sentence_generator: Arc<SentenceGenerator>,
    entity_resolver: Arc<EntityArgResolver>,
    runtime_registry: Arc<RuntimeRegistry>,

    // New components from P0–P5
    verb_classifier: Arc<VerbClassifier>,
    pack_manager: Arc<RwLock<PackManager>>,      // session-scoped, mutable
    macro_registry: Arc<MacroRegistry>,           // single canonical registry
    constraint_gate: Arc<PackConstraintGate>,

    // Infrastructure
    pool: PgPool,
}
```

### W0.2 — Constructor / factory

The service needs to be constructable from `AgentState` (or wherever the existing dependencies live). Find the current construction point for `IntentService` / `HybridVerbSearcher` and extend it:

```rust
impl ProcessUtteranceService {
    pub fn from_agent_state(
        state: &AgentState,
        session_pack_manager: Arc<RwLock<PackManager>>,
    ) -> Self {
        Self {
            scope_resolver: state.scope_resolver.clone(),
            verb_searcher: state.verb_searcher.clone(),  // or VerbSearcherFactory::build(...)
            arg_extractor: state.arg_extractor.clone(),
            sentence_generator: state.sentence_generator.clone(),
            entity_resolver: state.entity_resolver.clone(),
            runtime_registry: state.runtime_registry.clone(),
            verb_classifier: Arc::new(VerbClassifier::new(
                state.runtime_registry.clone(),
                state.macro_registry.clone(),
            )),
            pack_manager: session_pack_manager,
            macro_registry: state.macro_registry.clone(),
            constraint_gate: Arc::new(PackConstraintGate::new()),
            pool: state.pool.clone(),
        }
    }
}
```

**Important:** Find the actual field names by inspecting `AgentState` or equivalent. The names above are illustrative. The *dependencies* are normative.

### W0.3 — Implement the main method signature

Replace the stub:

```rust
impl ProcessUtteranceService {
    /// → SPEC §11.2
    pub async fn process_utterance(
        &self,
        session_id: &SessionId,
        utterance: &str,
    ) -> Result<OrchestratorResponse, PipelineError> {
        // Phases W1–W6 below
        todo!("Wire phases")
    }
}
```

### W0 Acceptance

- [ ] `ProcessUtteranceService` struct exists with all dependencies typed.
- [ ] Factory method constructs it from existing state.
- [ ] `process_utterance` compiles (as `todo!`).

**→ IMMEDIATELY proceed to W1. Progress: ~10%.**

---

## Phase W1 — Scope gate

**Goal:** Enforce that no intent processing happens without a resolved client group scope.

→ SPEC §5.2 (Pack Manager is first in pipeline), existing `ScopeResolver` logic, architectural decision that session state = runbook fold.

### W1.1 — Scope check

```rust
// Inside process_utterance:

// Derive current session state from runbook event log
let session_state = derive_session_state(session_id, &self.pool).await?;

// Phase 0: Is scope set?
if session_state.client_group_id.is_none() {
    // Try to resolve this utterance as a scope phrase
    let scope_outcome = self.scope_resolver
        .try_resolve_scope(utterance, &self.pool)
        .await;

    match scope_outcome {
        ScopeOutcome::Resolved(client_group) => {
            // Compile and execute the scope-setting DSL
            // e.g., (session.load-cluster :client "Allianz Global Investors")
            let scope_dsl = format!(
                "(session.load-cluster :client \"{}\")",
                client_group.name
            );
            // Compile this as a runbook, return it for execution
            // The caller executes it, which sets the scope for subsequent utterances
            return self.compile_single_statement(session_id, &scope_dsl, &session_state).await;
        }
        ScopeOutcome::Ambiguous(candidates) => {
            return Ok(OrchestratorResponse::Clarification(ClarificationRequest {
                question: format!(
                    "I found multiple matches: {}. Which client group?",
                    candidates.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", ")
                ),
                missing_fields: vec!["client_group".into()],
                context: serde_json::json!({ "candidates": candidates }),
            }));
        }
        ScopeOutcome::NotAScope => {
            return Ok(OrchestratorResponse::Clarification(ClarificationRequest {
                question: "Which client group would you like to work on?".into(),
                missing_fields: vec!["client_group".into()],
                context: serde_json::json!({}),
            }));
        }
    }
}
```

### W1.2 — `derive_session_state` implementation

This already exists as an architectural decision (session state = left fold over executed runbook entries). Locate the existing implementation or implement:

```rust
/// Session state is derived, never stored separately.
/// → Arch decision from REPL refactoring paper.
pub async fn derive_session_state(
    session_id: &SessionId,
    pool: &PgPool,
) -> Result<DerivedSessionState, PipelineError> {
    let entries = load_executed_runbook_entries(session_id, pool).await?;
    let mut state = DerivedSessionState::default();

    for entry in &entries {
        match entry.verb.as_str() {
            "session.load-cluster" => {
                state.client_group_id = entry.resolved_arg("client-id");
                state.client_group_name = entry.resolved_arg("client-name");
            }
            "session.set-cbu" => {
                state.active_cbu_id = entry.resolved_arg("cbu-id");
            }
            "pack.select" => {
                state.active_pack = entry.resolved_arg("pack");
            }
            _ => {}
        }
    }

    Ok(state)
}
```

### W1 Acceptance

- [ ] Utterance without scope → `ClarificationRequest("Which client group?")`.
- [ ] Scope phrase → compiled runbook with `session.load-cluster`.
- [ ] Ambiguous scope → clarification with candidates.
- [ ] Subsequent utterances with scope set → proceed to W2.

**→ IMMEDIATELY proceed to W2. Progress: ~25%.**

---

## Phase W2 — Verb discovery and classification (pack-constrained)

**Goal:** Find the verb, classify it, respect pack constraints on discovery.

→ SPEC §5.2 (Verb Discovery & Ranking), §7.1 (VerbClassifier), §6.3 (pack constraints)

### W2.1 — Get effective constraints

```rust
// Inside process_utterance, after scope gate passes:

let constraints = self.pack_manager.read().await.effective_constraints();

// If pack conflict (empty intersection), return immediately
if let Some(conflict) = constraints.conflict() {
    return Ok(OrchestratorResponse::ConstraintViolation(
        conflict.into_violation_detail()
    ));
}
```

### W2.2 — Verb search with constraint filtering

```rust
// Use HybridVerbSearcher with domain filter from pack constraints
let domain_filter = constraints.allowed_verbs.as_ref().map(|verbs| {
    // Convert to whatever domain filter HybridVerbSearcher expects
    // This might be a domain string, a verb prefix filter, or a whitelist
    DomainFilter::Whitelist(verbs.clone())
});

let candidates = self.verb_searcher
    .search(utterance, effective_user_id, domain_filter, 5)
    .await?;

if candidates.is_empty() {
    return Ok(OrchestratorResponse::Clarification(ClarificationRequest {
        question: "I didn't find a matching action. Could you rephrase?".into(),
        missing_fields: vec![],
        context: serde_json::json!({ "active_constraints": constraints }),
    }));
}
```

### W2.3 — Classify the top candidate

```rust
let top_verb = &candidates[0].verb;
let classification = self.verb_classifier.classify(top_verb);

match classification {
    VerbClassification::Unknown(v) => {
        return Ok(OrchestratorResponse::Clarification(ClarificationRequest {
            question: format!(
                "I matched '{}' but it's not a recognized action. Did you mean one of: {}?",
                v,
                candidates.iter().skip(1).take(3)
                    .map(|c| c.verb.as_str()).collect::<Vec<_>>().join(", ")
            ),
            missing_fields: vec!["verb".into()],
            context: serde_json::json!({ "candidates": candidates }),
        }));
    }
    VerbClassification::Primitive(_) | VerbClassification::Macro(_) => {
        // Continue to W3
    }
}
```

### W2 Acceptance

- [ ] Pack constraints filter verb discovery results.
- [ ] Empty constraint intersection → `ConstraintViolation`.
- [ ] No verb matches → `ClarificationRequest` with rephrase hint.
- [ ] Unknown verb → `ClarificationRequest` with alternatives.
- [ ] Top candidate classified → proceed to W3.

**→ IMMEDIATELY proceed to W3. Progress: ~40%.**

---

## Phase W3 — Argument extraction (branch on classification)

**Goal:** Extract args for primitive verbs (LLM) or macro params (schema-driven). Return clarification if incomplete.

→ SPEC §4.1 (pack prompts for clarification), §4.2 (macro typed params), §11.3 (ClarificationRequest)

### W3.1 — Primitive verb: LLM arg extraction

```rust
match classification {
    VerbClassification::Primitive(verb_id) => {
        let arg_outcome = self.arg_extractor
            .extract_args(top_verb, utterance, &session_context)
            .await?;

        match arg_outcome {
            ArgExtractionOutcome::Complete(args) => {
                // Resolve entity references
                let resolved = self.entity_resolver
                    .resolve_args(&args, &session_state)
                    .await?;

                // Assemble single DSL statement
                let dsl = assemble_dsl(top_verb, &resolved)?;

                // → W4 (single statement, no macro expansion needed)
                let candidate_steps = vec![dsl];
                return self.compile_and_gate(
                    session_id, candidate_steps, None, &session_state, &constraints
                ).await;
            }
            ArgExtractionOutcome::Incomplete { present, missing } => {
                // Generate clarification using pack prompts if available
                let question = self.generate_clarification(
                    top_verb, &missing, &constraints
                );
                return Ok(OrchestratorResponse::Clarification(ClarificationRequest {
                    question,
                    missing_fields: missing,
                    context: serde_json::json!({ "verb": top_verb, "present": present }),
                }));
            }
        }
    }
    // ... macro branch in W3.2
}
```

### W3.2 — Macro: schema-driven param extraction

```rust
    VerbClassification::Macro(macro_schema) => {
        // Extract params using MacroSchema (not LLM — schema-driven)
        let params = extract_macro_params(
            &macro_schema,
            utterance,
            &session_state,  // for autofill-from
        )?;

        match params {
            MacroParamOutcome::Complete(resolved_params) => {
                // Expand macro (pure function) → SPEC §8.1
                let expansion = expand_macro(
                    &macro_schema,
                    &resolved_params,
                    &session_state.snapshot(),
                )?;

                // → W4 (multiple steps from expansion)
                return self.compile_and_gate(
                    session_id,
                    expansion.steps,
                    Some(expansion.audit),
                    &session_state,
                    &constraints,
                ).await;
            }
            MacroParamOutcome::Incomplete { present, missing } => {
                return Ok(OrchestratorResponse::Clarification(ClarificationRequest {
                    question: format!(
                        "To run {}, I still need: {}",
                        macro_schema.name,
                        missing.join(", ")
                    ),
                    missing_fields: missing,
                    context: serde_json::json!({
                        "macro": macro_schema.name,
                        "present": present,
                    }),
                }));
            }
        }
    }
```

### W3 Acceptance

- [ ] Primitive verb with complete args → candidate DSL statement(s) ready for W4.
- [ ] Primitive verb with missing args → `ClarificationRequest` with specific missing fields.
- [ ] Macro with complete params → expanded DSL steps ready for W4.
- [ ] Macro with missing params → `ClarificationRequest` with macro-specific missing params.
- [ ] Autofill-from values resolved from session state for macros.

**→ IMMEDIATELY proceed to W4. Progress: ~60%.**

---

## Phase W4 — Constraint gate + compilation + persistence

**Goal:** Validate expanded steps against pack constraints, compile, persist, return `CompiledRunbook`.

→ SPEC §7.3 (PackConstraintGate), §8.2 (post-expansion validation), §9.1 (envelope), §11.4 (versioning)

### W4.1 — The `compile_and_gate` method

This is the shared tail of both the primitive and macro branches:

```rust
impl ProcessUtteranceService {
    /// Post-extraction pipeline: gate → validate → compile → persist → respond.
    async fn compile_and_gate(
        &self,
        session_id: &SessionId,
        candidate_steps: Vec<DslStatement>,  // single step or expanded macro
        macro_audit: Option<MacroExpansionAudit>,
        session_state: &DerivedSessionState,
        constraints: &EffectiveConstraints,
    ) -> Result<OrchestratorResponse, PipelineError> {

        // 1. Pack Constraint Gate (→ SPEC §7.3, §8.2)
        let candidate_verbs: Vec<String> = candidate_steps.iter()
            .map(|s| s.verb_name().to_string())
            .collect();

        if let Err(violation) = self.constraint_gate
            .check(&candidate_verbs, constraints)
        {
            return Ok(OrchestratorResponse::ConstraintViolation(violation));
        }

        // 2. Validate / Lint / SemReg
        validate_dsl_steps(&candidate_steps, &self.runtime_registry)?;

        // 3. DAG / Toposort
        let ordered_steps = toposort_steps(candidate_steps)?;

        // 4. Build replay envelope (→ SPEC §9.1)
        let envelope = ReplayEnvelope {
            session_cursor: session_state.event_cursor,
            entity_bindings: session_state.entity_bindings(),
            external_lookups: HashMap::new(),  // populated if external lookups were used
            macro_audit,
        };

        // 5. Assign monotonic version (→ SPEC §11.4)
        let version = next_runbook_version(session_id, &self.pool).await?;

        // 6. Persist compiled runbook
        let compiled = CompiledRunbook {
            id: CompiledRunbookId::new(),
            session_id: session_id.clone(),
            version,
            steps: ordered_steps,
            envelope,
            status: RunbookStatus::Compiled,
            created_at: OffsetDateTime::now_utc(),
        };
        persist_compiled_runbook(&compiled, &self.pool).await?;

        // 7. Generate preview (human-readable step summaries)
        let preview = compiled.steps.iter()
            .map(|s| self.sentence_generator.generate(s.verb_name(), s.args()))
            .collect();

        // 8. Return CompiledRunbook response (→ SPEC §11.3)
        Ok(OrchestratorResponse::Compiled(CompiledRunbookSummary {
            compiled_runbook_id: compiled.id,
            runbook_version: compiled.version,
            envelope_refs: compiled.envelope.refs(),
            preview,
        }))
    }
}
```

### W4.2 — Helper: `compile_single_statement`

Used by W1 for scope-setting DSL:

```rust
async fn compile_single_statement(
    &self,
    session_id: &SessionId,
    dsl: &str,
    session_state: &DerivedSessionState,
) -> Result<OrchestratorResponse, PipelineError> {
    let parsed = parse_dsl_statement(dsl)?;
    // No pack constraints for scope commands (they're pre-pack)
    let unconstrained = EffectiveConstraints::unconstrained();
    self.compile_and_gate(
        session_id, vec![parsed], None, session_state, &unconstrained
    ).await
}
```

### W4.3 — Session event feedback to Pack Manager

After the caller executes the compiled runbook, session events are emitted. Wire the Pack Manager subscription:

```rust
// In execute_runbook (already implemented), after each step completes:
// Emit session event → Pack Manager processes it

// This should already exist from P2. Verify the wiring:
// execute_runbook emits SessionEvent → PackManager.process_event()
// → SPEC §6.2 (Pack Manager watches events and advances gates)
```

### W4 Acceptance

- [ ] Pack constraint gate rejects out-of-scope expanded verbs → `ConstraintViolation`.
- [ ] Valid DSL compiles, persists, returns `CompiledRunbook` with monotonic version.
- [ ] Replay envelope captured with session cursor, entity bindings, macro audit.
- [ ] Preview contains human-readable sentence per step.
- [ ] Scope-setting utterances bypass pack constraints (pre-pack).

**→ IMMEDIATELY proceed to W5. Progress: ~80%.**

---

## Phase W5 — Integration: wire into agent routes / MCP handlers

**Goal:** Replace whatever currently dispatches user messages to use `ProcessUtteranceService.process_utterance()`.

### W5.1 — Find the current dispatch point

```bash
# Find where user chat messages currently enter the system
grep -rn "handle_message\|handle_chat\|agent_chat\|process_message" rust/src/api/ rust/src/mcp/
```

This is likely in `agent_service.rs` or `mcp/handlers/core.rs`. The current flow probably calls `IntentService` phases directly or via an orchestrator method.

### W5.2 — Replace with ProcessUtteranceService

At the identified dispatch point:

```rust
// BEFORE (conceptual — adapt to actual code):
let intent_result = intent_service.match_verb(input, context).await?;
let args = intent_service.extract_args(&intent_result.verb, input, context).await?;
let dsl = intent_service.assemble_dsl(&intent_result.verb, &args).await?;
// ... various ad-hoc handling ...

// AFTER:
let service = ProcessUtteranceService::from_agent_state(&state, session_pack_manager);
let response = service.process_utterance(&session_id, &utterance).await?;

match response {
    OrchestratorResponse::Clarification(c) => {
        // Return clarification to agent/UI
        reply_with_clarification(c)
    }
    OrchestratorResponse::ConstraintViolation(v) => {
        // Return violation explanation to agent/UI
        reply_with_constraint_violation(v)
    }
    OrchestratorResponse::Compiled(summary) => {
        // Option A: Auto-execute (for low-risk verbs per ConfirmPolicy)
        // Option B: Return preview for confirmation, execute on "yes"
        //
        // For now, return the compiled runbook summary.
        // The UI/agent decides whether to call execute_runbook.
        reply_with_compiled_runbook(summary)
    }
}
```

### W5.3 — Handle the confirmation flow

The existing architecture has a sentence playback / confirmation step. This maps to:

1. `process_utterance` returns `OrchestratorResponse::Compiled` with `preview` (sentences).
2. Agent displays: "I'll do: [sentence 1], [sentence 2]. Confirm?"
3. On "yes" → call `execute_runbook(session_id, compiled_runbook_id, None)`.
4. On "no" or edit → discard runbook (it stays `Compiled` but is never executed; or add a `Discarded` status).

This is a UI/orchestrator concern, not a `process_utterance` concern. The contract is clean: compile returns a preview, execute is a separate call.

### W5.4 — MCP tool surface (if applicable)

If MCP tools currently call into `IntentService` directly, they need the same treatment:

```bash
grep -rn "intent_service\|IntentService\|match_verb\|extract_args" rust/src/mcp/
```

Replace with `ProcessUtteranceService` calls. MCP tools should not bypass the pipeline.

### W5 Acceptance

- [ ] Agent chat endpoint calls `ProcessUtteranceService.process_utterance()`.
- [ ] MCP handlers call `ProcessUtteranceService.process_utterance()`.
- [ ] No direct `IntentService` phase calls remain in dispatch/handler code.
- [ ] Clarification responses reach the UI with question + missing fields.
- [ ] Compiled runbook responses reach the UI with preview sentences.
- [ ] Confirmation → `execute_runbook` call with `CompiledRunbookId`.

**→ IMMEDIATELY proceed to W6. Progress: ~92%.**

---

## Phase W6 — End-to-end integration tests

**Goal:** Prove the full path works through the canonical test harness.

### W6.1 — Required tests

| Test | Flow | Expected outcome |
|---|---|---|
| `e2e_no_scope` | utterance without scope set | `ClarificationRequest("Which client group?")` |
| `e2e_scope_resolve` | "Allianz" as first utterance | `CompiledRunbook` with `session.load-cluster` |
| `e2e_scope_ambiguous` | "Fund" (matches multiple) | `ClarificationRequest` with candidates |
| `e2e_primitive_verb` | "create a new CBU called Acme" (scope set) | `CompiledRunbook` with `cbu.create` step |
| `e2e_primitive_missing_args` | "create a CBU" (no name) | `ClarificationRequest` with missing name |
| `e2e_macro_expand` | utterance matching `structure.setup` | `CompiledRunbook` with expanded atomic steps |
| `e2e_macro_missing_params` | `structure.setup` without required param | `ClarificationRequest` with macro params |
| `e2e_pack_constraint_reject` | macro expansion violates active pack | `ConstraintViolation` with remediation |
| `e2e_compile_then_execute` | full path → compile → execute | `ExecutionResult` with session events |
| `e2e_session_state_derived` | execute scope → execute verb → derive state | State reflects both executed entries |

### W6.2 — All tests use the canonical harness

Every test above must go through:
```
process_utterance() → OrchestratorResponse → (optionally) execute_runbook()
```

No test calls `IntentService` phases, `HybridVerbSearcher`, or `ArgExtractor` directly for integration testing.

### W6 Acceptance

- [ ] All 10 tests pass.
- [ ] Tests use canonical harness (no internal type construction).
- [ ] Full utterance-to-execution path works for primitive verbs.
- [ ] Full utterance-to-execution path works for macros.
- [ ] Session state derivation is consistent after execution.

**→ W6 DONE. Progress: 100%. `process_utterance` is wired.**

---

## Dependency notes for Claude Code

1. **Find the actual types and field names** before writing code. The Rust signatures in this TODO are illustrative. Run:
   ```bash
   grep -rn "struct AgentState\|struct IntentService\|struct HybridVerbSearcher" rust/src/
   grep -rn "fn process_utterance\|fn handle_message\|fn agent_chat" rust/src/
   ```
   Adapt all code to match the real codebase.

2. **IntentService phases are reused, not rewritten.** `ScopeResolver`, `HybridVerbSearcher`, `ArgExtractor`, `SentenceGenerator`, `EntityArgResolver` are working components. `ProcessUtteranceService` composes them; it does not replace them.

3. **The stub at `runbook/mod.rs:93`** is the replacement target. The new `ProcessUtteranceService` either replaces the stub in-place or is called by it.

4. **Session state derivation may already exist** from the REPL refactoring. Search for `derive_session_state` or the left-fold pattern over runbook entries. Reuse if found.

5. **Pack Manager is session-scoped.** Each session gets its own `PackManager` instance (or a session-keyed projection). The `Arc<RwLock<PackManager>>` is per-session, not global.

6. **Continuation gates:** After each phase, emit progress and immediately continue. Do not stop between phases.
