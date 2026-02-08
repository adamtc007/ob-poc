-- 071b: Recreate views that were cascaded when orphan tables were dropped in 071.
--
-- Three views referenced by Rust code were dropped as collateral CASCADE damage:
--   1. v_cbu_service_gaps      — depended on onboarding_requests/onboarding_products (dropped)
--   2. v_cbu_unified_gaps      — depended on v_cbu_service_gaps
--   3. v_entity_regulatory_summary — depended on ob_ref.regulatory_tiers (dropped)
--
-- Fix: recreate each view without the dropped table references.

BEGIN;

--------------------------------------------------------------------------------
-- 1. v_cbu_service_gaps
--    Original had a UNION in the cbu_products CTE joining onboarding_requests
--    and onboarding_products (both dropped in 071). Remove that UNION branch;
--    keep only the direct cbus→products path.
--------------------------------------------------------------------------------
CREATE OR REPLACE VIEW "ob-poc".v_cbu_service_gaps AS
 WITH cbu_products AS (
         SELECT DISTINCT c.cbu_id,
            c.name AS cbu_name,
            p.product_id,
            p.product_code,
            p.name AS product_name
           FROM ("ob-poc".cbus c
             LEFT JOIN "ob-poc".products p ON ((p.product_id = c.product_id)))
          WHERE ((p.product_id IS NOT NULL) AND (p.is_active = true))
        ), required_resources AS (
         SELECT cp.cbu_id,
            cp.cbu_name,
            cp.product_code,
            cp.product_name,
            s.service_id,
            s.service_code,
            s.name AS service_name,
            ps.is_mandatory,
            srt.resource_id AS resource_type_id,
            srt.resource_code,
            srt.name AS resource_name,
            srt.provisioning_verb,
            srt.location_type,
            srt.per_market,
            srt.per_currency,
            srt.per_counterparty,
            COALESCE(src.is_required, true) AS is_required
           FROM ((((cbu_products cp
             JOIN "ob-poc".product_services ps ON ((ps.product_id = cp.product_id)))
             JOIN "ob-poc".services s ON (((s.service_id = ps.service_id) AND (s.is_active = true))))
             JOIN "ob-poc".service_resource_capabilities src ON (((src.service_id = s.service_id) AND (src.is_active = true))))
             JOIN "ob-poc".service_resource_types srt ON (((srt.resource_id = src.resource_id) AND (srt.is_active = true))))
          WHERE (COALESCE(src.is_required, true) = true)
        )
 SELECT cbu_id,
    cbu_name,
    product_code,
    product_name,
    service_code,
    service_name,
    is_mandatory,
    resource_code AS missing_resource_code,
    resource_name AS missing_resource_name,
    provisioning_verb,
    location_type,
    per_market,
    per_currency,
    per_counterparty
   FROM required_resources rr
  WHERE (NOT (EXISTS ( SELECT 1
           FROM "ob-poc".cbu_resource_instances cri
          WHERE ((cri.cbu_id = rr.cbu_id) AND (cri.resource_type_id = rr.resource_type_id) AND ((cri.status)::text = ANY (ARRAY[('PENDING'::character varying)::text, ('ACTIVE'::character varying)::text, ('PROVISIONED'::character varying)::text]))))))
  ORDER BY cbu_name, product_code, service_code, resource_code;

--------------------------------------------------------------------------------
-- 2. v_cbu_unified_gaps
--    UNION ALL of v_cbu_lifecycle_gaps (still exists) and v_cbu_service_gaps
--    (just recreated above). No structural changes needed.
--------------------------------------------------------------------------------
CREATE OR REPLACE VIEW "ob-poc".v_cbu_unified_gaps AS
 SELECT g.cbu_id,
    g.cbu_name,
    'LIFECYCLE'::text AS gap_source,
    g.instrument_class,
    g.market,
    g.counterparty_name,
    NULL::character varying AS product_code,
    g.lifecycle_code AS operation_code,
    g.lifecycle_name AS operation_name,
    g.missing_resource_code,
    g.missing_resource_name,
    g.provisioning_verb,
    g.location_type,
    g.per_market,
    g.per_currency,
    g.per_counterparty,
    g.is_mandatory AS is_required
   FROM "ob-poc".v_cbu_lifecycle_gaps g
UNION ALL
 SELECT g.cbu_id,
    g.cbu_name,
    'SERVICE'::text AS gap_source,
    NULL::character varying AS instrument_class,
    NULL::character varying AS market,
    NULL::character varying AS counterparty_name,
    g.product_code,
    g.service_code AS operation_code,
    g.service_name AS operation_name,
    g.missing_resource_code,
    g.missing_resource_name,
    g.provisioning_verb,
    g.location_type,
    g.per_market,
    g.per_currency,
    g.per_counterparty,
    g.is_mandatory AS is_required
   FROM "ob-poc".v_cbu_service_gaps g;

--------------------------------------------------------------------------------
-- 3. v_entity_regulatory_summary
--    Original joined ob_ref.regulatory_tiers (dropped in 071) for the
--    allows_simplified_dd column. Remove that join; derive the column as
--    false (conservative default — no simplified DD without tier data).
--------------------------------------------------------------------------------
CREATE OR REPLACE VIEW ob_kyc.v_entity_regulatory_summary AS
 SELECT e.entity_id,
    e.name AS entity_name,
    count(r.registration_id) AS registration_count,
    count(r.registration_id) FILTER (WHERE (r.registration_verified AND ((r.status)::text = 'ACTIVE'::text))) AS verified_count,
    false AS allows_simplified_dd,
    array_agg(DISTINCT r.regulator_code) FILTER (WHERE ((r.status)::text = 'ACTIVE'::text)) AS active_regulators,
    array_agg(DISTINCT r.regulator_code) FILTER (WHERE (r.registration_verified AND ((r.status)::text = 'ACTIVE'::text))) AS verified_regulators,
    max(r.verification_date) AS last_verified,
    min(r.verification_expires) FILTER (WHERE (r.verification_expires > CURRENT_DATE)) AS next_expiry
   FROM (("ob-poc".entities e
     LEFT JOIN ob_kyc.entity_regulatory_registrations r ON ((e.entity_id = r.entity_id)))
     LEFT JOIN ob_ref.regulators reg ON (((r.regulator_code)::text = (reg.regulator_code)::text)))
  GROUP BY e.entity_id, e.name;

COMMIT;
