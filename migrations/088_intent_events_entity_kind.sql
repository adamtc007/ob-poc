-- Migration 088: Add entity-kind fields to intent_events for entity-kind constrained verb selection.
-- These columns support the entity-kind filtering audit trail.

ALTER TABLE agent.intent_events
    ADD COLUMN IF NOT EXISTS dominant_entity_id UUID,
    ADD COLUMN IF NOT EXISTS dominant_entity_kind TEXT,
    ADD COLUMN IF NOT EXISTS entity_kind_filtered BOOLEAN NOT NULL DEFAULT FALSE;

-- Index for querying by dominant entity
CREATE INDEX IF NOT EXISTS idx_intent_events_dominant_entity
    ON agent.intent_events (dominant_entity_id)
    WHERE dominant_entity_id IS NOT NULL;
