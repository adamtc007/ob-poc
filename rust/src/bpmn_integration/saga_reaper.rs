//! BpmnStart saga reaper — **prototype skeleton** (2026-04-22).
//!
//! Closes the orphan-instance window left open after `BpmnStart` moved to
//! `pre_fetch`: if the Sequencer's outer transaction rolls back after
//! `StartProcess` has already been issued to the bpmn-server, the
//! bpmn-server holds a live process instance with no ob-poc-side trail.
//!
//! Design doc: `docs/todo/bpmn-start-saga-reaper-design-2026-04-22.md`.
//!
//! # Status
//!
//! Prototype — the module compiles and is unit-tested with a mock
//! client/pool, but production wiring (outbox marker, retention policy,
//! background task launch) is NOT complete. See §5 of the design doc for
//! the full follow-up list.
//!
//! # Correctness argument
//!
//! Claim: after `reaper_grace`, any `process_instance` without a
//! committed `bpmn_start_commit` outbox marker AND older than the grace
//! window MUST be orphaned.
//!
//! Proof sketch:
//!
//! 1. A live `process_instance` exists only if `BpmnStart::pre_fetch`
//!    fired `StartProcess` successfully.
//! 2. `BpmnStart::execute` writes the outbox marker via the same
//!    `TransactionScope` as every other runbook write.
//! 3. The outer tx commit is all-or-nothing — either the marker row
//!    commits (→ `pending` → drainer promotes to `done`) or it doesn't
//!    (→ row never visible outside the rolled-back tx).
//! 4. Therefore: outbox row absent after `reaper_grace` AND instance
//!    alive → outer tx rolled back → instance is orphaned.
//!
//! The converse holds by the same argument: outbox row present (any
//! status) → commit succeeded (or drainer is still working on it, which
//! is benign — the reaper's grace period lets `pending` rows settle).

use anyhow::{anyhow, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SagaReaperConfig {
    /// Minimum age before an instance is eligible for classification.
    /// MUST exceed the longest plausible commit latency + drainer poll.
    pub reaper_grace: Duration,
    /// How often the reaper wakes to scan.
    pub cycle_interval: Duration,
    /// Per-cycle rate limit — prevents a runaway reaper from mass-cancelling
    /// if a wider bug has created many "stale" instances at once.
    pub max_cancels_per_cycle: usize,
    /// Human-readable prefix on every cancel reason string.
    pub cancel_reason_prefix: &'static str,
}

impl Default for SagaReaperConfig {
    fn default() -> Self {
        Self {
            reaper_grace: Duration::from_secs(15 * 60),
            cycle_interval: Duration::from_secs(5 * 60),
            max_cancels_per_cycle: 50,
            cancel_reason_prefix: "ob-poc-saga-reaper",
        }
    }
}

// ---------------------------------------------------------------------------
// Instance snapshot + classification
// ---------------------------------------------------------------------------

/// Minimum evidence the reaper needs per candidate instance. Populated
/// from bpmn-server's `process_instances` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceSnapshot {
    pub instance_id: Uuid,
    pub correlation_id: String,
    /// Seconds since bpmn-server last touched the row. Used as the "staleness"
    /// proxy for whether the ob-poc outer tx should have committed by now.
    pub age_seconds: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classification {
    /// Outbox marker exists AND is `done`. Safe; outer tx committed.
    Committed,
    /// Outbox marker exists AND is `pending`. Drainer hasn't caught up,
    /// but the commit succeeded (the row wouldn't be visible otherwise).
    /// Treated as safe — reaper waits for the next cycle before revisiting.
    Pending,
    /// No outbox marker for `(correlation_id, instance_id)` after
    /// `reaper_grace` has elapsed → outer tx rolled back → orphan.
    Orphaned,
}

// ---------------------------------------------------------------------------
// Ports — abstract the two stores + the cancel RPC for testing
// ---------------------------------------------------------------------------

/// Reads stale candidates from bpmn-server's `process_instances`.
#[async_trait::async_trait]
pub trait InstanceSource: Send + Sync {
    async fn fetch_stale(&self, grace: Duration, limit: usize) -> Result<Vec<InstanceSnapshot>>;
}

/// Classifies a candidate by looking up the outbox marker on the ob-poc side.
#[async_trait::async_trait]
pub trait MarkerLookup: Send + Sync {
    async fn classify(&self, inst: &InstanceSnapshot) -> Result<Classification>;
}

/// Cancels an instance + writes a forensic audit row on success.
#[async_trait::async_trait]
pub trait InstanceCanceller: Send + Sync {
    async fn cancel_and_log(&self, inst: &InstanceSnapshot, reason: String) -> Result<()>;
}

// ---------------------------------------------------------------------------
// Reaper — pure logic, port-driven
// ---------------------------------------------------------------------------

pub struct SagaReaper {
    pub source: Arc<dyn InstanceSource>,
    pub markers: Arc<dyn MarkerLookup>,
    pub canceller: Arc<dyn InstanceCanceller>,
    pub cfg: SagaReaperConfig,
    pub shutdown: Arc<Notify>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ReaperCycleStats {
    pub inspected: usize,
    pub cancelled: usize,
    pub committed: usize,
    pub pending: usize,
    pub errors: usize,
}

impl SagaReaper {
    /// Background loop — one cycle per `cycle_interval` until shutdown.
    pub async fn run(self) {
        let mut ticker = tokio::time::interval(self.cfg.cycle_interval);
        loop {
            tokio::select! {
                _ = self.shutdown.notified() => {
                    tracing::info!("saga reaper: shutdown signalled");
                    break;
                }
                _ = ticker.tick() => {
                    match self.run_once().await {
                        Ok(stats) => tracing::debug!(?stats, "saga reaper cycle complete"),
                        Err(e) => tracing::warn!(error = %e, "saga reaper cycle failed"),
                    }
                }
            }
        }
    }

    /// One cycle — public for tests + on-demand trigger.
    pub async fn run_once(&self) -> Result<ReaperCycleStats> {
        let candidates = self
            .source
            .fetch_stale(self.cfg.reaper_grace, self.cfg.max_cancels_per_cycle * 4)
            .await?;
        let mut stats = ReaperCycleStats {
            inspected: candidates.len(),
            ..Default::default()
        };

        for inst in candidates {
            if stats.cancelled >= self.cfg.max_cancels_per_cycle {
                tracing::warn!(
                    cancelled = stats.cancelled,
                    limit = self.cfg.max_cancels_per_cycle,
                    "saga reaper: per-cycle cancel limit hit — deferring remaining candidates"
                );
                break;
            }
            match self.markers.classify(&inst).await {
                Ok(Classification::Committed) => stats.committed += 1,
                Ok(Classification::Pending) => stats.pending += 1,
                Ok(Classification::Orphaned) => {
                    let reason = format!(
                        "{}: outer-tx rollback detected ({}s stale, correlation={})",
                        self.cfg.cancel_reason_prefix, inst.age_seconds, inst.correlation_id
                    );
                    match self.canceller.cancel_and_log(&inst, reason).await {
                        Ok(()) => stats.cancelled += 1,
                        Err(e) => {
                            tracing::warn!(
                                instance = %inst.instance_id,
                                error = %e,
                                "saga reaper: cancel failed — retrying next cycle"
                            );
                            stats.errors += 1;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        instance = %inst.instance_id,
                        error = %e,
                        "saga reaper: classify failed — retrying next cycle"
                    );
                    stats.errors += 1;
                }
            }
        }

        Ok(stats)
    }
}

// ---------------------------------------------------------------------------
// Not-yet-implemented: production `InstanceSource` / `MarkerLookup` /
// `InstanceCanceller` impls. Those require:
//
//  - A bpmn-server DB pool (the reaper needs read access to
//    `process_instances` either via gRPC `ListOrphans` — not yet in the
//    proto — or by wiring a separate sqlx pool to the bpmn-server DB).
//  - The `bpmn_start_commit` outbox `effect_kind` (not yet defined in
//    `ob_poc_types::OutboxEffectKind`).
//  - The `bpmn_reaper_log` table (migration not written).
//
// See design doc §5 for the exact follow-up list.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Tests — port-driven; no DB, no gRPC.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Mock source that returns a canned list.
    struct MockSource(Vec<InstanceSnapshot>);

    #[async_trait::async_trait]
    impl InstanceSource for MockSource {
        async fn fetch_stale(&self, _g: Duration, _l: usize) -> Result<Vec<InstanceSnapshot>> {
            Ok(self.0.clone())
        }
    }

    /// Mock marker lookup that returns canned classifications keyed by
    /// instance_id.
    struct MockMarkers(std::collections::HashMap<Uuid, Classification>);

    #[async_trait::async_trait]
    impl MarkerLookup for MockMarkers {
        async fn classify(&self, inst: &InstanceSnapshot) -> Result<Classification> {
            self.0
                .get(&inst.instance_id)
                .copied()
                .ok_or_else(|| anyhow!("no mock classification for {}", inst.instance_id))
        }
    }

    /// Mock canceller that records invocations.
    struct MockCanceller(Mutex<Vec<(Uuid, String)>>);

    #[async_trait::async_trait]
    impl InstanceCanceller for MockCanceller {
        async fn cancel_and_log(&self, inst: &InstanceSnapshot, reason: String) -> Result<()> {
            self.0.lock().unwrap().push((inst.instance_id, reason));
            Ok(())
        }
    }

    fn mk_snapshot(age_seconds: u64) -> InstanceSnapshot {
        InstanceSnapshot {
            instance_id: Uuid::new_v4(),
            correlation_id: format!("corr-{}", age_seconds),
            age_seconds,
        }
    }

    #[tokio::test]
    async fn run_once_committed_only_does_not_cancel() {
        let snap = mk_snapshot(3600);
        let source = Arc::new(MockSource(vec![snap.clone()]));
        let mut m = std::collections::HashMap::new();
        m.insert(snap.instance_id, Classification::Committed);
        let markers = Arc::new(MockMarkers(m));
        let canceller = Arc::new(MockCanceller(Mutex::new(Vec::new())));

        let reaper = SagaReaper {
            source,
            markers,
            canceller: canceller.clone(),
            cfg: SagaReaperConfig::default(),
            shutdown: Arc::new(Notify::new()),
        };

        let stats = reaper.run_once().await.unwrap();
        assert_eq!(stats.inspected, 1);
        assert_eq!(stats.committed, 1);
        assert_eq!(stats.cancelled, 0);
        assert!(canceller.0.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn run_once_orphaned_gets_cancelled() {
        let snap = mk_snapshot(3600);
        let source = Arc::new(MockSource(vec![snap.clone()]));
        let mut m = std::collections::HashMap::new();
        m.insert(snap.instance_id, Classification::Orphaned);
        let markers = Arc::new(MockMarkers(m));
        let canceller = Arc::new(MockCanceller(Mutex::new(Vec::new())));

        let reaper = SagaReaper {
            source,
            markers,
            canceller: canceller.clone(),
            cfg: SagaReaperConfig::default(),
            shutdown: Arc::new(Notify::new()),
        };

        let stats = reaper.run_once().await.unwrap();
        assert_eq!(stats.inspected, 1);
        assert_eq!(stats.cancelled, 1);
        let logged = canceller.0.lock().unwrap();
        assert_eq!(logged.len(), 1);
        assert_eq!(logged[0].0, snap.instance_id);
        assert!(logged[0].1.contains("ob-poc-saga-reaper"));
        assert!(logged[0].1.contains("outer-tx rollback"));
        assert!(logged[0].1.contains(&snap.correlation_id));
    }

    #[tokio::test]
    async fn run_once_pending_does_not_cancel_yet() {
        let snap = mk_snapshot(3600);
        let source = Arc::new(MockSource(vec![snap.clone()]));
        let mut m = std::collections::HashMap::new();
        m.insert(snap.instance_id, Classification::Pending);
        let markers = Arc::new(MockMarkers(m));
        let canceller = Arc::new(MockCanceller(Mutex::new(Vec::new())));

        let reaper = SagaReaper {
            source,
            markers,
            canceller: canceller.clone(),
            cfg: SagaReaperConfig::default(),
            shutdown: Arc::new(Notify::new()),
        };

        let stats = reaper.run_once().await.unwrap();
        assert_eq!(stats.pending, 1);
        assert_eq!(stats.cancelled, 0);
    }

    #[tokio::test]
    async fn run_once_respects_max_cancels_per_cycle() {
        let snaps: Vec<InstanceSnapshot> = (0..10).map(|i| mk_snapshot(3600 + i)).collect();
        let mut m = std::collections::HashMap::new();
        for s in &snaps {
            m.insert(s.instance_id, Classification::Orphaned);
        }
        let source = Arc::new(MockSource(snaps.clone()));
        let markers = Arc::new(MockMarkers(m));
        let canceller = Arc::new(MockCanceller(Mutex::new(Vec::new())));

        let cfg = SagaReaperConfig {
            max_cancels_per_cycle: 3,
            ..SagaReaperConfig::default()
        };
        let reaper = SagaReaper {
            source,
            markers,
            canceller: canceller.clone(),
            cfg,
            shutdown: Arc::new(Notify::new()),
        };

        let stats = reaper.run_once().await.unwrap();
        assert_eq!(stats.cancelled, 3, "per-cycle limit respected");
        assert_eq!(canceller.0.lock().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn run_once_classify_error_continues_to_next_candidate() {
        let snap_bad = mk_snapshot(100);
        let snap_good = mk_snapshot(200);
        let source = Arc::new(MockSource(vec![snap_bad.clone(), snap_good.clone()]));
        // markers map only contains snap_good → snap_bad classify errors.
        let mut m = std::collections::HashMap::new();
        m.insert(snap_good.instance_id, Classification::Orphaned);
        let markers = Arc::new(MockMarkers(m));
        let canceller = Arc::new(MockCanceller(Mutex::new(Vec::new())));

        let reaper = SagaReaper {
            source,
            markers,
            canceller: canceller.clone(),
            cfg: SagaReaperConfig::default(),
            shutdown: Arc::new(Notify::new()),
        };

        let stats = reaper.run_once().await.unwrap();
        assert_eq!(stats.errors, 1);
        assert_eq!(stats.cancelled, 1);
        assert_eq!(canceller.0.lock().unwrap()[0].0, snap_good.instance_id);
    }
}
