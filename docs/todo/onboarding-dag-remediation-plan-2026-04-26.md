# Onboarding DAG Remediation Plan — 2026-04-26

> **Source review:** `docs/todo/onboarding-dag-deep-review-2026-04-26.md`
> **Predecessor:** v1.3 catalogue platform (CODE COMPLETE, 2026-04-25)
> **Target:** Tranche 4 — close the four-layer chain (Deal → CBU → Services → Resources) and backfill GatePipeline wiring across 24 orphan-by-pipeline slots.
>
> **Total scope:** 4 architectural slices (R1-R4) + 3 mechanical wiring slices (R5-R7).
> **Total sizing:** ~3-4 weeks of focused work; ~1100 LOC YAML + 3 migrations + ~73 verb edits.

---

## Strategic framing

The deep review identified two distinct problem classes:

1. **Architectural gaps** — the four-layer chain has missing layers (Layer 4 absent, Layer 3 stateless, Booking Principal tollgate unmodelled, Deal→CBU pivot indirect).
2. **Wiring gaps** — 24 of 44 stateful slots use [VERB-ONLY] verbs (no `transition_args:`), bypassing GatePipeline.

**Sequencing principle:** land architecture before mechanical backfill. Wiring 73 verbs against an incomplete model produces partial enforcement of the wrong shape — when Layer 4 lands later, those wirings need re-audit. Build the model end-to-end first; the wiring backfill becomes a single mechanical pass at the end.

**Slice independence:** R1/R2/R3 are independent (different DAG files, different tables, different verb domains). R4 is trivial and lands after R1-R3 are stable. R5/R6/R7 each ship as one PR per workspace once R1-R4 are in.

---

## Slice R1 — Layer 4 DAG (Lifecycle Resources)

### Scope

Introduce a new `lifecycle_resources_dag.yaml` workspace that models BNY-specific application instances and their binding to product services. Closes the gap "no layer separating *what generic capability* (custody-settlement) from *which BNY system* (CCC-Settlement-2.7-prod-eu)".

### Slot model

```yaml
workspace: lifecycle_resources
dag_id: lifecycle_resources_dag
slots:

  - id: workspace_root
    stateless: true

  - id: application
    stateless: true
    # Registry entry — id, name, vendor, owner_team, environment.
    # Lifecycle is at the *instance* level; the application itself
    # is just a catalogue card.

  - id: application_instance
    stateless: false
    state_machine:
      id: application_instance_lifecycle
      states:
        - { id: PROVISIONED }   # entry
        - { id: ACTIVE }
        - { id: MAINTENANCE_WINDOW }
        - { id: DEGRADED }
        - { id: OFFLINE }
        - { id: DECOMMISSIONED }   # terminal
      transitions:
        - { from: PROVISIONED,        to: ACTIVE,             progression_verbs: [application-instance.activate] }
        - { from: ACTIVE,             to: MAINTENANCE_WINDOW, progression_verbs: [application-instance.enter-maintenance] }
        - { from: MAINTENANCE_WINDOW, to: ACTIVE,             progression_verbs: [application-instance.exit-maintenance] }
        - { from: ACTIVE,             to: DEGRADED,           progression_verbs: ['(backend: health-check signal)'] }
        - { from: DEGRADED,           to: ACTIVE,             progression_verbs: ['(backend: health-check signal)'] }
        - { from: ACTIVE,             to: OFFLINE,            progression_verbs: [application-instance.take-offline] }
        - { from: OFFLINE,            to: ACTIVE,             progression_verbs: [application-instance.bring-online] }
        - { from: [ACTIVE, OFFLINE, MAINTENANCE_WINDOW, DEGRADED], to: DECOMMISSIONED, progression_verbs: [application-instance.decommission] }

  - id: capability_binding
    stateless: false
    parent_slot:
      workspace: lifecycle_resources
      slot: application_instance
      join:
        via: capability_bindings
        parent_fk: application_instance_id
        child_fk: id
    state_machine:
      id: capability_binding_lifecycle
      states:
        - { id: DRAFT }    # entry
        - { id: PILOT }
        - { id: LIVE }
        - { id: DEPRECATED }
        - { id: RETIRED }   # terminal
      transitions:
        - { from: DRAFT,      to: PILOT,      progression_verbs: [capability-binding.start-pilot] }
        - { from: PILOT,      to: LIVE,       progression_verbs: [capability-binding.promote-live] }
        - { from: PILOT,      to: DRAFT,      progression_verbs: [capability-binding.abort-pilot] }
        - { from: LIVE,       to: DEPRECATED, progression_verbs: [capability-binding.deprecate] }
        - { from: DEPRECATED, to: RETIRED,    progression_verbs: [capability-binding.retire] }
    # Cascade: if parent application_instance DECOMMISSIONED, all
    # bindings forced to RETIRED.
    state_dependency:
      cascade_on:
        - { parent_state: DECOMMISSIONED, child_default: RETIRED }
```

### Cross-workspace constraints (added to existing DAGs)

```yaml
# In cbu_dag.yaml — service_consumption.activate requires LIVE binding.
# (Added once R2 lands and product_service_taxonomy is stateful;
#  staged for R1+R2 joint slice or R4.)
cross_workspace_constraints:
  - id: service_consumption_active_requires_live_binding
    source:
      workspace: lifecycle_resources
      slot: capability_binding
      state: LIVE
      predicate: { ... join via cbu→service→capability_binding ... }
    target:
      workspace: cbu
      slot: service_consumption
      transition: { from: provisioned, to: active }
    severity: error
```

### Schema migration

`rust/migrations/20260427_lifecycle_resources_workspace.sql`:

```sql
-- Lifecycle Resources workspace (Tranche 4 R1).
-- Models BNY application instances and their binding to product services.

BEGIN;

CREATE TABLE IF NOT EXISTS "ob-poc".applications (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    name varchar(255) NOT NULL,
    vendor varchar(255),
    owner_team varchar(255),
    description text,
    created_at timestamptz DEFAULT now(),
    updated_at timestamptz DEFAULT now(),
    UNIQUE (name)
);

CREATE TABLE IF NOT EXISTS "ob-poc".application_instances (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    application_id uuid NOT NULL REFERENCES "ob-poc".applications(id),
    environment varchar(50) NOT NULL,           -- prod-eu / prod-us / uat / dev
    instance_label varchar(255) NOT NULL,
    lifecycle_status varchar(40) NOT NULL DEFAULT 'PROVISIONED'
        CHECK (lifecycle_status IN ('PROVISIONED','ACTIVE','MAINTENANCE_WINDOW','DEGRADED','OFFLINE','DECOMMISSIONED')),
    last_health_check_at timestamptz,
    health_check_status varchar(20),            -- healthy / degraded / failed
    decommissioned_at timestamptz,
    notes text,
    created_at timestamptz DEFAULT now(),
    updated_at timestamptz DEFAULT now(),
    UNIQUE (application_id, environment, instance_label)
);

CREATE TABLE IF NOT EXISTS "ob-poc".capability_bindings (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    application_instance_id uuid NOT NULL REFERENCES "ob-poc".application_instances(id),
    service_id uuid NOT NULL,                    -- FK to product_services once R2 lands
    binding_status varchar(20) NOT NULL DEFAULT 'DRAFT'
        CHECK (binding_status IN ('DRAFT','PILOT','LIVE','DEPRECATED','RETIRED')),
    pilot_started_at timestamptz,
    promoted_live_at timestamptz,
    deprecated_at timestamptz,
    retired_at timestamptz,
    notes text,
    created_at timestamptz DEFAULT now(),
    updated_at timestamptz DEFAULT now(),
    UNIQUE (application_instance_id, service_id)
);

CREATE INDEX idx_capability_bindings_status ON "ob-poc".capability_bindings(binding_status);
CREATE INDEX idx_application_instances_status ON "ob-poc".application_instances(lifecycle_status);

COMMIT;
```

### New verbs (~14)

`rust/config/verbs/application.yaml`, `rust/config/verbs/application-instance.yaml`, `rust/config/verbs/capability-binding.yaml`:

| Verb FQN | Behavior | transition_args |
|----------|----------|-----------------|
| application.register | crud | — |
| application.update | crud | — |
| application.decommission | crud | — |
| application-instance.provision | crud | — |
| application-instance.activate | crud | entity_id: instance-id, target_workspace: lifecycle_resources, target_slot: application_instance |
| application-instance.enter-maintenance | crud | (gated) |
| application-instance.exit-maintenance | crud | (gated) |
| application-instance.take-offline | crud | (gated) |
| application-instance.bring-online | crud | (gated) |
| application-instance.decommission | crud | (gated) |
| capability-binding.draft | crud | — |
| capability-binding.start-pilot | crud | (gated) |
| capability-binding.promote-live | crud | (gated) |
| capability-binding.abort-pilot | crud | (gated) |
| capability-binding.deprecate | crud | (gated) |
| capability-binding.retire | crud | (gated) |

### Files

- **NEW** `rust/config/sem_os_seeds/dag_taxonomies/lifecycle_resources_dag.yaml` (~350 LOC)
- **NEW** `rust/migrations/20260427_lifecycle_resources_workspace.sql` (~80 LOC)
- **NEW** `rust/config/verbs/application.yaml` (~120 LOC, 3 verbs)
- **NEW** `rust/config/verbs/application-instance.yaml` (~250 LOC, 6 verbs)
- **NEW** `rust/config/verbs/capability-binding.yaml` (~220 LOC, 6 verbs)
- **EDIT** `rust/crates/dsl-runtime/src/cross_workspace/slot_state.rs` — add slot resolutions for application_instance / capability_binding (PostgresSlotStateProvider dispatch table, +6 rows)
- **EDIT** `rust/config/packs/` — new pack `lifecycle-resources.yaml` for the workspace (~40 LOC)

### Acceptance criteria

- [ ] `cargo run -p xtask --quiet -- reconcile validate` clean (0 errors / 0 warnings).
- [ ] `cargo run -p xtask --quiet -- verbs check` and `verbs lint` clean.
- [ ] Schema applies: `psql -d data_designer -f migrations/20260427_lifecycle_resources_workspace.sql && cargo sqlx prepare --workspace`.
- [ ] Pack discovery test: workspace appears in `WorkspaceKind::LifecycleResources`.
- [ ] All 14 verbs invocable via `cargo x harness`.

### LOC + time estimate

~1,000 LOC across 5 new files + 1 migration. **Time: 1.5 weeks** (DAG design 2 days, migration 1 day, verbs 3 days, runtime wiring 1 day, test/iterate 2 days).

### Dependencies

None (independent slice). Can run in parallel with R2, R3.

### Risk + mitigation

- **Risk:** capability_binding referencing service_id before R2 lands (product_services table doesn't exist yet).
  *Mitigation:* declare service_id as plain uuid initially; add FK constraint in R2's migration.

---

## Slice R2 — Elevate product_service_taxonomy_dag to stateful

### Scope

Add lifecycle to `service` and `service_version` slots in `product_service_taxonomy_dag`. Pattern after `attribute_def_lifecycle` (ungoverned → draft → active → deprecated → retired). Enables cross_workspace_constraints from cbu.service_consumption to product_maintenance.service.

### Slot edits

Replace stateless `service` slot with:

```yaml
- id: service
  stateless: false
  state_machine:
    id: service_lifecycle
    states:
      - { id: ungoverned }   # entry — discovered but not yet governed
      - { id: draft }
      - { id: active }
      - { id: deprecated }
      - { id: retired }      # terminal
    transitions:
      - { from: ungoverned, to: draft,      progression_verbs: [service.define] }
      - { from: draft,      to: active,     progression_verbs: ['(backend: changeset.publish includes this service)'] }
      - { from: active,     to: draft,      progression_verbs: [service.propose-revision] }
      - { from: active,     to: deprecated, progression_verbs: [service.deprecate] }
      - { from: deprecated, to: retired,    progression_verbs: [service.retire] }

- id: service_version
  stateless: false
  parent_slot:
    workspace: product_maintenance
    slot: service
    join:
      via: service_versions
      parent_fk: service_id
      child_fk: id
  state_machine:
    id: service_version_lifecycle
    states:
      - { id: drafted }      # entry
      - { id: reviewed }
      - { id: published }
      - { id: superseded }
      - { id: retired }      # terminal
    transitions:
      - { from: drafted,    to: reviewed,   progression_verbs: [service-version.submit-for-review] }
      - { from: reviewed,   to: published,  progression_verbs: [service-version.publish] }
      - { from: published,  to: superseded, progression_verbs: ['(backend: new published version of same service)'] }
      - { from: [published, superseded], to: retired, progression_verbs: [service-version.retire] }
```

### Schema migration

`rust/migrations/20260428_service_lifecycle.sql`:

```sql
BEGIN;

ALTER TABLE "ob-poc".product_services
  ADD COLUMN IF NOT EXISTS lifecycle_status varchar(20)
    DEFAULT 'ungoverned'
    CHECK (lifecycle_status IN ('ungoverned','draft','active','deprecated','retired'));

CREATE TABLE IF NOT EXISTS "ob-poc".service_versions (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    service_id uuid NOT NULL REFERENCES "ob-poc".product_services(id),
    version varchar(20) NOT NULL,
    lifecycle_status varchar(20) NOT NULL DEFAULT 'drafted'
        CHECK (lifecycle_status IN ('drafted','reviewed','published','superseded','retired')),
    spec jsonb,
    drafted_at timestamptz DEFAULT now(),
    reviewed_at timestamptz,
    published_at timestamptz,
    superseded_at timestamptz,
    retired_at timestamptz,
    notes text,
    created_at timestamptz DEFAULT now(),
    updated_at timestamptz DEFAULT now(),
    UNIQUE (service_id, version)
);

CREATE INDEX idx_service_versions_status ON "ob-poc".service_versions(lifecycle_status);

-- Backfill: existing services that exist → ungoverned.
-- Tranche 4 R6 (governance pass) will manually progress them via changesets.

COMMIT;
```

### New verbs (~12)

`rust/config/verbs/service.yaml` + `rust/config/verbs/service-version.yaml`:

| Verb FQN | transition_args |
|----------|-----------------|
| service.define | entity_id: service-id, target_workspace: product_maintenance, target_slot: service |
| service.propose-revision | (gated) |
| service.deprecate | (gated) |
| service.retire | (gated) |
| service-version.draft | — |
| service-version.submit-for-review | (gated, target_slot: service_version) |
| service-version.publish | (gated) |
| service-version.retire | (gated) |
| service-version.update | crud (no transition_args) |
| service-version.compare | crud read-only |
| service.list-versions | crud read-only |
| service.show | crud read-only |

### Cross-workspace constraint (added to cbu_dag.yaml)

```yaml
- id: service_consumption_requires_active_service
  source:
    workspace: product_maintenance
    slot: service
    state: active
  target:
    workspace: cbu
    slot: service_consumption
    transition: { from: proposed, to: provisioned }
  severity: error
  message: "Cannot provision a service that isn't active in the catalogue"
```

### Files

- **EDIT** `rust/config/sem_os_seeds/dag_taxonomies/product_service_taxonomy_dag.yaml` (~150 LOC delta — replace 2 stateless slots with stateful)
- **EDIT** `rust/config/sem_os_seeds/dag_taxonomies/cbu_dag.yaml` (+15 LOC — new constraint)
- **NEW** `rust/migrations/20260428_service_lifecycle.sql` (~50 LOC)
- **NEW** `rust/config/verbs/service.yaml` (~180 LOC, 4 lifecycle + 2 read verbs)
- **NEW** `rust/config/verbs/service-version.yaml` (~200 LOC, 6 verbs)
- **EDIT** `rust/crates/dsl-runtime/src/cross_workspace/slot_state.rs` — +2 dispatch rows (service, service_version)
- **EDIT** `rust/config/packs/product-service-taxonomy.yaml` — change pack from read-only to read-write for governance verbs (~10 LOC)

### Acceptance criteria

- [ ] Validator clean.
- [ ] Migration applies; existing product_services rows backfill to `lifecycle_status='ungoverned'`.
- [ ] cbu_operationally_active aggregate includes the new "active service" check.
- [ ] Test: cannot service-consumption.provision a service whose lifecycle_status is ungoverned (Mode A blocking fires).

### LOC + time estimate

~600 LOC. **Time: 1 week** (DAG edit 1 day, migration 1 day, verbs 2 days, constraint wiring + test 1 day, e2e validation 1 day).

### Dependencies

None of R1/R3. Slight ordering with R4 (R4 introduces another constraint on service_consumption — should land together).

### Risk + mitigation

- **Risk:** governance pack changes break existing read-only navigation.
  *Mitigation:* split product_service_taxonomy into two packs: `product-explorer` (read-only, current behaviour) and `product-governance` (write, new). Governance pack gated behind admin AgentMode.

---

## Slice R3 — Booking Principal clearance lifecycle

### Scope

Adam named BAC + KYC + Booking Principals as the three deal tollgates. BAC and KYC are state-machined; BP is a stateless slot in instrument_matrix_dag. Add a clearance lifecycle.

### Decision: dedicated DAG vs add to deal_dag

**Recommendation: add slot to deal_dag** (not a separate workspace).
- Booking Principal clearance is *part of the commercial onboarding tollgate*, not a standalone workspace.
- Pattern matches BAC_APPROVAL state on deal itself (commercial gating).
- Avoids a 4-slot mini-workspace overhead.

### Slot model (added to deal_dag.yaml)

```yaml
- id: booking_principal_clearance
  stateless: false
  state_machine:
    id: booking_principal_clearance_lifecycle
    states:
      - { id: PENDING }   # entry
      - { id: SCREENING }
      - { id: APPROVED }
      - { id: REJECTED }
      - { id: ACTIVE }
      - { id: SUSPENDED }
      - { id: REVOKED }   # terminal
    transitions:
      - { from: PENDING,    to: SCREENING, progression_verbs: [booking-principal-clearance.start-screening] }
      - { from: SCREENING,  to: APPROVED,  progression_verbs: [booking-principal-clearance.approve] }
      - { from: SCREENING,  to: REJECTED,  progression_verbs: [booking-principal-clearance.reject] }
      - { from: REJECTED,   to: PENDING,   progression_verbs: [booking-principal-clearance.reopen] }
      - { from: APPROVED,   to: ACTIVE,    progression_verbs: [booking-principal-clearance.activate] }
      - { from: ACTIVE,     to: SUSPENDED, progression_verbs: [booking-principal-clearance.suspend] }
      - { from: SUSPENDED,  to: ACTIVE,    progression_verbs: [booking-principal-clearance.reinstate] }
      - { from: [APPROVED, ACTIVE, SUSPENDED], to: REVOKED, progression_verbs: [booking-principal-clearance.revoke] }
```

### Cross-workspace constraint (added to deal_dag.yaml)

Replace existing `deal.KYC_CLEARANCE → CONTRACTED` constraint with a richer compound:

```yaml
# Existing kept:
- id: deal_contracted_requires_kyc_approved
  source: { workspace: kyc, slot: kyc_case, state: APPROVED }
  target: { workspace: deal, slot: deal, transition: { from: KYC_CLEARANCE, to: CONTRACTED } }
  severity: error

# NEW:
- id: deal_contracted_requires_bp_approved
  source: { workspace: deal, slot: booking_principal_clearance, state: [APPROVED, ACTIVE] }
  target: { workspace: deal, slot: deal, transition: { from: KYC_CLEARANCE, to: CONTRACTED } }
  severity: error
  message: "Cannot contract deal without Booking Principal clearance approved"
```

This makes deal CONTRACTED require BOTH KYC + BP (compound gating per Adam's tollgate framing).

### Schema migration

`rust/migrations/20260429_booking_principal_clearance.sql`:

```sql
BEGIN;

CREATE TABLE IF NOT EXISTS "ob-poc".booking_principal_clearances (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    booking_principal_id uuid NOT NULL,           -- existing booking_principals or entity FK
    deal_id uuid REFERENCES "ob-poc".deals(id),   -- nullable: clearance is principal-scoped, optionally deal-scoped
    cbu_id uuid REFERENCES "ob-poc".cbus(id),
    clearance_status varchar(20) NOT NULL DEFAULT 'PENDING'
        CHECK (clearance_status IN ('PENDING','SCREENING','APPROVED','REJECTED','ACTIVE','SUSPENDED','REVOKED')),
    screening_started_at timestamptz,
    approved_at timestamptz,
    rejected_at timestamptz,
    rejection_reason text,
    activated_at timestamptz,
    revoked_at timestamptz,
    notes text,
    created_at timestamptz DEFAULT now(),
    updated_at timestamptz DEFAULT now(),
    UNIQUE (booking_principal_id, deal_id, cbu_id)
);

CREATE INDEX idx_bp_clearance_status ON "ob-poc".booking_principal_clearances(clearance_status);
CREATE INDEX idx_bp_clearance_deal ON "ob-poc".booking_principal_clearances(deal_id);

COMMIT;
```

### New verbs (~7)

`rust/config/verbs/booking-principal-clearance.yaml`:

| Verb FQN | transition_args |
|----------|-----------------|
| booking-principal-clearance.create | crud (no transition_args — entry state) |
| booking-principal-clearance.start-screening | gated |
| booking-principal-clearance.approve | gated |
| booking-principal-clearance.reject | gated |
| booking-principal-clearance.reopen | gated |
| booking-principal-clearance.activate | gated |
| booking-principal-clearance.suspend | gated |
| booking-principal-clearance.reinstate | gated |
| booking-principal-clearance.revoke | gated |

### Files

- **EDIT** `rust/config/sem_os_seeds/dag_taxonomies/deal_dag.yaml` (+80 LOC for slot + constraint)
- **NEW** `rust/migrations/20260429_booking_principal_clearance.sql` (~40 LOC)
- **NEW** `rust/config/verbs/booking-principal-clearance.yaml` (~280 LOC, 9 verbs)
- **EDIT** `rust/crates/dsl-runtime/src/cross_workspace/slot_state.rs` — +1 dispatch row
- **EDIT** `rust/config/packs/deal-lifecycle.yaml` — add 9 new BP verbs to allowed_verbs

### Acceptance criteria

- [ ] Validator clean.
- [ ] Migration applies.
- [ ] Test: deal cannot transition KYC_CLEARANCE → CONTRACTED unless BP clearance is APPROVED or ACTIVE (Mode A blocking).
- [ ] Test: `booking-principal-clearance.suspend` cascades correctly (deal in OFFBOARDING is informed).

### LOC + time estimate

~400 LOC. **Time: 4 days** (DAG edit 1 day, migration 1 day, verbs 1 day, test/integration 1 day).

### Dependencies

None of R1/R2. Independent slice.

### Risk + mitigation

- **Risk:** existing deals already in CONTRACTED have no booking_principal_clearance row → backfill needed.
  *Mitigation:* migration includes data backfill — for every existing CONTRACTED deal, INSERT a clearance row with status=ACTIVE (assume historical clearance was implicit).

---

## Slice R4 — Deal → CBU subscription constraint (trivial)

### Scope

Add a single cross_workspace_constraint expressing "CBU cannot start consuming services until a Deal is CONTRACTED for that client".

### Edit (cbu_dag.yaml)

```yaml
cross_workspace_constraints:
  # ... existing constraints ...

  - id: service_consumption_requires_deal_contracted
    source:
      workspace: deal
      slot: deal
      state: [CONTRACTED, ONBOARDING, ACTIVE]
      predicate:
        # Match on client_group_id — the deal's client_group must equal
        # the CBU's client_group.
        join: { via: cbus, src_col: client_group_id, tgt_col: client_group_id }
    target:
      workspace: cbu
      slot: service_consumption
      transition: { from: proposed, to: provisioned }
    severity: error
    message: "CBU cannot subscribe to services until a Deal is CONTRACTED for the client"
```

### Files

- **EDIT** `rust/config/sem_os_seeds/dag_taxonomies/cbu_dag.yaml` (+20 LOC)

### Acceptance criteria

- [ ] Validator clean.
- [ ] Test: service-consumption.provision blocked when no deal exists for the client_group.
- [ ] Test: service-consumption.provision succeeds once a deal in CONTRACTED state exists.

### LOC + time estimate

~20 LOC. **Time: 2 hours** (edit + test).

### Dependencies

R2 should land first (so service_consumption_requires_active_service constraint is in place — both compose). Trivial otherwise.

### Risk + mitigation

- **Risk:** existing CBUs in operational state with no deal → constraint flips them to non-progressable.
  *Mitigation:* constraint is on the *transition* (proposed → provisioned), not on the resting state. Existing CBUs in `active` service_consumption are unaffected.

---

## Slice R5 — P1 verb wiring backfill (12 verbs)

### Scope

Add `transition_args:` to verbs that fire transitions referenced by Mode A constraints or Mode B aggregates. These are runtime-gating-critical.

### Verbs to edit

| Verb | File | target_slot | Why critical |
|------|------|-------------|--------------|
| trading-profile.submit | trading-profile.yaml | im.trading_profile | Target of `mandate_requires_validated_cbu` Mode A |
| trading-profile.approve | trading-profile.yaml | im.trading_profile | Feeds CBU operationally_active Mode B |
| trading-profile.reject | trading-profile.yaml | im.trading_profile | (same) |
| trading-profile.go-live | trading-profile.yaml | im.trading_profile | (same) |
| trading-profile.suspend | trading-profile.yaml | im.trading_profile | (same) |
| trading-profile.reactivate | trading-profile.yaml | im.trading_profile | (same) |
| evidence.verify | kyc/evidence.yaml | cbu.cbu_evidence | Feeds cbu_operationally_active |
| evidence.reject | kyc/evidence.yaml | cbu.cbu_evidence | (same) |
| kyc-case.update-status | kyc/kyc-case.yaml | kyc.kyc_case | State consulted by CBU + Deal Mode A |
| kyc-case.close | kyc/kyc-case.yaml | kyc.kyc_case | (same) |
| deal.update-onboarding-status | deal.yaml | deal.deal_onboarding_request | Feeds deal.ONBOARDING → ACTIVE precondition |
| service-resource.activate | (new in R1, gated already) | — | Already gated as part of R1 |

### Edit pattern (example)

```yaml
trading-profile.yaml:
  trading-profile:
    submit:
      description: Submit trading profile for approval
      behavior: crud
      args:
        - name: profile-id
          type: uuid
          required: true
      # ... other fields ...
      transition_args:
        entity_id_arg: profile-id
        target_workspace: instrument_matrix
        target_slot: trading_profile
```

### Files

- **EDIT** `rust/config/verbs/trading-profile.yaml` (+30 LOC for 6 transition_args blocks)
- **EDIT** `rust/config/verbs/kyc/evidence.yaml` (+10 LOC)
- **EDIT** `rust/config/verbs/kyc/kyc-case.yaml` (+10 LOC)
- **EDIT** `rust/config/verbs/deal.yaml` (+5 LOC for deal.update-onboarding-status)

### Acceptance criteria

- [ ] `cargo run -p xtask --quiet -- reconcile validate` clean.
- [ ] `grep -c 'transition_args:' config/verbs/trading-profile.yaml` returns 8 (was 2).
- [ ] Integration test: firing `trading-profile.submit` on an unvalidated CBU is rejected by GatePipeline (was previously [VERB-ONLY], so the gate didn't fire even though the constraint existed).
- [ ] Cascade test: kyc-case approval triggers any cascading transitions properly.

### LOC + time estimate

~60 LOC across 4 files. **Time: 4 hours** (mechanical edits + verb-test re-run + harness validation).

### Dependencies

None (pure additive YAML — backwards compatible).

### Risk + mitigation

- **Risk:** previously [VERB-ONLY] verbs becoming [GATED] may surface latent constraint violations in test data.
  *Mitigation:* run on staging DB first; fix or grandfather any non-conforming rows before promoting to prod.

---

## Slice R6 — KYC verb wiring (~13 verbs)

### Scope

Add `transition_args:` to KYC progression verbs. Eliminates the per-workspace inconsistency where KYC bypasses GatePipeline entirely. Enables future Mode A constraints inbound to KYC.

### Verbs to edit

| Verb | target_slot |
|------|-------------|
| kyc-case.escalate | kyc.kyc_case |
| kyc-case.reopen | kyc.kyc_case |
| screening.run | kyc.screening |
| screening.complete | kyc.screening |
| screening.review-hit | kyc.screening |
| ubo-registry.verify | kyc.kyc_ubo_registry |
| ubo-registry.approve | kyc.kyc_ubo_registry |
| ubo-registry.reject | kyc.kyc_ubo_registry |
| ubo-registry.expire | kyc.kyc_ubo_registry |
| red-flag.escalate | kyc.red_flag |
| red-flag.resolve | kyc.red_flag |
| red-flag.waive | kyc.red_flag |
| red-flag.update-rating | kyc.red_flag |

### Files

- **EDIT** `rust/config/verbs/kyc/kyc-case.yaml`
- **EDIT** `rust/config/verbs/screening.yaml`
- **EDIT** `rust/config/verbs/kyc/ubo-registry.yaml`
- **EDIT** `rust/config/verbs/kyc/red-flag.yaml`

### Acceptance criteria

- [ ] Validator clean.
- [ ] `grep -rc 'transition_args:' config/verbs/kyc/` returns >= 10 (was 0).
- [ ] Integration test: KYC verbs now intercepted by GatePipeline (verifiable via `tracing::debug!` events at gate-check stage).

### LOC + time estimate

~70 LOC across 4 files. **Time: 3 hours**.

### Dependencies

None.

### Risk + mitigation

- Same as R5.

---

## Slice R7 — Long-tail verb wiring backfill (~48 verbs)

### Scope

Mechanical backfill of remaining [VERB-ONLY] verbs. Operationally optional but completes coverage.

### Workspaces and verb counts

| File | Verb count | New transition_args |
|------|-----------|---------------------|
| billing.yaml | 8 | billing.{activate-profile, suspend-profile, close-profile, calculate-period, review-period, approve-period, generate-invoice, dispute-period} |
| deal.yaml | 3 | deal.{update-document-status, update-ubo-assessment, remove-product} |
| settlement-chain.yaml (or im subdomain) | 5 | settlement-chain.{add-hop, request-review, go-live, suspend, deactivate-chain} |
| trade-gateway.yaml | 3 | trade-gateway.{enable-gateway, activate-gateway, retire-gateway} |
| service-intent.yaml | 3 | service-intent.{suspend, resume, cancel} |
| delivery.yaml | 2 | delivery.{start, complete} |
| reconciliation.yaml | 2 | reconciliation.{activate, retire} |
| collateral-management.yaml | 2 | collateral-management.{activate, terminate} |
| sem-reg/changeset.yaml | 4 | changeset.{submit, enter-review, approve, reject} |
| sem-reg/governance.yaml | already partial | (R5 covers) |
| attribute.yaml | 3 | attribute.{define, retire, propose-revision} |
| derivation.yaml | 2 | derivation.{activate, recompute-stale} |
| service-resource.yaml | 3 | service-resource.{sync-definitions, check-attribute-gaps, mark-complete} |
| phrase.yaml | 7 | phrase.{check-collision, check-quality, submit-for-review, request-refinement, defer, propose, reactivate} |
| entity.yaml | 2 | entity.{identify, verify} |
| ubo-registry.yaml | 1 | ubo-registry.promote-to-ubo |
| registry/share-class.yaml | 5 | share-class.{launch, soft-close, hard-close, lift-hard-close, begin-winddown} |
| registry/investor.yaml | 1 | investor.request-documents |
| cbu.yaml | 2 | cbu.{verify-evidence, attach-evidence} |

### Files (~16 verb files edited)

### Acceptance criteria

- [ ] Validator clean.
- [ ] `grep -rc 'transition_args:' config/verbs/ | wc -l` reflects new total (~135 declarations).
- [ ] All 24 originally-orphan-by-pipeline slots now show at least one [GATED] inbound transition.
- [ ] Reconcile validator no new warnings.

### LOC + time estimate

~250 LOC across 16 files. **Time: 1.5 days** (mechanical edits + harness re-run + spot-test 5 verbs).

### Dependencies

R1, R2, R3 should land first (so the new verbs declared by those slices count toward the post-state).

### Risk + mitigation

- **Risk:** highest-volume slice; risk of YAML formatting errors.
  *Mitigation:* one PR per workspace (deal, kyc, im, semos, registry); CI verb-lint catches formatting before merge.

---

## Sequencing diagram

```
WEEK 1-2  ─┬── R1 (Layer 4 DAG)              ──── 1.5 weeks ────┐
           ├── R2 (Service lifecycle)         ──── 1 week ──────┤
           └── R3 (BP clearance)              ──── 4 days ──────┤
                                                                │
WEEK 3    ─── R4 (Deal→CBU constraint)        ── 2 hours ───────┤  ← all architectural in
                                                                │     R5/R6/R7 mechanical
WEEK 3-4  ─┬── R5 (P1 verbs)                  ── 4 hours ───────┤
           ├── R6 (KYC verbs)                 ── 3 hours ───────┤
           └── R7 (Long-tail wiring)          ── 1.5 days ──────┘

           Total: 3-4 weeks (one engineer, focused)
                  2-3 weeks (with R1+R2+R3 in parallel by 2-3 engineers)
```

### Parallelism opportunities

- R1, R2, R3 are independent (different DAG files, different migrations, different verb domains). Can run in parallel by 3 engineers.
- R5, R6, R7 are sequential after R1-R4 land but independent across workspaces — can split among engineers.
- R4 is gating on R2 only.

---

## Test strategy

### Per-slice tests

Each slice ships with:
- Validator green (`cargo x reconcile validate`)
- Verbs green (`cargo x verbs check && verbs lint`)
- Integration test demonstrating the new constraint/cascade fires correctly
- No regression in 353-case utterance harness

### End-to-end test (after R7)

Add a new e2e test: "Full Deal-to-Service-Consumption lifecycle":
1. Create deal in PROSPECT.
2. Progress deal through QUALIFYING → NEGOTIATING → BAC_APPROVAL → KYC_CLEARANCE → CONTRACTED.
3. Verify CBU.service_consumption.proposed → provisioned now succeeds (R4 + R2 constraints satisfied).
4. Verify capability_binding.LIVE constraint blocks if no application_instance.ACTIVE binding exists (R1 + R2 chain).
5. Suspend application_instance → verify cascade flips capability_binding to deprecated → CBU service_consumption suspended (R1 cascade + downstream Mode B).

Lives in `rust/tests/e2e_full_chain_lifecycle.rs`. ~300 LOC.

---

## Rollback strategy per slice

| Slice | Rollback |
|-------|----------|
| R1 | Revert YAML files; drop tables (no consumers if R7 hasn't landed); remove from slot_state.rs dispatch |
| R2 | Revert YAML; ALTER TABLE DROP COLUMN lifecycle_status; DROP TABLE service_versions |
| R3 | Revert YAML; DROP TABLE booking_principal_clearances |
| R4 | One-line YAML revert |
| R5/R6/R7 | YAML reverts (pure additive — `transition_args:` blocks can be removed cleanly) |

Each slice's migration is forward-only per project rules (CLAUDE.md "Migrations are forward-only"). Rollback above is for catastrophic failure only — normal reversion is a *new* migration that drops the table.

---

## Out of scope (explicit)

- **Multi-level recursive cascade execution.** v1.3 ships single-level cascade only (TODO in step_executor_bridge.rs). R1's capability_binding cascade only fires one level deep — full recursive cascade is a separate v1.4 architectural slice.
- **Backfilling DAG progression_verbs comments to match transition_args.** Cosmetic; tracked separately.
- **Replacing macro-expanded transitions with direct verb references.** Some DAG transitions reference macros (case.approve, screening-ops.full); these expand at runtime to underlying verbs which DO get gated. Macros remain in DAG progression_verbs as documentation.
- **Pack rebalancing.** Adding 73+ new verbs may push some packs over their allowed_verbs ceiling. R7 includes pack edits where strictly necessary; broader pack restructuring is separate.

---

## Observability deliverables (proposed bundled with R5)

To verify gate-pipeline coverage progress over time:

1. **Metric:** `gate_pipeline_dispatch_total{workspace, slot, verb_fqn}` — counter incremented on each verb dispatched through GatePipeline.
2. **Metric:** `verb_dispatch_total{workspace, verb_fqn, gated="true|false"}` — labels reflect whether transition_args triggered registry lookup.
3. **Dashboard:** "Verb wiring coverage" — per-workspace bar chart of % verbs that fired with `gated=true` over rolling 7d.

These let us see R5/R6/R7 progress in production without re-grepping YAML.

**Effort:** +1 day on top of R5 (Prometheus counter additions in `step_executor_bridge.rs` `pre_dispatch_gate_check`).

---

## Open decisions for Adam

Before kicking off:

1. **R1 scope: layer-4 DAG owner workspace.** Two options:
   - (a) Standalone `lifecycle_resources` workspace (proposed) — clean separation, requires new pack + workspace registration.
   - (b) Extend `instrument_matrix` workspace — fewer moving pieces, but conflates "what CBU consumes" with "which BNY system runs it".

2. **R3 scope: BP clearance attached to deal vs principal.** Two options:
   - (a) Per-deal-per-principal (proposed) — clearance is renewed for each deal involving that principal.
   - (b) Per-principal global, deals reference active clearance — clearance is a property of the principal entity, deals just check it.
   Option (a) is more conservative (matches existing per-deal KYC clearance pattern); option (b) is leaner but requires renewal logic.

3. **R7 scoping: ship as one big PR or 5 per-workspace PRs?** Per-workspace PRs are easier to review; one big PR has lower CI overhead. Recommend per-workspace.

4. **Engineer staffing:** if 2-3 engineers available, parallelize R1+R2+R3 in week 1-2; otherwise serial 3-4 weeks.

---

## What this plan does NOT commit to

- Specific calendar dates (depends on staffing decision in Open Decision #4).
- Frontend UI changes for new verbs (separate work — react components for capability_binding.{start-pilot, promote-live} etc.).
- Migration-of-existing-data scripts beyond what's in each migration's BEGIN/COMMIT block (R3 migration includes a backfill; others may or may not, decided per-slice during implementation).

---

**End of plan.** Next step: triage the 4 open decisions above; once resolved, kick off R1+R2+R3 in parallel.
