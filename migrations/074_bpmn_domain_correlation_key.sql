-- Migration 074: Add domain_correlation_key to bpmn_correlations
--
-- The domain_correlation_key stores the primary business identifier (e.g.,
-- case_id) that links a BPMN process instance to domain entities. This
-- enables lifecycle signal verbs (request.remind, request.cancel, etc.) to
-- discover active BPMN correlations for a given case and route signals
-- through the BPMN engine alongside legacy outstanding_requests updates.
--
-- The key is extracted from the DurableConfig.correlation_field at dispatch
-- time and stored as text for generic lookup.

ALTER TABLE "ob-poc".bpmn_correlations
    ADD COLUMN IF NOT EXISTS domain_correlation_key TEXT;

-- Lookup by domain key + process_key for lifecycle signal routing.
-- Only active correlations are interesting for signal routing.
CREATE INDEX IF NOT EXISTS idx_bpmn_corr_domain_key
    ON "ob-poc".bpmn_correlations(process_key, domain_correlation_key)
    WHERE status = 'active' AND domain_correlation_key IS NOT NULL;
