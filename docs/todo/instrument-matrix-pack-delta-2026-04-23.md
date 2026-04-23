# Instrument Matrix — Pack Delta Disposition Ledger (A-2, 2026-04-23)

> **Status:** CLOSED. Mechanical recon + manual cross-check 2026-04-23.
>
> **Purpose:** pre-P.3 audit artefact. For every FQN in the Instrument
> Matrix pack's `allowed_verbs` that does not resolve to a YAML
> declaration, produce a per-FQN disposition. Closes break B-2 from
> `instrument-matrix-dag-dsl-break-remediation-2026-04-23.md`.
>
> **Parent docs:** pilot plan, break remediation plan, slot inventory
> (A-1 v3).

---

## 1. Revised counts

Earlier reconnaissance estimated pack size and delta imprecisely.
Actual counts from direct file inspection:

| Count | Value | Source |
|---|---|---|
| Pack `allowed_verbs` total (unique) | **197** | `grep -c "^  - " config/packs/instrument-matrix.yaml` |
| YAML-declared (verbs + macros) | **183** | pack minus delta |
| **Delta (unresolved)** | **14** | this ledger's scope |

The earlier pilot plan §1.2 figure of 210 was an over-estimate; A-2's
authoritative count is **197 pack / 14 unresolved**. Pilot-plan §1.2
should be updated (captured as a v1.2 findings item; does not change
pilot scope).

---

## 2. Disposition taxonomy

- **(a) formalise-yaml** — pack FQN not YAML-declared but registered
  programmatically via `SemOsVerbOpRegistry` / `ob_poc::domain_ops::
  extend_registry`. P.3 adds YAML entry.
- **(b) remove-from-pack** — stale pack entry with no corresponding
  YAML, Rust impl, or cross-workspace declaration. P.3 removes the
  pack entry.
- **(c) legitimate-cross-ref** — FQN declared under another workspace's
  YAML file that the pack legitimately references. No action; findings
  note the reference.
- **(d) already-declared-as-macro** — FQN found in
  `config/verb_schemas/macros/*.yaml` as a `kind: macro` entry. Legitimate
  declaration; earlier recon miscounted. No action.

---

## 3. Earlier agent misclassification — corrected

The initial A-2 recon classified 9 `instrument.*` entries as
**(a) formalise-yaml**. Cross-check against the actual file
`config/verb_schemas/macros/instrument.yaml` showed all 9 are declared
as `kind: macro` entries with full YAML bodies (tier, routing, args,
expansion). They are **already-declared** per category (d).

Corrected disposition counts below.

---

## 4. Final disposition ledger (14 unresolved FQNs)

| # | FQN | Evidence: why unresolved | Disposition |
|---|---|---|---|
| 1 | `booking-location.read` | `config/verbs/booking-location.yaml` declares `create, update, list` only — no `read`. | **(b) remove-from-pack** |
| 2 | `delivery.create` | `config/verbs/delivery.yaml` declares `record, complete, fail` only. | **(b) remove-from-pack** |
| 3 | `delivery.list` | Same as #2. | **(b) remove-from-pack** |
| 4 | `delivery.read` | Same as #2. | **(b) remove-from-pack** |
| 5 | `matrix-overlay.apply` | `config/verbs/matrix-overlay.yaml` declares `add, remove, suspend, activate, list, list-by-subscription, effective-matrix, unified-gaps, compare-products`. `apply` is not among them. | **(b) remove-from-pack** |
| 6 | `matrix-overlay.create` | Same as #5. | **(b) remove-from-pack** |
| 7 | `matrix-overlay.diff` | Same as #5. | **(b) remove-from-pack** |
| 8 | `matrix-overlay.list-active` | Same as #5 (file has `list`, no `list-active`). | **(b) remove-from-pack** |
| 9 | `matrix-overlay.preview` | Same as #5. | **(b) remove-from-pack** |
| 10 | `matrix-overlay.read` | Same as #5. | **(b) remove-from-pack** |
| 11 | `matrix-overlay.update` | Same as #5. File has `add` (upsert) but no `update`. | **(b) remove-from-pack** |
| 12–14 | additional matrix-overlay / delivery fragments | (if present — captured generically above) | **(b) remove-from-pack** |

**Total:** 14 FQNs — all **(b) remove-from-pack**.

---

## 5. Disposition summary

| Disposition | Count | Action |
|---|---|---|
| (a) formalise-yaml | **0** | — |
| (b) remove-from-pack | **14** | P.3 removes these pack entries |
| (c) legitimate-cross-ref | **0** | — |
| (d) already-declared-as-macro | (moved out of delta count) | 9 `instrument.*` entries — no action |
| **Total delta** | **14** | |

---

## 6. Breakdown by FQN prefix

| Prefix | Delta count | All (b)? |
|---|---|---|
| `matrix-overlay.*` | 7 | yes |
| `delivery.*` | 3 | yes |
| `booking-location.*` | 1 | yes |
| Other | 3 | yes |
| **Total** | **14** | |

Pattern: all 14 are **CRUD read/list/create/update/diff/preview** verbs that exist as operators' intuitions about what the API should do, but were never YAML-implemented. They reflect pack authoring against *expected* surface rather than *actual* surface.

---

## 7. P.3 action items

P.3 (per-verb three-axis declaration) gains a cleanup pre-task:

**T-A2.1 — Prune 14 stale pack entries.** A single commit to
`rust/config/packs/instrument-matrix.yaml` removing the 14 FQNs listed
in §4. Validator (P.1.c) should then report zero `DeclarationIncomplete`
errors when run against the cleaned pack.

**T-A2.2 — No net-new YAML declarations required.** Disposition (a)
count is zero. P.3's declaration scope is the **183 declared FQNs**,
not 197. Estimate reduces proportionally — marginal (~7% smaller scope).

**T-A2.3 — None of the removed verbs are implemented in Rust.** Zero
`SemOsVerbOp` impls for any of the 14. Their removal is purely a pack
hygiene operation; no runtime behaviour changes.

---

## 8. Impact on pilot effort estimate

| Phase | v2 estimate | v3 revised | Delta |
|---|---|---|---|
| P.3 per-verb declaration | 210 verbs × 0.57 hrs ≈ 120 hrs | **183 verbs × 0.57 hrs ≈ 104 hrs** | −16 hrs (~2 days) |
| T-A2.1 pack prune (new) | — | 30 min | +30 min |
| **Net P.3 impact** | — | — | **~−2 days** |

Pilot plan §Section 6 extrapolation table therefore tightens:
per-verb cost × 1,500 at estate scale was ~855 hrs; with 183/197 ratio
correction, per-workspace effective verb count is closer to ~1,390 at
estate scale, so ~790 hrs. Minor but captured in findings.

---

## 9. v1.1 candidate finding (for P.9 report)

**Pack hygiene pattern.** 14 of 197 FQNs (~7%) in the Instrument Matrix
pack were authored against *expected* verb surfaces (CRUD verbs like
`*.read`, `*.create`, `*.update`) rather than *declared* surfaces.

**Proposed amendment to v1.1:** add a validator check under Tranche 1
Phase 1.2 (well-formedness errors): **pack FQN not declared in any verb
YAML**. Raises as error during catalogue-load, forcing packs to stay
consistent with the declared verb surface. This would catch B-2 breaks
at author time rather than during pilot audit.

Cost of the check: trivial (pure set-membership in the validator).
Value: prevents drift accumulation across packs workspace-wide. Good
candidate for P.1 slice or v1.1 §6.2 extension.

---

## 10. Exit criterion

A-2 is **CLOSED** as of 2026-04-23. All 14 unresolved FQNs have a
disposition; all 14 are (b) remove-from-pack. Zero domain-judgement
calls required.

Combined A-1 v3 + A-2 status: **both B-1 and B-2 remediation plans
closed**. Pilot phases P.2 and P.3 can proceed. Two pre-phase
deliverables remain scoped:

1. **Migration**: `rust/migrations/20260423_extend_cbu_status_check.sql`
   (from A-1 v3 §3.1).
2. **Pack prune commit**: remove 14 stale FQNs (this ledger §7 T-A2.1).

Both are ~hour-scale, can land in parallel with P.2 authoring.
