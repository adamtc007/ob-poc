//! `ProcessRegistryService` — allows plugin verbs to start BPMN process
//! instances without depending on bpmn-runtime or ob-poc-web directly.
//!
//! The host (ob-poc-web) registers a concrete `ProcessRegistry` impl at
//! startup. Verb ops call `ctx.service::<dyn ProcessRegistryService>()`.
//!
//! Returns opaque JSON so this crate has no dependency on bpmn-runtime types.

use anyhow::Result;

/// Start a named BPMN process and return its initial state as JSON.
///
/// The returned `Value` has the shape:
/// ```json
/// {
///   "instance_id": "<uuid>",
///   "status": "parked" | "running",
///   "bpmn_form": { ... } | null
/// }
/// ```
/// where `bpmn_form` is a `BpmnFormPending`-compatible object when the process
/// immediately parks at a human task, otherwise absent.
#[async_trait::async_trait]
pub trait ProcessRegistryService: Send + Sync {
    async fn start_process(
        &self,
        process_name: &str,
        initial_data: serde_json::Value,
    ) -> Result<serde_json::Value>;
}
