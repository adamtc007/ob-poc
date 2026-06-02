//! Tests for the human-task park/resume cycle.
//!
//! Verifies that a verb returning `VerbEffect::RequestHumanTask` parks the
//! fiber, and that delivering `EventKind::HumanTaskComplete` resumes it with
//! the submission data injected into instance context.

use bpmn_runtime::{
    InstanceStatus, JourneyStore, VerbContext, VerbEffect, VerbError, VerbHandler, VerbOutput,
};
use bpmn_test_harness::Scenario;
use serde_json::json;

const FORM_PROCESS: &str = r#"
(node form-start    :kind start-event)
(node collect-data  :kind service-task :verb dsl.form)
(node process-data  :kind service-task)
(node form-end      :kind end-event)

(flow form-start   -> collect-data)
(flow collect-data -> process-data)
(flow process-data -> form-end)
"#;

/// Test verb that emits RequestHumanTask and parks the fiber.
struct FormVerb {
    form_ref: String,
}

#[async_trait::async_trait]
impl VerbHandler for FormVerb {
    fn verb_ref(&self) -> &str {
        "dsl.form"
    }

    async fn invoke(&self, _ctx: VerbContext) -> Result<VerbOutput, VerbError> {
        Ok(VerbOutput {
            data: Default::default(),
            effects: vec![VerbEffect::RequestHumanTask {
                role: "current_user".into(),
                form_data: json!({
                    "form_ref": self.form_ref,
                    "mode": "capture",
                    "prefill_data": {},
                }),
            }],
        })
    }
}

#[tokio::test]
async fn human_task_parks_fiber_and_resumes_on_completion() {
    let result = Scenario::new(FORM_PROCESS)
        .with_verb_handler(Box::new(FormVerb {
            form_ref: "test-form".into(),
        }))
        .run_to_quiescence(json!({}))
        .await;

    // After run_to_quiescence the instance should be Running (not Completed)
    // because the human task has parked the fiber.
    let status = result.status().await;
    assert_eq!(
        status,
        InstanceStatus::Active,
        "instance should be Running (parked at human task)"
    );

    // Exactly one token is parked at collect-data
    let tokens = result.tokens().await;
    let token = tokens
        .into_iter()
        .find(|t| t.current_node == "collect-data")
        .expect("expected token parked at collect-data");

    // Deliver the form submission and run to quiescence
    result
        .engine
        .human_task_complete(
            result.instance_id,
            "collect-data",
            token.id,
            json!({ "customer_name": "Allianz", "risk_tier": "LOW" }),
        )
        .await
        .expect("human_task_complete failed");

    // Instance should now be Completed
    assert_eq!(result.status().await, InstanceStatus::Completed);

    // Submission data should have been written to instance context
    let data = result
        .store
        .read_instance_data(result.instance_id, "customer_name")
        .await
        .expect("read_instance_data failed");
    assert_eq!(
        data,
        Some(json!("Allianz")),
        "submission data should be in instance context"
    );
}

#[tokio::test]
async fn human_task_complete_with_empty_submission_advances_token() {
    let result = Scenario::new(FORM_PROCESS)
        .with_verb_handler(Box::new(FormVerb {
            form_ref: "ack-form".into(),
        }))
        .run_to_quiescence(json!({}))
        .await;

    let tokens = result.tokens().await;
    let token = tokens
        .into_iter()
        .find(|t| t.current_node == "collect-data")
        .expect("expected token parked at collect-data");

    // Deliver empty submission (display-only mode — just an ack)
    result
        .engine
        .human_task_complete(result.instance_id, "collect-data", token.id, json!({}))
        .await
        .expect("human_task_complete failed");

    assert_eq!(result.status().await, InstanceStatus::Completed);
}
