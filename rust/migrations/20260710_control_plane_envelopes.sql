-- T4.2 (EOP-PLAN-CONTROLPLANE-001): sealed-envelope persistence, single-use
-- enforcement, and TTL. Beside the runbook store, mirroring the session
-- invocation-record pattern ("ob-poc".repl_invocation_records — see
-- session_repository.rs::save_invocation): one row per sealed
-- ExecutionEnvelope, an upsert-on-insert row with a status column and a
-- validity window. EnvelopeHandle rehydration is only ever a status-checked
-- UPDATE against this table (try_consume), never a raw deserialize of a
-- stored envelope body — no envelope content is persisted, only its
-- identity (id + content_hash) and lifecycle bookkeeping (K-13/§6.10.4).
CREATE TABLE "ob-poc".control_plane_envelopes (
    envelope_id UUID PRIMARY KEY,
    -- Hex-encoded SHA-256 of the sealed envelope's content
    -- (EnvelopeHandle::content_hash_hex()) — lets a consumer detect a
    -- handle that no longer matches the envelope it was minted from.
    content_hash TEXT NOT NULL,
    session_id UUID NOT NULL,
    verb_fqn TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'sealed'
        CHECK (status IN ('sealed', 'consumed', 'expired', 'voided')),
    not_before TIMESTAMPTZ NOT NULL,
    not_after TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    void_reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);

CREATE INDEX idx_control_plane_envelopes_session
    ON "ob-poc".control_plane_envelopes (session_id, created_at DESC);

-- Fast path for TTL sweeps / expiry queries.
CREATE INDEX idx_control_plane_envelopes_sealed_not_after
    ON "ob-poc".control_plane_envelopes (not_after)
    WHERE status = 'sealed';

COMMENT ON TABLE "ob-poc".control_plane_envelopes IS
    'T4.2 sealed ExecutionEnvelope identity + single-use/TTL bookkeeping (EOP-PLAN-CONTROLPLANE-001). No envelope content is stored, only its handle identity and lifecycle status; rehydration is a status-checked consume, never a raw deserialize.';
