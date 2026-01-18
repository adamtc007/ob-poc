-- ESPER Learned Aliases
-- Stores user-learned phrase→command mappings for navigation commands.
-- Integrates with the existing agent learning infrastructure.

CREATE TABLE IF NOT EXISTS agent.esper_aliases (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- The phrase that triggered this learning (normalized to lowercase)
    phrase TEXT NOT NULL,

    -- The command key this phrase maps to (e.g., "zoom_in", "scale_universe")
    command_key TEXT NOT NULL,

    -- How many times this phrase→command mapping was observed
    occurrence_count INT NOT NULL DEFAULT 1,

    -- Confidence score (0.00-1.00), increases with occurrences
    confidence DECIMAL(3,2) NOT NULL DEFAULT 0.50,

    -- Whether this alias has been auto-approved (after 3x threshold)
    auto_approved BOOLEAN NOT NULL DEFAULT FALSE,

    -- Source of the learning
    source TEXT NOT NULL DEFAULT 'user_correction',

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Each phrase can only map to one command
    UNIQUE(phrase, command_key)
);

-- Index for fast phrase lookup during warmup
CREATE INDEX IF NOT EXISTS idx_esper_aliases_phrase
    ON agent.esper_aliases(LOWER(phrase));

-- Index for loading approved aliases at startup
CREATE INDEX IF NOT EXISTS idx_esper_aliases_approved
    ON agent.esper_aliases(auto_approved)
    WHERE auto_approved = true;

-- Function to record/update an ESPER alias
CREATE OR REPLACE FUNCTION agent.upsert_esper_alias(
    p_phrase TEXT,
    p_command_key TEXT,
    p_source TEXT DEFAULT 'user_correction'
) RETURNS agent.esper_aliases AS $$
DECLARE
    v_result agent.esper_aliases;
    v_threshold INT := 3;  -- Auto-approve after 3 occurrences
BEGIN
    INSERT INTO agent.esper_aliases (phrase, command_key, source)
    VALUES (LOWER(TRIM(p_phrase)), p_command_key, p_source)
    ON CONFLICT (phrase, command_key) DO UPDATE SET
        occurrence_count = agent.esper_aliases.occurrence_count + 1,
        confidence = LEAST(1.0, agent.esper_aliases.confidence + 0.15),
        auto_approved = CASE
            WHEN agent.esper_aliases.occurrence_count + 1 >= v_threshold THEN TRUE
            ELSE agent.esper_aliases.auto_approved
        END,
        updated_at = NOW()
    RETURNING * INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

COMMENT ON TABLE agent.esper_aliases IS
    'Learned phrase→command mappings for ESPER navigation commands';
COMMENT ON COLUMN agent.esper_aliases.phrase IS
    'User phrase (normalized to lowercase)';
COMMENT ON COLUMN agent.esper_aliases.command_key IS
    'ESPER command key from config (e.g., zoom_in, scale_universe)';
COMMENT ON COLUMN agent.esper_aliases.auto_approved IS
    'True if alias passed 3x occurrence threshold';
