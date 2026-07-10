//! G14 — Write-Set Attestation (V&S §6.7.1, post-execution).
//!
//! No production analogue exists today (RR-4; ledger C-032: "CRUD executor
//! executes metadata-driven insert/update/delete/upsert without comparing
//! to a WriteSetProof"). T5.1-T5.3 add real write capture and attestation
//! at the `PgTransactionScope` boundary (`ob-poc::sequencer_tx`) — this
//! module owns only the *comparison* (`attest`), matching §9.1: this crate
//! never executes SQL, it only decides whether an already-captured set of
//! writes stays within an already-derived bound.
//!
//! Distinct from G7 (`write_set` — pre-execution derivation of the bound):
//! G7 answers "what is the command allowed to write?"; G14 answers "did
//! the runtime actually stay inside that bound?" A command can pass G7 and
//! still fail G14 if its execution touched more than it declared.

use uuid::Uuid;

use crate::gate::{Gate, GateId, GateResult};
use crate::write_set::WriteSetProof;

/// One write the runtime actually performed, as reported by the caller
/// (`PgTransactionScope::record_write` — self-reported, since sqlx offers
/// no post-hoc introspection of which table/columns a raw `sqlx::query!`
/// touched; see that method's doc for the honesty note this implies).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CapturedWrite {
    pub table: String,
    pub entity_id: Uuid,
    pub columns: Vec<String>,
}

/// `WriteSetAttestation` — V&S §6.7.1 "Output".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttestationOutcome {
    /// Every captured write is covered by the declared `WriteSetProof`.
    Bounded,
    /// At least one captured write falls outside the declared bound
    /// (unlisted table, unlisted entity, or unlisted column). Carries
    /// *every* excess write, not just the first — collect-where-
    /// independent discipline (§6.16), so a breach report is a complete
    /// work item for whoever triages it, not a truncated hint.
    Breach { excess: Vec<CapturedWrite> },
}

/// Pure comparison: every element of `captured` must be covered by
/// `expected` — same table, same entity id, and every column a subset of
/// `expected.allowed_columns()`. No I/O, no re-derivation of the bound.
pub fn attest(captured: &[CapturedWrite], expected: &WriteSetProof) -> AttestationOutcome {
    let excess: Vec<CapturedWrite> = captured
        .iter()
        .filter(|write| {
            !expected.tables().contains(&write.table)
                || !expected.entity_ids().contains(&write.entity_id)
                || !write.columns.iter().all(|c| expected.allowed_columns().contains(c))
        })
        .cloned()
        .collect();

    if excess.is_empty() {
        AttestationOutcome::Bounded
    } else {
        AttestationOutcome::Breach { excess }
    }
}

/// Pre-computed input for the G14 gate. Primitive-typed (mirrors
/// `WriteSetProof`'s fields rather than requiring one, since the crate's
/// own gate-evaluation context — `context::EvaluationContext` — carries
/// only primitives per gate, never proof objects; production
/// pre-commit enforcement (T5.2) calls `attest` directly against a real
/// `WriteSetProof` at `PgTransactionScope::commit_attested`, this input
/// shape exists only so `evaluate_shadow` can report G14 consistently
/// alongside every other gate).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct WriteSetAttestationInput {
    pub captured: Vec<CapturedWrite>,
    pub expected_tables: Vec<String>,
    pub expected_entity_ids: Vec<Uuid>,
    pub expected_allowed_columns: Vec<String>,
}

fn decide(input: &WriteSetAttestationInput) -> AttestationOutcome {
    let excess: Vec<CapturedWrite> = input
        .captured
        .iter()
        .filter(|write| {
            !input.expected_tables.contains(&write.table)
                || !input.expected_entity_ids.contains(&write.entity_id)
                || !write.columns.iter().all(|c| input.expected_allowed_columns.contains(c))
        })
        .cloned()
        .collect();
    if excess.is_empty() {
        AttestationOutcome::Bounded
    } else {
        AttestationOutcome::Breach { excess }
    }
}

/// T5 adapter: `Gate<crate::context::EvaluationContext>` impl for G14.
pub struct WriteSetAttestationGate;

impl Gate<crate::context::EvaluationContext> for WriteSetAttestationGate {
    fn id(&self) -> GateId {
        GateId::WriteSetAttestation
    }

    fn evaluate(&self, ctx: &crate::context::EvaluationContext) -> GateResult {
        let Some(input) = &ctx.write_set_attestation else {
            return GateResult::Failure("no WriteSetAttestationInput supplied".to_string());
        };
        match decide(input) {
            AttestationOutcome::Bounded => GateResult::Success,
            AttestationOutcome::Breach { excess } => {
                GateResult::Failure(format!("write-set breach: {} excess write(s)", excess.len()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_within_declared_bound_attests_clean() {
        let proof = crate::write_set::tests_support::proof(
            vec![Uuid::nil()],
            vec!["validation_state".to_string()],
            vec!["ob-poc.cbus".to_string()],
            vec!["status".to_string()],
            "idem-1",
        );
        let captured = vec![CapturedWrite {
            table: "ob-poc.cbus".to_string(),
            entity_id: Uuid::nil(),
            columns: vec!["status".to_string()],
        }];
        assert_eq!(attest(&captured, &proof), AttestationOutcome::Bounded);
    }

    #[test]
    fn write_to_undeclared_table_is_a_breach() {
        let proof = crate::write_set::tests_support::proof(
            vec![Uuid::nil()],
            vec!["validation_state".to_string()],
            vec!["ob-poc.cbus".to_string()],
            vec!["status".to_string()],
            "idem-1",
        );
        let captured = vec![CapturedWrite {
            table: "ob-poc.entities".to_string(),
            entity_id: Uuid::nil(),
            columns: vec!["name".to_string()],
        }];
        let outcome = attest(&captured, &proof);
        assert!(matches!(&outcome, AttestationOutcome::Breach { excess } if excess.len() == 1));
    }

    #[test]
    fn write_to_undeclared_column_is_a_breach() {
        let proof = crate::write_set::tests_support::proof(
            vec![Uuid::nil()],
            vec!["validation_state".to_string()],
            vec!["ob-poc.cbus".to_string()],
            vec!["status".to_string()],
            "idem-1",
        );
        let captured = vec![CapturedWrite {
            table: "ob-poc.cbus".to_string(),
            entity_id: Uuid::nil(),
            columns: vec!["status".to_string(), "legal_name".to_string()],
        }];
        assert!(matches!(attest(&captured, &proof), AttestationOutcome::Breach { .. }));
    }

    #[test]
    fn write_to_undeclared_entity_is_a_breach() {
        let proof = crate::write_set::tests_support::proof(
            vec![Uuid::nil()],
            vec!["validation_state".to_string()],
            vec!["ob-poc.cbus".to_string()],
            vec!["status".to_string()],
            "idem-1",
        );
        let captured = vec![CapturedWrite {
            table: "ob-poc.cbus".to_string(),
            entity_id: Uuid::new_v4(),
            columns: vec!["status".to_string()],
        }];
        assert!(matches!(attest(&captured, &proof), AttestationOutcome::Breach { .. }));
    }

    #[test]
    fn breach_collects_every_excess_write_not_just_the_first() {
        let proof = crate::write_set::tests_support::proof(
            vec![Uuid::nil()],
            vec![],
            vec!["ob-poc.cbus".to_string()],
            vec!["status".to_string()],
            "idem-1",
        );
        let captured = vec![
            CapturedWrite {
                table: "ob-poc.entities".to_string(),
                entity_id: Uuid::nil(),
                columns: vec!["name".to_string()],
            },
            CapturedWrite {
                table: "ob-poc.cases".to_string(),
                entity_id: Uuid::nil(),
                columns: vec!["status".to_string()],
            },
        ];
        match attest(&captured, &proof) {
            AttestationOutcome::Breach { excess } => assert_eq!(excess.len(), 2),
            AttestationOutcome::Bounded => panic!("expected breach"),
        }
    }

    fn base_input() -> WriteSetAttestationInput {
        WriteSetAttestationInput {
            captured: vec![CapturedWrite {
                table: "ob-poc.cbus".to_string(),
                entity_id: Uuid::nil(),
                columns: vec!["status".to_string()],
            }],
            expected_tables: vec!["ob-poc.cbus".to_string()],
            expected_entity_ids: vec![Uuid::nil()],
            expected_allowed_columns: vec!["status".to_string()],
        }
    }

    #[test]
    fn gate_evaluate_reports_success_when_bounded() {
        let ctx = crate::context::EvaluationContext {
            write_set_attestation: Some(base_input()),
            ..Default::default()
        };
        assert_eq!(WriteSetAttestationGate.evaluate(&ctx), GateResult::Success);
        assert_eq!(WriteSetAttestationGate.id(), GateId::WriteSetAttestation);
    }

    #[test]
    fn gate_evaluate_reports_failure_on_breach() {
        let ctx = crate::context::EvaluationContext {
            write_set_attestation: Some(WriteSetAttestationInput {
                captured: vec![CapturedWrite {
                    table: "ob-poc.entities".to_string(),
                    entity_id: Uuid::nil(),
                    columns: vec!["name".to_string()],
                }],
                ..base_input()
            }),
            ..Default::default()
        };
        assert!(matches!(
            WriteSetAttestationGate.evaluate(&ctx),
            GateResult::Failure(_)
        ));
    }

    #[test]
    fn gate_evaluate_fails_closed_when_input_missing() {
        let ctx = crate::context::EvaluationContext::default();
        assert!(matches!(
            WriteSetAttestationGate.evaluate(&ctx),
            GateResult::Failure(_)
        ));
    }
}
