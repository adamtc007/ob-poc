// authoring dep removed in Phase 2.7 — see lib.rs
use bpmn_lite_compiler::{lowering, parser, verifier};
use bpmn_lite_types::events::RuntimeEvent;
use bpmn_lite_store::store::ProcessStore;
use bpmn_lite_types::*;
use bpmn_lite_vm::{apply_completion, compute_hash, TickOutcome, Vm};
use anyhow::{anyhow, Result};
use ob_poc_types::session_stack::SessionStackState;
use std::collections::BTreeMap;
use std::sync::Arc;
#[allow(unused_imports)]
use uuid::Uuid;

const MAX_BPMN_XML_BYTES: usize = 2_000_000;
const MAX_IR_NODES: usize = 2_048;
const MAX_IR_EDGES: usize = 4_096;
const MAX_BYTECODE_INSTRUCTIONS: usize = 10_000;
const MAX_TASK_MANIFEST: usize = 512;

/// BpmnLiteEngine is the top-level facade that wires together the compiler,
/// VM, and store. gRPC handlers delegate to this.
pub struct BpmnLiteEngine {
    store: Arc<dyn ProcessStore>,
    tenant_id: String,
}

/// Parameters for starting a new process instance.
pub struct StartParams {
    pub process_key: String,
    pub bytecode_version: [u8; 32],
    pub domain_payload: String,
    pub domain_payload_hash: [u8; 32],
    pub correlation_id: String,
    pub session_stack: SessionStackState,
    pub entry_id: Uuid,
    pub runbook_id: Uuid,
}

/// Result of compiling BPMN XML.
#[derive(Debug, Clone)]
pub struct CompileResult {
    pub bytecode_version: [u8; 32],
    pub task_types: Vec<String>,
    pub diagnostics: Vec<String>,
}

/// Snapshot of a process instance for the Inspect RPC.
#[derive(Debug, Clone)]
pub struct ProcessInspection {
    pub instance_id: Uuid,
    pub tenant_id: String,
    pub process_key: String,
    pub bytecode_version: [u8; 32],
    pub domain_payload_hash: [u8; 32],
    pub state: ProcessState,
    pub fibers: Vec<FiberInspection>,
    pub incidents: Vec<Incident>,
}

#[derive(Debug, Clone)]
pub struct FiberInspection {
    pub fiber_id: Uuid,
    pub pc: Addr,
    pub wait_state: WaitState,
    pub stack_depth: usize,
}

fn enforce_ir_limits(ir: &bpmn_lite_compiler::ir::IRGraph) -> Result<()> {
    if ir.node_count() > MAX_IR_NODES {
        return Err(anyhow!(
            "BPMN graph has too many nodes: {} > {}",
            ir.node_count(),
            MAX_IR_NODES
        ));
    }
    if ir.edge_count() > MAX_IR_EDGES {
        return Err(anyhow!(
            "BPMN graph has too many sequence flows: {} > {}",
            ir.edge_count(),
            MAX_IR_EDGES
        ));
    }
    Ok(())
}

fn enforce_program_limits(program: &CompiledProgram) -> Result<()> {
    if program.program.len() > MAX_BYTECODE_INSTRUCTIONS {
        return Err(anyhow!(
            "compiled bytecode has too many instructions: {} > {}",
            program.program.len(),
            MAX_BYTECODE_INSTRUCTIONS
        ));
    }
    if program.task_manifest.len() > MAX_TASK_MANIFEST {
        return Err(anyhow!(
            "compiled task manifest has too many task types: {} > {}",
            program.task_manifest.len(),
            MAX_TASK_MANIFEST
        ));
    }
    Ok(())
}

impl BpmnLiteEngine {
    pub fn new(store: Arc<dyn ProcessStore>) -> Self {
        Self::new_with_tenant(store, "default")
    }

    pub fn new_with_tenant(store: Arc<dyn ProcessStore>, tenant_id: impl Into<String>) -> Self {
        Self {
            store,
            tenant_id: tenant_id.into(),
        }
    }

    /// Compile BPMN XML → verified IR → bytecode, store the program.
    pub async fn compile(&self, bpmn_xml: &str) -> Result<CompileResult> {
        if bpmn_xml.len() > MAX_BPMN_XML_BYTES {
            return Err(anyhow!(
                "BPMN XML exceeds max size: {} bytes > {}",
                bpmn_xml.len(),
                MAX_BPMN_XML_BYTES
            ));
        }
        let ir = parser::parse_bpmn(bpmn_xml)?;
        enforce_ir_limits(&ir)?;
        let errors = verifier::verify(&ir);
        if !errors.is_empty() {
            let msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
            return Err(anyhow!("Verification failed:\n{}", msgs.join("\n")));
        }
        let program = lowering::lower(&ir)?;
        enforce_program_limits(&program)?;

        // Bytecode verification (bounded loop enforcement)
        let bytecode_errors = verifier::verify_bytecode(&program);
        if !bytecode_errors.is_empty() {
            let msgs: Vec<String> = bytecode_errors.iter().map(|e| e.message.clone()).collect();
            return Err(anyhow!(
                "Bytecode verification failed:\n{}",
                msgs.join("\n")
            ));
        }

        let bytecode_version = program.bytecode_version;
        let task_types = program.task_manifest.clone();

        self.store.store_program(bytecode_version, &program).await?;

        Ok(CompileResult {
            bytecode_version,
            task_types,
            diagnostics: vec![],
        })
    }

    /// Persist an already-compiled program through this engine's
    /// `ProcessStore` and surface the same `CompileResult` shape the
    /// retired `compile_from_dto` / `compile_from_yaml` produced.
    /// Used by the `bpmn-lite-authoring` free-function pipeline
    /// (`authoring::compile_from_dto` / `compile_from_yaml`) after
    /// it has lowered IR → bytecode.
    ///
    /// Phase 2.7 (2026-05-14) edge inversion landed here: the
    /// engine no longer parses YAML or DTOs. It accepts an
    /// already-lowered `CompiledProgram`, applies the same engine-
    /// level limit checks, and persists. Callers that need
    /// authoring-side compilation pull in `bpmn-lite-authoring`
    /// directly.
    pub async fn store_compiled_program(
        &self,
        program: CompiledProgram,
    ) -> Result<CompileResult> {
        enforce_program_limits(&program)?;

        let bytecode_errors = verifier::verify_bytecode(&program);
        if !bytecode_errors.is_empty() {
            let msgs: Vec<String> = bytecode_errors.iter().map(|e| e.message.clone()).collect();
            return Err(anyhow!(
                "Bytecode verification failed:\n{}",
                msgs.join("\n")
            ));
        }

        let bytecode_version = program.bytecode_version;
        let task_types = program.task_manifest.clone();

        self.store.store_program(bytecode_version, &program).await?;

        Ok(CompileResult {
            bytecode_version,
            task_types,
            diagnostics: vec![],
        })
    }

    /// Start a new process instance.
    pub async fn start(
        &self,
        process_key: &str,
        bytecode_version: [u8; 32],
        domain_payload: &str,
        domain_payload_hash: [u8; 32],
        correlation_id: &str,
    ) -> Result<Uuid> {
        self.start_with_params(StartParams {
            process_key: process_key.to_string(),
            bytecode_version,
            domain_payload: domain_payload.to_string(),
            domain_payload_hash,
            correlation_id: correlation_id.to_string(),
            session_stack: SessionStackState::default(),
            entry_id: Uuid::nil(),
            runbook_id: Uuid::nil(),
        })
        .await
    }

    /// Start a new process instance with full parameters including session stack.
    pub async fn start_with_params(&self, params: StartParams) -> Result<Uuid> {
        let _program = self
            .store
            .load_program(params.bytecode_version)
            .await?
            .ok_or_else(|| anyhow!("No program found for bytecode version"))?;

        let instance_id = Uuid::now_v7();
        let instance = ProcessInstance {
            instance_id,
            tenant_id: self.tenant_id.clone(),
            process_key: params.process_key,
            bytecode_version: params.bytecode_version,
            domain_payload: params.domain_payload.into(),
            domain_payload_hash: params.domain_payload_hash,
            session_stack: params.session_stack,
            flags: BTreeMap::new(),
            counters: BTreeMap::new(),
            join_expected: BTreeMap::new(),
            state: ProcessState::Running,
            correlation_id: params.correlation_id,
            entry_id: params.entry_id,
            runbook_id: params.runbook_id,
            created_at: now_ms(),
        };
        let fiber_id = Uuid::now_v7();
        let root_fiber = Fiber::new(fiber_id, 0);

        self.store
            .atomic_start(
                &instance,
                &root_fiber,
                &RuntimeEvent::InstanceStarted {
                    instance_id,
                    bytecode_version: params.bytecode_version,
                },
            )
            .await?;

        Ok(instance_id)
    }

    /// Tick all running instances. Returns count ticked.
    pub async fn tick_all(&self) -> Result<u32> {
        let ids = self.store.list_running_instances(&self.tenant_id).await?;
        self.tick_instance_ids(ids).await
    }

    /// Claim and tick a bounded batch of running instances for scheduler loops.
    pub async fn tick_claimed_batch(
        &self,
        owner: &str,
        limit: usize,
        lease_ms: u64,
    ) -> Result<u32> {
        let ids = self
            .store
            .claim_running_instances(&self.tenant_id, owner, limit, lease_ms)
            .await?;
        self.tick_instance_ids(ids).await
    }

    async fn tick_instance_ids(&self, ids: Vec<Uuid>) -> Result<u32> {
        let mut ticked = 0u32;
        for id in ids {
            if let Err(e) = self.tick_instance(id).await {
                tracing::warn!(instance_id = %id, error = %e, "tick_instance_ids: instance tick failed");
            }
            ticked += 1;
        }
        Ok(ticked)
    }

    /// Advance all runnable fibers for a specific instance.
    /// Jobs are left in the queue — use `activate_jobs()` to dequeue them.
    pub async fn tick_instance(&self, instance_id: Uuid) -> Result<()> {
        let mut instance = self
            .store
            .load_instance(instance_id)
            .await?
            .ok_or_else(|| anyhow!("Instance not found: {}", instance_id))?;

        let program = self
            .store
            .load_program(instance.bytecode_version)
            .await?
            .ok_or_else(|| anyhow!("Program not found for instance {}", instance_id))?;

        let fibers = self.store.load_fibers(instance_id).await?;

        for fiber in fibers {
            if fiber.wait != WaitState::Running {
                continue;
            }

            let mut fiber = fiber;
            let vm = Vm::new(self.store.clone());
            let outcome = vm
                .run_fiber(&mut fiber, &mut instance, &program, 1000)
                .await?;

            // Save updated instance (flags + counters)
            self.store.save_instance(&instance).await?;

            match outcome {
                TickOutcome::Parked(WaitState::Job { .. }) => {
                    // Job enqueued by VM — leave in queue for activate_jobs()
                }
                TickOutcome::Ended => {
                    // Fiber ended — check if all fibers are done
                    let remaining = self.store.load_fibers(instance_id).await?;
                    if remaining.is_empty() {
                        self.store
                            .update_instance_state(
                                instance_id,
                                ProcessState::Completed { at: now_ms() },
                            )
                            .await?;
                        self.store
                            .append_event(instance_id, &RuntimeEvent::Completed { at: now_ms() })
                            .await?;
                    }
                }
                TickOutcome::Terminated => {
                    // EndTerminate: kill the entire instance immediately.
                    let terminating_fiber_id = fiber.fiber_id;

                    // 1. Emit WaitCancelled for all OTHER fibers with active waits
                    let all_fibers = self.store.load_fibers(instance_id).await?;
                    for other in &all_fibers {
                        if other.fiber_id == terminating_fiber_id {
                            continue;
                        }
                        let wait_desc = describe_wait(&other.wait);
                        if !wait_desc.is_empty() {
                            self.store
                                .append_event(
                                    instance_id,
                                    &RuntimeEvent::WaitCancelled {
                                        fiber_id: other.fiber_id,
                                        wait_desc,
                                        reason: "terminate_end_event".to_string(),
                                    },
                                )
                                .await?;
                        }
                    }

                    // 2. Purge pending/inflight jobs
                    self.store.cancel_jobs_for_instance(instance_id).await?;

                    // 3. Delete ALL fibers (including the terminating one)
                    self.store.delete_all_fibers(instance_id).await?;

                    // 4. Set state → Terminated
                    let at = now_ms();
                    self.store
                        .update_instance_state(instance_id, ProcessState::Terminated { at })
                        .await?;

                    // 5. Emit Terminated event
                    self.store
                        .append_event(
                            instance_id,
                            &RuntimeEvent::Terminated {
                                at,
                                fiber_id: terminating_fiber_id,
                            },
                        )
                        .await?;

                    // 6. BREAK — no more fibers to process
                    break;
                }
                _ => {
                    // Parked on timer/msg/join/incident, or still running
                }
            }
        }

        // --- Boundary timer promotion pass: Job → Race ---
        let fibers_for_promotion = self.store.load_fibers(instance_id).await?;
        let now_promo = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        for fiber in &fibers_for_promotion {
            if let WaitState::Job { ref job_key } = fiber.wait {
                if let Some(&race_id) = program.boundary_map.get(&fiber.pc) {
                    if let Some(race_entry) = program.race_plan.get(&race_id) {
                        // Compute absolute deadline from timer arm
                        let timer_deadline_ms = race_entry.arms.iter().find_map(|arm| match arm {
                            WaitArm::Timer { duration_ms, .. } => Some(now_promo + duration_ms),
                            WaitArm::Deadline { deadline_ms, .. } => Some(*deadline_ms),
                            _ => None,
                        });

                        // Compute timer_arm_index for non-interrupting logic
                        let timer_arm_index = race_entry.arms.iter().position(|arm| {
                            matches!(arm, WaitArm::Timer { .. } | WaitArm::Deadline { .. })
                        });

                        // Read interrupting flag from timer arm (default true for safety)
                        let interrupting = race_entry
                            .arms
                            .iter()
                            .find_map(|arm| match arm {
                                WaitArm::Timer { interrupting, .. } => Some(*interrupting),
                                _ => None,
                            })
                            .unwrap_or(true);

                        // Read cycle spec from timer arm
                        let cycle_remaining = race_entry.arms.iter().find_map(|arm| match arm {
                            WaitArm::Timer { cycle: Some(c), .. } => Some(c.max_fires),
                            _ => None,
                        });

                        // Promote: Job → Race, PRESERVING the exact job_key
                        let mut promoted = fiber.clone();
                        promoted.wait = WaitState::Race {
                            race_id,
                            timer_deadline_ms,
                            job_key: Some(job_key.clone()),
                            interrupting,
                            timer_arm_index,
                            cycle_remaining,
                            cycle_fired_count: 0,
                        };
                        self.store.save_fiber(instance_id, &promoted).await?;

                        // Emit RaceRegistered event
                        let arm_descs: Vec<bpmn_lite_types::events::WaitArmDesc> =
                            race_entry.arms.iter().map(|a| a.into()).collect();
                        self.store
                            .append_event(
                                instance_id,
                                &RuntimeEvent::RaceRegistered {
                                    race_id,
                                    fiber_id: fiber.fiber_id,
                                    arms: arm_descs,
                                },
                            )
                            .await?;
                    }
                }
            }
        }

        // --- Race timer check pass ---
        let fibers_after = self.store.load_fibers(instance_id).await?;
        let now_race = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        for fiber in fibers_after {
            if let WaitState::Race {
                race_id,
                timer_deadline_ms,
                ref job_key,
                interrupting,
                timer_arm_index,
                cycle_remaining,
                cycle_fired_count,
            } = &fiber.wait
            {
                let race_id = *race_id;
                let stored_job_key = job_key.clone();
                let is_interrupting = *interrupting;
                let stored_timer_arm_index = *timer_arm_index;
                let stored_cycle_remaining = *cycle_remaining;
                let stored_cycle_fired_count = *cycle_fired_count;

                // Check explicit timer_deadline_ms first (boundary timer promotion path)
                if let Some(deadline) = timer_deadline_ms {
                    if now_race >= *deadline {
                        if let Some(race_entry) = program.race_plan.get(&race_id) {
                            // Use stored timer_arm_index if available, else compute
                            let timer_arm_idx = stored_timer_arm_index.or_else(|| {
                                race_entry.arms.iter().position(|arm| {
                                    matches!(arm, WaitArm::Timer { .. } | WaitArm::Deadline { .. })
                                })
                            });

                            if let Some(idx) = timer_arm_idx {
                                if is_interrupting {
                                    // ── INTERRUPTING: resolve race (existing behavior) ──
                                    let mut fiber = fiber.clone();
                                    let mut instance =
                                        self.store.load_instance(instance_id).await?.ok_or_else(
                                            || anyhow!("Instance not found: {}", instance_id),
                                        )?;
                                    let vm = Vm::new(self.store.clone());
                                    vm.resolve_race(
                                        &mut instance,
                                        &mut fiber,
                                        race_id,
                                        idx,
                                        &race_entry.arms,
                                    )
                                    .await?;

                                    // Ack the pending job
                                    if let Some(jk) = &stored_job_key {
                                        let _ = self.store.ack_job(jk).await;
                                    }
                                } else {
                                    // ── NON-INTERRUPTING: fork child fiber, main stays in Race ──
                                    let resume_at = match &race_entry.arms[idx] {
                                        WaitArm::Timer { resume_at, .. } => *resume_at,
                                        WaitArm::Deadline { resume_at, .. } => *resume_at,
                                        _ => continue,
                                    };

                                    // Spawn child fiber at escalation path
                                    let child_fiber_id = Uuid::now_v7();
                                    let child_fiber = Fiber::new(child_fiber_id, resume_at);
                                    self.store.save_fiber(instance_id, &child_fiber).await?;

                                    // Emit BoundaryFired event
                                    let boundary_element_id =
                                        race_entry.boundary_element_id.clone().unwrap_or_default();
                                    self.store
                                        .append_event(
                                            instance_id,
                                            &RuntimeEvent::BoundaryFired {
                                                race_id,
                                                fiber_id: fiber.fiber_id,
                                                spawned_fiber_id: child_fiber_id,
                                                boundary_element_id,
                                                resume_at,
                                            },
                                        )
                                        .await?;

                                    let new_fired_count = stored_cycle_fired_count + 1;

                                    // Emit TimerCycleIteration
                                    let new_remaining =
                                        stored_cycle_remaining.map(|r| r.saturating_sub(1));
                                    self.store
                                        .append_event(
                                            instance_id,
                                            &RuntimeEvent::TimerCycleIteration {
                                                race_id,
                                                fiber_id: fiber.fiber_id,
                                                iteration: new_fired_count,
                                                remaining: new_remaining.unwrap_or(0),
                                            },
                                        )
                                        .await?;

                                    // Check if cycle exhausted
                                    let cycle_exhausted = new_remaining == Some(0);

                                    if cycle_exhausted {
                                        // All cycles consumed — emit exhausted, resolve race
                                        self.store
                                            .append_event(
                                                instance_id,
                                                &RuntimeEvent::TimerCycleExhausted {
                                                    race_id,
                                                    fiber_id: fiber.fiber_id,
                                                    total_fired: new_fired_count,
                                                },
                                            )
                                            .await?;

                                        // Remove timer from race — fiber goes back to plain Job wait
                                        let mut updated_fiber = fiber.clone();
                                        if let Some(jk) = &stored_job_key {
                                            updated_fiber.wait = WaitState::Job {
                                                job_key: jk.clone(),
                                            };
                                        }
                                        self.store.save_fiber(instance_id, &updated_fiber).await?;
                                    } else {
                                        // Re-register timer with new deadline + updated counts
                                        let new_deadline = match &race_entry.arms[idx] {
                                            WaitArm::Timer { duration_ms, .. } => {
                                                now_race + duration_ms
                                            }
                                            _ => now_race + 60_000, // fallback 1min
                                        };

                                        let mut updated_fiber = fiber.clone();
                                        updated_fiber.wait = WaitState::Race {
                                            race_id,
                                            timer_deadline_ms: Some(new_deadline),
                                            job_key: stored_job_key.clone(),
                                            interrupting: false,
                                            timer_arm_index: stored_timer_arm_index,
                                            cycle_remaining: new_remaining,
                                            cycle_fired_count: new_fired_count,
                                        };
                                        self.store.save_fiber(instance_id, &updated_fiber).await?;
                                    }
                                }
                            }
                        }
                        continue;
                    }
                }

                // Fallback: check Deadline arms in race_plan (WaitAny opcode path)
                if let Some(race_entry) = program.race_plan.get(&race_id) {
                    for (i, arm) in race_entry.arms.iter().enumerate() {
                        let expired = match arm {
                            WaitArm::Deadline { deadline_ms, .. } => now_race >= *deadline_ms,
                            _ => false,
                        };

                        if expired {
                            let mut fiber = fiber.clone();
                            let mut instance = self
                                .store
                                .load_instance(instance_id)
                                .await?
                                .ok_or_else(|| anyhow!("Instance not found: {}", instance_id))?;
                            let vm = Vm::new(self.store.clone());
                            vm.resolve_race(
                                &mut instance,
                                &mut fiber,
                                race_id,
                                i,
                                &race_entry.arms,
                            )
                            .await?;
                            break; // Only one winner per race
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Advance all runnable fibers and return any job activations.
    /// Convenience method for in-process use — dequeues jobs immediately.
    pub async fn run_instance(&self, instance_id: Uuid) -> Result<Vec<JobActivation>> {
        self.tick_instance(instance_id).await?;

        let instance = self
            .store
            .load_instance(instance_id)
            .await?
            .ok_or_else(|| anyhow!("Instance not found: {}", instance_id))?;

        let program = self
            .store
            .load_program(instance.bytecode_version)
            .await?
            .ok_or_else(|| anyhow!("Program not found for instance {}", instance_id))?;

        let jobs = self
            .store
            .dequeue_jobs(&program.task_manifest, 100, &self.tenant_id)
            .await?;
        Ok(jobs)
    }

    /// Emit a SignalIgnored audit event for late/ghost completions.
    async fn emit_late(&self, instance_id: Uuid, desc: String) -> Result<()> {
        self.store
            .append_event(
                instance_id,
                &RuntimeEvent::SignalIgnored { signal_desc: desc },
            )
            .await?;
        Ok(())
    }

    /// Complete a job — resume the parked fiber.
    ///
    /// Three guards protect against ghost signals:
    ///   1. Dedupe — already-processed job_key is silently accepted
    ///   2. State  — Cancelled/Completed instance → SignalIgnored event
    ///   3. Hash   — worker-supplied expected/current hash must match instance snapshot
    pub async fn complete_job(
        &self,
        job_key: &str,
        domain_payload: &str,
        expected_instance_payload_hash: [u8; 32],
        orch_flags: BTreeMap<String, Value>,
    ) -> Result<()> {
        let (instance_id, _task_type_id, pc) = parse_job_key(job_key)?;

        // ── Guard 1: dedupe ──
        if self.store.dedupe_get(job_key).await?.is_some() {
            return Ok(());
        }

        // ── Guard 2: instance state ──
        let mut instance = self
            .store
            .load_instance(instance_id)
            .await?
            .ok_or_else(|| anyhow!("Instance not found: {}", instance_id))?;

        if instance.state.is_terminal() {
            self.emit_late(
                instance_id,
                format!("complete_job on {:?} instance: {}", instance.state, job_key),
            )
            .await?;
            return Ok(());
        }

        // ── Guard 3: payload hash ──
        let expected = compute_hash(&instance.domain_payload);
        if expected_instance_payload_hash != expected {
            return Err(anyhow!(
                "Payload hash mismatch on complete_job: job_key={}",
                job_key
            ));
        }

        let program = self
            .store
            .load_program(instance.bytecode_version)
            .await?
            .ok_or_else(|| anyhow!("Program not found"))?;

        let completion = JobCompletion {
            job_key: job_key.to_string(),
            domain_payload: domain_payload.to_string(),
            expected_instance_payload_hash,
            orch_flags,
        };

        let vm = Vm::new(self.store.clone());

        // Check if this job's ExecNative has a boundary timer (race)
        let resumed = if let Some(&race_id) = program.boundary_map.get(&pc) {
            // Race-aware completion: find fiber in Race state
            let fibers = self.store.load_fibers(instance_id).await?;
            let race_fiber = fibers.iter().find(
                |f| matches!(&f.wait, WaitState::Race { race_id: rid, .. } if *rid == race_id),
            );

            if let Some(parked) = race_fiber {
                let mut fiber = parked.clone();
                if let Some(race_entry) = program.race_plan.get(&race_id) {
                    let internal_idx = race_entry
                        .arms
                        .iter()
                        .position(|arm| matches!(arm, WaitArm::Internal { .. }))
                        .unwrap_or(0);

                    vm.resolve_race(
                        &mut instance,
                        &mut fiber,
                        race_id,
                        internal_idx,
                        &race_entry.arms,
                    )
                    .await?;
                }
                self.store.ack_job(job_key).await?;
                true // race path always means fiber was resumed
            } else {
                // Fiber still in Job state (promotion hasn't happened)
                // or race already resolved (late arrival) — try vm path
                let maybe_fid = vm
                    .complete_job(&mut instance, &completion, &program)
                    .await?;
                maybe_fid.is_some()
            }
        } else {
            // No boundary timer — standard complete_job path
            let maybe_fid = vm
                .complete_job(&mut instance, &completion, &program)
                .await?;
            maybe_fid.is_some()
        };

        if resumed {
            // Mutation ownership: engine applies completion + persists atomically
            apply_completion(&mut instance, &completion);
            self.store.atomic_complete(&instance, &completion).await?;
        } else {
            // No fiber matched — ghost signal
            self.emit_late(
                instance_id,
                format!("complete_job: no matching fiber for job_key={}", job_key),
            )
            .await?;
        }

        Ok(())
    }

    /// Fail a job — route to error boundary handler or create an incident.
    ///
    /// Only `BusinessRejection` errors trigger error routing. `Transient` and
    /// `ContractViolation` always create incidents.
    pub async fn fail_job(
        &self,
        job_key: &str,
        error_class: ErrorClass,
        message: &str,
    ) -> Result<()> {
        let (instance_id, _task_type_id, _pc) = parse_job_key(job_key)?;

        let instance = self
            .store
            .load_instance(instance_id)
            .await?
            .ok_or_else(|| anyhow!("Instance not found: {}", instance_id))?;

        // Guard 1: terminal state
        if instance.state.is_terminal() {
            self.emit_late(
                instance_id,
                format!("fail_job(key={}, state={:?})", job_key, instance.state),
            )
            .await?;
            self.store.ack_job(job_key).await?;
            return Ok(());
        }

        let program = self
            .store
            .load_program(instance.bytecode_version)
            .await?
            .ok_or_else(|| anyhow!("Program not found"))?;

        // Find parked fiber
        let fibers = self.store.load_fibers(instance_id).await?;
        let parked = fibers
            .iter()
            .find(|f| matches!(&f.wait, WaitState::Job { job_key: jk } if jk == job_key));

        let Some(parked_fiber) = parked else {
            // Guard 2: no matching fiber (ghost)
            self.emit_late(instance_id, format!("fail_job(key={}, no fiber)", job_key))
                .await?;
            self.store.ack_job(job_key).await?;
            return Ok(());
        };

        let mut fiber = parked_fiber.clone();

        // Check error routing (only for BusinessRejection)
        if let ErrorClass::BusinessRejection { rejection_code } = &error_class {
            if let Some(routes) = program.error_route_map.get(&fiber.pc) {
                let matched = routes.iter().find(|r| match &r.error_code {
                    Some(code) => code == rejection_code,
                    None => true, // catch-all
                });

                if let Some(route) = matched {
                    // ERROR ROUTE PATH: advance fiber to escalation
                    fiber.pc = route.resume_at;
                    fiber.wait = WaitState::Running;
                    self.store.save_fiber(instance_id, &fiber).await?;
                    self.store.ack_job(job_key).await?;
                    self.store
                        .append_event(
                            instance_id,
                            &RuntimeEvent::ErrorRouted {
                                job_key: job_key.to_string(),
                                error_code: rejection_code.clone(),
                                boundary_id: route.boundary_element_id.clone(),
                                resume_at: route.resume_at,
                            },
                        )
                        .await?;
                    return Ok(());
                }
            }
        }

        // INCIDENT PATH: no route matched (or not BusinessRejection)
        let incident_id = Uuid::now_v7();
        let service_task_id = program
            .debug_map
            .get(&fiber.pc)
            .cloned()
            .unwrap_or_else(|| format!("pc_{}", fiber.pc));

        let incident = Incident {
            incident_id,
            process_instance_id: instance_id,
            fiber_id: fiber.fiber_id,
            service_task_id: service_task_id.clone(),
            bytecode_addr: fiber.pc,
            error_class,
            message: message.to_string(),
            retry_count: 0,
            created_at: now_ms(),
            resolved_at: None,
            resolution: None,
        };

        self.store.save_incident(&incident).await?;
        self.store
            .append_event(
                instance_id,
                &RuntimeEvent::IncidentCreated {
                    incident_id,
                    service_task_id,
                    job_key: Some(job_key.to_string()),
                },
            )
            .await?;

        fiber.wait = WaitState::Incident { incident_id };
        self.store.save_fiber(instance_id, &fiber).await?;

        let mut instance = instance;
        instance.state = ProcessState::Failed { incident_id };
        self.store.save_instance(&instance).await?;
        self.store.ack_job(job_key).await?;

        Ok(())
    }

    /// Signal a waiting fiber (message correlation).
    /// Handles both plain WaitMsg and Race (WaitAny with Msg arm).
    ///
    /// Guards: Cancelled/Completed instances get SignalIgnored.
    /// No-match (no fiber waiting for a message) also emits SignalIgnored.
    pub async fn signal(
        &self,
        instance_id: Uuid,
        _msg_name: &str,
        _corr_key: &str,
        domain_payload: Option<&str>,
        domain_payload_hash: Option<[u8; 32]>,
        _msg_id: Option<&str>,
    ) -> Result<()> {
        let mut instance = self
            .store
            .load_instance(instance_id)
            .await?
            .ok_or_else(|| anyhow!("Instance not found: {}", instance_id))?;

        // ── State guard ──
        if instance.state.is_terminal() {
            self.emit_late(
                instance_id,
                format!("signal on {:?} instance: msg={}", instance.state, _msg_name),
            )
            .await?;
            return Ok(());
        }

        let program = self
            .store
            .load_program(instance.bytecode_version)
            .await?
            .ok_or_else(|| anyhow!("Program not found"))?;

        let fibers = self.store.load_fibers(instance_id).await?;

        for fiber in fibers {
            match &fiber.wait {
                // Existing: plain WaitMsg
                WaitState::Msg { .. } => {
                    let mut fiber = fiber;
                    fiber.wait = WaitState::Running;
                    fiber.pc += 1;

                    if let (Some(payload), Some(hash)) = (domain_payload, domain_payload_hash) {
                        self.store
                            .update_instance_payload(instance_id, payload, &hash)
                            .await?;
                    }

                    self.store.save_fiber(instance_id, &fiber).await?;
                    self.store
                        .append_event(
                            instance_id,
                            &RuntimeEvent::MsgReceived {
                                name: 0,
                                corr_key: Value::Str(0),
                                msg_ref: None,
                            },
                        )
                        .await?;
                    return Ok(());
                }

                // Race — check if any Msg arm matches
                WaitState::Race { race_id, .. } => {
                    let race_id = *race_id;
                    if let Some(race_entry) = program.race_plan.get(&race_id) {
                        for (i, arm) in race_entry.arms.iter().enumerate() {
                            if let WaitArm::Msg { .. } = arm {
                                let mut fiber = fiber.clone();
                                let vm = Vm::new(self.store.clone());

                                if let (Some(payload), Some(hash)) =
                                    (domain_payload, domain_payload_hash)
                                {
                                    self.store
                                        .update_instance_payload(instance_id, payload, &hash)
                                        .await?;
                                }

                                vm.resolve_race(
                                    &mut instance,
                                    &mut fiber,
                                    race_id,
                                    i,
                                    &race_entry.arms,
                                )
                                .await?;
                                return Ok(());
                            }
                        }
                    }
                }

                _ => continue,
            }
        }

        // No fiber matched — ghost signal
        self.emit_late(
            instance_id,
            format!("signal: no waiting fiber for msg={}", _msg_name),
        )
        .await?;
        Ok(())
    }

    /// Cancel a process instance.
    ///
    /// Emits WaitCancelled per parked fiber, purges pending/inflight jobs,
    /// then deletes all fibers and marks instance Cancelled.
    pub async fn cancel(&self, instance_id: Uuid, reason: &str) -> Result<()> {
        // 1. Emit WaitCancelled per parked fiber (before deletion)
        let fibers = self.store.load_fibers(instance_id).await?;
        for fiber in &fibers {
            let wait_desc = describe_wait(&fiber.wait);
            if !wait_desc.is_empty() {
                self.store
                    .append_event(
                        instance_id,
                        &RuntimeEvent::WaitCancelled {
                            fiber_id: fiber.fiber_id,
                            wait_desc,
                            reason: reason.to_string(),
                        },
                    )
                    .await?;
            }
        }

        // 2. Purge pending + inflight jobs for this instance
        self.store.cancel_jobs_for_instance(instance_id).await?;

        // 3. Update state, delete fibers, emit Cancelled
        self.store
            .update_instance_state(
                instance_id,
                ProcessState::Cancelled {
                    reason: reason.to_string(),
                    at: now_ms(),
                },
            )
            .await?;
        self.store.delete_all_fibers(instance_id).await?;
        self.store
            .append_event(
                instance_id,
                &RuntimeEvent::Cancelled {
                    reason: reason.to_string(),
                },
            )
            .await?;
        Ok(())
    }

    /// Inspect a process instance.
    pub async fn inspect(&self, instance_id: Uuid) -> Result<ProcessInspection> {
        let instance = self
            .store
            .load_instance(instance_id)
            .await?
            .ok_or_else(|| anyhow!("Instance not found: {}", instance_id))?;

        let fibers = self.store.load_fibers(instance_id).await?;
        let fiber_inspections: Vec<FiberInspection> = fibers
            .iter()
            .map(|f| FiberInspection {
                fiber_id: f.fiber_id,
                pc: f.pc,
                wait_state: f.wait.clone(),
                stack_depth: f.stack.len(),
            })
            .collect();

        let incidents = self.store.load_incidents(instance_id).await?;

        Ok(ProcessInspection {
            instance_id,
            tenant_id: instance.tenant_id,
            process_key: instance.process_key,
            bytecode_version: instance.bytecode_version,
            domain_payload_hash: instance.domain_payload_hash,
            state: instance.state,
            fibers: fiber_inspections,
            incidents,
        })
    }

    /// Activate jobs — dequeue from the job queue.
    pub async fn activate_jobs(
        &self,
        task_types: &[String],
        max_jobs: usize,
    ) -> Result<Vec<JobActivation>> {
        self.store
            .dequeue_jobs(task_types, max_jobs, &self.tenant_id)
            .await
    }

    /// Read events from the event log.
    pub async fn read_events(
        &self,
        instance_id: Uuid,
        from_seq: u64,
    ) -> Result<Vec<(u64, RuntimeEvent)>> {
        self.store.read_events(instance_id, from_seq).await
    }

    pub async fn health_check(&self) -> Result<()> {
        self.store.health_check().await
    }
}

/// Parse a job_key in format "instance_id:service_task_id:pc:epoch"
/// where service_task_id is a string (BPMN element ID from debug_map).
/// The epoch is parsed but discarded — only instance_id, task_id, and pc are returned.
fn parse_job_key(job_key: &str) -> Result<(Uuid, String, u32)> {
    // Format: uuid:service_task_id:pc:epoch
    // Strategy: split from the RIGHT for epoch (last), then pc (second-to-last),
    // then from the LEFT for UUID (first 36 chars), remainder is service_task_id.
    let mut parts = job_key.rsplitn(2, ':');
    let _epoch_str = parts
        .next()
        .ok_or_else(|| anyhow!("Invalid job_key format: {}", job_key))?;
    let rest = parts
        .next()
        .ok_or_else(|| anyhow!("Invalid job_key format: {}", job_key))?;

    // rest is now "uuid:service_task_id:pc"
    let mut parts2 = rest.rsplitn(2, ':');
    let pc_str = parts2
        .next()
        .ok_or_else(|| anyhow!("Invalid job_key format: {}", job_key))?;
    let rest2 = parts2
        .next()
        .ok_or_else(|| anyhow!("Invalid job_key format: {}", job_key))?;

    // rest2 is now "uuid:service_task_id"
    let mut parts3 = rest2.splitn(2, ':');
    let uuid_str = parts3
        .next()
        .ok_or_else(|| anyhow!("Invalid job_key format: {}", job_key))?;
    let service_task_id = parts3
        .next()
        .ok_or_else(|| anyhow!("Invalid job_key format: {}", job_key))?
        .to_string();

    let instance_id =
        Uuid::parse_str(uuid_str).map_err(|e| anyhow!("Invalid UUID in job_key: {}", e))?;
    let pc: u32 = pc_str
        .parse()
        .map_err(|e| anyhow!("Invalid pc in job_key: {}", e))?;
    Ok((instance_id, service_task_id, pc))
}

/// Human-readable description of a fiber's wait state for audit events.
/// Returns empty string for Running (not actually waiting).
fn describe_wait(wait: &WaitState) -> String {
    match wait {
        WaitState::Running => String::new(),
        WaitState::Timer { .. } => "Timer".to_string(),
        WaitState::Msg { .. } => "Msg".to_string(),
        WaitState::Job { job_key } => format!("Job({})", job_key),
        WaitState::Join { .. } => "Join".to_string(),
        WaitState::Incident { .. } => "Incident".to_string(),
        WaitState::Race {
            race_id, job_key, ..
        } => {
            if let Some(jk) = job_key {
                format!("Race(id={},job={})", race_id, jk)
            } else {
                format!("Race(id={})", race_id)
            }
        }
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
