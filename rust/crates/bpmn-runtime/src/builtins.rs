//! Built-in verb handlers shipped with the bpmn-runtime.
//!
//! Register these with [`crate::VerbRegistry`] before starting an engine
//! instance that needs them.
//!
//! # `dsl.form`
//!
//! The `dsl.form` verb parks the current fiber and surfaces a form
//! rendering request to the calling context.
//!
//! **Rust side is form.io-agnostic.** The handler resolves `:context` args
//! into a plain `prefill_data` object and emits
//! `VerbEffect::RequestHumanTask { role, form_data }` where `form_data`
//! carries `{form_ref, mode, prefill_data}`. No schema fetch, no rendering.
//!
//! The JS/React side fetches the schema by `form_ref`, renders via Form.io,
//! prefills, and on submit POST `/api/forms/:token_id/submit` which delivers
//! a `HumanTaskComplete` event to resume the parked fiber.

use crate::verb::{VerbContext, VerbEffect, VerbError, VerbHandler, VerbOutput};

/// Built-in handler for `dsl.form`.
///
/// Emits `RequestHumanTask` carrying `{form_ref, mode, prefill_data}`.
/// Parks the fiber; resumes on `HumanTaskComplete` with submission data.
pub struct DslFormHandler;

#[async_trait::async_trait]
impl VerbHandler for DslFormHandler {
    fn verb_ref(&self) -> &str {
        "dsl.form"
    }

    async fn invoke(&self, ctx: VerbContext) -> Result<VerbOutput, VerbError> {
        // form-ref comes from the node's :form-ref slot. In the current runtime,
        // node slots aren't automatically propagated into VerbContext.inputs —
        // the caller is responsible for injecting them. Fall back to an empty
        // string rather than failing so the park/resume cycle still works.
        let form_ref = ctx
            .inputs
            .get("form-ref")
            .or_else(|| ctx.at_slots.get("form-ref"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();

        let mode = ctx
            .inputs
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("capture")
            .to_owned();

        // Resolve context args into a flat prefill_data object.
        let prefill_data = ctx
            .inputs
            .get("context")
            .cloned()
            .unwrap_or(serde_json::json!({}));

        let form_data = serde_json::json!({
            "form_ref": form_ref,
            "mode": mode,
            "prefill_data": prefill_data,
        });

        Ok(VerbOutput {
            data: Default::default(),
            effects: vec![VerbEffect::RequestHumanTask {
                role: "current_user".into(),
                form_data,
            }],
        })
    }
}

/// Register all built-in handlers into a [`VerbRegistry`].
pub fn register_builtins(registry: &mut crate::VerbRegistry) {
    registry.register(Box::new(DslFormHandler));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn ctx_with(inputs: serde_json::Value) -> VerbContext {
        VerbContext {
            at_slots: BTreeMap::new(),
            inputs: inputs
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            outputs: BTreeMap::new(),
            effects: Vec::new(),
            token_id: Uuid::nil(),
            instance_id: Uuid::nil(),
        }
    }

    #[tokio::test]
    async fn emits_request_human_task_with_form_data() {
        let handler = DslFormHandler;
        let ctx = ctx_with(serde_json::json!({
            "form-ref": "kyc.review",
            "mode": "display",
            "context": {"customer": "Allianz"}
        }));
        let output = handler.invoke(ctx).await.unwrap();
        assert!(output.data.is_empty());
        assert_eq!(output.effects.len(), 1);
        if let VerbEffect::RequestHumanTask { role, form_data } = &output.effects[0] {
            assert_eq!(role, "current_user");
            assert_eq!(form_data["form_ref"], "kyc.review");
            assert_eq!(form_data["mode"], "display");
            assert_eq!(form_data["prefill_data"]["customer"], "Allianz");
        } else {
            panic!("expected RequestHumanTask effect");
        }
    }

    #[tokio::test]
    async fn missing_form_ref_uses_empty_string() {
        let handler = DslFormHandler;
        let ctx = ctx_with(serde_json::json!({}));
        let output = handler.invoke(ctx).await.unwrap();
        // No error — parks with empty form_ref
        if let VerbEffect::RequestHumanTask { form_data, .. } = &output.effects[0] {
            assert_eq!(form_data["form_ref"], "");
        }
    }

    #[tokio::test]
    async fn defaults_mode_to_capture() {
        let handler = DslFormHandler;
        let ctx = ctx_with(serde_json::json!({"form-ref": "test.form"}));
        let output = handler.invoke(ctx).await.unwrap();
        if let VerbEffect::RequestHumanTask { form_data, .. } = &output.effects[0] {
            assert_eq!(form_data["mode"], "capture");
        }
    }
}
