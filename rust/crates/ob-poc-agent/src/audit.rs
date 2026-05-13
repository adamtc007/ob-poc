//! Audit emission for the Sage ACP runtime.
//!
//! Phase 2.9 of the Sage ACP capability plan introduced the
//! [`AuditRecord`] + [`JsonlAuditSink`]; Phase 5.3 (this module)
//! adds the OTLP HTTP+JSON exporter and the [`MultiAuditSink`] fan-
//! out so a single planning round-trip writes once and is observed
//! both locally (JSONL) and centrally (OTLP collector → backend of
//! choice). Every prompt that traverses the planning loop produces
//! one [`AuditRecord`] that captures the inputs, the draft, the
//! validation outcome, and the goal-frame metadata.
//!
//! ## Replay-grade audit
//!
//! Each record carries:
//! - `goal_frame_id`, `created_at` — correlate against the editor
//!   session and the agent's own trace.
//! - `pack_id`, `pack_hash` — the manifest the agent saw is replay-
//!   verifiable byte-for-byte (matches V&S §6.5 audit invariant).
//! - `verb_fqn`, `draft_source` — the constrained-composition pick.
//! - `validation_passed`, `validation_diagnostic_count` — proof the
//!   draft cleared the LSP-shaped channel before the response.
//! - `knowledge_provider` — which knowledge transport answered the
//!   substrate query (Phase 4 distinguishes between the spike stub
//!   and the real `sem_os_mcp` client).
//!
//! ## Reliability
//!
//! `JsonlAuditSink::emit` is best-effort: on IO failure it logs at
//! warn level and continues. Audit is for post-hoc analysis, not for
//! gating execution. Phase 5.3 adds OTLP push as the durable
//! companion sink.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use crate::planning::{DraftSource, PlanningOutcome};
use crate::repl_channel::ValidationOutcome;

/// One audited prompt round-trip. JSON-serialised as a single line
/// to the local sink. The shape is stable across spike and Phase 5
/// — the OTLP exporter wraps the same record in an OTLP event
/// envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    /// Goal frame correlation id (`gf-<uuid>`).
    pub goal_frame_id: String,
    /// When the record was emitted.
    pub emitted_at: DateTime<Utc>,
    /// When the goal frame itself was constructed (planning round-
    /// trip start).
    pub created_at: DateTime<Utc>,
    /// Utterance text, length-truncated. Phase 4 redaction policy
    /// hashes / classifies before persisting.
    pub utterance: String,
    /// Pack the session anchored against.
    pub pack_id: String,
    /// SHA-256 of pack manifest YAML (replay invariant).
    pub pack_hash: String,
    /// Workspace tag (matches the serde-rename used elsewhere in
    /// the system — e.g. `cbu`, `onboarding_request`).
    pub workspace: String,
    /// Optional intent summary from the goal frame. `None` in Phase
    /// 2; Phase 3.4 fills it once the motivation prompt template
    /// lands.
    pub intent_summary: Option<String>,
    /// Verb FQN the planning loop drafted.
    pub verb_fqn: String,
    /// Identifier for which call site produced the draft.
    pub draft_source: String,
    /// Whether the LSP-shaped channel accepted the draft.
    pub validation_passed: bool,
    /// Diagnostic count from the channel.
    pub validation_diagnostic_count: usize,
    /// Knowledge provider label (`stub`, `phase-2-spike`,
    /// `sem_os_mcp@…`, etc.) or `none` when no client was wired.
    pub knowledge_provider: String,
}

impl AuditRecord {
    /// Build a record from the planning loop's outputs.
    pub fn from_outcome(
        outcome: &PlanningOutcome,
        validation: &ValidationOutcome,
        knowledge_provider: Option<&str>,
    ) -> Self {
        let draft_source = match outcome.source {
            DraftSource::LlmTool => "llm_tool",
            DraftSource::DeterministicFallback => "deterministic_fallback",
        }
        .to_string();
        Self {
            goal_frame_id: outcome.goal_frame.id.clone(),
            emitted_at: Utc::now(),
            created_at: outcome.goal_frame.created_at,
            utterance: outcome.goal_frame.utterance.clone(),
            pack_id: outcome.goal_frame.pack_id.clone(),
            pack_hash: outcome.goal_frame.pack_hash.clone(),
            workspace: outcome.goal_frame.workspace.clone(),
            intent_summary: outcome.goal_frame.intent_summary.clone(),
            verb_fqn: outcome.verb_fqn.clone(),
            draft_source,
            validation_passed: validation.passed(),
            validation_diagnostic_count: validation.diagnostics.len(),
            knowledge_provider: knowledge_provider.unwrap_or("none").to_string(),
        }
    }
}

/// Where audit records go. The spike emits to JSONL; Phase 5 adds
/// `OtlpAuditSink` and a `MultiSink` that fans out to both.
#[async_trait]
pub trait AuditSink: Send + Sync {
    async fn emit(&self, record: AuditRecord);
}

/// Best-effort JSONL sink. Appends one JSON object per line.
///
/// The mutex serialises writes so concurrent `emit` calls cannot
/// interleave bytes within a record. The Phase 2 binary handles one
/// prompt at a time so this is the simplest correct shape; Phase 4
/// can swap for an MPSC-backed writer task if concurrent emission
/// becomes a bottleneck.
pub struct JsonlAuditSink {
    path: PathBuf,
    write_lock: Mutex<()>,
}

impl JsonlAuditSink {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            write_lock: Mutex::new(()),
        }
    }

    /// The file the sink writes to.
    pub fn path(&self) -> &Path {
        &self.path
    }

    async fn append_line(&self, line: String) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;
        file.write_all(line.as_bytes()).await?;
        file.write_all(b"\n").await?;
        file.flush().await?;
        Ok(())
    }
}

#[async_trait]
impl AuditSink for JsonlAuditSink {
    async fn emit(&self, record: AuditRecord) {
        let Ok(line) = serde_json::to_string(&record) else {
            tracing::warn!(
                target: "sage-acp",
                "failed to serialise audit record; dropping"
            );
            return;
        };
        let _guard = self.write_lock.lock().await;
        if let Err(error) = self.append_line(line).await {
            tracing::warn!(
                target: "sage-acp",
                %error,
                path = %self.path.display(),
                "audit emission failed"
            );
        }
    }
}

/// Discards records. Useful when running without a writable sink
/// (e.g. tests, CI smoke). The binary integrator picks this when
/// `OBPOC_SAGE_AUDIT` is set to `none`.
#[derive(Debug, Default, Clone)]
pub struct NullAuditSink;

#[async_trait]
impl AuditSink for NullAuditSink {
    async fn emit(&self, _record: AuditRecord) {
        // Drop on the floor.
    }
}

/// Fan-out sink. Emits a record into every inner sink, awaiting
/// each in sequence. The spike has at most one prompt in flight so
/// sequential emission is the simplest correct shape. Phase 6+ can
/// fan out concurrently via `futures::join_all` once latency
/// becomes a measured concern.
///
/// Per-sink failures are isolated by each implementation
/// (`JsonlAuditSink` and `OtlpAuditSink` both `tracing::warn` and
/// continue); `MultiAuditSink` never short-circuits.
pub struct MultiAuditSink {
    sinks: Vec<Box<dyn AuditSink>>,
    label: String,
}

impl MultiAuditSink {
    pub fn new(sinks: Vec<Box<dyn AuditSink>>) -> Self {
        let label = sinks
            .iter()
            .map(|_| "sink")
            .collect::<Vec<_>>()
            .join(",");
        Self { sinks, label }
    }

    /// Builder convenience — overrides the diagnostic label
    /// (`"jsonl+otlp"`, etc.). The binary integrator threads this
    /// into the startup log so operators see which sinks are wired.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn sink_count(&self) -> usize {
        self.sinks.len()
    }
}

#[async_trait]
impl AuditSink for MultiAuditSink {
    async fn emit(&self, record: AuditRecord) {
        for sink in &self.sinks {
            sink.emit(record.clone()).await;
        }
    }
}

/// OTLP HTTP+JSON exporter — Phase 5.3.
///
/// Each [`AuditRecord`] is wrapped in a minimal OTLP `LogsData`
/// envelope (`ResourceLogs[ScopeLogs[LogRecord]]`) and POSTed as
/// `application/json` to the configured endpoint. The endpoint
/// should point at an OTLP collector's `/v1/logs` route (or the
/// equivalent on a backend that accepts OTLP/HTTP+JSON directly).
///
/// ## Best-effort semantics
///
/// Emission is non-blocking from the agent's perspective: failures
/// are logged at `warn` and dropped. The planning loop never waits
/// on a slow collector — V&S §6.9 lists OTLP push as a durability
/// companion to the local JSONL sink, not as a gate.
///
/// ## Wire format
///
/// The exporter follows the OTLP/HTTP+JSON wire format
/// (opentelemetry-proto v1.0.0). Each record produces one
/// `LogRecord` with severity `INFO` (9). The `AuditRecord` fields
/// project to OTLP attributes one-for-one so a collector can route
/// or filter without parsing the body.
pub struct OtlpAuditSink {
    endpoint: String,
    service_name: String,
    client: reqwest::Client,
}

impl OtlpAuditSink {
    /// Construct an OTLP sink pointing at `endpoint`. `service_name`
    /// is stamped as the `service.name` resource attribute so a
    /// shared collector can multiplex traffic from multiple Sage
    /// agents.
    ///
    /// Builds a long-lived `reqwest::Client` with a 5-second total
    /// request timeout so a misconfigured collector cannot stall
    /// the planning loop.
    pub fn new(endpoint: impl Into<String>, service_name: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("reqwest client builder with default config is infallible");
        Self {
            endpoint: endpoint.into(),
            service_name: service_name.into(),
            client,
        }
    }

    /// The endpoint URL the sink POSTs to. Used by the binary
    /// integrator's startup log.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Build the OTLP/HTTP+JSON envelope for a record. Pure function
    /// — separated from `emit` so tests can assert the wire shape
    /// without spinning up an HTTP server.
    pub fn build_payload(&self, record: &AuditRecord) -> serde_json::Value {
        // Per OTLP/HTTP+JSON: timeUnixNano is a string-encoded i64
        // (the JSON spec restricts numbers, so 64-bit ints travel
        // as strings). chrono's `timestamp_nanos_opt` returns
        // `Option<i64>` — None only when the timestamp is out of
        // i64 range (year ~2262), which we never produce; default
        // to 0 in that impossible case.
        let time_unix_nano = record
            .emitted_at
            .timestamp_nanos_opt()
            .unwrap_or_default()
            .to_string();
        let attributes = vec![
            string_attribute("goal_frame_id", &record.goal_frame_id),
            string_attribute("pack_id", &record.pack_id),
            string_attribute("pack_hash", &record.pack_hash),
            string_attribute("workspace", &record.workspace),
            string_attribute("verb_fqn", &record.verb_fqn),
            string_attribute("draft_source", &record.draft_source),
            string_attribute("knowledge_provider", &record.knowledge_provider),
            bool_attribute("validation_passed", record.validation_passed),
            int_attribute(
                "validation_diagnostic_count",
                record.validation_diagnostic_count as i64,
            ),
        ];
        serde_json::json!({
            "resourceLogs": [{
                "resource": {
                    "attributes": [
                        string_attribute("service.name", &self.service_name),
                    ]
                },
                "scopeLogs": [{
                    "scope": {
                        "name": "ob-poc-agent.audit",
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                    "logRecords": [{
                        "timeUnixNano": time_unix_nano,
                        "severityNumber": 9,
                        "severityText": "INFO",
                        "body": {"stringValue": format!(
                            "sage planning round-trip — {} ({})",
                            record.verb_fqn, record.draft_source
                        )},
                        "attributes": attributes,
                    }]
                }]
            }]
        })
    }
}

fn string_attribute(key: &str, value: &str) -> serde_json::Value {
    serde_json::json!({
        "key": key,
        "value": {"stringValue": value}
    })
}

fn bool_attribute(key: &str, value: bool) -> serde_json::Value {
    serde_json::json!({
        "key": key,
        "value": {"boolValue": value}
    })
}

fn int_attribute(key: &str, value: i64) -> serde_json::Value {
    serde_json::json!({
        "key": key,
        "value": {"intValue": value.to_string()}
    })
}

#[async_trait]
impl AuditSink for OtlpAuditSink {
    async fn emit(&self, record: AuditRecord) {
        let payload = self.build_payload(&record);
        match self
            .client
            .post(&self.endpoint)
            .json(&payload)
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status();
                if !status.is_success() {
                    let body = response.text().await.unwrap_or_default();
                    tracing::warn!(
                        target: "sage-acp",
                        endpoint = %self.endpoint,
                        %status,
                        body = %truncate(&body, 256),
                        "OTLP collector returned non-success status — record dropped"
                    );
                }
            }
            Err(error) => {
                tracing::warn!(
                    target: "sage-acp",
                    endpoint = %self.endpoint,
                    %error,
                    "OTLP push failed — record dropped"
                );
            }
        }
    }
}

/// Truncate a string to at most `max` bytes, appending `…` if cut.
/// Used for log lines that quote a collector response body so a
/// chatty collector cannot blow up log volume.
fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    // Walk back to a UTF-8 char boundary.
    let mut cut = max;
    while !s.is_char_boundary(cut) && cut > 0 {
        cut -= 1;
    }
    format!("{}…", &s[..cut])
}

/// Resolution outcome for the OTLP endpoint env var.
/// `Disabled` when `OBPOC_SAGE_OTLP_ENDPOINT` is unset / empty;
/// `Endpoint(url)` otherwise. The binary integrator constructs the
/// sink only on `Endpoint`.
#[derive(Debug, Clone)]
pub enum OtlpEndpoint {
    Endpoint(String),
    Disabled,
}

/// Read `OBPOC_SAGE_OTLP_ENDPOINT`. Empty values are treated
/// identically to unset (matches the binary's `ANTHROPIC_API_KEY`
/// handling — an empty env var is no env var).
pub fn default_otlp_endpoint() -> OtlpEndpoint {
    match std::env::var("OBPOC_SAGE_OTLP_ENDPOINT") {
        Ok(value) if !value.trim().is_empty() => OtlpEndpoint::Endpoint(value),
        _ => OtlpEndpoint::Disabled,
    }
}

/// Default audit-file path discovery for the binary integrator.
/// Resolution order:
/// 1. `OBPOC_SAGE_AUDIT` env (explicit path or the literal `none` to
///    disable).
/// 2. `$XDG_STATE_HOME/sage-acp/audit.jsonl` if `XDG_STATE_HOME` set.
/// 3. `$HOME/.cache/sage-acp/audit.jsonl`.
/// 4. `./sage-acp-audit.jsonl` as the last-resort fallback.
pub fn default_audit_path() -> AuditPath {
    if let Ok(value) = std::env::var("OBPOC_SAGE_AUDIT") {
        if value == "none" {
            return AuditPath::Disabled;
        }
        return AuditPath::File(PathBuf::from(value));
    }
    if let Ok(xdg_state) = std::env::var("XDG_STATE_HOME") {
        return AuditPath::File(PathBuf::from(xdg_state).join("sage-acp").join("audit.jsonl"));
    }
    if let Ok(home) = std::env::var("HOME") {
        return AuditPath::File(
            PathBuf::from(home)
                .join(".cache")
                .join("sage-acp")
                .join("audit.jsonl"),
        );
    }
    AuditPath::File(PathBuf::from("sage-acp-audit.jsonl"))
}

/// Outcome of [`default_audit_path`]. Distinguishes "audit disabled
/// by operator" from "audit enabled at <path>".
#[derive(Debug, Clone)]
pub enum AuditPath {
    File(PathBuf),
    Disabled,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goal_frame::{GoalFrame, GoalFrameStatus};
    use crate::repl_channel::DraftDiagnostic;
    use chrono::Utc;
    use tempfile::TempDir;

    fn sample_outcome() -> PlanningOutcome {
        let now = Utc::now();
        PlanningOutcome {
            goal_frame: GoalFrame {
                id: "gf-test-123".to_string(),
                utterance: "set up a book".to_string(),
                pack_id: "book-setup".to_string(),
                pack_hash: "deadbeef".to_string(),
                workspace: "cbu".to_string(),
                intent_summary: None,
                created_at: now,
                updated_at: now,
                status: GoalFrameStatus::Proposed,
                constellation: None,
                frontier: None,
                blockers: None,
                approval: None,
                refused_drafts: Vec::new(),
                active_verb_surface: None,
            },
            verb_fqn: "cbu.create".to_string(),
            source: DraftSource::DeterministicFallback,
        }
    }

    fn empty_validation() -> ValidationOutcome {
        ValidationOutcome {
            source: "(cbu.create)".to_string(),
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn record_from_outcome_captures_inputs() {
        let outcome = sample_outcome();
        let validation = empty_validation();
        let record = AuditRecord::from_outcome(&outcome, &validation, Some("phase-2-spike"));
        assert_eq!(record.goal_frame_id, "gf-test-123");
        assert_eq!(record.pack_id, "book-setup");
        assert_eq!(record.pack_hash, "deadbeef");
        assert_eq!(record.verb_fqn, "cbu.create");
        assert_eq!(record.draft_source, "deterministic_fallback");
        assert!(record.validation_passed);
        assert_eq!(record.validation_diagnostic_count, 0);
        assert_eq!(record.knowledge_provider, "phase-2-spike");
    }

    #[test]
    fn record_marks_failed_validation_and_diagnostic_count() {
        let outcome = sample_outcome();
        let validation = ValidationOutcome {
            source: "garbage".to_string(),
            diagnostics: vec![DraftDiagnostic {
                severity: crate::repl_channel::DraftDiagnosticSeverity::Error,
                message: "parse fail".to_string(),
                line: 1,
                column: 0,
            }],
        };
        let record = AuditRecord::from_outcome(&outcome, &validation, None);
        assert!(!record.validation_passed);
        assert_eq!(record.validation_diagnostic_count, 1);
        assert_eq!(record.knowledge_provider, "none");
    }

    #[tokio::test]
    async fn jsonl_sink_appends_one_line_per_record() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nested").join("audit.jsonl");
        let sink = JsonlAuditSink::new(&path);
        let validation = empty_validation();
        sink.emit(AuditRecord::from_outcome(
            &sample_outcome(),
            &validation,
            Some("stub"),
        ))
        .await;
        sink.emit(AuditRecord::from_outcome(
            &sample_outcome(),
            &validation,
            Some("stub"),
        ))
        .await;
        let contents = tokio::fs::read_to_string(&path).await.unwrap();
        let line_count = contents.lines().count();
        assert_eq!(line_count, 2);
        for line in contents.lines() {
            let parsed: AuditRecord = serde_json::from_str(line).unwrap();
            assert_eq!(parsed.pack_id, "book-setup");
        }
    }

    #[tokio::test]
    async fn null_sink_drops_records() {
        let sink = NullAuditSink;
        let validation = empty_validation();
        // Should not panic / error.
        sink.emit(AuditRecord::from_outcome(
            &sample_outcome(),
            &validation,
            None,
        ))
        .await;
    }

    #[test]
    fn otlp_payload_carries_canonical_attributes() {
        let outcome = sample_outcome();
        let validation = empty_validation();
        let record = AuditRecord::from_outcome(&outcome, &validation, Some("phase-2-spike"));
        let sink = OtlpAuditSink::new("http://localhost:4318/v1/logs", "sage-acp");
        let payload = sink.build_payload(&record);

        // Walk to the single LogRecord and check shape.
        let resource_logs = payload["resourceLogs"].as_array().expect("resourceLogs");
        assert_eq!(resource_logs.len(), 1);
        let scope_logs = resource_logs[0]["scopeLogs"].as_array().expect("scopeLogs");
        assert_eq!(scope_logs.len(), 1);
        let log_records = scope_logs[0]["logRecords"]
            .as_array()
            .expect("logRecords");
        assert_eq!(log_records.len(), 1);
        let log = &log_records[0];

        assert_eq!(log["severityNumber"], 9);
        assert_eq!(log["severityText"], "INFO");

        // Resource attribute carries service.name.
        let resource_attrs = resource_logs[0]["resource"]["attributes"]
            .as_array()
            .expect("resource attrs");
        let service_name = resource_attrs
            .iter()
            .find(|a| a["key"] == "service.name")
            .expect("service.name present");
        assert_eq!(service_name["value"]["stringValue"], "sage-acp");

        // Record fields all project onto LogRecord attributes.
        let attrs = log["attributes"].as_array().expect("attrs");
        let keys: Vec<&str> = attrs
            .iter()
            .filter_map(|a| a["key"].as_str())
            .collect();
        for expected in [
            "goal_frame_id",
            "pack_id",
            "pack_hash",
            "workspace",
            "verb_fqn",
            "draft_source",
            "knowledge_provider",
            "validation_passed",
            "validation_diagnostic_count",
        ] {
            assert!(
                keys.contains(&expected),
                "missing OTLP attribute '{expected}' in {keys:?}"
            );
        }

        // bool + int attrs use the typed variants, not string.
        let validation_passed = attrs
            .iter()
            .find(|a| a["key"] == "validation_passed")
            .unwrap();
        assert_eq!(validation_passed["value"]["boolValue"], true);
        let diag_count = attrs
            .iter()
            .find(|a| a["key"] == "validation_diagnostic_count")
            .unwrap();
        // OTLP/HTTP+JSON encodes int64 as string per the proto3-JSON
        // mapping rules.
        assert_eq!(diag_count["value"]["intValue"], "0");
    }

    #[test]
    fn otlp_payload_time_unix_nano_is_string() {
        let outcome = sample_outcome();
        let validation = empty_validation();
        let record = AuditRecord::from_outcome(&outcome, &validation, None);
        let sink = OtlpAuditSink::new("http://localhost:4318/v1/logs", "sage-acp");
        let payload = sink.build_payload(&record);
        let log = &payload["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0];
        let time = &log["timeUnixNano"];
        let s = time.as_str().expect("timeUnixNano must be string per OTLP JSON");
        // Must parse as i64 and be in the right ballpark (post-2020 nanos).
        let parsed: i64 = s.parse().expect("string must parse to i64");
        assert!(parsed > 1_577_836_800_000_000_000, "got {parsed}");
    }

    #[tokio::test]
    async fn multi_sink_fans_out_to_every_inner_sink() {
        let dir = TempDir::new().unwrap();
        let path_a = dir.path().join("a.jsonl");
        let path_b = dir.path().join("b.jsonl");
        let multi = MultiAuditSink::new(vec![
            Box::new(JsonlAuditSink::new(&path_a)),
            Box::new(JsonlAuditSink::new(&path_b)),
        ])
        .with_label("jsonl+jsonl");
        assert_eq!(multi.sink_count(), 2);
        assert_eq!(multi.label(), "jsonl+jsonl");

        let validation = empty_validation();
        multi
            .emit(AuditRecord::from_outcome(
                &sample_outcome(),
                &validation,
                None,
            ))
            .await;

        for path in [&path_a, &path_b] {
            let contents = tokio::fs::read_to_string(path).await.unwrap();
            assert_eq!(contents.lines().count(), 1, "{} did not get record", path.display());
        }
    }

    #[test]
    fn default_otlp_endpoint_disabled_when_unset_or_empty() {
        let original = std::env::var("OBPOC_SAGE_OTLP_ENDPOINT").ok();

        std::env::remove_var("OBPOC_SAGE_OTLP_ENDPOINT");
        assert!(matches!(default_otlp_endpoint(), OtlpEndpoint::Disabled));

        std::env::set_var("OBPOC_SAGE_OTLP_ENDPOINT", "");
        assert!(matches!(default_otlp_endpoint(), OtlpEndpoint::Disabled));

        std::env::set_var("OBPOC_SAGE_OTLP_ENDPOINT", "   ");
        assert!(matches!(default_otlp_endpoint(), OtlpEndpoint::Disabled));

        std::env::set_var("OBPOC_SAGE_OTLP_ENDPOINT", "http://localhost:4318/v1/logs");
        match default_otlp_endpoint() {
            OtlpEndpoint::Endpoint(url) => assert_eq!(url, "http://localhost:4318/v1/logs"),
            OtlpEndpoint::Disabled => panic!("expected Endpoint"),
        }

        match original {
            Some(value) => std::env::set_var("OBPOC_SAGE_OTLP_ENDPOINT", value),
            None => std::env::remove_var("OBPOC_SAGE_OTLP_ENDPOINT"),
        }
    }

    // Both env-var branches exercised in a single test so cargo's
    // parallel test runner cannot race on the shared env state.
    #[test]
    fn default_audit_path_branches_on_env() {
        let original = std::env::var("OBPOC_SAGE_AUDIT").ok();

        std::env::set_var("OBPOC_SAGE_AUDIT", "none");
        match default_audit_path() {
            AuditPath::Disabled => {}
            AuditPath::File(p) => panic!("expected Disabled, got {}", p.display()),
        }

        std::env::set_var("OBPOC_SAGE_AUDIT", "/tmp/sage-audit-test.jsonl");
        match default_audit_path() {
            AuditPath::File(p) => assert_eq!(p, PathBuf::from("/tmp/sage-audit-test.jsonl")),
            AuditPath::Disabled => panic!("expected File"),
        }

        match original {
            Some(value) => std::env::set_var("OBPOC_SAGE_AUDIT", value),
            None => std::env::remove_var("OBPOC_SAGE_AUDIT"),
        }
    }
}
