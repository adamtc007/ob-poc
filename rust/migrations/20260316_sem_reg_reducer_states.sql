CREATE TABLE IF NOT EXISTS sem_reg.reducer_states (
    reducer_state_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type TEXT NOT NULL,
    entity_id UUID NOT NULL,
    current_state TEXT NOT NULL,
    lane TEXT,
    phase TEXT,
    computed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (entity_type, entity_id)
);

CREATE INDEX IF NOT EXISTS idx_reducer_states_entity
    ON sem_reg.reducer_states (entity_type, entity_id);
