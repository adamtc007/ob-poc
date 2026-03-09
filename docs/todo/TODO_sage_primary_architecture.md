# Sage → Coder: Asymmetric Threshold Architecture

**Codebase:** `ob-poc` (Rust, `rust/` directory)  
**Build:** `RUSTC_WRAPPER= cargo check -p ob-poc`  
**Test:** `RUSTC_WRAPPER= cargo test --lib -p ob-poc`  

---

## The One Principle

```
┌──────────────────────────────────────────────────────────────────────┐
│                                                                      │
│  SAGE (default)                         CODER (gated)                │
│                                                                      │
│  show · list · describe · trace         create · update · delete     │
│  search · check · count · view          add · remove · import        │
│  find · explore · report · what         assign · publish · approve   │
│  who · where · which · how              submit · reject · flag       │
│                                         set · modify · move · link   │
│  LOW threshold — fire easily            HIGH threshold — prove it    │
│  No confirmation needed                 Always confirm before exec   │
│  No REPL, no DSL generation             REPL + staged DSL + confirm  │
│  Wrong result = user sees stale data    Wrong result = data corrupted│
│                                                                      │
│  ═══════════════════ THE PIVOT ═══════════════════════════════       │
│                                                                      │
│  Sage → Coder handoff requires ALL FOUR:                             │
│    1. Write-polarity clue word detected (deterministic)              │
│    2. Sage summarises the mutation in plain language:                 │
│       "So you want to create a new CBU for Allianz — UK fund?"      │
│    3. User explicitly confirms: "yes" / "go ahead" / "do it"        │
│    4. Coder resolves to a state_write verb (metadata-verified)       │
│                                                                      │
│  If ANY of the four fails → stay in Sage, show context instead      │
│  The user NEVER sees DSL, verb FQNs, or REPL output.                │
│  They see a plain-language "here's what will change" and say yes/no. │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

**The asymmetry is the design.** Reads are cheap to get wrong (user sees slightly wrong data, asks again). Writes are expensive to get wrong (data corrupted, audit trail poisoned, compliance evidence destroyed). The thresholds must reflect this.

---

## Why the Current Architecture Fails

The current orchestrator applies **symmetric confidence** to reads and writes:

```
"show me the CBUs"    → verb_search(1,123 verbs) → LLM_arg_extract → DSL → REPL
"create a new CBU"    → verb_search(1,123 verbs) → LLM_arg_extract → DSL → REPL
                        ↑ same path, same search, same threshold
```

Results:
- "show me the CBUs" searches all verbs including `cbu.create` — wastes cycles, risks wrong match
- "create a new CBU" searches all verbs including `cbu.list` — same problem in reverse
- No polarity filter on candidates — verb search considers read AND write verbs for every utterance
- No threshold difference — a 0.6 confidence score sends both reads and writes to execution
- Sage classification exists but is gated behind env vars and restricted to Structure plane
- 8 hack functions (`data_management_rewrite`, `is_structure_semantics_verb`, etc.) try to paper over the missing separation

---

## Phase A: Tag Every Verb (YAML-only, Codex 5.4)

**957 verbs need `side_effects` metadata.** Without this, the pivot cannot be enforced.

Every verb in `rust/config/verbs/**/*.yaml` gets one of:

```yaml
metadata:
  side_effects: facts_only     # Sage can serve directly, no confirmation
```

```yaml
metadata:
  side_effects: state_write    # Only Coder, only after confirmation
```

### Classification is mechanical from verb name:

**`facts_only`** — verb name starts with or contains:
`list`, `get`, `read`, `show`, `describe`, `view`, `search`, `find`, `trace`, `count`, `check`, `report`, `summary`, `inspect`, `export`, `render`, `diff`, `health`, `validate` (when checking not mutating), `coverage`, `freshness`, `gaps`

**`state_write`** — verb name starts with or contains:
`create`, `add`, `new`, `register`, `update`, `set`, `edit`, `modify`, `rename`, `delete`, `remove`, `retire`, `archive`, `assign`, `attach`, `link`, `enroll`, `import`, `sync`, `load`, `publish`, `approve`, `reject`, `submit`, `propose`, `promote`, `flag`, `resolve`, `close`, `open`, `supersede`, `revoke`, `grant`, `build`, `generate` (when creating records), `compute` (writes determination runs), `run` (triggers side effects), `advance`, `drain`, `bootstrap`, `cleanup`, `reindex`

### Process

For each YAML file: open it, find every verb, add `side_effects:` to the `metadata:` block. If no `metadata:` block exists, create a minimal one.

**Files sorted by verb count (do largest first):**

```
deal.yaml              42 verbs
trading-profile.yaml   32 verbs
capital.yaml           30 verbs
service-resource.yaml  26 verbs
client-group.yaml      23 verbs
fund.yaml              22 verbs
ownership.yaml         22 verbs
cbu.yaml               20 verbs
ubo.yaml               20 verbs
document.yaml          20 verbs
... (~90 more files, same rule)
```

### Verification

```bash
python3 -c "
import yaml, glob
r = w = m = 0
for vf in glob.glob('rust/config/verbs/**/*.yaml', recursive=True):
    if '/templates/' in vf or '_meta' in vf: continue
    with open(vf) as fh:
        try: data = yaml.safe_load(fh)
        except: continue
    if not data or 'domains' not in data: continue
    for dn, dd in data['domains'].items():
        if not dd or 'verbs' not in dd: continue
        for vn, vd in dd['verbs'].items():
            if not isinstance(vd, dict):
                m += 1; print(f'  NO METADATA: {dn}.{vn}'); continue
            se = vd.get('metadata', {}).get('side_effects')
            if se == 'facts_only': r += 1
            elif se == 'state_write': w += 1
            else: m += 1; print(f'  MISSING: {dn}.{vn}')
print(f'\nfacts_only: {r}  state_write: {w}  missing: {m}  total: {r+w+m}')
assert m == 0, f'{m} verbs still missing side_effects'
"
```

**Target: 0 missing. Expected split: ~55-60% facts_only, ~40-45% state_write.**

---

## Phase B: Sage-Primary Orchestrator

### B.0 — New types

**File:** new `rust/src/sage/disposition.rs`

```rust
/// The single routing decision. Sage or Coder. Nothing else.
#[derive(Debug, Clone)]
pub enum UtteranceDisposition {
    /// Sage serves directly. Low threshold. No confirmation.
    Serve(ServeIntent),

    /// Coder takes over. High threshold. Always confirms with user first.
    Delegate(DelegateIntent),
}

#[derive(Debug, Clone)]
pub struct ServeIntent {
    pub summary: String,
    pub domain: String,
    pub action: OutcomeAction,
    pub subject: Option<EntityRef>,
}

#[derive(Debug, Clone)]
pub struct DelegateIntent {
    pub summary: String,
    pub outcome: OutcomeIntent,
}

/// Held in session state between "So you want to X?" and user "yes".
#[derive(Debug, Clone)]
pub struct PendingMutation {
    /// Plain-language confirmation shown to user
    pub confirmation_text: String,
    /// Bullet-point summary of what will change
    pub change_summary: Vec<String>,
    /// The resolved coder result (verb + args), ready to execute on "yes"
    pub coder_result: CoderResult,
    /// Original intent for tracing
    pub intent: OutcomeIntent,
}
```

**Session state contract:** When `handle_delegate()` returns a pending confirmation, the `PendingMutation` is stored in session state. The next user message is checked for confirmation words ("yes", "go ahead", "do it", "proceed", "confirm"). If confirmed → `handle_confirmed_mutation()`. If anything else → treated as a new utterance (the pending mutation expires).

**Confirmation builder:**

```rust
/// Build a human-readable mutation confirmation. No DSL. No FQNs.
fn build_mutation_confirmation(
    intent: &OutcomeIntent,
    coder_result: &CoderResult,
    lookup: &LookupResult,
) -> PendingMutation {
    // Example output:
    //   confirmation_text: "So you want to create a new CBU for Allianz — UK fund?"
    //   change_summary:
    //     - "Create CBU 'Allianz UK Fund' (jurisdiction: GB)"
    //     - "Link to client group Allianz SE"
    //     - "Set initial status: PENDING"
    
    let action_word = match intent.action {
        OutcomeAction::Create => "create",
        OutcomeAction::Update => "update",
        OutcomeAction::Delete => "delete",
        OutcomeAction::Assign => "assign",
        OutcomeAction::Import => "import",
        OutcomeAction::Publish => "publish",
        _ => "change",
    };
    
    let subject_name = intent.subject.as_ref()
        .map(|s| s.mention.as_str())
        .unwrap_or("this");
    
    PendingMutation {
        confirmation_text: format!(
            "So you want to {} {}?",
            action_word,
            subject_name,
        ),
        change_summary: describe_changes(coder_result),
        coder_result: coder_result.clone(),
        intent: intent.clone(),
    }
}
```

### B.1 — The routing function

**File:** `rust/src/agent/orchestrator.rs`

```rust
/// ONE decision point. No side doors. No fallbacks.
fn route(intent: &OutcomeIntent) -> UtteranceDisposition {
    match intent.polarity {
        // Read → Sage serves. Always. No exceptions.
        IntentPolarity::Read => UtteranceDisposition::Serve(ServeIntent {
            summary: intent.summary.clone(),
            domain: intent.domain_concept.clone(),
            action: intent.action.clone(),
            subject: intent.subject.clone(),
        }),

        // Write → Coder. Must prove it.
        IntentPolarity::Write => UtteranceDisposition::Delegate(DelegateIntent {
            summary: intent.summary.clone(),
            outcome: intent.clone(),
        }),

        // Ambiguous → Sage serves (default to safe side).
        // "run the screening" → show screening status, not run a new one.
        IntentPolarity::Ambiguous => UtteranceDisposition::Serve(ServeIntent {
            summary: intent.summary.clone(),
            domain: intent.domain_concept.clone(),
            action: intent.action.clone(),
            subject: intent.subject.clone(),
        }),
    }
}
```

### B.2 — Sage serve path (low threshold)

```rust
async fn handle_serve(
    ctx: &OrchestratorContext,
    utterance: &str,
    intent: &OutcomeIntent,
    serve: &ServeIntent,
) -> anyhow::Result<OrchestratorOutcome> {
    // 1. Entity linking
    let lookup = entity_link(ctx, utterance).await;

    // 2. Resolve verb — ONLY from facts_only verb surface
    let verb = resolve_verb_from_surface(ctx, intent, VerbFilter::FactsOnly).await?;

    // 3. Assemble args + generate DSL
    let dsl = assemble_dsl(&verb, intent, &lookup)?;

    // 4. Execute immediately — no confirmation for reads
    let result = execute_dsl(ctx, &dsl).await?;

    // 5. Return
    Ok(build_outcome(ctx, utterance, intent, &verb, &dsl, result, "sage_serve"))
}
```

**Low threshold:** search ~550-630 facts_only verbs only. Accept Medium confidence. Execute immediately. Wrong answer = user sees stale data, asks again.

### B.3 — Coder delegate path (high threshold, two-step)

The delegate path has TWO steps separated by user interaction:

**Step 1: Sage describes the mutation, asks for confirmation**

```rust
async fn handle_delegate(
    ctx: &OrchestratorContext,
    utterance: &str,
    intent: &OutcomeIntent,
    delegate: &DelegateIntent,
) -> anyhow::Result<OrchestratorOutcome> {
    // 1. Entity linking (for subject resolution)
    let lookup = entity_link(ctx, utterance).await;

    // 2. Coder resolves verb — ONLY from state_write verb surface
    //    But does NOT execute yet. Just resolves what WOULD happen.
    let coder = CoderEngine::load()?;
    let coder_result = coder.resolve(&delegate.outcome)?;

    // 3. HARD VERIFY: resolved verb MUST be state_write
    let verb_meta = lookup_verb_metadata(&coder_result.verb_fqn)?;
    if verb_meta.side_effects.as_deref() != Some("state_write") {
        return Err(anyhow::anyhow!(
            "BLOCKED: Coder resolved '{}' for write intent but verb is not state_write",
            coder_result.verb_fqn,
        ));
    }

    // 4. LOW confidence → don't even propose, ask for clarification
    if intent.confidence == SageConfidence::Low {
        return Ok(build_clarification_outcome(ctx, utterance, intent));
    }

    // 5. Build plain-language confirmation — NO DSL, NO verb FQNs
    //    The user sees: "So you want to create a new CBU for Allianz — UK fund?
    //                    This will:
    //                      • create CBU 'Allianz UK Fund' (jurisdiction: GB)
    //                      • link to client group Allianz SE
    //                    Go ahead?"
    let confirmation = build_mutation_confirmation(intent, &coder_result, &lookup);

    // 6. Return as pending confirmation — execution waits for user "yes"
    Ok(build_pending_confirmation_outcome(
        ctx, utterance, intent, &coder_result, &confirmation, "sage_delegate"
    ))
}
```

**Step 2: User confirms → Coder executes (separate invocation)**

```rust
/// Called when user confirms a pending mutation ("yes", "go ahead", "do it")
async fn handle_confirmed_mutation(
    ctx: &OrchestratorContext,
    pending: &PendingMutation,
) -> anyhow::Result<OrchestratorOutcome> {
    // 1. Generate DSL from the already-resolved coder result
    let dsl = assemble_dsl_from_coder_result(&pending.coder_result)?;

    // 2. Execute via REPL
    let result = execute_dsl(ctx, &dsl).await?;

    // 3. Return result — Sage formats for human consumption
    //    User sees: "Done. CBU 'Allianz UK Fund' created. Here's the updated list:"
    //    User NEVER sees the DSL that was executed.
    Ok(build_mutation_result_outcome(ctx, &pending, &result, "sage_confirmed"))
}
```

**The user never sees DSL.** They never see verb FQNs. They see:
- **Before:** "So you want to create a new CBU for Allianz — UK fund?"
- **After:** "Done. CBU 'Allianz UK Fund' created."

The REPL is behind the curtain. The DSL is an implementation detail.

### B.4 — New entry point

```rust
pub async fn sage_handle_utterance(
    ctx: &OrchestratorContext,
    utterance: &str,
) -> anyhow::Result<OrchestratorOutcome> {
    // Check for pending mutation confirmation first
    if let Some(pending) = ctx.session_state.take_pending_mutation() {
        if is_confirmation(utterance) {
            // User said "yes" → execute the already-resolved mutation
            return handle_confirmed_mutation(ctx, &pending).await;
        }
        // User said something else → pending mutation expires, treat as new utterance
        tracing::info!("Pending mutation expired — user did not confirm");
    }

    // Sage classifies — ALWAYS, first, unconditional
    let sage_ctx = build_sage_context(ctx);
    let engine = ctx.sage_engine.clone()
        .unwrap_or_else(|| Arc::new(DeterministicSage));
    let intent = engine.classify(utterance, &sage_ctx).await?;

    // ONE routing decision
    match route(&intent) {
        UtteranceDisposition::Serve(serve) => {
            handle_serve(ctx, utterance, &intent, &serve).await
        }
        UtteranceDisposition::Delegate(delegate) => {
            handle_delegate(ctx, utterance, &intent, &delegate).await
        }
    }
}

/// Is this a user confirmation? Deliberately narrow — must be unambiguous.
fn is_confirmation(utterance: &str) -> bool {
    let lower = utterance.trim().to_lowercase();
    matches!(lower.as_str(),
        "yes" | "y" | "go ahead" | "do it" | "proceed" | "confirm"
        | "yes please" | "go for it" | "ok" | "yep" | "sure"
        | "yes, go ahead" | "yes, do it" | "approved"
    )
}
```

**~40 lines. The entire pipeline, including mutation confirmation.**

### B.5 — Wire in, rename old

Wherever `handle_utterance()` is called:

```rust
let outcome = if std::env::var("SAGE_DISABLED").ok().as_deref() == Some("1") {
    legacy_handle_utterance(ctx, utterance).await?
} else {
    sage_handle_utterance(ctx, utterance).await?
};
```

Rename `handle_utterance()` → `legacy_handle_utterance()`.

### B.6 — Load side_effects into VerbMetadataIndex

**File:** `rust/src/sage/verb_index.rs`

Add `side_effects: Option<String>` to `VerbMeta`. Load from YAML `metadata.side_effects`. Expose `VerbMetadataIndex::facts_only_verbs()` and `VerbMetadataIndex::state_write_verbs()` filtered iterators.

### B.7 — Polarity pre-filter in verb resolution

**File:** `rust/src/sage/verb_resolve.rs`

Before scoring candidates, filter by polarity:

```rust
let candidates = match intent.polarity {
    IntentPolarity::Read => index.facts_only_verbs(),
    IntentPolarity::Write => index.state_write_verbs(),
    IntentPolarity::Ambiguous => index.facts_only_verbs(), // safe default
};
```

"show me the CBUs" never even **sees** `cbu.create` as a candidate. The search space halves. Accuracy doubles.

### B.8 — Delete dead code

After B.1-B.7 verified:

| Delete | Why |
|--------|-----|
| `data_management_rewrite()` | Sage polarity replaces it |
| `is_structure_semantics_verb()` | `side_effects` metadata replaces it |
| `is_data_management_focus()` | Sage context replaces stage_focus sniffing |
| `filter_candidates_for_data_management_structure()` | Polarity filter replaces it |
| `should_use_generic_task_subject_for_sage()` | Dead |
| `allow_data_management_structure_fast_path()` | Dead |
| `can_skip_fast_path_parse_validation()` | Dead |
| `build_sage_fast_path_result()` | Dead — no "fast path", just "the path" |
| `SAGE_SHADOW` / `SAGE_FAST_PATH` env var checks | Dead — Sage is always on |

---

## The Threshold Asymmetry in Numbers

| | Sage (serve) | Coder (delegate) |
|---|---|---|
| **Verb pool** | ~550-630 facts_only | ~400-450 state_write |
| **Confidence gate** | Medium or High | High only |
| **User interaction** | None — results shown immediately | "So you want to X?" → user "yes" → execute |
| **User sees** | Query results, data, lists | Plain-language confirmation, then results |
| **User never sees** | — | DSL, verb FQNs, REPL output |
| **Failure cost** | User sees wrong data | Data corruption |
| **Ambiguous routes here?** | Yes (safe default) | Never |
| **Cross-polarity block** | state_write verb → hard error | facts_only verb → hard error |

### The User's Experience

```
User:  "what deals does Allianz have?"
       → Sage serves immediately, shows 3 deals

User:  "show me the rate cards on the custody deal"
       → Sage serves immediately, shows rate card table

User:  "add a new rate card line for safekeeping at 2 bps"
       → Sage: "So you want to add a rate card line to the
                Allianz custody deal?
                  • Fee type: safekeeping
                  • Rate: 2 basis points on AUM
                  • Effective from: today
                Go ahead?"

User:  "yes"
       → Coder executes behind the curtain
       → Sage: "Done. Rate card line added. Here's the updated card:"
       → Shows updated rate card table (back in Sage read-only land)

User:  "actually make that 1.5 bps"
       → Sage: "So you want to update the safekeeping rate to 1.5 bps?"

User:  "no wait, show me what the competitors are charging first"
       → Pending mutation expires. Sage stays in read-only.
```

Every mutation goes through a visible, conversational confirmation. The user is always in control. The system never silently mutates.

---

## Execution Order

| Step | What | Who | Prerequisite |
|------|------|-----|-------------|
| A | Tag 957 verbs with side_effects | Codex 5.4 | None |
| B.0 | disposition.rs + PendingMutation types | Codex 5.4 | A |
| B.1 | route() function | Review Opus → Codex | A |
| B.2 | handle_serve() | Codex 5.4 | B.0, B.1 |
| B.3 | handle_delegate() + handle_confirmed_mutation() | Codex 5.4 | B.0, B.1 |
| B.4 | sage_handle_utterance() + is_confirmation() | Codex 5.4 | B.2, B.3 |
| B.5 | Wire in + legacy rename + session state for PendingMutation | Codex 5.4 | B.4 |
| B.6 | VerbMeta + side_effects loading | Codex 5.4 | A |
| B.7 | Polarity pre-filter | Codex 5.4 | B.6 |
| B.8 | Delete dead code (8 hack functions + env vars) | After full verification | B.5 |

---

## Invariants (non-negotiable)

| ID | Invariant |
|----|-----------|
| S-1 | Sage is always-on. No env var gating. |
| S-2 | Read-polarity utterance NEVER reaches the Coder. |
| S-3 | Ambiguous-polarity utterance NEVER reaches the Coder. |
| S-4 | Write-polarity utterance ALWAYS shows plain-language confirmation before execution. |
| S-5 | User NEVER sees DSL, verb FQNs, or REPL output. |
| S-6 | Coder NEVER resolves a facts_only verb. Hard error if it does. |
| S-7 | Pending mutation expires if user says anything other than a confirmation word. |
| S-8 | After mutation execution, flow returns to Sage (read-only land). |
