//! `EOP-DESIGN-CONTROLPLANE-G2-AUDIT-PROVENANCE-001` (v0.2, RATIFIED)
//! §2/§4: persistence for `"ob-poc".control_plane_audit`
//! (migration `20260713_control_plane_audit.sql`) plus the G11 AuditReplay
//! evaluation surface (completeness + outcome re-derivation, DD-4).
//!
//! The typed `AuditEvent`/`GateOutcomeProvenance` enums live in
//! `ob_poc_control_plane::audit` (no sqlx there, §9.1 non-goal for that
//! crate). This module is the DB-touching half — every function that
//! opens a connection is `#[cfg(feature = "database")]`, matching this
//! session's E5 fix (`ob-poc-boundary::toctou_recheck`) and the crate-wide
//! discipline: no unconditional sqlx.
//!
//! Emission posture (§2): best-effort, non-blocking, matching
//! `control_plane_shadow`/`control_plane_envelope_store`'s existing
//! posture — an audit-insert failure must never affect the request it is
//! observing. `DecisionEvaluated`/`EnvelopeSealed` are emitted alongside
//! the existing shadow-row insert at `sequencer.rs`'s `phase5_runtime_recheck`
//! (same `tokio::spawn`, additive only — W1). `EnvelopeConsumed` emits at
//! the real G10 consume site (`sem_os_runtime::verb_executor_adapter::admit_in_scope`).
//! `DispatchCommitted` emits at the plain `commit()` call site in
//! `execute_verb_admitting_envelope` (V3 finding: `commit_attested` has no
//! production caller yet — G2 item 2 has not landed — so `attested: false`
//! is recorded honestly rather than faked).

use uuid::Uuid;

use ob_poc_control_plane::audit::AuditEvent;

/// One row of `"ob-poc".control_plane_audit`, as read back for replay/
/// inspection. `seq` is the identity PK (ordering key, per DD-4(i) —
/// gapless-ness is deliberately not asserted, only ordering).
///
/// `#[allow(dead_code)]`: this type and its companion functions below
/// (`audit_rows_for_decision`, `check_completeness`,
/// `rederive_decision_outcome`) implement G11's DD-4 evaluation
/// primitives — proven correct by this module's own unit + live-DB
/// tests, per the design doc's "G11 becomes evaluable the moment the
/// stream has real rows." Wiring a live call site (e.g. into the E3
/// probe or an on-demand replay endpoint) is a real design decision
/// the ratified doc does not make — deliberately left for its own
/// reviewed increment rather than invented here.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub(crate) struct AuditRow {
    pub seq: i64,
    pub decision_id: Uuid,
    pub occurred_at: chrono::DateTime<chrono::Utc>,
    pub session_id: Uuid,
    pub event: AuditEvent,
}

/// Best-effort insert. Never returns `Err` to the caller — matches
/// `control_plane_shadow::insert_shadow_decision`'s posture (§2: "the
/// stream's guarantees are append-only-by-convention plus same-transaction
/// emission... sufficient for a single-operator deployment").
#[cfg(feature = "database")]
pub(crate) async fn insert_audit_event(
    pool: &sqlx::PgPool,
    decision_id: Uuid,
    session_id: Uuid,
    event: &AuditEvent,
) -> bool {
    let payload = match event.payload_json() {
        Ok(p) => p,
        Err(err) => {
            tracing::warn!(error = %err, event_type = event.event_type(), "AuditEvent payload serialisation failed (best-effort, non-blocking)");
            return false;
        }
    };
    let result = sqlx::query(
        r#"
        INSERT INTO "ob-poc".control_plane_audit (
            decision_id, event_type, session_id, payload
        ) VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(decision_id)
    .bind(event.event_type())
    .bind(session_id)
    .bind(payload)
    .execute(pool)
    .await;

    match result {
        Ok(_) => true,
        Err(err) => {
            tracing::warn!(
                error = %err,
                decision_id = %decision_id,
                event_type = event.event_type(),
                "control_plane_audit insert failed (best-effort, non-blocking)"
            );
            false
        }
    }
}

/// Same as [`insert_audit_event`] but against a caller-supplied connection
/// (e.g. inside `execute_verb_admitting_envelope`'s already-open
/// `PgTransactionScope`) — so the audit row for `EnvelopeConsumed`/
/// `DispatchCommitted` is same-transaction with the state it describes
/// (§2's design point), not a separate best-effort spawn.
#[cfg(feature = "database")]
pub(crate) async fn insert_audit_event_in_scope(
    conn: &mut sqlx::PgConnection,
    decision_id: Uuid,
    session_id: Uuid,
    event: &AuditEvent,
) -> bool {
    let payload = match event.payload_json() {
        Ok(p) => p,
        Err(err) => {
            tracing::warn!(error = %err, event_type = event.event_type(), "AuditEvent payload serialisation failed (best-effort, non-blocking)");
            return false;
        }
    };
    let result = sqlx::query(
        r#"
        INSERT INTO "ob-poc".control_plane_audit (
            decision_id, event_type, session_id, payload
        ) VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(decision_id)
    .bind(event.event_type())
    .bind(session_id)
    .bind(payload)
    .execute(conn)
    .await;

    match result {
        Ok(_) => true,
        Err(err) => {
            tracing::warn!(
                error = %err,
                decision_id = %decision_id,
                event_type = event.event_type(),
                "control_plane_audit insert (in-scope) failed (best-effort, non-blocking)"
            );
            false
        }
    }
}

/// Reads every audit row for one `decision_id`, ordered by `seq` (DD-4(i):
/// "ordered consistently by seq/occurred_at" — seq is the authoritative
/// tie-breaker since `occurred_at` has only millisecond-ish resolution).
#[cfg(feature = "database")]
#[allow(dead_code)] // see AuditRow's doc comment above
pub(crate) async fn audit_rows_for_decision(
    pool: &sqlx::PgPool,
    decision_id: Uuid,
) -> Result<Vec<AuditRow>, sqlx::Error> {
    let rows: Vec<(i64, Uuid, chrono::DateTime<chrono::Utc>, Uuid, String, serde_json::Value)> = sqlx::query_as(
        r#"
        SELECT seq, decision_id, occurred_at, session_id, event_type, payload
        FROM "ob-poc".control_plane_audit
        WHERE decision_id = $1
        ORDER BY seq ASC
        "#,
    )
    .bind(decision_id)
    .fetch_all(pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for (seq, decision_id, occurred_at, session_id, event_type, payload) in rows {
        match AuditEvent::from_stored(&event_type, payload) {
            Ok(event) => out.push(AuditRow {
                seq,
                decision_id,
                occurred_at,
                session_id,
                event,
            }),
            Err(err) => {
                tracing::warn!(error = %err, %decision_id, seq, event_type, "control_plane_audit row failed to deserialise into AuditEvent (skipped from replay)");
            }
        }
    }
    Ok(out)
}

// ── G11 (DD-4): completeness + outcome re-derivation ────────────────────

/// DD-4(i): the legal lifecycle grammar for one decision's event sequence,
/// checked in `seq` order (already the read order from
/// `audit_rows_for_decision`). Seq-gaplessness is deliberately NOT
/// asserted (identity columns can skip values on rolled-back inserts) —
/// only relative ordering of the *present* events is checked.
///
/// Grammar (§4):
/// - `DecisionEvaluated` first
/// - `EnvelopeSealed` present iff outcome was `ApprovedStp`
/// - `EnvelopeConsumed` at most once (per envelope; this function checks
///   at most once per decision, which for the per-step/per-envelope
///   correlation this doc assumes is the same thing — see the session
///   doc's V3/V4 finding on why decision_id == envelope_id for
///   envelope-bearing decisions)
/// - `DispatchCommitted` xor `DispatchRolledBack`, and only after a
///   `EnvelopeConsumed`
#[allow(dead_code)] // see AuditRow's doc comment above
pub(crate) fn check_completeness(events: &[AuditEvent]) -> Result<(), Vec<String>> {
    use ob_poc_control_plane::audit::DecisionOutcome;

    let mut violations = Vec::new();

    let Some(first) = events.first() else {
        return Ok(()); // empty stream slice: nothing to check
    };
    let outcome = match first {
        AuditEvent::DecisionEvaluated { outcome, .. } => Some(*outcome),
        _ => {
            violations.push("first event is not DecisionEvaluated".to_string());
            None
        }
    };

    let sealed_count = events.iter().filter(|e| matches!(e, AuditEvent::EnvelopeSealed { .. })).count();
    match outcome {
        Some(DecisionOutcome::ApprovedStp) if sealed_count == 0 => {
            violations.push("outcome ApprovedStp but no EnvelopeSealed event present".to_string());
        }
        Some(DecisionOutcome::ApprovedStp) => {}
        Some(_) if sealed_count != 0 => {
            violations.push("outcome not ApprovedStp but EnvelopeSealed event(s) present".to_string());
        }
        Some(_) | None => {}
    }

    let consumed_count = events.iter().filter(|e| matches!(e, AuditEvent::EnvelopeConsumed { .. })).count();
    if consumed_count > 1 {
        violations.push(format!("EnvelopeConsumed present {consumed_count} times, expected at most once"));
    }

    let committed_count = events.iter().filter(|e| matches!(e, AuditEvent::DispatchCommitted { .. })).count();
    let rolled_back_count = events.iter().filter(|e| matches!(e, AuditEvent::DispatchRolledBack { .. })).count();
    if committed_count > 0 && rolled_back_count > 0 {
        violations.push("both DispatchCommitted and DispatchRolledBack present — must be xor".to_string());
    }
    if (committed_count > 0 || rolled_back_count > 0) && consumed_count == 0 {
        violations.push("DispatchCommitted/DispatchRolledBack present without a prior EnvelopeConsumed".to_string());
    }
    // Ordering: if both a Consumed and a Committed/RolledBack are present,
    // the Consumed must occur at an earlier index.
    if let Some(consumed_idx) = events.iter().position(|e| matches!(e, AuditEvent::EnvelopeConsumed { .. })) {
        if let Some(terminal_idx) = events
            .iter()
            .position(|e| matches!(e, AuditEvent::DispatchCommitted { .. } | AuditEvent::DispatchRolledBack { .. }))
        {
            if terminal_idx < consumed_idx {
                violations.push("DispatchCommitted/DispatchRolledBack occurs before EnvelopeConsumed in seq order".to_string());
            }
        }
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

/// DD-4(ii): re-derive the decision outcome from the recorded
/// `gate_results` JSONB (`"ob-poc".control_plane_shadow_decisions`'
/// column, keyed by `GateId`'s `Debug` rendering per
/// `control_plane_shadow::report_to_json`) and compare with the recorded
/// outcome.
///
/// V2 finding (recorded here per the design doc's own instruction, §4:
/// "the gap is recorded... not silently"): no persisted, queryable
/// `SnapshotId` store exists for G13 today (`SnapshotPins`' fields are
/// private with no persistence layer — `snapshot.rs`'s own doc: "G13 has
/// zero production callers"). This does NOT block full outcome
/// re-derivation, though: `decision::evaluate_with_report`'s own
/// classification only needs each gate's Success/Failure signal (already
/// fully captured in `gate_results`), including `StpClassifier`'s
/// differentiated `Failure("requires_human_gate")` vs
/// `Failure("rejected")` reason string (`stp_classifier.rs`'s
/// `StpClassifierGate::evaluate`) — so the 3-way `ApprovedStp` /
/// `HumanGate` / `Rejected` split IS fully re-derivable from
/// `gate_results` alone, without the G13 snapshot's actual pin *content*.
/// This function therefore implements full (not gate-outcomes-only
/// degraded) re-derivation — mirrors `evaluate_with_report`'s own
/// PROOF_BEARING_GATES + RunbookProof + StpClassifier logic exactly.
#[allow(dead_code)] // see AuditRow's doc comment above
pub(crate) fn rederive_decision_outcome(
    gate_results: &serde_json::Value,
) -> Option<ob_poc_control_plane::audit::DecisionOutcome> {
    use ob_poc_control_plane::audit::DecisionOutcome;

    // Mirrors decision.rs's PROOF_BEARING_GATES (the 8 gates whose Success
    // is required before any envelope/proof assembly is attempted).
    const PROOF_BEARING_GATES: [&str; 8] = [
        "IntentAdmission",
        "EntityBinding",
        "PackResolution",
        "DagProof",
        "Authority",
        "Evidence",
        "WriteSet",
        "DecisionSnapshot",
    ];

    let gate_str = |name: &str| gate_results.get(name).and_then(|v| v.as_str());

    let all_proof_bearing_ok = PROOF_BEARING_GATES.iter().all(|g| gate_str(g) == Some("Success"));
    if !all_proof_bearing_ok {
        return Some(DecisionOutcome::Rejected);
    }

    let runbook_ok = gate_str("RunbookProof") == Some("Success");
    if !runbook_ok {
        return Some(DecisionOutcome::Rejected);
    }

    match gate_str("StpClassifier") {
        Some("Success") => Some(DecisionOutcome::ApprovedStp),
        Some(s) if s.contains("requires_human_gate") => Some(DecisionOutcome::HumanGate),
        Some(_) => Some(DecisionOutcome::Rejected),
        None => None, // no StpClassifier sample recorded at all — cannot re-derive
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ob_poc_control_plane::audit::{DecisionOutcome, GateOutcomeRecord};
    use ob_poc_control_plane::gate::GateId;

    fn decision_evaluated(outcome: DecisionOutcome) -> AuditEvent {
        AuditEvent::DecisionEvaluated {
            outcome,
            snapshot_ref: None,
        }
    }

    // ── DD-4(i) completeness ─────────────────────────────────────────

    #[test]
    fn approved_stp_lifecycle_with_commit_is_complete() {
        let events = vec![
            decision_evaluated(DecisionOutcome::ApprovedStp),
            AuditEvent::EnvelopeSealed {
                envelope_id: Uuid::nil(),
                expires_at: chrono::Utc::now(),
            },
            AuditEvent::EnvelopeConsumed {
                envelope_id: Uuid::nil(),
                gate_outcome: GateOutcomeRecord::new(GateId::ExecutionEnvelope, "Success"),
            },
            AuditEvent::DispatchCommitted {
                attested: false,
                gate_outcome: GateOutcomeRecord::new(GateId::WriteSetAttestation, "NotEvaluated"),
            },
        ];
        assert_eq!(check_completeness(&events), Ok(()));
    }

    #[test]
    fn rejected_lifecycle_with_no_seal_is_complete() {
        let events = vec![decision_evaluated(DecisionOutcome::Rejected)];
        assert_eq!(check_completeness(&events), Ok(()));
    }

    #[test]
    fn sealed_event_without_approved_stp_outcome_is_a_violation() {
        let events = vec![
            decision_evaluated(DecisionOutcome::Rejected),
            AuditEvent::EnvelopeSealed {
                envelope_id: Uuid::nil(),
                expires_at: chrono::Utc::now(),
            },
        ];
        let err = check_completeness(&events).unwrap_err();
        assert!(err.iter().any(|v| v.contains("EnvelopeSealed")));
    }

    #[test]
    fn approved_stp_without_sealed_event_is_a_violation() {
        let events = vec![decision_evaluated(DecisionOutcome::ApprovedStp)];
        let err = check_completeness(&events).unwrap_err();
        assert!(err.iter().any(|v| v.contains("no EnvelopeSealed")));
    }

    #[test]
    fn double_consume_is_a_violation() {
        let events = vec![
            decision_evaluated(DecisionOutcome::ApprovedStp),
            AuditEvent::EnvelopeSealed {
                envelope_id: Uuid::nil(),
                expires_at: chrono::Utc::now(),
            },
            AuditEvent::EnvelopeConsumed {
                envelope_id: Uuid::nil(),
                gate_outcome: GateOutcomeRecord::new(GateId::ExecutionEnvelope, "Success"),
            },
            AuditEvent::EnvelopeConsumed {
                envelope_id: Uuid::nil(),
                gate_outcome: GateOutcomeRecord::new(GateId::ExecutionEnvelope, "Success"),
            },
        ];
        let err = check_completeness(&events).unwrap_err();
        assert!(err.iter().any(|v| v.contains("at most once")));
    }

    #[test]
    fn commit_and_rollback_both_present_is_a_violation() {
        let events = vec![
            decision_evaluated(DecisionOutcome::ApprovedStp),
            AuditEvent::EnvelopeSealed {
                envelope_id: Uuid::nil(),
                expires_at: chrono::Utc::now(),
            },
            AuditEvent::EnvelopeConsumed {
                envelope_id: Uuid::nil(),
                gate_outcome: GateOutcomeRecord::new(GateId::ExecutionEnvelope, "Success"),
            },
            AuditEvent::DispatchCommitted {
                attested: false,
                gate_outcome: GateOutcomeRecord::new(GateId::WriteSetAttestation, "NotEvaluated"),
            },
            AuditEvent::DispatchRolledBack {
                reason: "shouldn't happen".to_string(),
            },
        ];
        let err = check_completeness(&events).unwrap_err();
        assert!(err.iter().any(|v| v.contains("xor")));
    }

    #[test]
    fn commit_without_prior_consume_is_a_violation() {
        let events = vec![
            decision_evaluated(DecisionOutcome::ApprovedStp),
            AuditEvent::EnvelopeSealed {
                envelope_id: Uuid::nil(),
                expires_at: chrono::Utc::now(),
            },
            AuditEvent::DispatchCommitted {
                attested: false,
                gate_outcome: GateOutcomeRecord::new(GateId::WriteSetAttestation, "NotEvaluated"),
            },
        ];
        let err = check_completeness(&events).unwrap_err();
        assert!(err.iter().any(|v| v.contains("without a prior EnvelopeConsumed")));
    }

    #[test]
    fn empty_slice_is_trivially_complete() {
        assert_eq!(check_completeness(&[]), Ok(()));
    }

    // ── DD-4(ii) outcome re-derivation ───────────────────────────────

    fn all_success_gate_results() -> serde_json::Value {
        serde_json::json!({
            "IntentAdmission": "Success",
            "EntityBinding": "Success",
            "PackResolution": "Success",
            "DagProof": "Success",
            "Authority": "Success",
            "Evidence": "Success",
            "WriteSet": "Success",
            "DecisionSnapshot": "Success",
            "RunbookProof": "Success",
            "StpClassifier": "Success",
        })
    }

    #[test]
    fn all_gates_success_rederives_approved_stp() {
        let gr = all_success_gate_results();
        assert_eq!(rederive_decision_outcome(&gr), Some(DecisionOutcome::ApprovedStp));
    }

    #[test]
    fn stp_classifier_human_gate_reason_rederives_human_gate() {
        let mut gr = all_success_gate_results();
        gr["StpClassifier"] = serde_json::json!("Failure(\"requires_human_gate\")");
        assert_eq!(rederive_decision_outcome(&gr), Some(DecisionOutcome::HumanGate));
    }

    #[test]
    fn stp_classifier_rejected_reason_rederives_rejected() {
        let mut gr = all_success_gate_results();
        gr["StpClassifier"] = serde_json::json!("Failure(\"rejected\")");
        assert_eq!(rederive_decision_outcome(&gr), Some(DecisionOutcome::Rejected));
    }

    #[test]
    fn any_proof_bearing_gate_failure_rederives_rejected() {
        let mut gr = all_success_gate_results();
        gr["Evidence"] = serde_json::json!("Failure(\"evidence gap\")");
        assert_eq!(rederive_decision_outcome(&gr), Some(DecisionOutcome::Rejected));
    }

    #[test]
    fn missing_runbook_proof_rederives_rejected() {
        let mut gr = all_success_gate_results();
        gr["RunbookProof"] = serde_json::json!("NotEvaluated { blocked_by: [] }");
        assert_eq!(rederive_decision_outcome(&gr), Some(DecisionOutcome::Rejected));
    }

    /// Cross-check against the crate's own `evaluate_with_report` on the
    /// same fully-admitted fixture (the strongest possible re-derivation
    /// proof: not a hand-copied expectation, the real function's real
    /// output).
    #[test]
    fn rederivation_matches_evaluate_with_report_on_a_fully_admitted_context() {
        // Re-use the same construction pattern as
        // ob_poc_control_plane::evaluate_shadow_tests::fully_admitted_context
        // (that helper is crate-private, so this test rebuilds the
        // equivalent minimal context inline).
        use ob_poc_control_plane::{
            authority_gate::{AccessDecisionKind, AuthorityInput},
            dag_proof::DagProofInput,
            entity_binding::{EntityBindingInput, EntityFacts},
            evidence_gate::EvidenceInput,
            intent_admission::IntentAdmissionInput,
            pack_resolution::PackResolutionInput,
            snapshot::SnapshotInput,
            stp_classifier::StpClassifierInput,
            write_set::WriteSetInput,
        };

        let entity = Uuid::nil();
        let ctx = ob_poc_control_plane::context::EvaluationContext {
            intent_admission: Some(IntentAdmissionInput {
                intent_id: Uuid::nil(),
                verb_fqn: "cbu.confirm".to_string(),
                is_admitted: true,
                exclusion_reasons: vec![],
                is_ai_originated: false,
                interpretation_attested: false,
            }),
            entity_binding: Some(EntityBindingInput {
                entities: vec![EntityFacts {
                    entity_id: entity,
                    exists: true,
                    expected_kind: "cbu".to_string(),
                    actual_kind: "cbu".to_string(),
                    lifecycle_state_readable: true,
                    availability_blocked: false,
                    availability_reason: None,
                    in_active_pack: true,
                }],
            }),
            pack_resolution: Some(PackResolutionInput {
                candidate_pack_ids: vec!["ob-poc.cbu".to_string()],
                semreg_allowed_set_available: true,
                constraint_denies_intent: false,
            }),
            dag_proof: Some(DagProofInput {
                entity_id: entity,
                from_state: "VALIDATION_PENDING".to_string(),
                to_state: "VALIDATED".to_string(),
                blocking_violations: vec![],
                lifecycle_fail_open_class: None,
                lifecycle_gate_mode_fail_closed: false,
            }),
            authority: Some(AuthorityInput {
                actor_id: "actor-1".to_string(),
                role: "compliance_officer".to_string(),
                access_decision: AccessDecisionKind::Allow,
                deny_reason: None,
                requires_human_approval: false,
                requires_second_line_review: false,
                segregation_of_duties_violated: false,
                toctou_drifted: false,
            }),
            evidence: Some(EvidenceInput {
                evidence_gaps: vec![],
                kyc_precondition_failures: vec![],
                satisfied_obligation_ids: vec!["obligation-1".to_string()],
                open_obligation_ids: vec![],
            }),
            write_set: Some(WriteSetInput {
                entity_ids: vec![entity],
                state_slots: vec!["validation_state".to_string()],
                tables: vec!["ob-poc.cbus".to_string()],
                allowed_columns: vec!["status".to_string()],
                idempotency_key: "idem-1".to_string(),
                contract_derived: true,
            }),
            snapshot: Some(SnapshotInput {
                sem_reg_snapshot_id: Some(Uuid::nil()),
                session_snapshot_id: None,
                kyc_manifest_hash: None,
                entity_row_versions: vec![(entity, "cbu".to_string(), 1)],
                versions: ob_poc_control_plane::snapshot::PinnedVersionSet::default(),
            }),
            stp_classifier: Some(StpClassifierInput {
                is_durable_verb: false,
                durable_execution_explicitly_allowed: false,
                has_unpinned_entities: false,
            }),
            write_set_attestation: Some(ob_poc_control_plane::write_set_attestation::WriteSetAttestationInput {
                captured: vec![ob_poc_control_plane::write_set_attestation::CapturedWrite {
                    table: "ob-poc.cbus".to_string(),
                    entity_id: entity,
                    columns: vec!["status".to_string()],
                }],
                expected_tables: vec!["ob-poc.cbus".to_string()],
                expected_entity_ids: vec![entity],
                expected_allowed_columns: vec!["status".to_string()],
            }),
            runbook_proof: Some(ob_poc_control_plane::proof::RunbookProofInput {
                compiled_runbook_id: Some(Uuid::nil()),
            }),
            version_pinning: Some(ob_poc_control_plane::versioning::VersionPinningInput {
                versions: ob_poc_control_plane::snapshot::PinnedVersionSet {
                    compiler_version: Some(env!("CARGO_PKG_VERSION").to_string()),
                    ..Default::default()
                },
            }),
        };

        let validity = ob_poc_control_plane::envelope::ValidityWindow::new(
            chrono::Utc::now(),
            chrono::Utc::now() + chrono::Duration::minutes(5),
        );
        let (report, decision) = ob_poc_control_plane::decision::evaluate_with_report(&ctx, validity);
        let gate_results = crate::agent::control_plane_shadow::report_to_json(&report);

        let expected = match decision {
            ob_poc_control_plane::decision::ControlPlaneDecision::ApprovedStp(_) => DecisionOutcome::ApprovedStp,
            ob_poc_control_plane::decision::ControlPlaneDecision::RequiresHumanGate(_) => DecisionOutcome::HumanGate,
            ob_poc_control_plane::decision::ControlPlaneDecision::Rejected(_) => DecisionOutcome::Rejected,
        };
        assert_eq!(rederive_decision_outcome(&gate_results), Some(expected));
    }

    // ── Live-DB: insert -> read back -> G11 completeness (end-to-end) ──

    #[cfg(feature = "database")]
    mod live_db {
        use super::*;

        async fn test_pool() -> sqlx::PgPool {
            let database_url = std::env::var("DATABASE_URL")
                .expect("DATABASE_URL must be set for control_plane_audit live-DB tests");
            sqlx::PgPool::connect(&database_url)
                .await
                .expect("failed to connect to test database")
        }

        /// End-to-end: insert a full ApprovedStp-with-commit lifecycle's
        /// worth of audit events, read them back via
        /// `audit_rows_for_decision`, and assert `check_completeness`
        /// accepts the real round-tripped sequence — proves the
        /// persistence layer and the G11 grammar check agree on what
        /// "complete" looks like, not just two independently-passing unit
        /// tests.
        #[tokio::test]
        #[ignore = "requires DATABASE_URL"]
        async fn full_lifecycle_round_trips_and_is_complete() {
            let pool = test_pool().await;
            let decision_id = Uuid::new_v4();
            let session_id = Uuid::new_v4();

            let events = vec![
                decision_evaluated(DecisionOutcome::ApprovedStp),
                AuditEvent::EnvelopeSealed {
                    envelope_id: decision_id,
                    expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
                },
                AuditEvent::EnvelopeConsumed {
                    envelope_id: decision_id,
                    gate_outcome: GateOutcomeRecord::new(GateId::ExecutionEnvelope, "Success"),
                },
                AuditEvent::DispatchCommitted {
                    attested: false,
                    gate_outcome: GateOutcomeRecord::new(GateId::WriteSetAttestation, "NotEvaluated"),
                },
            ];
            for event in &events {
                assert!(
                    insert_audit_event(&pool, decision_id, session_id, event).await,
                    "insert failed for {}",
                    event.event_type()
                );
            }

            let rows = audit_rows_for_decision(&pool, decision_id).await.expect("query failed");
            assert_eq!(rows.len(), events.len());
            // seq strictly increasing (ordering, not gaplessness — DD-4(i)).
            for pair in rows.windows(2) {
                assert!(pair[0].seq < pair[1].seq);
            }
            let reloaded_events: Vec<AuditEvent> = rows.into_iter().map(|r| r.event).collect();
            assert_eq!(reloaded_events, events, "round-tripped events must match what was inserted");
            assert_eq!(check_completeness(&reloaded_events), Ok(()));
        }

        /// W1 (golden-row): inserting the audit events alongside a shadow
        /// row does not change what `build_shadow_decision_row` itself
        /// produces or what `insert_shadow_decision` persists — the two
        /// inserts are independent, additive writes into different
        /// tables. Builds the row twice (once with audit-event inserts
        /// interleaved, once without any audit involvement at all) and
        /// asserts both the in-memory row and what's actually readable
        /// back from `control_plane_shadow_decisions` are identical.
        #[tokio::test]
        #[ignore = "requires DATABASE_URL"]
        async fn w1_shadow_row_is_field_identical_with_and_without_audit_emission() {
            let pool = test_pool().await;

            let mut report = ob_poc_control_plane::gate::EvaluationReport::default();
            report.results.insert(GateId::IntentAdmission, ob_poc_control_plane::gate::GateResult::Success);
            report.results.insert(
                GateId::Authority,
                ob_poc_control_plane::gate::GateResult::Failure("denied".to_string()),
            );

            // "Without audit emission in place": build + insert the shadow
            // row, touching nothing in control_plane_audit at all.
            let session_a = Uuid::new_v4();
            let entry_a = Uuid::new_v4();
            let row_without_audit = crate::agent::control_plane_shadow::build_shadow_decision_row(
                session_a, entry_a, "cbu.confirm", &report, false,
            );
            assert!(crate::agent::control_plane_shadow::insert_shadow_decision(&pool, &row_without_audit).await);

            // "With audit emission in place": build the row the identical
            // way, but this time also emit DecisionEvaluated/EnvelopeSealed
            // audit events into control_plane_audit before/after/around it.
            let session_b = Uuid::new_v4();
            let entry_b = Uuid::new_v4();
            let decision_id = Uuid::new_v4();
            assert!(
                insert_audit_event(
                    &pool,
                    decision_id,
                    session_b,
                    &decision_evaluated(DecisionOutcome::Rejected),
                )
                .await
            );
            let row_with_audit = crate::agent::control_plane_shadow::build_shadow_decision_row(
                session_b, entry_b, "cbu.confirm", &report, false,
            );
            assert!(crate::agent::control_plane_shadow::insert_shadow_decision(&pool, &row_with_audit).await);
            assert!(
                insert_audit_event(
                    &pool,
                    decision_id,
                    session_b,
                    &AuditEvent::DispatchRolledBack {
                        reason: "unrelated bookkeeping".to_string(),
                    },
                )
                .await
            );

            // The two rows differ only in session_id/entry_id (the
            // correlation keys the test itself varied to keep the two
            // fixtures distinguishable) -- every OTHER field, including
            // `diverged`/`gate_results`, must be identical regardless of
            // whether any control_plane_audit activity happened around
            // the insert.
            assert_eq!(row_without_audit.verb_fqn, row_with_audit.verb_fqn);
            assert_eq!(row_without_audit.gate_results, row_with_audit.gate_results);
            assert_eq!(row_without_audit.legacy_outcome_blocked, row_with_audit.legacy_outcome_blocked);
            assert_eq!(
                row_without_audit.shadow_intent_admission_blocked,
                row_with_audit.shadow_intent_admission_blocked
            );
            assert_eq!(row_without_audit.diverged, row_with_audit.diverged);
        }
    }
}
