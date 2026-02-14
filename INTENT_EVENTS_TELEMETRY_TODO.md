# TODO — Wire Up `intent_events` Telemetry (Single-Pipeline) + Establish Tuning Lifecycle

**Repo:** `ob-poc`  
**Objective:** Add a first-class, append-only telemetry stream for the **single orchestrator pipeline**:
- log utterance fingerprint + candidates + SemReg effect + selection + outcome + run_sheet linkage
- enable offline review of “chat prompt / intent guesses / response loop”
- support periodic “semi-static tune-up” (boost weights, aliases, schema tweaks) without mutating SemReg snapshots online

This is a *rewire*: most data already exists in `IntentTrace`; we persist it and add a small review + tuning loop.

---

## 0) Design constraints (must hold)

- **Single source of truth for state:** run_sheet / repl DSL remains the state machine.
- **Telemetry is append-only:** never drives execution directly.
- **PII safe by default:** store hashes/ids/keys, not raw sensitive values.
- **Single emission point:** only the orchestrator writes `intent_events` (no side paths).
- **Non-blocking:** telemetry failure must not break the main pipeline (best-effort with explicit trace flag).

---

## 1) Schema (minimal v0.1)

### 1.1 Migration: `agent.intent_events`
Add a migration under your standard numbering scheme.

**SQL (suggested)**
```sql
CREATE SCHEMA IF NOT EXISTS agent;

CREATE TABLE IF NOT EXISTS agent.intent_events (
  event_id            uuid PRIMARY KEY,
  ts                  timestamptz NOT NULL DEFAULT now(),

  session_id          uuid NOT NULL,
  actor_id            text NOT NULL,
  entrypoint          text NOT NULL,        -- chat|mcp|repl|execute

  utterance_hash      text NOT NULL,        -- sha256(normalized)
  utterance_preview   text NULL,            -- redacted/trimmed (optional)
  scope               text NULL,

  subject_ref_type    text NULL,            -- entity|case|none
  subject_ref_id      uuid NULL,

  semreg_mode         text NOT NULL,        -- applied|deny_all|unavailable|fail_open
  semreg_denied_verbs jsonb NULL,

  verb_candidates_pre  jsonb NULL,          -- top N
  verb_candidates_post jsonb NULL,          -- after SemReg filter/boost

  chosen_verb_fqn     text NULL,
  selection_source    text NULL,            -- discovery|user_choice|semreg|macro
  forced_verb_fqn     text NULL,

  outcome             text NOT NULL,        -- matched|clarify_verb|clarify_args|no_allowed_verbs|direct_dsl_denied|error|...
  dsl_hash            text NULL,
  run_sheet_entry_id  uuid NULL,

  macro_semreg_checked bool NOT NULL DEFAULT false,
  macro_denied_verbs   jsonb NULL,

  error_code          text NULL
);

CREATE INDEX IF NOT EXISTS intent_events_ts_idx ON agent.intent_events(ts);
CREATE INDEX IF NOT EXISTS intent_events_session_idx ON agent.intent_events(session_id, ts);
CREATE INDEX IF NOT EXISTS intent_events_utter_hash_idx ON agent.intent_events(utterance_hash);
CREATE INDEX IF NOT EXISTS intent_events_chosen_verb_idx ON agent.intent_events(chosen_verb_fqn);
```

**Acceptance**
- Migration applies cleanly; no new write paths yet.

---

## 2) Rust model + store (write-only)

### 2.1 Add model
- [ ] Add `rust/src/agent/telemetry/intent_event.rs`
  - `struct IntentEventRow { ... }` mirroring table
  - Use `serde_json::Value` for jsonb fields

### 2.2 Add store
- [ ] Add `rust/src/agent/telemetry/store.rs`
  - `async fn insert_intent_event(db: &PgPool, row: IntentEventRow) -> Result<()>`
  - Must be best-effort: return error upward only if you explicitly want to fail (default: log + continue)

### 2.3 Hashing + preview redaction helpers
- [ ] Add `rust/src/agent/telemetry/redaction.rs`
  - `fn normalize_utterance(s: &str) -> String` (lowercase, trim, collapse whitespace)
  - `fn utterance_hash(normalized: &str) -> String` (sha256 hex)
  - `fn preview_redacted(raw: &str) -> String` (max 80 chars + basic masking)
  - Optional: disable preview entirely in strict settings

**Acceptance**
- You can construct an IntentEventRow without leaking raw args/PII.

---

## 3) Wire emission at the single orchestrator exit

### 3.1 Add a “telemetry emit” hook in orchestrator
File: `rust/src/agent/orchestrator.rs`

- [ ] At the end of `handle_utterance(...)` (one place!), build `IntentEventRow` from:
  - `IntentTrace` (already contains candidates, selection_source, macro flags, etc.)
  - OrchestratorContext (session_id, actor_id, entrypoint, scope, subject_ref)
  - Outcome (matched/clarify/no_allowed/error)
  - run_sheet entry id + dsl hash (if staged)

- [ ] Call `telemetry::store::insert_intent_event(...)`
  - Wrap in `match` and set `trace.telemetry_persisted=true/false` (add a flag)
  - Never panic on telemetry failure

**Acceptance**
- Every orchestrator call produces exactly one `intent_events` row (or logs a failure flag).

### 3.2 Ensure no other module inserts intent_events
- [ ] Add a static guard test:
  - ripgrep for `insert_intent_event` usage outside orchestrator module (excluding tests)

---

## 4) Capture the “chat prompt / intent guesses / response loop” for review

You want to review the conversational loop and tune it. There are two useful layers:

### 4.1 Persist the orchestrator’s “agent response packet” snapshot (optional)
If you want to review the *paragraph output* and see how it relates to intent choices:

- [ ] Add `agent.chat_events` table (optional v0.1):
  - `event_id`, `ts`, `session_id`, `turn_id`, `assistant_message_hash`, `assistant_message_preview`, `intent_event_id`
- [ ] Link chat output to the intent_event row that produced it.

**PII note:** store hashes + short redacted previews, or store nothing and just keep a pointer to the run_sheet entry.

### 4.2 Store the prompt template version
- [ ] Add `prompt_version` string field into intent_events (or chat_events):
  - record which prompt pack / system prompt version was used
  - this is crucial for A/B tuning

**Acceptance**
- You can compare behavior across prompt versions without ambiguity.

---

## 5) Review workflow (what you do with the data)

### 5.1 Add 5 starter queries (SQL views or CLI)
Create:
- `agent.v_intent_top_clarify_verbs` (verbs causing clarify_verb most)
- `agent.v_intent_semreg_overrides` (cases where forced_verb_fqn differs)
- `agent.v_intent_semreg_denies` (denied verbs frequency)
- `agent.v_intent_arg_fill_failures` (if you later log missing args)
- `agent.v_intent_macro_denies` (macro expansions producing denied verbs)

Optionally expose via an internal endpoint `/api/admin/telemetry/*` guarded by PolicyGate.

### 5.2 Add a small CLI command
- [ ] `xtask telemetry report --since 7d`
  - prints top failure modes and hottest verbs

**Acceptance**
- You can see patterns in minutes, without manual DB spelunking.

---

## 6) The tuning lifecycle (yes — this is the lifecycle)

Here’s the pragmatic loop:

1) **Run** the system normally (SemReg snapshots stable, orchestrator emits telemetry)
2) **Review** weekly (or after major changes):
   - clarify rates, SemReg deny rates, forced regen rates, macro denies, execution failures
3) **Tune “semi-statically”** (i.e., controlled changes):
   - update boost weights tables (runtime ranking layer)
   - update attribute aliases map
   - refine prompt templates / arg schemas
   - update macro definitions
4) **Publish** new SemReg snapshots only when the *meaning* changes (contracts, types, governance)
5) Repeat

SemReg remains “policy + contracts”. Telemetry drives “ranking + usability improvements”.

---

## 7) Optional: runtime ranking layer (non-snapshot) for boosts

If you want adaptive tuning without republishing SemReg:

- [ ] Add `agent.verb_boost_rules` table:
  - (scope_bucket, subject_kind, phase, verb_fqn, boost_weight, reason)
- [ ] Orchestrator applies these boosts *in addition to* SemReg.
- [ ] Update boosts via batch job from telemetry.

---

## 8) Tests

- [ ] Unit: utterance hash is stable across whitespace changes
- [ ] Integration: orchestrator writes exactly one intent_events row per call
- [ ] Regression: telemetry failure does not fail the request (but sets trace flag)
- [ ] Static guard: only orchestrator writes intent_events

---

## “Done” checklist

- [ ] Migration applied: `agent.intent_events`
- [ ] Orchestrator emits an intent_event per utterance (best-effort)
- [ ] PII-safe storage (hash + redacted preview optional)
- [ ] Reports/queries exist to review behavior
- [ ] Prompt version is recorded for comparisons
- [ ] Static guard prevents telemetry writes from spreading to side paths

