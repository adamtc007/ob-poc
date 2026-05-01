# Decision Log — 2026-04-29 Carrier Completeness + Reconciliation Refactor

> **Companion to** `refactor-todo-2026-04-29.md` and `refactor-inventory.md`.
> **Discipline:** append-only. One entry per significant decision. Reviewed by Adam at refactor completion.

Format per entry:

```
D-NNN — <one-line summary>
  Timestamp: <ISO>
  Phase: <§N>
  Subject: <verb FQN | M-ID | file path | etc.>
  Decision: <verdict>
  Rationale: <1–3 sentences>
  Reversibility: low | medium | high
  Reference: <file:line where the change lands>
```

---

## D-001 — Phase 1 inventory complete

  Timestamp: 2026-04-29
  Phase: §1
  Subject: refactor-inventory.md
  Decision: Inventory written; surfaced 4 blockers (B1–B4) in `refactor-inventory.md`.
  Rationale: Live DB introspection exposed two facts not in the TODO: (a) §2.4 already done; (b) §2.3 has unbackfilled rows. Inventory documents both plus the v1.3 versioning conflict and the §2/§5 ordering dependency.
  Reversibility: high — inventory is descriptive, not actionable.
  Reference: docs/governance/refactor-inventory.md

## D-002 — Phase 6 deferral marker landed at service_pipeline_service_impl.rs:165

  Timestamp: 2026-04-29
  Phase: §6
  Subject: rust/src/services/service_pipeline_service_impl.rs
  Decision: FIXME comment block added before the bypass UPDATE. No behavioural change.
  Rationale: Q16 defers the bypass-write fix to R.1 cleanup; the marker ensures the issue surfaces during R.1 grep.
  Reversibility: high — comment-only.
  Reference: rust/src/services/service_pipeline_service_impl.rs:165

## D-003 — Phase 3: M-036 orphan states REDEEMING, REDEEMED removed

  Timestamp: 2026-04-29
  Phase: §3
  Subject: cbu_dag.yaml::M-036 cbu.investor_lifecycle
  Decision: Removed REDEEMING and REDEEMED state entries; left replacement comment in place.
  Rationale: Q9 (b) — DAG declared both states with descriptive comments but zero transitions reach or leave them (verified at lines 562–579). Carrier-completeness invariant satisfied by removal; redemption flow re-modelled later if needed.
  Reversibility: high — state set is straightforward to reinstate.
  Reference: rust/config/sem_os_seeds/dag_taxonomies/cbu_dag.yaml:552-557

## D-004 — Phase 5.1: M-045 collapsed BAC_APPROVAL + KYC_CLEARANCE → IN_CLEARANCE

  Timestamp: 2026-04-29
  Phase: §5.1
  Subject: deal_dag.yaml::M-045 deal_commercial_lifecycle
  Decision: Replaced BAC_APPROVAL and KYC_CLEARANCE states with single IN_CLEARANCE state. Substates encoded as columns (deals.bac_status, deals.kyc_clearance_status) rather than nested substate_machines (DAG schema does not yet support nested machines). Updated transitions, terminal-negative `from:` lists, overall_lifecycle phase, cross-workspace constraints (retargeted to "IN_CLEARANCE -> CONTRACTED"), book_setup cross-workspace reference. Added intra-DAG cross_slot_constraint `deal_contracted_requires_bac_approved`.
  Rationale: Q4 + Q21 (b) — BAC and KYC clearance are parallel gates, not sequential phases. Encoded with columns because the validator forbids same-workspace cross_workspace_constraints and column-state predicates aren't first-class state-machine states.
  Reversibility: medium — DAG amendment with downstream constraint references.
  Reference: rust/config/sem_os_seeds/dag_taxonomies/deal_dag.yaml; book_setup_dag.yaml lines 33, 378-391

## D-005 — Phase 2.1: cbu_service_consumption carrier extended with S-15 linkage columns

  Timestamp: 2026-04-29
  Phase: §2.1
  Subject: rust/migrations/20260429_carrier_01_cbu_service_consumption.sql
  Decision: Existing table from 20260424_tranche_2_3_dag_alignment.sql extended with `service_id` (FK→services) and `onboarding_request_id` (FK→deal_onboarding_requests.request_id, NOT deal_onboarding_request_id as TODO assumed — column was named differently). Migration is idempotent.
  Rationale: TODO §2.1 assumed table didn't exist; live DB introspection found it. Authoring was reframed as additive ALTER TABLE rather than CREATE TABLE.
  Reversibility: medium — column drop + FK drop reverses cleanly.
  Reference: rust/migrations/20260429_carrier_01_cbu_service_consumption.sql

## D-006 — Phase 2.2: service_intent / cbu_service_consumption semantic comments

  Timestamp: 2026-04-29
  Phase: §2.2
  Subject: rust/migrations/20260429_carrier_02_service_intent_comments.sql
  Decision: COMMENT ON TABLE for service_intents pinning the M-026 semantic distinction. cbu_service_consumption COMMENT set in carrier_01.
  Rationale: Q2 (a) — intent layer (3 states) coexists with operational layer (6 states); comments make the distinction discoverable.
  Reversibility: high — comment-only.
  Reference: rust/migrations/20260429_carrier_02_service_intent_comments.sql

## D-007 — Phase 2.5 + B1: deals.operational_status added with ONBOARDING backfill

  Timestamp: 2026-04-29
  Phase: §2.5
  Subject: rust/migrations/20260429_carrier_03_deals_operational_status.sql
  Decision: Added deals.operational_status column (5-state CHECK). Backfill executed: 2 ONBOARDING rows moved to deal_status='CONTRACTED', operational_status='ONBOARDING'. Defensive backfill for ACTIVE/SUSPENDED/WINDING_DOWN/OFFBOARDED rows (none in live DB).
  Rationale: B1 — under the new commercial-only deal_status set, ONBOARDING is no longer valid; rows must be migrated before carrier_04 tightens the CHECK.
  Reversibility: medium — backfill not symmetric (operational_status='ONBOARDING' rows can't trivially reverse to deal_status='ONBOARDING').
  Reference: rust/migrations/20260429_carrier_03_deals_operational_status.sql

## D-008 — Phase 2.3: deals_status_check tightened to 9-state commercial set

  Timestamp: 2026-04-29
  Phase: §2.3
  Subject: rust/migrations/20260429_carrier_04_deals_status_in_clearance.sql
  Decision: deals_status_check rewritten to 9 states (PROSPECT, QUALIFYING, NEGOTIATING, IN_CLEARANCE, CONTRACTED, LOST, REJECTED, WITHDRAWN, CANCELLED). Pre-rewrite UPDATE folds BAC_APPROVAL/KYC_CLEARANCE rows (none live) into IN_CLEARANCE.
  Rationale: TODO §2.3, post-Phase-5.1 substate model. Order within migration is critical: drop CHECK → backfill → install new CHECK.
  Reversibility: medium — old 15-state CHECK can be reinstated via further migration.
  Reference: rust/migrations/20260429_carrier_04_deals_status_in_clearance.sql

## D-009 — Phase 2.4: cbus.operational_status re-asserted (live name preserved)

  Timestamp: 2026-04-29
  Phase: §2.4
  Subject: rust/migrations/20260429_carrier_05_cbus_operational_status_reassert.sql
  Decision: Idempotent re-assertion. Constraint name preserved as live `chk_cbu_operational_status` (TODO proposed `cbus_operational_status_check`); migration drops both names if present and re-installs `chk_cbu_operational_status`.
  Rationale: B2 — column + CHECK already live; ship migration for traceability and to lock-in carrier on fresh databases.
  Reversibility: high — no-op against live; reversible against fresh DB.
  Reference: rust/migrations/20260429_carrier_05_cbus_operational_status_reassert.sql

## D-010 — Phase 2.6: deal_slas.sla_status carrier added

  Timestamp: 2026-04-29
  Phase: §2.6
  Subject: rust/migrations/20260429_carrier_06_deal_slas_status.sql
  Decision: Added deal_slas.sla_status column (6-state CHECK, nullable).
  Rationale: TODO §2.6 — M-052 carrier. Live row count 0; no backfill needed.
  Reversibility: high — column drop reverses cleanly.
  Reference: rust/migrations/20260429_carrier_06_deal_slas_status.sql

## D-011 — Phase 2.7: cbu_settlement_chains.lifecycle_status carrier added

  Timestamp: 2026-04-29
  Phase: §2.7
  Subject: rust/migrations/20260429_carrier_07_settlement_chain_lifecycle_status.sql
  Decision: Added lifecycle_status column (7-state CHECK). Backfilled 1 row from is_active=true → 'live'. Legacy is_active boolean retained for backward compat.
  Rationale: TODO §2.7 — M-021 carrier; migrate boolean to proper state column.
  Reversibility: high — column drop reverses; is_active preserved.
  Reference: rust/migrations/20260429_carrier_07_settlement_chain_lifecycle_status.sql

## D-012 — Phase 5.2: deals.bac_status + deals.kyc_clearance_status substate columns

  Timestamp: 2026-04-29
  Phase: §5.2
  Subject: rust/migrations/20260429_carrier_08_deals_in_clearance_substates.sql
  Decision: Added two parallel substate columns (4-state CHECK each, nullable). Defensive seed for any deal already in IN_CLEARANCE (live count 0).
  Rationale: TODO §5.2 — encodes IN_CLEARANCE compound state in schema. Both nullable: only deals at IN_CLEARANCE/CONTRACTED carry substate semantics.
  Reversibility: medium — column drop reverses; loses substate history.
  Reference: rust/migrations/20260429_carrier_08_deals_in_clearance_substates.sql

## D-013 — Phase 5.3: BAC verb descriptions updated; deal.update-kyc-clearance verb added

  Timestamp: 2026-04-29
  Phase: §5.3
  Subject: rust/config/verbs/deal.yaml
  Decision: Updated descriptions of submit-for-bac, bac-approve, bac-reject to reflect substate semantics (writes to bac_status, not deal_status, except submit-for-bac which also enters IN_CLEARANCE). Added new `update-kyc-clearance` preserving verb (state_effect=preserving, no transition_args). Verb count: 1282 → 1283.
  Rationale: TODO §5.3 — surface the substate semantics in verb declarations for catalogue discoverability. transition_args target deal slot (correct — handler-level column dispatch is runtime concern).
  Reversibility: high — verb YAML edits.
  Reference: rust/config/verbs/deal.yaml

## D-016 — Phase 4 architectural reframe: state-node green-switch model (v1.4 principle P17)

  Timestamp: 2026-04-29
  Phase: §4 / §7 (v1.4 spec foundation)
  Subject: rust/crates/dsl-core/src/config/dag.rs::StateDef + 12 DAG taxonomies
  Decision: Adam established the canonical state-machine model: each state node has a green-switch predicate (its entry/transit test); verbs run while source is green and make the state changes that satisfy the destination's green-switch; the destination's green_when IS the postcondition of the transition. Schema change: added `green_when: Option<String>` to `StateDef`. Applied as worked examples to 9 tollgate cases across 5 DAGs (kyc_case.APPROVED, kyc_decision.CLEARED, ubo_registry.{IDENTIFIED,PROVABLE,PROVED,APPROVED}, booking_principal_clearance.APPROVED, cbu_corporate_action.approved, manco.APPROVED, trading_profile.APPROVED, delivery.DELIVERED, changeset.approved, deal.CONTRACTED).
  Rationale: Adam's framing — "all dag patterns are state transformations where state nodes get proofs/updates; the node state is a switch (go/no-go); the move to next state is a test." Replaces verb-centric transitions with state-centric green-criteria. Aligns with v1.3 P17 cross-workspace state composition; extends it to intra-DAG state semantics.
  Reversibility: high — green_when is Option<String>, additive; states without it remain permissive.
  Reference: rust/crates/dsl-core/src/config/dag.rs:268-279; rust/config/sem_os_seeds/dag_taxonomies/{kyc,booking_principal,cbu,instrument_matrix,semos_maintenance,deal}_dag.yaml; full sweep across 290+ verb-driven transitions deferred for follow-up engagement (out of scope for this refactor).

## D-017 — Phase 4 §4.3 KYC verb-set drift partially reconciled

  Timestamp: 2026-04-29
  Phase: §4.3
  Subject: rust/config/verbs/kyc/{kyc-case,red-flag}.yaml + rust/config/sem_os_seeds/dag_taxonomies/kyc_dag.yaml
  Decision: Added 5 new YAML verbs (kyc-case.{approve, reject, approve-with-conditions}, red-flag.{escalate, update-rating}). Amended DAG vias: case.* → kyc-case.* (3 amendments); evidence.{verify,reject,waive,link} → evidence.{mark-verified,mark-rejected,mark-waived,attach-document} (preserving YAML's mark-* prefix per Adam's preference for syntactic correctness — "mark-verified" expresses the marking action, not the verification act); red-flag.resolve → red-flag.close (preserving YAML); ubo-registry naming (promote/advance/reject; approve transitions become tollgate-driven via destination green_when). Verb count 1283 → 1288.
  Rationale: TODO §4.3 reconciliation; Adam adjudicated naming conventions (mark- prefix retained, close vs resolve preserves YAML).
  Reversibility: medium — verb additions reversible; DAG via renames trivially reversible.
  Reference: rust/config/verbs/kyc/kyc-case.yaml, rust/config/verbs/kyc/red-flag.yaml, rust/config/sem_os_seeds/dag_taxonomies/kyc_dag.yaml

## D-018 — DAG green_when Predicate AST and parser

  Timestamp: 2026-04-30
  Phase: SemOS DAG Architecture Phase 1
  Subject: rust/crates/dsl-core/src/config/predicate/{ast,parser}.rs
  Decision: Added a typed predicate module for DAG `green_when` expressions in `dsl-core`, preserving capability separation from runtime SQL evaluation. The AST covers conjunction, existence, state membership, attribute comparisons, universal/negative/existential quantifiers, count predicates, and obtained-validity predicates. The parser accepts the current v1.4 free-text convention and all 13 confirmed worked-example predicates. `Predicate::Obtained` and `Predicate::Count` are forward-compatible variants: they are normative in the Vision / Phase 1 sketch but are not emitted by the parser until authored predicate fixtures require those syntaxes.
  Rationale: Free-text `green_when` predicates must become machine-readable before Frontier can evaluate them. Keeping AST/parser in `dsl-core` preserves a pure configuration layer; SQL compilation and runtime evaluation remain a later Frontier/runtime concern.
  Reversibility: medium — additive module and tests; downstream phases will depend on the AST shape once evaluator work begins.
  Reference: rust/crates/dsl-core/src/config/predicate/ast.rs; rust/crates/dsl-core/src/config/predicate/parser.rs; rust/crates/dsl-core/tests/predicate_ast.rs

## D-019 — PredicateBinding schema, DAG-authoritative source model

  Timestamp: 2026-04-30
  Phase: SemOS DAG Architecture Phase 1 support / Phase 2-3 bridge
  Subject: rust/crates/dsl-core/src/config/dag.rs::PredicateBinding + DAG taxonomy `predicate_bindings:`
  Decision: Added `StateMachine.predicate_bindings` as additive metadata for every entity kind referenced by a `green_when` predicate. Bindings carry `source_kind` (`substrate`, `dag_entity`, `dsl_fact`), optional carrier details, join keys, value/state columns, and required-universe declarations. Populated bindings for the 13 worked-example predicates with `source_kind: dag_entity` unless the DAG already provides carrier semantics. The DAG plus DSL catalogue are the SemOS source of truth; database tables are carriers that may lag or be resolved later.
  Rationale: Predicate evaluation cannot be diagnosable without a binding layer for names like `mandatory_disclosure`, `steward_approval`, or `evidence_requirement`. Making the binding explicit in the DAG prevents evaluator code from guessing table names or collapsing required-set semantics into weak existence checks.
  Reversibility: high — additive YAML/schema metadata; consumers can ignore until Frontier evaluator support lands.
  Reference: rust/crates/dsl-core/src/config/dag.rs; rust/config/sem_os_seeds/dag_taxonomies/{booking_principal,cbu,deal,instrument_matrix,kyc,semos_maintenance}_dag.yaml; rust/crates/dsl-core/tests/predicate_ast.rs

## D-020 — Boot-time green_when validation promoted to DAG validator

  Timestamp: 2026-04-30
  Phase: SemOS DAG Architecture Phase 1.5
  Subject: rust/crates/dsl-core/src/config/dag_validator.rs
  Decision: Promoted `green_when` checks from test-only coverage into `validate_dags`. The validator now parses every non-empty `StateDef.green_when` and emits `DagError::GreenWhenParseError` for malformed predicates. It also walks the parsed AST and emits `DagError::GreenWhenUnboundEntity` when a predicate references an entity kind without a corresponding `state_machine.predicate_bindings` entry.
  Rationale: DAG YAML is boot-time architectural input. A malformed predicate or unbound predicate entity should fail the same pre-DB startup gate as other DAG structural errors, not wait for integration tests or runtime Frontier hydration.
  Reversibility: high — validator-only enforcement; removing the checks returns to test-time-only enforcement.
  Reference: rust/crates/dsl-core/src/config/dag_validator.rs; rust/crates/dsl-core/tests/predicate_ast.rs

## D-015 — Phase 4 drift survey: §4.2 already resolved, §4.3 has real drift

  Timestamp: 2026-04-29
  Phase: §4
  Subject: docs/governance/phase-4-drift-survey-2026-04-29.md
  Decision: Surveyed all 9 §4.2 clusters and 4 §4.3 KYC families. §4.2 (workspace-ownership drift) is already fully resolved in live YAML — substrate audit was based on a stale snapshot. §4.3 (KYC verb-set drift) has real drift in all 4 families (~12 verb adds/renames + ~3 DAG via amendments). Two open business-decision points need Adam's call: (a) evidence family `mark-*` prefix retention; (b) ubo-registry approve/promote/advance mapping.
  Rationale: Per cluster-by-cluster checkpoint discipline (user agreed earlier this session), surveying first avoids 50+ unnecessary edits driven by stale audit claims.
  Reversibility: high — survey is descriptive.
  Reference: docs/governance/phase-4-drift-survey-2026-04-29.md

## D-014 — Carrier batch validation passed

  Timestamp: 2026-04-29
  Phase: §2 + §5
  Subject: cargo x reconcile validate
  Decision: All 8 carrier migrations applied successfully against live DB. Validator reports 0 structural errors, 0 well-formedness errors, 0 cross-DAG errors after DAG amendments.
  Rationale: Verification gate satisfied per TODO §2.8.
  Reversibility: n/a — validation result.
  Reference: docs/governance/refactor-inventory.md (verification queries embedded)

---
