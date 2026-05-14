//! BPMN-Lite engine.
//!
//! The orchestration crate — wires `bpmn-lite-compiler` (compile),
//! `bpmn-lite-vm` (execute), and `bpmn-lite-store` (persist)
//! together behind a single `BpmnLiteEngine` facade. Owns the
//! process lifecycle API (`start`, `signal`, `cancel`, `inspect`,
//! `complete_job`, `fail_job`) and the background scheduler
//! (`tick_*`).
//!
//! Empty at Phase 1 skeleton — `BpmnLiteEngine` and its `impl`
//! block live in `bpmn-lite-core/src/engine/{mod,tests}.rs`
//! until the Phase 2 migration slice. That slice also inverts
//! the engine→authoring edge: the YAML/DTO/publish methods
//! (`compile_from_dto`, `compile_from_yaml`, `compile_and_publish`)
//! leave the engine entirely and become free functions in
//! `bpmn-lite-authoring`, which calls into `bpmn-lite-compiler`
//! directly. The engine keeps only `compile(bpmn_xml)` plus a new
//! `compile_program(&CompiledProgram)` entry point for callers
//! that already hold an artefact.
