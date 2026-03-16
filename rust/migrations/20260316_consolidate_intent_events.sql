-- Phase 0.5: Resolve duplicate intent_events tables
-- Source: P3-C CF-1
-- Problem: intent_events exists in both ob-poc and agent schema.
-- agent.intent_events is canonical (used by telemetry store).

-- Migrate any stray rows from ob-poc to agent (idempotent)
DO $$
BEGIN
  IF EXISTS (
    SELECT 1 FROM information_schema.tables
    WHERE table_schema = 'ob-poc' AND table_name = 'intent_events'
  ) THEN
    -- Only migrate if agent.intent_events also exists
    IF EXISTS (
      SELECT 1 FROM information_schema.tables
      WHERE table_schema = 'agent' AND table_name = 'intent_events'
    ) THEN
      -- Insert any rows from ob-poc that don't exist in agent
      INSERT INTO agent.intent_events
      SELECT * FROM "ob-poc".intent_events
      ON CONFLICT DO NOTHING;
    END IF;

    -- Drop the ob-poc duplicate
    DROP TABLE "ob-poc".intent_events;
  END IF;
END $$;

-- Document canonical location
DO $$
BEGIN
  IF EXISTS (
    SELECT 1 FROM information_schema.tables
    WHERE table_schema = 'agent' AND table_name = 'intent_events'
  ) THEN
    COMMENT ON TABLE agent.intent_events IS
      'Canonical intent telemetry table. Duplicate in ob-poc schema removed in migration 20260316_consolidate_intent_events.';
  END IF;
END $$;
