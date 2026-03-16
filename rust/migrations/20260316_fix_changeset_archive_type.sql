-- Phase 0.4: Fix archive table type mismatch
-- Source: P3-D C-1, P4 RISK-5
-- Problem: change_sets_archive.owner_id is UUID but source
-- sem_reg.changesets.owner_actor_id is TEXT. Archive INSERT fails.

-- Check if table exists before altering (sem_reg_authoring schema)
DO $$
BEGIN
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema = 'sem_reg_authoring'
      AND table_name = 'change_sets_archive'
      AND column_name = 'owner_id'
  ) THEN
    ALTER TABLE sem_reg_authoring.change_sets_archive
      ALTER COLUMN owner_id TYPE TEXT USING owner_id::TEXT;
    ALTER TABLE sem_reg_authoring.change_sets_archive
      RENAME COLUMN owner_id TO owner_actor_id;
  END IF;

  -- Also check sem_reg schema variant
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema = 'sem_reg'
      AND table_name = 'change_sets_archive'
      AND column_name = 'owner_id'
  ) THEN
    ALTER TABLE sem_reg.change_sets_archive
      ALTER COLUMN owner_id TYPE TEXT USING owner_id::TEXT;
    ALTER TABLE sem_reg.change_sets_archive
      RENAME COLUMN owner_id TO owner_actor_id;
  END IF;
END $$;
