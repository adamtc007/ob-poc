//! Policy module — central enforcement for single-pipeline invariants.
//!
//! `PolicyGate` is the single source of truth for all bypass/privilege decisions.
//! No endpoint or tool should make gating decisions without consulting it.
//!
//! HTTP-binding (header → ActorContext) lives one tier up in
//! `ob_poc::api::policy_headers` so envelope stays transport-neutral.

pub mod gate;

pub use gate::{ActorResolver, PolicyGate, PolicySnapshot};
