# BpmnStart saga reaper — design

> **Status:** design — not yet implemented.
> **Scope:** close the orphan-instance window left open by the F.1d Pattern-B
> remediation of `bpmn.start`.
> **Owner:** three-plane refactor / BPMN integration.
> **Cross-refs:** `docs/todo/pattern-b-a1-remediation-ledger.md` §F.1 saga
> follow-ups, `rust/src/domain_ops/bpmn_lite_ops.rs` (BpmnStart docstring).

---

## 1. Problem

`bpmn.start` now satisfies the A1 invariant (no external effects inside the
inner transaction) by issuing the `StartProcess` gRPC call inside `pre_fetch`,
**before** the Sequencer opens its outer transaction. The returned
`instance_id` is threaded into the envelope's args and returned from `execute`
without any further external I/O. On the happy path, the outer tx commits
and the bpmn-server's live instance is reconciled with whatever rows the
runbook wrote that reference it.

The saga window this opens is unchanged from pre-F.1d — but it is now **the
only residual orphan risk in the Pattern-B migration**. The window:

1. `pre_fetch` fires `StartProcess` on the bpmn-server. A `process_instance`
   row is written to the bpmn-server's DB with a live correlation_id.
2. `execute` returns the instance_id.
3. Some **later** step in the same runbook fails, or the Sequencer aborts
   the outer transaction for any reason (TOCTOU drift, advisory-lock
   contention, panic).
4. The outer tx rolls back. Every DB write the runbook made in ob-poc is
   undone, **including whatever row recorded the instance_id**.
5. The bpmn-server still holds a live process instance with a correlation_id
   that no longer resolves to anything on the caller side. It continues to
   tick its VM, fire timers, ask for jobs, and generate events until it
   completes or errors out.

Net effect: **a ghost BPMN process, visible to operators only via direct DB
inspection of `process_instances`**, consuming worker time and possibly
triggering downstream effects (signals, HTTP calls from service tasks).

---

## 2. Correlation evidence we already have

- `StartRequest.correlation_id` is required (proto line 50) and is indexed
  on `process_instances` (migration 001 line 18: `idx_instances_correlation`).
- ob-poc passes `Uuid::now_v7()` as `correlation_id` in the current
  `BpmnStart::pre_fetch` (line 174 of `bpmn_lite_ops.rs`). This value is
  NOT persisted on the ob-poc side — which is actually fine for this design:
  the reaper doesn't need to look up correlation_ids, it just needs a way
  to ask "is this instance orphaned?".
- The bpmn-server's `Cancel` RPC exists (proto line 11, 73) and takes
  `(process_instance_id, reason)`.

**Design choice:** use a narrower, more reliable signal than correlation_id
tracking. See §3.

---

## 3. Reaper strategy

Two viable approaches; recommending (A).

### (A) Two-Phase Commit marker in ob-poc outbox — **recommended**

When `BpmnStart::pre_fetch` fires `StartProcess`, it ALSO stages an outbox
row in the inner transaction (via `TransactionScope`) with:

| Column | Value |
|--------|-------|
| `effect_kind` | `bpmn_start_commit` |
| `payload` | `{ "instance_id": "...", "correlation_id": "..." }` |
| `idempotency_key` | derived from `(correlation_id, instance_id)` |
| `status` | `pending` at write, promoted to `done` by the drainer post-commit |

The drainer consumer for `bpmn_start_commit` is a **no-op on the bpmn side**
— it just marks the row `done`. The row's presence after commit is the
proof that the outer transaction committed.

**Reaper logic** (runs in ob-poc-web or a xtask binary every N minutes):

```
for each process_instances row where updated_at < now() - reaper_grace:
    look up outbox row by idempotency_key = (correlation_id, instance_id)
    if outbox row exists AND status = 'done':
        # Happy path. Commit succeeded. Leave instance alone.
        continue
    if outbox row exists AND status = 'pending':
        # Drainer hasn't drained it yet. Not an orphan. Skip.
        continue
    if no outbox row AND instance age > reaper_grace:
        # Outer tx rolled back — outbox row was part of the rolled-back writes.
        # Instance is orphaned. Cancel it.
        bpmn_client.cancel(instance_id, "saga-reaper: outer tx rollback")
        log an audit row to ob-poc.bpmn_reaper_log for forensics
```

**Grace period:** 15 minutes by default. Tuned to be longer than the
Sequencer's longest single-runbook TTL plus drainer poll interval.

**Cross-DB coupling:** the reaper queries both bpmn-server's
`process_instances` and ob-poc's `outbox`. It does NOT try to join them in
SQL — it does an ob-poc `SELECT` per candidate instance. Expected candidate
count per cycle: low single digits in steady state, because committed
runbooks leave their `done` outbox row behind indefinitely (until drainer
TTL cleanup).

**Failure modes:**

- Reaper sees an instance that committed but the outbox `done` row was
  cleaned up before the reaper ran. Mitigation: reaper-specific outbox
  retention policy — keep `bpmn_start_commit` rows for at least
  `max(reaper_grace * 2, 24h)`. Alternatively: write the same marker to a
  dedicated `bpmn_start_commits` audit table outside the outbox lifecycle.
- Reaper cancel races with a legitimately-slow commit. Mitigation: grace
  period > longest plausible commit latency. Additionally: `Cancel` on an
  already-completed instance returns a structured error which the reaper
  logs as a no-op (expected race, not a correctness issue).
- bpmn-server temporarily unavailable — reaper retries on the next cycle.
  Instances accumulate but don't execute further, so the delta is bounded.

### (B) Query the bpmn-server's DB directly

Skip the outbox marker. Reaper queries
`bpmn-server.process_instances WHERE correlation_id NOT IN (SELECT ... FROM
ob-poc.<entity that stores instance_id>)`. Cheap to implement if every
caller records its instance_id, expensive to implement if each caller records
it in a different table — which is the current state.

Rejected because: no single ob-poc table stores bpmn instance_ids, and
retrofitting would push state out of the verb body (a new A1 violation
risk). Outbox marker is the cleanest transaction-scoped record.

---

## 4. Interface sketch

A standalone module that depends on both an ob-poc-side `sqlx::PgPool` and a
`BpmnClient`. Lives in ob-poc because all the policy (what counts as orphan,
grace period, cancel reason) is ob-poc-side.

```rust
// rust/src/bpmn_integration/saga_reaper.rs (NEW — prototype)

pub struct SagaReaperConfig {
    pub reaper_grace: Duration,          // e.g. 15 min
    pub cycle_interval: Duration,         // e.g. 5 min
    pub max_cancels_per_cycle: usize,     // rate limit safeguard
    pub cancel_reason_prefix: &'static str,
}

pub struct SagaReaper {
    obpoc_pool: PgPool,
    bpmn_pool: PgPool,          // direct read access to bpmn-server DB
    bpmn_client: BpmnClient,    // for Cancel RPC
    cfg: SagaReaperConfig,
    shutdown: Arc<Notify>,
}

impl SagaReaper {
    pub async fn run(self) {
        let mut ticker = tokio::time::interval(self.cfg.cycle_interval);
        loop {
            tokio::select! {
                _ = self.shutdown.notified() => break,
                _ = ticker.tick() => {
                    if let Err(e) = self.run_once().await {
                        tracing::warn!(error = %e, "saga reaper cycle failed");
                    }
                }
            }
        }
    }

    async fn run_once(&self) -> Result<ReaperCycleStats> {
        let candidates = self.fetch_stale_instances().await?;
        let mut cancelled = 0;
        for inst in candidates {
            if cancelled >= self.cfg.max_cancels_per_cycle { break; }
            match self.classify(&inst).await? {
                Classification::Committed => { /* skip */ }
                Classification::Pending => { /* skip */ }
                Classification::Orphaned => {
                    self.cancel_and_log(&inst).await?;
                    cancelled += 1;
                }
            }
        }
        Ok(ReaperCycleStats { cancelled, inspected: candidates.len() })
    }

    async fn fetch_stale_instances(&self) -> Result<Vec<InstanceSnapshot>> {
        // SELECT instance_id, correlation_id, updated_at
        //   FROM process_instances
        //   WHERE updated_at < now() - $1::interval
        // LIMIT 100
    }

    async fn classify(&self, inst: &InstanceSnapshot) -> Result<Classification> {
        // SELECT status FROM "ob-poc".outbox
        //   WHERE idempotency_key = $1 AND effect_kind = 'bpmn_start_commit'
        //   LIMIT 1
        // map: None → Orphaned; Some(Pending) → Pending; Some(Done) → Committed.
    }

    async fn cancel_and_log(&self, inst: &InstanceSnapshot) -> Result<()> {
        self.bpmn_client.cancel(inst.instance_id,
            format!("{}: outer tx rollback detected", self.cfg.cancel_reason_prefix))
            .await?;
        // INSERT INTO "ob-poc".bpmn_reaper_log
        //   (instance_id, correlation_id, cancelled_at, cancel_reason) VALUES (...)
    }
}

enum Classification { Committed, Pending, Orphaned }
```

---

## 5. Required follow-ups before implementation

1. **Outbox `effect_kind`** — add `bpmn_start_commit` to the
   `OutboxEffectKind` enum (ob-poc-types). Idempotent consumer that just
   marks `status=done`.
2. **BpmnStart wiring** — in `BpmnStart::execute`, write the outbox row
   via `TransactionScope` using the instance_id returned from `pre_fetch`.
   (This couples a second outbox write to the transaction scope — Cheap,
   same pattern as narrate + bpmn_signal + bpmn_cancel.)
3. **`bpmn_reaper_log` table** — new migration in ob-poc for forensics.
   Simple: `(instance_id uuid pk, correlation_id text, cancelled_at
   timestamptz, cancel_reason text, error text null)`.
4. **Retention policy** — the drainer's TTL cleanup must NOT remove
   `bpmn_start_commit` rows younger than `reaper_grace * 2`. Simplest:
   carve out `bpmn_start_commit` from the default retention rule and
   give it a 24h floor.
5. **Startup wiring** — `ob-poc-web::main` launches the reaper as a
   background task alongside the outbox drainer and
   `MaintenanceSpawnConsumer`. Graceful shutdown via the same `Notify`.
6. **Integration test** — spin up bpmn-lite, fire `bpmn.start`, force
   outer-tx rollback, wait `reaper_grace`, assert instance was cancelled
   and `bpmn_reaper_log` row written.
7. **Ops dashboard** — add `bpmn_saga_reaper_cancelled_total` counter to
   the existing Prometheus registry (next to outbox drainer metrics).

---

## 6. Deferred decisions

- **Cross-tenant reaper scope.** bpmn-server's `process_instances` has a
  `tenant_id` column. The reaper initially runs per-ob-poc-deployment
  (single tenant), so it filters to `tenant_id = 'default'`. Multi-tenant
  ob-poc deployments need a reaper instance per tenant, or a single reaper
  that iterates the tenant set. Out of scope for the initial prototype.
- **Panic / crash recovery.** If ob-poc-web crashes between the
  `StartProcess` gRPC call and the inner-tx outbox write, the instance is
  orphaned without any outbox trace. Reaper handles this case correctly
  (no outbox row → Orphaned), but the grace period must be long enough
  for the ob-poc-web process to restart and drain its in-flight runbooks.
  15 min is a comfortable upper bound.
- **Alternative: make `StartProcess` accept an idempotency key.** Would
  allow retries without creating duplicate instances. Proto change +
  bpmn-server change; not on the critical path for saga closure.

---

## 7. Acceptance criteria

The reaper is CLOSED-ready when:

- [ ] Unit tests cover all three `Classification` branches.
- [ ] Integration test above is green.
- [ ] L4 lint continues to pass (the reaper lives in
      `rust/src/bpmn_integration/`, not in a verb, so L4 does not apply
      — this is explicit in the L4 scope doc).
- [ ] `pattern-b-a1-remediation-ledger.md` §F.1 saga follow-ups row
      updated to **CLOSED** with a pointer to this doc + the
      implementation commit.
- [ ] `three-plane-v0.3-wiring-followon-2026-04-22.md` §F.1 row
      updated from "saga follow-up pending" to "saga reaper landed,
      orphan window closed".

---

## 8. Estimate

- Prototype (§4 skeleton + §5 items 1-3): **~2 days** of focused work.
- Production wiring (§5 items 4-7 + integration test): **~3 days**.
- Total: **~1 week**.

Not gating any other phase. Can proceed any time after Phase F.1d
(`BpmnStart` migration) lands — which it has, 2026-04-22.
