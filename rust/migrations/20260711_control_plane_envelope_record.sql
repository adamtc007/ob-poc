-- T10.1 (EOP-PLAN-CONTROLPLANE-001 Addendum C, B2): adds the sealed
-- envelope's flattened, storable content (`ob_poc_control_plane::envelope::
-- EnvelopeRecord`, serialised) beside the identity-only bookkeeping T4.2
-- already persists. Deliberately a nullable column, not a schema
-- redesign: existing 'sealed'/'consumed'/'expired'/'voided' rows and the
-- T4.2 `persist_sealed` call shape are untouched; only real T10.1 seal
-- attempts populate it. `EnvelopeRecord` itself carries no proof-typed
-- value that could be deserialised back into an execution-accepted
-- ExecutionEnvelope (see that type's own doc) — this column is read-only
-- observability + T10.2's pin-comparison source, never a rehydration path.
ALTER TABLE "ob-poc".control_plane_envelopes
    ADD COLUMN record JSONB;

COMMENT ON COLUMN "ob-poc".control_plane_envelopes.record IS
    'T10.1: serialised EnvelopeRecord (flattened, primitive-typed projection of the sealed envelope — pins, bound entities, pack id, validity). NULL for pre-T10.1 rows and any row inserted without a real seal.';
