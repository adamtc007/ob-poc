//! Constellation hydration / summary projection.
//!
//! `constellation.hydrate` and `constellation.summary` walk a SemOS
//! constellation map (a static schema like `struct.lux.ucits.sicav`)
//! against a CBU's persisted state, producing a `HydratedConstellation`
//! / `ConstellationSummary` for downstream UI projection. Those
//! concrete types live deep in `ob_poc::sem_os_runtime::*` and pull
//! the entire reducer + slot-walker surface; rather than thread them
//! across the plane boundary, this trait projects each result through
//! `serde_json::to_value` — the consumer ops already wrap the output
//! as `VerbExecutionOutcome::Record(...)`, which is JSON-shaped, so
//! no fidelity is lost.
//!
//! Introduced in Phase 5a composite-blocker #9 for `constellation_ops`.
//! The ob-poc bridge (`ObPocConstellationRuntime`) delegates to
//! `crate::sem_os_runtime::constellation_runtime::handle_constellation_{hydrate,summary}`.
//! Consumers obtain the impl via
//! [`crate::VerbExecutionContext::service::<dyn ConstellationRuntime>`].

use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

/// Constellation hydration + summary against the SemOS runtime map
/// catalogue. Both methods take a CBU id, an optional KYC case id (for
/// case-bound constellations), and the map name (e.g.
/// `struct.lux.ucits.sicav`). The bridge resolves the map, walks it
/// against the database, and returns a JSON projection.
#[async_trait]
pub trait ConstellationRuntime: Send + Sync {
    /// Hydrate a full constellation tree. JSON shape mirrors the
    /// internal `HydratedConstellation` (slots, reducer state,
    /// state-machine bindings).
    async fn hydrate(
        &self,
        cbu_id: Uuid,
        case_id: Option<Uuid>,
        map_name: &str,
    ) -> Result<serde_json::Value>;

    /// Compute a compact summary (slot counts, gap totals, progress
    /// markers) over a hydrated constellation. JSON shape mirrors the
    /// internal `ConstellationSummary`.
    async fn summary(
        &self,
        cbu_id: Uuid,
        case_id: Option<Uuid>,
        map_name: &str,
    ) -> Result<serde_json::Value>;
}
