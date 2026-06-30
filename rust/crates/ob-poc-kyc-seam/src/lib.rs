//! KYC/UBO runtime↔store seam — EOP-DD-KYCUBO-002 §4.
#![deny(unreachable_pub)]
//!
//! The reusable bridge between the Sequencer's transaction model and the
//! durable verb stream. Two responsibilities, both pure impedance-matching —
//! **no determination logic** lives here (that is the substrate's):
//!
//! 1. [`append_in_scope`] — the §3.6 chokepoint: run the §3 append inside the
//!    `TransactionScope` the Sequencer already owns. Every KYC stream append
//!    routes through this one function, so the guard/audit have a single
//!    enforcement point.
//! 2. [`IntentEventDraft`] / [`map_principal`] — map the runtime execution
//!    identity (string `actor_id`, role vec) to the substrate's thin principal
//!    and stamp correlation / idempotency.
//!
//! This is the **only** KYC crate that depends on `dsl-runtime`; the store
//! (`ob-poc-kyc-store`) stays a pure Postgres membrane taking `&mut PgConnection`.

mod seam;

pub use seam::{append_in_scope, map_principal, IntentEventDraft};
