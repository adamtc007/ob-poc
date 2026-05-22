//! Operational metrics for the bpmn-lite journey runtime.
//!
//! [`RuntimeMetrics`] is a cheap-to-clone set of atomic counters. Call
//! [`RuntimeMetrics::snapshot`] to get a serialisable point-in-time view, or
//! [`RuntimeMetrics::prometheus_text`] to produce Prometheus-format output.

use serde::Serialize;
use std::sync::{atomic::{AtomicU64, Ordering}, Arc};

/// Operational metrics for the journey runtime.
///
/// All counters are `Arc<AtomicU64>` so the struct is cheap to clone and can
/// be shared across threads without additional synchronisation.
#[derive(Clone, Default)]
pub struct RuntimeMetrics {
    pub instances_started: Arc<AtomicU64>,
    pub instances_completed: Arc<AtomicU64>,
    pub instances_failed: Arc<AtomicU64>,
    pub instances_cancelled: Arc<AtomicU64>,
    pub events_processed: Arc<AtomicU64>,
    pub verbs_invoked: Arc<AtomicU64>,
    pub gateway_decisions: Arc<AtomicU64>,
    pub parallel_forks: Arc<AtomicU64>,
    pub joins_fired: Arc<AtomicU64>,
    pub merge_conflicts: Arc<AtomicU64>,
    pub timer_events_fired: Arc<AtomicU64>,
    pub human_tasks_completed: Arc<AtomicU64>,
}

impl RuntimeMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    /// Increment a counter by one (relaxed ordering — counters only).
    pub fn increment(counter: &Arc<AtomicU64>) {
        counter.fetch_add(1, Ordering::Relaxed);
    }

    /// Snapshot of all metrics as a serialisable struct.
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            instances_started: self.instances_started.load(Ordering::Relaxed),
            instances_completed: self.instances_completed.load(Ordering::Relaxed),
            instances_failed: self.instances_failed.load(Ordering::Relaxed),
            instances_cancelled: self.instances_cancelled.load(Ordering::Relaxed),
            events_processed: self.events_processed.load(Ordering::Relaxed),
            verbs_invoked: self.verbs_invoked.load(Ordering::Relaxed),
            gateway_decisions: self.gateway_decisions.load(Ordering::Relaxed),
            parallel_forks: self.parallel_forks.load(Ordering::Relaxed),
            joins_fired: self.joins_fired.load(Ordering::Relaxed),
            merge_conflicts: self.merge_conflicts.load(Ordering::Relaxed),
            timer_events_fired: self.timer_events_fired.load(Ordering::Relaxed),
            human_tasks_completed: self.human_tasks_completed.load(Ordering::Relaxed),
        }
    }

    /// Produce Prometheus text-format output for the most important counters.
    pub fn prometheus_text(&self) -> String {
        let s = self.snapshot();
        format!(
            "# HELP bpmn_instances_started Total workflow instances started\n\
             # TYPE bpmn_instances_started counter\n\
             bpmn_instances_started {}\n\
             # HELP bpmn_instances_completed Total workflow instances completed\n\
             # TYPE bpmn_instances_completed counter\n\
             bpmn_instances_completed {}\n\
             # HELP bpmn_instances_failed Total workflow instances failed\n\
             # TYPE bpmn_instances_failed counter\n\
             bpmn_instances_failed {}\n\
             # HELP bpmn_instances_cancelled Total workflow instances cancelled\n\
             # TYPE bpmn_instances_cancelled counter\n\
             bpmn_instances_cancelled {}\n\
             # HELP bpmn_events_processed Total events processed\n\
             # TYPE bpmn_events_processed counter\n\
             bpmn_events_processed {}\n\
             # HELP bpmn_verbs_invoked Total verb invocations\n\
             # TYPE bpmn_verbs_invoked counter\n\
             bpmn_verbs_invoked {}\n\
             # HELP bpmn_gateway_decisions Total gateway decisions made\n\
             # TYPE bpmn_gateway_decisions counter\n\
             bpmn_gateway_decisions {}\n\
             # HELP bpmn_parallel_forks Total parallel fork activations\n\
             # TYPE bpmn_parallel_forks counter\n\
             bpmn_parallel_forks {}\n\
             # HELP bpmn_joins_fired Total parallel join completions\n\
             # TYPE bpmn_joins_fired counter\n\
             bpmn_joins_fired {}\n\
             # HELP bpmn_merge_conflicts Total merge conflicts detected at joins\n\
             # TYPE bpmn_merge_conflicts counter\n\
             bpmn_merge_conflicts {}\n\
             # HELP bpmn_timer_events_fired Total timer events fired\n\
             # TYPE bpmn_timer_events_fired counter\n\
             bpmn_timer_events_fired {}\n\
             # HELP bpmn_human_tasks_completed Total human tasks completed\n\
             # TYPE bpmn_human_tasks_completed counter\n\
             bpmn_human_tasks_completed {}\n",
            s.instances_started,
            s.instances_completed,
            s.instances_failed,
            s.instances_cancelled,
            s.events_processed,
            s.verbs_invoked,
            s.gateway_decisions,
            s.parallel_forks,
            s.joins_fired,
            s.merge_conflicts,
            s.timer_events_fired,
            s.human_tasks_completed,
        )
    }
}

/// Point-in-time snapshot of all runtime counters.
#[derive(Debug, Clone, Serialize)]
pub struct MetricsSnapshot {
    pub instances_started: u64,
    pub instances_completed: u64,
    pub instances_failed: u64,
    pub instances_cancelled: u64,
    pub events_processed: u64,
    pub verbs_invoked: u64,
    pub gateway_decisions: u64,
    pub parallel_forks: u64,
    pub joins_fired: u64,
    pub merge_conflicts: u64,
    pub timer_events_fired: u64,
    pub human_tasks_completed: u64,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_increment_and_snapshot() {
        let m = RuntimeMetrics::new();
        RuntimeMetrics::increment(&m.instances_started);
        RuntimeMetrics::increment(&m.instances_started);
        RuntimeMetrics::increment(&m.instances_completed);
        let s = m.snapshot();
        assert_eq!(s.instances_started, 2);
        assert_eq!(s.instances_completed, 1);
        assert_eq!(s.instances_failed, 0);
    }

    #[test]
    fn prometheus_text_contains_metric_names() {
        let m = RuntimeMetrics::new();
        RuntimeMetrics::increment(&m.gateway_decisions);
        RuntimeMetrics::increment(&m.merge_conflicts);
        let text = m.prometheus_text();
        assert!(text.contains("bpmn_instances_started 0"));
        assert!(text.contains("bpmn_gateway_decisions 1"));
        assert!(text.contains("bpmn_merge_conflicts 1"));
    }

    #[test]
    fn clone_shares_counters() {
        let m1 = RuntimeMetrics::new();
        let m2 = m1.clone();
        RuntimeMetrics::increment(&m1.events_processed);
        // m2 shares the same Arc — it sees the increment.
        assert_eq!(m2.events_processed.load(std::sync::atomic::Ordering::Relaxed), 1);
    }

    #[test]
    fn prometheus_text_format_valid() {
        let m = RuntimeMetrics::new();
        let text = m.prometheus_text();
        // Every counter block starts with # HELP
        let help_lines = text.lines().filter(|l| l.starts_with("# HELP")).count();
        let type_lines = text.lines().filter(|l| l.starts_with("# TYPE")).count();
        assert_eq!(help_lines, 12);
        assert_eq!(type_lines, 12);
    }
}
