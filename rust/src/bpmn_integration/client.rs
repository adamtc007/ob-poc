//! Typed gRPC client wrapper for the BPMN-Lite service.
//!
//! Hides raw proto types behind domain-friendly Rust types.
//! Connection is lazy by default (no network call until first RPC).

use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use tonic::transport::Channel;
use uuid::Uuid;

/// Re-export generated proto types (internal use only)
#[allow(clippy::enum_variant_names)]
pub(crate) mod proto {
    tonic::include_proto!("bpmn_lite.v1");
}

use proto::bpmn_lite_client::BpmnLiteClient;

/// Default gRPC endpoint for BPMN-Lite service.
const DEFAULT_GRPC_URL: &str = "http://[::1]:50052";

/// Environment variable for overriding the gRPC endpoint.
const ENV_GRPC_URL: &str = "BPMN_LITE_GRPC_URL";

// ---------------------------------------------------------------------------
// Domain result types (hide proto from callers)
// ---------------------------------------------------------------------------

/// Result of compiling BPMN XML to bytecode.
#[derive(Debug, Clone)]
pub struct CompileResult {
    pub bytecode_version: Vec<u8>,
    pub diagnostics: Vec<CompileDiagnostic>,
}

/// Single diagnostic from compilation.
#[derive(Debug, Clone)]
pub struct CompileDiagnostic {
    pub severity: String,
    pub message: String,
    pub element_id: String,
}

/// Request to start a BPMN process instance.
#[derive(Debug, Clone)]
pub struct StartProcessRequest {
    pub process_key: String,
    pub bytecode_version: Vec<u8>,
    pub domain_payload: String,
    pub domain_payload_hash: Vec<u8>,
    pub orch_flags: HashMap<String, OrchestratorFlag>,
    pub correlation_id: Uuid,
}

/// Typed orchestrator flag value (maps to ProtoValue oneof).
#[derive(Debug, Clone)]
pub enum OrchestratorFlag {
    Bool(bool),
    Int(i64),
    Str(String),
}

/// Result of inspecting a process instance.
#[derive(Debug, Clone)]
pub struct ProcessInspection {
    pub state: String,
    pub fibers: Vec<FiberSnapshot>,
    pub waits: Vec<WaitSnapshot>,
    pub bytecode_version: Vec<u8>,
    pub domain_payload_hash: String,
}

/// Snapshot of a single fiber in the VM.
#[derive(Debug, Clone)]
pub struct FiberSnapshot {
    pub fiber_id: String,
    pub pc: u32,
    pub wait_state: String,
}

/// Snapshot of a wait condition.
#[derive(Debug, Clone)]
pub struct WaitSnapshot {
    pub fiber_id: String,
    pub wait_type: String,
    pub detail: String,
}

/// A single activated job from the job worker protocol.
#[derive(Debug, Clone)]
pub struct JobActivation {
    pub job_key: String,
    pub process_instance_id: String,
    pub task_type: String,
    pub service_task_id: String,
    pub domain_payload: String,
    pub domain_payload_hash: Vec<u8>,
    pub orch_flags: HashMap<String, OrchestratorFlag>,
    pub retries_remaining: i32,
}

/// Request to complete a job.
#[derive(Debug, Clone)]
pub struct CompleteJobRequest {
    pub job_key: String,
    pub domain_payload: String,
    pub domain_payload_hash: Vec<u8>,
    pub orch_flags: HashMap<String, OrchestratorFlag>,
}

/// A lifecycle event from the event stream.
#[derive(Debug, Clone)]
pub struct BpmnLifecycleEvent {
    pub sequence: u64,
    pub event_type: String,
    pub process_instance_id: String,
    pub payload_json: String,
}

// ---------------------------------------------------------------------------
// Connection
// ---------------------------------------------------------------------------

/// Typed wrapper around the generated BPMN-Lite gRPC client.
///
/// Provides domain-friendly methods that hide proto serialization.
/// Cloneable — tonic channels are internally reference-counted.
#[derive(Debug, Clone)]
pub struct BpmnLiteConnection {
    client: BpmnLiteClient<Channel>,
    url: String,
}

impl BpmnLiteConnection {
    /// Create a lazy connection (no network call until first RPC).
    ///
    /// Uses `BPMN_LITE_GRPC_URL` env var, falling back to `http://[::1]:50052`.
    pub fn from_env() -> Result<Self> {
        let url = std::env::var(ENV_GRPC_URL).unwrap_or_else(|_| DEFAULT_GRPC_URL.to_string());
        Self::connect_lazy(&url)
    }

    /// Create a lazy connection to the specified URL.
    ///
    /// The channel is established on first use, so this always succeeds
    /// even if the service is not running.
    pub fn connect_lazy(url: &str) -> Result<Self> {
        let endpoint = Channel::from_shared(url.to_string())
            .map_err(|e| anyhow!("Invalid gRPC URL '{}': {}", url, e))?;
        let channel = endpoint.connect_lazy();
        Ok(Self {
            client: BpmnLiteClient::new(channel),
            url: url.to_string(),
        })
    }

    /// Create an eager connection (waits for the channel to be ready).
    pub async fn connect(url: &str) -> Result<Self> {
        let channel = Channel::from_shared(url.to_string())
            .map_err(|e| anyhow!("Invalid gRPC URL '{}': {}", url, e))?
            .connect()
            .await
            .context("Failed to connect to BPMN-Lite service")?;
        Ok(Self {
            client: BpmnLiteClient::new(channel),
            url: url.to_string(),
        })
    }

    /// The URL this connection points to.
    pub fn url(&self) -> &str {
        &self.url
    }

    // -----------------------------------------------------------------------
    // Model lifecycle
    // -----------------------------------------------------------------------

    /// Compile BPMN XML to bytecode.
    pub async fn compile(&self, bpmn_xml: &str) -> Result<CompileResult> {
        let mut client = self.client.clone();
        let resp = client
            .compile(proto::CompileRequest {
                bpmn_xml: bpmn_xml.to_string(),
                validate_only: false,
            })
            .await
            .context("Compile RPC failed")?
            .into_inner();

        Ok(CompileResult {
            bytecode_version: resp.bytecode_version,
            diagnostics: resp
                .diagnostics
                .into_iter()
                .map(|d| CompileDiagnostic {
                    severity: d.severity,
                    message: d.message,
                    element_id: d.element_id,
                })
                .collect(),
        })
    }

    // -----------------------------------------------------------------------
    // Process lifecycle
    // -----------------------------------------------------------------------

    /// Start a new process instance. Returns the process instance ID.
    pub async fn start_process(&self, req: StartProcessRequest) -> Result<Uuid> {
        let mut client = self.client.clone();
        let resp = client
            .start_process(proto::StartRequest {
                process_key: req.process_key,
                bytecode_version: req.bytecode_version,
                domain_payload: req.domain_payload,
                domain_payload_hash: req.domain_payload_hash,
                orch_flags: to_proto_flags(&req.orch_flags),
                correlation_id: req.correlation_id.to_string(),
            })
            .await
            .context("StartProcess RPC failed")?
            .into_inner();

        Uuid::parse_str(&resp.process_instance_id)
            .context("Invalid process_instance_id from BPMN-Lite")
    }

    /// Send a signal (message) to a process instance.
    pub async fn signal(
        &self,
        instance_id: Uuid,
        message_name: &str,
        payload: Option<&[u8]>,
    ) -> Result<()> {
        let mut client = self.client.clone();
        client
            .signal(proto::SignalRequest {
                process_instance_id: instance_id.to_string(),
                message_name: message_name.to_string(),
                correlation_key: None,
                payload: payload.unwrap_or_default().to_vec(),
                msg_id: Uuid::new_v4().to_string(),
            })
            .await
            .context("Signal RPC failed")?;
        Ok(())
    }

    /// Cancel a process instance.
    pub async fn cancel(&self, instance_id: Uuid, reason: &str) -> Result<()> {
        let mut client = self.client.clone();
        client
            .cancel(proto::CancelRequest {
                process_instance_id: instance_id.to_string(),
                reason: reason.to_string(),
            })
            .await
            .context("Cancel RPC failed")?;
        Ok(())
    }

    /// Inspect a running process instance.
    pub async fn inspect(&self, instance_id: Uuid) -> Result<ProcessInspection> {
        let mut client = self.client.clone();
        let resp = client
            .inspect(proto::InspectRequest {
                process_instance_id: instance_id.to_string(),
            })
            .await
            .context("Inspect RPC failed")?
            .into_inner();

        Ok(ProcessInspection {
            state: resp.state,
            fibers: resp
                .fibers
                .into_iter()
                .map(|f| FiberSnapshot {
                    fiber_id: f.fiber_id,
                    pc: f.pc,
                    wait_state: f.wait_state,
                })
                .collect(),
            waits: resp
                .waits
                .into_iter()
                .map(|w| WaitSnapshot {
                    fiber_id: w.fiber_id,
                    wait_type: w.wait_type,
                    detail: w.detail,
                })
                .collect(),
            bytecode_version: resp.bytecode_version,
            domain_payload_hash: resp.domain_payload_hash,
        })
    }

    // -----------------------------------------------------------------------
    // Job worker protocol
    // -----------------------------------------------------------------------

    /// Activate jobs (streaming). Returns a vector of activated jobs.
    ///
    /// The BPMN-Lite server streams jobs as they become available.
    /// This method collects all streamed jobs into a vector.
    pub async fn activate_jobs(
        &self,
        task_types: &[String],
        max_jobs: i32,
        timeout_ms: i64,
        worker_id: &str,
    ) -> Result<Vec<JobActivation>> {
        let mut client = self.client.clone();
        let resp = client
            .activate_jobs(proto::ActivateJobsRequest {
                task_types: task_types.to_vec(),
                max_jobs,
                timeout_ms,
                worker_id: worker_id.to_string(),
            })
            .await
            .context("ActivateJobs RPC failed")?;

        let mut stream = resp.into_inner();
        let mut jobs = Vec::new();

        while let Some(msg) = stream.message().await.context("Job stream error")? {
            jobs.push(JobActivation {
                job_key: msg.job_key,
                process_instance_id: msg.process_instance_id,
                task_type: msg.task_type,
                service_task_id: msg.service_task_id,
                domain_payload: msg.domain_payload,
                domain_payload_hash: msg.domain_payload_hash,
                orch_flags: from_proto_flags(msg.orch_flags),
                retries_remaining: msg.retries_remaining,
            });
        }

        Ok(jobs)
    }

    /// Complete a job successfully.
    pub async fn complete_job(&self, req: CompleteJobRequest) -> Result<()> {
        let mut client = self.client.clone();
        client
            .complete_job(proto::CompleteJobRequest {
                job_key: req.job_key,
                domain_payload: req.domain_payload,
                domain_payload_hash: req.domain_payload_hash,
                orch_flags: to_proto_flags(&req.orch_flags),
            })
            .await
            .context("CompleteJob RPC failed")?;
        Ok(())
    }

    /// Fail a job with an error.
    pub async fn fail_job(
        &self,
        job_key: &str,
        error_class: &str,
        message: &str,
        retry_hint_ms: i64,
    ) -> Result<()> {
        let mut client = self.client.clone();
        client
            .fail_job(proto::FailJobRequest {
                job_key: job_key.to_string(),
                error_class: error_class.to_string(),
                message: message.to_string(),
                retry_hint_ms,
            })
            .await
            .context("FailJob RPC failed")?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Event stream
    // -----------------------------------------------------------------------

    /// Subscribe to lifecycle events for a process instance.
    ///
    /// Returns a tonic streaming receiver. The caller should loop over
    /// `stream.message().await` to receive events.
    pub async fn subscribe_events(
        &self,
        instance_id: Uuid,
    ) -> Result<tonic::Streaming<proto::LifecycleEvent>> {
        let mut client = self.client.clone();
        let resp = client
            .subscribe_events(proto::SubscribeRequest {
                process_instance_id: instance_id.to_string(),
            })
            .await
            .context("SubscribeEvents RPC failed")?;
        Ok(resp.into_inner())
    }
}

// ---------------------------------------------------------------------------
// Proto ↔ Domain conversion helpers
// ---------------------------------------------------------------------------

fn to_proto_flags(flags: &HashMap<String, OrchestratorFlag>) -> HashMap<String, proto::ProtoValue> {
    flags
        .iter()
        .map(|(k, v)| {
            let pv = match v {
                OrchestratorFlag::Bool(b) => proto::ProtoValue {
                    kind: Some(proto::proto_value::Kind::BoolValue(*b)),
                },
                OrchestratorFlag::Int(i) => proto::ProtoValue {
                    kind: Some(proto::proto_value::Kind::I64Value(*i)),
                },
                OrchestratorFlag::Str(s) => proto::ProtoValue {
                    kind: Some(proto::proto_value::Kind::StrValue(s.clone())),
                },
            };
            (k.clone(), pv)
        })
        .collect()
}

fn from_proto_flags(
    flags: HashMap<String, proto::ProtoValue>,
) -> HashMap<String, OrchestratorFlag> {
    flags
        .into_iter()
        .filter_map(|(k, v)| {
            v.kind.map(|kind| {
                let flag = match kind {
                    proto::proto_value::Kind::BoolValue(b) => OrchestratorFlag::Bool(b),
                    proto::proto_value::Kind::I64Value(i) => OrchestratorFlag::Int(i),
                    proto::proto_value::Kind::StrValue(s) => OrchestratorFlag::Str(s),
                };
                (k, flag)
            })
        })
        .collect()
}

/// Convert a domain lifecycle event from proto.
pub(crate) fn lifecycle_event_from_proto(event: proto::LifecycleEvent) -> BpmnLifecycleEvent {
    BpmnLifecycleEvent {
        sequence: event.sequence,
        event_type: event.event_type,
        process_instance_id: event.process_instance_id,
        payload_json: event.payload_json,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connect_lazy_succeeds_without_service() {
        let conn = BpmnLiteConnection::connect_lazy("http://[::1]:50052");
        assert!(conn.is_ok());
        assert_eq!(conn.unwrap().url(), "http://[::1]:50052");
    }

    #[test]
    fn test_connect_lazy_rejects_invalid_url() {
        // Invalid URLs fail before needing a runtime
        let conn = BpmnLiteConnection::connect_lazy("");
        assert!(conn.is_err());
    }

    #[test]
    fn test_orch_flag_roundtrip() {
        let mut flags = HashMap::new();
        flags.insert("active".to_string(), OrchestratorFlag::Bool(true));
        flags.insert("retries".to_string(), OrchestratorFlag::Int(3));
        flags.insert(
            "mode".to_string(),
            OrchestratorFlag::Str("fast".to_string()),
        );

        let proto_flags = to_proto_flags(&flags);
        let roundtripped = from_proto_flags(proto_flags);

        assert_eq!(roundtripped.len(), 3);
        match &roundtripped["active"] {
            OrchestratorFlag::Bool(b) => assert!(b),
            _ => panic!("Expected Bool"),
        }
        match &roundtripped["retries"] {
            OrchestratorFlag::Int(i) => assert_eq!(*i, 3),
            _ => panic!("Expected Int"),
        }
        match &roundtripped["mode"] {
            OrchestratorFlag::Str(s) => assert_eq!(s, "fast"),
            _ => panic!("Expected Str"),
        }
    }

    #[tokio::test]
    async fn test_from_env_uses_default() {
        // Clear env var to ensure default is used
        std::env::remove_var("BPMN_LITE_GRPC_URL");
        let conn = BpmnLiteConnection::from_env();
        assert!(conn.is_ok());
        assert_eq!(conn.unwrap().url(), DEFAULT_GRPC_URL);
    }
}
