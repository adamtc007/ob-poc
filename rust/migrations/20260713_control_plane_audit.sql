-- EOP-DESIGN-CONTROLPLANE-G2-AUDIT-PROVENANCE-001 (v0.2, RATIFIED) §2:
-- append-only audit stream, one row per control-plane lifecycle event.
--
-- Deviation from the design doc's literal DDL, recorded here per
-- CLAUDE.md's migration discipline (forward-only, defects reported not
-- silently patched): the doc's schema sketch declares `session_id TEXT`
-- "same convention as shadow rows" -- but `control_plane_shadow_decisions`
-- (20260710_control_plane_shadow_decisions.sql) and `control_plane_envelopes`
-- (20260710_control_plane_envelopes.sql) both actually use `session_id
-- UUID NOT NULL`. This migration follows the REAL convention (UUID), not
-- the doc's literal type, since the doc's own stated intent ("same
-- convention as shadow rows") is best honoured by matching the type those
-- rows actually use. See the implementing session doc (V5) for the full
-- verification note.
--
-- No hash chain, no digests (struck at ratification -- see the design
-- doc's status header). Append-only-by-convention plus same-transaction
-- emission where a transaction exists is the stream's only guarantee.
CREATE TABLE "ob-poc".control_plane_audit (
    seq         BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    decision_id UUID        NOT NULL,
    event_type  TEXT        NOT NULL,   -- serialized AuditEvent discriminant
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    session_id  UUID        NOT NULL,   -- same convention as shadow rows (V5)
    payload     JSONB       NOT NULL    -- typed per event_type, see ob_poc_control_plane::audit::AuditEvent
);

CREATE INDEX idx_control_plane_audit_decision_id
    ON "ob-poc".control_plane_audit (decision_id);

CREATE INDEX idx_control_plane_audit_event_type
    ON "ob-poc".control_plane_audit (event_type);

COMMENT ON TABLE "ob-poc".control_plane_audit IS
    'EOP-DESIGN-CONTROLPLANE-G2-AUDIT-PROVENANCE-001 §2: append-only per-decision lifecycle event stream. Storage substrate for the consume_seam/post_dispatch gate-outcome provenance values and the G11 AuditReplay evaluation surface. No hash chain (struck at ratification -- single-operator deployment, no adversarial threat model).';
