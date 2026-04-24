# Tranche 3 — Book-Setup Findings Report (2026-04-24)

> **Status:** CLOSED. Second Tranche 3 workspace. Uses v1.3
> conventions from day one.

---

## 1. Delivery summary

9 phases executed in ~25 min. Book-Setup is a JOURNEY workspace —
macro-heavy (42/49 pack entries are macros) with minimal owned-entity
state machine additions. Smaller authoring scope than entity-owning
workspaces.

| Phase | Deliverable | Effort |
|---|---|---|
| T3-B-1 (P.1) | SKIP — infra reused | 0 |
| T3-B-2-prep | Pack-delta check — clean (49/49 resolve; 7 verbs + 42 macros, 0 unresolved) | 2 min |
| T3-B-2 | `book_setup_dag.yaml` (~550 LOC: 8-phase journey lifecycle, 1 new stateful `book` slot, 5 cross-workspace references, 4 stateless) | ~18 min |
| T3-B-3 | Three-axis retrofit SKIP — all 7 direct verbs already declared in their owning workspaces (cbu, entity, session); 42 macros don't need three_axis | 0 |
| T3-B-4 | Tier review SKIP — no new verbs added (book.* verbs are aspirational, declared in state machine but not yet in verb YAML; pending Tranche 3 migration) | 0 |
| T3-B-5 | Runtime triage: 0 utterances in fixture for book_setup workspace (trivially 0/0) | 1 min |
| T3-B-9 | This document | ~4 min |

**Validator terminal state:** 673 / 1184 declared (unchanged — no
new verb declarations this workspace), 0 structural errors, 0
well-formedness errors, 0 warnings. **6 DAGs loaded.**

---

## 2. Workspace shape — journey, not entity

Book-Setup is architecturally DIFFERENT from Tranche 2 workspaces:

- **Entity-owning workspaces** (IM, KYC, Deal, CBU, SemOsMaintenance):
  DAG defines state machines for entities the workspace OWNS.
  Authoring is entity-centric.
- **Journey workspace** (Book-Setup, and likely
  onboarding-request to follow): DAG sequences operations on
  entities OWNED BY OTHER WORKSPACES. Authoring is
  workflow-centric.

Pack profile:
- 7 direct verbs (14%): cbu.*, entity.create, session.load-galaxy
- 42 macros (86%): structure.*, struct.*, mandate.*, party.*

The only new stateful slot is `book` — a conceptual container
tracking the setup-journey progress. All other slots reference
their owning workspaces (cbu, entity, trading_profile, kyc_case).

**Implication:** Tranche 3 journey workspaces produce thinner DAGs
than Tranche 2 entity workspaces. Effort-per-workspace drops further.

---

## 3. `book` slot — new conceptual entity

Authored as a new stateful slot tracking the book-setup journey:

```
proposed → structure_chosen → entities_provisioned →
cbus_scaffolded → parties_assigned → mandates_defined →
ready_for_deal
       ↘ abandoned (terminal)
```

7 forward states + abandoned terminal. Each state represents
progress through the journey; transitions invoke macros / verbs in
adjacent workspaces.

**Schema migration deferred (D-2):** `client_books` table
(book_id, client_group_id, name, status, jurisdiction_hint,
structure_template) is added in Tranche 3 migration window. DAG
leads.

**Book verbs to declare (follow-up):** `book.select-structure`,
`book.mark-ready`, `book.abandon`. Not added in T3-B since they
depend on the table existing. Listed in the state machine
transitions as intent; verb YAML declarations follow.

---

## 4. Cross-workspace constraints (V1.3-1)

Two constraints authored (both gate book advancement on other
workspaces' state):

1. **book → cbus_scaffolded** requires KYC case in DISCOVERY+
   (cross-workspace: KYC → book-setup)
2. **book → ready_for_deal** requires Deal at KYC_CLEARANCE+
   (advisory warning; Deal is the commercial gate downstream)

First time a non-primary Tranche-3 workspace declares Mode A
cross-workspace constraints.

---

## 5. Cross-workspace aggregate — absent, correctly

Per the pattern documented in T3-S SemOsMaintenance findings:
Book-Setup does NOT host a `derived_cross_workspace_state`
aggregate. Book state is locally-derivable from its own progress
markers + directly-joined CBU/entity data.

A "book-ready-to-deal" aggregate could live on Deal workspace
(deal.kyc_cleared_plus_book_ready) but that's downstream scope,
not T3-B.

---

## 6. Follow-up work (Tranche 3 migration window)

Flagged for schema-migration batch:

| Target | Action |
|---|---|
| `client_books` table | New table per book state machine |
| `client_books.status` CHECK | 8 states (proposed → abandoned) |
| `client_group.book_id` / `cbus.book_id` FK | Link CBUs to books |
| `book.select-structure` verb declaration | Pending table |
| `book.mark-ready` verb declaration | Pending table |
| `book.abandon` verb declaration | Pending table |

Book-Setup can operate WITHOUT these today via macro + existing
verbs, but the `book` state machine is authoritative DAG-only
until the schema catches up.

---

## 7. Tranche 3 progress

| Workspace | Status |
|---|---|
| **SemOsMaintenance** | ✅ CLOSED |
| **Book-Setup** | ✅ CLOSED (this session) |
| session-bootstrap | ⏳ pending |
| onboarding-request | ⏳ pending |
| product-service-taxonomy | ⏳ pending |

3 remaining. Estimated ~1.5-2 hours total.

---

## 8. Closure

**T3-B CLOSED.** 6th production DAG. First journey-workspace DAG.
Pattern established for onboarding-request + session-bootstrap
(both likely journey-shaped too). product-service-taxonomy will
likely return to the reference-data-heavy shape (more slots, less
lifecycle).

Next: session-bootstrap.

**T3-B-9 end.**
