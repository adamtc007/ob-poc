# EOP-SESSION-CONTROLPLANE-G14-TABLE-FORMAT-FIX-001 — Implementation session log

### Target: the STOP-condition finding from `EOP-SESSION-CONTROLPLANE-G2-ITEMS-2-3-CLOSURE-001` §2.5 — "The STOP that keeps arming off"
### Date: 2026-07-14
### Branch: `codex/phase-1-5-governance-closure` (not merged, not committed by this session)

---

## 0. Verdict up front

**Fixed.** `build_write_set_input`'s `WriteSetInput.tables` is now
schema-qualified to the exact `"{schema}.{table}"` format
`crud_executor.rs`'s `record_write` self-reports in production, so the
prior session's proven "a fully legitimate write on a correctly-derived
column still misclassifies as a breach" scenario now attests `Bounded`.

Fix chosen: **normalize at the derivation site**
(`build_write_set_input`, via a new pure `qualify_footprint_table`
helper) — not schema-qualifying `domain_metadata.yaml`'s `writes:`/
`reads:` entries at the source, and not stripping schema at the
`attest()` comparison boundary. §1 explains why, including a real,
verified counter-case (`team.*`) that would have made the "always
default to `ob-poc`" version of the source-qualification approach
actively wrong for some verbs. Two independent, narrower, pre-existing
data-quality bugs in `domain_metadata.yaml` itself were found during
verification and are documented, not fixed (§1.4, §3) — fixing them is
out of this fix's scope (editing YAML content, not table-name *format*).

`set_expected_write_set` **remains unwired** — confirmed explicitly in
§5. This session only made the *derivation* correct; arming stays a
separate, deliberately deferred decision per this program's own "G14 is
the plan's ONE production-behavior change" framing, same posture the
prior session left it in.

---

## 1. Investigation — both recommended options, plus what the data actually looks like

### 1.1 Re-confirming the STOP is live, from current source

Re-read `rust/src/agent/control_plane_shadow.rs`'s
`derived_columns_are_correct_but_table_name_format_does_not_match_captured_writes`
test (the prior session's ground truth) and ran it before touching
anything — confirmed it still failed-by-design (asserted `Breach`,
i.e. proved the bug), unchanged since `3b8b12e2`.

### 1.2 Confirming the table-name format on the production side, for all 4 CRUD operations

Read `rust/crates/dsl-runtime/src/crud_executor.rs` line by line (not
summarized). `dispatch()` computes `schema = crud.schema.as_deref()
.unwrap_or("ob-poc")` and `table = crud.table` once
(`crud_mapping.table`/`crud_mapping.schema` from the verb's own YAML),
then routes to the 4 operation handlers. All 4 `record_write` call
sites use the byte-identical `format!("{schema}.{table}")`, sourced
from that same pair:

- `execute_insert` — line 334
- `execute_update` — line 412
- `execute_delete` — line 504
- `execute_upsert` — line 590

The prior session verified this for Insert/Update/Upsert (as part of
its column-derivation work); this session additionally confirmed
Delete uses the identical format (it wasn't part of the prior
session's column-coverage scope, but the table-name-format bug applies
identically regardless of operation kind).

### 1.3 Investigating "schema-qualify the source" — found a real counter-case

`sem_os_obpoc_adapter::metadata::VerbFootprint.writes: Vec<String>` is
consumed in 5 places (grepped across `rust/src` and `rust/crates`):
`sem_os_footprint_audit.rs` (emptiness check only, format-agnostic),
`control_plane_shadow.rs` (this fix's target),
`sem_os_obpoc_adapter::metadata::compute_reverse_index` (builds the
`read_by`/`written_by` reverse index, keyed by whatever string is
given), `sem_os_obpoc_adapter::scanner.rs`
(`contract.writes_to = footprint.writes.clone()` — copied into
SemReg's own `VerbContract`/`EntityTypeDef` display metadata,
`written_by_verbs`/`read_by_verbs`, consumed by SemReg registry
publication/documentation, with golden-value tests
(`scanner.rs::deal.writes_to == vec!["deals", "deal_events"]`) that
assert the bare form). A separate, unrelated `verb.writes:
Vec<VerbWriteConfig>` field in `dsl-semos-frontend/src/loader.rs`
(DSL authoring `:writes-json` slot) is a different type entirely, not
`VerbFootprint`.

Editing `domain_metadata.yaml`'s data (or `DomainMetadata::from_yaml`'s
parse-time behavior) to schema-qualify would therefore ripple into the
reverse index and SemReg's own governance-display metadata — broad,
cosmetic-but-real blast radius, and several of the 5 consumers have
their own tests asserting the *bare* form today. Narrower to change
only `control_plane_shadow.rs`'s own derived artifact.

Separately, and more decisively: read `config/verbs/team.yaml`
directly and found `team.create`/`team.add-member`/`team.remove-member`
declare `crud_mapping.schema: teams` (not `ob-poc`) for tables named
`teams`/`memberships` — and `domain_metadata.yaml`'s own
`team.*` footprint entries declare those same table names *bare*
(`writes: [teams]`, `writes: [memberships]`). A schema-qualification
rule that blindly defaults every bare name to `"ob-poc.{table}"` would
be **actively wrong** for these three verbs — `record_write` will
really report `"teams.teams"`/`"teams.memberships"`, not
`"ob-poc.teams"`/`"ob-poc.memberships"`. This is a genuine, verified
counter-case to the "just default everything to ob-poc" version of
option 1, found by cross-referencing `domain_metadata.yaml`'s
`team.*` block against `team.yaml`'s real `crud_mapping`, not assumed.

(Checked whether this could be fixed by deriving the *real* per-verb
schema from `RuntimeCrudConfig.schema`/`table` — the same
`runtime_registry()` source `derive_crud_allowed_columns` already
reads — instead of defaulting to `ob-poc`. This works for a verb's
*primary* CRUD table, but `domain_metadata.yaml` legitimately declares
multiple write tables per verb for plugin-behavior verbs with no
single `RuntimeCrudConfig` at all — e.g. `cbu.assign-role: writes:
[cbus, cbu_entity_roles]`, confirmed via `grep` — so a
runtime-registry-only derivation cannot cover the general case either.
Not pursued further this session; noted as a possible more-precise
follow-up in §6.)

### 1.4 What the real file actually contains — checked, not assumed

Per the task's own instruction to check the real data rather than
assume uniformity: grepped every `reads:`/`writes:` entry in
`rust/config/sem_os_seeds/domain_metadata.yaml` for dotted (already
schema-qualified) names. The **only** four distinct prefixes that
appear anywhere in the file are `kyc.`, `sem_reg.`, `sem_reg_authoring.`,
`sem_reg_pub.` — everything else is bare.

Cross-checked against `migrations/master-schema.sql`'s `CREATE SCHEMA`
list: `_sqlx_test`, `"ob-poc"`, `sem_reg`, `sem_reg_authoring`,
`sem_reg_pub`. **There is no `kyc` schema.** Both `kyc.`-prefixed
entries in the file (`kyc.cases`, used by `session.set-case`'s
`reads:`; `kyc.ownership_snapshots`, used by `ownership.compute`'s
`reads:`/`writes:` and `ownership.snapshot.list`'s `reads:`) name
tables that really live in `"ob-poc"`
(`CREATE TABLE "ob-poc".cases`, `CREATE TABLE
"ob-poc".ownership_snapshots` — confirmed by direct grep of
`master-schema.sql`). The file's own header comment (lines 9-11)
states the convention it intends: *"Tables without a schema prefix
default to `ob-poc`. Tables in other schemas use `schema.table`
notation (e.g., `kyc.cases`)."* — the `kyc.cases` example in that very
comment is itself the bug: `kyc` was used as a domain-grouping label,
not the real schema.

So `domain_metadata.yaml` has **two independent, narrower, pre-existing
data-quality defects**, neither introduced nor corrected by this
session:
1. The `kyc.` prefix (2 entries) names a schema that doesn't exist —
   should be `ob-poc.cases`/`ob-poc.ownership_snapshots`.
2. The `team.*` domain (3 entries) declares bare names that default to
   the wrong schema — should be `teams.teams`/`teams.memberships`.

Every other domain in the file with dotted entries (`sem_reg`,
`sem_reg_authoring`, `sem_reg_pub`) matches a real schema correctly.
And confirmed (grep) that `team.yaml` and `access-review.yaml` are the
**only** two verb YAML files declaring a non-`ob-poc` `crud_mapping.schema`
(`teams`/`client_portal`) anywhere in the repo — `access-review.*` has
**zero** entries in `domain_metadata.yaml` at all (so
`build_write_set_input` already returns `None` for those verbs
regardless of this fix — no regression risk there).

### 1.5 Decision

Normalize at the derivation site: `qualify_footprint_table` (new,
pure, `rust/src/agent/control_plane_shadow.rs`) implements
`domain_metadata.yaml`'s own documented convention exactly — bare name
→ `"ob-poc.{table}"`; a name already containing `.` is trusted
verbatim. Correct for the overwhelming majority of the corpus
(everything except the 5 known-defective entries above, which were
already wrong before this session and remain wrong after it — no
regression, and precisely disclosed rather than silently left
unexplained). `build_write_set_input` maps every entry in
`footprint.writes` through it before constructing `WriteSetInput.tables`.

This was chosen over both of the prior session's suggested options
because:
- Schema-qualifying the YAML source (option A) has a real, broader
  blast radius (5 consumers, several with golden-value tests on the
  bare form) and — per §1.3's `team.*` finding — a naive "always
  default to `ob-poc`" version of it would be **actively wrong**, not
  just less localized.
- Normalizing at the `attest()` comparison boundary (option B, e.g.
  stripping schema prefixes before comparing) would weaken the
  security-relevant comparison itself (two same-named tables in
  different schemas would spuriously match) and doesn't fix anything
  the derivation-site fix doesn't already fix — no reason to touch the
  security-critical comparison function for a data-shape problem that
  is fully addressable one layer up, in the one function that already
  owns "turn declarative config into a `WriteSetInput`."

---

## 2. Implementation

`rust/src/agent/control_plane_shadow.rs`:
- New `fn qualify_footprint_table(table: &str) -> String` — pure,
  private, documented with the full investigation above (verified
  convention + both known exceptions, so a future reader doesn't have
  to re-derive this).
- `build_write_set_input` now maps `footprint.writes` through it before
  constructing `WriteSetInput.tables` (previously
  `tables: footprint.writes.clone()`).
- Removed the now-resolved "known remaining gap" doc paragraph from
  `derive_crud_allowed_columns` (it described exactly the bug this
  session fixes).

No other file touched. `sem_os_obpoc_adapter::metadata` (the YAML
loader), `domain_metadata.yaml` itself, `write_set_attestation.rs`
(`attest()`), and `write_set.rs` (`WriteSetProof`) are all byte-for-byte
unchanged from before this session.

---

## 3. Tests

New (this session), all in `control_plane_shadow.rs`'s existing
`#[cfg(test)] mod tests`:
- `derived_columns_are_correct_and_table_name_now_matches_captured_writes`
  — **supersedes** the prior session's
  `derived_columns_are_correct_but_table_name_format_does_not_match_captured_writes`.
  Same scenario (real `capability-binding.draft` verb, correctly-derived
  non-empty `allowed_columns`, a genuinely-declared column write),
  reversed assertion: `attest()` now returns `AttestationOutcome::Bounded`,
  not `Breach`. Also asserts `ws.tables == ["ob-poc.capability_bindings"]`
  (the schema-qualified form) directly.
- `qualify_footprint_table_bare_name_defaults_to_ob_poc` — the common
  case.
- `qualify_footprint_table_already_dotted_name_passes_through_verbatim`
  — proves the `sem_reg.*`/`sem_reg_authoring.*` family (real schemas)
  is trusted as-is.
- `qualify_footprint_table_known_gap_kyc_prefix_is_not_a_real_schema` —
  documents, with a real assertion (not just prose), that the `kyc.`
  pre-existing defect (§1.4) is unchanged by this fix: `kyc.cases` stays
  `kyc.cases`, not the real `ob-poc.cases`.
- `build_write_set_input_qualifies_every_table_in_a_multi_table_footprint`
  — proves a footprint declaring several write tables in one verb
  (the real `cbu.assign-role`-shaped pattern) gets every table
  qualified, not just the first.

Updated (stale assertions from the pre-fix bare-name behavior):
- `build_write_set_input_some_with_tables_when_footprint_declares_writes`
  — `ws.tables` assertion changed from `["deals"]` to `["ob-poc.deals"]`.

All 4 CRUD operations' real table-name format were checked directly
against `crud_executor.rs` (§1.2) — the fix and its tests are not
scoped to only the Insert/Update/Upsert subset the prior session's
column-derivation work covered; `qualify_footprint_table` is
operation-agnostic (it only ever sees `domain_metadata.yaml` table
names, never a `CrudOperation`), so it is correct for Delete and any
future operation kind by construction, not merely by omission.

---

## 4. Verification — real command output

```
$ cargo build --workspace
   Compiling ob-poc v0.1.0 (...)
   Compiling ob-poc-web v0.1.0 (...)
   Compiling xtask v0.0.0 (...)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 34.82s   # zero errors

$ cargo clippy -p ob-poc --lib -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 16.28s   # zero warnings

$ cargo test -p ob-poc --lib agent::control_plane_shadow
running 56 tests
... (51 run, 5 #[ignore]d requiring DATABASE_URL)
test result: ok. 51 passed; 0 failed; 5 ignored; 0 measured; 2349 filtered out

$ DATABASE_URL=postgresql:///data_designer cargo test -p ob-poc --lib --features database \
    control_plane_shadow -- --ignored --test-threads=1
running 5 tests
test agent::control_plane_shadow::tests::g2_reaches_success_end_to_end_against_a_real_cbu_row ... ok
test agent::control_plane_shadow::tests::g3_none_leaves_authority_and_evidence_blocked ... ok
test agent::control_plane_shadow::tests::g3_reaches_success_and_unblocks_authority_evidence ... ok
test agent::control_plane_shadow::tests::t9_1_closure_all_seven_gates_reach_a_real_outcome_on_one_dispatch ... ok
test agent::control_plane_shadow::tests::toctou_unpinned_entity_requires_human_gate ... ok
test result: ok. 5 passed; 0 failed; 0 ignored

$ cargo test -p ob-poc-control-plane
... write_set_attestation::tests:: (10 tests, all unchanged) ... ok
test result: ok. 120 passed; 0 failed; 0 ignored

$ DATABASE_URL=postgresql:///data_designer bash scripts/check-invariants.sh e3
E3_INVARIANT_FAILURE: 0 gate(s) have zero substantive production samples anywhere: [];
1 gate(s) have samples only at the WRONG provenance (expected-provenance mismatch): ["WriteSetAttestation"]
  E3: DOES NOT HOLD
```

(`[e3]`'s baseline in `invariants-expected.toml` is `status = "fail"`,
last updated by the prior session at 13/14 gates passing, sole gap
`WriteSetAttestation` — unchanged by this session, as expected: this
fix touches G7's `WriteSetInput` derivation, not G14's own
`write_set_attestation` field, which `build_evaluation_context` never
populates for shadow eval — see §5.)

```
$ DATABASE_URL=postgresql:///data_designer bash scripts/check-invariants.sh all
== E1: ledger rows provably CLOSED ==
  E1: DOES NOT HOLD
== E2: execution only via envelope admission (structural + dynamic) ==
  E2: structural half HOLDS; dynamic evidence shows Path D NotEnforced by default -> DOES NOT HOLD
== E3: G1-G14 evaluated in production with metrics flowing ==
  E3: DOES NOT HOLD (WriteSetAttestation only, unchanged — see above)
== E4: Mode-1 register rows version-pinned or human-gated-and-tested ==
  E4: DOES NOT HOLD
== E5: workspace hygiene ==
  E5: DOES NOT HOLD
== Summary: 5/5 invariants do not hold ==
```

All 5 match `invariants-expected.toml`'s pre-existing ratchet baseline
(`status = "fail"` for e1-e5) — no new divergence introduced by this
session, confirmed by the ratchet gate itself:

```
$ DATABASE_URL=postgresql:///data_designer bash scripts/check-invariants.sh ratchet
...
  E5: DOES NOT HOLD
  [e5] actual=fail expected=fail — MATCH

== Ratchet: 0/5 invariant(s) diverge from invariants-expected.toml ==
```

```
$ cargo test -p ob-poc --lib -- test_plugin_verb_coverage
test result: ok. 1 passed; 0 failed   (unaffected — this session touched no verb registration)
```

---

## 5. Scope-boundary checks (explicit, not just omission)

- **`set_expected_write_set` remains unwired.** Grepped every call
  site: `rust/src/sequencer_tx.rs:321,373` (the real, separately-built
  `WriteSetProof` used by `PgTransactionScope::commit_attested`) and
  `rust/src/sem_os_runtime/verb_executor_adapter.rs` (comments only,
  explicitly documenting it is *not* called there). None of these
  construct their `WriteSetProof` from `build_write_set_input`'s
  output — that function's output only ever feeds G7's shadow
  evaluation (`build_evaluation_context`'s `write_set` field) and,
  before this session, the prior STOP-condition test. This session did
  not touch `sequencer_tx.rs`, `verb_executor_adapter.rs`, or
  `execute_verb_admitting_envelope`'s commit path at all.
- **G14 (`WriteSetAttestation`)'s shadow evaluation is not populated by
  this change.** `build_evaluation_context` never sets
  `ctx.write_set_attestation` (confirmed: not among the fields it
  constructs; falls through to `..Default::default()` → `None`), so
  G14 still evaluates as `Failure("no WriteSetAttestationInput
  supplied")` in shadow, exactly as before this session. This fix only
  changes what `build_write_set_input` feeds into **G7**'s
  (`WriteSet`) shadow evaluation and — if a future session arms it —
  what would eventually be compared for G14. `WriteSetGate::decide`
  (G7) only checks `contract_derived` + non-empty `tables`, so this
  fix does not change G7's shadow *verdict* either — it changes the
  *data*, from structurally-guaranteed-wrong to correct-for-the-
  documented-convention, which only becomes observable once/if G14 is
  armed.
- **This session's `invariants-expected.toml` recommendation: none.**
  No `[eN]` status moves — this fix is upstream of arming, and arming
  is what would move E3's G14 line. Recorded here so "no
  recommendation" isn't confused with "not checked."

---

## 6. Known remaining gaps (precisely, not swept under the rug)

1. `domain_metadata.yaml`'s `kyc.cases`/`kyc.ownership_snapshots`
   entries use a non-existent `kyc` schema — should read
   `ob-poc.cases`/`ob-poc.ownership_snapshots`. Affects
   `session.set-case`, `ownership.compute`, `ownership.snapshot.list`.
2. `domain_metadata.yaml`'s `team.create`/`team.add-member`/
   `team.remove-member` entries are bare (`teams`, `memberships`) but
   `config/verbs/team.yaml` declares `crud_mapping.schema: teams` for
   them — `qualify_footprint_table` will produce
   `ob-poc.teams`/`ob-poc.memberships`, which will not match a real
   `record_write` capture of `teams.teams`/`teams.memberships`.

Both are narrow, pre-existing (not introduced by this session), and
require editing `domain_metadata.yaml`'s actual content — a
data-correction follow-up, not a table-name-*format* fix. Recommend a
future session either (a) hand-correct these 5 entries directly, or
(b) build a small `verb_fqn → real crud schema` lookup (via
`runtime_registry()`, per §1.3's parenthetical) to override
`qualify_footprint_table`'s default for verbs with a registered
`RuntimeCrudConfig`, falling back to the documented-convention default
for plugin verbs with no single CRUD table. Neither attempted this
session (scope discipline — this session's charter was the
table-name-*format* mismatch specifically).

Once these are corrected (or the wider registry-driven derivation from
option (b) lands), the derivation for `capability-binding.draft`-shaped
verbs (Insert/Update/Upsert with explicit `returning`) becomes
genuinely arming-ready on the table-name axis. Per the prior session's
own framing and this program's "G14 is the ONE production-behavior
change" discipline: **arming `set_expected_write_set` should still be
its own, separately reviewed diff**, not bundled with this fix or the
follow-up data corrections above.

---

## 7. Files changed

- `rust/src/agent/control_plane_shadow.rs` — new `qualify_footprint_table`
  (pure, private); `build_write_set_input` now schema-qualifies
  `WriteSetInput.tables`; 5 new tests, 2 updated stale assertions
  (`derived_columns_are_correct_but_table_name_format_does_not_match_captured_writes`
  renamed/reversed to
  `derived_columns_are_correct_and_table_name_now_matches_captured_writes`;
  `build_write_set_input_some_with_tables_when_footprint_declares_writes`'s
  `ws.tables` assertion updated).

No migration, no YAML, no other Rust file touched.
`invariants-expected.toml` untouched (no recommendation this session —
see §5). Pre-existing dirty files left untouched:
`observatory-wasm/Cargo.lock`, `rust/cbu_mismatches.json`,
`rust/mismatches.json`, `rust/reports/phase0_confusion_matrix.json`,
`rust/reports/step0_trial_evaluation.json`.

Nothing committed by this session — working tree left for independent
review.
