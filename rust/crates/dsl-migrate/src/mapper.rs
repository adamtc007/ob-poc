//! IR → bpmn-lite DSL atoms.

use crate::reporter::MigrationElement;
use crate::xml_reader::{
    BpmnBoundaryEvent, BpmnElement, BpmnGateway, BpmnProcess, EventType, GatewayType, SequenceFlow,
    TaskType,
};

pub struct MappedDsl {
    /// DSL atom lines for the process.
    pub atom_lines: Vec<String>,
    /// Per-element migration status for the coverage report.
    pub element_statuses: Vec<MigrationElement>,
}

pub fn map_process(process: &BpmnProcess) -> MappedDsl {
    let mut lines: Vec<String> = Vec::new();
    let mut statuses: Vec<MigrationElement> = Vec::new();

    // Migration provenance marker
    lines.push(format!(
        "(migration-source :source-id \"{}\" :migrated-at \"2026-05-22T00:00:00Z\")",
        process.id,
    ));
    lines.push(String::new());

    // Map each top-level element (boundary events mapped separately below)
    for element in &process.elements {
        if let BpmnElement::BoundaryEvent(_) = element {
            continue; // handled in second pass
        }
        let (atom_line, status) = map_element(element);
        if let Some(line) = atom_line {
            lines.push(line);
        }
        statuses.push(status);
    }

    // Boundary events as attachment atoms
    for element in &process.elements {
        if let BpmnElement::BoundaryEvent(be) = element {
            let (line, status) = map_boundary_event(be);
            if let Some(l) = line {
                lines.push(l);
            }
            statuses.push(status);
        }
    }

    // Sequence flows (may produce HumanResolve statuses for FEEL conditions)
    if !process.sequence_flows.is_empty() {
        lines.push(String::new());
        for flow in &process.sequence_flows {
            let (line, maybe_status) = map_sequence_flow_with_status(flow);
            lines.push(line);
            if let Some(status) = maybe_status {
                statuses.push(status);
            }
        }
    }

    MappedDsl {
        atom_lines: lines,
        element_statuses: statuses,
    }
}

// ─── Element mapper ──────────────────────────────────────────────────────────

fn map_element(element: &BpmnElement) -> (Option<String>, MigrationElement) {
    match element {
        BpmnElement::StartEvent(e) => {
            let kind = event_kind(&e.event_type, "start");
            let line = format!("(node {} :kind {})", safe_id(&e.id), kind);
            (
                Some(line),
                MigrationElement::clean(&e.id, e.name.as_deref(), "start-event"),
            )
        }
        BpmnElement::EndEvent(e) => {
            let kind = event_kind(&e.event_type, "end");
            let line = format!("(node {} :kind {})", safe_id(&e.id), kind);
            (
                Some(line),
                MigrationElement::clean(&e.id, e.name.as_deref(), "end-event"),
            )
        }
        BpmnElement::IntermediateCatchEvent(e) => {
            let kind = event_kind(&e.event_type, "intermediate-catch");
            let line = format!("(node {} :kind {})", safe_id(&e.id), kind);
            (
                Some(line),
                MigrationElement::clean(&e.id, e.name.as_deref(), "intermediate-catch"),
            )
        }
        BpmnElement::IntermediateThrowEvent(e) => {
            let kind = event_kind(&e.event_type, "intermediate-throw");
            let line = format!("(node {} :kind {})", safe_id(&e.id), kind);
            (
                Some(line),
                MigrationElement::clean(&e.id, e.name.as_deref(), "intermediate-throw"),
            )
        }
        BpmnElement::Task(t) => {
            let node_kind = task_kind(&t.task_type);

            // Try verb resolution for service / business-rule tasks
            let verb_ref = if matches!(t.task_type, TaskType::Service | TaskType::BusinessRule) {
                t.implementation
                    .as_deref()
                    .and_then(crate::verb_resolver::resolve_verb)
            } else {
                None
            };

            let (line, status) = if let Some(verb) = &verb_ref {
                let l = format!(
                    "(node {} :kind {} :verb (invoke {} :args {{}}))",
                    safe_id(&t.id),
                    node_kind,
                    verb,
                );
                (l, MigrationElement::clean(&t.id, t.name.as_deref(), node_kind))
            } else if let Some(impl_str) = &t.implementation {
                let comment = format!(
                    "; [HUMAN-RESOLVE] verb: {} → ?\n(node {} :kind {})",
                    impl_str,
                    safe_id(&t.id),
                    node_kind
                );
                (
                    comment,
                    MigrationElement::human_resolve(
                        &t.id,
                        t.name.as_deref(),
                        node_kind,
                        &format!("unresolved implementation: {}", impl_str),
                    ),
                )
            } else {
                let l = format!("(node {} :kind {})", safe_id(&t.id), node_kind);
                (l, MigrationElement::clean(&t.id, t.name.as_deref(), node_kind))
            };

            (Some(line), status)
        }
        BpmnElement::Gateway(g) => map_gateway(g),
        BpmnElement::SubProcess(sp) => {
            let line = format!("(node {} :kind subprocess)", safe_id(&sp.id));
            (
                Some(line),
                MigrationElement::clean(&sp.id, sp.name.as_deref(), "subprocess"),
            )
        }
        BpmnElement::BoundaryEvent(_) => {
            // handled in second pass
            (None, MigrationElement::skip("boundary-event"))
        }
        BpmnElement::Unknown { tag, id, name } => {
            let comment = format!(
                "; [HUMAN-RESOLVE] unsupported element: {} id={}",
                tag, id
            );
            (
                Some(comment),
                MigrationElement::human_resolve(
                    id,
                    name.as_deref(),
                    tag,
                    "unsupported element",
                ),
            )
        }
    }
}

fn map_gateway(g: &BpmnGateway) -> (Option<String>, MigrationElement) {
    match g.gateway_type {
        GatewayType::Complex => (
            None,
            MigrationElement::rejected(
                &g.id,
                g.name.as_deref(),
                "gateway",
                "complex gateway rejected — use inclusive + predicate",
            ),
        ),
        _ => {
            let kind = match g.gateway_type {
                GatewayType::Exclusive => "exclusive",
                GatewayType::Inclusive => "inclusive",
                GatewayType::Parallel => "parallel",
                GatewayType::EventBased => "event-based",
                GatewayType::Complex => unreachable!(),
            };
            let line = format!("(gateway {} :kind {})", safe_id(&g.id), kind);
            (
                Some(line),
                MigrationElement::clean(&g.id, g.name.as_deref(), "gateway"),
            )
        }
    }
}

fn map_boundary_event(be: &BpmnBoundaryEvent) -> (Option<String>, MigrationElement) {
    let event_kind_str = match &be.event_type {
        EventType::Error { .. } => "error",
        EventType::Timer { .. } => "timer",
        EventType::Message { .. } => "message",
        EventType::Signal { .. } => "signal",
        EventType::Escalation { .. } => "escalation",
        EventType::Compensation => "compensation",
        _ => "error",
    };
    let line = format!(
        "(boundary-attachment {} {} :event-kind {} :interrupting {})",
        safe_id(&be.attached_to_ref),
        safe_id(&be.id),
        event_kind_str,
        be.cancel_activity,
    );
    (
        Some(line),
        MigrationElement::clean(&be.id, be.name.as_deref(), "boundary-event"),
    )
}

fn map_sequence_flow_with_status(flow: &SequenceFlow) -> (String, Option<MigrationElement>) {
    if let Some(cond) = &flow.condition_expression {
        if looks_like_feel(cond) {
            let line = format!(
                "; [HUMAN-RESOLVE] FEEL condition: {}\n(flow {} -> {} :condition \"TODO\")",
                cond,
                safe_id(&flow.source_ref),
                safe_id(&flow.target_ref),
            );
            let status = MigrationElement::human_resolve(
                &flow.id,
                flow.name.as_deref(),
                "sequence-flow",
                &format!("FEEL condition requires manual translation: {}", cond),
            );
            (line, Some(status))
        } else {
            let line = format!(
                "(flow {} -> {} :condition \"{}\")",
                safe_id(&flow.source_ref),
                safe_id(&flow.target_ref),
                cond,
            );
            (line, None)
        }
    } else {
        let line = format!(
            "(flow {} -> {})",
            safe_id(&flow.source_ref),
            safe_id(&flow.target_ref),
        );
        (line, None)
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn event_kind(event_type: &EventType, prefix: &str) -> String {
    match event_type {
        EventType::None => prefix.to_string(),
        EventType::Message { .. } => format!("{}-message", prefix),
        EventType::Timer { .. } => format!("{}-timer", prefix),
        EventType::Error { .. } => format!("{}-error", prefix),
        EventType::Signal { .. } => format!("{}-signal", prefix),
        EventType::Escalation { .. } => format!("{}-escalation", prefix),
        EventType::Terminate => format!("{}-terminate", prefix),
        EventType::Compensation => format!("{}-compensation", prefix),
        EventType::Link { .. } => format!("{}-link", prefix),
    }
}

fn task_kind(tt: &TaskType) -> &'static str {
    match tt {
        TaskType::Service => "service-task",
        TaskType::User => "user-task",
        TaskType::Manual => "manual-task",
        TaskType::BusinessRule => "business-rule-task",
        TaskType::Script => "script-task",
        TaskType::Send => "send-task",
        TaskType::Receive => "receive-task",
        TaskType::CallActivity => "call-activity",
    }
}

/// Convert a Camunda element ID to a DSL-safe kebab identifier.
fn safe_id(id: &str) -> String {
    id.replace(['_', ' '], "-")
        .to_lowercase()
}

/// Detect FEEL expressions that cannot be directly used in our DSL.
fn looks_like_feel(expr: &str) -> bool {
    expr.contains("#{")
        || expr.contains("${")
        || expr.contains("= \"")
        || (expr.contains("==") && !expr.starts_with('('))
        // FEEL list/range operators
        || expr.contains('[')
        || expr.contains("not(")
        // JUEL-style expressions
        || (expr.contains("#{") || expr.starts_with("${"))
}
