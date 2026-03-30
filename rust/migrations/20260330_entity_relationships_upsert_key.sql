-- Unique partial index for ubo.add-ownership upsert (ON CONFLICT)
-- Only active relationships (effective_to IS NULL) are constrained.
-- Allows multiple historical relationships between the same entity pair.

CREATE UNIQUE INDEX IF NOT EXISTS entity_relationships_upsert_key
  ON "ob-poc".entity_relationships (from_entity_id, to_entity_id, relationship_type)
  WHERE effective_to IS NULL;
