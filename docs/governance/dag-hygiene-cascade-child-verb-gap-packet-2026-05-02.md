# DAG Hygiene Cascade Child Verb Gap Packet - 2026-05-02

Scope: Implemented cascade child-verb gap packet retained as governance evidence.

Purpose: identify which Phase 5 cascade refactors can proceed using existing registered verbs, and where Adam approval is required before adding child verbs.

## Summary

Initial inventory found partial child coverage:

- Clear existing children: `capital.issue-shares`, `kyc-case.update-status`, `service-resource.provision`, `delivery.start`, `client-group.entity-remove`, `cbu.unlink-structure`.
- Existing-but-not-exact children: `cbu.remove-role`, `cbu.unlink-structure`, `client-group.entity-remove`, `trading-profile.clone-to`.
- Missing or unconfirmed children: entity relationship upsert, CBU group membership remove, entity deactivate by entity-id, service-intent activate, template-to-CBU trading-profile clone, CBU role terminate by CBU/all roles.

This packet recommended adding small child verbs only where the current source had SemOS-governed row mutations with no exact existing verb. It did not recommend new architecture.

Implementation status: Adam decisions below have been applied. Cascade parents covered by this tranche now use existing registry child dispatch for approved off-carrier SemOS-governed writes.

## Existing Child Verbs Confirmed

| Child FQN | Evidence | Current fit |
| --- | --- | --- |
| `capital.issue-shares` | `rust/crates/sem_os_postgres/src/ops/capital.rs` | Updates `share_classes.issued_shares` by additional share count. Likely child for `capital.adjust-holding` if parent can pass delta not absolute target. |
| `kyc-case.update-status` | `rust/crates/sem_os_postgres/src/ops/kyc_case.rs` | Non-terminal status update only. Does not close terminal APPROVED/REJECTED cases; those use `kyc-case.close`. |
| `delivery.start` | `rust/src/domain_ops/simple_status_op.rs` | SimpleStatus verb for delivery lifecycle, but not an insert into `service_delivery_map`. |
| `service-resource.provision` | `rust/crates/sem_os_postgres/src/ops/service_resource.rs` | Inserts `cbu_resource_instances`; accepts `cbu-id`, `resource-type`, optional `product-id`, optional `service-id`. Good child for resource instance creation. |
| `client-group.entity-remove` | `rust/crates/sem_os_postgres/src/ops/client_group.rs` | Removes or marks a specific `(group_id, entity_id)` membership. Not exact for `client_group_entity SET cbu_id = NULL WHERE cbu_id = ...`. |
| `cbu.unlink-structure` | `rust/crates/sem_os_postgres/src/ops/cbu.rs` | Soft-terminates one active link by `link-id` and `reason`. Not exact for bulk delete by CBU id. |
| `cbu.remove-role` | `rust/config/verbs/cbu.yaml` | CRUD role unlink on `cbu_entity_roles`, keyed as a role unlink. Need confirm executor path and arg shape before using from plugin parent. |
| `trading-profile.clone-to` | `rust/config/verbs/trading-profile.yaml` | Clones existing profile to another CBU. Not exact for template-to-instance clone unless templates are represented as source profiles. |

## Parent Inventory

### `capital.adjust-holding`

Current off-carrier write:

- `share_classes.issued_shares` update.

Candidate child:

- `capital.issue-shares`.

Gap:

- Confirm parent semantics are additive. `capital.issue-shares` takes `additional-shares`; if `adjust-holding` computes an absolute target, it must translate to a delta before dispatch.

Recommendation:

- Proceed after confirming argument mapping.

### `cbu.decide`

Current writes:

- `cbus.status` is the parent carrier state.
- `cases.status` or `cases.escalation_level` is off-carrier SemOS-governed case state.
- `case_evaluation_snapshots` insert is audit/event evidence and should remain in parent unless separately reclassified.

Candidate children:

- `kyc-case.update-status` for non-terminal `REVIEW`.
- `kyc-case.close` for terminal `APPROVED` / `REJECTED`.

Gap:

- The original Phase 5 note named only `kyc-case.update-status`, but current code maps APPROVED/REJECTED to terminal case states. `kyc-case.update-status` intentionally rejects terminal statuses.

Recommendation:

- Refactor with two children: `kyc-case.close` for APPROVED/REJECTED, `kyc-case.update-status` for REFERRED/REVIEW. Confirm `kyc-case.close` args before implementation.

### `cbu.assign-ownership`

Current parent carrier:

- `cbu_entity_roles` insert/update.

Current off-carrier write:

- `entity_relationships` upsert with `relationship_type = 'ownership'`, percentage, ownership_type, effective_from, source, confidence.

Candidate child:

- None confirmed.

Recommended child:

- Add an entity-relationship upsert child, for example `entity-relationship.upsert`, if Adam approves the FQN.

Reason:

- Three parent verbs converge on this carrier and currently duplicate relationship graph writes.

### `cbu.assign-control`

Current parent carrier:

- `cbu_entity_roles` insert/update.

Current off-carrier write:

- `entity_relationships` upsert with `relationship_type = 'control'`, control_type, effective_from, source, confidence.

Candidate child:

- None confirmed.

Recommended child:

- Same approved entity-relationship upsert child as `cbu.assign-ownership`.

### `cbu.assign-trust-role`

Current parent carrier:

- `cbu_entity_roles` insert/update.

Current off-carrier write:

- `entity_relationships` upsert with trust relationship type and trust-interest metadata.

Candidate child:

- None confirmed.

Recommended child:

- Same approved entity-relationship upsert child as `cbu.assign-ownership`, with trust metadata args.

### `cbu.create`

Current parent carrier:

- `cbus` insert/update.

Current off-carrier writes:

- `cbu_entity_roles` insert for fund asset-owner role.
- `client_group_entity.cbu_id` update for fund entity.
- `cbu_entity_roles` insert for manco management-company role.

Candidate children:

- Existing `cbu.assign-fund-role` may cover some role inserts, but arg and emitted-state semantics differ.
- No exact confirmed child for linking an existing client-group entity row to a CBU by setting `client_group_entity.cbu_id`.

Gaps:

- Need exact child for CBU role assignment by role name where target semantics match `cbu.create`.
- Need exact child for `client_group_entity.cbu_id = cbu_id`.

Recommendation:

- Do not refactor `cbu.create` until approved child verbs exist or existing role verbs are confirmed exact.

### `cbu.add-product`

Current parent carrier:

- Product assignment intent is expressed by `cbu.add-product`, but the concrete writes are all child rows.

Current off-carrier writes:

- `service_delivery_map` insert with `delivery_status = 'PENDING'`.
- `cbu_resource_instances` insert with `status = 'PENDING'`.

Candidate children:

- `service-resource.provision` covers resource instance creation if called per required resource.
- `delivery.start` is not an insert child for `service_delivery_map`; it changes lifecycle status of an existing delivery row.
- `service-intent.activate` was not confirmed.
- `trading-profile.clone-to` exists, but `trading-profile.clone-from-template` was not confirmed.

Gaps:

- Need child for creating service delivery map rows, likely `delivery.create` or approved equivalent.
- Need child for service intent activation if `service_intent.active` seeding remains in scope.
- Need child for trading-profile template clone if template-to-instance clone remains in scope.

Recommendation:

- Do not refactor `cbu.add-product` until child FQNs are approved.

### `cbu.delete-cascade`

Current parent carrier:

- `cbus.deleted_at` update.

Current off-carrier writes:

- `client_group_entity.cbu_id = NULL` for all rows linked to the CBU.
- `cbu_group_members` delete by CBU.
- `cbu_structure_links` delete where parent or child CBU matches.
- `entities.deleted_at` update for exclusive entities.
- `cbu_entity_roles` delete by CBU.

Candidate children:

- `client-group.entity-remove` removes a `(group_id, entity_id)` membership, not a CBU link.
- `cbu.unlink-structure` soft-terminates one link by `link-id`; current parent bulk-deletes links by CBU.
- `cbu.remove-role` may remove one role link; not confirmed for all roles by CBU.
- No exact entity deactivate child was confirmed. `refdata.core.deactivate` is unrelated.

Gaps:

- Need child for unlinking client-group entity rows from a CBU without deleting group membership.
- Need child for removing CBU group membership rows.
- Need child or approved loop using `cbu.unlink-structure` after selecting link ids; this changes delete semantics to soft termination.
- Need child for entity deactivation by `entity-id`.
- Need child for terminating/removing all CBU entity roles for a CBU, or an approved loop over `cbu.remove-role`.

Recommendation:

- Do not refactor `cbu.delete-cascade` until Adam approves soft-termination semantics and missing child FQNs.

## Approval Questions

1. Approve adding `entity-relationship.upsert` as the shared child for ownership/control/trust relationship graph writes?
2. For `cbu.decide`, approve using `kyc-case.close` for APPROVED/REJECTED and `kyc-case.update-status` for REFERRED?
3. For `cbu.create`, should role linking use existing role verbs if arg shapes can be made exact, or should we add narrow child verbs for create-time role attachment?
4. For `cbu.add-product`, approve new child FQN for `service_delivery_map` insert: `delivery.create`?
5. For `cbu.add-product`, is `service-intent.activate` still required in this campaign, and should it be added if absent?
6. For trading profile template clone, is existing `trading-profile.clone-to` sufficient, or do we need `trading-profile.clone-from-template`?
7. For `cbu.delete-cascade`, should hard deletes of structure links and roles become soft/semantic terminations via child verbs, or must exact hard-delete behavior be preserved?
8. Approve adding narrow children for CBU deletion cleanup:
   - `client-group.unlink-cbu`
   - `cbu-group.remove-member`
   - `entity.deactivate`
   - `cbu-role.terminate`

## Stop Point - Closed

The original stop point was:

- `capital.adjust-holding`, after additive mapping is confirmed.
- `cbu.decide`, after `kyc-case.close` arg mapping is confirmed.

Adam approval was captured in the decision block below. The implementation proceeded using those decisions.

## Adam Decisions - 2026-05-02

1. `entity-relationship.upsert` approved as the shared child for ownership/control/trust relationship graph writes.
2. `cbu.decide` approved to use `kyc-case.close` for APPROVED/REJECTED and `kyc-case.update-status` for REFERRED.
3. `cbu.create` must use existing general role verbs where they can upsert exactly. Do not add create-time special-case role verbs if a general verb can express the write.
4. Do not model `cbu.add-product` as a user-facing `delivery.create` lifecycle step. The intended lifecycle is: add product, look up dependent services from product taxonomy, then discover/look up dependent resources from service-resource taxonomy/slot switch rules, and provision automatically.
5. `service-intent.activate` is not required for this campaign.
6. Trading profile clone should reuse the existing clone verb with an argument naming the template/source to clone from. Do not add a separate clone architecture.
7. `cbu.delete-cascade` needs both modes: hard deletes for testing and soft/semantic termination for production.
8. Narrow deletion cleanup children approved where needed:
   - `client-group.unlink-cbu`
   - `cbu-group.remove-member`
   - `entity.deactivate`
   - `cbu-role.terminate`

## Implementation Closeout - 2026-05-02

- `SemOsChildDispatcher` dispatches child verbs through `SemOsVerbOpRegistry` inside the caller's transaction scope.
- `cbu.create`, `cbu.add-product`, `cbu.decide`, `cbu.delete-cascade`, and CBU role assignment paths use registry child dispatch for approved off-carrier writes.
- `entity-relationship.upsert` is the shared child for ownership/control/trust/fund-role relationship graph writes.
- `cbu.delete-cascade` supports hard-delete mode for tests and semantic/soft cleanup mode for production child cleanup verbs where supported.
- `reconcile validate` now scans the known cascade parent slices and fails if direct off-carrier mutations are reintroduced.
