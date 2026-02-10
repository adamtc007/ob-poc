use crate::compiler::{lowering, parser, verifier};
use crate::events::RuntimeEvent;
use crate::store::ProcessStore;
use crate::types::*;
use crate::vm::{apply_completion, compute_hash, TickOutcome, Vm};
use anyhow::{anyhow, Result};
use std::collections::BTreeMap;
use std::sync::Arc;
#[allow(unused_imports)]
use uuid::Uuid;

/// BpmnLiteEngine is the top-level facade that wires together the compiler,
/// VM, and store. gRPC handlers delegate to this.
pub struct BpmnLiteEngine {
    store: Arc<dyn ProcessStore>,
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
    pub process_key: String,
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

impl BpmnLiteEngine {
    pub fn new(store: Arc<dyn ProcessStore>) -> Self {
        Self { store }
    }

    /// Compile BPMN XML → verified IR → bytecode, store the program.
    pub async fn compile(&self, bpmn_xml: &str) -> Result<CompileResult> {
        let ir = parser::parse_bpmn(bpmn_xml)?;
        let errors = verifier::verify(&ir);
        if !errors.is_empty() {
            let msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
            return Err(anyhow!("Verification failed:\n{}", msgs.join("\n")));
        }
        let program = lowering::lower(&ir)?;

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

    /// Start a new process instance.
    pub async fn start(
        &self,
        process_key: &str,
        bytecode_version: [u8; 32],
        domain_payload: &str,
        domain_payload_hash: [u8; 32],
        correlation_id: &str,
    ) -> Result<Uuid> {
        // Verify program exists
        let _program = self
            .store
            .load_program(bytecode_version)
            .await?
            .ok_or_else(|| anyhow!("No program found for bytecode version"))?;

        let instance_id = Uuid::now_v7();
        let instance = ProcessInstance {
            instance_id,
            process_key: process_key.to_string(),
            bytecode_version,
            domain_payload: domain_payload.to_string(),
            domain_payload_hash,
            flags: BTreeMap::new(),
            counters: BTreeMap::new(),
            join_expected: BTreeMap::new(),
            state: ProcessState::Running,
            correlation_id: correlation_id.to_string(),
            created_at: now_ms(),
        };
        self.store.save_instance(&instance).await?;

        // Create root fiber at pc=0
        let fiber_id = Uuid::now_v7();
        let root_fiber = Fiber::new(fiber_id, 0);
        self.store.save_fiber(instance_id, &root_fiber).await?;

        // Emit InstanceStarted event
        self.store
            .append_event(
                instance_id,
                &RuntimeEvent::InstanceStarted {
                    instance_id,
                    bytecode_version,
                },
            )
            .await?;

        Ok(instance_id)
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
                        let arm_descs: Vec<crate::events::WaitArmDesc> =
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

        let jobs = self.store.dequeue_jobs(&program.task_manifest, 100).await?;
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
    ///   3. Hash   — domain_payload_hash must match instance snapshot
    pub async fn complete_job(
        &self,
        job_key: &str,
        domain_payload: &str,
        domain_payload_hash: [u8; 32],
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
        if domain_payload_hash != expected {
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
            domain_payload_hash,
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
            // Mutation ownership: engine applies completion + persists
            apply_completion(&mut instance, &completion);
            self.store.save_instance(&instance).await?;
            self.store
                .dedupe_put(&completion.job_key, &completion)
                .await?;
            self.store
                .save_payload_version(
                    instance_id,
                    &completion.domain_payload_hash,
                    &completion.domain_payload,
                )
                .await?;
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
            process_key: instance.process_key,
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
        self.store.dequeue_jobs(task_types, max_jobs).await
    }

    /// Read events from the event log.
    pub async fn read_events(
        &self,
        instance_id: Uuid,
        from_seq: u64,
    ) -> Result<Vec<(u64, RuntimeEvent)>> {
        self.store.read_events(instance_id, from_seq).await
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store_memory::MemoryStore;
    use crate::vm::compute_hash;

    /// Integration test: compile → start → run → activate jobs → complete → verify completion
    #[tokio::test]
    async fn test_engine_full_lifecycle() {
        let store: Arc<dyn ProcessStore> = Arc::new(MemoryStore::new());
        let engine = BpmnLiteEngine::new(store.clone());

        // 1. Compile a minimal BPMN
        let bpmn = r#"<?xml version="1.0" encoding="UTF-8"?>
        <bpmn:definitions xmlns:bpmn="http://www.omg.org/spec/BPMN/20100524/MODEL"
                          xmlns:zeebe="http://camunda.org/schema/zeebe/1.0">
          <bpmn:process id="test_proc" isExecutable="true">
            <bpmn:startEvent id="start" />
            <bpmn:serviceTask id="task1" name="Do Work">
              <bpmn:extensionElements>
                <zeebe:taskDefinition type="do_work" />
              </bpmn:extensionElements>
            </bpmn:serviceTask>
            <bpmn:endEvent id="end" />
            <bpmn:sequenceFlow id="f1" sourceRef="start" targetRef="task1" />
            <bpmn:sequenceFlow id="f2" sourceRef="task1" targetRef="end" />
          </bpmn:process>
        </bpmn:definitions>"#;

        let compile_result = engine.compile(bpmn).await.unwrap();
        assert!(!compile_result.task_types.is_empty());

        // 2. Start a process
        let payload = r#"{"case":"test"}"#;
        let hash = compute_hash(payload);
        let instance_id = engine
            .start(
                "test_proc",
                compile_result.bytecode_version,
                payload,
                hash,
                "corr-1",
            )
            .await
            .unwrap();

        // 3. Run the instance — should enqueue a job and park
        let activations = engine.run_instance(instance_id).await.unwrap();

        // 4. Inspect — should be Running with a fiber parked on Job
        let inspection = engine.inspect(instance_id).await.unwrap();
        assert_eq!(inspection.state, ProcessState::Running);

        // 5. Activate jobs — may have been dequeued in run_instance already
        let extra_jobs = engine
            .activate_jobs(&["do_work".to_string()], 10)
            .await
            .unwrap();
        let all_jobs: Vec<_> = activations.into_iter().chain(extra_jobs).collect();
        assert!(
            !all_jobs.is_empty(),
            "Should have at least one job activation"
        );

        let job = &all_jobs[0];
        let job_key = job.job_key.clone();

        // 6. Complete the job
        // domain_payload_hash must match the INSTANCE's current payload hash
        let result_payload = r#"{"result":"done"}"#;
        engine
            .complete_job(&job_key, result_payload, hash, BTreeMap::new())
            .await
            .unwrap();

        // 7. Run instance again to advance past the completed job
        engine.run_instance(instance_id).await.unwrap();

        // 8. Inspect — should be Completed
        let final_inspection = engine.inspect(instance_id).await.unwrap();
        assert!(
            matches!(final_inspection.state, ProcessState::Completed { .. }),
            "Expected Completed, got {:?}",
            final_inspection.state
        );

        // 9. Verify events
        let events = engine.read_events(instance_id, 0).await.unwrap();
        assert!(events.len() >= 2); // At least InstanceStarted + Completed
    }

    // ── Shared BPMN fixture for T-CANCEL tests ──

    const SINGLE_TASK_BPMN: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
    <bpmn:definitions xmlns:bpmn="http://www.omg.org/spec/BPMN/20100524/MODEL"
                      xmlns:zeebe="http://camunda.org/schema/zeebe/1.0">
      <bpmn:process id="cancel_proc" isExecutable="true">
        <bpmn:startEvent id="start" />
        <bpmn:serviceTask id="task1" name="Work">
          <bpmn:extensionElements>
            <zeebe:taskDefinition type="do_work" />
          </bpmn:extensionElements>
        </bpmn:serviceTask>
        <bpmn:endEvent id="end" />
        <bpmn:sequenceFlow id="f1" sourceRef="start" targetRef="task1" />
        <bpmn:sequenceFlow id="f2" sourceRef="task1" targetRef="end" />
      </bpmn:process>
    </bpmn:definitions>"#;

    /// Helper: compile + start + run until job is parked, return (engine, store, instance_id, job_key, hash).
    async fn setup_parked_job() -> (
        BpmnLiteEngine,
        Arc<dyn ProcessStore>,
        Uuid,
        String,
        [u8; 32],
    ) {
        let store: Arc<dyn ProcessStore> = Arc::new(MemoryStore::new());
        let engine = BpmnLiteEngine::new(store.clone());

        let cr = engine.compile(SINGLE_TASK_BPMN).await.unwrap();
        let payload = r#"{"case":"cancel-test"}"#;
        let hash = compute_hash(payload);
        let iid = engine
            .start(
                "cancel_proc",
                cr.bytecode_version,
                payload,
                hash,
                "corr-cancel",
            )
            .await
            .unwrap();

        let activations = engine.run_instance(iid).await.unwrap();
        let extra = engine
            .activate_jobs(&["do_work".to_string()], 10)
            .await
            .unwrap();
        let all: Vec<_> = activations.into_iter().chain(extra).collect();
        assert!(!all.is_empty(), "Expected at least one job activation");
        let job_key = all[0].job_key.clone();

        (engine, store, iid, job_key, hash)
    }

    // ── T-CANCEL-1: complete_job on cancelled instance → Ok + SignalIgnored ──

    #[tokio::test]
    async fn t_cancel_complete_after_cancel() {
        let (engine, store, iid, job_key, hash) = setup_parked_job().await;

        // Cancel the instance while job is parked
        engine.cancel(iid, "user-requested").await.unwrap();

        // Attempt complete_job on cancelled instance — should succeed (no error)
        let result = engine
            .complete_job(&job_key, r#"{"late":"true"}"#, hash, BTreeMap::new())
            .await;
        assert!(
            result.is_ok(),
            "complete_job on cancelled instance should not error"
        );

        // Verify SignalIgnored event was emitted
        let events = store.read_events(iid, 0).await.unwrap();
        let has_signal_ignored = events.iter().any(|(_, e)| {
            matches!(e, RuntimeEvent::SignalIgnored { signal_desc } if signal_desc.contains("Cancelled"))
        });
        assert!(
            has_signal_ignored,
            "Expected SignalIgnored event, got: {:?}",
            events.iter().map(|(_, e)| e).collect::<Vec<_>>()
        );

        // Verify instance is still Cancelled (no state corruption)
        let inspection = engine.inspect(iid).await.unwrap();
        assert!(matches!(inspection.state, ProcessState::Cancelled { .. }));
    }

    // ── T-CANCEL-2: duplicate complete_job → Ok (dedupe, no double mutation) ──

    #[tokio::test]
    async fn t_cancel_duplicate_complete() {
        let (engine, store, iid, job_key, hash) = setup_parked_job().await;

        // First complete — should succeed normally
        engine
            .complete_job(&job_key, r#"{"r":"first"}"#, hash, BTreeMap::new())
            .await
            .unwrap();

        // Count events after first complete
        let events_after_first = store.read_events(iid, 0).await.unwrap().len();

        // Second complete with same job_key — should be silently accepted (dedupe)
        let result = engine
            .complete_job(&job_key, r#"{"r":"second"}"#, hash, BTreeMap::new())
            .await;
        assert!(result.is_ok(), "Duplicate complete_job should not error");

        // No new events should be emitted (dedupe short-circuits)
        let events_after_second = store.read_events(iid, 0).await.unwrap().len();
        assert_eq!(
            events_after_first, events_after_second,
            "Dedupe should not emit additional events"
        );
    }

    // ── T-CANCEL-3: cancel purges job queue + emits WaitCancelled ──

    #[tokio::test]
    async fn t_cancel_purges_jobs() {
        let (engine, store, iid, _job_key, _hash) = setup_parked_job().await;

        // Verify fiber is parked on Job before cancel
        let inspection = engine.inspect(iid).await.unwrap();
        assert_eq!(inspection.fibers.len(), 1);
        assert!(matches!(
            inspection.fibers[0].wait_state,
            WaitState::Job { .. }
        ));

        // Cancel — should purge jobs and emit WaitCancelled
        engine.cancel(iid, "cleanup").await.unwrap();

        // Verify no fibers remain
        let post_cancel = engine.inspect(iid).await.unwrap();
        assert!(
            post_cancel.fibers.is_empty(),
            "All fibers should be deleted"
        );

        // Verify job queue is empty (no orphan jobs)
        let remaining_jobs = engine
            .activate_jobs(&["do_work".to_string()], 10)
            .await
            .unwrap();
        assert!(
            remaining_jobs.is_empty(),
            "Job queue should be purged after cancel"
        );

        // Verify WaitCancelled event was emitted
        let events = store.read_events(iid, 0).await.unwrap();
        let has_wait_cancelled = events.iter().any(
            |(_, e)| matches!(e, RuntimeEvent::WaitCancelled { reason, .. } if reason == "cleanup"),
        );
        assert!(
            has_wait_cancelled,
            "Expected WaitCancelled event, got: {:?}",
            events.iter().map(|(_, e)| e).collect::<Vec<_>>()
        );

        // Verify Cancelled event also emitted
        let has_cancelled = events
            .iter()
            .any(|(_, e)| matches!(e, RuntimeEvent::Cancelled { reason } if reason == "cleanup"));
        assert!(has_cancelled, "Expected Cancelled event");
    }

    // ── T-CANCEL-4: signal on completed instance → Ok + SignalIgnored ──

    #[tokio::test]
    async fn t_cancel_signal_after_complete() {
        let (engine, store, iid, job_key, hash) = setup_parked_job().await;

        // Complete the job and advance to End
        engine
            .complete_job(&job_key, r#"{"done":true}"#, hash, BTreeMap::new())
            .await
            .unwrap();
        engine.run_instance(iid).await.unwrap();

        // Verify instance is Completed
        let inspection = engine.inspect(iid).await.unwrap();
        assert!(
            matches!(inspection.state, ProcessState::Completed { .. }),
            "Expected Completed, got {:?}",
            inspection.state
        );

        // Signal on completed instance — should succeed (no error)
        let result = engine
            .signal(iid, "late_msg", "corr-1", None, None, None)
            .await;
        assert!(
            result.is_ok(),
            "signal on completed instance should not error"
        );

        // Verify SignalIgnored event was emitted
        let events = store.read_events(iid, 0).await.unwrap();
        let has_signal_ignored = events.iter().any(|(_, e)| {
            matches!(e, RuntimeEvent::SignalIgnored { signal_desc } if signal_desc.contains("Completed"))
        });
        assert!(
            has_signal_ignored,
            "Expected SignalIgnored event for completed instance"
        );
    }

    // ── T-CANCEL-5: signal on running instance with no Msg fiber → Ok + SignalIgnored ──

    #[tokio::test]
    async fn t_cancel_signal_no_match() {
        let (engine, store, iid, _job_key, _hash) = setup_parked_job().await;

        // Instance is Running with fiber parked on Job (not Msg)
        let inspection = engine.inspect(iid).await.unwrap();
        assert_eq!(inspection.state, ProcessState::Running);
        assert!(matches!(
            inspection.fibers[0].wait_state,
            WaitState::Job { .. }
        ));

        // Signal — no fiber is waiting for a message
        let result = engine
            .signal(iid, "ghost_msg", "corr-ghost", None, None, None)
            .await;
        assert!(
            result.is_ok(),
            "signal with no matching fiber should not error"
        );

        // Verify SignalIgnored event was emitted
        let events = store.read_events(iid, 0).await.unwrap();
        let has_signal_ignored = events.iter().any(|(_, e)| {
            matches!(e, RuntimeEvent::SignalIgnored { signal_desc } if signal_desc.contains("no waiting fiber"))
        });
        assert!(
            has_signal_ignored,
            "Expected SignalIgnored event for no-match signal, got: {:?}",
            events.iter().map(|(_, e)| e).collect::<Vec<_>>()
        );
    }

    // ═══════════════════════════════════════════════════════════
    //  Phase 2A: Non-Interrupting Boundary Timer Tests (T-NI)
    // ═══════════════════════════════════════════════════════════

    /// BPMN with non-interrupting boundary timer (cancelActivity="false").
    const NI_BOUNDARY_BPMN: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
    <bpmn:definitions xmlns:bpmn="http://www.omg.org/spec/BPMN/20100524/MODEL"
                      xmlns:zeebe="http://camunda.org/schema/zeebe/1.0">
      <bpmn:process id="ni_proc" isExecutable="true">
        <bpmn:startEvent id="start" />
        <bpmn:serviceTask id="long_task" name="Long Running Task">
          <bpmn:extensionElements>
            <zeebe:taskDefinition type="long_work" />
          </bpmn:extensionElements>
        </bpmn:serviceTask>
        <bpmn:boundaryEvent id="reminder" attachedToRef="long_task" cancelActivity="false">
          <bpmn:timerEventDefinition>
            <bpmn:timeDuration>PT1S</bpmn:timeDuration>
          </bpmn:timerEventDefinition>
        </bpmn:boundaryEvent>
        <bpmn:serviceTask id="send_reminder" name="Send Reminder">
          <bpmn:extensionElements>
            <zeebe:taskDefinition type="send_reminder" />
          </bpmn:extensionElements>
        </bpmn:serviceTask>
        <bpmn:endEvent id="end_normal" />
        <bpmn:endEvent id="end_reminder" />
        <bpmn:sequenceFlow id="f1" sourceRef="start" targetRef="long_task" />
        <bpmn:sequenceFlow id="f2" sourceRef="long_task" targetRef="end_normal" />
        <bpmn:sequenceFlow id="f3" sourceRef="reminder" targetRef="send_reminder" />
        <bpmn:sequenceFlow id="f4" sourceRef="send_reminder" targetRef="end_reminder" />
      </bpmn:process>
    </bpmn:definitions>"#;

    /// BPMN with non-interrupting cycle timer (R3/PT1S — fires 3 times).
    const NI_CYCLE_BPMN: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
    <bpmn:definitions xmlns:bpmn="http://www.omg.org/spec/BPMN/20100524/MODEL"
                      xmlns:zeebe="http://camunda.org/schema/zeebe/1.0">
      <bpmn:process id="ni_cycle_proc" isExecutable="true">
        <bpmn:startEvent id="start" />
        <bpmn:serviceTask id="long_task" name="Long Running Task">
          <bpmn:extensionElements>
            <zeebe:taskDefinition type="long_work" />
          </bpmn:extensionElements>
        </bpmn:serviceTask>
        <bpmn:boundaryEvent id="reminder" attachedToRef="long_task" cancelActivity="false">
          <bpmn:timerEventDefinition>
            <bpmn:timeCycle>R3/PT1S</bpmn:timeCycle>
          </bpmn:timerEventDefinition>
        </bpmn:boundaryEvent>
        <bpmn:serviceTask id="send_reminder" name="Send Reminder">
          <bpmn:extensionElements>
            <zeebe:taskDefinition type="send_reminder" />
          </bpmn:extensionElements>
        </bpmn:serviceTask>
        <bpmn:endEvent id="end_normal" />
        <bpmn:endEvent id="end_reminder" />
        <bpmn:sequenceFlow id="f1" sourceRef="start" targetRef="long_task" />
        <bpmn:sequenceFlow id="f2" sourceRef="long_task" targetRef="end_normal" />
        <bpmn:sequenceFlow id="f3" sourceRef="reminder" targetRef="send_reminder" />
        <bpmn:sequenceFlow id="f4" sourceRef="send_reminder" targetRef="end_reminder" />
      </bpmn:process>
    </bpmn:definitions>"#;

    /// Helper: compile + start + tick until fiber is promoted to Race, return components.
    /// Manipulates timer deadline to be in the past for immediate firing.
    async fn setup_ni_race(
        bpmn: &str,
    ) -> (
        BpmnLiteEngine,
        Arc<dyn ProcessStore>,
        Uuid,
        String,
        [u8; 32],
    ) {
        let store: Arc<dyn ProcessStore> = Arc::new(MemoryStore::new());
        let engine = BpmnLiteEngine::new(store.clone());

        let cr = engine.compile(bpmn).await.unwrap();
        let payload = r#"{"case":"ni-test"}"#;
        let hash = compute_hash(payload);
        let iid = engine
            .start("ni_proc", cr.bytecode_version, payload, hash, "corr-ni")
            .await
            .unwrap();

        // Tick to get fiber parked on Job, then promoted to Race
        engine.tick_instance(iid).await.unwrap();

        // Dequeue jobs so we have the job_key
        let jobs = engine
            .activate_jobs(&["long_work".to_string()], 10)
            .await
            .unwrap();
        assert!(!jobs.is_empty(), "Expected job activation");
        let job_key = jobs[0].job_key.clone();

        // Verify fiber is now in Race state
        let fibers = store.load_fibers(iid).await.unwrap();
        assert_eq!(fibers.len(), 1);
        assert!(
            matches!(&fibers[0].wait, WaitState::Race { .. }),
            "Expected Race, got {:?}",
            fibers[0].wait
        );

        // Manipulate deadline to be in the past so next tick fires the timer
        let mut fiber = fibers[0].clone();
        if let WaitState::Race {
            ref mut timer_deadline_ms,
            ..
        } = fiber.wait
        {
            *timer_deadline_ms = Some(0); // epoch = definitely in the past
        }
        store.save_fiber(iid, &fiber).await.unwrap();

        (engine, store, iid, job_key, hash)
    }

    // ── T-NI-1: Non-interrupting timer fires → spawns child, main stays in Race ──

    #[tokio::test]
    async fn t_ni_1_non_interrupting_spawns_child() {
        let (engine, store, iid, job_key, _hash) = setup_ni_race(NI_BOUNDARY_BPMN).await;

        // Tick — timer deadline is in the past, should fire non-interrupting
        engine.tick_instance(iid).await.unwrap();

        // Verify: should now have 2 fibers (main in Race/Job, child Running)
        let fibers = store.load_fibers(iid).await.unwrap();
        assert_eq!(
            fibers.len(),
            2,
            "Expected 2 fibers (main + spawned child), got {}",
            fibers.len()
        );

        // Main fiber should still reference the job (either Race or Job state)
        let main_fiber = fibers.iter().find(|f| {
            matches!(&f.wait, WaitState::Race { job_key: Some(jk), .. } if *jk == job_key)
                || matches!(&f.wait, WaitState::Job { job_key: jk } if *jk == job_key)
        });
        assert!(
            main_fiber.is_some(),
            "Main fiber should still have the job_key"
        );

        // Verify BoundaryFired event was emitted
        let events = store.read_events(iid, 0).await.unwrap();
        let has_boundary_fired = events
            .iter()
            .any(|(_, e)| matches!(e, RuntimeEvent::BoundaryFired { .. }));
        assert!(
            has_boundary_fired,
            "Expected BoundaryFired event, got: {:?}",
            events.iter().map(|(_, e)| e).collect::<Vec<_>>()
        );

        // Instance should still be Running (not Completed)
        let inspection = engine.inspect(iid).await.unwrap();
        assert_eq!(inspection.state, ProcessState::Running);
    }

    // ── T-NI-2: Cycle R3 fires 3 times, spawns 3 child fibers ──

    #[tokio::test]
    async fn t_ni_2_cycle_fires_multiple_times() {
        let (engine, store, iid, _job_key, _hash) = setup_ni_race(NI_CYCLE_BPMN).await;

        // Fire 3 iterations by ticking + resetting deadline each time
        for i in 0..3 {
            engine.tick_instance(iid).await.unwrap();

            let fibers = store.load_fibers(iid).await.unwrap();
            // After each fire: 1 main + (i+1) child fibers
            // But child fibers may have run to End and been removed
            // Just check that total is >= 1 (main still exists)
            assert!(
                !fibers.is_empty(),
                "Fibers should not be empty after iteration {}",
                i
            );

            // Reset deadline on the Race fiber for next iteration (if still in Race)
            for f in &fibers {
                if let WaitState::Race { .. } = &f.wait {
                    let mut updated = f.clone();
                    if let WaitState::Race {
                        ref mut timer_deadline_ms,
                        ..
                    } = updated.wait
                    {
                        *timer_deadline_ms = Some(0);
                    }
                    store.save_fiber(iid, &updated).await.unwrap();
                }
            }
        }

        // Verify 3 BoundaryFired events were emitted
        let events = store.read_events(iid, 0).await.unwrap();
        let boundary_fired_count = events
            .iter()
            .filter(|(_, e)| matches!(e, RuntimeEvent::BoundaryFired { .. }))
            .count();
        assert_eq!(
            boundary_fired_count, 3,
            "Expected 3 BoundaryFired events, got {}",
            boundary_fired_count
        );

        // Verify 3 TimerCycleIteration events
        let iteration_count = events
            .iter()
            .filter(|(_, e)| matches!(e, RuntimeEvent::TimerCycleIteration { .. }))
            .count();
        assert_eq!(
            iteration_count, 3,
            "Expected 3 TimerCycleIteration events, got {}",
            iteration_count
        );
    }

    // ── T-NI-3: Cycle exhausted → fiber reverts to plain Job wait ──

    #[tokio::test]
    async fn t_ni_3_cycle_exhausted_reverts_to_job() {
        let (engine, store, iid, job_key, _hash) = setup_ni_race(NI_CYCLE_BPMN).await;

        // Fire all 3 iterations
        for _ in 0..3 {
            engine.tick_instance(iid).await.unwrap();

            // Reset deadline for next tick
            let fibers = store.load_fibers(iid).await.unwrap();
            for f in &fibers {
                if let WaitState::Race { .. } = &f.wait {
                    let mut updated = f.clone();
                    if let WaitState::Race {
                        ref mut timer_deadline_ms,
                        ..
                    } = updated.wait
                    {
                        *timer_deadline_ms = Some(0);
                    }
                    store.save_fiber(iid, &updated).await.unwrap();
                }
            }
        }

        // After 3 fires, the main fiber should revert to Job state (cycle exhausted)
        let fibers = store.load_fibers(iid).await.unwrap();
        let main_has_job = fibers
            .iter()
            .any(|f| matches!(&f.wait, WaitState::Job { job_key: jk } if *jk == job_key));
        assert!(
            main_has_job,
            "After cycle exhaustion, main fiber should revert to Job wait. Got: {:?}",
            fibers.iter().map(|f| &f.wait).collect::<Vec<_>>()
        );

        // Verify TimerCycleExhausted event
        let events = store.read_events(iid, 0).await.unwrap();
        let has_exhausted = events
            .iter()
            .any(|(_, e)| matches!(e, RuntimeEvent::TimerCycleExhausted { total_fired: 3, .. }));
        assert!(
            has_exhausted,
            "Expected TimerCycleExhausted with total_fired=3"
        );
    }

    // ── T-NI-4: Job completes before non-interrupting timer → normal resolution ──

    #[tokio::test]
    async fn t_ni_4_job_completes_before_timer() {
        let store: Arc<dyn ProcessStore> = Arc::new(MemoryStore::new());
        let engine = BpmnLiteEngine::new(store.clone());

        let cr = engine.compile(NI_BOUNDARY_BPMN).await.unwrap();
        let payload = r#"{"case":"ni-job-first"}"#;
        let hash = compute_hash(payload);
        let iid = engine
            .start("ni_proc", cr.bytecode_version, payload, hash, "corr-ni4")
            .await
            .unwrap();

        // Tick to promote fiber to Race
        engine.tick_instance(iid).await.unwrap();

        let jobs = engine
            .activate_jobs(&["long_work".to_string()], 10)
            .await
            .unwrap();
        assert!(!jobs.is_empty());
        let job_key = jobs[0].job_key.clone();

        // Complete the job BEFORE the timer fires
        let result_payload = r#"{"result":"done"}"#;
        engine
            .complete_job(&job_key, result_payload, hash, BTreeMap::new())
            .await
            .unwrap();

        // Tick to advance past the completed job
        engine.tick_instance(iid).await.unwrap();

        // Run the child tasks if any were spawned
        let remaining_jobs = engine
            .activate_jobs(&["long_work".to_string(), "send_reminder".to_string()], 10)
            .await
            .unwrap();
        for job in &remaining_jobs {
            let _ = engine
                .complete_job(
                    &job.job_key,
                    r#"{"r":"done"}"#,
                    compute_hash(
                        &store
                            .load_instance(iid)
                            .await
                            .unwrap()
                            .unwrap()
                            .domain_payload,
                    ),
                    BTreeMap::new(),
                )
                .await;
        }

        // Keep ticking to reach completion
        for _ in 0..5 {
            engine.tick_instance(iid).await.unwrap();
        }

        // Instance should eventually complete (job resolved the race via Internal arm)
        let inspection = engine.inspect(iid).await.unwrap();
        assert!(
            matches!(inspection.state, ProcessState::Completed { .. }),
            "Expected Completed after job finishes, got {:?}",
            inspection.state
        );

        // No BoundaryFired events (timer never fired)
        let events = store.read_events(iid, 0).await.unwrap();
        let boundary_fired = events
            .iter()
            .any(|(_, e)| matches!(e, RuntimeEvent::BoundaryFired { .. }));
        assert!(
            !boundary_fired,
            "BoundaryFired should not have been emitted when job completes first"
        );
    }

    // ── T-NI-5: Verifier rejects cycle + interrupting=true ──

    #[tokio::test]
    async fn t_ni_5_verifier_rejects_cycle_interrupting() {
        let bpmn = r#"<?xml version="1.0" encoding="UTF-8"?>
        <bpmn:definitions xmlns:bpmn="http://www.omg.org/spec/BPMN/20100524/MODEL"
                          xmlns:zeebe="http://camunda.org/schema/zeebe/1.0">
          <bpmn:process id="bad_proc" isExecutable="true">
            <bpmn:startEvent id="start" />
            <bpmn:serviceTask id="task1" name="Work">
              <bpmn:extensionElements>
                <zeebe:taskDefinition type="do_work" />
              </bpmn:extensionElements>
            </bpmn:serviceTask>
            <bpmn:boundaryEvent id="bad_timer" attachedToRef="task1" cancelActivity="true">
              <bpmn:timerEventDefinition>
                <bpmn:timeCycle>R3/PT1H</bpmn:timeCycle>
              </bpmn:timerEventDefinition>
            </bpmn:boundaryEvent>
            <bpmn:endEvent id="end" />
            <bpmn:endEvent id="end2" />
            <bpmn:sequenceFlow id="f1" sourceRef="start" targetRef="task1" />
            <bpmn:sequenceFlow id="f2" sourceRef="task1" targetRef="end" />
            <bpmn:sequenceFlow id="f3" sourceRef="bad_timer" targetRef="end2" />
          </bpmn:process>
        </bpmn:definitions>"#;

        let store: Arc<dyn ProcessStore> = Arc::new(MemoryStore::new());
        let engine = BpmnLiteEngine::new(store);

        let result = engine.compile(bpmn).await;
        assert!(result.is_err(), "Should reject cycle + interrupting=true");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("cycle timers must be non-interrupting"),
            "Error should mention cycle + non-interrupting, got: {}",
            err_msg
        );
    }

    // ── Phase 5.1: Terminate End Event tests ────────────────────────

    /// T-TERM-1: Single fiber hits EndTerminate → instance Terminated.
    #[tokio::test]
    async fn t_term_1_single_fiber_terminate() {
        let store = Arc::new(MemoryStore::new());
        let engine = BpmnLiteEngine::new(store.clone());

        let program = CompiledProgram {
            bytecode_version: [40u8; 32],
            program: vec![
                Instr::ExecNative {
                    task_type: 0,
                    argc: 0,
                    retc: 0,
                },
                Instr::EndTerminate,
            ],
            debug_map: BTreeMap::from([(0, "task_a".to_string())]),
            join_plan: BTreeMap::new(),
            wait_plan: BTreeMap::new(),
            race_plan: BTreeMap::new(),
            boundary_map: BTreeMap::new(),
            write_set: BTreeMap::new(),
            task_manifest: vec!["task_a".to_string()],
            error_route_map: BTreeMap::new(),
        };
        store
            .store_program(program.bytecode_version, &program)
            .await
            .unwrap();

        let instance_id = engine
            .start("test", program.bytecode_version, "{}", [0u8; 32], "corr-1")
            .await
            .unwrap();
        let jobs = engine.run_instance(instance_id).await.unwrap();
        assert_eq!(jobs.len(), 1);

        let payload = "{}";
        let hash = compute_hash(payload);
        engine
            .complete_job(&jobs[0].job_key, payload, hash, BTreeMap::new())
            .await
            .unwrap();
        engine.tick_instance(instance_id).await.unwrap();

        // Assert: Terminated
        let instance = store.load_instance(instance_id).await.unwrap().unwrap();
        assert!(
            matches!(instance.state, ProcessState::Terminated { .. }),
            "Expected Terminated, got {:?}",
            instance.state
        );

        // Assert: no fibers remain
        let fibers = store.load_fibers(instance_id).await.unwrap();
        assert!(fibers.is_empty());

        // Assert: Terminated event
        let events = store.read_events(instance_id, 0).await.unwrap();
        let has_term = events
            .iter()
            .any(|(_, e)| matches!(e, RuntimeEvent::Terminated { .. }));
        assert!(has_term);
    }

    /// T-TERM-2: Parallel flow — one branch terminates, other branch killed.
    /// Order-independent: handles either fiber executing first after Fork.
    #[tokio::test]
    async fn t_term_2_parallel_terminate_kills_siblings() {
        let store = Arc::new(MemoryStore::new());
        let engine = BpmnLiteEngine::new(store.clone());

        // Fork → Branch A (EndTerminate), Branch B (ExecNative → End)
        let program = CompiledProgram {
            bytecode_version: [41u8; 32],
            program: vec![
                Instr::Fork {
                    targets: Box::new([1, 2]),
                }, // 0: fork
                Instr::EndTerminate, // 1: Branch A terminates
                Instr::ExecNative {
                    task_type: 0,
                    argc: 0,
                    retc: 0,
                }, // 2: Branch B task
                Instr::End,          // 3: Branch B end
            ],
            debug_map: BTreeMap::from([(2, "slow_task".to_string())]),
            join_plan: BTreeMap::new(),
            wait_plan: BTreeMap::new(),
            race_plan: BTreeMap::new(),
            boundary_map: BTreeMap::new(),
            write_set: BTreeMap::new(),
            task_manifest: vec!["slow_task".to_string()],
            error_route_map: BTreeMap::new(),
        };
        store
            .store_program(program.bytecode_version, &program)
            .await
            .unwrap();

        let instance_id = engine
            .start("test", program.bytecode_version, "{}", [0u8; 32], "corr-2")
            .await
            .unwrap();

        // Tick until instance reaches terminal state.
        for _ in 0..5 {
            engine.tick_instance(instance_id).await.unwrap();
            let inst = store.load_instance(instance_id).await.unwrap().unwrap();
            if inst.state.is_terminal() {
                break;
            }
        }

        // Assert: instance is Terminated (not Completed, not Running)
        let instance = store.load_instance(instance_id).await.unwrap().unwrap();
        assert!(
            matches!(instance.state, ProcessState::Terminated { .. }),
            "Expected Terminated, got {:?}",
            instance.state
        );

        // Assert: no fibers remain
        let fibers = store.load_fibers(instance_id).await.unwrap();
        assert!(fibers.is_empty(), "All fibers should be deleted");

        // Assert: Terminated event emitted
        let events = store.read_events(instance_id, 0).await.unwrap();
        let has_term = events
            .iter()
            .any(|(_, e)| matches!(e, RuntimeEvent::Terminated { .. }));
        assert!(has_term, "Should emit Terminated event");

        // Assert: no jobs for this instance remain
        let jobs = store
            .dequeue_jobs(&["slow_task".to_string()], 100)
            .await
            .unwrap();
        let instance_jobs: Vec<_> = jobs
            .iter()
            .filter(|j| j.process_instance_id == instance_id)
            .collect();
        assert!(
            instance_jobs.is_empty(),
            "No jobs should remain for terminated instance"
        );
    }

    /// T-TERM-3: complete_job on Terminated instance → safe via is_terminal() guard.
    #[tokio::test]
    async fn t_term_3_complete_job_after_terminate() {
        let store = Arc::new(MemoryStore::new());
        let engine = BpmnLiteEngine::new(store.clone());

        // Single fiber: ExecNative → EndTerminate
        let program = CompiledProgram {
            bytecode_version: [42u8; 32],
            program: vec![
                Instr::ExecNative {
                    task_type: 0,
                    argc: 0,
                    retc: 0,
                },
                Instr::EndTerminate,
            ],
            debug_map: BTreeMap::from([(0, "task_x".to_string())]),
            join_plan: BTreeMap::new(),
            wait_plan: BTreeMap::new(),
            race_plan: BTreeMap::new(),
            boundary_map: BTreeMap::new(),
            write_set: BTreeMap::new(),
            task_manifest: vec!["task_x".to_string()],
            error_route_map: BTreeMap::new(),
        };
        store
            .store_program(program.bytecode_version, &program)
            .await
            .unwrap();

        let instance_id = engine
            .start("test", program.bytecode_version, "{}", [0u8; 32], "corr-3")
            .await
            .unwrap();
        let jobs = engine.run_instance(instance_id).await.unwrap();
        let job_key = jobs[0].job_key.clone();

        // Complete the job → fiber advances to EndTerminate → instance Terminated
        let payload = "{}";
        let hash = compute_hash(payload);
        engine
            .complete_job(&job_key, payload, hash, BTreeMap::new())
            .await
            .unwrap();
        engine.tick_instance(instance_id).await.unwrap();

        assert!(matches!(
            store
                .load_instance(instance_id)
                .await
                .unwrap()
                .unwrap()
                .state,
            ProcessState::Terminated { .. }
        ));

        // Now try a SECOND complete_job with the same key (ghost signal)
        // Should be safe — is_terminal() guard catches it
        let result = engine
            .complete_job(&job_key, payload, hash, BTreeMap::new())
            .await;
        assert!(
            result.is_ok(),
            "Late complete_job on Terminated instance should not error"
        );

        // State unchanged
        let instance = store.load_instance(instance_id).await.unwrap().unwrap();
        assert!(matches!(instance.state, ProcessState::Terminated { .. }));
    }

    /// T-TERM-4: Parser + lowering: <terminateEventDefinition> → EndTerminate instruction.
    /// NOTE: engine.compile() returns CompileResult. Use store.load_program() to inspect bytecode.
    #[tokio::test]
    async fn t_term_4_parse_terminate_end_event() {
        let bpmn_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<bpmn:definitions xmlns:bpmn="http://www.omg.org/spec/BPMN/20100524/MODEL"
                  xmlns:zeebe="http://camunda.org/schema/zeebe/1.0">
  <bpmn:process id="proc_1" isExecutable="true">
    <bpmn:startEvent id="start"/>
    <bpmn:serviceTask id="task_a" name="Task A">
      <bpmn:extensionElements>
        <zeebe:taskDefinition type="task_a"/>
      </bpmn:extensionElements>
    </bpmn:serviceTask>
    <bpmn:endEvent id="end_term">
      <bpmn:terminateEventDefinition/>
    </bpmn:endEvent>
    <bpmn:sequenceFlow id="f1" sourceRef="start" targetRef="task_a"/>
    <bpmn:sequenceFlow id="f2" sourceRef="task_a" targetRef="end_term"/>
  </bpmn:process>
</bpmn:definitions>"#;

        let store = Arc::new(MemoryStore::new());
        let engine = BpmnLiteEngine::new(store.clone());

        let compile_result = engine.compile(bpmn_xml).await;
        assert!(
            compile_result.is_ok(),
            "Should compile: {:?}",
            compile_result.err()
        );

        let compiled = compile_result.unwrap();

        // Load the actual program from store to inspect instructions
        let program = store
            .load_program(compiled.bytecode_version)
            .await
            .unwrap()
            .expect("Program should be stored after compile");

        let has_end_terminate = program
            .program
            .iter()
            .any(|i| matches!(i, Instr::EndTerminate));
        assert!(
            has_end_terminate,
            "Program should contain EndTerminate instruction"
        );
    }

    // ═══════════════════════════════════════════════════════════
    //  Phase 5.2: Error boundary routing
    // ═══════════════════════════════════════════════════════════

    /// T-ERR-1: BusinessRejection with matching error route → fiber routes to escalation.
    #[tokio::test]
    async fn t_err_1_business_error_routes_to_handler() {
        let store = Arc::new(MemoryStore::new());
        let engine = BpmnLiteEngine::new(store.clone());

        // Bytecode:
        // 0: ExecNative(sanctions_check)  — parks fiber
        // 1: Jump(4)                      — normal continuation
        // 2: ExecNative(enhanced_review)  — error handler path
        // 3: End                          — error handler end
        // 4: End                          — normal end
        let program = CompiledProgram {
            bytecode_version: [50u8; 32],
            program: vec![
                Instr::ExecNative {
                    task_type: 0,
                    argc: 0,
                    retc: 0,
                }, // 0
                Instr::Jump { target: 4 }, // 1
                Instr::ExecNative {
                    task_type: 1,
                    argc: 0,
                    retc: 0,
                }, // 2: error handler
                Instr::End,                // 3
                Instr::End,                // 4
            ],
            debug_map: BTreeMap::from([
                (0, "sanctions_check".to_string()),
                (2, "enhanced_review".to_string()),
            ]),
            join_plan: BTreeMap::new(),
            wait_plan: BTreeMap::new(),
            race_plan: BTreeMap::new(),
            boundary_map: BTreeMap::new(),
            write_set: BTreeMap::new(),
            task_manifest: vec!["sanctions_check".to_string(), "enhanced_review".to_string()],
            error_route_map: BTreeMap::from([(
                0,
                vec![ErrorRoute {
                    error_code: Some("SANCTIONS_HIT".to_string()),
                    resume_at: 2,
                    boundary_element_id: "catch_sanctions".to_string(),
                }],
            )]),
        };
        store
            .store_program(program.bytecode_version, &program)
            .await
            .unwrap();

        let instance_id = engine
            .start("test", program.bytecode_version, "{}", [0u8; 32], "corr-1")
            .await
            .unwrap();
        let jobs = engine.run_instance(instance_id).await.unwrap();
        assert_eq!(jobs.len(), 1);
        let job_key = jobs[0].job_key.clone();

        // Fail with matching error code
        engine
            .fail_job(
                &job_key,
                ErrorClass::BusinessRejection {
                    rejection_code: "SANCTIONS_HIT".to_string(),
                },
                "Sanctions screening returned a hit",
            )
            .await
            .unwrap();

        // Assert: ErrorRouted event emitted
        let events = store.read_events(instance_id, 0).await.unwrap();
        let has_routed = events.iter().any(|(_, e)| {
            matches!(e, RuntimeEvent::ErrorRouted { error_code, .. } if error_code == "SANCTIONS_HIT")
        });
        assert!(has_routed, "Should emit ErrorRouted event");

        // Assert: NO incident created
        let has_incident = events
            .iter()
            .any(|(_, e)| matches!(e, RuntimeEvent::IncidentCreated { .. }));
        assert!(
            !has_incident,
            "Should NOT create incident when error route matches"
        );

        // Assert: instance is still Running (not Failed)
        let instance = store.load_instance(instance_id).await.unwrap().unwrap();
        assert!(
            matches!(instance.state, ProcessState::Running),
            "Instance should stay Running after error routing, got {:?}",
            instance.state
        );

        // Assert: fiber was routed to error handler (pc=2)
        let fibers = store.load_fibers(instance_id).await.unwrap();
        let routed_fiber = fibers.iter().find(|f| f.wait == WaitState::Running);
        assert!(
            routed_fiber.is_some(),
            "Fiber should be Running at error handler path"
        );

        // Tick to advance the routed fiber
        engine.tick_instance(instance_id).await.unwrap();

        // Should now have a job for enhanced_review
        let new_jobs = store
            .dequeue_jobs(&["enhanced_review".to_string()], 10)
            .await
            .unwrap();
        assert!(
            !new_jobs.is_empty(),
            "Should activate enhanced_review job after routing"
        );
    }

    /// T-ERR-2: BusinessRejection with NO matching route → incident (existing behavior).
    #[tokio::test]
    async fn t_err_2_unmatched_error_creates_incident() {
        let store = Arc::new(MemoryStore::new());
        let engine = BpmnLiteEngine::new(store.clone());

        // Same program but error_route_map only catches SANCTIONS_HIT
        let program = CompiledProgram {
            bytecode_version: [51u8; 32],
            program: vec![
                Instr::ExecNative {
                    task_type: 0,
                    argc: 0,
                    retc: 0,
                },
                Instr::End,
            ],
            debug_map: BTreeMap::from([(0, "task_a".to_string())]),
            join_plan: BTreeMap::new(),
            wait_plan: BTreeMap::new(),
            race_plan: BTreeMap::new(),
            boundary_map: BTreeMap::new(),
            write_set: BTreeMap::new(),
            task_manifest: vec!["task_a".to_string()],
            error_route_map: BTreeMap::from([(
                0,
                vec![ErrorRoute {
                    error_code: Some("SANCTIONS_HIT".to_string()),
                    resume_at: 99, // doesn't matter, won't be used
                    boundary_element_id: "catch_sanctions".to_string(),
                }],
            )]),
        };
        store
            .store_program(program.bytecode_version, &program)
            .await
            .unwrap();

        let instance_id = engine
            .start("test", program.bytecode_version, "{}", [0u8; 32], "corr-2")
            .await
            .unwrap();
        let jobs = engine.run_instance(instance_id).await.unwrap();

        // Fail with NON-matching error code
        engine
            .fail_job(
                &jobs[0].job_key,
                ErrorClass::BusinessRejection {
                    rejection_code: "KYC_EXPIRED".to_string(),
                },
                "KYC expired",
            )
            .await
            .unwrap();

        // Assert: incident created
        let events = store.read_events(instance_id, 0).await.unwrap();
        let has_incident = events
            .iter()
            .any(|(_, e)| matches!(e, RuntimeEvent::IncidentCreated { .. }));
        assert!(has_incident, "Unmatched error should create incident");

        // Assert: NO ErrorRouted
        let has_routed = events
            .iter()
            .any(|(_, e)| matches!(e, RuntimeEvent::ErrorRouted { .. }));
        assert!(
            !has_routed,
            "Should NOT emit ErrorRouted for unmatched code"
        );

        // Assert: instance Failed
        let instance = store.load_instance(instance_id).await.unwrap().unwrap();
        assert!(matches!(instance.state, ProcessState::Failed { .. }));
    }

    /// T-ERR-3: Catch-all error route (error_code: None) catches any BusinessRejection.
    #[tokio::test]
    async fn t_err_3_catch_all_routes_any_business_error() {
        let store = Arc::new(MemoryStore::new());
        let engine = BpmnLiteEngine::new(store.clone());

        let program = CompiledProgram {
            bytecode_version: [52u8; 32],
            program: vec![
                Instr::ExecNative {
                    task_type: 0,
                    argc: 0,
                    retc: 0,
                }, // 0
                Instr::Jump { target: 3 }, // 1
                Instr::End,                // 2: error handler end
                Instr::End,                // 3: normal end
            ],
            debug_map: BTreeMap::from([(0, "task_a".to_string())]),
            join_plan: BTreeMap::new(),
            wait_plan: BTreeMap::new(),
            race_plan: BTreeMap::new(),
            boundary_map: BTreeMap::new(),
            write_set: BTreeMap::new(),
            task_manifest: vec!["task_a".to_string()],
            error_route_map: BTreeMap::from([(
                0,
                vec![ErrorRoute {
                    error_code: None, // catch-all
                    resume_at: 2,
                    boundary_element_id: "catch_all".to_string(),
                }],
            )]),
        };
        store
            .store_program(program.bytecode_version, &program)
            .await
            .unwrap();

        let instance_id = engine
            .start("test", program.bytecode_version, "{}", [0u8; 32], "corr-3")
            .await
            .unwrap();
        let jobs = engine.run_instance(instance_id).await.unwrap();

        // Fail with ANY business error — catch-all should match
        engine
            .fail_job(
                &jobs[0].job_key,
                ErrorClass::BusinessRejection {
                    rejection_code: "ANYTHING_GOES".to_string(),
                },
                "some error",
            )
            .await
            .unwrap();

        // Assert: routed, not incident
        let events = store.read_events(instance_id, 0).await.unwrap();
        let has_routed = events
            .iter()
            .any(|(_, e)| matches!(e, RuntimeEvent::ErrorRouted { .. }));
        assert!(has_routed, "Catch-all should route any BusinessRejection");

        let instance = store.load_instance(instance_id).await.unwrap().unwrap();
        assert!(matches!(instance.state, ProcessState::Running));
    }

    /// T-ERR-4: Transient error always creates incident, even with error route present.
    #[tokio::test]
    async fn t_err_4_transient_error_always_incident() {
        let store = Arc::new(MemoryStore::new());
        let engine = BpmnLiteEngine::new(store.clone());

        let program = CompiledProgram {
            bytecode_version: [53u8; 32],
            program: vec![
                Instr::ExecNative {
                    task_type: 0,
                    argc: 0,
                    retc: 0,
                },
                Instr::End,
                Instr::End, // error handler (won't be used)
            ],
            debug_map: BTreeMap::from([(0, "task_a".to_string())]),
            join_plan: BTreeMap::new(),
            wait_plan: BTreeMap::new(),
            race_plan: BTreeMap::new(),
            boundary_map: BTreeMap::new(),
            write_set: BTreeMap::new(),
            task_manifest: vec!["task_a".to_string()],
            error_route_map: BTreeMap::from([(
                0,
                vec![ErrorRoute {
                    error_code: None, // catch-all
                    resume_at: 2,
                    boundary_element_id: "catch_all".to_string(),
                }],
            )]),
        };
        store
            .store_program(program.bytecode_version, &program)
            .await
            .unwrap();

        let instance_id = engine
            .start("test", program.bytecode_version, "{}", [0u8; 32], "corr-4")
            .await
            .unwrap();
        let jobs = engine.run_instance(instance_id).await.unwrap();

        // Fail with Transient — error routes should NOT apply
        engine
            .fail_job(&jobs[0].job_key, ErrorClass::Transient, "timeout")
            .await
            .unwrap();

        // Assert: incident, NOT routed
        let events = store.read_events(instance_id, 0).await.unwrap();
        let has_incident = events
            .iter()
            .any(|(_, e)| matches!(e, RuntimeEvent::IncidentCreated { .. }));
        assert!(has_incident, "Transient errors must always create incident");

        let has_routed = events
            .iter()
            .any(|(_, e)| matches!(e, RuntimeEvent::ErrorRouted { .. }));
        assert!(
            !has_routed,
            "Transient errors must NOT trigger error routes"
        );
    }

    /// T-ERR-5: fail_job on terminated instance → safe via is_terminal() guard.
    #[tokio::test]
    async fn t_err_5_fail_job_on_terminated_instance() {
        let store = Arc::new(MemoryStore::new());
        let engine = BpmnLiteEngine::new(store.clone());

        // Single fiber: ExecNative → EndTerminate
        let program = CompiledProgram {
            bytecode_version: [54u8; 32],
            program: vec![
                Instr::ExecNative {
                    task_type: 0,
                    argc: 0,
                    retc: 0,
                },
                Instr::EndTerminate,
            ],
            debug_map: BTreeMap::from([(0, "task_a".to_string())]),
            join_plan: BTreeMap::new(),
            wait_plan: BTreeMap::new(),
            race_plan: BTreeMap::new(),
            boundary_map: BTreeMap::new(),
            write_set: BTreeMap::new(),
            task_manifest: vec!["task_a".to_string()],
            error_route_map: BTreeMap::new(),
        };
        store
            .store_program(program.bytecode_version, &program)
            .await
            .unwrap();

        let instance_id = engine
            .start("test", program.bytecode_version, "{}", [0u8; 32], "corr-5")
            .await
            .unwrap();
        let jobs = engine.run_instance(instance_id).await.unwrap();
        let job_key = jobs[0].job_key.clone();

        // Complete job → EndTerminate → Terminated
        let payload = "{}";
        let hash = compute_hash(payload);
        engine
            .complete_job(&job_key, payload, hash, BTreeMap::new())
            .await
            .unwrap();
        engine.tick_instance(instance_id).await.unwrap();

        assert!(matches!(
            store
                .load_instance(instance_id)
                .await
                .unwrap()
                .unwrap()
                .state,
            ProcessState::Terminated { .. }
        ));

        // Late fail_job — should be safe
        let result = engine
            .fail_job(
                &job_key,
                ErrorClass::BusinessRejection {
                    rejection_code: "LATE".to_string(),
                },
                "late failure",
            )
            .await;
        assert!(
            result.is_ok(),
            "fail_job on terminated instance should not error"
        );

        // Assert: SignalIgnored event
        let events = store.read_events(instance_id, 0).await.unwrap();
        let has_ignored = events
            .iter()
            .any(|(_, e)| matches!(e, RuntimeEvent::SignalIgnored { .. }));
        assert!(has_ignored, "Should emit SignalIgnored for late fail_job");
    }

    // ═══════════════════════════════════════════════════════════
    //  Phase 5.3: Bounded loops
    // ═══════════════════════════════════════════════════════════

    /// T-LOOP-1: IncCounter + BrCounterLt retry loop executes exactly N times.
    #[tokio::test]
    async fn t_loop_1_bounded_retry_executes_n_times() {
        let store = Arc::new(MemoryStore::new());
        let engine = BpmnLiteEngine::new(store.clone());

        // Simulates: task_a fails → error route → IncCounter → BrCounterLt(limit=3) → retry or end
        // Bytecode:
        // 0: ExecNative(task_a)         — parks fiber
        // 1: Jump(5)                    — normal end (skip error handler)
        // 2: IncCounter(0)              — error handler: bump counter
        // 3: BrCounterLt(0, 3, 0)      — if counter<3, retry task_a
        // 4: End                        — counter exhausted, escalation end
        // 5: End                        — normal end
        let program = CompiledProgram {
            bytecode_version: [60u8; 32],
            program: vec![
                Instr::ExecNative {
                    task_type: 0,
                    argc: 0,
                    retc: 0,
                }, // 0
                Instr::Jump { target: 5 },           // 1
                Instr::IncCounter { counter_id: 0 }, // 2
                Instr::BrCounterLt {
                    counter_id: 0,
                    limit: 3,
                    target: 0,
                }, // 3
                Instr::End,                          // 4
                Instr::End,                          // 5
            ],
            debug_map: BTreeMap::from([(0, "task_a".to_string())]),
            join_plan: BTreeMap::new(),
            wait_plan: BTreeMap::new(),
            race_plan: BTreeMap::new(),
            boundary_map: BTreeMap::new(),
            write_set: BTreeMap::new(),
            task_manifest: vec!["task_a".to_string()],
            error_route_map: BTreeMap::from([(
                0,
                vec![ErrorRoute {
                    error_code: Some("RETRY_ME".to_string()),
                    resume_at: 2,
                    boundary_element_id: "catch_retry".to_string(),
                }],
            )]),
        };
        store
            .store_program(program.bytecode_version, &program)
            .await
            .unwrap();

        let instance_id = engine
            .start("test", program.bytecode_version, "{}", [0u8; 32], "corr-1")
            .await
            .unwrap();

        // Iteration 1: activate → fail → error route → IncCounter(counter=1) → BrCounterLt(1<3 → retry)
        let jobs = engine.run_instance(instance_id).await.unwrap();
        assert_eq!(jobs.len(), 1);
        engine
            .fail_job(
                &jobs[0].job_key,
                ErrorClass::BusinessRejection {
                    rejection_code: "RETRY_ME".to_string(),
                },
                "attempt 1",
            )
            .await
            .unwrap();
        // Fiber is Running at addr 2 (IncCounter). Tick to advance through IncCounter → BrCounterLt → back to 0
        engine.tick_instance(instance_id).await.unwrap();
        // Now fiber is at addr 0 again (ExecNative), parks on job
        let jobs = engine.run_instance(instance_id).await.unwrap();
        assert_eq!(jobs.len(), 1, "Iteration 2 should activate task_a");

        // Iteration 2: fail again
        engine
            .fail_job(
                &jobs[0].job_key,
                ErrorClass::BusinessRejection {
                    rejection_code: "RETRY_ME".to_string(),
                },
                "attempt 2",
            )
            .await
            .unwrap();
        engine.tick_instance(instance_id).await.unwrap();
        let jobs = engine.run_instance(instance_id).await.unwrap();
        assert_eq!(jobs.len(), 1, "Iteration 3 should activate task_a");

        // Iteration 3: fail one more time → counter=3, BrCounterLt(3<3=false) → fall through to End
        engine
            .fail_job(
                &jobs[0].job_key,
                ErrorClass::BusinessRejection {
                    rejection_code: "RETRY_ME".to_string(),
                },
                "attempt 3",
            )
            .await
            .unwrap();
        engine.tick_instance(instance_id).await.unwrap();

        // Counter exhausted: fiber fell through to addr 4 (End). Tick to complete.
        engine.tick_instance(instance_id).await.unwrap();

        // Assert: instance completed (via End, not stuck in loop)
        let instance = store.load_instance(instance_id).await.unwrap().unwrap();
        assert!(
            matches!(instance.state, ProcessState::Completed { .. }),
            "Expected Completed after counter exhaustion, got {:?}",
            instance.state
        );

        // Assert: counter value is 3
        assert_eq!(instance.counters.get(&0), Some(&3));

        // Assert: 3 ErrorRouted events
        let events = store.read_events(instance_id, 0).await.unwrap();
        let routed_count = events
            .iter()
            .filter(|(_, e)| matches!(e, RuntimeEvent::ErrorRouted { .. }))
            .count();
        assert_eq!(routed_count, 3, "Should have exactly 3 error routes");
    }

    /// T-LOOP-2: Job keys are unique across loop iterations (loop_epoch in key).
    #[tokio::test]
    async fn t_loop_2_unique_job_keys_per_iteration() {
        let store = Arc::new(MemoryStore::new());
        let engine = BpmnLiteEngine::new(store.clone());

        let program = CompiledProgram {
            bytecode_version: [61u8; 32],
            program: vec![
                Instr::ExecNative {
                    task_type: 0,
                    argc: 0,
                    retc: 0,
                }, // 0
                Instr::Jump { target: 5 },           // 1
                Instr::IncCounter { counter_id: 0 }, // 2
                Instr::BrCounterLt {
                    counter_id: 0,
                    limit: 2,
                    target: 0,
                }, // 3
                Instr::End,                          // 4
                Instr::End,                          // 5
            ],
            debug_map: BTreeMap::from([(0, "task_a".to_string())]),
            join_plan: BTreeMap::new(),
            wait_plan: BTreeMap::new(),
            race_plan: BTreeMap::new(),
            boundary_map: BTreeMap::new(),
            write_set: BTreeMap::new(),
            task_manifest: vec!["task_a".to_string()],
            error_route_map: BTreeMap::from([(
                0,
                vec![ErrorRoute {
                    error_code: None, // catch-all
                    resume_at: 2,
                    boundary_element_id: "catch_all".to_string(),
                }],
            )]),
        };
        store
            .store_program(program.bytecode_version, &program)
            .await
            .unwrap();

        let instance_id = engine
            .start("test", program.bytecode_version, "{}", [0u8; 32], "corr-2")
            .await
            .unwrap();

        let mut all_job_keys = Vec::new();

        // Iteration 1
        let jobs = engine.run_instance(instance_id).await.unwrap();
        all_job_keys.push(jobs[0].job_key.clone());
        engine
            .fail_job(
                &jobs[0].job_key,
                ErrorClass::BusinessRejection {
                    rejection_code: "ERR".to_string(),
                },
                "err",
            )
            .await
            .unwrap();
        engine.tick_instance(instance_id).await.unwrap();

        // Iteration 2
        let jobs = engine.run_instance(instance_id).await.unwrap();
        all_job_keys.push(jobs[0].job_key.clone());

        // Assert: job keys are different despite same PC
        assert_ne!(
            all_job_keys[0], all_job_keys[1],
            "Job keys must differ across iterations: {:?}",
            all_job_keys
        );

        // Both keys should end with different epochs
        assert!(
            all_job_keys[0].ends_with(":0"),
            "First key epoch 0: {}",
            all_job_keys[0]
        );
        assert!(
            all_job_keys[1].ends_with(":1"),
            "Second key epoch 1: {}",
            all_job_keys[1]
        );
    }

    /// T-LOOP-3: BrCounterLt with counter=0 (never incremented) → always branches if limit>0.
    #[tokio::test]
    async fn t_loop_3_counter_starts_at_zero() {
        let store: Arc<dyn ProcessStore> = Arc::new(MemoryStore::new());
        let vm = Vm::new(store.clone());

        let program = CompiledProgram {
            bytecode_version: [62u8; 32],
            program: vec![
                Instr::BrCounterLt {
                    counter_id: 5,
                    limit: 1,
                    target: 2,
                }, // 0: counter=0 < 1 → jump to 2
                Instr::Fail { code: 99 }, // 1: unreachable
                Instr::End,               // 2: landed here
            ],
            debug_map: BTreeMap::new(),
            join_plan: BTreeMap::new(),
            wait_plan: BTreeMap::new(),
            race_plan: BTreeMap::new(),
            boundary_map: BTreeMap::new(),
            write_set: BTreeMap::new(),
            task_manifest: vec![],
            error_route_map: BTreeMap::new(),
        };
        store
            .store_program(program.bytecode_version, &program)
            .await
            .unwrap();

        let mut instance = ProcessInstance {
            instance_id: Uuid::now_v7(),
            process_key: "test".to_string(),
            bytecode_version: program.bytecode_version,
            domain_payload: "{}".to_string(),
            domain_payload_hash: [0u8; 32],
            flags: BTreeMap::new(),
            counters: BTreeMap::new(),
            join_expected: BTreeMap::new(),
            state: ProcessState::Running,
            correlation_id: "corr".to_string(),
            created_at: 0,
        };
        store.save_instance(&instance).await.unwrap();

        let mut fiber = Fiber::new(Uuid::now_v7(), 0);
        store
            .save_fiber(instance.instance_id, &fiber)
            .await
            .unwrap();

        let outcome = vm
            .run_fiber(&mut fiber, &mut instance, &program, 100)
            .await
            .unwrap();

        // Should have jumped to 2 (End) and ended
        assert!(
            matches!(outcome, TickOutcome::Ended),
            "Counter 5 starts at 0, 0 < 1 should branch to End. Got: {:?}",
            outcome
        );
    }

    /// T-LOOP-4: Bytecode verifier rejects unguarded backward Jump.
    #[tokio::test]
    async fn t_loop_4_verifier_rejects_backward_jump() {
        let program = CompiledProgram {
            bytecode_version: [63u8; 32],
            program: vec![
                Instr::ExecNative {
                    task_type: 0,
                    argc: 0,
                    retc: 0,
                }, // 0
                Instr::Jump { target: 0 }, // 1: backward jump! infinite loop
                Instr::End,                // 2: unreachable
            ],
            debug_map: BTreeMap::from([(0, "task_a".to_string())]),
            join_plan: BTreeMap::new(),
            wait_plan: BTreeMap::new(),
            race_plan: BTreeMap::new(),
            boundary_map: BTreeMap::new(),
            write_set: BTreeMap::new(),
            task_manifest: vec!["task_a".to_string()],
            error_route_map: BTreeMap::new(),
        };

        let errors = crate::compiler::verifier::verify_bytecode(&program);
        assert!(!errors.is_empty(), "Should reject backward Jump");
        assert!(
            errors[0].message.contains("Backward jump"),
            "Error should mention backward jump: {}",
            errors[0].message
        );
    }

    /// T-LOOP-5: Bytecode verifier allows BrCounterLt backward jump.
    #[tokio::test]
    async fn t_loop_5_verifier_allows_br_counter_lt_backward() {
        let program = CompiledProgram {
            bytecode_version: [64u8; 32],
            program: vec![
                Instr::ExecNative {
                    task_type: 0,
                    argc: 0,
                    retc: 0,
                }, // 0
                Instr::IncCounter { counter_id: 0 }, // 1
                Instr::BrCounterLt {
                    counter_id: 0,
                    limit: 3,
                    target: 0,
                }, // 2: backward, but bounded
                Instr::End,                          // 3
            ],
            debug_map: BTreeMap::new(),
            join_plan: BTreeMap::new(),
            wait_plan: BTreeMap::new(),
            race_plan: BTreeMap::new(),
            boundary_map: BTreeMap::new(),
            write_set: BTreeMap::new(),
            task_manifest: vec!["task_a".to_string()],
            error_route_map: BTreeMap::new(),
        };

        let errors = crate::compiler::verifier::verify_bytecode(&program);
        assert!(
            errors.is_empty(),
            "BrCounterLt backward should be allowed, got errors: {:?}",
            errors.iter().map(|e| &e.message).collect::<Vec<_>>()
        );
    }

    // ═══════════════════════════════════════════════════════════
    //  Phase 5A: Inclusive gateway
    // ═══════════════════════════════════════════════════════════

    /// T-IG-1: All conditions truthy → all branches run → join waits for all → completes.
    #[tokio::test]
    async fn t_ig_1_all_branches_taken() {
        let store = Arc::new(MemoryStore::new());
        let engine = BpmnLiteEngine::new(store.clone());

        let program = CompiledProgram {
            bytecode_version: [70u8; 32],
            program: vec![
                Instr::ForkInclusive {
                    branches: Box::new([
                        InclusiveBranch {
                            condition_flag: None,
                            target: 2,
                        },
                        InclusiveBranch {
                            condition_flag: Some(0),
                            target: 4,
                        },
                        InclusiveBranch {
                            condition_flag: Some(1),
                            target: 6,
                        },
                    ]),
                    join_id: 0,
                    default_target: None,
                },
                Instr::End, // 1: placeholder
                Instr::ExecNative {
                    task_type: 0,
                    argc: 0,
                    retc: 0,
                }, // 2: identity_check
                Instr::JoinDynamic { id: 0, next: 8 }, // 3
                Instr::ExecNative {
                    task_type: 1,
                    argc: 0,
                    retc: 0,
                }, // 4: edd_check
                Instr::JoinDynamic { id: 0, next: 8 }, // 5
                Instr::ExecNative {
                    task_type: 2,
                    argc: 0,
                    retc: 0,
                }, // 6: pep_screening
                Instr::JoinDynamic { id: 0, next: 8 }, // 7
                Instr::End, // 8: done
            ],
            debug_map: BTreeMap::from([
                (2, "identity_check".to_string()),
                (4, "edd_check".to_string()),
                (6, "pep_screening".to_string()),
            ]),
            join_plan: BTreeMap::new(),
            wait_plan: BTreeMap::new(),
            race_plan: BTreeMap::new(),
            boundary_map: BTreeMap::new(),
            write_set: BTreeMap::new(),
            task_manifest: vec![
                "identity_check".to_string(),
                "edd_check".to_string(),
                "pep_screening".to_string(),
            ],
            error_route_map: BTreeMap::new(),
        };
        store
            .store_program(program.bytecode_version, &program)
            .await
            .unwrap();

        // Start with both flags true → all 3 branches taken
        let instance_id = engine
            .start("test", program.bytecode_version, "{}", [0u8; 32], "corr-1")
            .await
            .unwrap();

        // Set flags before first tick
        let mut inst = store.load_instance(instance_id).await.unwrap().unwrap();
        inst.flags.insert(0, Value::Bool(true)); // high_risk
        inst.flags.insert(1, Value::Bool(true)); // pep_flagged
        store.save_instance(&inst).await.unwrap();

        // Tick → ForkInclusive evaluates: all 3 taken → 3 fibers spawned
        engine.tick_instance(instance_id).await.unwrap();

        // Assert: InclusiveForkTaken event with expected=3
        let events = store.read_events(instance_id, 0).await.unwrap();
        let fork_event = events
            .iter()
            .find(|(_, e)| matches!(e, RuntimeEvent::InclusiveForkTaken { .. }));
        assert!(fork_event.is_some(), "Should emit InclusiveForkTaken");

        // Assert: join_expected[0] = 3
        let inst = store.load_instance(instance_id).await.unwrap().unwrap();
        assert_eq!(inst.join_expected.get(&0), Some(&3));

        // Run → 3 jobs activated
        let jobs = engine.run_instance(instance_id).await.unwrap();
        assert_eq!(jobs.len(), 3, "All 3 branches should activate jobs");

        // Complete all 3 jobs
        for job in &jobs {
            let payload = "{}";
            let hash = crate::vm::compute_hash(payload);
            engine
                .complete_job(&job.job_key, payload, hash, BTreeMap::new())
                .await
                .unwrap();
        }

        // Tick until complete
        for _ in 0..5 {
            engine.tick_instance(instance_id).await.unwrap();
            let inst = store.load_instance(instance_id).await.unwrap().unwrap();
            if inst.state.is_terminal() {
                break;
            }
        }

        let inst = store.load_instance(instance_id).await.unwrap().unwrap();
        assert!(
            matches!(inst.state, ProcessState::Completed { .. }),
            "Expected Completed, got {:?}",
            inst.state
        );
    }

    /// T-IG-2: Only 1 of 3 conditions truthy → 1 branch runs → join waits for 1 → immediate release.
    #[tokio::test]
    async fn t_ig_2_single_branch_taken() {
        let store = Arc::new(MemoryStore::new());
        let engine = BpmnLiteEngine::new(store.clone());

        let program = CompiledProgram {
            bytecode_version: [71u8; 32],
            program: vec![
                Instr::ForkInclusive {
                    branches: Box::new([
                        InclusiveBranch {
                            condition_flag: None,
                            target: 2,
                        },
                        InclusiveBranch {
                            condition_flag: Some(0),
                            target: 4,
                        },
                        InclusiveBranch {
                            condition_flag: Some(1),
                            target: 6,
                        },
                    ]),
                    join_id: 0,
                    default_target: None,
                },
                Instr::End,
                Instr::ExecNative {
                    task_type: 0,
                    argc: 0,
                    retc: 0,
                },
                Instr::JoinDynamic { id: 0, next: 8 },
                Instr::ExecNative {
                    task_type: 1,
                    argc: 0,
                    retc: 0,
                },
                Instr::JoinDynamic { id: 0, next: 8 },
                Instr::ExecNative {
                    task_type: 2,
                    argc: 0,
                    retc: 0,
                },
                Instr::JoinDynamic { id: 0, next: 8 },
                Instr::End,
            ],
            debug_map: BTreeMap::from([
                (2, "identity_check".to_string()),
                (4, "edd_check".to_string()),
                (6, "pep_screening".to_string()),
            ]),
            join_plan: BTreeMap::new(),
            wait_plan: BTreeMap::new(),
            race_plan: BTreeMap::new(),
            boundary_map: BTreeMap::new(),
            write_set: BTreeMap::new(),
            task_manifest: vec![
                "identity_check".to_string(),
                "edd_check".to_string(),
                "pep_screening".to_string(),
            ],
            error_route_map: BTreeMap::new(),
        };
        store
            .store_program(program.bytecode_version, &program)
            .await
            .unwrap();

        // Start with flags FALSE → only unconditional branch (A) taken
        let instance_id = engine
            .start("test", program.bytecode_version, "{}", [0u8; 32], "corr-2")
            .await
            .unwrap();

        // Tick → ForkInclusive: only branch A taken → 1 fiber
        engine.tick_instance(instance_id).await.unwrap();

        // Assert: join_expected[0] = 1
        let inst = store.load_instance(instance_id).await.unwrap().unwrap();
        assert_eq!(inst.join_expected.get(&0), Some(&1));

        // Run → 1 job
        let jobs = engine.run_instance(instance_id).await.unwrap();
        assert_eq!(jobs.len(), 1, "Only unconditional branch should spawn");

        // Complete job → JoinDynamic expects 1, 1 arrives → immediate release
        let payload = "{}";
        let hash = crate::vm::compute_hash(payload);
        engine
            .complete_job(&jobs[0].job_key, payload, hash, BTreeMap::new())
            .await
            .unwrap();

        for _ in 0..5 {
            engine.tick_instance(instance_id).await.unwrap();
            let inst = store.load_instance(instance_id).await.unwrap().unwrap();
            if inst.state.is_terminal() {
                break;
            }
        }

        let inst = store.load_instance(instance_id).await.unwrap().unwrap();
        assert!(matches!(inst.state, ProcessState::Completed { .. }));
    }

    /// T-IG-3: Zero conditions match, no default → incident.
    #[tokio::test]
    async fn t_ig_3_zero_match_no_default_incident() {
        let store = Arc::new(MemoryStore::new());
        let engine = BpmnLiteEngine::new(store.clone());

        // ALL branches conditional — no unconditional
        let program = CompiledProgram {
            bytecode_version: [72u8; 32],
            program: vec![
                Instr::ForkInclusive {
                    branches: Box::new([
                        InclusiveBranch {
                            condition_flag: Some(0),
                            target: 2,
                        },
                        InclusiveBranch {
                            condition_flag: Some(1),
                            target: 4,
                        },
                    ]),
                    join_id: 0,
                    default_target: None, // no default!
                },
                Instr::End,
                Instr::ExecNative {
                    task_type: 0,
                    argc: 0,
                    retc: 0,
                },
                Instr::JoinDynamic { id: 0, next: 6 },
                Instr::ExecNative {
                    task_type: 1,
                    argc: 0,
                    retc: 0,
                },
                Instr::JoinDynamic { id: 0, next: 6 },
                Instr::End,
            ],
            debug_map: BTreeMap::new(),
            join_plan: BTreeMap::new(),
            wait_plan: BTreeMap::new(),
            race_plan: BTreeMap::new(),
            boundary_map: BTreeMap::new(),
            write_set: BTreeMap::new(),
            task_manifest: vec!["task_a".to_string(), "task_b".to_string()],
            error_route_map: BTreeMap::new(),
        };
        store
            .store_program(program.bytecode_version, &program)
            .await
            .unwrap();

        let instance_id = engine
            .start("test", program.bytecode_version, "{}", [0u8; 32], "corr-3")
            .await
            .unwrap();
        // No flags set → all conditions false → zero match

        engine.tick_instance(instance_id).await.unwrap();

        // Assert: instance Failed with incident
        let inst = store.load_instance(instance_id).await.unwrap().unwrap();
        assert!(
            matches!(inst.state, ProcessState::Failed { .. }),
            "Zero match with no default should create incident, got {:?}",
            inst.state
        );

        let events = store.read_events(instance_id, 0).await.unwrap();
        let has_incident = events
            .iter()
            .any(|(_, e)| matches!(e, RuntimeEvent::IncidentCreated { .. }));
        assert!(has_incident, "Should emit IncidentCreated");
    }

    /// T-IG-4: Zero conditions match WITH default → default branch runs.
    #[tokio::test]
    async fn t_ig_4_zero_match_with_default() {
        let store = Arc::new(MemoryStore::new());
        let engine = BpmnLiteEngine::new(store.clone());

        let program = CompiledProgram {
            bytecode_version: [73u8; 32],
            program: vec![
                Instr::ForkInclusive {
                    branches: Box::new([InclusiveBranch {
                        condition_flag: Some(0),
                        target: 2,
                    }]),
                    join_id: 0,
                    default_target: Some(4), // default branch
                },
                Instr::End,
                Instr::ExecNative {
                    task_type: 0,
                    argc: 0,
                    retc: 0,
                }, // 2: conditional
                Instr::JoinDynamic { id: 0, next: 6 },
                Instr::ExecNative {
                    task_type: 1,
                    argc: 0,
                    retc: 0,
                }, // 4: default
                Instr::JoinDynamic { id: 0, next: 6 },
                Instr::End, // 6: done
            ],
            debug_map: BTreeMap::from([
                (2, "conditional_task".to_string()),
                (4, "default_task".to_string()),
            ]),
            join_plan: BTreeMap::new(),
            wait_plan: BTreeMap::new(),
            race_plan: BTreeMap::new(),
            boundary_map: BTreeMap::new(),
            write_set: BTreeMap::new(),
            task_manifest: vec!["conditional_task".to_string(), "default_task".to_string()],
            error_route_map: BTreeMap::new(),
        };
        store
            .store_program(program.bytecode_version, &program)
            .await
            .unwrap();

        let instance_id = engine
            .start("test", program.bytecode_version, "{}", [0u8; 32], "corr-4")
            .await
            .unwrap();
        // No flags → condition false → default taken

        engine.tick_instance(instance_id).await.unwrap();

        // Assert: join_expected = 1 (default branch only)
        let inst = store.load_instance(instance_id).await.unwrap().unwrap();
        assert_eq!(inst.join_expected.get(&0), Some(&1));

        // Run → should get default_task job
        let jobs = engine.run_instance(instance_id).await.unwrap();
        assert_eq!(jobs.len(), 1);

        // Complete and finish
        let payload = "{}";
        let hash = crate::vm::compute_hash(payload);
        engine
            .complete_job(&jobs[0].job_key, payload, hash, BTreeMap::new())
            .await
            .unwrap();
        for _ in 0..5 {
            engine.tick_instance(instance_id).await.unwrap();
            let inst = store.load_instance(instance_id).await.unwrap().unwrap();
            if inst.state.is_terminal() {
                break;
            }
        }

        let inst = store.load_instance(instance_id).await.unwrap().unwrap();
        assert!(matches!(inst.state, ProcessState::Completed { .. }));
    }

    /// T-IG-5: JoinDynamic releases only after dynamic expected count arrivals.
    #[tokio::test]
    async fn t_ig_5_join_waits_for_dynamic_count() {
        let store = Arc::new(MemoryStore::new());
        let engine = BpmnLiteEngine::new(store.clone());

        // 2 of 3 branches taken → join waits for exactly 2
        let program = CompiledProgram {
            bytecode_version: [74u8; 32],
            program: vec![
                Instr::ForkInclusive {
                    branches: Box::new([
                        InclusiveBranch {
                            condition_flag: None,
                            target: 2,
                        },
                        InclusiveBranch {
                            condition_flag: Some(0),
                            target: 4,
                        },
                        InclusiveBranch {
                            condition_flag: Some(1),
                            target: 6,
                        },
                    ]),
                    join_id: 0,
                    default_target: None,
                },
                Instr::End,
                Instr::ExecNative {
                    task_type: 0,
                    argc: 0,
                    retc: 0,
                },
                Instr::JoinDynamic { id: 0, next: 8 },
                Instr::ExecNative {
                    task_type: 1,
                    argc: 0,
                    retc: 0,
                },
                Instr::JoinDynamic { id: 0, next: 8 },
                Instr::ExecNative {
                    task_type: 2,
                    argc: 0,
                    retc: 0,
                },
                Instr::JoinDynamic { id: 0, next: 8 },
                Instr::End,
            ],
            debug_map: BTreeMap::from([
                (2, "task_a".to_string()),
                (4, "task_b".to_string()),
                (6, "task_c".to_string()),
            ]),
            join_plan: BTreeMap::new(),
            wait_plan: BTreeMap::new(),
            race_plan: BTreeMap::new(),
            boundary_map: BTreeMap::new(),
            write_set: BTreeMap::new(),
            task_manifest: vec![
                "task_a".to_string(),
                "task_b".to_string(),
                "task_c".to_string(),
            ],
            error_route_map: BTreeMap::new(),
        };
        store
            .store_program(program.bytecode_version, &program)
            .await
            .unwrap();

        let instance_id = engine
            .start("test", program.bytecode_version, "{}", [0u8; 32], "corr-5")
            .await
            .unwrap();

        // Set flag_0=true, flag_1=false → 2 branches taken (unconditional + flag_0)
        let mut inst = store.load_instance(instance_id).await.unwrap().unwrap();
        inst.flags.insert(0, Value::Bool(true));
        // flag 1 not set = false
        store.save_instance(&inst).await.unwrap();

        engine.tick_instance(instance_id).await.unwrap();

        assert_eq!(
            store
                .load_instance(instance_id)
                .await
                .unwrap()
                .unwrap()
                .join_expected
                .get(&0),
            Some(&2)
        );

        // Run → 2 jobs
        let jobs = engine.run_instance(instance_id).await.unwrap();
        assert_eq!(jobs.len(), 2, "Should have 2 jobs (branches A and B)");

        // Complete first job → join has 1/2, should NOT release yet
        let payload = "{}";
        let hash = crate::vm::compute_hash(payload);
        engine
            .complete_job(&jobs[0].job_key, payload, hash, BTreeMap::new())
            .await
            .unwrap();
        engine.tick_instance(instance_id).await.unwrap();

        // Instance still Running (waiting for 2nd branch)
        let inst = store.load_instance(instance_id).await.unwrap().unwrap();
        assert!(
            matches!(inst.state, ProcessState::Running),
            "Should still be Running, got {:?}",
            inst.state
        );

        // Complete second job → join has 2/2, releases
        engine
            .complete_job(&jobs[1].job_key, payload, hash, BTreeMap::new())
            .await
            .unwrap();
        for _ in 0..5 {
            engine.tick_instance(instance_id).await.unwrap();
            let inst = store.load_instance(instance_id).await.unwrap().unwrap();
            if inst.state.is_terminal() {
                break;
            }
        }

        let inst = store.load_instance(instance_id).await.unwrap().unwrap();
        assert!(matches!(inst.state, ProcessState::Completed { .. }));
    }

    /// T-IG-6: Full compiler pipeline — parse inclusiveGateway from BPMN XML.
    #[tokio::test]
    async fn t_ig_6_parse_inclusive_gateway() {
        let bpmn_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<bpmn:definitions xmlns:bpmn="http://www.omg.org/spec/BPMN/20100524/MODEL"
                  xmlns:zeebe="http://camunda.org/schema/zeebe/1.0">
  <bpmn:process id="proc_1" isExecutable="true">
    <bpmn:startEvent id="start"/>
    <bpmn:serviceTask id="prep" name="Prep">
      <bpmn:extensionElements><zeebe:taskDefinition type="prep"/></bpmn:extensionElements>
    </bpmn:serviceTask>
    <bpmn:inclusiveGateway id="ig_fork" gatewayDirection="Diverging"/>
    <bpmn:serviceTask id="task_a" name="Identity Check">
      <bpmn:extensionElements><zeebe:taskDefinition type="identity_check"/></bpmn:extensionElements>
    </bpmn:serviceTask>
    <bpmn:serviceTask id="task_b" name="EDD Check">
      <bpmn:extensionElements><zeebe:taskDefinition type="edd_check"/></bpmn:extensionElements>
    </bpmn:serviceTask>
    <bpmn:inclusiveGateway id="ig_join" gatewayDirection="Converging"/>
    <bpmn:endEvent id="end"/>
    <bpmn:sequenceFlow id="f1" sourceRef="start" targetRef="prep"/>
    <bpmn:sequenceFlow id="f2" sourceRef="prep" targetRef="ig_fork"/>
    <bpmn:sequenceFlow id="f3" sourceRef="ig_fork" targetRef="task_a"/>
    <bpmn:sequenceFlow id="f4" sourceRef="ig_fork" targetRef="task_b">
      <bpmn:conditionExpression>= high_risk</bpmn:conditionExpression>
    </bpmn:sequenceFlow>
    <bpmn:sequenceFlow id="f5" sourceRef="task_a" targetRef="ig_join"/>
    <bpmn:sequenceFlow id="f6" sourceRef="task_b" targetRef="ig_join"/>
    <bpmn:sequenceFlow id="f7" sourceRef="ig_join" targetRef="end"/>
  </bpmn:process>
</bpmn:definitions>"#;

        let store = Arc::new(MemoryStore::new());
        let engine = BpmnLiteEngine::new(store.clone());

        let result = engine.compile(bpmn_xml).await;
        assert!(
            result.is_ok(),
            "Should compile inclusive gateway BPMN: {:?}",
            result.err()
        );

        let compiled = result.unwrap();
        let program = store
            .load_program(compiled.bytecode_version)
            .await
            .unwrap()
            .unwrap();

        // Should contain ForkInclusive and JoinDynamic instructions
        let has_fork_inclusive = program
            .program
            .iter()
            .any(|i| matches!(i, Instr::ForkInclusive { .. }));
        assert!(
            has_fork_inclusive,
            "Should contain ForkInclusive instruction"
        );

        let has_join_dynamic = program
            .program
            .iter()
            .any(|i| matches!(i, Instr::JoinDynamic { .. }));
        assert!(has_join_dynamic, "Should contain JoinDynamic instruction");
    }
}
