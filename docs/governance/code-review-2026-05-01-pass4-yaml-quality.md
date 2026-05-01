# Code Review — Pass 4: Authored YAML quality

**Commit:** `2642126b "Implement SemOS DAG architecture phases"`
**Files in scope:**
- 21 shape rules in `rust/config/sem_os_seeds/shape_rules/*.yaml`
- 17 amended `struct_*.yaml` constellation maps in `rust/config/sem_os_seeds/constellation_maps/`
- 7 amended DAG taxonomies in `rust/config/sem_os_seeds/dag_taxonomies/` (cbu, deal, kyc, instrument_matrix, booking_principal, book_setup, semos_maintenance)
- new state machine `rust/config/sem_os_seeds/state_machines/cbu_evidence_lifecycle.yaml`
- governance: `docs/governance/dag-schema-coordination-warnings-1_5b.yaml`

**Build status:** N/A (content review). `cargo test -p dsl-core` passes (carried over from Pass 3).
**Reviewer:** Zed-Claude
**Date:** 2026-05-01

## Summary

15 findings: 4 MUST-FIX, 6 SHOULD-FIX, 4 CONSIDER, 1 NOTE. The Lux SICAV pilot's gate-metadata authoring on the **CBU DAG** side (`cbu`, `entity_proper_person`, `entity_limited_company_ubo`, `cbu_evidence`, `share_class`, `manco`) is structurally clean — closure values match each slot's nature, eligibility kinds are plausible, entry_states match each slot's entry-flagged state — with one exception: `cbu_evidence.entry_state: PENDING` does not match the state machine's actual entry-flagged state `UPLOADED` (P4-001). The pilot's **constellation-map** authoring of `administrator` / `auditor` from the AIF RAIF sibling template is structurally faithful (the new slots' join/cardinality/depends_on/state_machine/overlays/verbs blocks are byte-equivalent to the AIF RAIF reference, with the gate-metadata block additionally added per Phase 1.5C scope). The Phase 1.5C explicitly-deferred `domiciliation_agent` is correctly absent and surfaced via the shape rule's `deferred_roles` list.

The substantive content defects are at the cross-file integrity layer: (a) the cbu_dag.yaml `cbu.VALIDATED.green_when` references `every mandate.state in {APPROVED, ACTIVE}` (uppercase), but the `trading_profile_lifecycle.yaml` standalone state machine that owns `mandate`'s lifecycle uses lowercase state IDs (`approved`, `active`) — the predicate will silently never evaluate green (P4-002); (b) the inline `cbu_evidence` state machine in `cbu_dag.yaml` and the standalone `cbu_evidence_lifecycle.yaml` are both authored with the same `id` but different state IDs, terminal_states, and transition shapes — two conflicting sources of truth for the same lifecycle (P4-003); (c) the `mandate` slot's closure is set to `closed_unbounded` in `lux_ucits_sicav.yaml` (shape rule) and `closed_bounded` in `struct_lux_ucits_sicav.yaml` (constellation map), neither location being the authoritative owner per §6.3 (P4-004).

The shape-rule layer is a thin substrate. The three intended-ancestor shape rules (`base_cbu`, `regulated_fund`, `ucits`) are all empty (`slots: {}`, no structural_facts), so the 4-tier cascade has no ancestor content to compose. Of 18 leaf shape rules, only `lux_ucits_sicav.yaml` carries authored slot-level gate-metadata; the other 17 carry only structural_facts. This is consistent with §10.6's pilot scope (Lux SICAV only) but the broader sweep is empty (P4-009). Two shape rules (`hedge_cross_border`, `ie_hedge_icav`) list `prime-broker` in both `required_roles` and `optional_roles` (P4-005). Two shape rules carry unresolved template placeholders (`${arg.master_jurisdiction.internal}`, `${arg.fund_type.internal}`) in `structural_facts` fields where literal authored values are expected (P4-006).

No `+` sigil usage anywhere in the seeds; the parse-time guard verified in Pass 3 has no occurrences to defend against today. No state-machine refinement primitives, no cross-workspace constraint directives in any shape rule — Phase 2 territory per spec.

## Findings

### MUST-FIX

#### [MUST-FIX] [QUALITY] [COMPLIANCE] P4-001 — `cbu_evidence` slot's `entry_state: PENDING` does not match the inline state machine's entry-flagged state `UPLOADED`

**File:** `rust/config/sem_os_seeds/dag_taxonomies/cbu_dag.yaml`
**Lines:** 528 (`entry_state: PENDING`); 549–550 (`UPLOADED` flagged `entry: true`)
**Spec reference:** §10.5 (`entry_state` annotation must be authored on the slot per Phase 1.5B); D-018 table row for `entry_state` ("State not in resolved machine → ResolveError").

**Observation:**

```yaml
- id: cbu_evidence
  stateless: false
  closure: closed_unbounded
  entry_state: PENDING                          # <-- says PENDING
  state_machine:
    id: cbu_evidence_lifecycle
    states:
      - id: UPLOADED
        entry: true                             # <-- entry flag here
        description: "Evidence uploaded and linked; review not complete."
      - id: REVIEWED
      - id: APPROVED
      - id: REJECTED
      - id: EXPIRED
```

The slot's gate-metadata says `entry_state: PENDING`. The inline state machine has no `PENDING` state — the state machine was renamed during this commit from `PENDING/VERIFIED/REJECTED/EXPIRED` to `UPLOADED/REVIEWED/APPROVED/REJECTED/EXPIRED` (per the diff at line 540–588), but the `entry_state` annotation on the slot was not updated.

**Issue:** D-018 mandates that `entry_state` must be a valid state in the resolved machine (`State not in resolved machine → ResolveError`). The schema validator does not currently enforce this consistency check (Pass 3 surfaces no `EntryStateUnknown` lint), so the defect compiles, parses, and runs without error. At runtime, the dispatcher would treat the slot as having an undefined entry state, which is a hidden gate failure.

The cbu_dag.yaml's `cross_slot_constraints` (line 1304-1310 in diff context) was correctly updated to track the rename (`status = 'VERIFIED'` → `'APPROVED'`); the slot's own `entry_state` annotation was missed.

**Disposition (suggested):** change `entry_state: PENDING` → `entry_state: UPLOADED` on `cbu_evidence`. Add a dag-validator lint that asserts every `slot.entry_state` is a state-id in the slot's state_machine.states (or in the entry-flagged subset).

---

#### [MUST-FIX] [QUALITY] [COMPLIANCE] P4-002 — `cbu.VALIDATED.green_when` references uppercase mandate states; `trading_profile_lifecycle` is lowercase

**File:** `rust/config/sem_os_seeds/dag_taxonomies/cbu_dag.yaml`, `rust/config/sem_os_seeds/state_machines/trading_profile_lifecycle.yaml`
**Lines:** cbu_dag.yaml:341–347 (predicate); trading_profile_lifecycle.yaml:3–11 (state IDs)
**Spec reference:** I2 (destination predicates are postconditions); §4.1 worked CBU example uses `mandate.state ∈ {APPROVED, ACTIVE}`; §6.3 ("the mandate lifecycle is owned by Instrument Matrix as `trading_profile`").

**Observation:**

`cbu_dag.yaml` cbu slot's `VALIDATED` green_when:

```yaml
green_when: |
  every entity_proper_person.state = VERIFIED
  AND every entity_limited_company_ubo.state in {DISCOVERED, PUBLIC_FLOAT, EXEMPT}
  AND every mandate.state in {APPROVED, ACTIVE}            # <-- uppercase
  AND every cbu_evidence.state = APPROVED
  AND no investor_disqualifying_flag exists
  AND investment_managers.completeness = green
```

`trading_profile_lifecycle.yaml`:

```yaml
state_machine: trading_profile_lifecycle
states:
  [draft, submitted, approved, active, suspended, archived, rejected]
                     ^^^^^^^^  ^^^^^^                       <-- lowercase
```

The predicate compares against `APPROVED` and `ACTIVE`; the actual states are `approved` and `active`. String comparison is case-sensitive in the evaluator (Pass 2 hydrator's `cmp_string_or_number`).

**Issue:** the predicate is parsable, the predicate-binding for `mandate` exists (cbu_dag.yaml line 320–323), the validator's unbound-entity check passes — but at evaluation time, the state-set lookup will never match. The `cbu.VALIDATED` postcondition is silently un-satisfiable from this clause forever.

This is the practical consequence of the convention drift between inline-DAG state machines (UPPERCASE state IDs throughout `cbu_dag.yaml`, `kyc_dag.yaml`, etc.) and standalone state machine YAMLs (lowercase state IDs throughout `state_machines/*.yaml`). The §4.1 worked example uses uppercase; the actual authored state machine uses lowercase. One of the two has to give.

**Disposition (suggested):** decide canonical case (the inline-DAG convention is dominant; the standalone state_machines/*.yaml directory should be normalised to UPPERCASE). Until then, change the predicate to match: `every mandate.state in {approved, active}`. Add a validator lint that resolves predicate-binding state references against the bound entity's state machine and rejects unknown state IDs (extends Pass 3 P3-002's slot-id-vs-entity-kind concern).

---

#### [MUST-FIX] [QUALITY] [COMPLIANCE] P4-003 — Two conflicting `cbu_evidence_lifecycle` definitions: inline (in cbu_dag.yaml) vs standalone (state_machines/cbu_evidence_lifecycle.yaml)

**File:** `rust/config/sem_os_seeds/dag_taxonomies/cbu_dag.yaml` (inline state machine, lines 525–588), `rust/config/sem_os_seeds/state_machines/cbu_evidence_lifecycle.yaml` (standalone, lines 1–53)
**Spec reference:** I1 (strict capability separation); D-016 (state-machine extension primitives presuppose a single base machine).

**Observation:** both files declare a state machine with id `cbu_evidence_lifecycle` but diverge:

| Field | Inline (cbu_dag.yaml) | Standalone (state_machines/...) |
|---|---|---|
| state IDs | `UPLOADED`, `REVIEWED`, `APPROVED`, `REJECTED`, `EXPIRED` | `uploaded`, `reviewed`, `approved`, `rejected`, `expired` |
| terminal_states | `[APPROVED, REJECTED]` | (not declared) |
| transition `approved → expired` | `via: "(backend: time-decay trigger ...)"` (free text, not a verb) | `verbs: [evidence.expire]` |
| transition schema key | `via: <scalar>` | `verbs: [<list>]` |
| green_when on states | UPLOADED/REVIEWED/APPROVED/EXPIRED carry `green_when` predicates | (states are bare; no green_when) |
| reducer | (none) | `reducer:` with `overlay_sources`, `conditions`, `rules` block |

**Issue:** D-016's named-primitive contract (`insert_between`, `add_branch`, etc.) presupposes a single canonical base state machine that shape rules refine. Two parallel definitions for the same `id` create ambiguity: which is authoritative? The Resolver's `compose_transitions` reads only the inline-DAG version (Pass 1 P1-003); the standalone YAML is loaded by the runtime's separate state-machine-config layer. In effect, two consumers read two different "truths" for the same logical lifecycle.

This is a Pass 4 content defect rooted in the broader Pass 1 architecture defect (state-machine primitives unimplemented, standalone state_machines/ directory not yet integrated with the resolver).

**Disposition (suggested):** decide which is authoritative — most likely the inline cbu_dag.yaml form (which carries gate-metadata, predicate_bindings, and green_when), with the standalone YAML repurposed as a reducer-condition specification once D-009's reducer-migration workstream begins. Until then, normalise state IDs and transition schemas to match (rename one or the other), and document which file the runtime reads.

---

#### [MUST-FIX] [QUALITY] [COMPLIANCE] P4-004 — `mandate` closure is conflictingly authored in shape rule and constellation map; neither location owns its lifecycle

**File:** `rust/config/sem_os_seeds/shape_rules/lux_ucits_sicav.yaml` (line 31–33), `rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` (line 244)
**Spec reference:** §6.3 ("The mandate lifecycle is *not* a CBU DAG slot; it is owned by Instrument Matrix as `trading_profile`"); D-018 (4-tier cascade).

**Observation:**

`lux_ucits_sicav.yaml` shape rule:
```yaml
slots:
  mandate:
    closure: closed_unbounded                  # <-- shape rule says unbounded
    entry_state: empty
```

`struct_lux_ucits_sicav.yaml` constellation map:
```yaml
mandate:
  type: mandate
  closure: closed_bounded                       # <-- constellation map says bounded
  table: cbu_trading_profiles
  ...
```

Two values for `mandate.closure` on the same `(workspace=cbu, shape=struct.lux.ucits.sicav)` pair. Per Pass 1 P1-001 the seed-step ranks DAG above constellation (incorrect) and Pass 1 P1-002 there's no shape-rule conflict detection — so the resolver's output is determined by the order in `compose_slot`, not by spec.

Per §6.3, the mandate's lifecycle is owned by Instrument Matrix's `trading_profile` slot. Neither the CBU shape rule nor the CBU constellation map should be declaring closure on a slot whose authoritative lifecycle lives in a different workspace. Both authoring locations are semantically wrong; the conflict between them is a downstream symptom.

**Issue:** even if the conflict didn't exist, the closure annotation should not be on the `mandate` slot in the CBU constellation/shape — it should be on `instrument_matrix::trading_profile`. The CBU side should expose the slot for hydration only, with closure inherited from the cross-workspace owner.

**Disposition (suggested):** remove `closure` from both authoring sites for the `mandate` slot. If a hydration-side annotation is needed, surface it as a derived value resolved through the IM workspace's authored DAG. Add a closure-ownership lint: only the workspace that owns a slot's state machine may author its closure.

---

### SHOULD-FIX

#### [SHOULD-FIX] [QUALITY] P4-005 — `prime-broker` is in both `required_roles` and `optional_roles` on two shape rules

**File:** `rust/config/sem_os_seeds/shape_rules/hedge_cross_border.yaml` (lines 10–11), `rust/config/sem_os_seeds/shape_rules/ie_hedge_icav.yaml` (lines 10–11)
**Spec reference:** §7 (shape-aware authoring).

**Observation:**

`hedge_cross_border.yaml`:
```yaml
required_roles: [aifm, depositary, prime-broker]
optional_roles: [investment-manager, administrator, auditor, prime-broker]
```

`ie_hedge_icav.yaml`:
```yaml
required_roles: [aifm, depositary, prime-broker]
optional_roles: [investment-manager, administrator, auditor, prime-broker, executing-broker]
```

`prime-broker` appears in both lists.

**Issue:** by definition, a role cannot be both required (must be present) and optional (may be absent). The contradiction is a content authoring error — one of the two list memberships is redundant or wrong. The `shape_rule_composition_extracts_ie_hedge_icav_macro_facts` test (`shape_rule_composition.rs:271–310`) asserts this exact authoring as the expected output, which means the test fixture was authored to match the bug rather than catch it.

**Disposition (suggested):** decide whether `prime-broker` is required or optional for hedge funds. If "required for prime-broker-touching strategies, optional for others," the single-shape model can't represent that — split into two shapes or move the conditionality into a downstream gate predicate. Update both shape rules and the corresponding test assertions.

---

#### [SHOULD-FIX] [QUALITY] [COMPLIANCE] P4-006 — Two shape rules carry unresolved template placeholders in `structural_facts`

**File:** `rust/config/sem_os_seeds/shape_rules/hedge_cross_border.yaml` (line 5), `rust/config/sem_os_seeds/shape_rules/us_private_fund_delaware_lp.yaml` (line 9), `rust/config/sem_os_seeds/shape_rules/pe_cross_border.yaml` (line 5)
**Spec reference:** §3.3 (`StructuralFacts` is authored declarative content, consumed by Resolver as fixed inputs); §9.2 ("The algorithm is a pure function. No I/O during composition (inputs are loaded into memory once at Resolver construction time)").

**Observation:**

```yaml
# hedge_cross_border.yaml
structural_facts:
  jurisdiction: "${arg.master_jurisdiction.internal}"

# pe_cross_border.yaml
structural_facts:
  jurisdiction: "${arg.main_fund_jurisdiction.internal}"

# us_private_fund_delaware_lp.yaml
structural_facts:
  trading_profile_type: "${arg.fund_type.internal}"
```

These `${arg....}` placeholders are template variables that get substituted by a downstream macro / runbook expansion — they are not literal authored values.

**Issue:** the Resolver consumes `structural_facts.jurisdiction` as a fact (`Some("UK")`, `Some("LU")`, etc.). When the input is a placeholder string `"${arg.master_jurisdiction.internal}"`, the composer stores that literal string as the resolved fact. Downstream consumers reading `template.structural_facts.jurisdiction.as_deref()` get the placeholder string, not a resolved jurisdiction code. There is no resolver-side substitution.

The `shape_rule_composition_extracts_cross_border_macro_facts` test (`shape_rule_composition.rs:582–613`) asserts these placeholder strings as the expected output — the test is fixture-aligned with the bug. A consumer using `template.structural_facts.jurisdiction` to filter or index would see `"${arg.master_jurisdiction.internal}"` as a real jurisdiction value.

**Disposition (suggested):** structural_facts are authored truths, not template placeholders. Either (a) author concrete values (e.g., `jurisdiction: cross-border` as a sentinel), (b) make the field `Option<EnumOrPlaceholder>` and explicitly model the deferred-substitution case, or (c) move cross-border shapes into a separate authoring layer where placeholders are recognised. The current state pollutes the structural facts surface.

---

#### [SHOULD-FIX] [QUALITY] [COMPLIANCE] P4-007 — Convention drift: inline-DAG state machines use UPPERCASE state IDs; standalone state_machines/*.yaml use lowercase

**File:** `rust/config/sem_os_seeds/dag_taxonomies/*.yaml` (all 12) vs `rust/config/sem_os_seeds/state_machines/*.yaml` (21 files)
**Spec reference:** I1 (strict capability separation); §6.3 (cross-workspace state references).

**Observation:** every inline state machine in the DAG taxonomies uses UPPERCASE state IDs (`DRAFT`, `DISCOVERED`, `VALIDATED`, `APPROVED`, `REJECTED`, etc.); every standalone state machine YAML uses lowercase (`draft`, `submitted`, `approved`, `active`, `suspended`, etc.). This is observable across all 12 DAG YAMLs and all 21 state_machines/ YAMLs.

The drift is the root cause of P4-002 (cbu's predicate referencing `APPROVED` but trading_profile having `approved`) and is latent for any future cross-workspace predicate that references a slot whose lifecycle is owned by a standalone state machine.

**Issue:** the architecture document doesn't specify a state-ID casing convention. Both forms exist; both are consumed by different code paths. Cross-references between conventions silently fail (P4-002 is one example).

**Disposition (suggested):** ratify a convention — UPPERCASE is dominant in the inline DAGs, the existing `cbu_dag.yaml` rename batch even normalised `[CONTRACTED, ACTIVE]`-style sets — and migrate the 21 standalone YAMLs to match. Add a build-time invariant test: every state ID across all DAG taxonomies and all standalone state machines is UPPERCASE (or all-lowercase, whichever is chosen).

---

#### [SHOULD-FIX] [QUALITY] [COMPLIANCE] P4-008 — `cbu.VALIDATED.green_when` references invented entity bindings without DAG-side carrier definition

**File:** `rust/config/sem_os_seeds/dag_taxonomies/cbu_dag.yaml`
**Lines:** 341–347
**Spec reference:** I2 (destination predicates as postconditions); D-019 (predicate-binding entity must be bindable to substrate).

**Observation:**

```yaml
predicate_bindings:
  - entity: investor_disqualifying_flag
    source_kind: dag_entity
    scope: attached_to this cbu
  - entity: investment_managers
    source_kind: dag_entity
    scope: completeness assertion for investment-manager role population
```

```yaml
green_when: |
  ...
  AND no investor_disqualifying_flag exists
  AND investment_managers.completeness = green
```

`investor_disqualifying_flag` and `investment_managers` are bound as `dag_entity` (i.e., not yet to a substrate carrier). `source_entity:` is omitted on both — they have no substrate path. Compare with `entity_proper_person`, `entity_limited_company_ubo`, `mandate`, `cbu_evidence` (the same predicate_bindings list line 312–325) which are also `dag_entity`-only but have corresponding slots in cbu_dag.yaml that DO map to substrate (`"ob-poc".entity_proper_persons`, etc., via the inline state-machine `source_entity` field).

`investor_disqualifying_flag` and `investment_managers` are NOT slot ids in cbu_dag.yaml. There's no slot `investor_disqualifying_flag`, no slot `investment_managers`, and no other DAG taxonomy declares them. They are invented entity references that have no carrier.

**Issue:** Pass 3's `validate_green_when_predicates` accepts these because the predicate_bindings list contains them (the validator only checks "is this entity bound somewhere?"). But the bindings themselves are stubs — `dag_entity` without `source_entity` cannot be evaluated against substrate. At runtime, the predicate clause `no investor_disqualifying_flag exists` cannot be evaluated and either silently passes (treated as no rows) or silently fails (treated as evaluation error).

The `investment_managers.completeness = green` clause uses an attribute name `completeness` that has no defined column and no defined semantics — the `scope: completeness assertion for investment-manager role population` is documentation prose, not a binding.

**Disposition (suggested):** either author concrete substrate carriers for both entities (`source_entity: '"ob-poc".cbu_disqualifying_flags'` etc.), or remove the clauses and replace with a well-defined alternative (e.g., aggregate over `cbu_red_flags`). The current state encodes "we know what we want, we haven't built it yet" inside an authored predicate that the validator cannot detect as incomplete.

---

#### [SHOULD-FIX] [QUALITY] P4-009 — The shape-rule layer is a thin substrate; only `lux_ucits_sicav.yaml` carries authored slot-level gate metadata

**File:** `rust/config/sem_os_seeds/shape_rules/*.yaml` (21 files)
**Lines:** all leaf shape rules except `lux_ucits_sicav.yaml`

**Observation:**

| Shape rule | structural_facts | slot refinements |
|---|---|---|
| base_cbu.yaml | empty | `{}` |
| regulated_fund.yaml | empty | `{}` |
| ucits.yaml | empty | `{}` |
| lux_ucits_sicav.yaml | full | 6 slots authored |
| 17 other leaves | full | `{}` |

The 4-tier cascade (D-018 rule 1) "leaf shape rule → ancestor shape rule → constellation map → DAG taxonomy → default" has no ancestor content to compose. Every fact comes from either the leaf shape's `structural_facts` (via P4-006 the trading-profile/jurisdiction route) or DAG/constellation directly.

**Issue:** consistent with §10.6's pilot scope (Phase 1.5C touches Lux SICAV only). However, the current state has two implications:
1. The three ancestor shape rules (`base_cbu`, `regulated_fund`, `ucits`) carry no authored content — they're file-system markers for the `extends:` graph but not meaningful inheritance points. `lux_ucits_sicav.yaml`'s `extends: [ucits]` could be `extends: [base.cbu]` or `extends: [regulated.fund]` with no information loss.
2. Across the 17 non-pilot leaf shapes (`lux_aif_raif`, `ie_ucits_icav`, `us_40act_open_end`, etc.), no slot-level gate metadata is authored. Their constellation maps may carry the equivalent, but the shape-rule layer is empty for them — meaning any closure / eligibility / cardinality_max inferred from the shape's identity is missing.

**Disposition (suggested):** consistent with §10.8's Phase 2 sweep schedule. For Phase 1.5C/D as shipped, no remediation needed beyond documenting that the ancestor rules are placeholder until Phase 2. If the ancestor rules will *never* carry content (i.e., each leaf duplicates structural_facts), simplify the `extends:` graph to be flat.

---

#### [SHOULD-FIX] [QUALITY] [COMPLIANCE] P4-010 — Phase 1.5C Acceptance #5 ("byte-comparable to lux_aif_raif modulo intentional differences") is technically unmet; the SICAV slots have additional gate metadata not present in the AIF RAIF template

**File:** `rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml` (lines 117–192, admin/auditor); `rust/config/sem_os_seeds/constellation_maps/struct_lux_aif_raif.yaml` (lines 104–163, admin/auditor)
**Spec reference:** §10.6 acceptance #5 ("the new slots in `struct_lux_ucits_sicav.yaml` are byte-comparable to the equivalents in `struct_lux_aif_raif.yaml` modulo intentional UCITS-vs-AIF differences").

**Observation:** the new SICAV `administrator` and `auditor` slots include the gate-metadata fields:

```yaml
administrator:
  type: entity
  entity_kinds: [company]
  closure: closed_bounded            # <-- not in AIF RAIF template
  eligibility:                       # <-- not in AIF RAIF template
    entity_kinds: [company]
  cardinality_max: 1                 # <-- not in AIF RAIF template
  entry_state: empty                 # <-- not in AIF RAIF template
  join: ...
  cardinality: optional
  ...
```

The AIF RAIF version has only the structural fields (`type`, `entity_kinds`, `join`, `cardinality`, `depends_on`, `placeholder`, `state_machine`, `overlays`, `verbs`). The SICAV version adds the four Phase 1.5B gate-metadata fields.

**Issue:** acceptance #5's "byte-comparable" framing was authored in v0.4 of the architecture, before Phase 1.5C scope was clarified to also annotate gate metadata on the new slots. The SICAV admin/auditor slots are *structurally* identical to the AIF RAIF template (the join/cardinality/depends_on/state_machine/overlays/verbs blocks match line-for-line) but carry additional gate-metadata authoring per Phase 1.5C scope. The acceptance criterion as worded is unmet; the substantive intent (the structural bones come from the template) is met.

**Disposition (suggested):** either back-fill the AIF RAIF template with the gate-metadata fields (extends Phase 1.5C scope to cover AIF RAIF), or reword acceptance #5 to "structural blocks (join, cardinality, depends_on, state_machine, overlays, verbs) are byte-comparable; gate-metadata authoring is intentional per Phase 1.5C scope." The current state — bigger SICAV than the template it was sourced from — is the right direction even if it bends the literal acceptance.

---

### CONSIDER

#### [CONSIDER] [STYLE] P4-011 — `extends:` taxonomy is inconsistent: PE shapes, manager LLP, and US private funds extend `base.cbu` while regulated funds (UCITS, AIF, 40-Act) extend `regulated.fund`

**File:** `rust/config/sem_os_seeds/shape_rules/*.yaml`

**Observation:**
- Extends `regulated.fund`: lux_aif_raif, lux_ucits_sicav (via ucits), ie_aif_icav, ie_hedge_icav, ie_ucits_icav, hedge_cross_border, uk_authorised_*, us_40act_*, us_etf_40act
- Extends `base.cbu` directly: lux_pe_scsp, pe_cross_border, uk_manager_llp, uk_private_equity_lp, us_private_fund_delaware_lp

**Issue:** the bypass is semantically signposted (PE / manager / private fund are not "regulated funds" in the UCITS/40-Act sense) but factually equivalent (since `base_cbu`, `regulated_fund`, `ucits` are all empty). The intent is clear; the structure doesn't enforce it. Future authors may be confused about whether to introduce a `private_fund` ancestor or extend `base_cbu` directly.

**Disposition (suggested):** introduce ancestors for the non-regulated branches (e.g., `private_fund`, `manager`) to make the taxonomy structurally coherent. Or document the convention in `base_cbu.yaml`'s comment.

---

#### [CONSIDER] [QUALITY] P4-012 — `lux_ucits_sicav.yaml::structural_facts.deferred_roles: [domiciliation-agent]` is the right authoring shape; the resolver does not yet consume it

**File:** `rust/config/sem_os_seeds/shape_rules/lux_ucits_sicav.yaml` (line 12)

**Observation:** the shape rule correctly surfaces the explicitly-deferred role per Phase 1.5C scope:

```yaml
deferred_roles: [domiciliation-agent]
```

`StructuralFacts.deferred_roles: Vec<String>` is composed by the resolver and exposed on `ResolvedTemplate.structural_facts.deferred_roles`. The resolver's tests assert it on the SICAV pilot. No downstream consumer — gate dispatcher, Frontier, agent prompt — currently reads `deferred_roles`.

**Issue:** the field is correctly authored but currently has no effect. Authors who add a `deferred_roles` entry in another shape would observe no behavioural change, leading to "I wrote it; nothing happened" confusion.

**Disposition (suggested):** wire `deferred_roles` into the agent prompt or the `ResolverManifest` text output so authoring it has an observable effect. Or document that `deferred_roles` is currently a documentation field consumed only by the manifest reviewer.

---

#### [CONSIDER] [QUALITY] P4-013 — Schema-coordination known-deferred allowlist contains exactly one entry; `deal_lifecycle` state-machine name mismatch with no Phase 2 ratification yet

**File:** `docs/governance/dag-schema-coordination-warnings-1_5b.yaml`
**Spec reference:** D-011 (warnings harden to errors at end of Phase 2); §9.7.

**Observation:**

```yaml
warnings:
  - id: deal_lifecycle_state_machine_name_mismatch
    source: rust/config/sem_os_seeds/constellation_maps/deal_lifecycle.yaml
    constellation: deal.lifecycle
    slot_id: deal
    dag_state_machine: deal_commercial_lifecycle
    constellation_state_machine: deal_lifecycle
    sunset_commitment: "Resolve by renaming or reconciling the deal constellation state_machine reference before Phase 2 closure, or ratify a D-NNN exception before merge."
```

One known-deferred entry. No D-NNN ratification yet. The `strict_authored_seed_schema_coordination_preserves_known_deferred_only` test asserts exactly one warning matches this entry.

**Issue:** the entry is honest (it's a real authoring drift) and the validator is correctly configured to retain it as a warning until Phase 2. The CI gate enforces "no new mismatches" via the strict-mode test. The classification:

- (a) **genuine drift to address in Phase 2:** yes. The Deal commercial lifecycle vs the constellation map's `deal_lifecycle` reference is a name mismatch the Deal team needs to reconcile. No silent path forward; the sunset commitment names the merge-blocking gate.
- (b) artifact of incomplete authoring: no.
- (c) misclassification: no.

The structure is sound, but the single-entry allowlist signals that drift detection is currently sparse — either the codebase has very few schema-coordination issues (good), or the validator's coverage is narrower than the spec describes.

**Disposition (suggested):** confirm with the Deal workstream owner that the rename is scheduled in Phase 2. Add an acceptance check that the allowlist size cannot grow without an explicit D-NNN ratification.

---

#### [CONSIDER] [STYLE] P4-014 — `cbu_evidence` standalone state_machine YAML uses `verbs:` (list) but the inline form uses `via:` (scalar)

**File:** `rust/config/sem_os_seeds/state_machines/*.yaml` (`verbs:` list form), `rust/config/sem_os_seeds/dag_taxonomies/*.yaml` (`via:` scalar/sequence form)
**Spec reference:** D-016 (state-machine refinement).

**Observation:** transition schemas differ across the two file conventions:

```yaml
# state_machines/*.yaml
- from: uploaded
  to: reviewed
  verbs: [cbu.review-evidence]      # <-- list

# dag_taxonomies/*.yaml
- from: UPLOADED
  to: REVIEWED
  via: cbu.review-evidence           # <-- scalar
```

`TransitionDef.via` is `Option<YamlValue>` (Pass 1 P1-009). `verbs:` isn't recognised by the inline-DAG parser at all — the standalone YAML uses a different schema entirely.

**Issue:** Pass 1 P1-009 already flagged that the YamlValue round-trip in `compose_transitions` is fragile. Compounded here: the standalone state machine YAMLs use `verbs:` (a key the inline parser doesn't recognise), so they cannot share schema with the inline form. Migrating standalone state machines to the inline form (per P4-003's resolution) requires renaming `verbs:` → `via:` across all 21 standalone files.

**Disposition (suggested):** part of the P4-003 / P4-007 normalisation. Decide one schema; migrate the other.

---

### NOTE

#### [NOTE] P4-015 — No `+` sigil usage and no state-machine refinement primitives anywhere in the seeds

**File:** `rust/config/sem_os_seeds/shape_rules/*.yaml`, `rust/config/sem_os_seeds/dag_taxonomies/*.yaml`, `rust/config/sem_os_seeds/constellation_maps/*.yaml`

**Observation:** searched all seed files for `+attachment_predicates`, `+addition_predicates`, `+aggregate_breach_checks`, `tighten_constraint`, `add_constraint`, `replace_constraint`, `insert_between`, `add_branch`, `add_terminal`, `refine_reducer`, `raw_add`, `raw_remove`. Zero matches.

**Issue:** consistent with §10.8's Phase 2 sweep being unstarted. The code surface for these primitives is in place (Pass 1 P1-003 / P1-004 noted both as deserialized-but-unused), but no authored YAMLs exercise them. The Pass 3 `+`-sigil parse-time guard has nothing to defend against today; the unused state-machine primitives have nothing to compose.

**Disposition:** no action. Re-verify after Phase 2 authoring.

---

## Coverage notes

**What this pass covered:**
- All 21 shape rule YAMLs read end-to-end (the largest, `lux_ucits_sicav.yaml`, is 45 lines; total ~250 LOC across 21 files).
- `cbu_dag.yaml` diff (+173 lines) read in full; cross-checked entry_state values against state machine entries; cross-checked predicate_bindings against authored states.
- Lux SICAV constellation map's admin/auditor diff (+87 lines) cross-referenced against `struct_lux_aif_raif.yaml` template.
- New state machine `cbu_evidence_lifecycle.yaml` cross-checked against the inline cbu_dag.yaml form.
- Schema-coordination known-deferred YAML.
- All 12 DAG taxonomies queried for `cross_workspace_constraints`, `replaceable_by_shape`, `extensible_by_shape`.
- D-015 (cross-workspace constraint precedence), D-016 (state-machine refinement primitives), D-018 (per-field merge precedence) checked against authored YAMLs.

**What this pass deliberately did not cover:**
- Domain plausibility of the 17 non-pilot shape rules' role lists (e.g., is `ltaf` actually required to have an `authorised-corporate-director`? — that's a fund-structure-expert question outside the reviewer's domain).
- Other DAG taxonomy diffs (deal, kyc, instrument_matrix, booking_principal, book_setup, semos_maintenance) — sampled but not exhaustively reviewed for Phase 1.5C-equivalent gate-metadata coverage.
- Constellation map diffs other than Lux SICAV (the 16 other amended constellation maps are -1 line each per the diff, removing the legacy `bulk_macros` block — a non-substantive cleanup).

**Inconclusive / verified by code only:**
- Whether the convention drift (P4-007) is fixable without breaking external consumers — depends on whether any production code references state IDs in either case form.
- Whether `investor_disqualifying_flag` and `investment_managers` (P4-008) have planned substrate carriers — would need review with the CBU domain owner.

## Recommended next steps

In priority order — the four MUST-FIX findings are the load-bearing content defects.

1. **MUST-FIX** P4-001 (cbu_evidence entry_state mismatch): one-line YAML fix; add validator lint.
2. **MUST-FIX** P4-002 (mandate state casing): the cbu.VALIDATED predicate is silently un-satisfiable; either fix the predicate or the state machine.
3. **MUST-FIX** P4-003 (two cbu_evidence_lifecycle definitions): converge on one source of truth.
4. **MUST-FIX** P4-004 (mandate closure conflict + ownership): both authoring sites are wrong; closure belongs to IM workspace.
5. **SHOULD-FIX** P4-007 (UPPERCASE vs lowercase state ID convention drift): root cause of P4-002 and a future-proofing concern.
6. **SHOULD-FIX** P4-008 (invented entity bindings without carrier): predicate is an aspirational placeholder, not an evaluable rule.
7. **SHOULD-FIX** P4-005 / P4-006 (prime-broker double-listing; template placeholders in structural_facts): content cleanups.
8. **SHOULD-FIX** P4-009 / P4-010: documentation, scope clarifications.
9. **CONSIDER** / **NOTE**: low-priority polish.
