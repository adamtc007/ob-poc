# EOP-DD-KYCUBO-KIT-TS0 — Structure-Class Strategy Design

| | |
|---|---|
| **Document** | EOP-DD-KYCUBO-KIT-TS0 |
| **Version** | 0.1 — for ratification (accept / amend / defer, per section) |
| **Binds to** | EOP-PLAN-KYCUBO-KIT-001 v0.6 §TS.0–TS.4 + Appendix A (R4); EOP-VS-KYCUBO-001 v0.6 §7.1 (control taxonomy); EOP-DD-KYCUBO-KIT-T6 v0.1 (ratified matrix — the fail-closed guard 6a/8a this doc unwinds class by class); state-of-tree facts of 2026-08-12 (R4 dossier, citations inline) |
| **Status** | **RATIFIED 2026-08-12 — ACCEPT ALL (§1a/1b/1c, §2.1–2.6 incl. both OPEN trust rulings as drafted, §3).** Settlor: included unless proven irrevocable (fail-closed toward inclusion). Beneficiary: excluded from the role strategy, economic axis only. §2.2 encodes as the named `fund_control_strategy` (auditability of the freeze pin). §2.6 supersede+reassert semantics as drafted. Encoding: §1+§2.1 → TS.1; §2.2+2.3 → TS.2; §2.4+2.5 → TS.3; §2.6 → TS.4. |

**What this document is.** The T6.1 exemplar (`StructureClassSupported`, matrix rows 6a/8a) made the 6 structure classes without a `DeterminationStrategy` **fail-closed**: `Trust`, `Foundation`, `InvestmentFund`, `StateOwned`, `Cooperative`, `Nominee` cannot reach `select-strategy` or `freeze` (`control.rs:495-515`; pinned set `IMPLEMENTED_STRATEGY_CLASSES`, `control.rs:415-421`). Fail-closed is the ratchet, not the destination. This document proposes, per class, the control-basis model that turns each refusal into a real determination — plus the one blocking cross-cutting ruling (wire vocabulary, §1) that TS.1 cannot start without.

**R4 facts this design stands on** (verified 2026-08-12; every claim re-checkable at the citation):

1. Strategy dispatch is an **explicit string arg**, not derived from `structure_class`: `select-strategy` records `args.strategy` into `ControlState.selected_strategy` (`fold/control.rs:361-367`); `freeze` matches it — `"ownership_prong_strategy" | "control_prong_strategy" | other => Err` (`kyc_stream_ops.rs:545-563`). `structure_class` enters only as the fail-closed gate.
2. A strategy is small: the `DeterminationStrategy` trait has exactly two methods — `name()` and `resolve(&ControlState, EntityId, &BTreeSet<PersonId>, f64) -> Vec<ProngCandidate>` (`determination.rs:65-79`); both existing impls are zero-field unit structs.
3. **The trust collapse is real and two-layered.** `EdgeKind::TrustRole(TrustRoleKind)` exists in full (Settlor/Trustee/Protector/Beneficiary — `fold/control.rs:45,53-58`) but has **no wire value**: `assert-control`'s `valid_values` lists 7 strings for 8 variants (`dsl-kyc.yaml:63`), and `edge_kind_from_payload`'s `Some(_) | None` catch-all maps `"trust_role"` — or any typo, or an omitted key — silently to `DominantInfluence` (`fold/control.rs:230-240`). Layer two: because all trust assertions collapse to the same kind, (a) derived edge ids collide (`format!("control:{}:{}:{:?}", from, to, kind)`, `fold/control.rs:308`) and (b) even with caller-supplied distinct `edge-id`s, `NoDuplicateActiveEdge` (T6.2 row 1) rejects the second assertion as a duplicate of `(from, to, DominantInfluence)` (`fold/control.rs:549-556`). **Settlor + trustee + beneficiary between the same entity pair is not representable today.**
4. No DB migration is needed for any of this: `edge_kind` is unconstrained `jsonb` (`rust/migrations/20260630_kyc_control_edge_projection.sql:13-26`); the CHECK-constraint migration (`20260702_kyc_projection_check_constraints.sql`) constrains `status` only; the projection is disposable and rebuilt per subject (K-34, `ob-poc-kyc-store/src/projection.rs:59-79`).
5. **`EdgeKind` is unguarded.** Unlike the StructureClass count (11) and the freeze strategy arms (2) — both machine-pinned in `kyc_pack_closure.rs:520-533` — no tooth pins the `EdgeKind` wire-value set, only one true `match` on it exists (constructive, with a catch-all), and the control-prong traversal filter is default-include (`!is_economic() && !Nominee`, `fold/control.rs:715-720`): a new variant compiles silently and silently traverses. The 5/6 implemented split is also not directly pinned — widening `IMPLEMENTED_STRATEGY_CLASSES` without a strategy arm trips nothing.
6. Prior art per class: trust is best-seeded (type complete, wire missing); nominee has a wired `EdgeKind` deliberately excluded from traversal, and `ubo.edge.pierce-nominee` exists **only as one doc comment** (`fold/control.rs:710`) — no YAML, no op; the other four have nothing in-crate (BODS `UboType::StateOwned`, `dsl-runtime/src/bods/types.rs:438`, is candidate reference vocabulary only).

---

## §1 The wire-vocabulary ruling (blocking — TS.1 cannot start without it)

### 1a. Trust wire values: sub-kind-explicit, not a two-arg scheme — **PROPOSED**

Add **four** values to `assert-control`'s `kind` `valid_values` (`dsl-kyc.yaml:63`), each mapping to a distinct `TrustRole` sub-kind in `edge_kind_from_payload`:

| Wire string | Maps to |
|---|---|
| `trust_settlor` | `EdgeKind::TrustRole(TrustRoleKind::Settlor)` |
| `trust_trustee` | `EdgeKind::TrustRole(TrustRoleKind::Trustee)` |
| `trust_protector` | `EdgeKind::TrustRole(TrustRoleKind::Protector)` |
| `trust_beneficiary` | `EdgeKind::TrustRole(TrustRoleKind::Beneficiary)` |

**Why sub-kind-explicit rather than a single `trust_role` value + a second `role` arg:** the R4 fact-3 collision is fixed *for free* — the sub-kind sits inside the `{:?}` edge-id render, so the four roles derive four distinct edge ids, and `NoDuplicateActiveEdge`'s `(from, to, kind)` key distinguishes them with **zero checker changes**. A two-arg scheme would need a new keyed precondition variant and a normalizer to fold the second arg into the kind — strictly more machinery for the same wire expressiveness. The 7-value list grows to 11; one flat enum-shaped arg stays one flat enum-shaped arg (consistent with the `valid_values`-is-mandatory selector discipline, CLAUDE.md 2026-07-15).

### 1b. Kill the silent catch-all — **PROPOSED, with the layer choice as the ruling**

Today `Some(_) | None => DominantInfluence` (`fold/control.rs:238`) means a typo'd kind silently becomes catch-all control. The fold must stay **infallible** (total per-event dispatch is a load-bearing property — D2), so the rejection cannot live in the fold arm itself. Two placements:

- **(i) Op-normalizer, fail-closed (RECOMMENDED):** `UboEdgeAssertControl`'s normalizer in `kyc_stream_ops.rs` (the file's existing `normalize_*` pattern, e.g. `normalize_edge_id_payload`) hard-errors on any `kind` string outside the wire set *before* the event is appended. Nothing unrecognized ever enters the stream; the fold's catch-all becomes genuinely unreachable-in-practice but stays as the total-dispatch backstop for historical events.
- **(ii) Fold-side telemetry only:** keep the collapse, add an `unknown_kind` marker to the edge. Rejected as primary: it records the wrong thing instead of preventing it — the exact "silently-wrong" defect class (R3, M4 `edge_kind` bug) this program keeps re-finding.

Ruling asked: **accept (i)**; (ii) only as an additional breadcrumb if wanted.

### 1c. Two new closure-tooth pins — **PROPOSED**

Closing R4 fact 5, added to `kyc_pack_closure.rs` in the same TS.1 tranche as 1a:

- **Wire-value pin:** parse `assert-control`'s `valid_values` live from the YAML (same mechanism as the existing `structure_class_valid_values()`, `kyc_pack_closure.rs:350-363`) and pin the count + exact set. Any future `EdgeKind` addition becomes a conscious edit.
- **Split pin:** pin `IMPLEMENTED_STRATEGY_CLASSES.len()` against the freeze-dispatch arm count (both already parseable — `freeze_strategy_arms()`, `kyc_pack_closure.rs:327-348`) so the guard set cannot widen without a real strategy arm landing, and vice versa.

---

## §2 Per-class control-basis models

Each subsection: the control basis in domain language; which `EdgeKind`s the strategy traverses; prong assignment; threshold semantics; the strategy's `name()` string (which becomes a `valid_values` entry on `select-strategy`/`freeze` — `dsl-kyc.yaml:355-359` — and a dispatch arm at `kyc_stream_ops.rs:545`).

### 2.1 Trust (TS.1) — `trust_role_strategy`

**Basis:** control of a trust follows **role**, not shareholding. There is no ownership prong — a trust has no shares. Every candidate is `Prong::ControlByOtherMeans`; `effective_ownership_pct = None`; threshold unused (mirrors `ControlProngStrategy`, `determination.rs:171-260`).

Per-role rulings (the substance of TS.1 — rule each):

| Role | Proposed treatment | Ruling flag |
|---|---|---|
| **Trustee** | Always a control candidate — legal control of trust assets. | accept/amend |
| **Protector** | Always a control candidate — veto/replacement power over trustees. | accept/amend |
| **Settlor** | Control candidate **only if the trust is revocable** (or settlor retains powers). Revocability is not on the edge today — propose a payload field on `assert-control` (`trust-revocable: bool`, trust edges only), read by the strategy; absent ⇒ treated as revocable (**fail-closed toward inclusion**: over-identify, never silently drop a controller). | **OPEN — rule the default** |
| **Beneficiary** | **Economic**, not control: a named beneficiary with a fixed share belongs on the economic axis; a discretionary beneficiary has neither control nor a quantum. Propose: `trust_beneficiary` edges are **excluded** from `trust_role_strategy` candidates; where a beneficiary interest is quantified, it is asserted separately as `economic_interest` and the ownership prong applies as normal. | **OPEN — confirm exclusion** |

Traverses only `TrustRole(_)` kinds. Non-trust control kinds on a Trust-classified subject (e.g. a stray `voting_rights` edge) are ignored by this strategy — deliberate: if the structure genuinely mixes, classification is wrong, and reclassification is legal (matrix row 10).

### 2.2 InvestmentFund (TS.2) — `fund_control_strategy`

**Basis:** control of a fund vehicle sits with its **manager** (ManCo/AIFM/GP-analog), not its investors. `LimitedPartnershipFund` is already covered (GP chain via `GpStatutory` → `ControlProngStrategy`); `InvestmentFund` is the corporate-form fund (SICAV/OEIC/unit trust) whose control edge is the management relationship. **No new `EdgeKind`:** the management relationship asserts as `dominant_influence` (or `board_appointment` where literal). The strategy is a thin delegate to `ControlProngStrategy`'s traversal with fund framing; investor `economic_interest` edges stay on the economic axis and feed the existing ownership prong only when someone genuinely crosses the threshold. `name(): "fund_control_strategy"`. *(Alternative, flag if preferred: widen `IMPLEMENTED_STRATEGY_CLASSES` to map `InvestmentFund → control_prong_strategy` directly, zero new impl. Drafted as a named strategy for auditability of the freeze record — the pin says which model ran.)*

### 2.3 Foundation (TS.2) — `foundation_council_strategy`

**Basis:** a foundation has **no owners by construction** — control sits with the council/board. Traverses `board_appointment` + `dominant_influence`; all candidates `ControlByOtherMeans`; no threshold. Structurally the closest of the six to plain `ControlProngStrategy` — same alternative note as 2.2 applies.

### 2.4 StateOwned (TS.3) — `state_owned_strategy`

**Basis:** the controller is a state organ, not a natural person — the terminal answer is usually **SMO** (senior managing official). The strategy runs the control traversal for the rare genuine natural-person controller; when none crosses, the existing `apply-smo-fallback` path (already stage-gated by `ReconciledProjection` + `StrategySelected`, matrix row 7) supplies the determination. That is: `state_owned_strategy` legitimizes the SMO route for this class rather than inventing a new resolution model. Candidates it does emit are `ControlByOtherMeans`. BODS `UboType::StateOwned` (`bods/types.rs:438`) is the reference vocabulary for the projection rendering, not a dependency.

### 2.5 Cooperative (TS.3) — `cooperative_member_strategy`

**Basis:** one-member-one-vote — by construction no member holds ≥25% of votes through membership alone, so membership itself never yields a UBO; control arises only from **office** (board/management edges) or an anomalous concentrated voting arrangement. Traverses `voting_rights` + `board_appointment` + `dominant_influence`; expected common outcome is zero candidates → SMO fallback, same route as 2.4.

### 2.6 Nominee (TS.4 = K-8) — `ubo.edge.pierce-nominee` + `nominee_pierce_strategy`

The only class needing a **new verb**. Today `EdgeKind::Nominee` is wired on the wire but deliberately excluded from traversal (`fold/control.rs:709-713`) — attributing control to the nominee is exactly the wrong answer K-8 exists to prevent, so the class stays fail-closed until piercing exists.

**Verb semantics (rule this):** `ubo.edge.pierce-nominee` takes a target nominee edge + the disclosed nominator, and — **PROPOSED** — appends two effects in one governed event: (a) the nominee edge is **superseded** (reusing the existing supersession lifecycle, K-13 discipline: supersede, never contradict; `superseded_by` points at the pierce event), and (b) a **new control edge from the nominator** is asserted with the underlying kind (`kind` arg: what the nominator actually holds — voting_rights, economic_interest, …) and provenance marking it pierced (`pierced_from: <nominee_edge_id>` in the payload). The fold arm is additive; existing supersession machinery does the rest.

**Full kit citizenship** (the reintroduction discipline, exercised properly — same checklist K-G7 set): YAML verb + registered op + lexicon entry with preconditions **from birth** (`SubjectRegistered`, `EdgeExists` + `EdgeActive` on the target — matrix rows 3/4 vocabulary), fold arm, wire normalizer, verb-universe pin 20→21, manifest coverage, checker-reach tooth pickup (automatic — 9th/10th teeth), per-stud RED pair. `nominee_pierce_strategy` itself is then trivial: post-piercing the subject resolves by the **underlying** structure; the strategy errors if unpierced nominee edges remain active (fail-closed until the work is done) and otherwise delegates to the control/ownership prongs.

---

## §3 Cross-cutting build contract (every TS tranche)

Per R4 fact 1, one new strategy touches, invariably: **(1)** the strategy impl (`determination.rs`, unit struct + 2 methods); **(2)** a dispatch arm (`kyc_stream_ops.rs:545` match); **(3)** `select-strategy`/`freeze` `valid_values` (`dsl-kyc.yaml:355-359`); **(4)** the regenerated `.dsl` mirror (`rust/dsl-source/verbs/kyc/dsl-kyc.dsl` — regenerated, never hand-edited); **(5)** `IMPLEMENTED_STRATEGY_CLASSES` widened by exactly the classes the new strategy serves; **(6)** the seventh tooth's strategy-count pin consciously bumped (`kyc_pack_closure.rs:530-533`); **(7)** per-class RED-first gate fixtures (block-before/admit-after, the T6 pair pattern). Plus, once §1c lands, **(8)** the wire-value pin and **(9)** the split pin whenever their sets move. Each TS tranche is **one kit-hash batch**; open workbook sessions KitDrift and re-open (plan fact 3 — the system working, not a bug).

## §4 Deliberately NOT in TS.0

No code in this tranche — this document is the ruling surface only. Not proposed here: the row-9 matrix amendment (`kyc.subject.register` keyed `(subject_root, entity_id)` registration check — **separate pending ruling**, HALT-reported at T6.3); threshold-table governance (freeze's `threshold-pct` default-25.0 stays as-is pending its own reference-plane ruling); any `TrustRoleKind` widening (the 4 sub-kinds are taken as complete for TS.1); authority tightening on pierce-nominee beyond standard preconditions (`AuthoritySpec` is a separate ruling, per the T6.4 row-16 precedent).

## §5 Ratification

Reply per section — **1a / 1b / 1c / 2.1 (incl. the two OPEN per-role rulings) / 2.2 (incl. the named-vs-delegate alternative) / 2.3 / 2.4 / 2.5 / 2.6 (incl. the supersede+reassert semantics) / 3** — ACCEPT / AMEND / DEFER, or "accept all". On ratification: §1 + §2.1 encode as **TS.1** (one kit-hash batch); §2.2+2.3 → **TS.2**; §2.4+2.5 → **TS.3**; §2.6 → **TS.4**.

## Change Log

| Version | Date | Note |
|---|---|---|
| 0.1 | 2026-08-12 | Initial draft off the R4 dossier. Key design calls: sub-kind-explicit trust wire values (fixes the edge-id collision for free); fail-closed op-normalizer for unknown kinds (fold stays infallible); two new closure pins (EdgeKind wire set; implemented-split); trust = pure role strategy with settlor-revocability and beneficiary-exclusion flagged OPEN; fund/foundation lean on the existing control-prong traversal; state_owned/cooperative legitimize the SMO route; nominee = supersede+reassert pierce verb with full kit citizenship. |
