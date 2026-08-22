# EOP-DD-KYCUBO-CAPTURE-CHARTER — KYC Plain-English Ramp Capture Charter

| | |
|---|---|
| **Document** | EOP-DD-KYCUBO-CAPTURE-CHARTER |
| **Version** | 0.1 — DRAFT, for ratification (accept-all-or-amend, per section) |
| **Binds to** | EOP-PLAN-KYCUBO-KIT-T7 v0.1 §2/T7.1 (minimum-content list), §3 Q4 (charter authorship ruling), §5 (dependency gate: T7.1/T7.2 code may not start until this document is ratified) |
| **Status** | **RATIFIED 2026-08-14 — ACCEPT ALL 6 sections as drafted.** Unlocks EOP-PLAN-KYCUBO-KIT-T7 v0.1 §5: T7.1 (recorder + I-5 gate tooth) and T7.2 (phrase pin + tier-0 ramp v0) may now start. |

**Reading this document.** T7 lets an operator type a plain-English sentence at the KYC workbook and get back a *candidate* DSL line, which they still explicitly accept through the ordinary stage/preview/commit flow (`rust/src/domain_ops/kyc_workbook.rs::stage`). The ramp proposes; it never mutates state and never bypasses the frontier placement-set membership test. This charter governs the *telemetry* that proposal layer generates — what gets recorded about each ramp interaction, where it lives, who can read it, and what it may be used for. It does not govern ranker design (T7.3) or the ramp's retrieval mechanics (T7.2) — those are separate, later ratifications. Ratify per section: ACCEPT / AMEND (state the change) / DEFER (section stays unimplemented, consciously).

---

## §1 Scope of capture

Each ramp interaction produces at most one capture record, written only when the ramp actually renders a proposal (not on every keystroke). The record holds:

| Field | Content | Rationale |
|---|---|---|
| `utterance_text` | The operator's plain-English input | See PII ruling below |
| `placement_set_hash` | Hash of the frontier board identity (which legal moves were available), not the board's full contents | Lets later analysis ask "did the ramp propose a board member" without duplicating substrate state |
| `proposal` | The rendered candidate DSL line(s), including a `NONE_OF_THE_ABOVE` disposition if that's what was shown | This is the thing being evaluated for future tuning |
| `disposition` | select / clarify / abstain (the ramp's own classification of its output) | T7.3 promotion criteria (§5) are keyed on this |
| `user_action` | accepted / edited / rejected | Ground truth signal — an edited-then-accepted proposal is a near-miss, not a hit |
| `staged_move_id` | The resulting staged move's id, if `user_action = accepted` | Joins capture telemetry back to the governed append-protocol record it produced, without duplicating the append record itself |

**PII ruling: `utterance_text` is retained verbatim, not hashed or redacted.** KYC/UBO operator utterances routinely name real natural persons and legal entities by design — that's the domain. Hashing would make the record useless for its only purpose (T7.3 training/eval corpus review), and redaction would require a reliable PII detector, which does not exist in this pipeline and would itself need separate governance. Verbatim retention is only acceptable given the retention and access controls in §2/§3 below — this ruling is conditional on those, not free-standing.

**AMEND note:** the plan's T7.1 requirements list also invites a ruling on whether `placement_set_hash` alone (vs. full board contents) is sufficient — drafted as hash-only above, since full board contents are already reconstructable from the substrate at the recorded `as_of` and duplicating them here would be redundant state, not telemetry.

---

## §2 Retention + PII posture

- **Retention window:** 180 days from record creation, then hard-deleted by a scheduled job. (AMEND if the KYC/UBO program wants a longer or shorter window — this number is a starting proposal, not derived from any existing policy.)
- **Storage:** a new, dedicated table — **not** `kyc_intent_events` and not any other append-only substrate table. The capture record is proposal telemetry, not a governed verb-stream event; it must be deletable on the retention schedule without touching the substrate's immutability guarantees (T4.5's append protocol, §3 of the substrate design, is untouched by this charter). New migration, new table, e.g. `kyc_ramp_capture` — table name and schema are T7.1 implementation detail, not charter content, but the *separateness* from the verb stream is charter-level and binding.
- **Location:** same Postgres instance as the rest of ob-poc (`data_designer`), not a separate system — no cross-system data movement is in scope for T7.

---

## §3 Read access

- Readable by the KYC/UBO program team only (the same population that can ratify substrate design docs like this one and TS.0/T6).
- No general engineering read access, no ad-hoc `mcp__ob-poc-db__query` access from unrelated sessions — access is via a reviewed query path scoped to this table, not open SQL.
- Any read for a purpose beyond §4 (below) requires a separate governance review before it happens, not after.

---

## §4 Permitted tuning use

Captured records may be used **only** as a training/eval corpus for the T7.3 ranker (or its replacement, if T7.3 is redesigned). Specifically excluded, absent a separate ratification:

- No sharing outside the ob-poc KYC/UBO program (no third-party vendors, no cross-project reuse in other ob-poc subsystems).
- No use as a general-purpose LLM fine-tuning corpus beyond the placement-set ranking task T7.3 exists to solve.
- No use to build operator-level behavioral profiles.

---

## §5 Promotion criteria placeholder

Before any T7.3 model is allowed to influence a live session's ramp output, it must clear criteria in each of these categories (numeric thresholds are explicitly **out of scope for this charter** — they are a T7.3 gate-time ruling, not a data-governance ruling):

- **Recall@K** on placement-set boards (does the ranker surface the operator's intended move within the top K candidates).
- **False-select cap** (rate at which the ranker's top-1 disposition is `select` but the operator rejects or substantially edits it).
- **Abstention coverage** (the ranker must abstain, not guess, when the utterance doesn't map cleanly onto a board member — mirrors the DESIGN-003 tier-1 discipline this charter's parent plan mirrors, not adopts).
- **Latency** (the ramp sits in an interactive typing path; a slow ranker degrades the workbook UX it's meant to improve).

---

## §6 Non-goals

- This charter does not authorize any model to run against live session state. That authorization, if ever given, is a separate T7.3 ratification with its own promotion-criteria numbers (§5 names the categories only).
- This charter governs data handling for the capture record only — it does not rule on ramp retrieval mechanics (T7.2) or ranker architecture (T7.3).
- This charter does not modify `stage()`, the frontier placement-set membership test, the append protocol, or any existing test in `rust/tests/kyc_workbook.rs`.

---

## Ratification

Reply per section (§1–§6) with ACCEPT / AMEND (state the change) / DEFER, or reply "accept all" to ratify as drafted. Ratifying this document unblocks T7.1 (recorder + I-5 gate tooth) per the parent plan's §5 dependency graph.

---

## §7 Amendment (2026-08-17) — `user_action` widened to `MoveAttemptOutcome`

**AMEND §1.** The `user_action` field's value scope widens from
`accepted / edited / rejected` to the full closed set of
`semantic-decision-contracts::MoveAttemptOutcome` (the shared, domain-agnostic
terminal-outcome vocabulary adopted from the same crate `PlacementSet`'s
abstention identity already draws from — see
`EOP-PLAN-KYCUBO-KIT-T7_Plain-English-Ramp_v0.1.md` §8): `applied`,
`incomplete`, `ambiguous`, `inapplicable`, `disclosure_safe_refusal`, `stale`,
`compiler_refused`, `rejected_by_user`, `corrected`, `system_failure`.

**Rationale.** The original 3-value scope collapsed two distinct situations
into one `rejected` value: an operator declining an accepted proposal, and
the board itself refusing a proposal as no-longer-legal (`NotCurrentlyLegal`,
raised between `Propose` and `Stage`). `MoveAttemptOutcome` distinguishes
these (`rejected_by_user` vs. `compiler_refused`), which is a real precision
gain for the ground-truth signal this charter's §1 rationale column names as
the field's whole purpose. Reusing the shared enum here rather than
hand-extending the local `UserAction` a second time (edited-then-corrected
already needed its own value — `corrected`, for "operator staged a different,
still-legal move than what was proposed") also keeps KYC's telemetry
vocabulary aligned with the same vocabulary BPMN's gameboard work already
proved out, rather than re-diverging it.

**Current call-site mapping** (`rust/src/repl/kyc_workbook_surface.rs`'s
`Stage` arm) — only 3 of the 10 values are produced today; the rest are
adopted-but-unpopulated, matching the "list the whole taxonomy even when not
every arm has code behind it yet" convention already used for verb-YAML
`valid_values` (CLAUDE.md):

| Situation | `MoveAttemptOutcome` | `staged_move_id` |
|---|---|---|
| Staged verb matches the proposed candidate | `applied` | `Some(...)` |
| Staged verb is a different, still-legal candidate | `corrected` | `Some(...)` |
| Staged move is `NotCurrentlyLegal` | `compiler_refused` | `None` |

`staged_move_id` is therefore non-`NULL` for `applied` and `corrected` only
(§1's table amended accordingly; see the migration's column comment).

**Not amended:** `disposition`'s 3-value scope (`select` / `clarify` /
`abstain`) is untouched by this amendment — a separate, later amendment if
`GameDispositionKind` adoption (T7 §8 Phase 4) lands.

**Migration:** `rust/migrations/20260817_kyc_ramp_capture_move_attempt_outcome.sql`
— widens the `user_action` CHECK constraint; no data migration needed (table
was empty at amendment time).
