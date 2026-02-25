//! Stewardship guardrails — role constraints, proof chain integrity, stale draft detection.
//!
//! These are pure core functions with zero DB dependencies.
//! Called by `changeset_gate_preview` and `promote_changeset` in CoreService.

use uuid::Uuid;

use crate::{
    error::{GateSeverity, GateViolation, SemOsError},
    ports::SnapshotStore,
    principal::Principal,
    types::{ChangeKind, ChangesetEntry, Fqn, GovernanceTier, TrustClass},
};

// ── Role constraints ──────────────────────────────────────────

/// Allowed change kinds per role.
/// - `admin`: all operations
/// - `steward`: Add and Modify (no Remove of governed objects)
/// - `viewer` / other: none
fn allowed_change_kinds(role: &str, _entry: &ChangesetEntry) -> Vec<ChangeKind> {
    match role {
        "admin" => vec![ChangeKind::Add, ChangeKind::Modify, ChangeKind::Remove],
        "steward" => vec![ChangeKind::Add, ChangeKind::Modify],
        _ => vec![],
    }
}

/// Assert that the actor's roles permit every change_kind present in the changeset entries.
///
/// An actor only needs ONE role that permits the operation.
/// Returns `Err(SemOsError::Unauthorized)` if any entry's change_kind is not permitted.
pub fn validate_role_constraints(
    principal: &Principal,
    entries: &[ChangesetEntry],
) -> Result<(), SemOsError> {
    for entry in entries {
        let permitted = principal.roles.iter().any(|role| {
            let allowed = allowed_change_kinds(role, entry);
            allowed.contains(&entry.change_kind)
        });

        if !permitted {
            return Err(SemOsError::Unauthorized(format!(
                "Actor '{}' with roles {:?} may not perform '{}' on '{}'",
                principal.actor_id,
                principal.roles,
                entry.change_kind.as_str(),
                entry.object_fqn,
            )));
        }
    }

    Ok(())
}

// ── Proof chain compatibility ─────────────────────────────────

/// Validates that draft entries do not break existing proof chains.
///
/// A proof chain is broken when:
/// 1. A draft modifies or removes a `Governed`+`Proof` object
/// 2. Other active `Governed`+`Proof` objects reference it (via predecessor chain or FQN)
///
/// For now this performs a structural check: governed objects being removed
/// or downgraded trigger an error. Full transitive graph traversal is deferred.
pub async fn check_proof_chain_compatibility(
    entries: &[ChangesetEntry],
    snapshot_store: &dyn SnapshotStore,
) -> Result<(), SemOsError> {
    for entry in entries {
        // Only check Remove and Modify on existing objects
        if entry.change_kind == ChangeKind::Add {
            continue;
        }

        // Must have a base_snapshot_id for modification/removal
        let base_id = match entry.base_snapshot_id {
            Some(id) => id,
            None => continue,
        };

        // Resolve the current active snapshot for this FQN
        let fqn = Fqn::new(&entry.object_fqn);
        let current = match snapshot_store.resolve(&fqn, None).await {
            Ok(row) => row,
            Err(_) => continue, // Object no longer active — nothing to break
        };

        // Check if the current snapshot is governed + proof
        if current.governance_tier == GovernanceTier::Governed
            && current.trust_class == TrustClass::Proof
        {
            if entry.change_kind == ChangeKind::Remove {
                return Err(SemOsError::GateFailed(vec![GateViolation {
                    gate_id: "proof_chain_compatibility".into(),
                    severity: GateSeverity::Error,
                    message: format!(
                        "Cannot remove governed/proof object '{}' (snapshot {}) — \
                         proof chain integrity would be broken",
                        entry.object_fqn, base_id,
                    ),
                    remediation: Some("Deprecate the object instead of removing it".into()),
                }]));
            }

            // For Modify: check if the draft payload downgrades governance_tier or trust_class
            if entry.change_kind == ChangeKind::Modify {
                if let Some(draft_tier) = entry
                    .draft_payload
                    .get("governance_tier")
                    .and_then(|v| v.as_str())
                {
                    if draft_tier == "operational" {
                        return Err(SemOsError::GateFailed(vec![GateViolation {
                            gate_id: "proof_chain_compatibility".into(),
                            severity: GateSeverity::Error,
                            message: format!(
                                "Cannot downgrade governed/proof object '{}' to operational tier — \
                                 proof chain integrity would be broken",
                                entry.object_fqn,
                            ),
                            remediation: Some(
                                "Keep governance_tier as 'governed' for proof objects".into(),
                            ),
                        }]));
                    }
                }

                if let Some(draft_trust) = entry
                    .draft_payload
                    .get("trust_class")
                    .and_then(|v| v.as_str())
                {
                    if draft_trust != "proof" {
                        return Err(SemOsError::GateFailed(vec![GateViolation {
                            gate_id: "proof_chain_compatibility".into(),
                            severity: GateSeverity::Error,
                            message: format!(
                                "Cannot downgrade trust_class of governed/proof object '{}' \
                                 from proof to {} — proof chain integrity would be broken",
                                entry.object_fqn, draft_trust,
                            ),
                            remediation: Some(
                                "Keep trust_class as 'proof' for governed proof objects".into(),
                            ),
                        }]));
                    }
                }
            }
        }
    }

    Ok(())
}

// ── Stale draft detection ─────────────────────────────────────

/// A conflict where a changeset entry's base_snapshot_id no longer matches
/// the current active snapshot for that FQN (someone else published in the meantime).
#[derive(Debug, Clone)]
pub struct StaleDraftConflict {
    pub entry_id: Uuid,
    pub object_fqn: String,
    pub base_snapshot_id: Uuid,
    pub current_snapshot_id: Uuid,
}

/// Detect all entries where the `base_snapshot_id` recorded on the changeset entry
/// no longer matches the current active snapshot for that FQN.
///
/// Returns an empty vec if all entries are fresh.
pub async fn detect_stale_drafts(
    entries: &[ChangesetEntry],
    snapshot_store: &dyn SnapshotStore,
) -> Result<Vec<StaleDraftConflict>, SemOsError> {
    let mut conflicts = Vec::new();

    for entry in entries {
        let base_id = match entry.base_snapshot_id {
            Some(id) => id,
            None => continue, // New additions have no base — can't be stale
        };

        let fqn = Fqn::new(&entry.object_fqn);
        match snapshot_store.resolve(&fqn, None).await {
            Ok(current) => {
                if current.snapshot_id != base_id {
                    conflicts.push(StaleDraftConflict {
                        entry_id: entry.entry_id,
                        object_fqn: entry.object_fqn.clone(),
                        base_snapshot_id: base_id,
                        current_snapshot_id: current.snapshot_id,
                    });
                }
            }
            Err(_) => {
                // Object no longer has an active snapshot — the base was retired or removed.
                // This is also a conflict: the draft expected to modify something that no longer exists.
                conflicts.push(StaleDraftConflict {
                    entry_id: entry.entry_id,
                    object_fqn: entry.object_fqn.clone(),
                    base_snapshot_id: base_id,
                    current_snapshot_id: uuid::Uuid::nil(),
                });
            }
        }
    }

    Ok(conflicts)
}
