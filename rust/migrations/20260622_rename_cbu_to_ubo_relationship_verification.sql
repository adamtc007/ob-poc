-- Rename cbu_relationship_verification → ubo_relationship_verification.
--
-- This table holds KYC/UBO relationship-verification data (alleged vs proven
-- ownership/control %, proof documents, verification status). It is written
-- exclusively by ubo.* verbs and read by the KYC tollgate + UBO visualization;
-- no cbu.* verb touches it. It is cbu_id-SCOPED (a verification within the
-- context of a CBU), but it is NOT CBU-structural state — the `cbu_` prefix made
-- KYC/UBO data masquerade as CBU state.
--
-- Domain-isolation rule (2026-06-22): CBU is a purely structural container that
-- knows nothing about KYC; KYC reads CBU (via the mandatory ManCo), never the
-- reverse. Renaming to the owning domain (ubo) removes the misleading framing.
--
-- The table rename auto-cascades into dependent views (cbu_convergence_status,
-- ubo_convergence_status, ubo_expired_proofs, ubo_missing_proofs, …) and into
-- inbound FK references; constraints/indexes are renamed for hygiene.

ALTER TABLE "ob-poc".cbu_relationship_verification
    RENAME TO ubo_relationship_verification;

ALTER TABLE "ob-poc".ubo_relationship_verification
    RENAME CONSTRAINT cbu_relationship_verification_pkey
        TO ubo_relationship_verification_pkey;
ALTER TABLE "ob-poc".ubo_relationship_verification
    RENAME CONSTRAINT cbu_relationship_verification_cbu_id_relationship_id_key
        TO ubo_relationship_verification_cbu_id_relationship_id_key;
ALTER TABLE "ob-poc".ubo_relationship_verification
    RENAME CONSTRAINT cbu_relationship_verification_cbu_id_fkey
        TO ubo_relationship_verification_cbu_id_fkey;
ALTER TABLE "ob-poc".ubo_relationship_verification
    RENAME CONSTRAINT cbu_relationship_verification_proof_document_id_fkey
        TO ubo_relationship_verification_proof_document_id_fkey;
ALTER TABLE "ob-poc".ubo_relationship_verification
    RENAME CONSTRAINT cbu_relationship_verification_relationship_id_fkey
        TO ubo_relationship_verification_relationship_id_fkey;
ALTER TABLE "ob-poc".ubo_relationship_verification
    RENAME CONSTRAINT chk_crv_status TO chk_urv_status;

-- PG15+ names NOT NULL constraints `<table>_<col>_not_null`; a table rename does
-- not propagate to them. Rename for a clean, name-consistent schema dump.
ALTER TABLE "ob-poc".ubo_relationship_verification
    RENAME CONSTRAINT cbu_relationship_verification_verification_id_not_null
        TO ubo_relationship_verification_verification_id_not_null;
ALTER TABLE "ob-poc".ubo_relationship_verification
    RENAME CONSTRAINT cbu_relationship_verification_cbu_id_not_null
        TO ubo_relationship_verification_cbu_id_not_null;
ALTER TABLE "ob-poc".ubo_relationship_verification
    RENAME CONSTRAINT cbu_relationship_verification_relationship_id_not_null
        TO ubo_relationship_verification_relationship_id_not_null;
ALTER TABLE "ob-poc".ubo_relationship_verification
    RENAME CONSTRAINT cbu_relationship_verification_status_not_null
        TO ubo_relationship_verification_status_not_null;

ALTER INDEX "ob-poc".idx_cbu_rel_verif_cbu RENAME TO idx_ubo_rel_verif_cbu;
ALTER INDEX "ob-poc".idx_cbu_rel_verif_rel RENAME TO idx_ubo_rel_verif_rel;
ALTER INDEX "ob-poc".idx_cbu_rel_verif_status RENAME TO idx_ubo_rel_verif_status;

-- COMMENT strings are stored as text and are NOT auto-updated by a table rename;
-- refresh the one dependent-view comment that named the old table.
COMMENT ON VIEW "ob-poc".ubo_convergence_status IS
    'Computed convergence status per CBU from ubo_relationship_verification';
