//! BPMN-Lite compiler.
//!
//! Owns the pipeline that turns BPMN 2.0 XML (or a fed-in IR graph
//! from `bpmn-lite-authoring`'s YAML/DTO path) into a verified
//! `CompiledProgram` of bytecode the VM can execute.
//!
//! Empty at Phase 1 skeleton — the parser, IR, lowering, and
//! verifier modules live in `bpmn-lite-core/src/compiler/*` until
//! the Phase 2 migration slice (`compiler/* → bpmn-lite-compiler`).
