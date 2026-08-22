# Instrument Matrix — Fix Stream (Clean Baseline)

### EOP-PLAN-MANDATE-FIX-001

| | |
|---|---|
| **Document** | EOP-PLAN-MANDATE-FIX-001 |
| **Type** | Implementation plan — Fix stream only |
| **Version** | 0.1 — Draft |
| **Depends on** | EOP-AUDIT-MANDATE-IM-001 v0.1 (audit), EOP-FIND-MANDATE-T0-001 v0.1 (T0 reconciliation) |
| **Target** | Zed / Sonnet implementation session |
| **Status** | Draft — F0 is a blocking pre-check |

---

## §1 Purpose and framing

Establish a **coherent baseline** by fixing what is demonstrably broken and removing what was never built out. No modelling, no new capability, no refactor.

**The framing correction that makes this safe:** the audit established that **no real CBU has ever had an ACTIVE trading profile** — all 16 ACTIVE profiles belong to `tmm_*_test_cbu` fixtures, out of 2,302 CBUs — and `trading_profile_materializations` has **0 rows, ever**. This is not a live system with defects. It is an unfinished system.

Consequently this plan carries **no cutover risk, no consumer compatibility surface, and no behavioural-equivalence obligation**. Earlier drafts (EOP-PLAN-MANDATE-001 T3) budgeted for preserving behaviour across a cutover; there is no behaviour to preserve.

`data_designer` is the only environment in use. Zero row counts are therefore evidence of *never built out*, not *orphaned from something working elsewhere*.

---

## §2 Scope

**In scope**
1. The four LIVE-BROKEN items (B1–B4) from the audit.
2. Full removal of the trade-gateway concept — DSL verbs, DAG surface, tables, migration. To be redesigned later from the DSL down.

**Explicitly out of scope**
- Wiring `subcustodian_network` into settlement chains (Wire stream).
- Account-mapping attributes, base currency, auto-FX (Model stream).
- Any change to the document shape beyond fixing round-trip correctness.
- Anything in the DEAD bucket other than gateway.

---

## §3 Tranches

### F0 — Pre-check *(blocking, research only)*

Removal safety. Answer before any deletion:

1. **Does any DAG transition route `via` a `trade-gateway.*` verb?** If a slot's state machine transitions through gateway configuration, removing the surface leaves a dangling transition. `instrument_matrix_dag.yaml` is authoritative for state-machine semantics and is consumed by the **catalogue-load validator (P.1.c) before DB pool init** — a hole may fail startup, not merely look untidy.
2. **Is `dsl_verb_reconciliation` checked against the verb catalogue?** If so, verbs and their reconciliation entries must be removed together.
3. **Confirm the full gateway footprint.** Audit names: `cbu_gateway_connectivity`; four connectivity-writing verbs; three definition verbs targeting the absent `trade_gateways`; `202501_instruction_gateway.sql` (unapplied); `trade_gateway_surface` in the DAG. Verify this list is complete and that nothing outside it references gateway concepts.
4. **Re-confirm B1–B3 reproduce** against current `HEAD`.

**Gate:** the removal set is enumerated and closed; no gateway reference exists outside it.

---

### F1 — Gateway removal

Remove the whole concept: DSL verbs (definition and connectivity), `trade_gateway_surface` and its `dsl_verb_reconciliation` entries, `cbu_gateway_connectivity`, and `202501_instruction_gateway.sql`.

This dissolves **B4** (connectivity rows referencing gateways that cannot be created) rather than patching it.

**Gates**
- `catalogue_load_validator_passes` — the validator passes with the surface removed. *This is the real gate; a dangling transition fails startup.*
- `no_dangling_gateway_reference` — source scan: no verb, DAG entry, reconciliation entry, table, or migration references a gateway concept.
- `no_orphan_reconciliation` — every `dsl_verb_reconciliation` entry resolves to an existing verb.

---

### F2 — Document round-trip correctness *(B1)*

`TradingMatrixDocument` serialises a form it cannot deserialise: `created_at`/`updated_at` carry `skip_serializing_if` **without** `#[serde(default)]`, while every other optional field on the struct has it. Four incompatible shapes exist in `cbu_trading_profiles.document`; the template writer visibly changed between 11:07 and 11:09 on 2026-03-31, stranding its predecessors.

Fix the round-trip defect. **Do not** redesign the document — that is Model-stream work.

**Gates**
- `document_round_trips` — property test: for any `TradingMatrixDocument`, `deserialize(serialize(d)) == d`. Fuzz over field presence/absence, especially all-optional-absent. *Non-gameable: a universal, not a fixture.*
- `no_write_only_field` — type-level: no field is serialisable but not deserialisable. Catches the class, not the instance.
- Stranded rows: **decide and record** — migrate, delete, or leave. All four shapes are fixtures or templates; none is production data. Deletion is defensible and should be preferred unless a reason to keep them is stated.

---

### F3 — Remaining live-broken items *(B2, B3)*

Fix as scoped by the audit. Each lands independently with a gate asserting the specific failure no longer occurs. If a fix requires a modelling decision, **stop and escalate** — it belongs to the Model stream, not here.

**Gate per item:** the audit's stated failure is reproduced RED, then GREEN.

---

### F4 — Baseline verification

Assert the baseline is coherent — the point of the stream.

**Gates**
- `no_orphan_child_table` — no table whose parent table does not exist (the B4 class, generalised).
- `no_verb_targets_absent_table` — the audit found **24 CRUD verbs targeting tables absent from the live DB**. Enumerate them; each is either fixed, removed, or explicitly recorded as deferred with a reason. *No silent survivors.*
- `no_verb_calls_absent_function` — e.g. `cbu-custody.lookup-ssi` → `find_ssi_for_trade()`, absent from `pg_proc`. Same disposition rule.
- `catalogue_load_validator_passes` — end of stream.

---

## §4 Sequencing

```
F0 (pre-check — BLOCKING)
   ↓
F1 (gateway removal) ──┐
F2 (document B1) ──────┼──→ F4 (baseline verification)
F3 (B2, B3) ───────────┘
```

F1–F3 are independent and may land in any order or in parallel. F4 closes the stream.

---

## §5 Rules for this session

- **No modelling.** If a fix needs a design decision, stop and escalate.
- **No new capability.** Fix and remove only.
- **RED first.** Every gate fails before it passes; a gate that was never red proves nothing.
- **Removal is removal.** No deprecation shims, no commented-out code, no "kept for reference" tables. The decks are being cleared deliberately.
- **Deferrals are recorded.** Anything in F4 not fixed or removed gets an explicit written reason. Silence is not a disposition.

## §6 What this stream deliberately does not do

Does not touch `subcustodian_network` (85 real rows, zero inbound FKs — Wire stream), the `SettlementHop` projection dropping `account_number`/`intermediary_entity_id`/`ssi_id` (Wire), or anything about auto-FX, base currency, or account mapping (Model). Does not resolve the three incompatible specificity scales — the audit found the tie risk is **latent, not manifested** (zero true criteria-level duplicates), so it is a Model-stream design question, not a live bug.

## §7 Note on version control

`.git/info/exclude` line 10 contains `*.md`, so plan and findings documents do not appear in `git status` and will not commit without `-f`. Pre-existing local config, recorded here so the deliverables are not assumed to be under version control.
