-- EOP-PLAN share-register board design pass (2026-08-19): share-class.create's
-- YAML has always declared conflict_keys: [cbu_id, name] for its ON CONFLICT
-- upsert, but share_classes only ever had a UNIQUE (cbu_id, isin) constraint --
-- not (cbu_id, name). Verified directly: the CRUD executor's real
-- "INSERT ... ON CONFLICT (cbu_id, name) DO UPDATE ..." statement errors with
-- "no unique or exclusion constraint matching the ON CONFLICT specification"
-- on every call, not just real conflicts. isin is often absent for
-- non-listed/private share classes, so name is the right natural key here;
-- the DB just never got the constraint the verb has always assumed existed.

ALTER TABLE "ob-poc".share_classes
    ADD CONSTRAINT share_classes_cbu_id_name_key UNIQUE (cbu_id, name);
