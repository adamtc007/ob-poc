//! KYC/UBO runtime↔store seam — EOP-DD-KYCUBO-002 §4.
#![deny(unreachable_pub)]
//!
//! The reusable bridge between the Sequencer's transaction model and the
//! durable verb stream. Three responsibilities, all pure impedance-matching —
//! **no determination logic** lives here (that is the substrate's):
//!
//! 1. [`canonical_event_shape`] — R6's one event constructor (§3.4, T1 of
//!    the `EOP-VS-UBO-GAME-001` rip-and-replace): maps a verb's declared
//!    arguments to `(TargetBinding, payload, Option<EdgeId>)`. Every
//!    surface that builds a `kyc_ubo.*` `IntentEvent` calls this — the op
//!    layer and the workbook both, no exceptions besides the three
//!    documented on the function itself (freeze, the three decide.*
//!    verdicts, and one state-dependent payload field of type-correction).
//! 2. [`append_in_scope`] — the §3.6 chokepoint: run the §3 append inside the
//!    `TransactionScope` the Sequencer already owns. Every KYC stream append
//!    routes through this one function, so the guard/audit have a single
//!    enforcement point.
//! 3. [`IntentEventDraft`] / [`map_principal`] — map the runtime execution
//!    identity (string `actor_id`, role vec) to the substrate's thin principal
//!    and stamp correlation / idempotency.
//!
//! This is the **only** KYC crate that depends on `dsl-runtime`; the store
//! (`ob-poc-kyc-store`) stays a pure Postgres membrane taking `&mut PgConnection`.

mod canonical;
mod seam;

pub use canonical::canonical_event_shape;
pub use seam::{append_in_scope, map_principal, IntentEventDraft};
