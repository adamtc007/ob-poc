# Architecture Review: KYC/UBO Workspace & Constellation System

**Review Date:** 2026-03-30
**Scope:** KYC workspace, UBO/ownership domain, constellation topology, verb vocabulary, data model, state machines
**For:** Peer review — architecture, completeness, consistency

---

## 1. Executive Summary

The KYC/UBO system implements a custody bank's Know Your Customer and Ultimate Beneficial Owner workflows through a constellation-driven architecture. Client groups are the apex entity; ownership chains, control mapping, KYC case management, screening, evidence collection, and tollgate approval all hang off this root.

**Key architectural decisions:**
- **Constellation-driven state**: Entity state is modelled as hydrated slot trees, not flat tables. Each slot has a state machine, dependency chain, and verb palette.
- **Verb-first interaction**: All mutations go through named verbs with typed args. No raw SQL, no ad-hoc REST. Verbs are the API.
- **Two-phase KYC**: Group-level clearance (UBO/ownership/control) precedes per-CBU delta cases. A CBU cannot open a KYC case until the group perimeter is cleared.
- **Macro composition**: Complex workflows (full KYC review, UBO pipeline) are modelled as macros that expand to verb sequences, not monolithic operations.

**Statistics:**
- 4 constellation maps (kyc_onboarding, kyc_extended, group_ownership, governance_compliance)
- 3 constellation families (client_group_ownership, kyc_lifecycle, kyc_extended)
- 9 state machines governing entity lifecycles
- ~131 verbs across 14 domains
- 22 database tables in the KYC/UBO schema
- 15 macros (8 UBO, 4 screening, 3 KYC workflow)
- 1 journey pack (kyc-case) with 2 templates

---

## 2. Workspace Definition

```
WorkspaceKind::Kyc
  constellation_families: [ownership, clearance, delta_review, screening]
  subject_kinds: [ClientGroup, Case, Cbu]
  default_constellation_family: "ownership"
  default_constellation_map: "group.ownership"
  supports_handoff_mode: true
```

The KYC workspace defaults to the **group ownership** constellation — not the case management view. This reflects the two-phase architecture: you start with the group perimeter, then drill into per-CBU cases.

---

## 3. Constellation Topology

### 3.1 group.ownership (Priority 50 — Root)

The foundational constellation. All other KYC constellations depend on this completing first.

```
client_group [root, state_machine: client_group_lifecycle]
  │
  ├── gleif_import [entity, entity_kinds: company]
  │     16 GLEIF verbs (import, search, refresh, enrich, trace...)
  │
  ├── ubo_discovery [entity_graph, recursive max_depth: 5]
  │     state_machine: ubo_epistemic_lifecycle
  │     31 UBO verbs + 6 registry verbs + 2 snapshot verbs
  │
  ├── control_chain [entity_graph, recursive max_depth: 10]
  │     30 ownership/control verbs
  │
  ├── group_kyc_clearance [case, depends_on: control_chain]
  │     state_machine: kyc_case_lifecycle
  │     5 verbs (read, summarize, update-status, close)
  │
  └── cbu_identification [cbu, depends_on: control_chain + group_kyc_clearance(approved)]
        37 CBU verbs (full lifecycle: create, roles, products, evidence, validation)
```

**Key invariant:** `cbu_identification` depends on both `control_chain` completion AND `group_kyc_clearance` reaching `approved` state. No CBU can be identified until the group perimeter is cleared.

**Verb count:** ~119 verbs across 6 slots (largest constellation in the system)

### 3.2 kyc.onboarding (Priority 40 — Per-CBU Delta)

Fires after group clearance. Manages per-CBU case lifecycle.

```
cbu [root]
  │
  ├── client_group [optional, state_machine: client_group_lifecycle]
  │
  ├── group_kyc_clearance [case, depends_on: client_group(group_kyc_cleared)]
  │
  ├── kyc_case [mandatory, depends_on: cbu + group_kyc_clearance(approved)]
  │     state_machine: kyc_case_lifecycle
  │     11 verbs (create, read, summarize, assign, update, close, reopen, escalate...)
  │     └── tollgate [child, depends_on: kyc_case(review)]
  │           state_machine: (tollgate gates)
  │           11 verbs (evaluate, check-gate, metrics, thresholds, overrides...)
  │
  ├── entity_workstream [entity, entity_kinds: person/company]
  │     state_machine: entity_workstream_lifecycle
  │     37 verbs spanning 6 sub-domains:
  │       - entity-workstream.* (8): create, read, list, state, update, escalate-dd, complete, block
  │       - red-flag.* (8): raise, read, list, resolve, escalate, update, close, list-by-severity
  │       - requirement.* (7): create, create-set, check, list, list-for-entity, list-outstanding, waive, reinstate
  │       - document.* (9): solicit, solicit-batch, upload, verify, reject, read, list, compute-requirements, missing-for-entity
  │       - (all gated by entity_workstream state machine)
  │
  ├── screening [entity, depends_on: entity_workstream(open)]
  │     state_machine: screening_lifecycle
  │     13 verbs (run, sanctions, pep, adverse-media, bulk-refresh, review-hit, escalate, resolve, complete...)
  │
  ├── kyc_agreement [entity, entity_kinds: company]
  │     state_machine: agreement_lifecycle
  │     6 verbs (create, read, list, update, update-status, sign)
  │
  ├── identifier [entity, depends_on: entity_workstream(open)]
  │     state_machine: identifier_lifecycle
  │     11 verbs (add, read, list, verify, expire, update, search, resolve, list-by-type, set-primary, remove)
  │
  └── request [entity, depends_on: kyc_case]
        state_machine: document_request_lifecycle
        9 verbs (create, read, list, update, complete, cancel, assign, reopen, escalate)
```

**Design decision:** Red flags, requirements, and documents are **not separate slots** — they are verb palettes on the `entity_workstream` slot. This is because they are state annotations on the entity, not independently positioned entities. A red flag doesn't have its own state machine; it's a property of the workstream's state.

**Verb count:** ~96 verbs across 9 slots

### 3.3 kyc.extended (Priority 35 — Deep Investigation)

Extended topology for complex compliance analysis.

```
entity [root]
  │
  ├── board [entity, entity_kinds: person]
  │     9 verbs (appoint, resign, list, grant/revoke/list rights, analyze-control)
  │
  └── bods [entity, entity_kinds: person/company]
        9 verbs (discover-ubos, import, link, get-statement, list-by-entity, find-by-lei, list-persons, list-ownership, sync)
```

**Verb count:** 18 verbs across 2 slots

### 3.4 governance.compliance (Priority — Governance)

Cross-cutting governance concerns.

```
group [root]
  │
  ├── sla [22 verbs — SLA lifecycle, commitments, measurements, breaches]
  ├── access_review [20 verbs — campaigns, attestation, escalation]
  ├── regulatory [10 verbs — registrations, filings, verification]
  ├── ruleset [4 verbs — create, read, publish, retire]
  ├── delegation [8 verbs — delegation chains, revocation]
  ├── team [18 verbs — members, roles, capacity, governance/ops]
  ├── rule [3 verbs — create, read, update]
  └── rule_field [2 verbs — list, read]
```

**Verb count:** ~87 verbs across 8 slots

---

## 4. State Machines

### 4.1 client_group_lifecycle (10 states)

```
prospect → researching → ubo_mapped → control_mapped →
  group_kyc_cleared → cbus_identified → onboarding → active
```

**Key transitions:**
| From | To | Trigger verbs |
|------|----|---------------|
| prospect | researching | client-group.start-discovery, gleif.import-tree |
| researching | ubo_mapped | ubo.discover, ubo.compute-chains |
| ubo_mapped | control_mapped | control.build-graph, ownership.trace-chain |
| control_mapped | group_kyc_cleared | kyc-case.close (with approved status) |
| group_kyc_cleared | cbus_identified | cbu.create, cbu.ensure |
| cbus_identified | onboarding | cbu.assign-role (all mandatory roles filled) |

**Observations:**
- This is the master sequencer for the entire onboarding flow
- Each state acts as a gate — you can't skip steps
- The progression is deterministic (no branching except blocked states)

### 4.2 kyc_case_lifecycle (10 states)

```
intake → discovery → assessment → review → approved
                                        → rejected
                                        → expired
                                        → refer_to_regulator
                                        → do_not_onboard
  (any state) → blocked → (previous state)
```

**Key transitions:**
| From | To | Trigger verbs |
|------|----|---------------|
| intake | discovery | kyc-case.update-status |
| discovery | assessment | kyc-case.update-status |
| discovery | blocked | kyc-case.update-status |
| assessment | review | kyc-case.update-status |
| review | approved | kyc-case.close |
| review | rejected | kyc-case.close |
| approved | review | kyc-case.reopen |

### 4.3 entity_workstream_lifecycle (9 states)

```
open → documents_requested → screening_initiated → screening_complete →
  evidence_collected → verified → closed
  (multiple states) → blocked
  (multiple states) → enhanced_dd
```

**Key transitions:**
| From | To | Trigger verbs |
|------|----|---------------|
| open | documents_requested | document.solicit, document.solicit-batch |
| documents_requested | screening_initiated | screening.run |
| screening_initiated | screening_complete | screening.complete |
| screening_complete | evidence_collected | requirement.check (all satisfied) |
| evidence_collected | verified | entity-workstream.complete |
| any | enhanced_dd | entity-workstream.escalate-dd |
| any | blocked | entity-workstream.block |

### 4.4 ubo_epistemic_lifecycle (5 states)

```
undiscovered → alleged → provable → proved → approved
```

**Epistemic model:** Each state represents an increasing confidence level in the UBO determination. Evidence requirements escalate with each transition.

| State | Meaning | Required evidence |
|-------|---------|-------------------|
| undiscovered | No UBO information | None |
| alleged | Client-alleged ownership | Allegation record |
| provable | Documentary evidence exists | Ownership register / trust deed |
| proved | Evidence verified | Verified identity + screened |
| approved | Governance sign-off | Tollgate passed |

### 4.5 screening_lifecycle (12 states)

```
not_started → sanctions_pending → sanctions_clear →
  pep_pending → pep_clear → media_pending → media_clear → all_clear

  (any _pending) → *_hit → escalated → resolved

  all_clear → not_started (rescreening)
```

**Design:** The screening lifecycle is a sequential pipeline — sanctions first, then PEP, then adverse media. Each type must clear before the next starts. Hits branch into an escalation path.

### 4.6 Other State Machines

| Machine | States | Purpose |
|---------|--------|---------|
| agreement_lifecycle | 6 (pending→draft→sent→signed→active→terminated) | KYC service agreements |
| identifier_lifecycle | 3 (empty→captured→verified) + failed | External identifiers |
| document_request_lifecycle | 6 (pending→requested→received→verified/rejected/waived) | Document requests |
| document_lifecycle | 7 (missing→requested→received→in_qa→verified/rejected/waived) | Document requirements |

---

## 5. Verb Vocabulary Analysis

### 5.1 Domain Distribution

| Domain | Verb Count | Behavior Mix | Constellation |
|--------|-----------|--------------|---------------|
| client-group | 22 | crud + plugin | group.ownership |
| gleif | 16 | plugin | group.ownership |
| ubo | 23 | crud + plugin + macro | group.ownership |
| ubo.registry | 6 | crud + plugin | group.ownership |
| ubo.snapshot | 2 | plugin | group.ownership |
| ownership | 14 | plugin | group.ownership |
| control | 14 | plugin | group.ownership |
| kyc-case | 10 | crud + plugin | kyc.onboarding |
| entity-workstream | 9 | crud + plugin | kyc.onboarding |
| screening | 13 | crud + plugin | kyc.onboarding |
| tollgate | 11 | crud + plugin | kyc.onboarding |
| red-flag | 8 | crud | kyc.onboarding (on entity_workstream) |
| document | 20 | durable + crud + plugin | kyc.onboarding (on entity_workstream) |
| requirement | 8 | plugin | kyc.onboarding (on entity_workstream) |
| identifier | 11 | crud | kyc.onboarding |
| kyc-agreement | 6 | crud | kyc.onboarding |
| request | 9 | plugin + crud | kyc.onboarding |
| evidence | 5 | plugin | kyc.onboarding |
| allegation | 6 | crud | (observation domain) |
| trust | 8 | crud + plugin | kyc.extended |
| partnership | 7 | crud + plugin | kyc.extended |
| board | 9 | crud + plugin | kyc.extended |
| bods | 9 | plugin | kyc.extended |
| **Total** | **~246** | | |

### 5.2 Behavior Type Distribution

| Behavior | Count | Characteristics |
|----------|-------|----------------|
| **crud** | ~120 | Deterministic SQL — table/column/operation defined in YAML |
| **plugin** | ~100 | Custom Rust handler (`#[register_custom_op]`) — complex validation, cross-table logic |
| **durable** | 2 | BPMN-Lite workflows (document.solicit, document.solicit-batch) — 14-day timeout, task bindings |
| **macro** | 15 | Verb sequences — expand to multiple crud/plugin calls |
| **template** | 2 | Pack templates (new-kyc-case, renewal-kyc-case) |

### 5.3 Macro Vocabulary

**UBO Macros (8):** High-level aliases that route to verb sequences
```
ubo.discover      → ownership.compute
ubo.allege        → ubo.registry.create
ubo.verify        → ubo.registry.promote (CANDIDATE → IDENTIFIED)
ubo.promote       → ubo.registry.advance
ubo.approve       → ubo.registry.advance (→ APPROVED)
ubo.reject        → ubo.registry.reject
ubo.expire        → ubo.registry.expire
ubo.trace-chains  → ownership.snapshot.list
```

**Screening Macros (4):** Compound screening operations
```
screening.full           → screening.pep + screening.sanctions + screening.adverse-media
screening.pep-check      → screening.pep
screening.sanctions-check → screening.sanctions
screening.media-check    → screening.adverse-media
```

**KYC Workflow Macros (3):** End-to-end workflows
```
kyc.collect-documents → document.solicit-batch
kyc.full-review       → screening.pep + sanctions + adverse-media + document.solicit-batch
kyc.check-readiness   → document.missing-for-entity + requirement.list-outstanding
```

---

## 6. Data Model

### 6.1 Entity-Relationship Topology

```
CLIENT_GROUP (apex — the "who are we onboarding?")
  │
  ├── client_group_entity (membership: entities in this group)
  │     ├── entities (natural/legal persons)
  │     ├── client_group_entity_roles (directed roles: ManCo FOR fund X)
  │     └── cbus (subscription linkage)
  │
  ├── client_group_relationship (ownership edges between entities)
  │
  └── entity_relationships (global ownership/control graph)
        ├── from_entity_id → entities
        └── to_entity_id → entities
        Attributes: percentage, ownership_type, control_type, trust_role,
                    confidence, direct_or_indirect, temporal validity

CASES (per CBU or per client_group)
  │
  ├── entity_workstreams (per entity in case)
  │     ├── screenings (per workstream)
  │     │     └── screening results/hits
  │     ├── red_flags (risk indicators)
  │     └── document requests/requirements
  │
  ├── kyc_ubo_registry (case-scoped UBO determinations)
  │     ├── ubo_determination_runs (computation audit trail)
  │     ├── kyc_ubo_evidence (evidence linked to UBO entries)
  │     └── ubo_evidence (materialized evidence documents)
  │
  ├── tollgate_evaluations (gate pass/fail with override tracking)
  │
  ├── kyc_decisions (final determination: CLEARED/REJECTED/CONDITIONAL)
  │
  └── kyc_service_agreements (sponsor relationships)
```

### 6.2 Key Tables

| Table | PK | Key FKs | Purpose |
|-------|----|---------|---------|
| client_group | id (uuid) | — | Apex entity, discovery status tracking |
| client_group_entity | id (uuid) | group_id, entity_id, cbu_id | Membership + review status |
| entity_relationships | relationship_id (uuid) | from_entity_id, to_entity_id | Ownership/control graph edges with temporal validity, confidence, percentage |
| cases | case_id (uuid) | cbu_id, client_group_id, subject_entity_id | KYC case with status, risk rating, SLA deadline |
| entity_workstreams | workstream_id (uuid) | case_id, entity_id | Per-entity work breakdown with UBO flag, enhanced DD flag, blocker tracking |
| screenings | screening_id (uuid) | workstream_id | PEP/sanctions/adverse-media results with match count, review status |
| red_flags | red_flag_id (uuid) | case_id, workstream_id | Risk indicators with severity (SOFT/ESCALATE/HARD_STOP) and resolution tracking |
| kyc_ubo_registry | ubo_id (uuid) | case_id, subject_entity_id, ubo_person_id | UBO determinations with epistemic status (CANDIDATE→APPROVED) |
| tollgate_evaluations | evaluation_id (uuid) | case_id, tollgate_id | Gate evaluation results with override tracking |
| kyc_decisions | decision_id (uuid) | cbu_id, case_id | Final KYC decision with conditions and review interval |
| entity_identifiers | identifier_id (uuid) | entity_id | LEI, BIC, ISIN, CIK — LEI as global spine |

### 6.3 Key Integrity Constraints

1. **No self-referencing relationships:** `entity_relationships` has CHECK `from_entity_id != to_entity_id`
2. **Ownership percentage required:** When `relationship_type = 'ownership'`, percentage must be NOT NULL
3. **Temporal validity:** All relationships have `effective_from`/`effective_to` date range
4. **Confidence tracking:** Every relationship has a `confidence` field (high/medium/low/unknown)
5. **Case uniqueness:** One active case per CBU (enforced by application logic, not DB constraint)

---

## 7. Constellation Dependency Chain

The full dependency DAG for KYC onboarding:

```
                    client_group
                         │
                    gleif_import
                         │
                   ubo_discovery
                         │
                   control_chain
                    ╱          ╲
     group_kyc_clearance    cbu_identification
           (approved)             │
                ╲                 │
                 ╲────────────────│
                                  │
                             kyc_case (per CBU)
                           ╱    │    ╲
                 tollgate  ews  agreements  requests
                           │
                      screenings
                      identifiers
```

**Critical path:** client_group → gleif_import → ubo_discovery → control_chain → group_kyc_clearance(approved) → cbu_identification → kyc_case → entity_workstream → screening

**Minimum depth to first screening:** 8 dependency hops

---

## 8. ConstellationVerbIndex Integration

The newly wired `ConstellationVerbIndex` (Tier -0.5 in verb search) provides:

**Forward lookup:** Given user utterance clues (noun + action_stem), resolve to constellation-available verbs
```
("case", "close")     → [kyc-case.close]           ← single match, short-circuit
("screening", "run")   → [screening.run]             ← single match, short-circuit
("document", "solicit") → [document.solicit, document.solicit-batch] ← 2 candidates
("entity", "update")   → [entity-workstream.update-status, red-flag.update] ← 2 candidates
```

**Reverse lookup:** Given verb FQN, locate which constellation slot it lives on
```
screening.run → screening[empty]
kyc-case.close → kyc_case[open]
document.solicit → entity_workstream[documents_requested]
```

**Post-execution feedback loop:**
```
Execute verb → writes_since_push++ → rehydrate constellation →
  rebuild ConstellationVerbIndex → compute narration (suggested_next) →
  user types next action → constellation index resolves → execute → ...
```

---

## 9. Pack & Journey Configuration

### KYC Case Pack

**Allowed verbs:** 100 verbs spanning kyc, ubo, document, screening, case, evidence, red-flag, screening-ops, structure, tollgate, party domains

**Templates:**
1. **new-kyc-case:** create-case → ubo.discover → document.solicit-batch → screening.run → assign-reviewer
2. **renewal-kyc-case:** create-case → screening.run → (optional) document.solicit-batch

**Required questions:** entity_name, case_type (new/renewal)
**Optional questions:** reviewer, risk_rating, document_types

**Risk policy:** `require_confirm_before_execute: true`, `max_steps_without_confirm: 3`

---

## 10. Observations & Review Points

### 10.1 Strengths

1. **Two-phase architecture is sound.** Group clearance before CBU cases prevents orphaned cases and ensures the ownership perimeter is established first.

2. **State machine discipline.** Every slot has a state machine. Verb availability is gated by state. No verb can fire outside its declared states.

3. **Dependency chains enforce sequencing.** You can't screen until a workstream exists, can't create a workstream until a case exists, can't create a case until group clearance is approved.

4. **Macro abstraction.** Users say "discover UBOs" and the system routes through the correct underlying verb sequence. The vocabulary is practitioner-friendly.

5. **Entity_workstream as hub.** Putting red flags, documents, and requirements as verb palettes on the workstream slot (not separate slots) is the right call — they're state annotations, not independent entities.

### 10.2 Areas for Review

1. **Verb count on entity_workstream (37 verbs).** This slot carries verbs from 6 sub-domains. While architecturally correct (they're all annotations on entity state), the ConstellationVerbIndex shows collisions in the (noun, action) space for broad nouns like "entity" or "workstream". The domain-prefix noun extraction mitigates this but some disambiguation is still needed.

2. **group.ownership constellation size (119 verbs).** This is the largest constellation. The `control_chain` slot alone has 30 ownership/control verbs. Consider whether some of these could be collapsed (e.g., `ownership.right.add-to-class` vs `ownership.right.add-to-holder` — could these be a single `ownership.right.add` with a type argument?).

3. **Pack allowed_verbs drift.** The kyc-case pack lists 100 allowed verbs including some that don't appear in any constellation map (e.g., `case.add-party`, `case.select`, `kyc-workstream.add`). These may be legacy verb FQNs that need cleanup. If they're not on any constellation slot, the ConstellationVerbIndex can't resolve them.

4. **Screening lifecycle complexity.** The 12-state screening lifecycle with sequential type processing (sanctions → PEP → adverse-media) may be over-specified. In practice, many providers run all three screens concurrently. The sequential model was designed for manual review but may not match modern automated screening providers.

5. **UBO epistemic lifecycle is elegant but under-tested.** The 5-state progression (undiscovered → alleged → provable → proved → approved) with overlay conditions is sophisticated. The conditions check evidence verification and blocking screenings. However, only 3 test cases in the utterance harness cover UBO transitions.

6. **Tollgate thresholds are hard-coded.** The 3 standard gates (SKELETON_READY at 70%, EVIDENCE_COMPLETE at 100%, REVIEW_COMPLETE requiring all UBOs approved) have default thresholds in `tollgate_definitions`. These should be configurable per jurisdiction and risk band — a Luxembourg UCITS SICAV has different requirements than a Cayman hedge fund.

7. **Document lifecycle has two parallel tracks.** There's both a `document_lifecycle` state machine (missing→requested→received→verified) and a `document_request_lifecycle` (pending→requested→received→verified). The relationship between document requirements and document requests could be clarified — are these the same thing tracked in two tables, or genuinely different concepts?

---

## 11. Data Model Consistency Check

### Tables declared in constellation maps vs. schema

| Constellation slot | Declared table | Schema exists? | Notes |
|-------------------|---------------|----------------|-------|
| kyc_case.table: cases | cases | Yes | |
| entity_workstream (join via) | entity_workstreams | Yes | |
| screening (join via) | screenings | Yes | |
| kyc_agreement (join via) | kyc_agreements | **Check** | May be kyc_service_agreements |
| identifier (join via) | entity_identifiers | Yes | |
| request (join via) | kyc_requests | **Check** | Distinct from document requests? |
| tollgate (join via) | tollgate_evaluations | Yes | |
| ubo_discovery (join via) | ubo_registry | **Check** | May be kyc_ubo_registry |
| control_chain (join via) | ownership_snapshots | Yes | |
| client_group | client_group | Yes | |
| client_group_entity (join via) | client_group_entity | Yes | |
| board (join via) | board_appointments | **Check** | |
| bods (join via) | bods_statements | **Check** | |

Items marked **Check** need verification that the join table name in the constellation YAML matches the actual schema table name. Mismatches would cause hydration failures.

---

## 12. Recommendations

1. **Audit pack allowed_verbs against constellation maps.** Every verb in a pack should appear on at least one constellation slot in that pack's workspaces. Verbs that don't exist on any slot are dead weight that confuses pack scoring.

2. **Add integration test for the post-execution narration loop.** The full round-trip (execute → rehydrate → narration → constellation index → next verb) is the core value proposition but has no end-to-end test coverage.

3. **Verify join table names.** The constellation YAML declares join paths (`via: kyc_agreements`, `via: ubo_registry`). If these don't match actual table names, hydration silently returns empty slots.

4. **Consider verb consolidation in control_chain.** The 30 verbs on this slot include 8 `ownership.right.*` and 4 `ownership.reconcile.*` sub-verbs. These could potentially be reduced to fewer verbs with richer arguments.

5. **Document the two-phase architecture in a user-facing guide.** The group-then-CBU model is non-obvious. Practitioners expect to "open a case for Allianz" directly, but the system requires group clearance first. This needs clear UX messaging.

---

*End of review.*
