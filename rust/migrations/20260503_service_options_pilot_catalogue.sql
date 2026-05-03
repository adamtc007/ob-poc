-- Service Options v0.2 pilot catalogue content.
--
-- Scope: Custody and Fund Accounting only. This seed deliberately keeps
-- unknown SME content as required/no-default options so runtime validation
-- produces explicit gaps rather than fabricated business knowledge.

BEGIN;

-- Custody SETTLEMENT is the worked example pilot.
UPDATE "ob-poc".product_services ps
SET is_mandatory = true,
    is_default = true
FROM "ob-poc".products p,
     "ob-poc".services s
WHERE ps.product_id = p.product_id
  AND ps.service_id = s.service_id
  AND p.product_code = 'CUSTODY'
  AND s.service_code = 'SETTLEMENT';

WITH service_ctx AS (
    SELECT s.service_id, sv.id AS service_version_id
    FROM "ob-poc".services s
    JOIN "ob-poc".service_versions sv
      ON sv.service_id = s.service_id
     AND sv.lifecycle_status = 'published'
    WHERE s.service_code = 'SETTLEMENT'
    ORDER BY sv.published_at DESC NULLS LAST, sv.created_at DESC
    LIMIT 1
)
INSERT INTO "ob-poc".service_option_defs
    (service_id, service_version_id, option_key, option_kind,
     allowed_values, default_value, is_required, is_fanout_driver,
     fanout_axis, default_source_kind, source_path, fallback_policy,
     override_policy, lifecycle_status, description)
SELECT service_id, service_version_id, option_key, option_kind,
       allowed_values, default_value, is_required, is_fanout_driver,
       fanout_axis, default_source_kind, source_path, fallback_policy,
       'allowed_with_reason', 'active', description
FROM service_ctx
CROSS JOIN (VALUES
    ('markets', 'multi_choice',
     '["US_EQUITY", "EU_EQUITY", "APAC_EQUITY"]'::jsonb,
     '["US_EQUITY", "EU_EQUITY"]'::jsonb,
     true, true, 'market', 'cbu_profile', 'markets',
     '["manual"]'::jsonb,
     'Markets in scope for settlement resource eligibility and fan-out.'),
    ('settlement_speed', 'single_choice',
     '["T0", "T1", "T2"]'::jsonb,
     '"T2"'::jsonb,
     true, false, 'none', 'instrument_matrix', 'preferred_speed',
     '["manual"]'::jsonb,
     'Preferred settlement speed sourced from attached instrument matrix.'),
    ('default_counterparties', 'multi_choice',
     NULL::jsonb,
     '[]'::jsonb,
     false, true, 'counterparty', 'instrument_matrix', 'counterparties',
     '["manual"]'::jsonb,
     'Default settlement counterparties sourced from attached instrument matrix.')
) AS seed(option_key, option_kind, allowed_values, default_value, is_required,
          is_fanout_driver, fanout_axis, default_source_kind, source_path,
          fallback_policy, description)
ON CONFLICT (service_version_id, option_key)
DO UPDATE SET option_kind = EXCLUDED.option_kind,
              allowed_values = EXCLUDED.allowed_values,
              default_value = EXCLUDED.default_value,
              is_required = EXCLUDED.is_required,
              is_fanout_driver = EXCLUDED.is_fanout_driver,
              fanout_axis = EXCLUDED.fanout_axis,
              default_source_kind = EXCLUDED.default_source_kind,
              source_path = EXCLUDED.source_path,
              fallback_policy = EXCLUDED.fallback_policy,
              lifecycle_status = EXCLUDED.lifecycle_status,
              description = EXCLUDED.description,
              updated_at = now();

WITH ids AS (
    SELECT s.service_id,
           s.service_code,
           od.service_option_def_id,
           od.option_key,
           r.resource_id,
           r.resource_code
    FROM "ob-poc".services s
    JOIN "ob-poc".service_option_defs od ON od.service_id = s.service_id
    JOIN "ob-poc".service_resource_types r
      ON r.resource_code IN ('DTCC_SETTLE', 'EUROCLEAR', 'APAC_CLEAR', 'SWIFT_CONN', 'SETTLE_ACCT')
    WHERE s.service_code = 'SETTLEMENT'
)
INSERT INTO "ob-poc".service_resource_option_constraints
    (service_id, resource_id, service_option_def_id, supported_values,
     match_operator, priority, is_required_for_coverage, is_active)
SELECT service_id, resource_id, service_option_def_id, supported_values,
       'intersect', priority, true, true
FROM ids
JOIN (VALUES
    ('markets', 'DTCC_SETTLE', '{"markets": ["US_EQUITY"]}'::jsonb, 10),
    ('markets', 'EUROCLEAR', '{"markets": ["EU_EQUITY"]}'::jsonb, 20),
    ('markets', 'APAC_CLEAR', '{"markets": ["APAC_EQUITY"]}'::jsonb, 30),
    ('default_counterparties', 'SWIFT_CONN', '{"counterparties": ["*"]}'::jsonb, 40),
    ('markets', 'SETTLE_ACCT', '{"markets": ["US_EQUITY", "EU_EQUITY", "APAC_EQUITY"]}'::jsonb, 50)
) AS seed(option_key, resource_code, supported_values, priority)
USING (option_key, resource_code)
ON CONFLICT (service_id, resource_id, service_option_def_id, match_operator)
WHERE is_active = true
DO UPDATE SET supported_values = EXCLUDED.supported_values,
              priority = EXCLUDED.priority,
              is_required_for_coverage = EXCLUDED.is_required_for_coverage,
              updated_at = now();

WITH ids AS (
    SELECT s.service_id,
           s.service_code,
           od.service_option_def_id,
           od.option_key,
           r.resource_id,
           r.resource_code
    FROM "ob-poc".services s
    JOIN "ob-poc".service_option_defs od ON od.service_id = s.service_id
    JOIN "ob-poc".service_resource_types r
      ON r.resource_code IN ('DTCC_SETTLE', 'EUROCLEAR', 'APAC_CLEAR', 'SWIFT_CONN', 'SETTLE_ACCT')
    WHERE s.service_code = 'SETTLEMENT'
)
INSERT INTO "ob-poc".service_resource_fanout_rules
    (service_id, resource_id, service_option_def_id, fanout_axis,
     fanout_mode, group_by_policy, shared_when_null, priority, is_active)
SELECT ids.service_id, ids.resource_id, ids.service_option_def_id,
       seed.fanout_axis, seed.fanout_mode, '{}'::jsonb, true, seed.priority, true
FROM ids
JOIN (VALUES
    ('markets', 'DTCC_SETTLE', 'market', 'per_value', 10),
    ('markets', 'EUROCLEAR', 'market', 'per_value', 20),
    ('markets', 'APAC_CLEAR', 'market', 'per_value', 30),
    ('default_counterparties', 'SWIFT_CONN', 'counterparty', 'per_value', 40),
    ('markets', 'SETTLE_ACCT', 'none', 'shared', 50)
) AS seed(option_key, resource_code, fanout_axis, fanout_mode, priority)
USING (option_key, resource_code)
ON CONFLICT DO NOTHING;

-- Fund Accounting pilot. Required fields without defensible source data are
-- intentionally left without defaults so cbu.validate-option-coverage reports
-- named gaps.
WITH service_ctx AS (
    SELECT s.service_id, sv.id AS service_version_id, s.service_code
    FROM "ob-poc".services s
    JOIN "ob-poc".service_versions sv
      ON sv.service_id = s.service_id
     AND sv.lifecycle_status = 'published'
    WHERE s.service_code IN ('NAV_CALC', 'FUND_REPORTING', 'ASSET_PRICING')
)
INSERT INTO "ob-poc".service_option_defs
    (service_id, service_version_id, option_key, option_kind,
     allowed_values, default_value, is_required, is_fanout_driver,
     fanout_axis, default_source_kind, source_path, fallback_policy,
     override_policy, lifecycle_status, description)
SELECT service_id, service_version_id, option_key, option_kind,
       allowed_values, default_value, is_required, is_fanout_driver,
       fanout_axis, default_source_kind, source_path, fallback_policy,
       'allowed_with_reason', 'active', description
FROM service_ctx
JOIN (VALUES
    ('NAV_CALC', 'frequency', 'single_choice',
     '["daily", "weekly", "monthly"]'::jsonb, '"daily"'::jsonb,
     true, false, 'none', 'cbu_profile', 'nav.frequency', '["manual"]'::jsonb,
     'NAV calculation frequency.'),
    ('NAV_CALC', 'pricing_source_priority', 'multi_choice',
     '["BLOOMBERG_BVAL", "ICE_PRICING", "MARKIT_PRICING", "REFINITIV_FEED"]'::jsonb,
     '["BLOOMBERG_BVAL", "ICE_PRICING"]'::jsonb,
     true, false, 'none', 'manual', NULL, '[]'::jsonb,
     'Ordered pricing source preference for NAV calculation.'),
    ('NAV_CALC', 'valuation_cutoff', 'string',
     NULL::jsonb, '"17:00"'::jsonb,
     true, false, 'none', 'manual', NULL, '[]'::jsonb,
     'Valuation cutoff time.'),
    ('NAV_CALC', 'share_classes', 'multi_choice',
     NULL::jsonb, NULL::jsonb,
     true, true, 'share_class', 'cbu_profile', 'fund.share_classes', '["manual"]'::jsonb,
     'Share classes in scope for NAV calculation; requires CBU/fund structure data.'),
    ('FUND_REPORTING', 'reporting_frequency', 'single_choice',
     '["daily", "weekly", "monthly", "quarterly"]'::jsonb, '"monthly"'::jsonb,
     true, false, 'none', 'manual', NULL, '[]'::jsonb,
     'Fund reporting frequency.'),
    ('FUND_REPORTING', 'report_types', 'multi_choice',
     '["NAV", "HOLDINGS", "PERFORMANCE", "REGULATORY"]'::jsonb,
     '["NAV", "HOLDINGS"]'::jsonb,
     true, false, 'none', 'manual', NULL, '[]'::jsonb,
     'Report types required for the fund accounting mandate.'),
    ('FUND_REPORTING', 'delivery_format', 'single_choice',
     '["PDF", "Excel", "XML", "JSON"]'::jsonb, '"PDF"'::jsonb,
     true, false, 'none', 'manual', NULL, '[]'::jsonb,
     'Primary report delivery format.'),
    ('FUND_REPORTING', 'recipient_groups', 'multi_choice',
     NULL::jsonb, NULL::jsonb,
     true, false, 'none', 'cbu_profile', 'reporting.recipient_groups', '["manual"]'::jsonb,
     'Recipient groups are mandate-specific and must be supplied.'),
    ('ASSET_PRICING', 'pricing_sources', 'multi_choice',
     '["BLOOMBERG_BVAL", "ICE_PRICING", "MARKIT_PRICING", "REFINITIV_FEED"]'::jsonb,
     '["BLOOMBERG_BVAL", "ICE_PRICING"]'::jsonb,
     true, false, 'none', 'manual', NULL, '[]'::jsonb,
     'Permitted pricing source set for the CBU.'),
    ('ASSET_PRICING', 'asset_classes', 'multi_choice',
     NULL::jsonb, NULL::jsonb,
     true, true, 'fund', 'cbu_profile', 'fund.asset_classes', '["manual"]'::jsonb,
     'Asset classes requiring pricing coverage; left as an explicit validation gap until mandate data is supplied.')
) AS seed(service_code, option_key, option_kind, allowed_values, default_value,
          is_required, is_fanout_driver, fanout_axis, default_source_kind,
          source_path, fallback_policy, description)
USING (service_code)
ON CONFLICT (service_version_id, option_key)
DO UPDATE SET option_kind = EXCLUDED.option_kind,
              allowed_values = EXCLUDED.allowed_values,
              default_value = EXCLUDED.default_value,
              is_required = EXCLUDED.is_required,
              is_fanout_driver = EXCLUDED.is_fanout_driver,
              fanout_axis = EXCLUDED.fanout_axis,
              default_source_kind = EXCLUDED.default_source_kind,
              source_path = EXCLUDED.source_path,
              fallback_policy = EXCLUDED.fallback_policy,
              lifecycle_status = EXCLUDED.lifecycle_status,
              description = EXCLUDED.description,
              updated_at = now();

WITH ids AS (
    SELECT s.service_id,
           s.service_code,
           od.service_option_def_id,
           od.option_key,
           r.resource_id,
           r.resource_code
    FROM "ob-poc".services s
    JOIN "ob-poc".service_option_defs od ON od.service_id = s.service_id
    JOIN "ob-poc".service_resource_types r
      ON r.resource_code IN ('NAV_ENGINE', 'REPORTING_HUB', 'BLOOMBERG_BVAL',
                             'ICE_PRICING', 'MARKIT_PRICING', 'REFINITIV_FEED')
    WHERE s.service_code IN ('NAV_CALC', 'FUND_REPORTING', 'ASSET_PRICING')
)
INSERT INTO "ob-poc".service_resource_fanout_rules
    (service_id, resource_id, service_option_def_id, fanout_axis,
     fanout_mode, group_by_policy, shared_when_null, priority, is_active)
SELECT ids.service_id, ids.resource_id, ids.service_option_def_id,
       seed.fanout_axis, seed.fanout_mode, '{}'::jsonb, true, seed.priority, true
FROM ids
JOIN (VALUES
    ('NAV_CALC', 'share_classes', 'NAV_ENGINE', 'share_class', 'per_value', 10),
    ('FUND_REPORTING', 'report_types', 'REPORTING_HUB', 'none', 'shared', 10),
    ('ASSET_PRICING', 'asset_classes', 'BLOOMBERG_BVAL', 'fund', 'per_value', 10),
    ('ASSET_PRICING', 'asset_classes', 'ICE_PRICING', 'fund', 'per_value', 20),
    ('ASSET_PRICING', 'asset_classes', 'MARKIT_PRICING', 'fund', 'per_value', 30),
    ('ASSET_PRICING', 'asset_classes', 'REFINITIV_FEED', 'fund', 'per_value', 40)
) AS seed(service_code, option_key, resource_code, fanout_axis, fanout_mode, priority)
  ON seed.service_code = ids.service_code
 AND seed.option_key = ids.option_key
 AND seed.resource_code = ids.resource_code
ON CONFLICT DO NOTHING;

COMMIT;
