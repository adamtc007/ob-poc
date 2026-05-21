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
