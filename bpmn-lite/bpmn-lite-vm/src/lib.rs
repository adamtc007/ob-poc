//! BPMN-Lite bytecode interpreter.
//!
//! Single-concern crate that owns the fiber dispatch + instruction
//! execution + wait/race/loop primitives. Kept separate from
//! `bpmn-lite-engine` (the orchestrator) so the interpreter's
//! surface area stays locked once the instruction set is stable.
//!
//! Empty at Phase 1 skeleton — the interpreter lives in
//! `bpmn-lite-core/src/vm.rs` until the Phase 2 migration slice
//! (`vm.rs → bpmn-lite-vm`).
