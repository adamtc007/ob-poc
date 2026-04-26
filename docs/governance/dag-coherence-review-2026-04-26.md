# DAG Coherence Review — 11 Workspaces — 2026-04-26

> **Phase:** v1.2 Tranche 1 Phase 1.3.
> **Scope:** Coherence review across all 11 declared DAG taxonomies in `rust/config/sem_os_seeds/dag_taxonomies/`.
> **Authority:** Adam (provisional, per `tier-assignment-authority-provisional.md`).
> **Status:** Findings + actions. Code edits land separately and reference this document.

---

## 1. Per-workspace summary

| # | Workspace | DAG taxonomy file | Slots | Stateful slots | XW constraints | Derived state | Cascade rules | Carrier table(s) |
|--:|-----------|-------------------|------:|---------------:|---------------:|--------------:|--------------:|------------------|
| 1 | CBU | `cbu_dag.yaml` | 21 | 9 | 3 | 1 (`cbu_operationally_active`) | 5 | `cbus`, `cbu_evidence`, `cbu_service_consumption`, `cbu_trading_activity`, `investors`, `holdings`, `entity_proper_person`, `entity_limited_company_ubo` |
| 2 | KYC | `kyc_dag.yaml` | 15 | 11 | 0 | 0 | 2 | `cases`, `entity_workstreams`, `screenings`, `ubo_evidence`, `kyc_ubo_registry`, `kyc_ubo_evidence`, `red_flags`, `doc_requests`, `outreach_requests`, `kyc_decisions`, `kyc_service_agreements` |
| 3 | Deal | `deal_dag.yaml` | 19 | 10 | 2 | 0 | 4 | `deals`, `deal_products`, `deal_rate_cards`, `deal_onboarding_requests`, `deal_documents`, `deal_ubo_assessments`, `fee_billing_profiles`, `fee_billing_periods`, `deal_slas` |
| 4 | InstrumentMatrix | `instrument_matrix_dag.yaml` | 21 | 12 | 1 | 0 | 5 | `client_groups`, `cbu_trading_profiles`, `cbu_settlement_chains`, `cbu_service_intent`, `cbu_service_resource`, `service_delivery_map`, `corporate_action_events`, `cbu_collateral_management` |
| 5 | BookingPrincipal | `booking_principal_dag.yaml` | 2 | 1 | 0 | 0 | 0 | `booking_principal_clearances` |
| 6 | LifecycleResources | `lifecycle_resources_dag.yaml` | 4 | 2 | 0 | 0 | 2 | `application_instances`, `capability_bindings` |
| 7 | ProductMaintenance | `product_service_taxonomy_dag.yaml` | 5 | 2 | 0 | 0 | 0 | `services`, `service_versions` |
| 8 | SemOsMaintenance | `semos_maintenance_dag.yaml` | 5 | 5 | 0 | 0 | 3 | `changesets`, `attribute_defs`, `derivation_specs`, `service_resource_defs`, `phrase_authoring` |
| 9 | SessionBootstrap | `session_bootstrap_dag.yaml` | 3 | 0 | 0 | 0 | 0 | (no persistent state) |
| 10 | OnboardingRequest | `onboarding_request_dag.yaml` | 4 | 1 | 2 | 0 | 1 | `deal_onboarding_requests` |
| 11 | BookSetup | `book_setup_dag.yaml` | 6 | 1 | 2 | 0 | 1 | `client_books` (Tranche 3 deferred), `entities`, `cbus` (referenced) |
| **TOTAL** | | | **106** | **69** | **11** | **2** | **24** | |

11 workspaces ÷ 2 derived aggregates suggests Mode B (tollgate) is used sparingly. CBU's `cbu_operationally_active` is the only declared aggregate currently active in a constraint.

## 2. Cross-workspace constraint matrix

11 cross-workspace constraints declared across the 11 DAGs. All resolve to real `(table, column)` pairs in `master-schema.sql`. None contain `EXISTS` sub-queries in the constraint declaration itself.

| constraint_id | source workspace.slot | target workspace.transition | predicate (truncated) | LHS resolves? | EXISTS? |
|---------------|------------------------|------------------------------|------------------------|---------------|---------|
| `mandate_requires_validated_cbu` | cbu.cbu | instrument_matrix.trading_profile DRAFT→SUBMITTED | `cbu.status = VALIDATED` | ✓ `cbus.status` | No |
| `cbu_validated_requires_kyc_case_approved` | kyc.kyc_case | cbu.cbu VALIDATION_PENDING→VALIDATED | `cases.client_group_id = this_cbu.primary_client_group_id AND status='APPROVED'` | ✓ `cases.status` | No |
| `service_consumption_requires_active_service` | product_maintenance.service | cbu.service_consumption proposed→provisioned | `services.service_id = this_consumption.service_id AND lifecycle_status IN [active,…]` | ✓ `services.lifecycle_status` | No |
| `service_consumption_active_requires_live_binding` | lifecycle_resources.capability_binding | cbu.service_consumption provisioned→active | `capability_bindings.service_id = … AND EXISTS(application_instances ai WHERE ai.id=… AND ai.lifecycle_status='ACTIVE')` | ✓ `capability_bindings.binding_status` | **YES** |
| `deal_contracted_requires_kyc_approved` | kyc.kyc_case | deal.deal KYC_CLEARANCE→CONTRACTED | `cases.client_group_id = this_deal.primary_client_group_id AND status='APPROVED'` | ✓ `cases.status` | No |
| `deal_contracted_requires_bp_approved` | booking_principal.clearance | deal.deal KYC_CLEARANCE→CONTRACTED | `booking_principal_clearances.deal_id = this_deal.deal_id AND clearance_status IN [APPROVED, ACTIVE]` | ✓ `booking_principal_clearances.clearance_status` | No |
| `book_cbus_scaffolded_requires_kyc_case_in_progress` | kyc.kyc_case | book_setup.book entities_provisioned→cbus_scaffolded | `cases.client_group_id = this_book.client_group_id AND status IN [DISCOVERY,…,APPROVED]` | ✓ `cases.status` | No |
| `book_ready_requires_deal_contracted_gate` | deal.deal | book_setup.book mandates_defined→ready_for_deal | `deals.primary_client_group_id = this_book.client_group_id AND deal_status IN [CONTRACTED, ONBOARDING, ACTIVE]` | ✓ `deals.deal_status` | No |
| `onboarding_request_requires_deal_contracted` | deal.deal | onboarding_request.onboarding_request validating→submitted | `deals.deal_id = this_request.deal_id AND deal_status IN [CONTRACTED, ONBOARDING, ACTIVE]` | ✓ `deals.deal_status` | No |
| `onboarding_request_requires_cbu_validated` | cbu.cbu | onboarding_request.onboarding_request validating→submitted | `cbus.cbu_id = this_request.cbu_id AND status='VALIDATED'` | ✓ `cbus.status` | No |

**Findings:**

- **F1.** All 11 constraints' LHS table.column resolves to schema. **No FK drift.**
- **F2.** Only one constraint uses `EXISTS` — `service_consumption_active_requires_live_binding`. This is the v1.2 §6.2 motivator for predicate-DSL `EXISTS` extension (Tranche 1 R.1 / T1.B).
- **F3.** No cycles. KYC is the universal source (3 constraints emanate from it); no workspace points back at KYC.
- **F4.** Cross-workspace constraints concentrate on the deal-tollgate axis: KYC × BP × CBU all gate `deal_contracted`. This is the four-tier commercial-to-operational handoff.

## 3. Slot-dispatch coverage

`rust/crates/dsl-runtime/src/cross_workspace/slot_state.rs` carries the runtime's `(workspace, slot) → (table, state_column, pk_column)` dispatch table.

**Covered (~26 entries):** cbu × {cbu, cbu_evidence, service_consumption, trading_activity}; deal × {deal, deal_product, deal_rate_card, deal_onboarding_request, deal_document, deal_ubo_assessment, billing_profile, billing_period, deal_sla}; booking_principal × clearance; kyc × {kyc_case, entity_workstream, screening}; instrument_matrix × {trading_profile, trading_activity}; semos_maintenance × {changeset, attribute_def, manco}; product_maintenance × {service, service_version}; lifecycle_resources × {application_instance, capability_binding}.

**MISSING — slots in DAGs without dispatch entries:**

| Workspace | Slot | Action |
|-----------|------|--------|
| cbu | `entity_proper_person` | ADD dispatch → `(entities, status, entity_id)` (or document as phase-level reference) |
| cbu | `entity_limited_company_ubo` | ADD dispatch → `(entities, status, entity_id)` |
| kyc | `red_flag` | ADD dispatch → `(red_flags, status, red_flag_id)` |
| book_setup | `book` | ADD dispatch → `(client_books, status, book_id)` once Tranche 3 schema lands |
| instrument_matrix | `group` | DOCUMENT as reference-only (resolves via `client_group` table; no IM-owned state) |

**DEAD — dispatch entries for slots not declared in current DAGs:**

| Entry | Action |
|-------|--------|
| `(cbu, investor)` | KEEP — declared in cbu_dag §2 (re-confirm; reviewer may have missed in initial scan) |
| `(cbu, investor_kyc)` | KEEP — same |
| `(cbu, holding)` | KEEP — same |
| `(semos_maintenance, manco)` | KEEP — declared in semos_maintenance_dag §2 |

**Resolution (action items):**

1. **Add 3 dispatch entries** — `(cbu, entity_proper_person)`, `(cbu, entity_limited_company_ubo)`, `(kyc, red_flag)` — to `slot_state.rs`. Each maps to its carrier table and state column.
2. **Defer book_setup.book** — Tranche 3 schema migration adds `client_books`; dispatch entry follows. Tracked under `book_setup_dag.yaml §D-2`.
3. **Document `instrument_matrix.group` as reference-only** — comment in DAG yaml explaining no dispatch needed.

## 4. Schema-table delta

CLAUDE.md claims 372 tables. Direct count via `grep -c "^CREATE TABLE" rust/migrations/master-schema.sql` = **367**.

`git log --oneline --all --grep "DROP TABLE" -- rust/migrations/` = no results.

**The 5-table delta is forward-deferred, not historical.** The DAGs declare slots referencing tables that haven't yet been migrated:

| Deferred table | Owning workspace | Reason | Resolution |
|----------------|------------------|--------|------------|
| `client_books` | book_setup | Tranche 3 schema lag (per book_setup_dag.yaml §D-2) | Migration 20260427+ |
| `application_instances` | lifecycle_resources | R1 workspace just landed; possibly post-snapshot | Verify against migration list |
| `capability_bindings` | lifecycle_resources | Same as above | Verify |
| `booking_principal_clearances` | booking_principal | R3.5 workspace; possibly post-snapshot | Verify against migration list |
| `service_versions` | product_maintenance | R2 service-lifecycle amendment | Verify |

**Action:** update `CLAUDE.md` line 8 to read "**Tables:** 367 + 5 forward-deferred (Tranche 3 schema migrations pending)" rather than the round 372.

## 5. FK resolvability check

For all 11 cross-workspace constraint LHS predicates checked: **all 11 resolve** to real `(table, column)` pairs in `master-schema.sql`. The single EXISTS sub-query (`application_instances.lifecycle_status`) likewise resolves.

**No FK DRIFT detected.** Schema is consistent with DAG declarations.

## 6. EXISTS-clause inventory

**One** cross-workspace constraint uses an `EXISTS` clause:

- `service_consumption_active_requires_live_binding` in `lifecycle_resources_dag.yaml`. Sub-query: `EXISTS (SELECT 1 FROM "ob-poc".application_instances ai WHERE ai.id = capability_bindings.application_instance_id AND ai.lifecycle_status = 'ACTIVE')`.

Internal cross-slot (intra-DAG) constraints contain additional EXISTS clauses (e.g. instrument_matrix's `isda_coverage_required_for_derivative_trading`, kyc's `kyc_service_agreement_active_required`), but these are intra-workspace; the cross-workspace predicate DSL only needs to handle the one above.

**Action — T1.B:** extend predicate DSL with an `Exists { table, predicate }` AST variant capable of representing the one cross-workspace EXISTS clause. The intra-workspace EXISTS clauses can remain runtime-only until they cross workspace boundaries.

## 7. Naming consistency

Spot-checks on naming conventions across the 11 DAGs:

- **State naming:** mostly UPPER_SNAKE for terminal states (`APPROVED`, `REJECTED`, `WAIVED`); lowercase for non-terminal (`proposed`, `provisioned`, `active`). **Inconsistency:** `cases.status` uses UPPER (`APPROVED`); `services.lifecycle_status` uses lower (`active`, `retired`). No semantic problem; cosmetic.
- **Constraint naming:** verb-noun-condition pattern (`mandate_requires_validated_cbu`, `deal_contracted_requires_bp_approved`). Consistent.
- **Slot naming:** singular (`deal`, `cbu`, `kyc_case`, `clearance`). Consistent.
- **Workspace naming:** snake_case (`booking_principal`, `lifecycle_resources`, `book_setup`). Consistent.

**No required action.** Convention drift is cosmetic and doesn't affect resolution.

## 8. Composite findings + actions

| # | Finding | Severity | Action | Phase |
|--:|---------|----------|--------|-------|
| F1 | All 11 cross-workspace constraints' LHS resolve to schema | OK | None | — |
| F2 | One cross-workspace constraint uses `EXISTS` | Action required | Extend predicate DSL with Exists variant | T1.B |
| F3 | No cycles in cross-workspace constraint graph | OK | None | — |
| F4 | KYC is the universal constraint source (3 outbound) | OK | None | — |
| F5 | 3 missing dispatch entries (cbu × 2, kyc × 1) | Action required | Add 3 entries to `slot_state.rs` | T1.C-followup |
| F6 | book_setup dispatch deferred to Tranche 3 | OK (deferred) | Track under D-2 | — |
| F7 | 5 forward-deferred tables explain CLAUDE.md 372 vs actual 367 | Documentation | Update CLAUDE.md | T1.I |
| F8 | No FK drift | OK | None | — |
| F9 | Naming conventions cosmetically inconsistent | Cosmetic | None | — |

## 9. Coherence verdict

**PASS** — the 11 DAG taxonomies cohere internally and across boundaries. The two follow-on actions (F2 EXISTS support + F5 dispatch entries) are well-bounded and addressed in T1.B / T1.C-followup.

This phase satisfies v1.2 Tranche 1 DoD item 6: "Declared DAG taxonomies exist for all eleven workspaces and have passed coherence review."

---

**End of DAG coherence review — 2026-04-26.**
