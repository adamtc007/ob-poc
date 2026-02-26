-- Migration 102: Fix archive table column type mismatch.
-- Migration 100 defined owner_id UUID but the live table has owner_actor_id TEXT.
-- Also add missing updated_at column to match live table.

BEGIN;

-- Fix owner column: rename and retype
ALTER TABLE sem_reg_authoring.change_sets_archive
  RENAME COLUMN owner_id TO owner_actor_id;

ALTER TABLE sem_reg_authoring.change_sets_archive
  ALTER COLUMN owner_actor_id TYPE TEXT USING owner_actor_id::text;

-- Add missing updated_at column (nullable since existing rows lack it)
ALTER TABLE sem_reg_authoring.change_sets_archive
  ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ;

COMMIT;
