# Rust Workspace Visibility Audit

## Scope

Read-only audit of the `ob-poc` Rust workspace with a narrow goal:

- reduce unnecessary `pub`
- shrink crate/module visible surface area
- improve encapsulation and visibility hygiene
- preserve current behaviour and architecture

This review is conservative. It focuses on high-confidence reductions and avoids recommending broad churn where visibility may reflect real cross-crate contracts.

## Validation basis

The audit was grounded in:

- workspace membership from `cargo metadata`
- successful `cargo check`
- `cargo clippy --workspace --all-targets --all-features -D warnings`
- `cargo check` with visibility-focused lints:
  - `-W unreachable_pub`
  - `-W private_interfaces`
  - `-W private_bounds`
  - `-W unnameable_types`
- direct code reference searches across the repository

Note: full Clippy is currently blocked by a pre-existing non-visibility issue:

- [rust/src/api/observatory_routes.rs:1138](/Users/adamtc007/Developer/ob-poc/rust/src/api/observatory_routes.rs:1138)
  - `clippy::items_after_test_module`

## Overall assessment

The workspace is uneven from a visibility perspective.

Healthy pattern:

- small library crates that keep modules private and expose a deliberate facade via `pub use`

Leaky pattern:

- crates that already have a facade, but still keep most internal modules public
- the root `ob-poc` crate, which exposes deep subsystem trees almost wholesale

The crate structure is broadly workable, but internals are leaking in several places. The main opportunity is not redesign; it is to tighten boundaries around crates that already expose enough through root re-exports.

## Highest-value tightening areas

1. `entity-gateway`
   - Strong facade already exists.
   - Internal modules are still public without evidence they need to be.

2. `ob-workflow`
   - Same pattern as `entity-gateway`.
   - Root re-exports already define most of the intended API.

3. `dsl-lsp`
   - Public module tree appears to be driven largely by test convenience.
   - A narrow test-facing export would allow internal modules to become private.

4. `sem_os_server` and `sem_os_harness`
   - Server/test-support internals are exposed more broadly than their current usage justifies.

5. Root `ob-poc`
   - Particularly `graph`, `session`, and other subsystem trees.
   - Surface area is much wider than the observed external use.

## Crate-by-crate findings

### `dsl-core`

Role:

- shared parser/AST/config crate

Assessment:

- broad public surface, but multiple downstream crates appear to use those namespaces directly
- not a top priority for tightening without a follow-up compatibility pass

Recommendation:

- defer large reductions

### `dsl-lsp`

Role:

- LSP implementation crate

Likely true API:

- `DslLanguageServer`
- encoding helpers
- entity client types

Main issue:

- `analysis`, `handlers`, `server`, and related modules are public
- one integration test uses `dsl_lsp::handlers::diagnostics::analyze_document`

Recommendation:

- replace deep test access with a narrow exported helper
- then make internal modules private

### `entity-gateway`

Role:

- gRPC entity-resolution service and related search/index/config types

Likely true API:

- root re-exported config/index/search/service types
- generated `proto` surface

Main issue:

- `config`, `index`, `refresh`, `search_engine`, `search_expr`, and `server` are public despite already being facaded

Recommendation:

- keep `proto` public
- make the rest private and rely on root `pub use`

### `governed_query_proc`

Role:

- proc-macro crate

Likely true API:

- `#[governed_query]`

Main issue:

- internal helper modules/types are `pub` even though they are not exported from the crate root
- compiler confirms multiple `unreachable_pub` cases

Recommendation:

- reduce helper visibility to `pub(crate)` or private

### `inspector-projection`

Role:

- projection schema and generators for inspector views

Likely true API:

- schema types
- validation API
- selected generators re-exported at crate root

Main issue:

- `generator` module is public even though consumers appear to rely on root exports
- deal generator input types are publicly re-exported from `generator`, but not from crate root, and no usage was found

Recommendation:

- make `generator` private
- keep root re-exports only
- reduce clearly internal constants like `MAX_SCHEMA_VERSION`

### `ob-agentic`

Role:

- LLM/intent/lexicon crate

Likely true API:

- root facade plus selected lexicon pipeline types

Main issue:

- lexicon internals contain compiler-confirmed `unreachable_pub` and `unnameable_types`
- public config structs expose nested types that are not properly surfaced

Recommendation:

- tighten obvious helpers first
- treat lexicon config/type exposure as a second-pass structural cleanup

### `ob-execution-types`

Role:

- small shared execution type crate

Assessment:

- already uses a good private-modules-plus-reexports pattern

Recommendation:

- no immediate action

### `ob-poc`

Role:

- root application/library crate

Likely true API:

- selected types/services used by `ob-poc-web`, `xtask`, and tests

Main issue:

- far too many `pub mod` declarations at crate root
- many deep subsystem modules appear public for convenience rather than as intentional API

Notable areas:

- `graph`
- `session`
- `api`
- `agentic` shim duplicating `ob-agentic` exposure

Recommendation:

- tighten subsystem-by-subsystem
- start with facaded modules such as `graph`
- avoid one-shot root-level privatization

### `ob-poc-macros`

Role:

- proc-macro helpers

Main issue:

- helper functions in private child modules are `pub`

Recommendation:

- reduce to `pub(crate)`

### `ob-poc-types`

Role:

- shared DTO crate for cross-boundary/server-UI payloads

Assessment:

- wide surface is mostly intentional
- multiple external consumers use module-qualified paths directly

Recommendation:

- do not aggressively tighten in the first pass

### `ob-semantic-matcher`

Role:

- semantic matching and feedback crate

Assessment:

- some direct submodule use exists
- not enough evidence for broad tightening

Recommendation:

- limit changes to compiler-confirmed low-risk cases only

### `ob-templates`

Role:

- template facade crate

Assessment:

- healthy private-module pattern

Recommendation:

- no action

### `ob-workflow`

Role:

- workflow definitions, engine, queue, and related types

Likely true API:

- root re-exported workflow/document/task types

Main issue:

- `blob_store`, `cargo_ref`, `document`, `task_queue`, `listener` are public modules despite root re-exports already covering the usable API
- compiler also reports unnameable helper types in `definition.rs`

Recommendation:

- privatize modules first
- leave the helper-type exposure mismatch for a second pass

### `playbook-core` / `playbook-lower`

Assessment:

- already use private modules plus reexports

Recommendation:

- no action

### `sem_os_client`

Role:

- narrow trait boundary with concrete client implementations

Assessment:

- `http` and `inprocess` are real construction surfaces used by downstream crates

Recommendation:

- keep as-is

### `sem_os_core`

Role:

- shared core domain model/ports for Sem OS crates

Assessment:

- broad surface, but much of it appears genuinely cross-crate

Recommendation:

- defer tightening unless a dedicated Sem OS API pass is planned

### `sem_os_harness`

Role:

- test harness crate

Main issue:

- support modules are public with no external consumers found
- support struct fields are public despite crate-local use only

Recommendation:

- make helper modules private
- reduce helper struct field visibility

### `sem_os_obpoc_adapter`

Assessment:

- some broad exposure, but not enough evidence for aggressive tightening in this pass

Recommendation:

- defer except for compiler-confirmed low-risk cases

### `sem_os_postgres`

Role:

- Postgres adapter crate

Main issue:

- some internals are broader than needed
- however, `PgStores` fields are heavily used across the workspace

Recommendation:

- keep `PgStores` shape for now
- consider tightening modules where not used externally

### `sem_os_server`

Role:

- standalone REST server library + binary

Main issue:

- `handlers` and `error` are public without evidence of external consumers
- `router` and JWT middleware have real external use from binary/tests

Recommendation:

- make `handlers` and `error` private
- keep router/middleware surfaces that are actually used

### `xtask`

Role:

- binary-only developer tool

Main issue:

- very high number of `pub` functions/structs in bin-only modules

Recommendation:

- low-risk cleanup to `pub(crate)`/private
- architecturally low priority, but easy hygiene win

## High-confidence reductions

| File | Item | Current | Recommended | Reason | Risk |
|---|---|---|---|---|---|
| [rust/crates/entity-gateway/src/lib.rs](/Users/adamtc007/Developer/ob-poc/rust/crates/entity-gateway/src/lib.rs:57) | `config`, `index`, `refresh`, `search_engine`, `search_expr`, `server` | `pub mod` | `mod` | root facade already reexports the useful API; no direct external deep-path use found | Low |
| [rust/crates/ob-workflow/src/lib.rs](/Users/adamtc007/Developer/ob-poc/rust/crates/ob-workflow/src/lib.rs:52) | `blob_store`, `cargo_ref`, `document`, `task_queue`, `listener` | `pub mod` | `mod` | crate root already reexports the intended public API | Low |
| [rust/crates/dsl-lsp/src/lib.rs](/Users/adamtc007/Developer/ob-poc/rust/crates/dsl-lsp/src/lib.rs:6) | `analysis`, `encoding`, `entity_client`, `handlers`, `server` | `pub mod` | `mod` plus narrow test-facing export | only observed deep-path consumer is an integration test | Medium |
| [rust/crates/sem_os_server/src/lib.rs](/Users/adamtc007/Developer/ob-poc/rust/crates/sem_os_server/src/lib.rs:15) | `error` | `pub mod` | `mod` | no external use found | Low |
| [rust/crates/sem_os_server/src/lib.rs](/Users/adamtc007/Developer/ob-poc/rust/crates/sem_os_server/src/lib.rs:16) | `handlers` | `pub mod` | `mod` | internal-only router implementation detail | Low |
| [rust/crates/sem_os_harness/src/lib.rs](/Users/adamtc007/Developer/ob-poc/rust/crates/sem_os_harness/src/lib.rs:15) | `db`, `permissions`, `projections` | `pub mod` | `mod` | no external consumer found | Low |
| [rust/crates/sem_os_harness/src/db.rs](/Users/adamtc007/Developer/ob-poc/rust/crates/sem_os_harness/src/db.rs:14) | `IsolatedDb::pool`, `IsolatedDb::dbname` | `pub` fields | private / `pub(crate)` | only crate-local use observed | Low |
| [rust/crates/inspector-projection/src/lib.rs](/Users/adamtc007/Developer/ob-poc/rust/crates/inspector-projection/src/lib.rs:62) | `generator` | `pub mod` | `mod` | consumers appear to use root exports, not module path | Low |
| [rust/crates/inspector-projection/src/generator/mod.rs](/Users/adamtc007/Developer/ob-poc/rust/crates/inspector-projection/src/generator/mod.rs:19) | deal input/generator reexports | `pub use` | `pub(crate) use` or remove | no consumer found outside module/tests | Low |
| [rust/crates/inspector-projection/src/validate.rs](/Users/adamtc007/Developer/ob-poc/rust/crates/inspector-projection/src/validate.rs:16) | `MAX_SCHEMA_VERSION` | `pub const` | `pub(crate) const` | compiler-confirmed `unreachable_pub` | Low |
| [rust/crates/ob-poc-macros/src/id_type.rs](/Users/adamtc007/Developer/ob-poc/rust/crates/ob-poc-macros/src/id_type.rs:7) | `derive_id_type_impl` | `pub fn` | `pub(crate) fn` | internal helper in proc-macro crate | Low |
| [rust/crates/ob-poc-macros/src/register_op.rs](/Users/adamtc007/Developer/ob-poc/rust/crates/ob-poc-macros/src/register_op.rs:7) | `register_custom_op_impl` | `pub fn` | `pub(crate) fn` | internal helper in proc-macro crate | Low |
| [rust/crates/governed_query_proc/src/cache.rs](/Users/adamtc007/Developer/ob-poc/rust/crates/governed_query_proc/src/cache.rs:22) | `load_cache` | `pub fn` | `pub(crate) fn` | compiler-confirmed internal helper | Low |
| [rust/crates/governed_query_proc/src/parse.rs](/Users/adamtc007/Developer/ob-poc/rust/crates/governed_query_proc/src/parse.rs:8) | `GovernedQueryArgs` | `pub struct` | `pub(crate)` | proc-macro internal type | Low |
| [rust/crates/governed_query_proc/src/registry_types.rs](/Users/adamtc007/Developer/ob-poc/rust/crates/governed_query_proc/src/registry_types.rs:13) | internal enums/cache types | `pub` | `pub(crate)` | proc-macro internals only; compiler flagged many | Low |
| [rust/crates/ob-agentic/src/lexicon/db_resolver.rs](/Users/adamtc007/Developer/ob-poc/rust/crates/ob-agentic/src/lexicon/db_resolver.rs:19) | `DEFAULT_GATEWAY_ADDR` | `pub const` | `pub(super) const` | compiler-confirmed `unreachable_pub` | Low |
| [rust/crates/ob-agentic/src/lexicon/intent_parser.rs](/Users/adamtc007/Developer/ob-poc/rust/crates/ob-agentic/src/lexicon/intent_parser.rs:1107) | `parse_intent` | `pub fn` | `pub(super) fn` | helper below public `parse_tokens` API | Low |
| [rust/src/graph/mod.rs](/Users/adamtc007/Developer/ob-poc/rust/src/graph/mod.rs:19) | graph child modules | `pub mod` | `mod` | root already reexports graph API; no external deep-path use found | Medium |
| [rust/src/session/mod.rs](/Users/adamtc007/Developer/ob-poc/rust/src/session/mod.rs:19) | selected session child modules | `pub mod` | `mod` | most useful types already reexported from `session`; external deep-path use is narrow | Medium |

## Structural hotspots

These are real design-pressure points, not just stray `pub` keywords:

### 1. Root `ob-poc` crate as oversized public barrel

[rust/src/lib.rs](/Users/adamtc007/Developer/ob-poc/rust/src/lib.rs:20)

Issue:

- root crate exports large subsystem trees directly
- hard to distinguish stable surface from internal plumbing

Implication:

- future visibility tightening should happen per subsystem, not as a single root sweep

### 2. `ob-workflow` public API exposes helper types that are not facaded

[rust/crates/ob-workflow/src/definition.rs](/Users/adamtc007/Developer/ob-poc/rust/crates/ob-workflow/src/definition.rs:120)

Issue:

- `TriggerDef` contains `Vec<TriggerCondition>`
- `RequirementDef::Conditional` contains `ConditionalCheck`
- compiler reports these as publicly reachable but not nameable

Implication:

- this needs a deliberate API decision, not just privatization

### 3. `ob-agentic` lexicon config exposure mismatch

[rust/crates/ob-agentic/src/lexicon/loader.rs](/Users/adamtc007/Developer/ob-poc/rust/crates/ob-agentic/src/lexicon/loader.rs:116)

Issue:

- `LexiconConfig` publicly exposes `LawEntry` and `ModifiersConfig`
- those types are not properly surfaced at the crate facade level

Implication:

- either the config graph becomes part of the public facade, or the API should be narrowed

### 4. `taxonomy` combinator/source exposure mismatch

[rust/src/taxonomy/combinators/source.rs](/Users/adamtc007/Developer/ob-poc/rust/src/taxonomy/combinators/source.rs:14)

Issue:

- public combinator traits use `SourceItem`
- `taxonomy` root does not facade `SourceItem`

Implication:

- another genuine unnameable-type boundary problem

### 5. `dsl_v2` macro/expansion subsystem

- [rust/src/dsl_v2/expansion/types.rs](/Users/adamtc007/Developer/ob-poc/rust/src/dsl_v2/expansion/types.rs:99)
- [rust/src/dsl_v2/macros/expander.rs](/Users/adamtc007/Developer/ob-poc/rust/src/dsl_v2/macros/expander.rs:130)
- [rust/src/dsl_v2/macros/schema.rs](/Users/adamtc007/Developer/ob-poc/rust/src/dsl_v2/macros/schema.rs:317)

Issue:

- public helper types are trapped below partially private or non-facaded module structure

Implication:

- likely needs a small intentional facade before visibility can be tightened cleanly

## Refactoring strategy

### Guiding principles

- prefer leaf/internal modules first
- prefer `pub` -> `pub(crate)` before full privatization where cross-crate uncertainty exists
- use crate-root facades deliberately
- avoid signature redesign unless required by unnameable/public-shape mismatches

### Safe implementation order

#### Phase 1: compiler-confirmed helper cleanup

Targets:

- `ob-poc-macros`
- `governed_query_proc`
- optional `xtask` cleanup

Why first:

- high confidence
- almost no external API risk
- removes obvious noise quickly

Validation:

- `cargo check`

#### Phase 2: facade-tightening in leaf library crates

Targets:

- `entity-gateway`
- `ob-workflow`
- `inspector-projection`
- `sem_os_harness`

Why next:

- these crates already have usable root facades
- reductions should be mostly module-visibility changes, not logic changes

Validation:

- `cargo check`
- targeted follow-up search for broken deep-path imports

#### Phase 3: server/test-support cleanup

Targets:

- `sem_os_server`
- `dsl-lsp`

Notes:

- `dsl-lsp` needs test access redesigned before broad module privatization
- `sem_os_server` should keep only the externally used surfaces (`router`, JWT config/middleware where required)

Validation:

- `cargo check`

#### Phase 4: root `ob-poc` low-risk subsystem tightening

Targets:

- `graph`
- selected `session` modules

Why:

- observed external use is narrower than current exposure

Caution:

- do this subsystem-by-subsystem, not at `lib.rs` in one pass

Validation:

- `cargo check`
- re-run workspace searches for `ob_poc::graph::...` and `ob_poc::session::...`

#### Phase 5: structural API repairs

Targets:

- `ob-workflow` helper-type exposure
- `ob-agentic` lexicon config types
- `taxonomy::SourceItem` and related combinator exposure
- `dsl_v2` expansion/macro helper types

Why last:

- these may require small API-shape decisions, not just modifier changes

Validation:

- `cargo check`
- full Clippy once the existing Clippy blocker is fixed

## Suggested acceptance criteria

- no unnecessary `pub` remains in private child modules
- crate facades explicitly define the intended public API
- no deep internal module exposure remains without a concrete consumer
- no public item exposes helper types that are not nameable from the crate facade
- test-only access does not force production modules to stay public
- `cargo check` stays green
- once baseline Clippy is repaired, `cargo clippy --workspace --all-targets --all-features` stays green

## Do not touch yet

These should stay out of the first implementation pass:

- `ob-poc-types`
  - real shared DTO crate with direct external module-path consumers

- `sem_os_core`
  - broad, but much of the surface appears genuinely cross-crate

- `sem_os_client::http` and `sem_os_client::inprocess`
  - real client construction surfaces used in downstream crates/tests

- `entity-gateway::proto`
  - generated gRPC surface with real external use

- `sem_os_postgres::PgStores` public fields
  - many current consumers depend on field access directly

- `ob-workflow` trigger/conditional helper types
- `ob-agentic` lexicon config nested types
- `taxonomy` combinator/source public helper types
- `dsl_v2` macro/expansion helper types
  - all of the above need deliberate facade choices before visibility reduction

## Peer review prompts

Questions worth resolving before implementation:

- Should `entity-gateway` and `ob-workflow` treat crate-root reexports as the only supported API?
- Do we want test-only helper exports in `dsl-lsp`, or should tests move inside the crate?
- Should the root `ob-poc` crate continue to behave like a broad integration library, or should it intentionally narrow around selected facades?
- For unnameable-type cases, should we expose the helper type properly or make the containing API less structural?

## Recommended next step

For a low-risk first refactor pass, implement only:

1. `ob-poc-macros`
2. `governed_query_proc`
3. `entity-gateway`
4. `ob-workflow`
5. `sem_os_harness`

That slice is small enough to validate with `cargo check`, should remove a meaningful amount of visibility noise, and does not require re-architecting the workspace.
