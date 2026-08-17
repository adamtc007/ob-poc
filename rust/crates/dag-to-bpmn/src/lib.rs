//! Compiles one `dag_taxonomies` state-machine slot into a bpmn-lite DSL
//! workflow template.
//!
//! # Why text, not a `RailwayGraph` builder
//!
//! `dsl-bpmn-frontend::RailwayGraph` has no public constructor
//! (`RailwayGraph::empty()` is `pub(crate)`, the crate is
//! `#![deny(unreachable_pub)]`) — the only way to produce one is
//! `assemble()` from a parsed `AtomBag`, which requires DSL source text.
//! This crate emits that text and validates it through
//! `dsl_migrate_verify::compile_to_spec`, the same function the production
//! process registry (`ob-poc-web::process_registry`) uses to compile stored
//! `dsl_source` rows at startup. That gives this compiler
//! `dsl-bpmn-frontend`'s existing structural validation (reachability,
//! unterminated paths, gateway fan-out, duplicate names) for free, and
//! produces a human-readable `.dsl` artifact rather than an opaque struct.
//!
//! # Mapping (mechanical, not interpretive)
//!
//! See `docs/todo/EOP-DD-DAGBPMN-001_DAG-Taxonomy-to-BPMN-Template-Compiler_v0.1.md`
//! for the full ratified mapping. Summary:
//!
//! - The slot's one `entry: true` state -> a bare `start-event` node.
//! - Each state in `terminal_states` -> a bare `end-event` node.
//! - Each distinct `(to, via)` transition -> one `service-task` node bound
//!   to `via` via `:verb (invoke <fqn>)`. Non-entry, non-terminal states are
//!   *not* modeled as their own node — they're the implicit waypoint
//!   between two transition-task nodes.
//! - A state with more than one distinct outgoing `(to, via)` gets an
//!   `exclusive` gateway inserted before the branch (BPMN structurally
//!   requires this before diverging flows).
//! - A state with multiple incoming transitions needs no gateway — multiple
//!   edges converging on one node is valid BPMN (implicit OR-join).
//! - `from: "(any non-terminal)"` expands deterministically to every state
//!   not in `terminal_states`.
//! - Unsupported shapes (list-valued `via`, free-text `via` with no real
//!   verb FQN, zero/multiple entry states, an unresolvable `from`/`to`)
//!   fail loud with [`ShapeError`], naming the exact transition. Nothing is
//!   silently dropped or guessed.

#![deny(unreachable_pub)]

mod compile;
mod emit;
mod error;

pub use compile::{compile_slot, CompiledTemplate};
pub use error::{DagToBpmnError, ShapeError};
