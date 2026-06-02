use dsl_ast::AtomBag;
use dsl_bpmn_frontend::assemble;
use dsl_diagnostics::DiagnosticBag;

#[test]
fn test_boundary_event_no_outflow_fails_validation() {
    let src = r#"
    (node start :kind start-event)
    (node worker :kind service-task)
    (boundary-attachment worker :event-kind timer) ; unnamed boundary, falls back to worker-boundary
    (node end :kind end-event)

    (flow :source start :target worker)
    (flow :source worker :target end)
    "#;

    let (source_file, parse_diag) = dsl_parser::parse(src);
    let mut diag = DiagnosticBag::new();

    // Merge parse diagnostics
    for d in &parse_diag.diagnostics {
        diag.push(d.clone());
    }

    let bag = AtomBag::from_source_file(source_file, &mut diag);
    let _graph = assemble(&bag, &mut diag);

    let errs: Vec<_> = diag.errors().collect();
    for e in errs {
        println!("ERROR: {:?}", e);
    }

    let has_unterminated_error = diag
        .errors()
        .any(|d| d.code.as_deref() == Some("E1002") && d.message.contains("worker-boundary"));

    assert!(
        has_unterminated_error,
        "Expected UNTERMINATED_PATH error for isolated boundary event (worker-boundary)"
    );
}
