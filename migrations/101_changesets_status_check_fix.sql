-- 101_changesets_status_check_fix.sql
-- Allow both spellings during transition; canonical is under_review.
-- Forward-only: does not edit migration 095 or 099.
BEGIN;

ALTER TABLE sem_reg.changesets
  DROP CONSTRAINT IF EXISTS changesets_status_check;

ALTER TABLE sem_reg.changesets
  ADD CONSTRAINT changesets_status_check
  CHECK (status IN (
    'draft',
    'under_review',
    'in_review',
    'approved',
    'rejected',
    'published',
    'validated',
    'dry_run_passed',
    'dry_run_failed',
    'superseded'
  ));

COMMIT;
