//! SVG shape primitives and layout types for the dsl-render crate.

use dsl_bpmn_frontend::{GatewayKind, NodeKind, RailwayEdge};

// ---------------------------------------------------------------------------
// Layout types
// ---------------------------------------------------------------------------

/// A 2-D point in SVG canvas coordinates.
#[derive(Debug, Clone)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// Layout information for a single node.
#[derive(Debug, Clone)]
pub struct NodeLayout {
    pub id: String,
    pub pos: Point,
    pub width: f64,
    pub height: f64,
}

// ---------------------------------------------------------------------------
// Arrowhead marker definitions (regular string literal — avoids raw-string
// complications with hex color codes containing `"#` sequences).
// ---------------------------------------------------------------------------

pub const ARROWHEAD_DEF: &str =
    "  <marker id=\"arrowhead\" markerWidth=\"10\" markerHeight=\"7\" \
refX=\"9\" refY=\"3.5\" orient=\"auto\">\n    \
<polygon points=\"0 0, 10 3.5, 0 7\" fill=\"#555\" />\n  \
</marker>\n  \
<marker id=\"arrowhead-default\" markerWidth=\"10\" markerHeight=\"7\" \
refX=\"9\" refY=\"3.5\" orient=\"auto\">\n    \
<polygon points=\"0 0, 10 3.5, 0 7\" fill=\"#999\" />\n  \
</marker>";

// ---------------------------------------------------------------------------
// Node rendering
// ---------------------------------------------------------------------------

/// Render an SVG group for a process node.
pub fn render_node_shape(
    layout: &NodeLayout,
    kind: &NodeKind,
    label: &str,
    pack_badge: Option<&String>,
    include_labels: bool,
) -> String {
    let x = layout.pos.x;
    let y = layout.pos.y;
    let w = layout.width;
    let h = layout.height;
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;

    let (shape, css_class) = match kind {
        NodeKind::StartEvent => (
            circle(cx, cy, 20.0, "#22c55e", "#166534", 2.0),
            "start-event",
        ),
        NodeKind::StartEventMessage
        | NodeKind::StartEventTimer
        | NodeKind::StartEventSignal
        | NodeKind::StartEventError
        | NodeKind::StartEventEscalation
        | NodeKind::StartEventCompensation => (
            circle(cx, cy, 20.0, "#16a34a", "#166534", 2.0),
            "start-event",
        ),
        NodeKind::EndEvent => (
            circle(cx, cy, 20.0, "#ef4444", "#7f1d1d", 4.0),
            "end-event",
        ),
        NodeKind::EndEventTerminate => (
            circle(cx, cy, 20.0, "#991b1b", "#7f1d1d", 4.0),
            "end-event",
        ),
        NodeKind::EndEventError
        | NodeKind::EndEventMessage
        | NodeKind::EndEventSignal
        | NodeKind::EndEventCancel
        | NodeKind::EndEventEscalation
        | NodeKind::EndEventCompensation => (
            circle(cx, cy, 20.0, "#dc2626", "#7f1d1d", 4.0),
            "end-event",
        ),
        NodeKind::IntermediateCatchMessage
        | NodeKind::IntermediateCatchTimer
        | NodeKind::IntermediateCatchSignal
        | NodeKind::IntermediateCatchLink
        | NodeKind::IntermediateThrowMessage
        | NodeKind::IntermediateThrowSignal
        | NodeKind::IntermediateThrowLink
        | NodeKind::IntermediateThrowEscalation
        | NodeKind::IntermediateThrowCompensation => (
            circle(cx, cy, 20.0, "#fbbf24", "#92400e", 2.0),
            "intermediate-event",
        ),
        NodeKind::ServiceTask
        | NodeKind::ScriptTask
        | NodeKind::SendTask
        | NodeKind::ReceiveTask => (
            rect_rounded(x, y, w, h, "#dbeafe", "#1d4ed8"),
            "service-task",
        ),
        NodeKind::UserTask | NodeKind::ManualTask => (
            rect_rounded(x, y, w, h, "#ede9fe", "#5b21b6"),
            "user-task",
        ),
        NodeKind::BusinessRuleTask => (
            rect_rounded(x, y, w, h, "#e0e7ff", "#3730a3"),
            "business-rule-task",
        ),
        NodeKind::Subprocess
        | NodeKind::EventSubprocess
        | NodeKind::TransactionSubprocess
        | NodeKind::CallActivity => (
            rect_rounded(x, y, w, h, "#f3f4f6", "#6b7280"),
            "subprocess",
        ),
    };

    let label_svg = if include_labels {
        text_centered(cx, cy, label, 11.0, "#1f2937")
    } else {
        String::new()
    };

    // Pack provenance badge: small amber square in the top-right corner
    let badge = pack_badge
        .map(|_| {
            format!(
                "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"8\" height=\"8\" fill=\"#f59e0b\" rx=\"2\"/>",
                x + w - 10.0,
                y + 2.0
            )
        })
        .unwrap_or_default();

    let title = format!("<title>{}</title>", escape_xml(label));

    format!(
        "<g class=\"{}\">{}{}{}{}</g>\n",
        css_class, title, shape, label_svg, badge
    )
}

// ---------------------------------------------------------------------------
// Gateway rendering
// ---------------------------------------------------------------------------

/// Render an SVG group for a gateway.
pub fn render_gateway_shape(
    layout: &NodeLayout,
    kind: &GatewayKind,
    label: &str,
    pack_badge: Option<&String>,
    include_labels: bool,
) -> String {
    let x = layout.pos.x;
    let y = layout.pos.y;
    let w = layout.width;
    let h = layout.height;
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;

    let (fill, stroke, inner) = match kind {
        GatewayKind::Exclusive => (
            "#fed7aa",
            "#c2410c",
            // U+00D7 MULTIPLICATION SIGN = ×, rendered as &#xD7;
            format!(
                "<text x=\"{:.0}\" y=\"{:.0}\" text-anchor=\"middle\" \
dominant-baseline=\"middle\" font-size=\"20\" fill=\"#c2410c\" \
font-weight=\"bold\">&#xD7;</text>",
                cx,
                cy + 1.0
            ),
        ),
        GatewayKind::Inclusive => (
            "#fef9c3",
            "#a16207",
            format!(
                "<circle cx=\"{:.0}\" cy=\"{:.0}\" r=\"10\" fill=\"none\" \
stroke=\"#a16207\" stroke-width=\"2\"/>",
                cx, cy
            ),
        ),
        GatewayKind::Parallel => (
            "#ccfbf1",
            "#0f766e",
            format!(
                "<text x=\"{:.0}\" y=\"{:.0}\" text-anchor=\"middle\" \
dominant-baseline=\"middle\" font-size=\"20\" fill=\"#0f766e\" \
font-weight=\"bold\">+</text>",
                cx,
                cy + 1.0
            ),
        ),
        GatewayKind::EventBased | GatewayKind::ParallelEventBased => (
            "#ede9fe",
            "#5b21b6",
            format!(
                "<circle cx=\"{:.0}\" cy=\"{:.0}\" r=\"10\" fill=\"none\" \
stroke=\"#5b21b6\" stroke-width=\"2\"/>",
                cx, cy
            ),
        ),
    };

    let diamond = diamond_shape(cx, cy, w / 2.0, h / 2.0, fill, stroke);
    let label_svg = if include_labels && !label.is_empty() {
        text_centered(cx, cy + h / 2.0 + 14.0, label, 10.0, "#374151")
    } else {
        String::new()
    };

    let badge = pack_badge
        .map(|_| {
            format!(
                "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"8\" height=\"8\" fill=\"#f59e0b\" rx=\"2\"/>",
                cx + w / 2.0 - 10.0,
                y + 2.0
            )
        })
        .unwrap_or_default();

    format!(
        "<g class=\"gateway\"><title>{}</title>{}{}{}{}</g>\n",
        escape_xml(label),
        diamond,
        inner,
        label_svg,
        badge
    )
}

// ---------------------------------------------------------------------------
// Edge rendering
// ---------------------------------------------------------------------------

/// Render an SVG path for a sequence flow edge.
pub fn render_edge(
    src: &NodeLayout,
    tgt: &NodeLayout,
    edge: &RailwayEdge,
    include_labels: bool,
) -> String {
    // Connect right edge of source to left edge of target
    let x1 = src.pos.x + src.width;
    let y1 = src.pos.y + src.height / 2.0;
    let x2 = tgt.pos.x;
    let y2 = tgt.pos.y + tgt.height / 2.0;

    // Cubic bezier for smoother routing
    let mx = (x1 + x2) / 2.0;
    let path = format!(
        "M {:.1},{:.1} C {:.1},{:.1} {:.1},{:.1} {:.1},{:.1}",
        x1, y1, mx, y1, mx, y2, x2, y2
    );

    let stroke = if edge.is_default { "#9ca3af" } else { "#4b5563" };
    let dash = if edge.is_default {
        " stroke-dasharray=\"5,3\""
    } else {
        ""
    };
    let marker = if edge.is_default {
        "arrowhead-default"
    } else {
        "arrowhead"
    };

    let label_svg = if include_labels {
        if let Some(cond) = &edge.condition {
            let lx = (x1 + x2) / 2.0;
            let ly = (y1 + y2) / 2.0 - 8.0;
            let short: String = cond.chars().take(20).collect();
            format!(
                "<text x=\"{:.0}\" y=\"{:.0}\" font-size=\"9\" fill=\"#6b7280\" \
text-anchor=\"middle\" font-family=\"sans-serif\">{}</text>",
                lx,
                ly,
                escape_xml(&short)
            )
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    format!(
        "<path d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"1.5\"{} \
marker-end=\"url({}{}{})\"/>{}\n",
        path, stroke, dash, "(#", marker, ")", label_svg
    )
}

// ---------------------------------------------------------------------------
// Boundary event rendering
// ---------------------------------------------------------------------------

/// Render a small boundary event circle attached near the host node.
pub fn render_boundary_event(
    host_layout: &NodeLayout,
    event_id: &str,
    event_kind: &str,
    interrupting: bool,
) -> String {
    // Spread boundary events along the bottom edge by hashing the event name
    let hash: usize = event_id.bytes().map(|b| b as usize).sum::<usize>() % 3;
    let cx = host_layout.pos.x + host_layout.width * 0.25 + hash as f64 * 30.0;
    let cy = host_layout.pos.y + host_layout.height;

    let fill = match event_kind {
        "error" => "#fca5a5",
        "timer" => "#fde68a",
        "message" => "#bfdbfe",
        "signal" => "#d9f99d",
        "escalation" => "#fde68a",
        "compensation" => "#e9d5ff",
        "cancel" => "#fca5a5",
        _ => "#e5e7eb",
    };
    let stroke_dash = if interrupting {
        String::new()
    } else {
        " stroke-dasharray=\"3,2\"".to_owned()
    };

    format!(
        "<circle cx=\"{:.0}\" cy=\"{:.0}\" r=\"10\" fill=\"{}\" \
stroke=\"#374151\" stroke-width=\"1.5\"{}/>\n",
        cx, cy, fill, stroke_dash
    )
}

// ---------------------------------------------------------------------------
// Parallel join rendering
// ---------------------------------------------------------------------------

/// Render a parallel join as a filled teal rectangle.
pub fn render_parallel_join(layout: &NodeLayout, label: &str) -> String {
    let x = layout.pos.x;
    let y = layout.pos.y;
    let w = layout.width;
    let h = layout.height;
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;

    let shape = rect_rounded(x, y, w, h, "#ccfbf1", "#0f766e");
    let text = text_centered(cx, cy, label, 10.0, "#0f766e");
    let title = format!("<title>{}</title>", escape_xml(label));

    format!("<g class=\"parallel-join\">{}{}{}</g>\n", title, shape, text)
}

// ---------------------------------------------------------------------------
// SVG primitive helpers
// ---------------------------------------------------------------------------

pub(crate) fn circle(cx: f64, cy: f64, r: f64, fill: &str, stroke: &str, sw: f64) -> String {
    format!(
        "<circle cx=\"{:.0}\" cy=\"{:.0}\" r=\"{:.0}\" fill=\"{}\" \
stroke=\"{}\" stroke-width=\"{:.0}\"/>",
        cx, cy, r, fill, stroke, sw
    )
}

pub(crate) fn rect_rounded(x: f64, y: f64, w: f64, h: f64, fill: &str, stroke: &str) -> String {
    format!(
        "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{:.0}\" height=\"{:.0}\" rx=\"6\" ry=\"6\" \
fill=\"{}\" stroke=\"{}\" stroke-width=\"1.5\"/>",
        x, y, w, h, fill, stroke
    )
}

pub(crate) fn diamond_shape(
    cx: f64,
    cy: f64,
    rx: f64,
    ry: f64,
    fill: &str,
    stroke: &str,
) -> String {
    let points = format!(
        "{:.0},{:.0} {:.0},{:.0} {:.0},{:.0} {:.0},{:.0}",
        cx,
        cy - ry, // top
        cx + rx,
        cy, // right
        cx,
        cy + ry, // bottom
        cx - rx,
        cy // left
    );
    format!(
        "<polygon points=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1.5\"/>",
        points, fill, stroke
    )
}

pub(crate) fn text_centered(x: f64, y: f64, text: &str, size: f64, color: &str) -> String {
    let short: String = text.chars().take(16).collect();
    format!(
        "<text x=\"{:.0}\" y=\"{:.0}\" text-anchor=\"middle\" \
dominant-baseline=\"middle\" font-size=\"{}\" fill=\"{}\" \
font-family=\"sans-serif\">{}</text>",
        x,
        y,
        size,
        color,
        escape_xml(&short)
    )
}

/// Escape XML special characters in text content and attribute values.
pub(crate) fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
