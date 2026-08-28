-- EOP-DD-UBO-PROOF-001 §4 (T5, 2026-08-28): "EdgeStatus::Verified goes;
-- ... each assertion carries its set of proofs." `EdgeState.evidence_event_id`
-- (a single scalar "the" evidence event) is gone with the ratchet it fed —
-- an edge now carries zero or more proofs, not one. This is a DISPOSABLE,
-- REBUILDABLE fold projection (K-34, EOP-DD-KYCUBO-002 §5): dropping a
-- column loses nothing, since the stream is the system of record and the
-- table is rebuilt from it. No replacement column added here — the full
-- citation set (kind/source/date per proof) lives in the substrate fold
-- (`EdgeState::proofs`), not in this status-only projection.

ALTER TABLE "ob-poc".kyc_control_edge_projection
    DROP COLUMN IF EXISTS evidence_event_id;
