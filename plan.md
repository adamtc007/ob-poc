# SemOS Data Management Handoff Plan

## Session Summary (2026-03-05)
The pipeline investigation is largely complete. Main false-fail roots were found and fixed, and unified session input is working end-to-end. Current remaining gap is **product behavior** in SemOS Data Management mode.

Your clarification is now the top requirement:
- **Data Management is about data structures + semantics**
- It is **not** about record content retrieval
- It should **not** require `deal-id`, `cbu-id`, or `entity-id` for base exploratory prompts

## What Is Already Fixed
- Unified input path is active: `POST /api/session/:id/input`
- SemReg candidate truncation raised (was dropping valid verbs)
- Exact phrase fallback from `dsl_verbs.yaml_intent_patterns` is wired and active
- Domain inference bug fixed (`show ...` no longer always forced to `view` before data domains)
- Seed overlap reduced so `show me deal record` now resolves to `deal.list` instead of `deal.get`
- Build status: `cargo check -p ob-poc` passes

## Current Verified Behavior
Using fresh sessions:
- `data management` -> scopes correctly
- `show me deal record` -> `Staged: (deal.list)`
- `show me products` -> `Staged: (product.list)`
- `show me documents` -> still asks for `entity-id`
- `show me CBU` -> still asks for `cbu-id`

This confirms routing is improved, but the mode is still partially content-oriented.

## Remaining Problem Statement
In `semos-data-management`, prompts like:
- "show me deal record"
- "show me CBU"
- "show me documents"
- "show me products"
should default to **model/schema semantics**, not data-instance verbs requiring primary keys.

## Locked Contract (2026-03-06)
- In `semos-data-management` and `semos-data`, noun-only exploratory prompts resolve to structure semantics first.
- The default target for these prompts is Semantic Registry metadata for the inferred domain: schema, fields, relationships, and available verbs.
- Default behavior must not route directly to ID-requiring `*.get` or equivalent content verbs unless the user explicitly instance-targets the request.
- Explicit instance targeting currently includes direct `*-id` tokens, `id:` / `for id` forms, or `@`-style handles in the utterance.
- Baseline deterministic mappings are:
  - `show me deal record` -> registry/schema semantics for `deal`
  - `show me CBU` -> registry/schema semantics for `cbu`
  - `show me documents` -> registry/schema semantics for `document`
  - `show me products` -> registry/schema semantics for `product`

## Next Session Plan

## Phase 1: Define Explicit Data-Management Contract
- [x] Write/agree a contract doc for `semos-data-management` intent behavior.
- [x] Add hard rule: in this mode, noun-only exploration prompts map to structure intents first.
- [x] Document forbidden default behavior: no immediate `*.get` requiring IDs unless user explicitly asks for a specific instance.

## Phase 2: Introduce Structure-Semantics Intent Layer
- [x] Add a mode-aware mapper before normal verb ranking:
  - `show me deal record` -> structure view for `deal` domain
  - `show me cbu` -> structure view for `cbu` domain
  - `show me documents` -> structure view for `document` domain
  - `show me products` -> structure view for `product` domain
- [x] Implement as deterministic rewrite to dedicated DSL verbs (preferred) or a dedicated adapter branch.

## Phase 3: Add/Use Dedicated Schema Verbs
- [x] Create or standardize DSL verbs for structure inspection (examples):
  - `schema.domain.describe`
  - `schema.entity.describe`
  - `schema.entity.list-fields`
  - `schema.entity.list-relationships`
  - `schema.entity.list-verbs`
- [x] Back these verbs from registry/DSL metadata (`dsl_verbs`, arg contracts, metadata), not business records.

## Phase 4: Enforce Separation of Concerns
- [x] In `semos-data-management`, demote or block content verbs requiring IDs unless the utterance contains explicit instance targeting.
- [x] Keep content verbs available in other modes or when user explicitly requests instance-level operations.

## Phase 5: Tests + Trace Proof
- [x] Add intent tests for the four baseline utterances above.
- [x] Add assertions that responses are schema/semantic and do not request instance IDs by default.
- [x] Add call-stack trace artifact proving these prompts still flow through `/api/session/:id/input` only.

## Implementation Touchpoints
- `rust/src/mcp/intent_pipeline.rs`
- `rust/src/agent/orchestrator.rs`
- `rust/src/mcp/verb_search.rs`
- `rust/config/verbs/*.yaml` (new schema verbs)
- `rust/src/api/agent_service.rs` (mode contract wiring)
- tests under `rust/tests/*` (new semos data-management suite)

## Startup / Validation Commands
- Start backend:
  - `cd rust && DATABASE_URL=postgresql:///data_designer cargo run -p ob-poc-web`
- Validation harness pattern:
  - Create fresh session with `workflow_focus=semantic-os`
  - Send utterances via `/api/session/:id/input`
  - Verify schema/semantic responses for the four baseline prompts

## First Task Next Session
Implement Phase 1 + Phase 2 in one cut:
- lock contract,
- add deterministic mode-aware mapping,
- prove the four prompts no longer request IDs by default.
