-- 20260502_trading_profile_status_align_dag.sql
-- Aligns cbu_trading_profiles.status with instrument_matrix_dag trading_profile
-- lifecycle vocabulary.

UPDATE "ob-poc".cbu_trading_profiles
SET status = CASE status
    WHEN 'PENDING_REVIEW' THEN 'SUBMITTED'
    WHEN 'VALIDATED' THEN 'APPROVED'
    ELSE status
END
WHERE status IN ('PENDING_REVIEW', 'VALIDATED');

ALTER TABLE "ob-poc".cbu_trading_profiles
    DROP CONSTRAINT IF EXISTS cbu_trading_profiles_status_check;

ALTER TABLE "ob-poc".cbu_trading_profiles
    ADD CONSTRAINT cbu_trading_profiles_status_check CHECK (
        status IN (
            'DRAFT',
            'SUBMITTED',
            'APPROVED',
            'PARALLEL_RUN',
            'ACTIVE',
            'SUSPENDED',
            'REJECTED',
            'SUPERSEDED',
            'ARCHIVED'
        )
    );

DROP INDEX IF EXISTS "ob-poc".idx_trading_profiles_one_working_version;

CREATE UNIQUE INDEX idx_trading_profiles_one_working_version
    ON "ob-poc".cbu_trading_profiles USING btree (cbu_id)
    WHERE status IN ('DRAFT', 'SUBMITTED', 'APPROVED', 'PARALLEL_RUN');
