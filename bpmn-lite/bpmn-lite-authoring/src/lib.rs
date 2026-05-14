//! BPMN-Lite authoring.
//!
//! Owns the YAML / DTO authoring pipeline that turns a hand-written
//! workflow specification into a verified, lint-checked,
//! hash-stamped, BPMN-XML-exportable artefact ready for execution.
//!
//! Pipeline (per `publish_workflow`): parse YAML → validate DTO →
//! lint against contract registry (L1–L5) → DTO→IR → verify IR →
//! lower to bytecode → hash → extract task manifest → optional
//! BPMN-XML export → assemble `WorkflowTemplate`.
//!
//! **Status (Phase 1 skeleton):** the entire authoring pipeline
//! exists in `bpmn-lite-core/src/authoring/*` but has no external
//! caller — neither the gRPC service nor any CLI exposes it. The
//! Phase 0 audit flagged this as a real gap: making authoring a
//! shippable user-facing capability requires a `PublishWorkflow`
//! gRPC RPC (and matching `GetTemplate` retrieval) plus wiring
//! `compile_and_publish` to persist into the TemplateStore. Those
//! changes are out-of-scope for the restructure itself and tracked
//! as a follow-on slice.
//!
//! Empty at Phase 1 skeleton — code moves in via the Phase 2
//! migration slice (`authoring/* → bpmn-lite-authoring`).
