//! G5 — Authority and Policy Gate (V&S §6.5).
//!
//! T2.4 wires the adapter over `AccessDecision` partitioning +
//! `ActorResolver` (ledger C-006, C-007, C-008); the TOCTOU recheck result
//! is consumed as snapshot evidence, not re-implemented. Evidence readiness
//! is owned by the evidence gate (§6.6) — this gate consumes that gate's
//! outcome as a policy-time input, not a hard dependency (see the comment
//! on `GATE_DEPENDENCIES` in `gate.rs`).

use uuid::Uuid;

/// `AuthorityDecision` — V&S §6.5 "Output". Variant names mirror the
/// possible outcomes listed there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorityOutcome {
    Authorised(Authorised),
    RequiresHumanApproval,
    RequiresSecondLineReview,
    RejectedUnauthorised,
    RejectedSegregationOfDuties,
    RejectedPolicy,
}

/// Success-form proof: the actor/context is authorised to execute the
/// proposed command. Constructible only from within this module.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Authorised {
    actor_id: Uuid,
    role: String,
}

impl Authorised {
    // Called by the (future) T2.4 adapter; the only caller today is the
    // cfg(test) bridge below.
    #[allow(dead_code)]
    fn new(actor_id: Uuid, role: impl Into<String>) -> Self {
        Self {
            actor_id,
            role: role.into(),
        }
    }

    pub fn actor_id(&self) -> Uuid {
        self.actor_id
    }

    pub fn role(&self) -> &str {
        &self.role
    }
}

#[cfg(test)]
pub(crate) mod tests_support {
    use super::Authorised;
    use uuid::Uuid;

    pub(crate) fn authorised(actor_id: Uuid, role: &str) -> Authorised {
        Authorised::new(actor_id, role)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorised_is_constructible_within_its_own_module() {
        let authorised = Authorised::new(Uuid::nil(), "compliance_officer");
        assert_eq!(authorised.role(), "compliance_officer");
    }
}
