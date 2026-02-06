//! Unified Runbook Model — Pack-Guided REPL v2
//!
//! The Runbook is the single source of truth for a user's work-in-progress.
//! It replaces DslSheet, StagedRunbook, RunSheet, and LedgerEntry with one
//! model that carries sentences, DSL, slot provenance, and audit trail.
//!
//! # Key invariants
//!
//! - Entries are ordered by `sequence` (1-based, gapless after reorder).
//! - Every entry has both a human-readable `sentence` and machine-executable `dsl`.
//! - `SlotProvenance` tracks where each argument value came from.
//! - `RunbookEvent` provides an append-only audit log of all mutations.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Runbook (top-level container)
// ---------------------------------------------------------------------------

/// A runbook is an ordered collection of entries that together describe
/// the work a user wants to execute within a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Runbook {
    pub id: Uuid,
    pub session_id: Uuid,
    pub client_group_id: Option<Uuid>,

    // Pack provenance (None for ad-hoc / non-pack sessions)
    pub pack_id: Option<String>,
    pub pack_version: Option<String>,
    pub pack_manifest_hash: Option<String>,

    // Template provenance (None if entries were added manually)
    pub template_id: Option<String>,
    pub template_hash: Option<String>,

    pub status: RunbookStatus,
    pub entries: Vec<RunbookEntry>,
    pub audit: Vec<RunbookEvent>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    /// Single-level undo stack for removed entries.
    #[serde(skip)]
    pub undo_stack: Vec<RunbookEntry>,

    /// Active invocations indexed by correlation_key for O(1) signal routing.
    #[serde(skip)]
    pub invocation_index: HashMap<String, Uuid>,
}

/// Top-level runbook lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunbookStatus {
    Draft,
    Building,
    Ready,
    Executing,
    Completed,
    Parked,
    Aborted,
}

// ---------------------------------------------------------------------------
// RunbookEntry (individual step)
// ---------------------------------------------------------------------------

/// A single step inside a runbook — carries both the human sentence and
/// the machine-executable DSL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunbookEntry {
    pub id: Uuid,
    pub sequence: i32,

    /// Human-readable sentence, e.g. "Add IRS product to Allianz Lux"
    pub sentence: String,

    /// Freeform labels for grouping / display (e.g. chapter, section).
    pub labels: HashMap<String, String>,

    /// S-expression DSL, e.g. "(cbu.assign-product :cbu-name ...)"
    pub dsl: String,

    /// Fully-qualified verb name, e.g. "cbu.assign-product"
    pub verb: String,

    /// Extracted arguments (key → value).
    pub args: HashMap<String, String>,

    /// Where each argument value came from.
    pub slot_provenance: SlotProvenance,

    /// Optional audit of the LLM arg-extraction call.
    pub arg_extraction_audit: Option<ArgExtractionAudit>,

    pub status: EntryStatus,
    pub execution_mode: ExecutionMode,
    pub confirm_policy: ConfirmPolicy,

    /// Entity references that still need resolution.
    pub unresolved_refs: Vec<UnresolvedRef>,

    /// Entry IDs this step depends on (must execute first).
    pub depends_on: Vec<Uuid>,

    /// Execution result (populated after run).
    pub result: Option<serde_json::Value>,

    /// Invocation record when entry is Parked (populated on park, cleared on resume).
    #[serde(default)]
    pub invocation: Option<InvocationRecord>,
}

/// Per-entry lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryStatus {
    Proposed,
    Confirmed,
    Resolved,
    Executing,
    Completed,
    Failed,
    Parked,
    /// Step is skipped during execution.
    Disabled,
}

/// How the step should be executed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    /// Normal synchronous execution.
    Sync,
    /// Durable / async execution with retry.
    Durable,
    /// Requires explicit human approval before proceeding.
    HumanGate,
}

/// When to ask for user confirmation on this step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmPolicy {
    /// Always ask before executing.
    Always,
    /// Quick-confirm (show sentence, auto-proceed after brief pause).
    QuickConfirm,
    /// Configured by the pack manifest.
    PackConfigured,
}

// ---------------------------------------------------------------------------
// Invocation Record (Phase 5: Durable Execution + Human Gates)
// ---------------------------------------------------------------------------

/// Links a parked runbook entry to an external signal for resumption.
///
/// Created when an entry is parked (either durable async or human gate).
/// The `correlation_key` is used to route inbound signals back to the
/// correct session and entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvocationRecord {
    pub invocation_id: Uuid,
    pub entry_id: Uuid,
    pub runbook_id: Uuid,
    pub session_id: Uuid,
    /// Deterministic correlation key for signal routing.
    pub correlation_key: String,
    /// External task_id in workflow_pending_tasks (if durable).
    pub task_id: Option<Uuid>,
    /// What we're waiting for.
    pub gate_type: GateType,
    /// Snapshot of context needed for resumption.
    pub captured_context: serde_json::Value,
    pub parked_at: DateTime<Utc>,
    pub timeout_at: Option<DateTime<Utc>>,
    pub resumed_at: Option<DateTime<Utc>>,
    pub status: InvocationStatus,
}

/// What kind of gate is blocking an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateType {
    /// Waiting for an external system to complete a task.
    DurableTask,
    /// Waiting for a human to approve before execution.
    HumanApproval,
}

/// Lifecycle of an invocation record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvocationStatus {
    /// Waiting for signal.
    Active,
    /// Signal received, entry resumed.
    Completed,
    /// No signal received within timeout window.
    TimedOut,
    /// User or system cancelled the wait.
    Cancelled,
}

impl InvocationRecord {
    /// Create a new invocation record for parking an entry.
    pub fn new(
        entry_id: Uuid,
        runbook_id: Uuid,
        session_id: Uuid,
        correlation_key: String,
        gate_type: GateType,
    ) -> Self {
        Self {
            invocation_id: Uuid::new_v4(),
            entry_id,
            runbook_id,
            session_id,
            correlation_key,
            task_id: None,
            gate_type,
            captured_context: serde_json::json!({}),
            parked_at: Utc::now(),
            timeout_at: None,
            resumed_at: None,
            status: InvocationStatus::Active,
        }
    }

    /// Build a deterministic correlation key from runbook + entry IDs.
    pub fn make_correlation_key(runbook_id: Uuid, entry_id: Uuid) -> String {
        format!("{}:{}", runbook_id, entry_id)
    }
}

// ---------------------------------------------------------------------------
// Slot Provenance
// ---------------------------------------------------------------------------

/// Tracks where each argument value came from — essential for auditability.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SlotProvenance {
    pub slots: HashMap<String, SlotSource>,
}

/// Origin of a single argument value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlotSource {
    /// User typed it in conversation.
    UserProvided,
    /// Came from the pack template default.
    TemplateDefault,
    /// Inferred from session / client context.
    InferredFromContext,
    /// Carried forward from a previous step's output.
    CopiedFromPrevious,
}

// ---------------------------------------------------------------------------
// Arg Extraction Audit
// ---------------------------------------------------------------------------

/// Audit record for an LLM-based argument extraction call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgExtractionAudit {
    pub model_id: String,
    pub prompt_hash: String,
    pub user_input: String,
    pub extracted_args: HashMap<String, String>,
    pub confidence: f64,
    pub timestamp: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Unresolved Reference
// ---------------------------------------------------------------------------

/// An entity reference that has not yet been resolved to a UUID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnresolvedRef {
    pub ref_id: String,
    pub display_text: String,
    pub entity_type: Option<String>,
    pub search_column: Option<String>,
}

// ---------------------------------------------------------------------------
// RunbookEvent (append-only audit log)
// ---------------------------------------------------------------------------

/// Event-sourced audit trail for all runbook mutations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunbookEvent {
    Created {
        timestamp: DateTime<Utc>,
    },
    EntryAdded {
        entry_id: Uuid,
        verb: String,
        sentence: String,
        timestamp: DateTime<Utc>,
    },
    EntryRemoved {
        entry_id: Uuid,
        reason: String,
        timestamp: DateTime<Utc>,
    },
    EntriesReordered {
        new_order: Vec<Uuid>,
        timestamp: DateTime<Utc>,
    },
    EntryStatusChanged {
        entry_id: Uuid,
        from: EntryStatus,
        to: EntryStatus,
        timestamp: DateTime<Utc>,
    },
    StatusChanged {
        from: RunbookStatus,
        to: RunbookStatus,
        timestamp: DateTime<Utc>,
    },
    PackAssociated {
        pack_id: String,
        pack_version: String,
        manifest_hash: String,
        timestamp: DateTime<Utc>,
    },
    TemplateInstantiated {
        template_id: String,
        template_hash: String,
        entry_count: usize,
        timestamp: DateTime<Utc>,
    },
    EntryArgChanged {
        entry_id: Uuid,
        field: String,
        old_value: Option<String>,
        new_value: String,
        sentence_before: String,
        sentence_after: String,
        timestamp: DateTime<Utc>,
    },
    EntryDisabled {
        entry_id: Uuid,
        timestamp: DateTime<Utc>,
    },
    EntryEnabled {
        entry_id: Uuid,
        timestamp: DateTime<Utc>,
    },
    RunbookCleared {
        entry_count: usize,
        timestamp: DateTime<Utc>,
    },
    // Phase 5: Durable Execution + Human Gates
    EntryParked {
        entry_id: Uuid,
        gate_type: GateType,
        invocation_id: Uuid,
        correlation_key: String,
        timestamp: DateTime<Utc>,
    },
    EntryResumed {
        entry_id: Uuid,
        invocation_id: Uuid,
        result: Option<serde_json::Value>,
        timestamp: DateTime<Utc>,
    },
    HumanGateRequested {
        entry_id: Uuid,
        invocation_id: Uuid,
        approver_hint: Option<String>,
        timestamp: DateTime<Utc>,
    },
    HumanGateApproved {
        entry_id: Uuid,
        invocation_id: Uuid,
        approved_by: Option<String>,
        timestamp: DateTime<Utc>,
    },
    HumanGateRejected {
        entry_id: Uuid,
        invocation_id: Uuid,
        rejected_by: Option<String>,
        reason: Option<String>,
        timestamp: DateTime<Utc>,
    },
    // Phase 6: Pack Handoff
    HandoffReceived {
        source_runbook_id: Uuid,
        target_pack_id: String,
        forwarded_context: HashMap<String, String>,
        timestamp: DateTime<Utc>,
    },
}

// ---------------------------------------------------------------------------
// ReadinessReport
// ---------------------------------------------------------------------------

/// Report on whether a runbook is ready for execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessReport {
    pub ready: bool,
    pub total_entries: usize,
    pub enabled_entries: usize,
    pub disabled_entries: usize,
    pub issues: Vec<ReadinessIssue>,
}

/// A single issue blocking execution readiness.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessIssue {
    pub entry_id: Uuid,
    pub sequence: i32,
    pub issue: String,
}

// ---------------------------------------------------------------------------
// Runbook implementation
// ---------------------------------------------------------------------------

impl Runbook {
    /// Create a new empty runbook for a session.
    pub fn new(session_id: Uuid) -> Self {
        let now = Utc::now();
        let id = Uuid::new_v4();
        Self {
            id,
            session_id,
            client_group_id: None,
            pack_id: None,
            pack_version: None,
            pack_manifest_hash: None,
            template_id: None,
            template_hash: None,
            status: RunbookStatus::Draft,
            entries: Vec::new(),
            audit: vec![RunbookEvent::Created { timestamp: now }],
            created_at: now,
            updated_at: now,
            undo_stack: Vec::new(),
            invocation_index: HashMap::new(),
        }
    }

    /// Add an entry at the end of the runbook. Returns the entry's ID.
    pub fn add_entry(&mut self, entry: RunbookEntry) -> Uuid {
        let entry_id = entry.id;
        self.audit.push(RunbookEvent::EntryAdded {
            entry_id,
            verb: entry.verb.clone(),
            sentence: entry.sentence.clone(),
            timestamp: Utc::now(),
        });
        self.entries.push(entry);
        self.renumber();
        self.touch();
        entry_id
    }

    /// Remove an entry by ID. Returns the removed entry, or `None` if not found.
    pub fn remove_entry(&mut self, entry_id: Uuid) -> Option<RunbookEntry> {
        let pos = self.entries.iter().position(|e| e.id == entry_id)?;
        let removed = self.entries.remove(pos);
        self.audit.push(RunbookEvent::EntryRemoved {
            entry_id,
            reason: "user_removed".to_string(),
            timestamp: Utc::now(),
        });
        self.renumber();
        self.touch();
        Some(removed)
    }

    /// Reorder entries to match the given ID sequence.
    /// IDs not present in the runbook are ignored.
    /// Entries not in the provided list are appended at the end in their
    /// original relative order.
    pub fn reorder(&mut self, ordered_ids: &[Uuid]) {
        let mut reordered: Vec<RunbookEntry> = Vec::with_capacity(self.entries.len());
        let mut remaining = self.entries.clone();

        for id in ordered_ids {
            if let Some(pos) = remaining.iter().position(|e| e.id == *id) {
                reordered.push(remaining.remove(pos));
            }
        }
        // Append any entries not mentioned in ordered_ids.
        reordered.append(&mut remaining);

        self.entries = reordered;
        self.renumber();
        self.audit.push(RunbookEvent::EntriesReordered {
            new_order: self.entries.iter().map(|e| e.id).collect(),
            timestamp: Utc::now(),
        });
        self.touch();
    }

    /// Look up an entry by ID.
    pub fn entry_by_id(&self, entry_id: Uuid) -> Option<&RunbookEntry> {
        self.entries.iter().find(|e| e.id == entry_id)
    }

    /// Mutable lookup by ID.
    pub fn entry_by_id_mut(&mut self, entry_id: Uuid) -> Option<&mut RunbookEntry> {
        self.entries.iter_mut().find(|e| e.id == entry_id)
    }

    /// Return entries filtered by status.
    pub fn entries_by_status(&self, status: EntryStatus) -> Vec<&RunbookEntry> {
        self.entries.iter().filter(|e| e.status == status).collect()
    }

    /// Produce a list of human-readable sentences (in order).
    pub fn display_sentences(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|e| format!("{}. {}", e.sequence, e.sentence))
            .collect()
    }

    /// Record a status transition on the runbook itself.
    pub fn set_status(&mut self, new_status: RunbookStatus) {
        let old = self.status;
        self.status = new_status;
        self.audit.push(RunbookEvent::StatusChanged {
            from: old,
            to: new_status,
            timestamp: Utc::now(),
        });
        self.touch();
    }

    /// Record a status transition on a specific entry.
    pub fn set_entry_status(
        &mut self,
        entry_id: Uuid,
        new_status: EntryStatus,
    ) -> Option<EntryStatus> {
        let entry = self.entries.iter_mut().find(|e| e.id == entry_id)?;
        let old = entry.status;
        entry.status = new_status;
        self.audit.push(RunbookEvent::EntryStatusChanged {
            entry_id,
            from: old,
            to: new_status,
            timestamp: Utc::now(),
        });
        self.touch();
        Some(old)
    }

    // -- Phase 4: editing, disable/enable, clear, readiness, undo --

    /// Update a single argument on an entry. Returns the old value.
    /// Does NOT regenerate the sentence — caller must do that and call
    /// `update_entry_sentence()` afterward.
    pub fn update_entry_arg(
        &mut self,
        entry_id: Uuid,
        field: &str,
        new_value: String,
    ) -> Option<String> {
        let entry = self.entries.iter_mut().find(|e| e.id == entry_id)?;
        let old = entry.args.insert(field.to_string(), new_value);
        self.touch();
        old
    }

    /// Update the sentence and DSL on an entry (after arg editing).
    /// Emits an `EntryArgChanged` audit event.
    pub fn update_entry_sentence(
        &mut self,
        entry_id: Uuid,
        new_sentence: String,
        new_dsl: String,
        old_sentence: &str,
        field: &str,
        old_value: Option<String>,
        new_value: &str,
    ) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == entry_id) {
            let sentence_after = new_sentence.clone();
            entry.sentence = new_sentence;
            entry.dsl = new_dsl;
            self.audit.push(RunbookEvent::EntryArgChanged {
                entry_id,
                field: field.to_string(),
                old_value,
                new_value: new_value.to_string(),
                sentence_before: old_sentence.to_string(),
                sentence_after,
                timestamp: Utc::now(),
            });
            self.touch();
        }
    }

    /// Disable a step (skip during execution). Returns true if the entry
    /// was found and was not already disabled.
    pub fn disable_entry(&mut self, entry_id: Uuid) -> bool {
        let entry = match self.entries.iter_mut().find(|e| e.id == entry_id) {
            Some(e) => e,
            None => return false,
        };
        if entry.status == EntryStatus::Disabled {
            return false;
        }
        let old = entry.status;
        entry.status = EntryStatus::Disabled;
        self.audit.push(RunbookEvent::EntryStatusChanged {
            entry_id,
            from: old,
            to: EntryStatus::Disabled,
            timestamp: Utc::now(),
        });
        self.audit.push(RunbookEvent::EntryDisabled {
            entry_id,
            timestamp: Utc::now(),
        });
        self.touch();
        true
    }

    /// Enable a previously disabled step (restores to Confirmed).
    /// Returns true if the entry was found and was disabled.
    pub fn enable_entry(&mut self, entry_id: Uuid) -> bool {
        let entry = match self.entries.iter_mut().find(|e| e.id == entry_id) {
            Some(e) => e,
            None => return false,
        };
        if entry.status != EntryStatus::Disabled {
            return false;
        }
        entry.status = EntryStatus::Confirmed;
        self.audit.push(RunbookEvent::EntryStatusChanged {
            entry_id,
            from: EntryStatus::Disabled,
            to: EntryStatus::Confirmed,
            timestamp: Utc::now(),
        });
        self.audit.push(RunbookEvent::EntryEnabled {
            entry_id,
            timestamp: Utc::now(),
        });
        self.touch();
        true
    }

    /// Toggle a step between Disabled and Confirmed.
    /// Returns the new status, or None if the entry was not found.
    pub fn toggle_entry(&mut self, entry_id: Uuid) -> Option<EntryStatus> {
        let is_disabled = self
            .entries
            .iter()
            .find(|e| e.id == entry_id)
            .map(|e| e.status == EntryStatus::Disabled)?;
        if is_disabled {
            self.enable_entry(entry_id);
            Some(EntryStatus::Confirmed)
        } else {
            self.disable_entry(entry_id);
            Some(EntryStatus::Disabled)
        }
    }

    /// Clear all entries from the runbook. Returns removed count.
    pub fn clear(&mut self) -> usize {
        let count = self.entries.len();
        self.entries.clear();
        self.audit.push(RunbookEvent::RunbookCleared {
            entry_count: count,
            timestamp: Utc::now(),
        });
        self.touch();
        count
    }

    /// Check execution readiness — returns a report with blocking issues.
    pub fn readiness(&self) -> ReadinessReport {
        let mut issues = Vec::new();

        let enabled: Vec<_> = self
            .entries
            .iter()
            .filter(|e| e.status != EntryStatus::Disabled)
            .collect();
        let disabled_count = self.entries.len() - enabled.len();

        if enabled.is_empty() {
            return ReadinessReport {
                ready: false,
                total_entries: self.entries.len(),
                enabled_entries: 0,
                disabled_entries: disabled_count,
                issues: vec![ReadinessIssue {
                    entry_id: Uuid::nil(),
                    sequence: 0,
                    issue: "No enabled entries in runbook".to_string(),
                }],
            };
        }

        for entry in &enabled {
            if entry.status == EntryStatus::Proposed {
                issues.push(ReadinessIssue {
                    entry_id: entry.id,
                    sequence: entry.sequence,
                    issue: "Entry not confirmed (still Proposed)".to_string(),
                });
            }
            if entry.status == EntryStatus::Failed {
                issues.push(ReadinessIssue {
                    entry_id: entry.id,
                    sequence: entry.sequence,
                    issue: "Entry failed — must be reset or disabled".to_string(),
                });
            }
            if !entry.unresolved_refs.is_empty() {
                issues.push(ReadinessIssue {
                    entry_id: entry.id,
                    sequence: entry.sequence,
                    issue: format!(
                        "{} unresolved entity reference(s)",
                        entry.unresolved_refs.len()
                    ),
                });
            }
        }

        ReadinessReport {
            ready: issues.is_empty(),
            total_entries: self.entries.len(),
            enabled_entries: enabled.len(),
            disabled_entries: disabled_count,
            issues,
        }
    }

    /// Push an entry onto the undo stack (for potential redo).
    pub fn push_undo_entry(&mut self, entry: RunbookEntry) {
        self.undo_stack.push(entry);
    }

    /// Pop an entry from the undo stack (for redo).
    pub fn pop_undo_entry(&mut self) -> Option<RunbookEntry> {
        self.undo_stack.pop()
    }

    // -- Phase 5: park / resume --

    /// Park an entry: set status to Parked, store invocation, index by
    /// correlation key, and emit an `EntryParked` event.
    ///
    /// Returns `true` if the entry was found and parked.
    pub fn park_entry(&mut self, entry_id: Uuid, invocation: InvocationRecord) -> bool {
        let entry = match self.entries.iter_mut().find(|e| e.id == entry_id) {
            Some(e) => e,
            None => return false,
        };
        let old_status = entry.status;
        entry.status = EntryStatus::Parked;
        entry.invocation = Some(invocation.clone());

        self.invocation_index
            .insert(invocation.correlation_key.clone(), entry_id);

        self.audit.push(RunbookEvent::EntryStatusChanged {
            entry_id,
            from: old_status,
            to: EntryStatus::Parked,
            timestamp: Utc::now(),
        });
        self.audit.push(RunbookEvent::EntryParked {
            entry_id,
            gate_type: invocation.gate_type,
            invocation_id: invocation.invocation_id,
            correlation_key: invocation.correlation_key.clone(),
            timestamp: Utc::now(),
        });

        if invocation.gate_type == GateType::HumanApproval {
            self.audit.push(RunbookEvent::HumanGateRequested {
                entry_id,
                invocation_id: invocation.invocation_id,
                approver_hint: None,
                timestamp: Utc::now(),
            });
        }

        self.touch();
        true
    }

    /// Resume a parked entry by correlation key.
    ///
    /// Sets the entry status to `Completed`, clears the invocation, removes
    /// it from the index, and emits an `EntryResumed` event.
    ///
    /// Returns the entry_id if found and resumed, `None` if the correlation
    /// key is unknown or the entry is not currently parked (idempotent).
    pub fn resume_entry(
        &mut self,
        correlation_key: &str,
        result: Option<serde_json::Value>,
    ) -> Option<Uuid> {
        let entry_id = self.invocation_index.remove(correlation_key)?;
        let entry = self.entries.iter_mut().find(|e| e.id == entry_id)?;

        // Idempotent: if already resumed, return None.
        if entry.status != EntryStatus::Parked {
            return None;
        }

        let invocation_id = entry
            .invocation
            .as_ref()
            .map(|inv| inv.invocation_id)
            .unwrap_or_else(Uuid::nil);

        entry.status = EntryStatus::Completed;
        entry.result = result.clone();
        if let Some(ref mut inv) = entry.invocation {
            inv.status = InvocationStatus::Completed;
            inv.resumed_at = Some(Utc::now());
        }

        self.audit.push(RunbookEvent::EntryStatusChanged {
            entry_id,
            from: EntryStatus::Parked,
            to: EntryStatus::Completed,
            timestamp: Utc::now(),
        });
        self.audit.push(RunbookEvent::EntryResumed {
            entry_id,
            invocation_id,
            result,
            timestamp: Utc::now(),
        });

        self.touch();
        Some(entry_id)
    }

    /// Cancel all parked entries. Returns the count of entries cancelled.
    pub fn cancel_parked_entries(&mut self) -> usize {
        let parked_ids: Vec<Uuid> = self
            .entries
            .iter()
            .filter(|e| e.status == EntryStatus::Parked)
            .map(|e| e.id)
            .collect();

        for entry_id in &parked_ids {
            if let Some(entry) = self.entries.iter_mut().find(|e| e.id == *entry_id) {
                entry.status = EntryStatus::Failed;
                if let Some(ref mut inv) = entry.invocation {
                    inv.status = InvocationStatus::Cancelled;
                    if let Some(key) = self.invocation_index.remove(&inv.correlation_key) {
                        let _ = key;
                    }
                }
                self.audit.push(RunbookEvent::EntryStatusChanged {
                    entry_id: *entry_id,
                    from: EntryStatus::Parked,
                    to: EntryStatus::Failed,
                    timestamp: Utc::now(),
                });
            }
        }

        if !parked_ids.is_empty() {
            self.touch();
        }
        parked_ids.len()
    }

    /// Rebuild the invocation_index from entries.
    ///
    /// Must be called after deserialization since `invocation_index` is
    /// `#[serde(skip)]`.
    pub fn rebuild_invocation_index(&mut self) {
        self.invocation_index.clear();
        for entry in &self.entries {
            if entry.status == EntryStatus::Parked {
                if let Some(ref inv) = entry.invocation {
                    if inv.status == InvocationStatus::Active {
                        self.invocation_index
                            .insert(inv.correlation_key.clone(), entry.id);
                    }
                }
            }
        }
    }

    // -- private helpers --

    /// Re-assign 1-based gapless sequence numbers.
    fn renumber(&mut self) {
        for (i, entry) in self.entries.iter_mut().enumerate() {
            entry.sequence = (i + 1) as i32;
        }
    }

    /// Bump `updated_at`.
    fn touch(&mut self) {
        self.updated_at = Utc::now();
    }
}

// ---------------------------------------------------------------------------
// RunbookEntry builder (convenience for tests & template instantiation)
// ---------------------------------------------------------------------------

impl RunbookEntry {
    /// Create a new entry with the minimum required fields.
    /// Sequence is set to 0 and will be corrected by `Runbook::add_entry()`.
    pub fn new(verb: String, sentence: String, dsl: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            sequence: 0,
            sentence,
            labels: HashMap::new(),
            dsl,
            verb,
            args: HashMap::new(),
            slot_provenance: SlotProvenance::default(),
            arg_extraction_audit: None,
            status: EntryStatus::Proposed,
            execution_mode: ExecutionMode::Sync,
            confirm_policy: ConfirmPolicy::Always,
            unresolved_refs: Vec::new(),
            depends_on: Vec::new(),
            result: None,
            invocation: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entry(verb: &str, sentence: &str) -> RunbookEntry {
        RunbookEntry::new(
            verb.to_string(),
            sentence.to_string(),
            format!("({verb} :placeholder true)"),
        )
    }

    #[test]
    fn test_new_runbook_is_draft_with_created_event() {
        let rb = Runbook::new(Uuid::new_v4());
        assert_eq!(rb.status, RunbookStatus::Draft);
        assert!(rb.entries.is_empty());
        assert_eq!(rb.audit.len(), 1);
        assert!(matches!(rb.audit[0], RunbookEvent::Created { .. }));
    }

    #[test]
    fn test_add_entry_assigns_sequence() {
        let mut rb = Runbook::new(Uuid::new_v4());
        let e1 = sample_entry("cbu.create", "Create Allianz Lux CBU");
        let e2 = sample_entry("cbu.assign-product", "Add IRS product to Allianz Lux");

        rb.add_entry(e1);
        rb.add_entry(e2);

        assert_eq!(rb.entries.len(), 2);
        assert_eq!(rb.entries[0].sequence, 1);
        assert_eq!(rb.entries[1].sequence, 2);
    }

    #[test]
    fn test_add_entry_emits_audit_event() {
        let mut rb = Runbook::new(Uuid::new_v4());
        let entry = sample_entry("cbu.create", "Create fund");
        rb.add_entry(entry);

        // Created + EntryAdded
        assert_eq!(rb.audit.len(), 2);
        assert!(matches!(rb.audit[1], RunbookEvent::EntryAdded { .. }));
    }

    #[test]
    fn test_remove_entry() {
        let mut rb = Runbook::new(Uuid::new_v4());
        let e1 = sample_entry("cbu.create", "Create fund");
        let e2 = sample_entry("cbu.assign-product", "Add product");
        let id1 = rb.add_entry(e1);
        let _id2 = rb.add_entry(e2);

        let removed = rb.remove_entry(id1);
        assert!(removed.is_some());
        assert_eq!(rb.entries.len(), 1);
        // After removal, remaining entry should be re-sequenced to 1
        assert_eq!(rb.entries[0].sequence, 1);
    }

    #[test]
    fn test_remove_nonexistent_returns_none() {
        let mut rb = Runbook::new(Uuid::new_v4());
        assert!(rb.remove_entry(Uuid::new_v4()).is_none());
    }

    #[test]
    fn test_reorder_entries() {
        let mut rb = Runbook::new(Uuid::new_v4());
        let e1 = sample_entry("a.first", "First");
        let e2 = sample_entry("b.second", "Second");
        let e3 = sample_entry("c.third", "Third");
        let id1 = rb.add_entry(e1);
        let id2 = rb.add_entry(e2);
        let id3 = rb.add_entry(e3);

        // Reverse order
        rb.reorder(&[id3, id1, id2]);

        assert_eq!(rb.entries[0].id, id3);
        assert_eq!(rb.entries[1].id, id1);
        assert_eq!(rb.entries[2].id, id2);
        // Sequences should be 1, 2, 3
        assert_eq!(rb.entries[0].sequence, 1);
        assert_eq!(rb.entries[1].sequence, 2);
        assert_eq!(rb.entries[2].sequence, 3);
    }

    #[test]
    fn test_reorder_partial_appends_unmentioned() {
        let mut rb = Runbook::new(Uuid::new_v4());
        let e1 = sample_entry("a.first", "First");
        let e2 = sample_entry("b.second", "Second");
        let e3 = sample_entry("c.third", "Third");
        let id1 = rb.add_entry(e1);
        let id2 = rb.add_entry(e2);
        let id3 = rb.add_entry(e3);

        // Only mention id3 — id1 and id2 should be appended in original order
        rb.reorder(&[id3]);

        assert_eq!(rb.entries[0].id, id3);
        assert_eq!(rb.entries[1].id, id1);
        assert_eq!(rb.entries[2].id, id2);
    }

    #[test]
    fn test_entry_by_id() {
        let mut rb = Runbook::new(Uuid::new_v4());
        let entry = sample_entry("cbu.create", "Create fund");
        let id = rb.add_entry(entry);

        assert!(rb.entry_by_id(id).is_some());
        assert_eq!(rb.entry_by_id(id).unwrap().verb, "cbu.create");
        assert!(rb.entry_by_id(Uuid::new_v4()).is_none());
    }

    #[test]
    fn test_entries_by_status() {
        let mut rb = Runbook::new(Uuid::new_v4());
        let e1 = sample_entry("a.first", "First");
        let e2 = sample_entry("b.second", "Second");
        let id1 = rb.add_entry(e1);
        rb.add_entry(e2);

        rb.set_entry_status(id1, EntryStatus::Confirmed);

        let proposed = rb.entries_by_status(EntryStatus::Proposed);
        assert_eq!(proposed.len(), 1);
        let confirmed = rb.entries_by_status(EntryStatus::Confirmed);
        assert_eq!(confirmed.len(), 1);
    }

    #[test]
    fn test_display_sentences() {
        let mut rb = Runbook::new(Uuid::new_v4());
        rb.add_entry(sample_entry("cbu.create", "Create Allianz Lux CBU"));
        rb.add_entry(sample_entry("cbu.assign-product", "Add IRS product"));

        let sentences = rb.display_sentences();
        assert_eq!(sentences.len(), 2);
        assert_eq!(sentences[0], "1. Create Allianz Lux CBU");
        assert_eq!(sentences[1], "2. Add IRS product");
    }

    #[test]
    fn test_set_status_emits_event() {
        let mut rb = Runbook::new(Uuid::new_v4());
        rb.set_status(RunbookStatus::Building);

        assert_eq!(rb.status, RunbookStatus::Building);
        let last = rb.audit.last().unwrap();
        match last {
            RunbookEvent::StatusChanged { from, to, .. } => {
                assert_eq!(*from, RunbookStatus::Draft);
                assert_eq!(*to, RunbookStatus::Building);
            }
            _ => panic!("Expected StatusChanged event"),
        }
    }

    #[test]
    fn test_set_entry_status_emits_event() {
        let mut rb = Runbook::new(Uuid::new_v4());
        let entry = sample_entry("cbu.create", "Create fund");
        let id = rb.add_entry(entry);

        let old = rb.set_entry_status(id, EntryStatus::Confirmed);
        assert_eq!(old, Some(EntryStatus::Proposed));

        let entry = rb.entry_by_id(id).unwrap();
        assert_eq!(entry.status, EntryStatus::Confirmed);
    }

    #[test]
    fn test_provenance_tracking() {
        let mut entry = sample_entry("cbu.create", "Create fund");
        entry
            .slot_provenance
            .slots
            .insert("name".to_string(), SlotSource::UserProvided);
        entry
            .slot_provenance
            .slots
            .insert("jurisdiction".to_string(), SlotSource::InferredFromContext);
        entry
            .slot_provenance
            .slots
            .insert("kind".to_string(), SlotSource::TemplateDefault);

        assert_eq!(entry.slot_provenance.slots.len(), 3);
        assert_eq!(
            entry.slot_provenance.slots.get("name"),
            Some(&SlotSource::UserProvided)
        );
        assert_eq!(
            entry.slot_provenance.slots.get("jurisdiction"),
            Some(&SlotSource::InferredFromContext)
        );
        assert_eq!(
            entry.slot_provenance.slots.get("kind"),
            Some(&SlotSource::TemplateDefault)
        );
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut rb = Runbook::new(Uuid::new_v4());
        let mut entry = sample_entry("cbu.create", "Create Allianz Lux CBU");
        entry
            .args
            .insert("name".to_string(), "Allianz Lux".to_string());
        entry
            .slot_provenance
            .slots
            .insert("name".to_string(), SlotSource::UserProvided);
        rb.add_entry(entry);
        rb.set_status(RunbookStatus::Building);

        let json = serde_json::to_string(&rb).expect("serialize");
        let deserialized: Runbook = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.id, rb.id);
        assert_eq!(deserialized.status, RunbookStatus::Building);
        assert_eq!(deserialized.entries.len(), 1);
        assert_eq!(deserialized.entries[0].verb, "cbu.create");
        assert_eq!(
            deserialized.entries[0].slot_provenance.slots.get("name"),
            Some(&SlotSource::UserProvided)
        );
    }

    #[test]
    fn test_entry_new_defaults() {
        let entry = RunbookEntry::new(
            "cbu.create".to_string(),
            "Create fund".to_string(),
            "(cbu.create :name \"test\")".to_string(),
        );
        assert_eq!(entry.status, EntryStatus::Proposed);
        assert_eq!(entry.execution_mode, ExecutionMode::Sync);
        assert_eq!(entry.confirm_policy, ConfirmPolicy::Always);
        assert!(entry.depends_on.is_empty());
        assert!(entry.unresolved_refs.is_empty());
        assert!(entry.result.is_none());
        assert!(entry.arg_extraction_audit.is_none());
    }

    #[test]
    fn test_multiple_status_transitions_tracked() {
        let mut rb = Runbook::new(Uuid::new_v4());
        rb.set_status(RunbookStatus::Building);
        rb.set_status(RunbookStatus::Ready);
        rb.set_status(RunbookStatus::Executing);
        rb.set_status(RunbookStatus::Completed);

        // Created + 4 status changes
        let status_events: Vec<_> = rb
            .audit
            .iter()
            .filter(|e| matches!(e, RunbookEvent::StatusChanged { .. }))
            .collect();
        assert_eq!(status_events.len(), 4);
    }

    #[test]
    fn test_remove_entry_emits_audit() {
        let mut rb = Runbook::new(Uuid::new_v4());
        let entry = sample_entry("cbu.create", "Create fund");
        let id = rb.add_entry(entry);
        rb.remove_entry(id);

        let remove_events: Vec<_> = rb
            .audit
            .iter()
            .filter(|e| matches!(e, RunbookEvent::EntryRemoved { .. }))
            .collect();
        assert_eq!(remove_events.len(), 1);
    }

    #[test]
    fn test_reorder_emits_audit() {
        let mut rb = Runbook::new(Uuid::new_v4());
        let e1 = sample_entry("a.first", "First");
        let e2 = sample_entry("b.second", "Second");
        let id1 = rb.add_entry(e1);
        let id2 = rb.add_entry(e2);

        rb.reorder(&[id2, id1]);

        let reorder_events: Vec<_> = rb
            .audit
            .iter()
            .filter(|e| matches!(e, RunbookEvent::EntriesReordered { .. }))
            .collect();
        assert_eq!(reorder_events.len(), 1);
    }

    // -- Phase 5: park / resume tests --

    #[test]
    fn test_park_entry_sets_parked_status_and_emits_events() {
        let mut rb = Runbook::new(Uuid::new_v4());
        let mut entry = sample_entry("doc.solicit", "Request passport");
        entry.status = EntryStatus::Confirmed;
        let entry_id = rb.add_entry(entry);

        let inv = InvocationRecord::new(
            entry_id,
            rb.id,
            rb.session_id,
            InvocationRecord::make_correlation_key(rb.id, entry_id),
            GateType::DurableTask,
        );

        assert!(rb.park_entry(entry_id, inv));

        let entry = rb.entry_by_id(entry_id).unwrap();
        assert_eq!(entry.status, EntryStatus::Parked);
        assert!(entry.invocation.is_some());

        // Check audit trail
        let parked_events: Vec<_> = rb
            .audit
            .iter()
            .filter(|e| matches!(e, RunbookEvent::EntryParked { .. }))
            .collect();
        assert_eq!(parked_events.len(), 1);

        // No HumanGateRequested for DurableTask
        let gate_events: Vec<_> = rb
            .audit
            .iter()
            .filter(|e| matches!(e, RunbookEvent::HumanGateRequested { .. }))
            .collect();
        assert_eq!(gate_events.len(), 0);
    }

    #[test]
    fn test_park_human_gate_emits_gate_requested() {
        let mut rb = Runbook::new(Uuid::new_v4());
        let mut entry = sample_entry("kyc.approve", "Approve KYC case");
        entry.status = EntryStatus::Confirmed;
        let entry_id = rb.add_entry(entry);

        let inv = InvocationRecord::new(
            entry_id,
            rb.id,
            rb.session_id,
            InvocationRecord::make_correlation_key(rb.id, entry_id),
            GateType::HumanApproval,
        );

        assert!(rb.park_entry(entry_id, inv));

        let gate_events: Vec<_> = rb
            .audit
            .iter()
            .filter(|e| matches!(e, RunbookEvent::HumanGateRequested { .. }))
            .collect();
        assert_eq!(gate_events.len(), 1);
    }

    #[test]
    fn test_park_nonexistent_entry_returns_false() {
        let mut rb = Runbook::new(Uuid::new_v4());
        let inv = InvocationRecord::new(
            Uuid::new_v4(),
            rb.id,
            rb.session_id,
            "fake:key".to_string(),
            GateType::DurableTask,
        );
        assert!(!rb.park_entry(Uuid::new_v4(), inv));
    }

    #[test]
    fn test_resume_entry_sets_completed_and_emits_events() {
        let mut rb = Runbook::new(Uuid::new_v4());
        let mut entry = sample_entry("doc.solicit", "Request passport");
        entry.status = EntryStatus::Confirmed;
        let entry_id = rb.add_entry(entry);

        let corr_key = InvocationRecord::make_correlation_key(rb.id, entry_id);
        let inv = InvocationRecord::new(
            entry_id,
            rb.id,
            rb.session_id,
            corr_key.clone(),
            GateType::DurableTask,
        );
        rb.park_entry(entry_id, inv);

        let result = serde_json::json!({"doc_id": "abc-123"});
        let resumed_id = rb.resume_entry(&corr_key, Some(result.clone()));
        assert_eq!(resumed_id, Some(entry_id));

        let entry = rb.entry_by_id(entry_id).unwrap();
        assert_eq!(entry.status, EntryStatus::Completed);
        assert_eq!(entry.result, Some(result));
        assert_eq!(
            entry.invocation.as_ref().unwrap().status,
            InvocationStatus::Completed
        );
        assert!(entry.invocation.as_ref().unwrap().resumed_at.is_some());

        // Check correlation key removed from index
        assert!(rb.invocation_index.is_empty());

        // Check audit trail
        let resumed_events: Vec<_> = rb
            .audit
            .iter()
            .filter(|e| matches!(e, RunbookEvent::EntryResumed { .. }))
            .collect();
        assert_eq!(resumed_events.len(), 1);
    }

    #[test]
    fn test_resume_unknown_correlation_key_returns_none() {
        let mut rb = Runbook::new(Uuid::new_v4());
        assert!(rb.resume_entry("nonexistent:key", None).is_none());
    }

    #[test]
    fn test_resume_is_idempotent() {
        let mut rb = Runbook::new(Uuid::new_v4());
        let mut entry = sample_entry("doc.solicit", "Request passport");
        entry.status = EntryStatus::Confirmed;
        let entry_id = rb.add_entry(entry);

        let corr_key = InvocationRecord::make_correlation_key(rb.id, entry_id);
        let inv = InvocationRecord::new(
            entry_id,
            rb.id,
            rb.session_id,
            corr_key.clone(),
            GateType::DurableTask,
        );
        rb.park_entry(entry_id, inv);

        // First resume succeeds
        assert!(rb.resume_entry(&corr_key, None).is_some());
        // Second resume is no-op (correlation key already removed from index)
        assert!(rb.resume_entry(&corr_key, None).is_none());
    }

    #[test]
    fn test_cancel_parked_entries() {
        let mut rb = Runbook::new(Uuid::new_v4());

        let mut e1 = sample_entry("a.first", "First");
        e1.status = EntryStatus::Confirmed;
        let id1 = rb.add_entry(e1);

        let mut e2 = sample_entry("b.second", "Second");
        e2.status = EntryStatus::Confirmed;
        let id2 = rb.add_entry(e2);

        // Park both
        let inv1 = InvocationRecord::new(
            id1,
            rb.id,
            rb.session_id,
            InvocationRecord::make_correlation_key(rb.id, id1),
            GateType::DurableTask,
        );
        let inv2 = InvocationRecord::new(
            id2,
            rb.id,
            rb.session_id,
            InvocationRecord::make_correlation_key(rb.id, id2),
            GateType::HumanApproval,
        );
        rb.park_entry(id1, inv1);
        rb.park_entry(id2, inv2);

        let cancelled = rb.cancel_parked_entries();
        assert_eq!(cancelled, 2);

        assert_eq!(rb.entry_by_id(id1).unwrap().status, EntryStatus::Failed);
        assert_eq!(rb.entry_by_id(id2).unwrap().status, EntryStatus::Failed);
        assert!(rb.invocation_index.is_empty());
    }

    #[test]
    fn test_rebuild_invocation_index() {
        let mut rb = Runbook::new(Uuid::new_v4());
        let mut entry = sample_entry("doc.solicit", "Request passport");
        entry.status = EntryStatus::Confirmed;
        let entry_id = rb.add_entry(entry);

        let corr_key = InvocationRecord::make_correlation_key(rb.id, entry_id);
        let inv = InvocationRecord::new(
            entry_id,
            rb.id,
            rb.session_id,
            corr_key.clone(),
            GateType::DurableTask,
        );
        rb.park_entry(entry_id, inv);

        // Simulate deserialization: clear the index
        rb.invocation_index.clear();
        assert!(rb.invocation_index.is_empty());

        // Rebuild should restore it
        rb.rebuild_invocation_index();
        assert_eq!(rb.invocation_index.get(&corr_key), Some(&entry_id));
    }

    #[test]
    fn test_invocation_record_serialization_roundtrip() {
        let inv = InvocationRecord::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            "test:correlation:key".to_string(),
            GateType::HumanApproval,
        );

        let json = serde_json::to_string(&inv).unwrap();
        let deserialized: InvocationRecord = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.invocation_id, inv.invocation_id);
        assert_eq!(deserialized.correlation_key, inv.correlation_key);
        assert_eq!(deserialized.gate_type, GateType::HumanApproval);
        assert_eq!(deserialized.status, InvocationStatus::Active);
    }

    #[test]
    fn test_invocation_make_correlation_key() {
        let rb_id = Uuid::new_v4();
        let entry_id = Uuid::new_v4();
        let key = InvocationRecord::make_correlation_key(rb_id, entry_id);
        assert_eq!(key, format!("{}:{}", rb_id, entry_id));
    }
}
