//! G8 — STP Eligibility Classification (V&S §6.8).
//!
//! No production analogue exists today. T3.3 implements the real
//! classifier as a pure function over aggregated gate results + config
//! policy; it must be deterministic — the same (intent, ctx, pins) always
//! yields the same classification (§12.8, §12.11).
//!
//! The AI must not self-certify STP eligibility (§6.8) — this module's
//! (future) classifier is the only code path permitted to produce a
//! `StpEligibilityDecision`.

/// `StpEligibilityDecision` — V&S §6.8 "Output" / classification table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StpEligibilityDecision {
    /// Valid, authorised, evidence complete, no human gate required.
    StpExecutable,
    /// Valid plan, but approval/review required before execution.
    HumanGated,
    /// Invalid, ambiguous, unauthorised, incomplete or outside scope.
    Rejected,
}
