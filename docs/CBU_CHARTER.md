# CBU (Client Business Unit) Charter

## Definition

A CBU is the **relationship/mandate context** that anchors all client-related data.

## A CBU IS:
- The composite root for a client relationship with the bank
- A thin placeholder that gets incrementally enriched
- The anchor for: entities, KYC cases, documents, services, custody, investor registry
- Scoped by `cbu_category` (FUND_MANDATE, CORPORATE_GROUP, INSTITUTIONAL_ACCOUNT, etc.)

## A CBU IS NOT:
- A legal entity (entities are separate, linked via roles)
- A status container (lifecycle is derived from domain tables)
- A document store (documents link TO CBU, not stored IN it)

## Invariants (must always hold)

1. **Commercial Client Sync**: If `commercial_client_entity_id` is set, a corresponding row MUST exist in `cbu_entity_roles` with role='COMMERCIAL_CLIENT'

2. **Category Required**: Every CBU MUST have a `cbu_category` set

3. **At Least One Entity**: Every active CBU SHOULD have at least one entity assigned via `cbu_entity_roles`

4. **Jurisdiction Required**: Every CBU MUST have a `jurisdiction` set

## Lifecycle (derived, not stored)

CBU lifecycle is computed from domain tables:
- `kyc.cases.status` -> onboarding/KYC state
- `entity_kyc_status` -> per-entity clearance
- `service_delivery_map.delivery_status` -> service state  
- `cbu_resource_instances.status` -> operational state

Use `v_cbu_lifecycle` view for current state.

## Ownership

- Schema: `ob-poc.cbus`
- DSL domain: `cbu.*`
- Visualization: `CbuGraphBuilder` in `VisualizationRepository`
