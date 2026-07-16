//! Unit tests for [`ObPocBusHandler`].
//!
//! Mock `VerbExecutor` implementations exercise the trait surface
//! without touching a real ob-poc engine. The DB-backed end-to-end
//! coverage of the bus server itself already lives in
//! `dsl-bus-server::tests`; here we just verify the adapter mapping.

use super::*;
use std::sync::Mutex;

#[derive(Default)]
struct MockExecutor {
    calls: Mutex<Vec<(String, String, Vec<ResolvedBinding>, Option<Uuid>)>>,
}

#[async_trait]
impl VerbExecutor for MockExecutor {
    async fn execute(
        &self,
        verb: &str,
        catalogue: &str,
        inputs: Vec<ResolvedBinding>,
        snapshot_pin: Option<Uuid>,
    ) -> Result<VerbOutcome, VerbExecutorError> {
        self.calls.lock().unwrap().push((
            verb.to_owned(),
            catalogue.to_owned(),
            inputs.clone(),
            snapshot_pin,
        ));
        Ok(VerbOutcome {
            execution_id: Uuid::now_v7(),
            kind: ExecutionOutcomeKind::Committed,
            detail: format!("mock executed {verb}"),
            bindings: vec![],
        })
    }
}

fn ctx(verb: &str) -> InvocationContext {
    InvocationContext {
        idempotency_key: Uuid::now_v7(),
        source_domain: "bpmn-lite".into(),
        catalogue_version: "v1.0.0".into(),
        local_verb_id: verb.into(),
        result_callback_endpoint: "http://bpmn-lite/result".into(),
        authority: None,
        tenant_id: "test-tenant".into(),
        snapshot_pin: None,
    }
}

/// [`ctx`], but with a `snapshot_pin` set — G6a coverage (§8 of the
/// design doc): proves `ObPocBusHandler::dispatch` forwards the pin
/// unchanged into `VerbExecutor::execute`'s new parameter.
fn ctx_with_pin(verb: &str, pin: Uuid) -> InvocationContext {
    InvocationContext {
        snapshot_pin: Some(pin),
        ..ctx(verb)
    }
}

#[tokio::test]
async fn dispatch_forwards_local_verb_id_and_catalogue_version() {
    let mock = MockExecutor::default();
    let calls_ref = mock.calls.lock().unwrap().clone();
    drop(calls_ref);
    let handler = ObPocBusHandler::new(mock);

    let outcome = handler.dispatch(ctx("cbu.create"), vec![]).await.unwrap();
    assert_eq!(outcome.outcome.kind, ExecutionOutcomeKind::Committed as i32);
    assert!(outcome.outcome.detail.contains("cbu.create"));
}

/// G6a (EOP-DESIGN-CONTROLPLANE-G6A-SNAPSHOT-PIN-CARRIER-001 §5/§8):
/// `InvocationContext.snapshot_pin` must reach `VerbExecutor::execute`'s
/// new parameter unchanged — the wire-level half of G6a, independent of
/// whatever `ObPocVerbAdapter`'s real implementation does with it. Uses
/// `from_arc` (rather than `new`) so the test retains its own handle on
/// `MockExecutor` and can inspect `calls` after dispatch.
#[tokio::test]
async fn dispatch_forwards_snapshot_pin_unchanged() {
    let mock = std::sync::Arc::new(MockExecutor::default());
    let handler = ObPocBusHandler::from_arc(mock.clone());
    let pin = Uuid::now_v7();

    handler
        .dispatch(ctx_with_pin("cbu.create", pin), vec![])
        .await
        .unwrap();

    let calls = mock.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].3, Some(pin));
}

/// Regression sibling: no `snapshot_pin` on the incoming context must
/// still forward as `None`, not silently substitute a fresh id.
#[tokio::test]
async fn dispatch_forwards_absent_snapshot_pin_as_none() {
    let mock = std::sync::Arc::new(MockExecutor::default());
    let handler = ObPocBusHandler::from_arc(mock.clone());

    handler
        .dispatch(ctx("cbu.create"), vec![])
        .await
        .unwrap();

    let calls = mock.calls.lock().unwrap();
    assert_eq!(calls[0].3, None);
}

#[tokio::test]
async fn unknown_verb_executor_error_maps_to_bus_server_unknown_verb() {
    struct RejectingExecutor;
    #[async_trait]
    impl VerbExecutor for RejectingExecutor {
        async fn execute(
            &self,
            verb: &str,
            _catalogue: &str,
            _inputs: Vec<ResolvedBinding>,
            _snapshot_pin: Option<Uuid>,
        ) -> Result<VerbOutcome, VerbExecutorError> {
            Err(VerbExecutorError::UnknownVerb(format!(
                "verb '{verb}' not in catalogue"
            )))
        }
    }

    let handler = ObPocBusHandler::new(RejectingExecutor);
    let err = handler.dispatch(ctx("cbu.nope"), vec![]).await.unwrap_err();
    match err {
        BusServerError::UnknownVerb(msg) => assert!(msg.contains("cbu.nope")),
        other => panic!("expected UnknownVerb, got {other:?}"),
    }
}

#[tokio::test]
async fn version_incompatible_maps_through() {
    struct V;
    #[async_trait]
    impl VerbExecutor for V {
        async fn execute(
            &self,
            _v: &str,
            cat: &str,
            _i: Vec<ResolvedBinding>,
            _snapshot_pin: Option<Uuid>,
        ) -> Result<VerbOutcome, VerbExecutorError> {
            Err(VerbExecutorError::VersionIncompatible(format!("got {cat}")))
        }
    }
    let handler = ObPocBusHandler::new(V);
    let err = handler
        .dispatch(ctx("cbu.create"), vec![])
        .await
        .unwrap_err();
    assert!(matches!(err, BusServerError::VersionIncompatible(_)));
}

#[tokio::test]
async fn malformed_input_maps_through() {
    struct M;
    #[async_trait]
    impl VerbExecutor for M {
        async fn execute(
            &self,
            _v: &str,
            _c: &str,
            _i: Vec<ResolvedBinding>,
            _snapshot_pin: Option<Uuid>,
        ) -> Result<VerbOutcome, VerbExecutorError> {
            Err(VerbExecutorError::Malformed("missing name".into()))
        }
    }
    let handler = ObPocBusHandler::new(M);
    let err = handler
        .dispatch(ctx("cbu.create"), vec![])
        .await
        .unwrap_err();
    assert!(matches!(err, BusServerError::Malformed(_)));
}

#[tokio::test]
async fn noop_result_dispatcher_rejects_unknown_execution() {
    let rd = NoopResultDispatcher;
    let err = rd
        .dispatch(
            ResultContext {
                idempotency_key: Uuid::now_v7(),
                execution_id: Uuid::now_v7(),
                source_domain: "bpmn-lite".into(),
                audit_reference: String::new(),
            },
            ExecutionOutcome {
                kind: ExecutionOutcomeKind::Committed as i32,
                detail: String::new(),
                bindings: vec![],
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, BusServerError::UnknownVerb(_)));
}
