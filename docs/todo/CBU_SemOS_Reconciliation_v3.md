# CBU Canonical Reconciliation

> **Date:** 2026-03-12
> **Status:** Canonical working reconciliation document
> **Purpose:** Define the current CBU source of truth across schema, DSL, StateGraph, SemOS metadata, and utterance discovery

---

## 1. Scope

This document is the single working reconciliation artifact for CBU in `ob-poc`.

It replaces the prior split across:

- lifecycle definition
- deep-dive reference
- SemOS reconciliation notes
- verb rename / split matrix

It treats the live repo as source of truth and separates:

1. intrinsic persisted lifecycle state
2. linked-state obligations
3. computed readiness and projection state
4. current SemOS/runtime gaps
5. the canonical CBU verb surface

---

## 2. Canonical Sources

The authoritative sources used here are:

- `migrations/master-schema.sql`
- `rust/config/verbs/cbu.yaml`
- `rust/src/domain_ops/cbu_ops.rs`
- `rust/config/stategraphs/cbu.yaml`
- `rust/src/database/semantic_state_service.rs`
- `rust/config/lexicon/verb_concepts.yaml`
- `rust/config/verb_schemas/intent_tiers.yaml`
- `rust/config/sem_os_seeds/domain_metadata.yaml`
- `scripts/teach_cbu_phrases.sql`

This document is the human-readable reconciliation layer over those sources.

---

## 3. What CBU Is

A CBU is the operational onboarding and servicing anchor for a client structure.

In the current runtime it is:

- a row in `"ob-poc".cbus`
- linked to parties through `"ob-poc".cbu_entity_roles`
- linked to onboarding, KYC, evidence, products, and discovery signals
- used as an operational grouping and workflow anchor

It is not yet a fully SemOS-authored first-class object with one unified lifecycle contract spanning all dependent subsystems.

---

## 4. Canonical Lifecycle

### 4.1 Intrinsic stable states

The canonical stable intrinsic CBU lifecycle states are:

1. `DISCOVERED`
2. `VALIDATION_PENDING`
3. `VALIDATED`
4. `UPDATE_PENDING_PROOF`
5. `VALIDATION_FAILED`

These map directly to `cbus.status`.

### 4.2 State meanings

- `DISCOVERED`
  - the CBU exists as an onboarding anchor and validation has not completed
- `VALIDATION_PENDING`
  - the CBU is in formal review / validation
- `VALIDATED`
  - the current validation cycle passed
- `UPDATE_PENDING_PROOF`
  - a previously validated CBU now requires further proof or re-validation
- `VALIDATION_FAILED`
  - the current validation cycle did not pass

### 4.3 Explicit non-states

The following are not canonical intrinsic CBU lifecycle states:

- `DRAFT`
- `ACTIVE`
- `SUSPENDED`
- `REINSTATED`
- `TERMINATED`

Those may appear in older conceptual material or adjacent subsystems, but they are not the implemented CBU lifecycle source of truth.

### 4.4 Canonical transition table

The current canonical transition shape is:

1. `DISCOVERED -> VALIDATION_PENDING`
2. `DISCOVERED -> VALIDATION_FAILED`
3. `VALIDATION_PENDING -> VALIDATED`
4. `VALIDATION_PENDING -> VALIDATION_FAILED`
5. `VALIDATION_PENDING -> DISCOVERED`
6. `VALIDATED -> UPDATE_PENDING_PROOF`
7. `UPDATE_PENDING_PROOF -> VALIDATED`
8. `UPDATE_PENDING_PROOF -> VALIDATION_FAILED`
9. `VALIDATION_FAILED -> VALIDATION_PENDING`
10. `VALIDATION_FAILED -> DISCOVERED`

### 4.5 Current enforcement reality

This transition table is the canonical contract, but present enforcement is only partial:

- the valid status set is enforced on `cbus.status`
- the transition table exists in schema as a validation function
- explicit lifecycle verbs now exist in the DSL surface
- but repo cleanup and runtime hard-gating still need to converge fully around those verbs

So this section defines the canonical lifecycle target the platform should obey, not a claim that every historical path is already uniformly governed.

---

## 5. Linked-State Obligations

These are real CBU lifecycle-adjacent obligations, but they are not intrinsic `cbus.status` states.

### 5.1 Party and role obligations

Persisted through:

- `"ob-poc".cbu_entity_roles`
- `"ob-poc".roles`

Policy:

- party completeness is a linked obligation
- it must not be collapsed into a synthetic replacement for `cbus.status`

### 5.2 Evidence and document obligations

There are currently two overlapping evidence layers:

1. local `cbu_evidence`
2. generic governed document requirements

Policy:

- evidence readiness is a linked obligation
- document policy should increasingly be SemOS-governed
- this obligation must not be conflated with intrinsic lifecycle state

### 5.3 Product, servicing, and trading obligations

Operational readiness also depends on downstream state such as:

- product subscriptions
- service delivery maps
- trading profiles
- instrument universe
- SSI and booking rules
- ISDA / CSA
- pricing config
- share classes and holdings

Policy:

- these are linked readiness obligations
- they are operationally important
- they must not replace the core validation-centric lifecycle

### 5.4 KYC and workstream obligations

CBU participates in KYC/workstream state through related case and workflow tables.

Policy:

- KYC state is distributed across related workflow systems
- CBU core status is not the full KYC lifecycle

---

## 6. Computed and Projected State

### 6.1 Computed readiness

`derive_semantic_state()` is the current best implementation of the broader CBU journey.

It computes progress across dependent surfaces such as:

- product subscriptions
- KYC cases
- workstreams
- trading profiles
- instrument universe
- SSI and booking rules
- legal agreements
- servicing resources
- pricing config
- holdings structures

This semantic state is important, but it is not the intrinsic lifecycle definition.

### 6.2 StateGraph and discovery signals

The current CBU StateGraph is deliberately shallow and acts as a projection layer.

It currently models signals such as:

- `has_active_onboarding`
- `pending_document_count`
- `has_incomplete_ubo`

Rule:

- discovery/stategraph signals must be derived from lifecycle state plus obligations
- they must not become the lifecycle source of truth

---

## 7. Canonical CBU Verb Surface

The CBU DSL should be organized into four verb families.

### 7.1 Lifecycle verbs

These own intrinsic lifecycle movement:

- `cbu.create`
- `cbu.ensure`
- `cbu.submit-for-validation`
- `cbu.decide`
- `cbu.request-proof-update`
- `cbu.reopen-validation`

Lifecycle policy:

- `cbu.submit-for-validation` owns movement into `VALIDATION_PENDING`
- `cbu.decide` owns approval / rejection / referral outcomes
- `cbu.request-proof-update` owns movement into `UPDATE_PENDING_PROOF`
- `cbu.reopen-validation` owns explicit retry/reopen paths

### 7.2 Structure property verbs

These own non-lifecycle edits to the persisted `cbus` row:

- `cbu.rename`
- `cbu.set-jurisdiction`
- `cbu.set-client-type`
- `cbu.set-commercial-client`
- `cbu.set-category`

Property policy:

- property verbs must not directly imply lifecycle state movement

### 7.3 Linked-obligation verbs

These mutate lifecycle-adjacent obligations:

- `cbu.assign-role`
- `cbu.remove-role`
- `cbu.attach-evidence`
- `cbu.verify-evidence`
- `cbu.add-product`
- `cbu.remove-product`

### 7.4 Read and navigation verbs

- `cbu.read`
- `cbu.list`
- `cbu.show`
- `cbu.parties`
- `cbu.list-evidence`
- `cbu.list-subscriptions`
- `session.load-cbu`
- `session.unload-cbu`

---

## 8. Discovery and Lexicon Rules

The CBU discovery surface must optimize for precision, not generic mutation recall.

Rules:

- a lifecycle utterance must not prefer a property-edit verb
- a property-edit utterance must not prefer a lifecycle verb
- a product, party, or evidence utterance must not collapse into a generic core-row edit verb
- non-canonical state language must not be taught as preferred CBU lifecycle language

The active discovery/alignment sources that must agree are:

1. `rust/config/verbs/cbu.yaml`
2. `rust/config/lexicon/verb_concepts.yaml`
3. `scripts/teach_cbu_phrases.sql`
4. `rust/config/verb_schemas/intent_tiers.yaml`
5. agent and noun indexes that surface verb candidates

---

## 9. Root Causes of Prior Discovery Failures

The main historical root causes were:

1. `cbu.update` acted as a catch-all mutation verb
2. lexical training routed unrelated intents into that catch-all
3. some phrases taught non-canonical lifecycle language such as `active` / `inactive`
4. config surfaces were not consistently aligned on one CBU taxonomy

That degraded:

- SemOS mapping quality
- utterance precision
- explainability of verb selection
- confidence in lifecycle governance

The current cleanup direction is to keep only the narrow verb family above and remove legacy catch-all discovery paths.

---

## 10. Current SemOS Gaps

The material remaining gaps are:

1. SemOS does not yet own the CBU lifecycle contract end-to-end
2. evidence/document governance is still split across local and governed models
3. lifecycle hard enforcement is not yet fully unified around the explicit lifecycle verbs
4. product, trading, servicing, and KYC readiness are still modeled as adjacent surfaces rather than one SemOS-governed lifecycle vocabulary
5. StateGraph remains a shallow projection over a broader real operational journey

So the lifecycle definition is currently ahead of the full SemOS implementation.

---

## 11. Canonical Invariants

The CBU model should obey these invariants:

1. a CBU cannot be `VALIDATED` unless it has passed the current validation decision path
2. a validated CBU that undergoes material change must move to `UPDATE_PENDING_PROOF`
3. linked obligations must not be treated as synonyms for `cbus.status`
4. computed readiness must not overwrite intrinsic lifecycle state without an explicit transition rule
5. discovery/stategraph signals must remain projections

---

## 12. Working Conclusion

The canonical source of truth for CBU should be:

1. this reconciliation definition
2. schema/runtime constraints
3. the explicit CBU DSL taxonomy
4. SemOS metadata and policy representations
5. discovery/stategraph projections

Not the other way around.

The cleanup direction is therefore:

- keep the validation-centric intrinsic lifecycle explicit
- model party, evidence, product, trading, and servicing as linked obligations or readiness dimensions
- keep the verb lexicon narrow and crisp
- let SemOS eventually own the reconciled contract

