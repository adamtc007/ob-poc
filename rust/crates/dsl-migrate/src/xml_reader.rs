//! Camunda 8 BPMN XML → typed intermediate representation.
//!
//! Handles `bpmn:` and bare namespace prefixes. Parses elements, event definitions,
//! gateway types, boundary events, and sequence flow conditions.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use quick_xml::events::Event;
use quick_xml::Reader;

// ─── Typed IR ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BpmnProcess {
    pub id: String,
    pub name: Option<String>,
    pub elements: Vec<BpmnElement>,
    pub sequence_flows: Vec<SequenceFlow>,
}

#[derive(Debug, Clone)]
pub enum BpmnElement {
    StartEvent(BpmnEvent),
    EndEvent(BpmnEvent),
    IntermediateCatchEvent(BpmnEvent),
    IntermediateThrowEvent(BpmnEvent),
    Task(BpmnTask),
    Gateway(BpmnGateway),
    SubProcess(BpmnSubProcess),
    BoundaryEvent(BpmnBoundaryEvent),
    /// Unknown / unsupported element preserved for coverage reporting.
    Unknown {
        tag: String,
        id: String,
        name: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct BpmnEvent {
    pub id: String,
    pub name: Option<String>,
    pub event_type: EventType,
}

#[derive(Debug, Clone)]
pub enum EventType {
    None,
    Message { message_ref: Option<String> },
    Timer { timer_def: Option<String> },
    Error { error_ref: Option<String> },
    Signal { signal_ref: Option<String> },
    Escalation { escalation_ref: Option<String> },
    Terminate,
    Compensation,
    Link { link_name: Option<String> },
}

#[derive(Debug, Clone)]
pub struct BpmnTask {
    pub id: String,
    pub name: Option<String>,
    pub task_type: TaskType,
    /// Implementation hint: Camunda topic, class, or Zeebe task type.
    pub implementation: Option<String>,
    /// For user tasks: assignee expression.
    pub assignee: Option<String>,
    /// `camunda:property` / extension name→value pairs.
    pub camunda_props: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub enum TaskType {
    Service,
    User,
    Manual,
    BusinessRule,
    Script,
    Send,
    Receive,
    CallActivity,
}

#[derive(Debug, Clone)]
pub struct BpmnGateway {
    pub id: String,
    pub name: Option<String>,
    pub gateway_type: GatewayType,
}

#[derive(Debug, Clone)]
pub enum GatewayType {
    Exclusive,
    Inclusive,
    Parallel,
    EventBased,
    Complex,
}

#[derive(Debug, Clone)]
pub struct BpmnSubProcess {
    pub id: String,
    pub name: Option<String>,
    pub is_event_subprocess: bool,
    pub elements: Vec<BpmnElement>,
    pub sequence_flows: Vec<SequenceFlow>,
}

#[derive(Debug, Clone)]
pub struct BpmnBoundaryEvent {
    pub id: String,
    pub name: Option<String>,
    pub attached_to_ref: String,
    pub cancel_activity: bool,
    pub event_type: EventType,
}

#[derive(Debug, Clone)]
pub struct SequenceFlow {
    pub id: String,
    pub source_ref: String,
    pub target_ref: String,
    pub name: Option<String>,
    pub condition_expression: Option<String>,
}

// ─── Public entry point ─────────────────────────────────────────────────────

/// Parse a Camunda 8 BPMN XML string into a typed IR.
pub fn parse_bpmn_xml(xml: &str) -> Result<BpmnProcess> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(ref e) => {
                let tag = strip_prefix(e.name().as_ref());
                if tag == "process" {
                    let id = attr_str(e, b"id")
                        .unwrap_or_else(|| "unknown-process".to_string());
                    let name = attr_str(e, b"name");
                    return parse_container(&mut reader, id, name, "process");
                }
                // skip definitions / collaboration / other wrappers
            }
            Event::Empty(ref e) => {
                let tag = strip_prefix(e.name().as_ref());
                if tag == "process" {
                    return Ok(BpmnProcess {
                        id: attr_str(e, b"id").unwrap_or_else(|| "p".to_string()),
                        name: attr_str(e, b"name"),
                        elements: vec![],
                        sequence_flows: vec![],
                    });
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Err(anyhow!("No <process> element found in BPMN XML"))
}

// ─── Container parser (process / subprocess body) ───────────────────────────

fn parse_container(
    reader: &mut Reader<&[u8]>,
    id: String,
    name: Option<String>,
    closing_tag: &str,
) -> Result<BpmnProcess> {
    let mut elements: Vec<BpmnElement> = Vec::new();
    let mut sequence_flows: Vec<SequenceFlow> = Vec::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            // ── Self-closing elements ──────────────────────────────────────
            Event::Empty(ref e) => {
                let tag = strip_prefix(e.name().as_ref());
                match classify_element(&tag) {
                    ElementClass::Start => {
                        elements.push(BpmnElement::StartEvent(BpmnEvent {
                            id: attr_str(e, b"id").unwrap_or_default(),
                            name: attr_str(e, b"name"),
                            event_type: EventType::None,
                        }));
                    }
                    ElementClass::End => {
                        elements.push(BpmnElement::EndEvent(BpmnEvent {
                            id: attr_str(e, b"id").unwrap_or_default(),
                            name: attr_str(e, b"name"),
                            event_type: EventType::None,
                        }));
                    }
                    ElementClass::Task(tk) => {
                        // Self-closing task element (no children)
                        let id = attr_str(e, b"id").unwrap_or_default();
                        let name = attr_str(e, b"name");
                        let implementation = attr_str(e, b"topic")
                            .or_else(|| attr_str(e, b"class"))
                            .or_else(|| attr_str(e, b"expression"));
                        elements.push(BpmnElement::Task(BpmnTask {
                            id,
                            name,
                            task_type: tk,
                            implementation,
                            assignee: attr_str(e, b"assignee"),
                            camunda_props: std::collections::HashMap::new(),
                        }));
                    }
                    ElementClass::Gateway(gk) => {
                        elements.push(BpmnElement::Gateway(make_gateway(e, gk)));
                    }
                    ElementClass::SeqFlow => {
                        sequence_flows.push(make_seq_flow(e, None));
                    }
                    ElementClass::SubProcess => {
                        let id = attr_str(e, b"id").unwrap_or_default();
                        let name = attr_str(e, b"name");
                        let is_event = attr_str(e, b"triggeredByEvent")
                            .map(|v| v == "true")
                            .unwrap_or(false);
                        elements.push(BpmnElement::SubProcess(BpmnSubProcess {
                            id,
                            name,
                            is_event_subprocess: is_event,
                            elements: vec![],
                            sequence_flows: vec![],
                        }));
                    }
                    _ => {} // ignore infra / unknown self-closing
                }
            }

            // ── Elements with children ─────────────────────────────────────
            Event::Start(ref e) => {
                let tag = strip_prefix(e.name().as_ref());
                let eid = attr_str(e, b"id").unwrap_or_default();
                let ename = attr_str(e, b"name");

                match classify_element(&tag) {
                    ElementClass::Start => {
                        let ev = parse_event_body(e, reader, &tag)?;
                        elements.push(BpmnElement::StartEvent(ev));
                    }
                    ElementClass::End => {
                        let ev = parse_event_body(e, reader, &tag)?;
                        elements.push(BpmnElement::EndEvent(ev));
                    }
                    ElementClass::IntermediateCatch => {
                        let ev = parse_event_body(e, reader, &tag)?;
                        elements.push(BpmnElement::IntermediateCatchEvent(ev));
                    }
                    ElementClass::IntermediateThrow => {
                        let ev = parse_event_body(e, reader, &tag)?;
                        elements.push(BpmnElement::IntermediateThrowEvent(ev));
                    }
                    ElementClass::Task(tk) => {
                        let task = parse_task_body(e, reader, tk, &tag)?;
                        elements.push(BpmnElement::Task(task));
                    }
                    ElementClass::Gateway(gk) => {
                        skip_subtree(reader, &tag)?;
                        elements.push(BpmnElement::Gateway(make_gateway(e, gk)));
                    }
                    ElementClass::SubProcess => {
                        let is_event = attr_str(e, b"triggeredByEvent")
                            .map(|v| v == "true")
                            .unwrap_or(false);
                        let inner = parse_container(reader, eid.clone(), ename.clone(), &tag)?;
                        elements.push(BpmnElement::SubProcess(BpmnSubProcess {
                            id: eid,
                            name: ename,
                            is_event_subprocess: is_event,
                            elements: inner.elements,
                            sequence_flows: inner.sequence_flows,
                        }));
                    }
                    ElementClass::Boundary => {
                        let be = parse_boundary_body(e, reader)?;
                        elements.push(BpmnElement::BoundaryEvent(be));
                    }
                    ElementClass::SeqFlow => {
                        let cond = parse_seq_flow_condition(reader, &tag)?;
                        sequence_flows.push(make_seq_flow(e, cond));
                    }
                    ElementClass::Infra => {
                        skip_subtree(reader, &tag)?;
                    }
                    ElementClass::Unknown => {
                        skip_subtree(reader, &tag)?;
                        if !eid.is_empty() {
                            elements.push(BpmnElement::Unknown {
                                tag,
                                id: eid,
                                name: ename,
                            });
                        }
                    }
                }
            }

            Event::End(ref e) => {
                let tag = strip_prefix(e.name().as_ref());
                if tag == closing_tag {
                    break;
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(BpmnProcess {
        id,
        name,
        elements,
        sequence_flows,
    })
}

// ─── Element classifiers ─────────────────────────────────────────────────────

enum ElementClass {
    Start,
    End,
    IntermediateCatch,
    IntermediateThrow,
    Task(TaskType),
    Gateway(GatewayType),
    SubProcess,
    Boundary,
    SeqFlow,
    Infra,
    Unknown,
}

fn classify_element(tag: &str) -> ElementClass {
    match tag {
        "startEvent" => ElementClass::Start,
        "endEvent" => ElementClass::End,
        "intermediateCatchEvent" => ElementClass::IntermediateCatch,
        "intermediateThrowEvent" => ElementClass::IntermediateThrow,
        "serviceTask" => ElementClass::Task(TaskType::Service),
        "userTask" => ElementClass::Task(TaskType::User),
        "manualTask" => ElementClass::Task(TaskType::Manual),
        "businessRuleTask" => ElementClass::Task(TaskType::BusinessRule),
        "scriptTask" => ElementClass::Task(TaskType::Script),
        "sendTask" => ElementClass::Task(TaskType::Send),
        "receiveTask" => ElementClass::Task(TaskType::Receive),
        "callActivity" => ElementClass::Task(TaskType::CallActivity),
        "exclusiveGateway" => ElementClass::Gateway(GatewayType::Exclusive),
        "inclusiveGateway" => ElementClass::Gateway(GatewayType::Inclusive),
        "parallelGateway" => ElementClass::Gateway(GatewayType::Parallel),
        "eventBasedGateway" => ElementClass::Gateway(GatewayType::EventBased),
        "complexGateway" => ElementClass::Gateway(GatewayType::Complex),
        "subProcess" | "adHocSubProcess" => ElementClass::SubProcess,
        "boundaryEvent" => ElementClass::Boundary,
        "sequenceFlow" => ElementClass::SeqFlow,
        "laneSet"
        | "lane"
        | "dataObjectReference"
        | "dataStoreReference"
        | "dataObject"
        | "dataStore"
        | "textAnnotation"
        | "association"
        | "extensionElements"
        | "incoming"
        | "outgoing"
        | "documentation"
        | "ioSpecification"
        | "artifact"
        | "group" => ElementClass::Infra,
        _ => ElementClass::Unknown,
    }
}

// ─── Event body parser ───────────────────────────────────────────────────────

fn parse_event_body(
    attrs_elem: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
    closing_tag: &str,
) -> Result<BpmnEvent> {
    let id = attr_str(attrs_elem, b"id").unwrap_or_default();
    let name = attr_str(attrs_elem, b"name");
    let mut event_type = EventType::None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(ref e) => {
                let tag = strip_prefix(e.name().as_ref());
                event_type = event_def_from_tag(&tag, e);
                skip_subtree(reader, &tag)?;
            }
            Event::Empty(ref e) => {
                let tag = strip_prefix(e.name().as_ref());
                event_type = event_def_from_tag(&tag, e);
            }
            Event::End(ref e)
                if strip_prefix(e.name().as_ref()) == closing_tag => {
                    break;
                }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(BpmnEvent { id, name, event_type })
}

fn event_def_from_tag(tag: &str, e: &quick_xml::events::BytesStart) -> EventType {
    match tag {
        "messageEventDefinition" => EventType::Message {
            message_ref: attr_str(e, b"messageRef"),
        },
        "timerEventDefinition" => EventType::Timer {
            timer_def: attr_str(e, b"timeCycle")
                .or_else(|| attr_str(e, b"timeDuration"))
                .or_else(|| attr_str(e, b"timeDate")),
        },
        "errorEventDefinition" => EventType::Error {
            error_ref: attr_str(e, b"errorRef"),
        },
        "signalEventDefinition" => EventType::Signal {
            signal_ref: attr_str(e, b"signalRef"),
        },
        "escalationEventDefinition" => EventType::Escalation {
            escalation_ref: attr_str(e, b"escalationRef"),
        },
        "terminateEventDefinition" => EventType::Terminate,
        "compensateEventDefinition" | "compensationEventDefinition" => EventType::Compensation,
        "linkEventDefinition" => EventType::Link {
            link_name: attr_str(e, b"name"),
        },
        _ => EventType::None,
    }
}

// ─── Task body parser ────────────────────────────────────────────────────────

fn parse_task_body(
    attrs_elem: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
    task_type: TaskType,
    closing_tag: &str,
) -> Result<BpmnTask> {
    let id = attr_str(attrs_elem, b"id").unwrap_or_default();
    let name = attr_str(attrs_elem, b"name");
    let assignee = attr_str(attrs_elem, b"assignee");

    // Grab top-level camunda attributes for implementation hint
    let mut implementation = attr_str(attrs_elem, b"topic")
        .or_else(|| attr_str(attrs_elem, b"class"))
        .or_else(|| attr_str(attrs_elem, b"expression"));

    let mut camunda_props: HashMap<String, String> = HashMap::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let tag = strip_prefix(e.name().as_ref());
                match tag.as_str() {
                    "property" => {
                        if let (Some(k), Some(v)) =
                            (attr_str(e, b"name"), attr_str(e, b"value"))
                        {
                            if k == "topic" && implementation.is_none() {
                                implementation = Some(v.clone());
                            }
                            camunda_props.insert(k, v);
                        }
                    }
                    "taskDefinition" => {
                        // Zeebe task definition: type = worker topic
                        if let Some(t) = attr_str(e, b"type") {
                            if implementation.is_none() {
                                implementation = Some(t);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Event::End(ref e)
                if strip_prefix(e.name().as_ref()) == closing_tag => {
                    break;
                }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(BpmnTask {
        id,
        name,
        task_type,
        implementation,
        assignee,
        camunda_props,
    })
}

// ─── Boundary event parser ───────────────────────────────────────────────────

fn parse_boundary_body(
    attrs_elem: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
) -> Result<BpmnBoundaryEvent> {
    let id = attr_str(attrs_elem, b"id").unwrap_or_default();
    let name = attr_str(attrs_elem, b"name");
    let attached_to_ref = attr_str(attrs_elem, b"attachedToRef").unwrap_or_default();
    let cancel_activity = attr_str(attrs_elem, b"cancelActivity")
        .map(|v| v != "false")
        .unwrap_or(true);

    let ev = parse_event_body(attrs_elem, reader, "boundaryEvent")?;

    Ok(BpmnBoundaryEvent {
        id,
        name,
        attached_to_ref,
        cancel_activity,
        event_type: ev.event_type,
    })
}

// ─── Sequence flow condition parser ─────────────────────────────────────────

/// Drain to the sequence flow closing tag; if a `<conditionExpression>` child
/// is found, capture its text content.
fn parse_seq_flow_condition(
    reader: &mut Reader<&[u8]>,
    closing_tag: &str,
) -> Result<Option<String>> {
    let mut condition: Option<String> = None;
    let mut buf = Vec::new();
    let mut in_condition = false;

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(ref e) => {
                let tag = strip_prefix(e.name().as_ref());
                if tag == "conditionExpression" {
                    in_condition = true;
                }
            }
            Event::Empty(ref e) => {
                let tag = strip_prefix(e.name().as_ref());
                if tag == "conditionExpression" {
                    // self-closing conditionExpression with no text — skip
                }
            }
            Event::Text(ref t)
                if in_condition => {
                    condition = Some(t.unescape().unwrap_or_default().to_string());
                }
            Event::End(ref e) => {
                let tag = strip_prefix(e.name().as_ref());
                if tag == "conditionExpression" {
                    in_condition = false;
                } else if tag == closing_tag {
                    break;
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(condition)
}

// ─── Construction helpers ────────────────────────────────────────────────────

fn make_gateway(e: &quick_xml::events::BytesStart, gateway_type: GatewayType) -> BpmnGateway {
    BpmnGateway {
        id: attr_str(e, b"id").unwrap_or_default(),
        name: attr_str(e, b"name"),
        gateway_type,
    }
}

fn make_seq_flow(
    e: &quick_xml::events::BytesStart,
    condition_expression: Option<String>,
) -> SequenceFlow {
    SequenceFlow {
        id: attr_str(e, b"id").unwrap_or_default(),
        source_ref: attr_str(e, b"sourceRef").unwrap_or_default(),
        target_ref: attr_str(e, b"targetRef").unwrap_or_default(),
        name: attr_str(e, b"name"),
        condition_expression,
    }
}

// ─── Low-level helpers ───────────────────────────────────────────────────────

/// Strip namespace prefix: `bpmn:startEvent` → `startEvent`.
pub(crate) fn strip_prefix(tag: &[u8]) -> String {
    let s = std::str::from_utf8(tag).unwrap_or("");
    if let Some(pos) = s.find(':') {
        s[pos + 1..].to_string()
    } else {
        s.to_string()
    }
}

/// Read an attribute value by local name, ignoring namespace prefix.
fn attr_str(e: &quick_xml::events::BytesStart, name: &[u8]) -> Option<String> {
    let name_str = std::str::from_utf8(name).ok()?;
    let search_local = if let Some(p) = name_str.find(':') {
        &name_str[p + 1..]
    } else {
        name_str
    };

    for attr in e.attributes().flatten() {
        let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
        let key_local = if let Some(p) = key.find(':') { &key[p + 1..] } else { key };
        if key_local == search_local {
            return attr.unescape_value().ok().map(|v| v.to_string());
        }
    }
    None
}

/// Consume events until the matching end tag, handling nesting.
fn skip_subtree(reader: &mut Reader<&[u8]>, closing: &str) -> Result<()> {
    let mut depth = 1usize;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(ref e)
                if strip_prefix(e.name().as_ref()) == closing => {
                    depth += 1;
                }
            Event::End(ref e)
                if strip_prefix(e.name().as_ref()) == closing => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}
