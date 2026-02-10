use std::collections::BTreeMap;
use std::sync::Arc;

use tonic::{Request, Response, Status};
use uuid::Uuid;

use bpmn_lite_core::engine::BpmnLiteEngine;
use bpmn_lite_core::types::{ErrorClass, Value};

#[allow(clippy::enum_variant_names)]
pub mod proto {
    tonic::include_proto!("bpmn_lite.v1");
}

use proto::bpmn_lite_server::BpmnLite;
use proto::*;

pub struct BpmnLiteService {
    pub engine: Arc<BpmnLiteEngine>,
}

// --- Proto ↔ Core conversions ---

fn value_to_proto(v: &Value) -> ProtoValue {
    match v {
        Value::Bool(b) => ProtoValue {
            kind: Some(proto_value::Kind::BoolValue(*b)),
        },
        Value::I64(n) => ProtoValue {
            kind: Some(proto_value::Kind::I64Value(*n)),
        },
        Value::Str(idx) => ProtoValue {
            kind: Some(proto_value::Kind::StrValue(format!("str_{}", idx))),
        },
        Value::Ref(idx) => ProtoValue {
            kind: Some(proto_value::Kind::StrValue(format!("ref_{}", idx))),
        },
    }
}

fn proto_to_value(pv: &ProtoValue) -> Value {
    match &pv.kind {
        Some(proto_value::Kind::BoolValue(b)) => Value::Bool(*b),
        Some(proto_value::Kind::I64Value(n)) => Value::I64(*n),
        Some(proto_value::Kind::StrValue(_)) => Value::Str(0),
        None => Value::Bool(false),
    }
}

fn proto_to_orch_flags(
    map: &std::collections::HashMap<String, ProtoValue>,
) -> BTreeMap<String, Value> {
    map.iter()
        .map(|(k, v)| (k.clone(), proto_to_value(v)))
        .collect()
}

#[allow(clippy::result_large_err)]
fn parse_uuid(s: &str) -> Result<Uuid, Status> {
    Uuid::parse_str(s).map_err(|e| Status::invalid_argument(format!("Invalid UUID: {}", e)))
}

#[allow(clippy::result_large_err)]
fn parse_bytecode_version(bytes: &[u8]) -> Result<[u8; 32], Status> {
    bytes
        .try_into()
        .map_err(|_| Status::invalid_argument("bytecode_version must be exactly 32 bytes"))
}

#[allow(clippy::result_large_err)]
fn parse_hash(bytes: &[u8]) -> Result<[u8; 32], Status> {
    bytes
        .try_into()
        .map_err(|_| Status::invalid_argument("domain_payload_hash must be exactly 32 bytes"))
}

fn engine_err(e: anyhow::Error) -> Status {
    Status::internal(format!("{:#}", e))
}

/// Extract the instance_id (UUID) from a job_key formatted as "instance_id:service_task_id:pc".
#[allow(clippy::result_large_err)]
fn extract_instance_id_from_job_key(job_key: &str) -> Result<Uuid, Status> {
    let uuid_str = job_key
        .split(':')
        .next()
        .ok_or_else(|| Status::invalid_argument(format!("Invalid job_key: {}", job_key)))?;
    parse_uuid(uuid_str)
}

#[tonic::async_trait]
impl BpmnLite for BpmnLiteService {
    async fn compile(
        &self,
        request: Request<CompileRequest>,
    ) -> Result<Response<CompileResponse>, Status> {
        let req = request.into_inner();
        let result = self
            .engine
            .compile(&req.bpmn_xml)
            .await
            .map_err(|e| Status::invalid_argument(format!("Compilation failed: {:#}", e)))?;

        Ok(Response::new(CompileResponse {
            bytecode_version: result.bytecode_version.to_vec(),
            diagnostics: result
                .diagnostics
                .into_iter()
                .map(|msg| Diagnostic {
                    severity: "info".to_string(),
                    message: msg,
                    element_id: String::new(),
                })
                .collect(),
        }))
    }

    async fn start_process(
        &self,
        request: Request<StartRequest>,
    ) -> Result<Response<StartResponse>, Status> {
        let req = request.into_inner();
        let bytecode_version = parse_bytecode_version(&req.bytecode_version)?;
        let hash = parse_hash(&req.domain_payload_hash)?;

        let instance_id = self
            .engine
            .start(
                &req.process_key,
                bytecode_version,
                &req.domain_payload,
                hash,
                &req.correlation_id,
            )
            .await
            .map_err(engine_err)?;

        // Tick the instance to kick off any initial work (jobs stay in queue for ActivateJobs)
        self.engine
            .tick_instance(instance_id)
            .await
            .map_err(engine_err)?;

        Ok(Response::new(StartResponse {
            process_instance_id: instance_id.to_string(),
        }))
    }

    async fn signal(
        &self,
        request: Request<SignalRequest>,
    ) -> Result<Response<SignalResponse>, Status> {
        let req = request.into_inner();
        let instance_id = parse_uuid(&req.process_instance_id)?;

        let payload = if req.payload.is_empty() {
            None
        } else {
            Some(std::str::from_utf8(&req.payload).map_err(|e| {
                Status::invalid_argument(format!("payload must be valid UTF-8: {}", e))
            })?)
        };

        let hash = if req.payload.is_empty() {
            None
        } else {
            Some(bpmn_lite_core::vm::compute_hash(
                payload.unwrap_or_default(),
            ))
        };

        self.engine
            .signal(
                instance_id,
                &req.message_name,
                "",
                payload,
                hash,
                if req.msg_id.is_empty() {
                    None
                } else {
                    Some(req.msg_id.as_str())
                },
            )
            .await
            .map_err(engine_err)?;

        // Tick instance to advance past the signal (jobs stay in queue)
        self.engine
            .tick_instance(instance_id)
            .await
            .map_err(engine_err)?;

        Ok(Response::new(SignalResponse {}))
    }

    async fn cancel(
        &self,
        request: Request<CancelRequest>,
    ) -> Result<Response<CancelResponse>, Status> {
        let req = request.into_inner();
        let instance_id = parse_uuid(&req.process_instance_id)?;

        self.engine
            .cancel(instance_id, &req.reason)
            .await
            .map_err(engine_err)?;

        Ok(Response::new(CancelResponse {}))
    }

    async fn inspect(
        &self,
        request: Request<InspectRequest>,
    ) -> Result<Response<InspectResponse>, Status> {
        let req = request.into_inner();
        let instance_id = parse_uuid(&req.process_instance_id)?;

        let inspection = self.engine.inspect(instance_id).await.map_err(engine_err)?;

        let state_str = match &inspection.state {
            bpmn_lite_core::types::ProcessState::Running => "RUNNING",
            bpmn_lite_core::types::ProcessState::Completed { .. } => "COMPLETED",
            bpmn_lite_core::types::ProcessState::Cancelled { .. } => "CANCELLED",
            bpmn_lite_core::types::ProcessState::Failed { .. } => "FAILED",
            bpmn_lite_core::types::ProcessState::Terminated { .. } => "TERMINATED",
        };

        let fibers: Vec<FiberInfo> = inspection
            .fibers
            .iter()
            .map(|f| {
                let ws = format!("{:?}", f.wait_state);
                FiberInfo {
                    fiber_id: f.fiber_id.to_string(),
                    pc: f.pc,
                    wait_state: ws,
                }
            })
            .collect();

        let waits: Vec<WaitInfo> = inspection
            .fibers
            .iter()
            .filter(|f| !matches!(f.wait_state, bpmn_lite_core::types::WaitState::Running))
            .map(|f| {
                let (wt, detail) = match &f.wait_state {
                    bpmn_lite_core::types::WaitState::Timer { .. } => {
                        ("TIMER".to_string(), String::new())
                    }
                    bpmn_lite_core::types::WaitState::Msg { .. } => {
                        ("MESSAGE".to_string(), String::new())
                    }
                    bpmn_lite_core::types::WaitState::Job { job_key } => {
                        ("JOB".to_string(), job_key.clone())
                    }
                    bpmn_lite_core::types::WaitState::Join { .. } => {
                        ("JOIN".to_string(), String::new())
                    }
                    bpmn_lite_core::types::WaitState::Incident { incident_id } => {
                        ("INCIDENT".to_string(), incident_id.to_string())
                    }
                    bpmn_lite_core::types::WaitState::Race { race_id, .. } => {
                        ("RACE".to_string(), format!("race_{}", race_id))
                    }
                    bpmn_lite_core::types::WaitState::Running => unreachable!(),
                };
                WaitInfo {
                    fiber_id: f.fiber_id.to_string(),
                    wait_type: wt,
                    detail,
                }
            })
            .collect();

        Ok(Response::new(InspectResponse {
            state: state_str.to_string(),
            fibers,
            waits,
            bytecode_version: inspection
                .fibers
                .first()
                .map(|_| Vec::new())
                .unwrap_or_default(), // Not stored in FiberInspection, return empty
            domain_payload_hash: String::new(),
        }))
    }

    type ActivateJobsStream =
        tokio_stream::wrappers::ReceiverStream<Result<JobActivationMsg, Status>>;

    async fn activate_jobs(
        &self,
        request: Request<ActivateJobsRequest>,
    ) -> Result<Response<Self::ActivateJobsStream>, Status> {
        let req = request.into_inner();
        let max_jobs = req.max_jobs.max(1) as usize;

        let jobs = self
            .engine
            .activate_jobs(&req.task_types, max_jobs)
            .await
            .map_err(engine_err)?;

        let (tx, rx) = tokio::sync::mpsc::channel(jobs.len().max(1));

        for job in jobs {
            let msg = JobActivationMsg {
                job_key: job.job_key,
                process_instance_id: job.process_instance_id.to_string(),
                task_type: job.task_type,
                service_task_id: job.service_task_id,
                domain_payload: job.domain_payload,
                domain_payload_hash: job.domain_payload_hash.to_vec(),
                orch_flags: job
                    .orch_flags
                    .iter()
                    .map(|(k, v)| (k.clone(), value_to_proto(v)))
                    .collect(),
                retries_remaining: job.retries_remaining as i32,
            };
            let _ = tx.send(Ok(msg)).await;
        }

        Ok(Response::new(tokio_stream::wrappers::ReceiverStream::new(
            rx,
        )))
    }

    async fn complete_job(
        &self,
        request: Request<CompleteJobRequest>,
    ) -> Result<Response<CompleteJobResponse>, Status> {
        let req = request.into_inner();
        let hash = parse_hash(&req.domain_payload_hash)?;
        let orch_flags = proto_to_orch_flags(&req.orch_flags);

        // Extract instance_id from job_key before completing
        let instance_id = extract_instance_id_from_job_key(&req.job_key)?;

        self.engine
            .complete_job(&req.job_key, &req.domain_payload, hash, orch_flags)
            .await
            .map_err(engine_err)?;

        // Tick the instance so the resumed fiber advances (may hit End or next ExecNative)
        self.engine
            .tick_instance(instance_id)
            .await
            .map_err(engine_err)?;

        Ok(Response::new(CompleteJobResponse {}))
    }

    async fn fail_job(
        &self,
        request: Request<FailJobRequest>,
    ) -> Result<Response<FailJobResponse>, Status> {
        let req = request.into_inner();

        let error_class = match req.error_class.as_str() {
            "TRANSIENT" => ErrorClass::Transient,
            "CONTRACT_VIOLATION" => ErrorClass::ContractViolation,
            _ => ErrorClass::BusinessRejection {
                rejection_code: req.error_class.clone(),
            },
        };

        self.engine
            .fail_job(&req.job_key, error_class, &req.message)
            .await
            .map_err(engine_err)?;

        Ok(Response::new(FailJobResponse {}))
    }

    type SubscribeEventsStream =
        tokio_stream::wrappers::ReceiverStream<Result<LifecycleEvent, Status>>;

    async fn subscribe_events(
        &self,
        request: Request<SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeEventsStream>, Status> {
        use bpmn_lite_core::events::RuntimeEvent;

        let req = request.into_inner();
        let instance_id = parse_uuid(&req.process_instance_id)?;

        // Verify the instance exists before starting the tail.
        self.engine
            .read_events(instance_id, 0)
            .await
            .map_err(engine_err)?;

        let (tx, rx) = tokio::sync::mpsc::channel(64);
        let engine = self.engine.clone();

        // Spawn a background task that tails the event log.
        // Polls for new events every 100ms and stops when a terminal event
        // (Completed, Cancelled, IncidentCreated) is delivered.
        tokio::spawn(async move {
            let mut next_seq: u64 = 0;

            loop {
                let events = match engine.read_events(instance_id, next_seq).await {
                    Ok(e) => e,
                    Err(_) => break,
                };

                let mut terminal = false;

                for (seq, event) in &events {
                    // Check for terminal events before formatting.
                    if matches!(
                        event,
                        RuntimeEvent::Completed { .. }
                            | RuntimeEvent::Cancelled { .. }
                            | RuntimeEvent::Terminated { .. }
                            | RuntimeEvent::IncidentCreated { .. }
                    ) {
                        terminal = true;
                    }

                    let event_type = format!("{:?}", event);
                    let event_type = event_type
                        .split_once('{')
                        .or_else(|| event_type.split_once(' '))
                        .map(|(name, _)| name.trim().to_string())
                        .unwrap_or(event_type);

                    let payload_json = serde_json::to_string(&event).unwrap_or_default();

                    let msg = LifecycleEvent {
                        sequence: *seq,
                        event_type,
                        process_instance_id: instance_id.to_string(),
                        payload_json,
                    };

                    if tx.send(Ok(msg)).await.is_err() {
                        // Receiver dropped — stop tailing.
                        return;
                    }

                    // Advance past the last delivered sequence.
                    next_seq = seq + 1;
                }

                if terminal {
                    break;
                }

                // No new events yet — poll again after a short delay.
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        });

        Ok(Response::new(tokio_stream::wrappers::ReceiverStream::new(
            rx,
        )))
    }
}
