# Constellation UI Backend Execution Plan

## Objective

Close the remaining constellation UI/backend gaps that are still affecting operational usability:

1. Add case discovery so the chat-session constellation panel can bind to a real case without manual UUID entry.
2. Enrich the ownership-chain payload so the UI can render actual graph nodes and edges, not just summary counts.

## Scope

### Backend

- Extend [`rust/src/api/constellation_routes.rs`](/Users/adamtc007/Developer/ob-poc/rust/src/api/constellation_routes.rs) with `GET /api/cbu/:cbu_id/cases`.
- Extend constellation hydrated payloads in [`rust/src/sem_reg/constellation/hydrated.rs`](/Users/adamtc007/Developer/ob-poc/rust/src/sem_reg/constellation/hydrated.rs) with first-class ownership graph node/edge arrays.
- Update normalization in [`rust/src/sem_reg/constellation/normalize.rs`](/Users/adamtc007/Developer/ob-poc/rust/src/sem_reg/constellation/normalize.rs) to populate those arrays from existing raw hydration data.
- Add or update tests in [`rust/tests/constellation_hydration_tests.rs`](/Users/adamtc007/Developer/ob-poc/rust/tests/constellation_hydration_tests.rs).

### Frontend

- Extend [`ob-poc-ui-react/src/api/constellation.ts`](/Users/adamtc007/Developer/ob-poc/ob-poc-ui-react/src/api/constellation.ts) with case-list and graph payload types.
- Extend [`ob-poc-ui-react/src/lib/query.ts`](/Users/adamtc007/Developer/ob-poc/ob-poc-ui-react/src/lib/query.ts) with case query keys.
- Replace manual case UUID input in [`ob-poc-ui-react/src/features/chat/components/ConstellationPanel.tsx`](/Users/adamtc007/Developer/ob-poc/ob-poc-ui-react/src/features/chat/components/ConstellationPanel.tsx) with a case dropdown backed by the new endpoint.
- Add an ownership inspector view in the same component using the enriched graph node/edge payload.

## Execution Order

1. Backend payload changes.
2. Backend case-discovery route.
3. Rust verification.
4. Frontend API/type/query updates.
5. Frontend constellation panel updates.
6. Frontend verification.

## Completion Criteria

- The constellation panel can list available cases for the selected CBU and switch between them.
- The ownership-chain inspector renders actual node and edge data from the backend payload.
- `cargo check` passes in [`rust`](/Users/adamtc007/Developer/ob-poc/rust).
- `npm run build` passes in [`ob-poc-ui-react`](/Users/adamtc007/Developer/ob-poc/ob-poc-ui-react).
