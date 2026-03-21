ALTER TABLE "ob-poc".utterance_traces
    ADD COLUMN IF NOT EXISTS fallback_reason_code TEXT NULL;

CREATE INDEX IF NOT EXISTS idx_utterance_traces_fallback_reason_code
    ON "ob-poc".utterance_traces (fallback_reason_code)
    WHERE fallback_reason_code IS NOT NULL;
