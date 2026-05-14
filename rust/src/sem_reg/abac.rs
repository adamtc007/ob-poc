//! Canonical ABAC types and helpers re-exported from `sem_os_core`.

pub use sem_os_policy::abac::{
    evaluate_abac, evaluate_abac_with_evidence_grade, AccessDecision, AccessPurpose, ActorContext,
};
