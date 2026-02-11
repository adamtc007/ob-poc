-- Migration 072: Booking Principals
-- Deterministic booking principal selection with boundary-aware rules.
-- Creates 13 new tables, ALTERs 2 existing tables, adds overlap trigger,
-- coverage views, and rule field dictionary seed data.

BEGIN;

-- ============================================================
-- 0. Extension required for temporal exclusion constraints
-- ============================================================
CREATE EXTENSION IF NOT EXISTS btree_gist;

-- ============================================================
-- 1. ALTER existing tables
-- ============================================================

-- Extend products for booking principal evaluation
ALTER TABLE "ob-poc".products
    ADD COLUMN IF NOT EXISTS product_family text,
    ADD COLUMN IF NOT EXISTS effective_from timestamptz DEFAULT now(),
    ADD COLUMN IF NOT EXISTS effective_to timestamptz;

-- Extend services for lifecycle tagging
ALTER TABLE "ob-poc".services
    ADD COLUMN IF NOT EXISTS lifecycle_tags text[] DEFAULT '{}';

-- ============================================================
-- 2. LEGAL ENTITY — BNY's own contracting legal entities
-- ============================================================
CREATE TABLE "ob-poc".legal_entity (
    legal_entity_id         uuid DEFAULT uuidv7() NOT NULL PRIMARY KEY,
    lei                     text UNIQUE,
    name                    text NOT NULL,
    incorporation_jurisdiction text NOT NULL,
    status                  text NOT NULL DEFAULT 'active'
                            CHECK (status IN ('active', 'inactive')),
    entity_id               uuid REFERENCES "ob-poc".entities(entity_id),
    metadata                jsonb DEFAULT '{}',
    created_at              timestamptz DEFAULT now() NOT NULL,
    updated_at              timestamptz DEFAULT now() NOT NULL
);

COMMENT ON TABLE "ob-poc".legal_entity IS
  'BNY legal entities that can sign contracts. Curated reference set, not the general entity universe.';

-- ============================================================
-- 3. BOOKING LOCATION — jurisdictional perimeter
-- ============================================================
CREATE TABLE "ob-poc".booking_location (
    booking_location_id     uuid DEFAULT uuidv7() NOT NULL PRIMARY KEY,
    country_code            text NOT NULL,
    region_code             text,
    regulatory_regime_tags  text[] DEFAULT '{}',
    jurisdiction_code       varchar(10) REFERENCES "ob-poc".master_jurisdictions(jurisdiction_code),
    metadata                jsonb DEFAULT '{}',
    created_at              timestamptz DEFAULT now() NOT NULL,
    updated_at              timestamptz DEFAULT now() NOT NULL
);

COMMENT ON TABLE "ob-poc".booking_location IS
  'Jurisdictional perimeter in which activity is booked/regulated. References master_jurisdictions.';

-- ============================================================
-- 4. BOOKING PRINCIPAL — first-class policy anchor
-- ============================================================
CREATE TABLE "ob-poc".booking_principal (
    booking_principal_id    uuid DEFAULT uuidv7() NOT NULL PRIMARY KEY,
    legal_entity_id         uuid NOT NULL REFERENCES "ob-poc".legal_entity(legal_entity_id),
    booking_location_id     uuid REFERENCES "ob-poc".booking_location(booking_location_id),
    principal_code          text NOT NULL UNIQUE,
    book_code               text,
    status                  text NOT NULL DEFAULT 'active'
                            CHECK (status IN ('active', 'inactive')),
    effective_from          timestamptz NOT NULL DEFAULT now(),
    effective_to            timestamptz,
    CHECK (effective_to IS NULL OR effective_to > effective_from),
    metadata                jsonb DEFAULT '{}',
    created_at              timestamptz DEFAULT now() NOT NULL,
    updated_at              timestamptz DEFAULT now() NOT NULL
);

COMMENT ON TABLE "ob-poc".booking_principal IS
  'Contracting + booking authority envelope: LegalEntity + optional BookingLocation. First-class policy anchor.';

-- ============================================================
-- 5. CLIENT PROFILE — immutable evaluation snapshot
-- ============================================================
CREATE TABLE "ob-poc".client_profile (
    client_profile_id       uuid DEFAULT uuidv7() NOT NULL PRIMARY KEY,
    client_group_id         uuid NOT NULL,
    as_of                   timestamptz NOT NULL DEFAULT now(),
    segment                 text NOT NULL,
    domicile_country        text NOT NULL,
    entity_types            text[] DEFAULT '{}',
    risk_flags              jsonb DEFAULT '{}',
    metadata                jsonb DEFAULT '{}',
    created_at              timestamptz DEFAULT now() NOT NULL
    -- No updated_at: snapshots are immutable once created
);

COMMENT ON TABLE "ob-poc".client_profile IS
  'Point-in-time evaluation snapshot of client facts. Immutable once created.';

-- ============================================================
-- 6. CLIENT CLASSIFICATION — normalised regulatory classifications
-- ============================================================
CREATE TABLE "ob-poc".client_classification (
    client_classification_id uuid DEFAULT uuidv7() NOT NULL PRIMARY KEY,
    client_profile_id       uuid NOT NULL REFERENCES "ob-poc".client_profile(client_profile_id),
    classification_scheme   text NOT NULL,
    classification_value    text NOT NULL,
    jurisdiction_scope      text,
    effective_from          timestamptz,
    effective_to            timestamptz,
    CHECK (effective_to IS NULL OR effective_to > effective_from),
    source                  text,
    metadata                jsonb DEFAULT '{}',
    created_at              timestamptz DEFAULT now() NOT NULL,
    UNIQUE (client_profile_id, classification_scheme, jurisdiction_scope)
);

-- ============================================================
-- 7. SERVICE AVAILABILITY — three-lane (regulatory/commercial/operational)
-- ============================================================
CREATE TABLE "ob-poc".service_availability (
    service_availability_id uuid DEFAULT uuidv7() NOT NULL PRIMARY KEY,
    booking_principal_id    uuid NOT NULL REFERENCES "ob-poc".booking_principal(booking_principal_id),
    service_id              uuid NOT NULL REFERENCES "ob-poc".services(service_id),

    -- Lane 1: Regulatory perimeter
    regulatory_status       text NOT NULL DEFAULT 'permitted'
                            CHECK (regulatory_status IN ('permitted', 'restricted', 'prohibited')),
    regulatory_constraints  jsonb DEFAULT '{}',

    -- Lane 2: Commercial posture
    commercial_status       text NOT NULL DEFAULT 'offered'
                            CHECK (commercial_status IN ('offered', 'conditional', 'not_offered')),
    commercial_constraints  jsonb DEFAULT '{}',

    -- Lane 3: Operational capability
    operational_status      text NOT NULL DEFAULT 'supported'
                            CHECK (operational_status IN ('supported', 'limited', 'not_supported')),
    delivery_model          text CHECK (delivery_model IN ('direct', 'sub_custodian', 'partner', 'internal_network')),
    operational_constraints jsonb DEFAULT '{}',

    effective_from          timestamptz NOT NULL DEFAULT now(),
    effective_to            timestamptz,
    CHECK (effective_to IS NULL OR effective_to > effective_from),
    metadata                jsonb DEFAULT '{}',
    created_at              timestamptz DEFAULT now() NOT NULL,
    updated_at              timestamptz DEFAULT now() NOT NULL,

    -- Temporal uniqueness: one record per principal x service per effective window
    EXCLUDE USING gist (
        booking_principal_id WITH =,
        service_id WITH =,
        tstzrange(effective_from, effective_to) WITH &&
    )
);

COMMENT ON TABLE "ob-poc".service_availability IS
  'Three-lane availability: regulatory/commercial/operational per booking_principal x service. Temporal exclusion enforced.';

-- ============================================================
-- 8. CLIENT-PRINCIPAL RELATIONSHIP
-- ============================================================
CREATE TABLE "ob-poc".client_principal_relationship (
    client_principal_relationship_id uuid DEFAULT uuidv7() NOT NULL PRIMARY KEY,
    client_group_id         uuid NOT NULL,
    booking_principal_id    uuid NOT NULL REFERENCES "ob-poc".booking_principal(booking_principal_id),
    product_offering_id     uuid NOT NULL REFERENCES "ob-poc".products(product_id),
    relationship_status     text NOT NULL DEFAULT 'active'
                            CHECK (relationship_status IN ('active', 'pending', 'terminated')),
    contract_ref            text,
    onboarded_at            timestamptz,
    effective_from          timestamptz NOT NULL DEFAULT now(),
    effective_to            timestamptz,
    CHECK (effective_to IS NULL OR effective_to > effective_from),
    metadata                jsonb DEFAULT '{}',
    created_at              timestamptz DEFAULT now() NOT NULL,
    updated_at              timestamptz DEFAULT now() NOT NULL
);

CREATE INDEX idx_cpr_client_status
    ON "ob-poc".client_principal_relationship(client_group_id, relationship_status);
CREATE INDEX idx_cpr_principal_status
    ON "ob-poc".client_principal_relationship(booking_principal_id, relationship_status);
CREATE UNIQUE INDEX idx_cpr_unique_active
    ON "ob-poc".client_principal_relationship(client_group_id, booking_principal_id, product_offering_id)
    WHERE relationship_status = 'active';

COMMENT ON TABLE "ob-poc".client_principal_relationship IS
  'Active client-principal-offering relationships. Partial unique index enforces one active per triple.';

-- ============================================================
-- 9. RULESET — boundary-owned policy container
-- ============================================================
CREATE TABLE "ob-poc".ruleset (
    ruleset_id              uuid DEFAULT uuidv7() NOT NULL PRIMARY KEY,
    owner_type              text NOT NULL CHECK (owner_type IN ('principal', 'offering', 'global')),
    owner_id                uuid,
    name                    text NOT NULL,
    ruleset_boundary        text NOT NULL CHECK (ruleset_boundary IN ('regulatory', 'commercial', 'operational')),
    version                 int NOT NULL DEFAULT 1,
    effective_from          timestamptz NOT NULL DEFAULT now(),
    effective_to            timestamptz,
    CHECK (effective_to IS NULL OR effective_to > effective_from),
    status                  text NOT NULL DEFAULT 'draft'
                            CHECK (status IN ('draft', 'active', 'retired')),
    metadata                jsonb DEFAULT '{}',
    created_at              timestamptz DEFAULT now() NOT NULL,
    updated_at              timestamptz DEFAULT now() NOT NULL
);

-- Temporal overlap prevention: same owner + same boundary -> no overlap
CREATE OR REPLACE FUNCTION "ob-poc".check_ruleset_overlap()
RETURNS TRIGGER AS $$
BEGIN
  IF EXISTS (
    SELECT 1 FROM "ob-poc".ruleset
    WHERE owner_type = NEW.owner_type
      AND owner_id IS NOT DISTINCT FROM NEW.owner_id
      AND ruleset_boundary = NEW.ruleset_boundary
      AND status = 'active'
      AND ruleset_id != NEW.ruleset_id
      AND tstzrange(effective_from, effective_to) &&
          tstzrange(NEW.effective_from, NEW.effective_to)
  ) THEN
    RAISE EXCEPTION 'Overlapping active ruleset for (%, %, %)',
      NEW.owner_type, NEW.owner_id, NEW.ruleset_boundary;
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_ruleset_no_overlap
  BEFORE INSERT OR UPDATE ON "ob-poc".ruleset
  FOR EACH ROW WHEN (NEW.status = 'active')
  EXECUTE FUNCTION "ob-poc".check_ruleset_overlap();

-- ============================================================
-- 10. RULE — conditions + effects within a ruleset
-- ============================================================
CREATE TABLE "ob-poc".rule (
    rule_id                 uuid DEFAULT uuidv7() NOT NULL PRIMARY KEY,
    ruleset_id              uuid NOT NULL REFERENCES "ob-poc".ruleset(ruleset_id),
    name                    text NOT NULL,
    kind                    text NOT NULL CHECK (kind IN (
        'deny', 'require_gate', 'allow', 'constrain_principal', 'select_contract'
    )),
    when_expr               jsonb NOT NULL,
    then_effect             jsonb NOT NULL,
    explain                 text,
    priority                int NOT NULL DEFAULT 100,
    metadata                jsonb DEFAULT '{}',
    created_at              timestamptz DEFAULT now() NOT NULL,
    updated_at              timestamptz DEFAULT now() NOT NULL
);

CREATE INDEX idx_rule_ruleset ON "ob-poc".rule(ruleset_id);

-- ============================================================
-- 11. RULE FIELD DICTIONARY — closed-world field registry
-- ============================================================
CREATE TABLE "ob-poc".rule_field_dictionary (
    field_key               text PRIMARY KEY,
    field_type              text NOT NULL CHECK (field_type IN (
        'string', 'string_array', 'boolean', 'number', 'date'
    )),
    description             text,
    source_table            text,
    added_in_version        int NOT NULL DEFAULT 1,
    created_at              timestamptz DEFAULT now() NOT NULL
);

COMMENT ON TABLE "ob-poc".rule_field_dictionary IS
  'Closed-world field dictionary for rule expression validation. Unknown fields fail at publish, not at eval.';

-- ============================================================
-- 12. CONTRACT PACK + TEMPLATE
-- ============================================================
CREATE TABLE "ob-poc".contract_pack (
    contract_pack_id        uuid DEFAULT uuidv7() NOT NULL PRIMARY KEY,
    code                    text NOT NULL UNIQUE,
    name                    text NOT NULL,
    description             text,
    effective_from          timestamptz NOT NULL DEFAULT now(),
    effective_to            timestamptz,
    CHECK (effective_to IS NULL OR effective_to > effective_from),
    metadata                jsonb DEFAULT '{}',
    created_at              timestamptz DEFAULT now() NOT NULL,
    updated_at              timestamptz DEFAULT now() NOT NULL
);

CREATE TABLE "ob-poc".contract_template (
    contract_template_id    uuid DEFAULT uuidv7() NOT NULL PRIMARY KEY,
    contract_pack_id        uuid NOT NULL REFERENCES "ob-poc".contract_pack(contract_pack_id),
    template_type           text NOT NULL,
    template_ref            text,
    metadata                jsonb DEFAULT '{}',
    created_at              timestamptz DEFAULT now() NOT NULL,
    updated_at              timestamptz DEFAULT now() NOT NULL
);

-- ============================================================
-- 13. ELIGIBILITY EVALUATION — immutable audit record
-- ============================================================
CREATE TABLE "ob-poc".eligibility_evaluation (
    eligibility_evaluation_id uuid DEFAULT uuidv7() NOT NULL PRIMARY KEY,
    client_profile_id       uuid NOT NULL REFERENCES "ob-poc".client_profile(client_profile_id),
    client_group_id         uuid NOT NULL,
    product_offering_ids    uuid[] NOT NULL,
    requested_at            timestamptz NOT NULL DEFAULT now(),
    requested_by            text NOT NULL,
    policy_snapshot         jsonb NOT NULL,
    evaluation_context      jsonb,
    result                  jsonb NOT NULL,
    explain                 jsonb NOT NULL,
    selected_principal_id   uuid,
    selected_at             timestamptz,
    runbook_entry_id        uuid
);

CREATE INDEX idx_eval_client ON "ob-poc".eligibility_evaluation(client_group_id, requested_at DESC);
CREATE INDEX idx_eval_runbook ON "ob-poc".eligibility_evaluation(runbook_entry_id)
    WHERE runbook_entry_id IS NOT NULL;

COMMENT ON TABLE "ob-poc".eligibility_evaluation IS
  'Immutable evaluation audit record. Policy pin is automatic — this record IS the pin.';

-- ============================================================
-- 14. SEED: Rule field dictionary
-- ============================================================
INSERT INTO "ob-poc".rule_field_dictionary (field_key, field_type, description, source_table) VALUES
-- Client profile fields
('client.segment',                       'string',       'Client segment',                    'client_profile.segment'),
('client.domicile_country',              'string',       'Client legal domicile',             'client_profile.domicile_country'),
('client.entity_types',                  'string_array', 'Entity type classifications',       'client_profile.entity_types'),
('client.classification.mifid_ii',       'string',       'MiFID II classification',           'client_classification'),
('client.classification.dodd_frank',     'string',       'Dodd-Frank classification',         'client_classification'),
('client.classification.ucits_aifmd',    'string',       'UCITS/AIFMD classification',        'client_classification'),
('client.classification.fatca',          'string',       'FATCA classification',              'client_classification'),
('client.classification.crs',           'string',       'CRS classification',                'client_classification'),
('client.risk_flags.sanctions',          'boolean',      'Sanctions flag',                    'client_profile.risk_flags'),
('client.risk_flags.pep',                'boolean',      'PEP flag',                          'client_profile.risk_flags'),
-- Offering fields
('offering.code',                        'string',       'Product offering code',             'products.product_code'),
('offering.product_family',              'string',       'Product family',                    'products.product_family'),
-- Principal fields
('principal.code',                       'string',       'Booking principal code',            'booking_principal.principal_code'),
('principal.location.country',           'string',       'Booking location country',          'booking_location.country_code'),
('principal.location.region',            'string',       'Booking location region',           'booking_location.region_code'),
('principal.location.regulatory_regimes','string_array', 'Regulatory regime tags',            'booking_location.regulatory_regime_tags'),
-- Service fields
('service.delivery_model',               'string',       'Service delivery model',            'service_availability.delivery_model'),
-- Relationship fields (computed at eval time)
('relationship.exists',                  'boolean',      'Active relationship exists',        'client_principal_relationship (computed)'),
('relationship.status',                  'string',       'Relationship status',               'client_principal_relationship.relationship_status'),
('relationship.offerings',               'string_array', 'Active offering codes',             'client_principal_relationship (computed)'),
-- Deal context fields (transient — from evaluation_context)
('deal.market_countries',                'string_array', 'Markets in scope',                  'evaluation_context.deal.market_countries'),
('deal.instrument_types',                'string_array', 'Instrument classes',                'evaluation_context.deal.instrument_types'),
('deal.trading_venues',                  'string_array', 'Trading venues',                    'evaluation_context.deal.trading_venues');

-- ============================================================
-- 15. COVERAGE REPORTING VIEWS
-- ============================================================

-- Regulatory gaps: no principal permitted for (offering, jurisdiction)
CREATE OR REPLACE VIEW "ob-poc".v_regulatory_gaps AS
SELECT p.product_code AS offering_code,
       bl.country_code AS jurisdiction,
       'regulatory' AS gap_type,
       'No principal permitted in this jurisdiction' AS detail
FROM "ob-poc".products p
CROSS JOIN "ob-poc".booking_location bl
WHERE p.is_active = true
  AND NOT EXISTS (
    SELECT 1
    FROM "ob-poc".booking_principal bp
    JOIN "ob-poc".service_availability sa ON sa.booking_principal_id = bp.booking_principal_id
    JOIN "ob-poc".product_services ps ON ps.service_id = sa.service_id
                                      AND ps.product_id = p.product_id
    WHERE bp.booking_location_id = bl.booking_location_id
      AND bp.status = 'active'
      AND sa.regulatory_status = 'permitted'
      AND now() BETWEEN sa.effective_from AND COALESCE(sa.effective_to, 'infinity'::timestamptz)
  );

-- Commercial gaps: permitted but not offered
CREATE OR REPLACE VIEW "ob-poc".v_commercial_gaps AS
SELECT p.product_code AS offering_code,
       bl.country_code AS jurisdiction,
       bp.principal_code,
       'commercial' AS gap_type,
       'Permitted but not commercially offered' AS detail
FROM "ob-poc".products p
JOIN "ob-poc".product_services ps ON ps.product_id = p.product_id
JOIN "ob-poc".service_availability sa ON sa.service_id = ps.service_id
JOIN "ob-poc".booking_principal bp ON bp.booking_principal_id = sa.booking_principal_id
JOIN "ob-poc".booking_location bl ON bl.booking_location_id = bp.booking_location_id
WHERE p.is_active = true
  AND bp.status = 'active'
  AND sa.regulatory_status = 'permitted'
  AND sa.commercial_status IN ('not_offered', 'conditional')
  AND now() BETWEEN sa.effective_from AND COALESCE(sa.effective_to, 'infinity'::timestamptz);

-- Operational gaps: permitted and offered but not deliverable
CREATE OR REPLACE VIEW "ob-poc".v_operational_gaps AS
SELECT p.product_code AS offering_code,
       bl.country_code AS jurisdiction,
       bp.principal_code,
       'operational' AS gap_type,
       'Offered but operationally ' || sa.operational_status AS detail,
       sa.delivery_model
FROM "ob-poc".products p
JOIN "ob-poc".product_services ps ON ps.product_id = p.product_id
JOIN "ob-poc".service_availability sa ON sa.service_id = ps.service_id
JOIN "ob-poc".booking_principal bp ON bp.booking_principal_id = sa.booking_principal_id
JOIN "ob-poc".booking_location bl ON bl.booking_location_id = bp.booking_location_id
WHERE p.is_active = true
  AND bp.status = 'active'
  AND sa.regulatory_status = 'permitted'
  AND sa.commercial_status IN ('offered', 'conditional')
  AND sa.operational_status IN ('not_supported', 'limited')
  AND now() BETWEEN sa.effective_from AND COALESCE(sa.effective_to, 'infinity'::timestamptz);

-- Offerings with no active rulesets
CREATE OR REPLACE VIEW "ob-poc".v_offerings_without_rules AS
SELECT p.product_id, p.product_code, p.name
FROM "ob-poc".products p
WHERE p.is_active = true
  AND NOT EXISTS (
    SELECT 1 FROM "ob-poc".ruleset rs
    WHERE rs.owner_type = 'offering'
      AND rs.owner_id = p.product_id
      AND rs.status = 'active'
      AND now() BETWEEN rs.effective_from AND COALESCE(rs.effective_to, 'infinity'::timestamptz)
  );

COMMIT;
