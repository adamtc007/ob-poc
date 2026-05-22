# ob-poc Unified DSL v0.1 — Operational Runbook

## Deployment

### Schema migration

```bash
cd rust/
psql -d data_designer -f migrations/20260521_dsl_journey_runtime.sql
```

Tables created (all `dsl_` prefixed):
- `dsl_workflow_instance` — top-level instance records
- `dsl_journey_log` — append-only audit trail
- `dsl_active_token` — live execution positions
- `dsl_instance_data` — versioned application data
- `dsl_pending_wait` — external event registrations
- `dsl_pending_timer` — scheduled timers
- `dsl_switch_decision_request` — in-flight gateway decisions
- `dsl_event_queue` — inbound event queue (FOR UPDATE SKIP LOCKED)
- `dsl_join_arrival` — parallel join arrival tracking

### Pack registry

Pack DSL source files are at `rust/dsl-source/packs/`. Load at startup:

```rust
use dsl_resolution::{PackRegistry, load_packs_from_dir};
let mut registry = PackRegistry::new();
let mut diag = DiagnosticBag::new();
load_packs_from_dir(Path::new("dsl-source/packs"), &mut registry, &mut diag)?;
```

### Running tests

```bash
# All unified DSL crates
cargo test -p dsl-atoms -p dsl-diagnostics -p dsl-parser -p dsl-ast \
  -p dsl-bpmn-frontend -p dsl-lowering -p dsl-resolution \
  -p bpmn-runtime -p bpmn-test-harness

# Pack catalogue only
cargo test -p dsl-resolution

# Runtime scenarios only
cargo test -p bpmn-test-harness

# Include perf tests
cargo test -p bpmn-test-harness -- --include-ignored
```

## Monitoring

### Journey log queries

```sql
-- Active instances
SELECT id, journey_name, status, started_at
FROM dsl_workflow_instance
WHERE status = 'active'
ORDER BY started_at DESC;

-- Instance timeline
SELECT event_kind, from_node, to_node, recorded_at, data_delta
FROM dsl_journey_log
WHERE instance_id = $1
ORDER BY id;

-- Stuck tokens (active for > 1 hour)
SELECT t.id, t.instance_id, t.current_node, i.journey_name
FROM dsl_active_token t
JOIN dsl_workflow_instance i ON i.id = t.instance_id
WHERE t.created_at < now() - interval '1 hour'
  AND i.status = 'active';
```

## Common failure modes

| Symptom | Diagnosis | Resolution |
|---|---|---|
| Instance stays `active`, no tokens advancing | All tokens at unregistered verb nodes | Register verb handler or fire VerbCompletion event manually |
| Instance `failed` with `merge_conflict` in journal | Parallel branches wrote conflicting values to same location | Add `:merge` clause to parallel-join atom, re-deploy process |
| Instance `active`, token at gateway, no switch reply | Switch adaptor not configured for this gateway | Register adaptor or provide scripted reply |
| `UNDECLARED_MERGE` warning at compile | Parallel-join branches may conflict | Add explicit `:merge` clause |

## Backup and recovery

The `dsl_journey_log` table is the primary audit surface and recovery source. All state transitions are recorded. In the event of data loss in `dsl_active_token` or `dsl_instance_data`, the journey log can be replayed to reconstruct current state.

```sql
-- Last known position for a token
SELECT to_node FROM dsl_journey_log
WHERE instance_id = $1
ORDER BY id DESC
LIMIT 1;
```

---

## v0.2 Operational additions (Tranche 7)

### Metrics

`RuntimeEngine` now carries an internal `RuntimeMetrics` struct (12 atomic counters).

**Reading metrics in code:**
```rust
let snap = engine.metrics().snapshot(); // MetricsSnapshot (serde::Serialize)
let text = engine.metrics().prometheus_text(); // Prometheus text format
```

**Exposing on a `/metrics` HTTP endpoint (Axum example):**
```rust
async fn metrics_handler(
    axum::extract::State(engine): axum::extract::State<Arc<RuntimeEngine>>,
) -> impl axum::response::IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        engine.metrics().prometheus_text(),
    )
}
```

**Counter descriptions:**

| Counter | Meaning |
|---|---|
| `bpmn_instances_started` | `start_instance()` called |
| `bpmn_instances_completed` | Instance reached end-event |
| `bpmn_instances_failed` | Instance set to `Failed` |
| `bpmn_instances_cancelled` | Instance set to `Cancelled` |
| `bpmn_events_processed` | Events dequeued and dispatched |
| `bpmn_verbs_invoked` | Registered verb handlers invoked |
| `bpmn_gateway_decisions` | Switch adaptor returned a decision |
| `bpmn_parallel_forks` | Parallel gateway spawned child tokens |
| `bpmn_joins_fired` | All branches arrived at a join |
| `bpmn_merge_conflicts` | Unresolvable merge conflict at a join |
| `bpmn_timer_events_fired` | `TimerFired` events processed |
| `bpmn_human_tasks_completed` | Human-task completion events |

### PostgresJourneyStore startup

Enable the `postgres` feature in your dependency:
```toml
bpmn-runtime = { path = "...", features = ["postgres"] }
```

Create the pool and store at startup:
```rust
use bpmn_runtime::PostgresJourneyStore;
use sqlx::PgPool;

let pool = PgPool::connect(&std::env::var("DATABASE_URL")?).await?;
let store = Arc::new(PostgresJourneyStore::new(pool));
let engine = RuntimeEngine::new(store, spec, verb_registry, switch_adaptor);
```

Requires the schema migration:
```bash
psql -d data_designer -f rust/migrations/20260521_dsl_journey_runtime.sql
```

### Retention policy

```rust
use bpmn_runtime::RetentionPolicy;

let policy = RetentionPolicy {
    archive_after_days: 90,
    cold_storage_after_years: 7,
};

// Find candidates (PostgresJourneyStore implements this; InMemory is a no-op)
let candidates = store.find_archivable_instances(&policy).await?;
for id in candidates {
    let rows = store.archive_instance_log(id).await?;
    tracing::info!(%id, rows, "archived journey log");
}
```

### Alerting thresholds

Prometheus alert rules (paste into your `alert.rules.yaml`):

```yaml
groups:
  - name: bpmn_runtime
    rules:
      - alert: HighInstanceFailureRate
        expr: |
          rate(bpmn_instances_failed[5m])
          / rate(bpmn_instances_started[5m]) > 0.05
        for: 2m
        labels:
          severity: warning
        annotations:
          summary: "Instance failure rate > 5%"

      - alert: EventQueueDepth
        expr: |
          (SELECT COUNT(*) FROM dsl_event_queue WHERE claimed_at IS NULL) > 1000
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Event queue depth > 1000"

      - alert: MergeConflictSpike
        expr: rate(bpmn_merge_conflicts[5m]) > 0
        for: 0m
        labels:
          severity: critical
        annotations:
          summary: "Merge conflicts detected — process definitions may need :merge clauses"
```

### Performance smoke test

```bash
# Run the 100-instance in-memory smoke test (normally ignored)
cargo test -p bpmn-test-harness -- --include-ignored smoke_100

# Run all perf tests
cargo test -p bpmn-test-harness -- --include-ignored
```

Expected: < 500 ms for 100 sequential instances with InMemoryJourneyStore.
SLA: < 30 seconds (hard assert in the test).
