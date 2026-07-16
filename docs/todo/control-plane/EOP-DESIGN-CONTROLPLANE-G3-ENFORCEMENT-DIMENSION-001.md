# EOP-DESIGN-CONTROLPLANE-G3-ENFORCEMENT-DIMENSION-001

### Design doc, not implementation. Review before any code lands.
### Basis: EOP-PLAN-CONTROLPLANE-GRADUATION-001 v0.4 §2 AD-2(b) (ratified,
### architect, 2026-07-13) + tranche G3; EOP-RESEARCH-CONTROLPLANE-GRADUATION-001.md
### §A2/§B2-B5; EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001.md v0.3 §2-§5;
### EOP-DESIGN-CONTROLPLANE-T9.2-ATOMIC-ADMISSION-001.md (shape/rigor template).
### Status: RATIFIED (architect, 2026-07-13). G4 is now unblocked to start
### against this doc's mechanical spec. The §(f) runbook §5 amendment was
### applied to `EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001.md` (now v0.4) as
### part of ratification, per this tranche's own exit gate ("ratified doc
### committed; runbook §5 amended accordingly"). This RATIFIES the design
### (path-tag type, keying, grammar, signature propagation, double-admission
### interaction) — it does not itself implement G4's code.

---

## 0. What this doc must do, and what it must not do

AD-2 is **resolved**: `EnforcedVerbs` gains a path dimension, keyed by
(verb FQN, path tag), backward-compatible (untagged = all-paths). That
decision is not reopened here. What AD-2's ratification explicitly did
**not** produce is the mechanical spec: the path-tag enum's exact
variants, `EnforcedVerbs`'s exact new data shape, the env-var grammar,
and the exact signature/call-site diffs at each ingress point. That is
this doc's entire job. No code changes, no commits — this is the design
G4's implementer follows literally.

One finding surfaces during investigation that the plan's own text did
not anticipate at the level of detail this doc requires: **AD-2(b)'s
"four ingress points" is a tag-count, not a location-count.** Two of the
four tags (B, C) share one physical seam (the `dsl_v2` engine) and, in
today's production wiring, frequently share the exact same running
object instance end-to-end. §3 below addresses this directly — it is
the one place this doc corrects, rather than merely fleshes out, the
plan's cost framing.

---

## 1. Current state (verified against code, this session)

### 1.1 `EnforcedVerbs` — exact current shape

`rust/src/agent/control_plane_envelope_store.rs:27-45`:

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct EnforcedVerbs(std::collections::HashSet<String>);

impl EnforcedVerbs {
    pub(crate) fn from_env() -> Self {
        let raw = std::env::var("OB_POC_CONTROL_PLANE_ENFORCE_VERBS").unwrap_or_default();
        Self(
            raw.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        )
    }

    pub(crate) fn is_enforced(&self, verb_fqn: &str) -> bool {
        self.0.contains(verb_fqn)
    }
}
```

A flat `HashSet<String>` of verb FQNs, path-agnostic by construction —
matches R:§A2's CONFIRMED finding exactly, re-verified this session at
the cited line numbers (unchanged since the research doc's own read).

### 1.2 Every call site of `is_enforced`/`from_env`

Both `EnforcedVerbs::from_env()` construction sites and their surrounding
admission functions, confirmed by grep this session
(`control_plane_envelope_store.rs`):

| Function | `from_env()` call | `is_enforced()` call | Shape |
|---|---|---|---|
| `check_admission` (pool-based, line 90) | caller-supplied `&EnforcedVerbs` param | line 96 | Used by `admit_plan_checked` (plan-level, Path B/C) |
| `check_admission_in_scope` (scope-based, line 127) | caller-supplied `&EnforcedVerbs` param | line 133 | Used by `ObPocVerbExecutor::admit_in_scope` (Path A/D) |
| `admit_plan` (line 189) | `EnforcedVerbs::from_env()` inline, line 193 | via `check_admission` | Path B/C plan pre-flight |
| `ObPocVerbExecutor::admit_in_scope` (`sem_os_runtime/verb_executor_adapter.rs:169`) | `EnforcedVerbs::from_env()` inline | via `check_admission_in_scope` | Path A/D single-verb admission |

Both `from_env()` call sites read the env var **fresh on every call** —
no caching. This matters for §4(c): a malformed env var is re-parsed on
every admission check, not just at process start.

### 1.3 Two structurally different admission chains, not one

Investigation confirms a fact the plan's AD-2 rationale states but does
not itself derive in full: there are **two different admission call
chains**, not one shared function, and they differ in exactly the
dimension this design must thread a tag through:

- **Atomic, per-step, scope-joined** (Path A, Path D): `check_admission_in_scope`,
  called from `ObPocVerbExecutor::admit_in_scope`
  (`sem_os_runtime/verb_executor_adapter.rs:136-193`), itself called
  from `execute_verb_admitting_envelope`
  (`verb_executor_adapter.rs:540-620`) inside one `PgTransactionScope`
  that also holds the verb's own write (T9.2's atomicity property).
- **Pool-based, whole-plan pre-flight** (Path B, Path C): `check_admission`,
  called in a loop over every step by `admit_plan_checked`
  (`control_plane_envelope_store.rs:214-254`), itself called by
  `admit_plan` (line 189), before any step's transaction scope even
  opens (R:§B2's finding, re-confirmed this session).

Both chains ultimately call the same `EnforcedVerbs::is_enforced`
check — so a path dimension added to `EnforcedVerbs` is visible to both
chains uniformly, which is exactly AD-2(b)'s claimed cheapness for *that
half* of the mechanism. The threading cost lives entirely in getting the
correct tag value to each chain's call site — see §3.

---

## 2. The four ingress points — precise, with the count correction

### 2.1 Path A — Sequencer/runbook

`rust/src/runbook/step_executor_bridge.rs:553` (line re-verified this
session — unchanged from R's citation):

```rust
let outcome = match self
    .port
    .execute_verb_admitting_envelope(&step.verb, args, &mut ctx, None)
    .await
```

`self.port: Arc<dyn dsl_runtime::VerbExecutionPort>`
(`step_executor_bridge.rs:139`). This is the trait method — its default
implementation lives in `dsl-runtime` (`crates/dsl-runtime/src/port.rs:71-79`),
and the sole production override is `ObPocVerbExecutor`
(`sem_os_runtime/verb_executor_adapter.rs:540`). **Natural discriminant
already present**: the call site itself — this is the ONLY caller of
`execute_verb_admitting_envelope` inside `VerbExecutionPortStepExecutor`,
so the tag can be a compile-time constant supplied by this bridge, not
inferred from anything at runtime.

Production-confirmed as the primary dispatch path for non-durable
runbook steps: `rust/src/sequencer.rs:8322` (`dispatch_step`) checks
`self.verb_execution_port.is_some()` before falling back to the sync
`DslStepExecutor`, and `ob-poc-web/src/main.rs:1616-1622` wires
`verb_execution_port` unconditionally in production. So Path A is not
merely "one of the ingress points" — it is the dominant one for ordinary
(non-durable) session-driven verb dispatch today.

### 2.2 Path D — Bus adapter

`rust/crates/ob-poc-web/src/bus_runtime.rs:170`:

```rust
let result = self
    .executor
    .execute_verb_admitting_envelope(local_verb_id, args, &mut ctx, None)
    .await
```

`self.executor: Arc<ObPocVerbExecutor>` (`bus_runtime.rs:118-121`,
`ObPocVerbAdapter`). Same trait method as Path A, same sole override.
**Natural discriminant already present**, same reasoning as 2.1 — this
is `ObPocVerbAdapter::execute`'s only call to this method.

### 2.3 Paths B and C — the dsl_v2 engine, ONE seam, TWO tags, NOT two call sites

**The seam G4 targets**: `dsl_v2::executor::execute_verb_in_scope`
(`rust/src/dsl_v2/executor.rs:1914`, `pub(crate)`) — confirmed as the
convergence point both `execute_plan` and `execute_plan_atomic_in_scope`
reach per-step (R:§B2, re-verified: line number unchanged). No admission
check exists inside it today.

**How the seam is reached in production — traced this session, not
assumed:**

- `rust/src/repl/executor_bridge.rs`'s `RealDslExecutor` implements
  `DslExecutor` (not `DslExecutorV2` directly): `execute()` (line 151)
  and `execute_in_scope()` (line 185) both call `self.admit_plan(&plan)`
  (line 103-108, wrapping `agent::control_plane_envelope_store::admit_plan`)
  and then `executor.execute_plan`/`execute_plan_atomic_in_scope`, which
  reach the seam per-step.
- `DslExecutorV2` (`sequencer.rs:168-176`) has a **blanket impl** for any
  `T: DslExecutor` (`sequencer.rs:180-192`): `execute_v2()` just calls
  `self.execute(dsl)`. `RealDslExecutor` gets its `DslExecutorV2`
  conformance entirely through this blanket — it has no bespoke
  `execute_v2`.
- `bpmn_integration::dispatcher::WorkflowDispatcher` (`dispatcher.rs:42-63`)
  holds `inner: Arc<dyn DslExecutorV2>` and its own `execute_v2`
  (`dispatcher.rs:536-568`) branches on `self.config.route_for_verb(&verb_fqn)`:
  `ExecutionRoute::Direct` → `self.inner.execute_v2(...)` (delegates
  straight through); `ExecutionRoute::Orchestrated` → parks via bpmn-lite
  gRPC, **never reaching the dsl_v2 seam for that dispatch at all**.
- **Production wiring (`ob-poc-web/src/main.rs:1329-1352, 1625-1638`) confirms
  `inner` is a `RealDslExecutor` instance constructed once
  (line 1333) and owned exclusively by the one `WorkflowDispatcher`
  instance that wraps it — nothing else holds a reference to this
  particular `RealDslExecutor`.** When BPMN is configured
  (`bpmn_executor_v2 = Some(dispatcher)`), the orchestrator's single
  `executor_v2` slot (`sequencer.rs:247`, set via `with_executor_v2`,
  `main.rs:1627`) is the `WorkflowDispatcher`. When BPMN is not
  configured, `executor_v2` is instead a **different**, bare
  `RealDslExecutor` instance (`main.rs:1632-1637`, no `WorkflowDispatcher`
  wrapper at all — every dispatch is "Direct" by construction, there is
  no orchestrated branch to compare against).

**Consequence — the plan's four-ingress-point framing needs one
correction, not a rewrite:** at the code level there are not two
distinct call sites that reach the dsl_v2 seam for "Path B" versus
"Path C" as separately-invokable production entry points in the way
Path A (§2.1) and Path D (§2.2) are. There is:
- **One class of caller that reaches the seam via a `RealDslExecutor`
  instance never wrapped by `WorkflowDispatcher`** — the bare
  `executor_v2` fallback (no-BPMN deployments, `main.rs:1632`), the MCP
  `dsl_execute` tool, the legacy raw-execute route, and the batch/sheet
  executors, all named as `admit_plan` callers in
  `executor_bridge.rs:99-101`'s own doc comment ("every T9.3 ingress
  point... calls the same function"). None of these is reachable via
  `WorkflowDispatcher`. This is the runbook's Path B ("REPL/direct") —
  an umbrella over several call sites, not one.
- **One `RealDslExecutor` instance that is reachable ONLY via
  `WorkflowDispatcher`'s Direct branch** (`inner`, `main.rs:1333`) — this
  is the runbook's Path C. Because this instance has exactly one caller
  (`WorkflowDispatcher::execute_v2`'s Direct arm), every dispatch it
  ever receives IS, by construction, a Path-C dispatch — there is no
  ambiguity to resolve inside the object itself.

So the count of **physical ingress functions** is not "four, one per
tag" — it is closer to: 1 (Path A call site) + 1 (Path D call site) +
N≥2 (Path B's several `admit_plan` callers, all sharing one tag) + 1
(Path C's single dedicated `RealDslExecutor` instance) + 1 shared seam
(`execute_verb_in_scope`) all N+2 of the B/C callers eventually reach.
AD-2(b)'s "one enum tag at four ingress points" is correct as a
**tag-count** claim (4 named tags: A, B, C, D) but understates the
**call-site count** for B specifically. This does not change AD-2(b)'s
ratification — the cost is still bounded and the plan's own text hedges
"dsl_v2 seam once G4 lands/B+C" in a way compatible with this finding —
but G4's implementer needs the corrected shape, not the rounded-down
one, which is why this correction is recorded here rather than
discovered mid-implementation.

**What's available at each call site to serve as the tag** (design
answered in §3):
- Path B's several callers: nothing today distinguishes them from each
  other (all pass through the same `admit_plan`/`RealDslExecutor`
  shape); a single `PathTag::DslDirect` covers all of them, matching the
  runbook's own umbrella treatment.
- Path C's single `RealDslExecutor` instance: distinguished by
  **construction site**, not by any runtime signal — it is wrapped by
  `WorkflowDispatcher` and reachable no other way. The tag can be fixed
  at construction time (a builder method), no per-call plumbing needed.

---

## 3. The design

### (a) The path-tag type

```rust
// rust/crates/ob-poc-types/src/execution_path.rs

/// Which of the four RR-2 admission ingress points a verb dispatch
/// reached the control plane through. Threaded end-to-end from each
/// ingress (see EOP-DESIGN-CONTROLPLANE-G3-ENFORCEMENT-DIMENSION-001)
/// so `EnforcedVerbs` can express "graduate this verb on Path A only,"
/// matching the graduation runbook's §3 per-path order literally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ExecutionPath {
    /// Sequencer/runbook step dispatch — `VerbExecutionPortStepExecutor`
    /// → `execute_verb_admitting_envelope` (`step_executor_bridge.rs:553`).
    RunbookSequencer,
    /// Direct dsl_v2 dispatch NOT reached via `WorkflowDispatcher` —
    /// `RealDslExecutor::execute`/`execute_in_scope`, covering the MCP
    /// `dsl_execute` tool, the legacy raw-execute route, batch/sheet
    /// executors, and the no-BPMN `executor_v2` fallback.
    DslDirect,
    /// `WorkflowDispatcher`'s Direct-routed branch — the `RealDslExecutor`
    /// instance wrapped exclusively by a `WorkflowDispatcher`.
    WorkflowDispatched,
    /// Federated bus — `ObPocVerbAdapter::execute` → `execute_verb_admitting_envelope`
    /// (`bus_runtime.rs:170`).
    BusFederated,
}
```

**Home: `ob-poc-types`.** Verified this session, not assumed: it is
already the home of `EnvelopeHandle`
(`crates/ob-poc-types/src/envelope_handle.rs:1-19`), whose own module
doc states the exact rule that applies here — "a values-only boundary
crate both `dsl-runtime` and `ob-poc-control-plane` can depend on
without either depending on the other." `dsl-runtime` already imports
`ob_poc_types::EnvelopeHandle` in `VerbExecutionPort`'s trait signature
(`crates/dsl-runtime/src/port.rs:76`), and `dsl_v2::executor.rs` (the
`ob-poc` crate, home of the shared seam) already imports `ob_poc_types`
(`use ob_poc_types::ViewportState;`, `dsl_v2/executor.rs:23`, plus three
further uses). `ob-poc-control-plane` also already depends on
`ob-poc-types` (`crates/ob-poc-control-plane/Cargo.toml:20-21`, "has
zero execution-tier logic, only value types"). All four ingress crates
(`ob-poc` — Paths A/B/C via `rust/src`, `ob-poc-web` — Path D via
`crates/ob-poc-web/Cargo.toml:35`) already carry a dependency edge to
`ob-poc-types`. **No new crate edge required anywhere** — the same
zero-new-edges finding R:§B4 made for the admission-sharing question
applies here for the same structural reason.

`Copy + Hash + Eq` because it is used as (part of) a `HashMap`/`HashSet`
key in §(b). `Serialize`/`Deserialize` for the same reason `EnvelopeHandle`
carries them — telemetry/audit rows will want to record which path
admitted a dispatch (this is also useful, not designed here, for E3's
per-gate provenance dimension G2 item 4 is building independently — a
natural future join key, not this doc's scope to wire).

### (b) `EnforcedVerbs`'s new keying

**Chosen shape: `HashMap<String, PathScope>`, where:**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
enum PathScope {
    /// Untagged entry — enforced on every path. Backward-compatible
    /// default for any verb pinned before this design lands.
    All,
    /// Tagged entry — enforced only on the named paths.
    Only(std::collections::HashSet<ob_poc_types::ExecutionPath>),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct EnforcedVerbs(std::collections::HashMap<String, PathScope>);

impl EnforcedVerbs {
    pub(crate) fn is_enforced(&self, verb_fqn: &str, path: ExecutionPath) -> bool {
        match self.0.get(verb_fqn) {
            None => false,
            Some(PathScope::All) => true,
            Some(PathScope::Only(paths)) => paths.contains(&path),
        }
    }
}
```

**Rejected alternative: `HashSet<(String, PathTag)>`.** This was the
plan text's own first-mentioned shape (§2 AD-2's prose: "keyed by (verb
FQN, path tag)"). Rejected because it cannot express "untagged = all
paths" as a first-class member — an untagged entry would need a
sentinel `PathTag::Any` variant threaded through every real ingress
point's tag value too (since `is_enforced` would need to check both
`(verb, path)` AND `(verb, Any)` on every call), which either (i)
pollutes `ExecutionPath` with a non-physical variant that no ingress
point actually produces, muddying the type's meaning, or (ii) requires
`is_enforced` to do a second lookup per call, which is not wrong but is
strictly more code than the chosen shape for the identical outcome.
`HashMap<String, PathScope>` keeps `ExecutionPath` a closed set of the
four real, physically-producible tags and pushes the "all vs. some"
distinction into its own two-variant enum, which is where it
conceptually belongs — a verb's enforcement scope, not a fifth path.

**Why not `HashMap<String, HashSet<PathTag>>` with empty-set meaning
"none listed = enforce nowhere" and some other signal for "all"?**
Considered and rejected: it would need `EnforcedVerbs` to special-case an
out-of-band "all" marker (e.g. inserting all four current variants into
the set at parse time) — which silently breaks the moment a fifth path
is ever added (the historical untagged rows would stay pinned to the
four original variants, not to "still means all," a correctness trap
for a safety-relevant mechanism). `PathScope::All` names the intent
directly and needs no future migration when a fifth ingress point
someday exists.

**Deprecation of the pool-based/`(String)`-keyed constructor used in
tests** (`control_plane_envelope_store.rs:539-540`,
`fn set(verbs: &[&str]) -> EnforcedVerbs`): becomes
`fn set(verbs: &[&str]) -> EnforcedVerbs` returning `PathScope::All` for
each entry (same test semantics — "enforced everywhere" is exactly what
existing tests assert), plus a new `fn set_scoped(entries: &[(&str,
&[ExecutionPath])])` helper for the new per-path tests §5 requires.

### `is_enforced`'s new signature

```rust
pub(crate) fn is_enforced(&self, verb_fqn: &str, path: ExecutionPath) -> bool
```

Every existing caller gains one argument. §3(d) enumerates each.

### (c) Env-var syntax

**Grammar:**

```
OB_POC_CONTROL_PLANE_ENFORCE_VERBS = entry (',' entry)*
entry  = verb-fqn (':' path-tag (('|' path-tag))* )?
path-tag = 'A' | 'B' | 'C' | 'D'
```

Examples:
- `cbu.confirm` — untagged, `PathScope::All` (today's semantics,
  unchanged).
- `cbu.confirm:A` — enforced on Path A (`RunbookSequencer`) only.
- `cbu.confirm:A|D` — enforced on Path A and Path D, not B/C.
- `cbu.confirm,kyc.person.approve:A` — mixed: `cbu.confirm` untagged
  (all paths), `kyc.person.approve` scoped to A only.

**Letter choice**: `A`/`B`/`C`/`D` (not the full enum-variant names) —
matches the runbook's own vocabulary exactly (§2/§3/§4 all say "Path
A"/"Path B"/etc.), keeps the env var terse for an operator typing it by
hand during a graduation event (runbook §5 step 3, "additive to
whatever's already enforced"), and the mapping is a fixed 4-entry table
(`'A' → RunbookSequencer`, `'B' → DslDirect`, `'C' → WorkflowDispatched`,
`'D' → BusFederated`) — no ambiguity, no abbreviation collision risk
given there are only four.

**Parsing rule**: split on `,` first (unchanged from today), then for
each entry split on the first `:`. No colon → `PathScope::All`. Colon
present → split the remainder on `|`, map each letter through the fixed
table, collect into `PathScope::Only(HashSet)`. Whitespace trimmed
around the whole entry (unchanged from today's per-entry `.trim()`);
not additionally trimmed around each `|`-separated letter (a hand-typed
`A | D` is malformed by design — keep the grammar strict, since this env
var gates production admission).

**Malformed-entry behavior: fail closed for the WHOLE config, not just
the bad entry.** `from_env()` becomes fallible:

```rust
pub(crate) fn from_env() -> Result<Self, EnforcedVerbsParseError>
```

A single malformed entry (unrecognized path letter, empty tag list after
a colon, e.g. `cbu.confirm:`) makes the **entire** `EnforcedVerbs::from_env()`
call return `Err`. Every call site (`admit_plan`,
`ObPocVerbExecutor::admit_in_scope`) treats `Err` as **"cannot determine
enforcement state" → reject every enforced-looking dispatch it would
otherwise have to guess about**. Concretely: on parse failure, the
admission functions do not silently fall back to `EnforcedVerbs::default()`
(which would mean "nothing enforced," i.e. **fail open** — the wrong
direction for a safety-relevant admission gate) and do not partially
apply the entries that DID parse (ambiguous which of several
comma-separated entries an operator intended as a set). Instead: log at
`error!` level with the raw offending substring, and every dispatch that
reaches `admit_in_scope`/`admit_plan_checked` while the env var is
unparseable is rejected with a `SemOsError::Internal` naming the parse
failure — the same "loud, not silent" posture the T6.1a design note
(runbook §4, Path D open item) already established for a different
trust-boundary case ("a forged or replayed string in that field must
void loudly"). **Justification**: this is a process-wide env var read
fresh on every admission check (§1.2) — a typo introduced by an operator
mid-graduation-event (runbook §5 step 3, hand-edited, additive) must not
silently degrade to "nothing is enforced" (masking that verbs the
operator believed were graduated are now unguarded) nor silently degrade
to "enforce only the entries that happened to parse" (a different set of
verbs enforced than the operator typed, discovered only by an incident).
A hard, loud, whole-config failure is the only option that cannot be
mistaken for a correctly-applied state. This does temporarily halt
dispatch of every verb that WOULD be checked against `EnforcedVerbs`
regardless of scope-emptiness during the parse-failure window — a
production incident of its own if it happens, but a detectable one
(every rejection carries the parse-error message), unlike the two silent
alternatives.

### (d) Signature change propagation

**`admit_in_scope` today** (`sem_os_runtime/verb_executor_adapter.rs:136-141`):

```rust
async fn admit_in_scope(
    &self,
    verb_fqn: &str,
    envelope_handle: Option<ob_poc_types::EnvelopeHandle>,
    conn: &mut sqlx::PgConnection,
) -> dsl_runtime::Result<Option<ob_poc_control_plane::snapshot::SnapshotPins>>
```

Does **not** receive enough context today to determine the caller's
path — confirms the plan's premise; this is a real signature change, not
a data-already-there wiring exercise. New signature:

```rust
async fn admit_in_scope(
    &self,
    verb_fqn: &str,
    envelope_handle: Option<ob_poc_types::EnvelopeHandle>,
    path: ob_poc_types::ExecutionPath,
    conn: &mut sqlx::PgConnection,
) -> dsl_runtime::Result<Option<ob_poc_control_plane::snapshot::SnapshotPins>>
```

which threads `path` into `check_admission_in_scope(conn, &enforced,
verb_fqn, path, envelope_handle)`.

**`execute_verb_admitting_envelope`** — the `VerbExecutionPort` trait
method (`crates/dsl-runtime/src/port.rs:71-79`), the shared method Path
A and Path D both call — gains the same parameter:

```rust
async fn execute_verb_admitting_envelope(
    &self,
    verb_fqn: &str,
    args: serde_json::Value,
    ctx: &mut VerbExecutionContext,
    envelope_handle: Option<ob_poc_types::EnvelopeHandle>,
    path: ob_poc_types::ExecutionPath,
) -> Result<VerbExecutionResult> {
    // default impl unchanged in shape — ignores `path` exactly as it
    // already ignores `envelope_handle`, degrades to `execute_verb`.
    self.execute_verb(verb_fqn, args, ctx).await
}
```

Default-impl-degrades-safely precedent already exists here (the
`envelope_handle` parameter's own doc comment states this explicitly,
`port.rs:65-70`) — the same posture extends cleanly to `path`. **Two
production implementors need the call-site literal added, both
one-line, compile-time-constant changes**:
- `step_executor_bridge.rs:553` — add `ob_poc_types::ExecutionPath::RunbookSequencer`
  as the new trailing argument.
- `bus_runtime.rs:170` — add `ob_poc_types::ExecutionPath::BusFederated`.

`ObPocVerbExecutor::execute_verb_admitting_envelope`
(`verb_executor_adapter.rs:540`) — the sole non-default trait override —
gains the parameter and threads it into its own `admit_in_scope` call
(§(d) above).

Test-double implementors (`step_executor_bridge.rs`'s `UnusedPort`,
`sem_os_harness::HarnessMockExecutor`) — checked this session
(`grep -n "impl.*VerbExecutionPort for"`): **neither overrides
`execute_verb_admitting_envelope`**, both rely on the trait's default —
so neither needs a code change; the default's new (ignored) parameter is
transparent to them.

**Path B/C's `admit_plan` chain** — does **not** get the parameter
threaded through its own signature (`admit_plan`/`admit_plan_checked`/
`check_admission`). Per §2.3's finding, the tag for Path B/C is
determined at a different layer than the per-verb loop — see below.

**Path B/C's actual threading point: `dsl_v2::executor::ExecutionContext`.**
This struct (`dsl_v2/executor.rs:477-...`) is a plain, publicly-fielded
value built fresh per call by `RealDslExecutor::build_executor_and_ctx()`
(`executor_bridge.rs:113-128`) and threaded by `&mut` reference through
`execute_plan`/`execute_plan_atomic_in_scope`/`execute_verb_in_scope` for
the whole plan — i.e. it is **already the exact carrier this design
needs**, set once per dispatch, read at the seam, with zero new
signature widening on `execute_plan`/`admit_plan`/`DslExecutorV2::execute_v2`.
Design: add one field —

```rust
pub struct ExecutionContext {
    // ...existing fields...
    /// G3: which ingress path this dispatch entered through. Set once
    /// at context construction; read at `execute_verb_in_scope`'s (G4)
    /// admission check. Defaults to `DslDirect` — every `ExecutionContext`
    /// constructed by a caller that hasn't been updated to set this
    /// explicitly is Path B by default, matching this design's own
    /// umbrella treatment of every currently-unlabelled `admit_plan`
    /// caller (§2.3).
    pub execution_path: ob_poc_types::ExecutionPath,
}
```

`RealDslExecutor::build_executor_and_ctx()` sets it from a new field on
`RealDslExecutor` itself, `execution_path: ExecutionPath`, set at
construction via a new builder method `.with_execution_path(path)`
(same pattern as its existing `.with_services()`/`.with_sem_os_ops()`).
Each of `main.rs`'s distinct `RealDslExecutor::new(...)` call sites
(§2.3) tags itself once, at wiring time — **not per-dispatch, per-instance**:
- `main.rs:1333` (`inner`, wrapped exclusively by `WorkflowDispatcher`)
  → `.with_execution_path(ExecutionPath::WorkflowDispatched)`.
- `main.rs:1359` (`worker_executor`, `JobWorker` durable resume),
  `main.rs:1464` (`legacy_executor`, orchestrator ctor fallback),
  `main.rs:1632` (bare `executor_v2`, no-BPMN deployments) → all
  `.with_execution_path(ExecutionPath::DslDirect)` — matches §2.3's
  finding that none of these has a way to distinguish itself from any
  other Path-B caller and the runbook treats them as one path. The
  `RealDslExecutor` default (when `.with_execution_path` is never
  called) is also `DslDirect`, so any construction site this doc's
  author missed fails safe into the umbrella tag rather than an
  unset/panicking state.

At the seam (`dsl_v2/executor.rs:1914`, G4's insertion point), the
admission call reads `ctx.execution_path` and passes it into
`check_admission`/`check_admission_in_scope` exactly as Path A/D do —
one read, no new plumbing beyond the field itself.

**Every call site touched, summarised for G4's implementer:**

| File:line | Change |
|---|---|
| `crates/ob-poc-types/src/execution_path.rs` (new file) | Define `ExecutionPath` |
| `crates/ob-poc-types/src/lib.rs` | `pub mod execution_path; pub use execution_path::ExecutionPath;` |
| `agent/control_plane_envelope_store.rs:27-45` | `EnforcedVerbs` → `HashMap<String, PathScope>`; `is_enforced(verb_fqn, path)`; `from_env() -> Result<Self, EnforcedVerbsParseError>` |
| `agent/control_plane_envelope_store.rs:90-143` | `check_admission`/`check_admission_in_scope` gain `path: ExecutionPath` param, pass through to `is_enforced` |
| `agent/control_plane_envelope_store.rs:189-254` | `admit_plan`/`admit_plan_checked` read `path` from each step's own `ExecutionContext` at loop time (see below — NOT a new fn param; the plan-level loop iterates steps but the tag is context-scoped per whole-plan dispatch, so it's read once, hoisted above the loop) |
| `crates/dsl-runtime/src/port.rs:71-79` | `execute_verb_admitting_envelope` gains `path: ExecutionPath` param, default impl ignores it |
| `sem_os_runtime/verb_executor_adapter.rs:136-193` | `admit_in_scope` gains `path` param, threads to `check_admission_in_scope` |
| `sem_os_runtime/verb_executor_adapter.rs:540-620` | `execute_verb_admitting_envelope` override gains `path` param, threads to `admit_in_scope` |
| `runbook/step_executor_bridge.rs:553` | Add `ExecutionPath::RunbookSequencer` argument |
| `crates/ob-poc-web/src/bus_runtime.rs:170` | Add `ExecutionPath::BusFederated` argument |
| `dsl_v2/executor.rs:477-...` | `ExecutionContext` gains `execution_path: ExecutionPath` field (default `DslDirect`) |
| `dsl_v2/executor.rs:1914` (G4's own insertion, not this doc's) | Admission call reads `ctx.execution_path` |
| `repl/executor_bridge.rs` (`RealDslExecutor`) | New field `execution_path: ExecutionPath` (default `DslDirect`), builder `.with_execution_path(...)`, `build_executor_and_ctx()` sets `ctx.execution_path` from it |
| `ob-poc-web/src/main.rs:1333,1359,1464,1632` | Tag each `RealDslExecutor::new(...)` construction per §(d)'s table |

One correction to §(d)'s own table entry above, worth flagging plainly:
`admit_plan_checked`'s per-step loop (`control_plane_envelope_store.rs:219-252`)
iterates `&plan.steps`, but the path tag is a property of the **whole
call** (which `RealDslExecutor`/context dispatched this plan), not of
any individual step — so `admit_plan`/`admit_plan_checked` need a new
`path: ExecutionPath` **function parameter** after all (read once by the
caller from its own `ExecutionContext` and passed in), not a per-step
lookup. This is a genuine, small signature change on `admit_plan`
(`control_plane_envelope_store.rs:189`) and `admit_plan_checked` (line
214) that the ExecutionContext-carrier design does not avoid — only the
deeper `execute_plan`/`DslExecutorV2::execute_v2`/`dsl_v2::executor`
internals avoid new parameters, because the seam itself (§2.3, G4's own
territory) reads `ctx.execution_path` directly rather than needing it
passed down the call stack a second way.

### (e) Interaction with G4's double-admission guard

**The edge case is real, traced this session, not merely restated from
the plan.** `ObPocVerbExecutor::execute_verb_in_open_scope`'s Branch 3
(`verb_executor_adapter.rs:309-318`, the generic/unregistered-verb
fallthrough) calls `self.executor.execute_verb_in_scope(&vc, &mut
exec_ctx, scope)` — **the exact same seam** (`dsl_v2/executor.rs:1914`)
G4 plans to instrument directly for Path B/C. Concretely: a Path A
dispatch (`execute_verb_admitting_envelope` → `admit_in_scope` →
already admitted under `ExecutionPath::RunbookSequencer`) that resolves
to an unregistered/plugin FQN falls through Branch 3 into the identical
seam G4 is about to gate a second time.

**Resolution: the fallthrough carries the SAME tag as the outer call,
not a distinct "reached via fallthrough" tag — and the seam's admission
check must be skippable-with-proof for a dispatch that already carries
proof of admission under that exact tag.** Reasoning:

- A **distinct** fallthrough tag (e.g. a hypothetical `ExecutionPath::ObPocVerbExecutorFallthrough`)
  would let an operator graduate `some.verb` on `RunbookSequencer` and
  have it silently remain unenforced whenever that verb happens to route
  through Branch 3's fallthrough — the exact asymmetry AD-2(b)'s own
  rationale (E2's per-path exclusivity reasoning) was ratified to close.
  The fallthrough is not a fifth path from an operator's perspective; it
  is Path A (or D) continuing to execute the same verb dispatch by a
  different internal branch. Tagging it separately would reintroduce,
  one layer down, the exact bug AD-2(b) fixes one layer up.
- Therefore: `execute_verb_in_open_scope` must pass the SAME
  `ExecutionPath` value it received (from `execute_verb_admitting_envelope`'s
  own `path` parameter, §(d)) into whatever the seam needs to recognise
  "already admitted." The mechanism: `ExecutionContext` (`to_dsl_context(ctx)`,
  `verb_executor_adapter.rs:312`, converts `VerbExecutionContext` →
  `dsl_v2::executor::ExecutionContext` for Branch 3's call) already needs
  the `execution_path` field from §(d) populated — set it there to the
  SAME tag the outer `execute_verb_admitting_envelope` call carried, not
  left at the `DslDirect` default. `to_dsl_context` is the one conversion
  function on this path (confirmed the only caller of `execute_verb_in_scope`
  from Branch 3); this is a one-line addition to that function once its
  signature/callers thread `path` through (same table row as `execute_verb_in_open_scope`'s
  own new `path` parameter, needed regardless for `admit_in_scope`'s
  threading in §(d)).
- **Skip proof, not a boolean flag.** A bare `already_admitted: bool` on
  `ExecutionContext` is forgeable-by-omission (any future caller that
  doesn't set it explicitly defaults to `false`, which fails safe here —
  acceptable — but a caller that sets it to `true` incorrectly would
  silently bypass admission with no compiler or runtime signal). Given
  this doc's §(c) fail-closed discipline for parsing, the same rigor
  applies here: the seam's check should key its skip decision on
  **matching the two tags** — "this dispatch already carries an admitted
  `ExecutionPath` value equal to the seam's own caller-supplied
  `path`" — not a separate opaque boolean. Concretely, the seam's new
  admission call (G4's own code, not this doc's, but its contract is
  fixed here): `execute_verb_in_scope` receives `path: ExecutionPath`
  like every other call site in §(d)'s table; `ObPocVerbExecutor`'s
  Branch-3 caller is the ONLY caller that can also supply proof of prior
  admission (a new `already_admitted_for: Option<ExecutionPath>` field
  on `ExecutionContext`, set by `execute_verb_admitting_envelope` right
  after its own successful `admit_in_scope` call, cleared/never-set by
  every other constructor). The seam's check: `if ctx.already_admitted_for
  == Some(path) { skip } else { run the check }` — a value match, not a
  flag, so a caller cannot accidentally claim "already admitted" for a
  DIFFERENT path than the one it actually passed through.

This is the concrete "hard test" the plan's G4 item 2 names but leaves
undesigned: **Branch-3 fallthrough must neither double-consume nor
reject a properly admitted dispatch** — satisfied here because (i) the
tag match means the seam recognises this exact dispatch already cleared
`RunbookSequencer`/`BusFederated` admission and skips re-checking
`EnforcedVerbs` a second time (no double-consume of a single-use
envelope, since `check_admission_in_scope`'s consume-by-id logic never
runs twice), and (ii) any OTHER dispatch reaching the seam via Path
B/C's `DslDirect`/`WorkflowDispatched` tags (no `already_admitted_for`
set) is checked normally, so a genuinely un-admitted dispatch is never
waved through by this skip path.

### (f) Runbook §5 amendment

Current §5 step 1 (`EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001.md:252-253`):

> 1. Freeze the exact verb-FQN set being graduated (e.g. `cbu.confirm`) —
>    never graduate `*` in one move.

and step 3 (lines 256-257):

> 3. Set `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` to include the new verb(s),
>    additive to whatever's already enforced (comma-separated).

**Amendment (to land alongside this doc's ratification, not deferred to
G4):**

> 1. Freeze the exact (verb-FQN, path-tag) set being graduated — e.g.
>    `cbu.confirm:A` graduates `cbu.confirm` on Path A (Sequencer/runbook)
>    only, leaving it shadow-only on B/C/D. Never graduate a bare
>    untagged verb-FQN (`PathScope::All`) as a first move — the
>    graduation order in §3 exists precisely so each path earns its own
>    evidence window before being folded in; an untagged entry silently
>    grants every path enforcement the moment ANY one path's window
>    closes, which defeats §3's ordering. Untagged entries are reserved
>    for a verb that has already independently graduated on all four
>    paths (each with its own closed evidence window per §4) and is
>    being consolidated for operator-legibility, not for a first
>    graduation.
> 2. (unchanged)
> 3. Set `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` to include the new
>    `verb:path-tag` entry (or `verb:tag1|tag2` for multiple paths
>    graduating together, e.g. after both close their windows on the
>    same day), additive to whatever's already enforced
>    (comma-separated). See
>    `EOP-DESIGN-CONTROLPLANE-G3-ENFORCEMENT-DIMENSION-001.md` §3(c) for
>    the exact grammar and its fail-closed parse-error behavior — a
>    malformed entry rejects every dispatch this env var would otherwise
>    gate, not just the malformed one.
> 4/5. (unchanged, but step 5's ledger record now names the path tag(s)
>    graduated, not just the verb FQN — "which C-0xx rows this flip
>    closes" is now provable per-path, matching E2's own per-path
>    reasoning.)

§4's per-path precondition tables (Path A / Path B+C / Path D) are
unaffected in structure — they already enumerate preconditions per path;
this amendment only changes what §5's procedural steps DO with a
graduated verb once its path's preconditions are met, not the
preconditions themselves.

---

## 4. Worked example: pin `cbu.confirm` on Path A only

1. **Operator sets**: `OB_POC_CONTROL_PLANE_ENFORCE_VERBS=cbu.confirm:A`.
2. **Parse**: `EnforcedVerbs::from_env()` splits on `,` (one entry),
   splits `cbu.confirm:A` on the first `:` → verb `cbu.confirm`, tag
   string `A`. `A` maps to `ExecutionPath::RunbookSequencer` via the
   fixed table. Result: `EnforcedVerbs({"cbu.confirm": PathScope::Only({RunbookSequencer})})`.
3. **Path A dispatch** (`step_executor_bridge.rs:553`): calls
   `execute_verb_admitting_envelope("cbu.confirm", args, ctx, None,
   ExecutionPath::RunbookSequencer)`. `ObPocVerbExecutor`'s override
   calls `admit_in_scope("cbu.confirm", None, ExecutionPath::RunbookSequencer,
   conn)`, which calls `check_admission_in_scope(conn, &enforced,
   "cbu.confirm", ExecutionPath::RunbookSequencer, None)`. `enforced.is_enforced("cbu.confirm",
   RunbookSequencer)` → `PathScope::Only({RunbookSequencer}).contains(&RunbookSequencer)`
   → `true`. No envelope handle supplied (`None`) →
   `AdmissionDecision::RejectedNoEnvelope`. **Correctly rejects** — this
   verb is now genuinely enforced on Path A, and Path A's own
   seal→consume wiring (G1, separate tranche) is what supplies a real
   envelope handle once that lands; until then, every `cbu.confirm`
   dispatch via Path A is rejected outright, which is the intended
   "enforce mode" behaviour once a verb graduates, not a bug in this
   design.
4. **Path D dispatch** (`bus_runtime.rs:170`): calls
   `execute_verb_admitting_envelope("cbu.confirm", args, ctx, None,
   ExecutionPath::BusFederated)`. Same `enforced` set. `enforced.is_enforced("cbu.confirm",
   BusFederated)` → `PathScope::Only({RunbookSequencer}).contains(&BusFederated)`
   → `false` → `AdmissionDecision::NotEnforced`. **Dispatch proceeds
   exactly as before** — `cbu.confirm` over the bus is untouched by this
   graduation, which is the entire point of AD-2(b): Path A's
   graduation does not implicitly graduate Path D.
5. **Path B/C dispatch** (any `admit_plan` caller): `admit_plan(pool,
   plan, ExecutionPath::DslDirect)` (or `WorkflowDispatched` for the
   `inner`-wrapped instance) loops the plan's steps, calling
   `check_admission(pool, &enforced, "cbu.confirm", DslDirect, None)`
   for a step invoking `cbu.confirm`. `enforced.is_enforced("cbu.confirm",
   DslDirect)` → `false` → `NotEnforced`, plan admission proceeds. Same
   outcome as Path D — untouched.

This traces the exact worked scenario AD-2(b)'s own rationale names
("graduate this verb on Path A only") end to end through the concrete
mechanism, confirming the design closes the gap R:§A2 identified: "you
cannot enforce `cbu.confirm` on Path A only while leaving it shadow on
Path D as currently built" is no longer true under this design.

---

## 5. Backward-compatibility test plan

All tests below are additions to
`agent/control_plane_envelope_store.rs`'s existing `#[cfg(test)] mod
tests` (live-DB tests already exist there for the pre-G3 shape, e.g.
`check_admission_in_scope_matches_check_admission_behavior`,
line 881 — these are the direct ancestors the new tests extend, not
replace).

1. **Untagged entry enforces on every path** (the core backward-compat
   claim): `EnforcedVerbs::from_env()`-equivalent test helper constructs
   `EnforcedVerbs` with `"cbu.confirm"` untagged (`PathScope::All`);
   assert `is_enforced("cbu.confirm", path)` returns `true` for all four
   `ExecutionPath` variants. This is the literal test that any verb
   pinned before this design lands keeps its exact current behaviour —
   run against the SAME assertions the pre-existing test
   `listed_verb_is_enforced_others_are_not` (line 549) makes today,
   parameterised over all four paths instead of the old path-agnostic
   single call.
2. **Tagged entry enforces only on the named path(s)**: `"cbu.confirm:A"`
   → `is_enforced("cbu.confirm", RunbookSequencer) == true`,
   `is_enforced("cbu.confirm", DslDirect/WorkflowDispatched/BusFederated)
   == false` for each of the other three.
3. **Multi-tag entry**: `"cbu.confirm:A|D"` → true for
   `RunbookSequencer`/`BusFederated`, false for `DslDirect`/`WorkflowDispatched`.
4. **Malformed tag fails the whole config, not just the entry**:
   `"cbu.confirm:A,kyc.person.approve:Z"` (unrecognised letter `Z`) →
   `EnforcedVerbs::from_env()`-equivalent parse returns `Err`; assert
   `cbu.confirm`'s otherwise-valid tag `A` is NOT silently applied (the
   fail-closed-whole-config claim in §3(c), proven, not just asserted in
   prose).
5. **Empty tag after colon fails**: `"cbu.confirm:"` → `Err`.
6. **Live-DB, Path-A-tagged / Path-D-untouched** (extends the existing
   `check_admission_in_scope_matches_check_admission_behavior` shape,
   line 881): seal a real envelope for `cbu.confirm`, set
   `EnforcedVerbs` to `{"cbu.confirm": Only({RunbookSequencer})}`;
   assert `check_admission_in_scope(conn, &enforced, "cbu.confirm",
   RunbookSequencer, Some(handle))` admits and consumes; assert a
   SEPARATE `check_admission(pool, &enforced, "cbu.confirm", BusFederated,
   None)` (simulating Path D) returns `NotEnforced` for the exact same
   verb FQN in the exact same `EnforcedVerbs` value — the two-call
   proof that one config cannot leak enforcement across paths.
7. **Branch-3 fallthrough tag-match skip** (§(e)'s hard test, live-DB):
   admit a dispatch under `ExecutionPath::RunbookSequencer` with a real
   envelope through `execute_verb_admitting_envelope` for an
   unregistered/plugin FQN that falls through Branch 3 to the
   `dsl_v2` seam; assert the envelope is consumed **exactly once** (not
   twice — the seam's own check must have been skipped, not
   independently satisfied by a second consume) and the verb dispatches
   successfully. Negative case: construct an `ExecutionContext` with
   `already_admitted_for: Some(WorkflowDispatched)` but call the seam
   with `path: RunbookSequencer` (mismatched tag, simulating a
   hypothetical wiring bug) — assert the seam does NOT skip (tags must
   match exactly, not merely be "some admission happened").
8. **Regression — every existing test in `control_plane_envelope_store.rs`'s
   `#[cfg(test)] mod tests`** (lines 537-1041 range, `EnforcedVerbs`
   construction, `check_admission`/`check_admission_in_scope`,
   `admit_plan_checked` tests) continues to pass with the new
   `PathScope::All`-constructing `set()` helper and an explicit
   `ExecutionPath` argument threaded into each existing call — no
   assertion's expected outcome changes, only the call signature. This
   is the literal backward-compatibility bar: same inputs (mapped
   through the compat helper), same outputs.

---

## 6. Open questions for architect ratification

1. **§(d)'s JobWorker (`worker_executor`) tag**: this design assigns it
   `ExecutionPath::DslDirect` alongside the MCP tool/legacy route/no-BPMN
   fallback, on the grounds that the plan/runbook never named a
   "durable resume" path distinct from A/B/C/D and this is the least-
   surprising default. But a durable verb that originally PARKED via
   Path A (a runbook step) and later RESUMES via `JobWorker` is, in a
   real sense, still "the same Path A dispatch continuing" — tagging its
   resume as `DslDirect` means a verb graduated on Path A only would be
   admitted at park-time (Path A tag) but potentially re-admitted under
   a DIFFERENT tag at resume-time if resume ever reaches an admission
   check at all. **T9.2's own OQ4** (durable/park interaction, still
   open per that doc's §8 item 4) already flags that resume's re-entry
   path needs tracing "before or during implementation, not assumed" —
   this design doesn't resolve that trace, it only assigns a default tag
   for whichever `RealDslExecutor` instance JobWorker holds, pending
   that trace. Flagging explicitly: if T9.2's OQ4 trace finds resume
   re-enters through `execute_verb_admitting_envelope` itself (the
   admitting entry point), the correct tag is almost certainly "whichever
   path originally parked it," not a fixed construction-time tag — a
   design this doc cannot finish without that trace's answer. Not a
   blocker for G4 (durable verbs are a narrow slice, and `EnforcedVerbs`'s
   production-default-empty posture means this only bites a verb that is
   BOTH durable AND enforced AND resumed — not on G4's own critical
   path), but should not be silently forgotten either.
2. **Env-var letter stability**: this design fixes `A`/`B`/`C`/`D` to the
   four `ExecutionPath` variants by position/name match to the runbook's
   existing prose. If a fifth ingress point is ever added (not currently
   planned, no evidence any is imminent), the grammar in §3(c) extends
   trivially (a fifth letter), but this is worth architect awareness
   since the letters are being baked into an operator-facing env var
   grammar, not just internal code.
