//! ob-poc-authoring — editor / authoring-facing toolkit.
//!
//! ## Capability claim
//!
//! The toolkit that presents the registry to a human (or LLM) author
//! shaping it and turns author input into structured proposals: a
//! DecisionPacket-based clarification UX, a lexicon for fast vocabulary
//! lookup, the macro registry, schema lint diagnostics, the data
//! dictionary (`AttributeId` and friends), display-noun translation for
//! operator-facing wire output, the feedback inspector that turns
//! failures into reproducible cases, and language packs.
//!
//! This is the **tooling tier** sandwiched between the runtime that
//! executes author proposals (`sem_os_*`) and the boundary that contracts
//! with the author's editor (`ob-poc-boundary`).
//!
//! ## Anti-charter
//!
//! - NOT the runtime that executes author proposals. Changesets,
//!   governance gates, snapshot persistence — those live in `sem_os_*`.
//! - NOT the ACP/workbook protocol. The boundary contract with the
//!   editor lives in `ob-poc-boundary`.
//! - NOT the orchestrator. The pipeline that USES these tools (search
//!   verbs, propose phrases, route clarifications) lives in `ob-poc`.
//!
//! ## Public surface contract (post Phase 5)
//!
//! Top-level modules in this crate will be:
//! - `clarify` — DecisionPacket + answer kinds + confirmation UX.
//! - `lexicon` — bincode-backed in-memory vocabulary lookup
//!   (`LexiconService`, `LexiconSnapshot`).
//! - `macros` — operator macro registry + macro definition schema.
//! - `lint` — schema-validation diagnostics for verb / macro YAML.
//! - `data_dictionary` — `AttributeId` typed identifier + attribute
//!   metadata.
//! - `display_nouns` — internal-vocabulary → operator-vocabulary
//!   translation table (`translate_json`, `translate_string`,
//!   `DisplayNounTranslator`).
//! - `feedback` — on-demand failure inspector + classifier + redactor.
//! - `language_pack` — bundled author-facing copy + region/locale
//!   selection.
//!
//! ## Dependency discipline
//!
//! Must depend only on `ob-poc-types`, `ob-poc-diagnostics` (for the
//! events::* surface consumed by feedback), `ob-poc-macros` (for the
//! `#[derive(IdType)]` proc macro used by data_dictionary), and
//! primitives (`chrono`, `serde`, `uuid`, `bincode`, `regex`,
//! `unicode-normalization`, `smallvec`, `sha2`, `hex`, `anyhow`,
//! `thiserror`, `serde_json`, `serde_yaml`). DB-coupled tools gate
//! `sqlx` behind the `database` feature. Must NOT depend on
//! `dsl-core`, `dsl-runtime`, `sem_os_*`, `ob-poc-boundary`,
//! `ob-poc-sage`, `ob-poc-journey`, or any execution-tier surface.
//!
//! ## Migration status (2026-05-13)
//!
//! This crate is the destination for Phase 5 of the capability-crate
//! restructure (`docs/todo/capability-crate-restructure-v1.md`). Phase 5
//! moves eight modules out of `ob-poc-boundary::*` into this crate.
#![deny(unreachable_pub)]

// Phase 5.1 (2026-05-13): three independent authoring modules
// relocated from ob-poc-boundary. Charter-aligned: each module is an
// author-facing tool, not execution logic.
//   - clarify (2 files) — DecisionPacket + confirmation UX.
//   - data_dictionary (2 files) — AttributeId typed identifier
//     (uses #[derive(IdType)] from ob-poc-macros).
//   - display_nouns (1 file) — internal-vocab → operator-vocab
//     translation table.
pub mod clarify;
pub mod data_dictionary;
pub mod display_nouns;

// Phase 5.2 (2026-05-13): vocabulary / definitions / diagnostics
// authoring modules relocated from ob-poc-boundary. All three
// self-contained (zero external crate refs beyond ob-poc-types).
//   - lexicon (5 files) — bincode-backed in-memory vocabulary
//     (LexiconService + LexiconSnapshot + compiler + types).
//   - macros (3 files) — operator macro registry + definition schema.
//   - lint (3 files) — schema-validation diagnostics for verb/macro YAML.
pub mod lexicon;
pub mod lint;
pub mod macros;

// Phase 5.3 (2026-05-13): final Phase 5 slice — on-demand failure
// inspector. language_pack was evaluated and rejected: it uses
// sem_os_policy::domain_pack types and five intra-boundary modules
// import it; it stays in ob-poc-boundary where those deps are allowed.
//   - feedback (7 files) — failure classifier + inspector + redactor
//     + repro + todo + types. Consumes ob-poc-diagnostics::events::*.
#[cfg(feature = "database")]
pub mod feedback;
