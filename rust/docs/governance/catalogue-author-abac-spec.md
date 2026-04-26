# Catalogue-Author ABAC Spec — Tranche 3 Phase 3.B (2026-04-26)

> **Authority:** Adam-as-architectural-authority per `tier-assignment-authority-provisional.md`.
> **Spec reference:** v1.2 §8.4 DoD item 4 — "Catalogue-author ABAC gate active".
> **Status:** Specification + provisional enforcement (Phase 3.F Stage 1: opt-in pilot). Hard architectural enforcement is Phase 3.F Stage 2 onwards.

---

## 1. Role definition

```yaml
role: catalogue-author
description: |
  Permits authoring verb declaration proposals against the catalogue
  workspace. Holders may propose, list, and rollback their own staged
  proposals. Holders may NOT commit their own proposals (separation of
  duties — see §3 two-eye rule).

permissions:
  - catalogue.propose-verb-declaration
  - catalogue.list-proposals
  - catalogue.rollback-verb-declaration
  - catalogue.reopen-proposal

# Verbs requiring catalogue-author role + explicit reviewer separation:
gated_by_separation_of_duties:
  - catalogue.commit-verb-declaration
  - catalogue.reject-proposal
```

## 2. Principal model

A principal is a SemOS-side actor: human user, service-account, or
deterministic agent. The `catalogue-author` role is granted as an
attribute on the principal:

```yaml
principal:
  actor_id: "alice@example.com"
  roles:
    - catalogue-author
  attributes:
    catalogue_author_since: "2026-04-26"
    catalogue_author_scope: ["all"]   # or [<domain>, ...] for scoped roles
```

`catalogue_author_scope` allows future scoped roles (e.g. `catalogue-author-deal`
who can only author against the `deal.*` domain). For Tranche 3 Phase 3.B,
all catalogue authors have scope `["all"]`. Per-domain scoping is a
follow-on enhancement.

## 3. Two-eye rule

`catalogue.commit-verb-declaration` is the irreversible architectural
drift gate. It enforces:

1. **The committing principal MUST hold the `catalogue-author` role.**
2. **The committing principal MUST differ from the proposing principal.**

Both checks happen at three layers:

| Layer | Check | Failure mode |
|-------|-------|--------------|
| ABAC pre-flight | Principal carries `catalogue-author` role | `403 Forbidden` |
| Verb handler pre-flight | `approver` arg matches invoking principal AND differs from `proposed_by` row | `Two-eye rule violation` error |
| DB CHECK constraint | `catalogue_two_eye_rule` on `catalogue_proposals` table | Insert rejected by Postgres |

The DB CHECK is the last-resort guarantee — even if the ABAC layer is
bypassed (e.g. via a maintenance script), the constraint blocks the
write.

## 4. Catalogue-author lifecycle

| Lifecycle event | ABAC binding |
|-----------------|--------------|
| Role grant | Manual (admin operation; out of catalogue workspace) |
| Role revoke | Manual; in-flight proposals by the revoked principal go to REJECTED automatically (Phase 3.F Stage 2 cleanup) |
| Role expiry | Optional `catalogue_author_until` attribute supports time-bounded grants |

## 5. Audit trail

Every catalogue verb invocation produces an audit log entry capturing:

- Effective tier (post-escalation, post-composition).
- Baseline tier from the verb declaration.
- Fired escalation rules (if any).
- Composition reason for runbooks (which composition rule fired).
- Acting principal + role bindings.
- Two-eye rule outcome (proposer vs approver IDs).
- Proposed declaration JSON (for proposals).
- Verb FQN being authored.
- Outcome.

The audit log is the canonical record for organisational P-G review
(per v1.2 §13). No catalogue change is silent.

## 6. Phase 3.F enforcement stages

### Stage 1 — Opt-in pilot (this session)

- Catalogue workspace landed.
- ABAC role declared in this spec.
- `catalogue.commit-verb-declaration` enforces two-eye rule via verb
  handler + DB CHECK.
- Direct YAML edits still work; the workspace is opt-in.

### Stage 2 — Soft enforcement

- CI gate flags PRs that modify `rust/config/verbs/**` without going
  through the Catalogue workspace.
- ABAC role is enforced at the verb-dispatch boundary via the SemOS
  ABAC layer (already exists for other roles).

### Stage 3 — Read-only filesystem

- `rust/config/verbs/` mounts read-only at production runtime.
- Catalogue load reads from a runtime-managed store seeded from YAML
  at boot.

### Stage 4 — Hard enforcement (Tranche 3 final)

- YAML loading removed entirely.
- Catalogue is loaded exclusively from `catalogue_committed_verbs`.
- All catalogue changes flow through `catalogue.commit-verb-declaration`.
- Drift becomes architecturally impossible.

## 7. Provisional authority for this session

The catalogue-author role is **provisionally granted to Adam** for the
duration of the v1.2 Tranche 3 activity. This mirrors the §13 P-G
provisional designation: Adam acts as the catalogue-author authority
under the same revisability framing.

When organisational P-G is established and per-domain catalogue-author
roles are minted, this provisional grant is reviewed and either ratified
or replaced.

## 8. Sage / REPL interaction

Sage, when proposing catalogue changes:

- Computes the effective tier of the catalogue verb being invoked.
- For `catalogue.commit-verb-declaration` (`requires_explicit_authorisation`),
  Sage requires the user to type a paraphrased confirmation per
  `docs/policies/sage_autonomy.md`.
- For runbooks (e.g. `catalogue.tier-tightening` macro), Sage applies
  P12 composition: max-step-tier dominates, so the macro inherits
  `requires_explicit_authorisation` from its commit step.

REPL applies the same tier-keyed gates per `docs/policies/repl_confirmation.md`.

## 9. Related docs

- `docs/governance/tranche-3-design-2026-04-26.md` — full Tranche 3 design.
- `docs/governance/tier-assignment-authority-provisional.md` — provisional P-G.
- `docs/policies/sage_autonomy.md` — Sage tier-keyed autonomy policy.
- `docs/policies/repl_confirmation.md` — REPL tier-keyed confirmation policy.
- `rust/config/sem_os_seeds/dag_taxonomies/catalogue_dag.yaml` — Catalogue DAG taxonomy.
- `rust/migrations/20260427_catalogue_workspace.sql` — carrier table schema.
- `rust/src/domain_ops/catalogue_ops.rs` — verb implementations.

---

**End of Catalogue-Author ABAC Spec — 2026-04-26.**
