-- SemReg seed/data repair for legacy snapshot payloads.
--
-- Fixes:
-- 1) Marks historical rows incorrectly left as `active` after being closed.
-- 2) Normalizes legacy membership_rule payload keys:
--      subject_fqn/subject_type/membership_type
--    -> target_fqn/target_type/membership_kind (+ node_fqn fallback)
--
-- This removes false negatives/warnings in context resolution for strict mode.

BEGIN;

-- Closed rows should never remain active.
UPDATE sem_reg.snapshots
SET status = 'deprecated'::sem_reg.snapshot_status
WHERE status = 'active'::sem_reg.snapshot_status
  AND effective_until IS NOT NULL;

-- Normalize membership_rule payload shape for both active and historical rows.
WITH normalized AS (
    SELECT
        snapshot_id,
        (
            definition ||
            jsonb_build_object(
                'node_fqn',
                COALESCE(
                    definition->>'node_fqn',
                    definition->>'taxonomy_node_fqn',
                    definition->>'taxonomy_fqn'
                ),
                'target_type',
                COALESCE(definition->>'target_type', definition->>'subject_type'),
                'target_fqn',
                COALESCE(definition->>'target_fqn', definition->>'subject_fqn'),
                'membership_kind',
                COALESCE(
                    definition->>'membership_kind',
                    CASE lower(COALESCE(definition->>'membership_type', ''))
                        WHEN 'excluded' THEN 'excluded'
                        WHEN 'conditional' THEN 'conditional'
                        WHEN 'inherited' THEN 'inherited'
                        ELSE 'direct'
                    END
                )
            )
        ) AS new_definition
    FROM sem_reg.snapshots
    WHERE object_type = 'membership_rule'::sem_reg.object_type
)
UPDATE sem_reg.snapshots s
SET definition = n.new_definition
FROM normalized n
WHERE s.snapshot_id = n.snapshot_id
  AND s.definition IS DISTINCT FROM n.new_definition;

-- Any membership rows still missing target fields are malformed; retire if active.
UPDATE sem_reg.snapshots
SET status = 'deprecated'::sem_reg.snapshot_status
WHERE object_type = 'membership_rule'::sem_reg.object_type
  AND status = 'active'::sem_reg.snapshot_status
  AND (
      COALESCE(definition->>'target_fqn', '') = ''
      OR COALESCE(definition->>'target_type', '') = ''
      OR COALESCE(definition->>'membership_kind', '') = ''
  );

COMMIT;
