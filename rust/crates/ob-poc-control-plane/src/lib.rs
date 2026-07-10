//! ob-poc-control-plane — owns the execution-control decision for AI-led
//! governed execution (EOP-VS-CONTROLPLANE-001 / EOP-PLAN-CONTROLPLANE-001).
//!
//! T1 (this tranche set) lands the crate skeleton only: proof-carrying
//! types, the `ControlPlaneDecision` model, the `ExecutionEnvelope` sealed
//! constructor, and a declared-dependency-graph evaluator with all 14 gates
//! stubbed. No gate performs real validation yet — that starts at T2, where
//! each gate module gains an adapter over the existing validator it wraps
//! (see `docs/research/control-plane-ownership-ledger.md` for the C-0xx
//! disposition driving each adapter).
//!
//! This crate must not depend on any execution-tier crate (§9.1 non-goals):
//! it does not own LLM prompting, DSL parsing, DAG authoring, SemOS
//! authoring, runtime state mutation, or re-implementations of validator
//! logic owned elsewhere. It owns only the decision.
#![deny(unreachable_pub)]

pub mod authority_gate;
pub mod dag_proof;
pub mod entity_binding;
pub mod evidence_gate;
pub mod gate;
pub mod intent_admission;
pub mod pack_resolution;
pub mod snapshot;
pub mod stp_classifier;
pub mod write_set;

pub mod audit;
pub mod exceptions;
pub mod metrics;
pub mod versioning;

pub mod decision;
pub mod envelope;
pub mod proof;
