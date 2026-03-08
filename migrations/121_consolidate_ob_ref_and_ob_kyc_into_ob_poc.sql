BEGIN;

-- Enrich canonical regulators table with ob_ref metadata.
ALTER TABLE "ob-poc".regulators
    ADD COLUMN IF NOT EXISTS regulator_type VARCHAR,
    ADD COLUMN IF NOT EXISTS active BOOLEAN NOT NULL DEFAULT TRUE;

INSERT INTO "ob-poc".regulators (
    regulator_code,
    name,
    jurisdiction,
    tier,
    regulator_type,
    registry_url,
    active,
    created_at,
    updated_at
)
SELECT
    src.regulator_code,
    src.regulator_name,
    src.jurisdiction,
    src.regulatory_tier,
    src.regulator_type,
    src.registry_url,
    COALESCE(src.active, TRUE),
    COALESCE(src.created_at AT TIME ZONE 'UTC', NOW()),
    COALESCE(src.updated_at AT TIME ZONE 'UTC', NOW())
FROM ob_ref.regulators src
ON CONFLICT (regulator_code) DO UPDATE
SET
    name = EXCLUDED.name,
    jurisdiction = EXCLUDED.jurisdiction,
    tier = EXCLUDED.tier,
    regulator_type = EXCLUDED.regulator_type,
    registry_url = EXCLUDED.registry_url,
    active = EXCLUDED.active,
    updated_at = GREATEST("ob-poc".regulators.updated_at, EXCLUDED.updated_at);

CREATE INDEX IF NOT EXISTS idx_regulators_jurisdiction
    ON "ob-poc".regulators (jurisdiction);

CREATE INDEX IF NOT EXISTS idx_regulators_tier
    ON "ob-poc".regulators (tier);

-- Enrich canonical role_types table with ob_ref metadata while preserving role_code.
ALTER TABLE "ob-poc".role_types
    ADD COLUMN IF NOT EXISTS category VARCHAR,
    ADD COLUMN IF NOT EXISTS active BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ;

INSERT INTO "ob-poc".role_types (
    role_code,
    name,
    description,
    category,
    triggers_full_kyc,
    triggers_screening,
    triggers_id_verification,
    check_regulatory_status,
    if_regulated_obligation,
    cascade_to_entity_ubos,
    threshold_based,
    active,
    created_at,
    updated_at
)
SELECT
    src.code,
    src.name,
    src.description,
    src.category,
    src.triggers_full_kyc,
    src.triggers_screening,
    src.triggers_id_verification,
    src.check_regulatory_status,
    src.if_regulated_obligation,
    src.cascade_to_entity_ubos,
    src.threshold_based,
    COALESCE(src.active, TRUE),
    COALESCE(src.created_at AT TIME ZONE 'UTC', NOW()),
    COALESCE(src.updated_at AT TIME ZONE 'UTC', NOW())
FROM ob_ref.role_types src
ON CONFLICT (role_code) DO UPDATE
SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    category = EXCLUDED.category,
    triggers_full_kyc = EXCLUDED.triggers_full_kyc,
    triggers_screening = EXCLUDED.triggers_screening,
    triggers_id_verification = EXCLUDED.triggers_id_verification,
    check_regulatory_status = EXCLUDED.check_regulatory_status,
    if_regulated_obligation = EXCLUDED.if_regulated_obligation,
    cascade_to_entity_ubos = EXCLUDED.cascade_to_entity_ubos,
    threshold_based = EXCLUDED.threshold_based,
    active = EXCLUDED.active,
    updated_at = EXCLUDED.updated_at;

CREATE INDEX IF NOT EXISTS idx_role_types_category
    ON "ob-poc".role_types (category);

-- Move non-colliding ob_ref tables into the business schema.
ALTER TABLE ob_ref.request_types SET SCHEMA "ob-poc";
ALTER TABLE ob_ref.tollgate_definitions SET SCHEMA "ob-poc";
ALTER TABLE ob_ref.standards_mappings SET SCHEMA "ob-poc";
ALTER SEQUENCE IF EXISTS ob_ref.standards_mappings_mapping_id_seq SET SCHEMA "ob-poc";
ALTER TABLE "ob-poc".standards_mappings
    ALTER COLUMN mapping_id SET DEFAULT nextval('"ob-poc".standards_mappings_mapping_id_seq'::regclass);

-- Repoint tollgate_evaluations to the moved tollgate definitions.
ALTER TABLE "ob-poc".tollgate_evaluations
    DROP CONSTRAINT IF EXISTS tollgate_evaluations_tollgate_id_fkey,
    ADD CONSTRAINT tollgate_evaluations_tollgate_id_fkey
        FOREIGN KEY (tollgate_id) REFERENCES "ob-poc".tollgate_definitions(tollgate_id);

-- Move regulatory registrations and repoint them to canonical regulators.
DROP VIEW IF EXISTS ob_kyc.v_entity_regulatory_summary;
DROP FUNCTION IF EXISTS ob_kyc.entity_allows_simplified_dd(UUID);

ALTER TABLE ob_kyc.entity_regulatory_registrations
    DROP CONSTRAINT IF EXISTS entity_regulatory_registrations_regulator_code_fkey,
    DROP CONSTRAINT IF EXISTS entity_regulatory_registrations_home_regulator_code_fkey;

ALTER TABLE ob_kyc.entity_regulatory_registrations SET SCHEMA "ob-poc";

ALTER TABLE "ob-poc".entity_regulatory_registrations
    ADD CONSTRAINT entity_regulatory_registrations_regulator_code_fkey
        FOREIGN KEY (regulator_code) REFERENCES "ob-poc".regulators(regulator_code),
    ADD CONSTRAINT entity_regulatory_registrations_home_regulator_code_fkey
        FOREIGN KEY (home_regulator_code) REFERENCES "ob-poc".regulators(regulator_code);

CREATE OR REPLACE VIEW "ob-poc".v_entity_regulatory_summary AS
SELECT
    e.entity_id,
    e.name AS entity_name,
    COUNT(r.registration_id) AS registration_count,
    COUNT(r.registration_id) FILTER (
        WHERE r.registration_verified AND r.status = 'ACTIVE'
    ) AS verified_count,
    EXISTS (
        SELECT 1
        FROM "ob-poc".entity_regulatory_registrations verified
        JOIN "ob-poc".regulators reg
            ON verified.regulator_code = reg.regulator_code
        WHERE verified.entity_id = e.entity_id
          AND verified.status = 'ACTIVE'
          AND verified.registration_verified = TRUE
          AND reg.tier IN ('EQUIVALENT', 'ACCEPTABLE')
          AND COALESCE(reg.active, TRUE)
    ) AS allows_simplified_dd,
    ARRAY_AGG(DISTINCT r.regulator_code) FILTER (
        WHERE r.status = 'ACTIVE'
    ) AS active_regulators,
    ARRAY_AGG(DISTINCT r.regulator_code) FILTER (
        WHERE r.registration_verified AND r.status = 'ACTIVE'
    ) AS verified_regulators,
    MAX(r.verification_date) AS last_verified,
    MIN(r.verification_expires) FILTER (
        WHERE r.verification_expires > CURRENT_DATE
    ) AS next_expiry
FROM "ob-poc".entities e
LEFT JOIN "ob-poc".entity_regulatory_registrations r
    ON e.entity_id = r.entity_id
LEFT JOIN "ob-poc".regulators reg
    ON r.regulator_code = reg.regulator_code
GROUP BY e.entity_id, e.name;

CREATE OR REPLACE FUNCTION "ob-poc".entity_allows_simplified_dd(p_entity_id UUID)
RETURNS BOOLEAN
LANGUAGE plpgsql
STABLE
AS $function$
BEGIN
    RETURN EXISTS (
        SELECT 1
        FROM "ob-poc".entity_regulatory_registrations r
        JOIN "ob-poc".regulators reg ON r.regulator_code = reg.regulator_code
        WHERE r.entity_id = p_entity_id
          AND r.status = 'ACTIVE'
          AND r.registration_verified = TRUE
          AND reg.tier IN ('EQUIVALENT', 'ACCEPTABLE')
          AND COALESCE(reg.active, TRUE)
    );
END;
$function$;

-- Retire the duplicated ob_ref tables now that dependencies are rewired.
DROP TABLE ob_ref.regulators;
DROP TABLE ob_ref.role_types;

DROP SCHEMA ob_ref;
DROP SCHEMA ob_kyc;

COMMIT;
