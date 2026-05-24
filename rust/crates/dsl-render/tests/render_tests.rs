//! Integration tests for the dsl-render crate.

#[test]
fn linear_sequence_renders_to_svg() {
    let dsl = r#"
(node start-1 :kind start-event)
(node task-1 :kind service-task)
(node end-1 :kind end-event)
(flow start-1 -> task-1)
(flow task-1 -> end-1)
"#;
    let svg = dsl_render::render(dsl).unwrap();
    assert!(svg.starts_with("<svg"), "should produce SVG, got: {}", &svg[..svg.len().min(200)]);
    assert!(svg.contains("</svg>"), "should close SVG");
    assert!(svg.contains("circle"), "start event should be a circle");
    assert!(svg.len() > 200, "SVG should have content (len={})", svg.len());
}

#[test]
fn exclusive_gateway_renders_diamond() {
    let dsl = r#"
(node start-1 :kind start-event)
(gateway gw-1 :kind exclusive)
(node end-a :kind end-event)
(node end-b :kind end-event)
(flow start-1 -> gw-1)
(flow gw-1 -> end-a :condition "approved")
(flow gw-1 -> end-b :default true)
"#;
    let svg = dsl_render::render(dsl).unwrap();
    assert!(
        svg.contains("polygon"),
        "gateway should be a diamond polygon, svg={}",
        &svg[..svg.len().min(500)]
    );
    // The × character is rendered as &#xD7; in XML
    assert!(
        svg.contains("&#xD7;") || svg.contains('\u{D7}'),
        "exclusive gateway should show × (&#xD7;)"
    );
}

#[test]
fn parallel_gateway_renders_plus() {
    let dsl = r#"
(node start-1 :kind start-event)
(gateway fork-1 :kind parallel)
(node task-a :kind service-task)
(node task-b :kind user-task)
(node end-1 :kind end-event)
(flow start-1 -> fork-1)
(flow fork-1 -> task-a)
(flow fork-1 -> task-b)
(flow task-a -> end-1)
(flow task-b -> end-1)
"#;
    let svg = dsl_render::render(dsl).unwrap();
    assert!(svg.contains('+'), "parallel gateway should show +");
}

#[test]
fn provenance_badge_appears() {
    let dsl = r#"
(gateway kyc-gate :kind exclusive)
(node start-1 :kind start-event)
(node end-1 :kind end-event)
(flow start-1 -> kyc-gate)
(flow kyc-gate -> end-1 :default true)
(provenance test-prov :covers [kyc-gate] :source pack :source-id conjunctive-gate :version "1.0.0" :session "s1" :authored-at "2026-05-22T00:00:00Z")
"#;
    let svg = dsl_render::render(dsl).unwrap();
    // Badge rect uses fill color #f59e0b
    assert!(
        svg.contains("f59e0b"),
        "provenance badge color should appear in SVG"
    );
}

#[test]
fn boundary_attachment_renders() {
    let dsl = r#"
(node start-1 :kind start-event)
(node task-1 :kind user-task)
(node end-1 :kind end-event)
(node err-end :kind end-event-error)
(boundary-attachment task-1-error :host task-1 :event-kind error :interrupting true)
(flow start-1 -> task-1)
(flow task-1 -> end-1)
(flow task-1-error -> err-end)
"#;
    let svg = dsl_render::render(dsl).unwrap();
    assert!(!svg.is_empty());
    assert!(svg.starts_with("<svg"), "should produce SVG");
}

#[test]
fn empty_dsl_produces_minimal_svg() {
    let svg = dsl_render::render("").unwrap();
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("</svg>"));
}

#[test]
fn render_all_v01_examples() {
    // Various v0.1 patterns — just verify each renders without panic
    let examples = [
        // Linear
        r#"(node start :kind start-event) (node t1 :kind user-task) (node end :kind end-event) (flow start -> t1) (flow t1 -> end)"#,
        // Exclusive gateway
        r#"(node start :kind start-event) (gateway gw :kind exclusive) (node end-a :kind end-event) (node end-b :kind end-event) (flow start -> gw) (flow gw -> end-a) (flow gw -> end-b :default true)"#,
        // Parallel gateway
        r#"(node s :kind start-event) (gateway par :kind parallel) (node ta :kind service-task) (node tb :kind service-task) (node e :kind end-event) (flow s -> par) (flow par -> ta) (flow par -> tb) (flow ta -> e) (flow tb -> e)"#,
        // Inclusive gateway
        r#"(node s :kind start-event) (gateway inc :kind inclusive) (node e :kind end-event) (flow s -> inc) (flow inc -> e)"#,
        // Business rule task
        r#"(node s :kind start-event) (node br :kind business-rule-task) (node e :kind end-event) (flow s -> br) (flow br -> e)"#,
        // Subprocess
        r#"(node s :kind start-event) (node sp :kind subprocess) (node e :kind end-event) (flow s -> sp) (flow sp -> e)"#,
    ];
    for (i, dsl) in examples.iter().enumerate() {
        let result = dsl_render::render(dsl);
        assert!(result.is_ok(), "example {} failed: {:?}", i + 1, result.err());
        let svg = result.unwrap();
        assert!(
            svg.starts_with("<svg"),
            "example {} should produce SVG",
            i + 1
        );
    }
}

#[test]
fn render_options_no_labels() {
    let dsl = r#"
(node start :kind start-event)
(node task :kind service-task)
(node end :kind end-event)
(flow start -> task)
(flow task -> end)
"#;
    let opts = dsl_render::RenderOptions {
        include_labels: false,
        ..Default::default()
    };
    let svg = dsl_render::render_dsl(dsl, &opts).unwrap();
    assert!(svg.starts_with("<svg"));
    // With labels disabled, node names should not appear as text
    // (the nodes still render, just without the text elements)
    assert!(svg.contains("</svg>"));
}

#[test]
fn all_node_kinds_render() {
    // Exercise every NodeKind variant
    let kinds = [
        "start-event", "start-event-message", "start-event-timer",
        "start-event-signal", "start-event-error", "start-event-escalation",
        "start-event-compensation",
        "end-event", "end-event-terminate", "end-event-error",
        "end-event-message", "end-event-signal", "end-event-cancel",
        "end-event-escalation", "end-event-compensation",
        "service-task", "user-task", "send-task", "receive-task",
        "manual-task", "business-rule-task", "script-task",
        "subprocess", "event-subprocess", "transaction-subprocess",
        "call-activity",
    ];

    let start = "(node s-start :kind start-event)";
    let end = "(node s-end :kind end-event)";

    for kind in &kinds {
        let safe_id = kind.replace('-', "_");
        let dsl = format!(
            "{start}\n(node {safe_id} :kind {kind})\n{end}\n(flow s-start -> {safe_id})\n(flow {safe_id} -> s-end)"
        );
        let result = dsl_render::render(&dsl);
        assert!(
            result.is_ok(),
            "kind '{}' failed to render: {:?}",
            kind,
            result.err()
        );
        let svg = result.unwrap();
        assert!(svg.starts_with("<svg"), "kind '{}' did not produce SVG", kind);
    }
}

#[test]
fn all_gateway_kinds_render() {
    let gateway_kinds = [
        "exclusive",
        "inclusive",
        "parallel",
        "event-based",
        "parallel-event-based",
    ];

    for kind in &gateway_kinds {
        let safe_id = kind.replace('-', "_");
        let dsl = format!(
            "(node s :kind start-event)\n(gateway {safe_id} :kind {kind})\n(node e :kind end-event)\n(flow s -> {safe_id})\n(flow {safe_id} -> e)"
        );
        let result = dsl_render::render(&dsl);
        assert!(
            result.is_ok(),
            "gateway kind '{}' failed: {:?}",
            kind,
            result.err()
        );
        let svg = result.unwrap();
        assert!(
            svg.contains("polygon"),
            "gateway '{}' should render a diamond",
            kind
        );
    }
}
