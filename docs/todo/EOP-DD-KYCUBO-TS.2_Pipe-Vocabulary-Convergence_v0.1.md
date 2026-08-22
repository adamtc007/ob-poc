# EOP-DD-KYCUBO-TS.2 — Pipe Vocabulary Convergence
### Retiring the two-vocabulary split before it becomes permanent

| | |
|---|---|
| **Document** | EOP-DD-KYCUBO-TS.2 |
| **Version** | 0.1 — draft for ratification |
| **Binds to** | EOP-DD-KYCUBO-TS.0 (RATIFIED) §3 pipe vocabulary; EOP-DD-KYCUBO-TS.1 (RATIFIED, v0.3) §2/§2a geometry and §5 vocabulary debt |
| **Domain** | **D1.** Fact establishment only. No assessment, no policy, no clearance. |
| **Status** | **RATIFIED 2026-08-19.** All four questions closed (§7). Build-ready. |

---

## §1 The debt being paid

TS.1's build introduced a second linkage vocabulary: a 17-variant `Pipe` enum used by the type-geometry matrix, alongside the substrate's existing 8-variant `EdgeKind` which is what actually gets stored on an edge. They are joined by a classifier function. That was the right call under the tranche's fences — the alternative was a fold rewrite mid-build — but it leaves the exact shape this programme has paid for six times: **two declarations of one concept, joined by a mapping that can drift, with no compiler able to see the disagreement.**

It also leaves the geometry only **partially enforceable**. Officer, management mandate, membership, statutory, employment and containment linkages are declared permitted in the matrix and cannot be asserted at all, because `EdgeKind` has no variant for them.

## §2 What exists today (verified 2026-08-19)

`EdgeKind` (`fold/control.rs:33-50`) — 8 variants: `EconomicInterest`, `VotingRights`, `BoardAppointment`, `GpStatutory`, `DesignatedMember`, `TrustRole(Settlor|Trustee|Protector|Beneficiary)`, `Nominee`, `DominantInfluence`. Eleven wire strings (`EDGE_KIND_WIRE_VALUES`), since `TrustRole` expands to four.

**A defect found while mapping, worth fixing in this tranche:** `edge_kind_from_payload` falls back to `DominantInfluence` for *any* unrecognised or absent wire value. So `DominantInfluence` means both "genuine dominant influence, asserted deliberately" and "we could not parse this". Those are different facts and must not share a variant — a determination resting on a parse failure is indistinguishable today from one resting on a real assertion.

## §3 The mapping — and why it is not a function of `EdgeKind` alone

| Stored `EdgeKind` | Pipe (TS.0 §3) | Note |
|---|---|---|
| `VotingRights` | 1 voting shares | direct |
| `EconomicInterest` | **2 · 4 · 15 — depends on target type** | into a corporate → 2 non-voting; into an LP/LP-fund → 4 limited interest; into a fund → 15 unit issuance |
| `BoardAppointment` | 5 board appointment | direct |
| `GpStatutory` | 3 GP designation | direct |
| `DesignatedMember` | 3 GP designation | LLP designated-member control is the same shape; see §6 Q2 |
| `TrustRole(Trustee)` | 8 trustee powers | direct |
| `TrustRole(Settlor)` | 9 reserved powers | direct |
| `TrustRole(Protector)` | 9 reserved powers | direct |
| `TrustRole(Beneficiary)` | 10 beneficiary entitlement | direct |
| `Nominee` | 17 nominee holding | capacity overlay (TS.1 §2 v0.3) |
| `DominantInfluence` | 13 contractual control | **only where deliberately asserted** — see §2 defect |

**The finding: the classification needs the target's entity type.** `EconomicInterest` alone cannot say which pipe it is; `EconomicInterest` into a known type can. This is a good sign rather than a problem — it means the stored fact plus the ratified type catalogue is sufficient, with no information missing from history.

**Not expressible today at all** (declared in geometry, unassertable): 6 officer appointment · 7 management mandate · 11 membership rights · 12 statutory authority · 14 employment · 16 containment.

## §4 Historical rows — the ruling this document exists for

Convergence reinterprets every edge ever written. Under the append-only law (CTN-7b, K-13) history is never rewritten, so there are three candidate positions and only two are admissible.

**(a) Read-time classification — RECOMMENDED.** Stored rows are untouched. `Pipe` is *derived* at fold time from the stored `EdgeKind` plus the target's entity type (§3). No migration, no rewrite, append-only intact. Precision is not lost: the economic/control axis split that matters most is already captured by `EconomicInterest` versus `VotingRights`, and the type catalogue supplies the rest.

**(b) New-edges-only.** New assertions carry the finer vocabulary; old rows stay coarse forever. Honest but leaves two shapes in the stream permanently and makes every traversal branch on edge age.

**(c) Migration — REJECTED.** Rewriting stored rows to the new vocabulary violates append-only and destroys the record of what was actually asserted. Named here so it is a rejected option rather than an unconsidered one.

**Consequence of (a):** a determination over historical edges is exactly as precise as the facts recorded at the time, and says so. Where the type catalogue cannot disambiguate (an `EconomicInterest` into an entity whose type is still alleged), the classification is provisional and inherits the weakest-link rule (CTN-2h, TS.0 §2a) rather than guessing.

## §5 The convergence direction — one stored vocabulary, one derived view

**Do not keep two enums.** Grow `EdgeKind` with the six missing variants (officer appointment, management mandate, membership rights, statutory authority, employment, containment) so that **everything the geometry permits can actually be asserted**, and make `Pipe` a **derived classification function**, not a stored type:

```
stored:  EdgeKind  (grown, ~14 variants + trust sub-kinds)
derived: pipe_of(edge_kind, target_entity_type) -> Pipe
```

One source of truth on the wire and in the fold; the geometry's vocabulary becomes a view over it rather than a rival to it. This is the same discipline as CTN-2f (status computed, never stored) applied to linkage classification.

**Also in this tranche — split the parse-failure fallback.** `edge_kind_from_payload` must stop folding unrecognised wire values into `DominantInfluence`. An unparseable value is a distinct outcome: reject at assertion time (preferred, fail-closed), or record as an explicit `Unclassified` variant that no traversal treats as control. Either way, a parse failure must never be indistinguishable from a deliberate assertion.

## §6 Gate tests (RED first)

- `pipe_of_is_total` — every `(EdgeKind, EntityType)` pair classifies; no panic, no silent default.
- `pipe_mapping_is_exactly_known` — RED-honest pin of §3's table; a new variant or a changed mapping forces a conscious edit.
- `economic_interest_classifies_by_target_type` — the same stored edge yields pipe 2, 4 or 15 by target type, and yields a *provisional* classification when the target type is alleged.
- `every_geometry_pipe_is_assertable` — the closure tooth for this whole tranche: every pipe permitted anywhere in TS.1 §2 has an `EdgeKind` that can express it. Today this is RED for six pipes; it goes green when §5 lands, and it stays as the guard against the split reopening.
- `unparseable_wire_value_is_not_dominant_influence` — a junk wire value does not become a control edge.
- `historical_edges_reclassify_without_mutation` — folding a pre-convergence event stream yields correct pipes with zero writes to stored rows.
- All existing edge/fold/T2/T3/T4/T4.5 gates re-run green; `EDGE_KIND_WIRE_VALUES` and the closure teeth updated consciously with red→green evidence.

## §7 Open questions — ALL RESOLVED 2026-08-19

**Q1 — Historical ruling. RATIFIED: (a) read-time classification.** This is not a choice so much as the house pattern restated — no structure in the DB, only entities, assertions and metadata; the taxonomy is built on the fly by the constructor. Stored rows are never reinterpreted by rewriting; they are reinterpreted by being read.

**Q2 — `DesignatedMember` → pipe 3. RATIFIED: shared.** An LLP's control runs through its **partners** (pipe 3) and its **officers** (pipe 6); the partners are the people who matter for determination. "Designated member" is statutory naming for the same shape, not a distinct control mechanism.

**Q3 — Parse-failure handling. RATIFIED: reject at assertion. No `Unclassified` variant.** The unclassified case is handled **further back**, at entity ingest, not at the edge layer: an entity of unknown type cannot be loaded into the store at all. Research (GLEIF and similar) happens in Sage research mode through a separate REPL that saves entities; anything it cannot pin is recorded as an **allegation**.
> **The general law this states:** *unknown is not a third state — unknown is alleged.* The epistemic model (CTN-2e) already carries "we don't know yet", so inventing an `Unclassified` variant would be a second mechanism for a case allegation already handles. Nothing unclassified may reach the UBO layer, because nothing unclassified may enter the store.

**Q4 — Wire-value growth. CONFIRMED (not a decision).** Six new variants means six new wire strings and a manifest-hash change, so open T4 sessions KitDrift when this lands. That is a deterministic consequence of batch-then-reopen, recorded so it is anticipated rather than discovered.

## Change Log
| Version | Date | Note |
|---|---|---|
| 0.1 | 2026-08-19 | Initial draft. Pays the two-vocabulary debt TS.1's build incurred. Key rulings sought: historical rows by **read-time classification** (append-only intact, no migration); convergence by **growing `EdgeKind` and deriving `Pipe`**, not by keeping two enums; and splitting the `DominantInfluence` parse-failure fallback, which currently makes a junk wire value indistinguishable from a deliberate control assertion. Records that `EconomicInterest` classifies only in combination with the target's entity type — the stored facts plus the ratified catalogue are sufficient, with nothing missing from history. Seven gate tests, of which `every_geometry_pipe_is_assertable` is the tranche's closure tooth and the permanent guard against the split reopening. |
