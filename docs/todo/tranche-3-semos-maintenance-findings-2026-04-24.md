# Tranche 3 — SemOsMaintenance Findings Report (2026-04-24)

> **Status:** CLOSED. First Tranche 3 workspace. Uses v1.3 conventions
> from day one (D-5 interleave directive).
>
> **Parent docs:**
> - `catalogue-platform-refinement-v1_3.md`
> - `tranche-2-cross-workspace-reconciliation-2026-04-24.md`
> - CLAUDE.md §SemOS Maintenance workspace
> - `docs/annex-sem-os.md`

---

## 1. Delivery summary

9 phases executed in ~45 min. Governance workspace is conceptually
smaller than operational workspaces — fewer aggregates, less
category gating, no CBU coupling.

| Phase | Deliverable | Effort |
|---|---|---|
| T3-S-1 (P.1) | SKIP — infra reused | 0 |
| T3-S-2-prep | Pack-delta check — clean (0 unresolved) | 2 min |
| T3-S-2 | `semos_maintenance_dag.yaml` (~650 LOC: 8-phase overall lifecycle, 5 stateful slots, 4 stateless, 5 cross-slot constraints, 2 prune rules) | ~25 min |
| T3-S-3 | 89 three-axis declarations (service-resource already fully declared from prior work; skipped; other 7 files retrofitted) | ~5 min |
| T3-S-4 | 3 tier raises: attribute.deprecate, governance.publish, governance.publish-batch | ~3 min |
| T3-S-5 | Runtime triage: 26 B1 + 5 B2 + 0 B3 = 31/31 (100%) | ~3 min |
| T3-S-9 | This document | ~7 min |

**Validator terminal state:** 673 / 1184 declared (+89 from R-7's
584), 0 structural errors, 0 well-formedness errors (3-axis /
pack-hygiene / cross-DAG), 0 warnings.

---

## 2. V1.3 adoption — from day one

Per D-5 interleave directive, this is the first workspace authored
against the v1.3 spec. Conventions applied at authoring time (no
retrofit needed):

| Convention | Applied |
|---|---|
| V1.3-4 `expected_lifetime: long_lived` | All 5 stateful slots |
| V1.3-4 `suspended_state_exempt: true` | All 5 (governance uses reject/retire/rollback, not generic SUSPENDED) |
| V1.3-5 `owner:` governance artefact | All state machines tagged (stewards / system) |
| V1.3-5 `dual_lifecycle:` | `attribute_def` (External governed path + Internal operational auto-approved path) |
| V1.3-6 `periodic_review_cadence:` | `attribute_def` (P1Y base, HIGH → P6M) |
| V1.3-7 commercial-commitment tier | Applied to governance.publish + publish-batch (raised to requires_explicit_authorisation) |

No schema migration deferrals (D-2) — SemOs registry tables already
have the state columns the DAG references.

---

## 3. Workspace-specific observations

### 3.1 No cross-workspace aggregate hosted here

SemOsMaintenance DEFINES things for downstream workspaces to use.
It does not HOST an aggregate state — that pattern is for
downstream workspaces that consume governance output. Section 5
`derived_cross_workspace_state:` is empty by design.

**Proposed follow-up:** downstream workspaces (CBU, IM, Deal, KYC)
could declare a `registry_consistency` aggregate that flags when
they reference superseded attribute versions (staleness signal).
Not done in T3-S — belongs on the consuming workspace per V1.3-13
host-workspace pattern.

### 3.2 Governance lifecycle as cross-entity ceremony

All 5 stateful slots (changeset + attribute_def + derivation_spec +
service_resource_def + phrase_authoring) share the ProposeValidate-
SignOffPublish ceremony despite having per-entity state machines.
The overall_lifecycle §1 synthesises these into a common 8-phase
view (proposed → validating → under_review → approved → published →
deprecated → retired, plus rejected terminal-negative).

This is a **cross-slot lifecycle aggregation pattern** analogous to
V1.3-2 (cross-workspace aggregate) but intra-DAG. Not a new v1.3
amendment — it's already expressible via `overall_lifecycle.phases`
with `any_of:` derivation clauses across slots.

### 3.3 Attribute_def dual_lifecycle — canonical V1.3-5 use case

Applied V1.3-5 dual_lifecycle to attribute_def:
- **Primary (External):** ungoverned → draft → active → deprecated → retired. Full changeset ceremony.
- **Dual (Internal):** auto_active → auto_updated / auto_retired. No ceremony — auto-approved via `attribute.define-internal`.

This is the canonical operational-vs-governance split AttributeVisibility
model (External vs Internal — see CLAUDE.md §SemOS-first). V1.3-5
expresses this cleanly; was hard to represent in v1.2.

### 3.4 V1.3-13 candidate absence (correctly)

SemOsMaintenance has no derived_cross_workspace_state — governance
doesn't AGGREGATE, it DEFINES. V1.3-13 is host-pattern; the hosts
are downstream (CBU primarily). Aligns with §2 host_workspace of
spec — authors choose hosts that semantically own the aggregate,
not sources that merely contribute.

---

## 4. Test fixture hygiene (no action needed)

31 test utterances (6 marked `sem_os_maintenance`, 25 marked
`semos_maintenance`). All 31 resolve:
- 26 to declared verbs
- 5 to declared macros / narration-intercept

**0 stale references.** First workspace in Tranche 2/3 with zero
fixture drift.

---

## 5. Tranche 3 progress

| Workspace | Status |
|---|---|
| **SemOsMaintenance** | ✅ CLOSED (this session) — first v1.3-native workspace |
| book-setup | ⏳ pending |
| onboarding-request | ⏳ pending |
| product-service-taxonomy | ⏳ pending |
| session-bootstrap | ⏳ pending |

4 remaining. Estimated total ~3-4 hours to close Tranche 3 primary
workspaces.

---

## 6. Closure

**T3-S CLOSED.** SemOsMaintenance is the fifth production DAG.
First to author fully against v1.3 spec rather than retrofit.
Validator infrastructure proven — caught periodic_review_cadence
without re-review transition (fixed by adding attribute.propose-revision
transition), then green.

Next: book-setup.

**T3-S-9 end.**
