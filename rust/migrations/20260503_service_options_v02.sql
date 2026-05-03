-- Service Options v0.2 schema foundation.
--
-- Adds the design-time and runtime carriers for the approved Custody/Fund
-- Accounting service-options framework. This migration intentionally does not
-- wire verbs, DAG slots, or UI surfaces; it only establishes compatibility-safe
-- storage for later phases.

BEGIN;

-- ---------------------------------------------------------------------------
-- Design-time predicates for conditional product/service/option applicability.
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS "ob-poc".product_service_conditions (
    condition_id uuid PRIMARY KEY DEFAULT uuidv7(),
    condition_key text NOT NULL UNIQUE,
    description text,
    predicate jsonb NOT NULL,
    predicate_dsl text,
    lifecycle_status text NOT NULL DEFAULT 'active',
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT product_service_conditions_lifecycle_status_check
        CHECK (lifecycle_status IN ('draft', 'active', 'deprecated', 'retired'))
);

COMMENT ON TABLE "ob-poc".product_service_conditions IS
    'Structured predicates for conditional product-service and option applicability. predicate_dsl is governance/readability text; predicate JSONB is execution input.';

-- ---------------------------------------------------------------------------
-- Service option definitions, scoped to a service version.
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS "ob-poc".service_option_defs (
    service_option_def_id uuid PRIMARY KEY DEFAULT uuidv7(),
    service_id uuid NOT NULL REFERENCES "ob-poc".services(service_id) ON DELETE CASCADE,
    service_version_id uuid NOT NULL REFERENCES "ob-poc".service_versions(id) ON DELETE CASCADE,
    option_key text NOT NULL,
    option_kind text NOT NULL,
    allowed_values jsonb,
    default_value jsonb,
    is_required boolean NOT NULL DEFAULT false,
    is_fanout_driver boolean NOT NULL DEFAULT false,
    fanout_axis text NOT NULL DEFAULT 'none',
    default_source_kind text NOT NULL,
    source_path text,
    fallback_policy jsonb NOT NULL DEFAULT '[]'::jsonb,
    override_policy text NOT NULL DEFAULT 'allowed_with_reason',
    lifecycle_status text NOT NULL DEFAULT 'drafted',
    description text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT service_option_defs_option_kind_check
        CHECK (option_kind IN ('single_choice', 'multi_choice', 'range', 'boolean', 'structured', 'string')),
    CONSTRAINT service_option_defs_fanout_axis_check
        CHECK (fanout_axis IN ('none', 'market', 'currency', 'counterparty', 'account', 'fund', 'share_class', 'legal_entity', 'instruction_channel', 'jurisdiction', 'booking_principal')),
    CONSTRAINT service_option_defs_default_source_kind_check
        CHECK (default_source_kind IN ('derived', 'cbu_profile', 'instrument_matrix', 'legal_entity', 'document', 'product_option', 'manual', 'option_binding')),
    CONSTRAINT service_option_defs_override_policy_check
        CHECK (override_policy IN ('forbidden', 'allowed_with_reason', 'allowed', 'requires_approval')),
    CONSTRAINT service_option_defs_lifecycle_status_check
        CHECK (lifecycle_status IN ('drafted', 'active', 'deprecated', 'retired')),
    CONSTRAINT service_option_defs_fanout_driver_axis_check
        CHECK (is_fanout_driver OR fanout_axis = 'none'),
    CONSTRAINT service_option_defs_fallback_policy_array_check
        CHECK (jsonb_typeof(fallback_policy) = 'array'),
    UNIQUE (service_version_id, option_key)
);

CREATE INDEX IF NOT EXISTS idx_service_option_defs_service
    ON "ob-poc".service_option_defs(service_id);

CREATE INDEX IF NOT EXISTS idx_service_option_defs_version_status
    ON "ob-poc".service_option_defs(service_version_id, lifecycle_status);

COMMENT ON TABLE "ob-poc".service_option_defs IS
    'Design-time service option declarations. Each option is scoped to a service version so historical activation replay remains deterministic.';

-- ---------------------------------------------------------------------------
-- Product-service option overrides.
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS "ob-poc".product_service_option_overrides (
    override_id uuid PRIMARY KEY DEFAULT uuidv7(),
    product_id uuid NOT NULL REFERENCES "ob-poc".products(product_id) ON DELETE CASCADE,
    service_id uuid NOT NULL REFERENCES "ob-poc".services(service_id) ON DELETE CASCADE,
    service_option_def_id uuid NOT NULL REFERENCES "ob-poc".service_option_defs(service_option_def_id) ON DELETE CASCADE,
    default_value_override jsonb,
    allowed_values_override jsonb,
    is_required_override boolean,
    source_precedence_override jsonb,
    activation_condition_ref uuid REFERENCES "ob-poc".product_service_conditions(condition_id),
    effective_from timestamptz NOT NULL DEFAULT now(),
    effective_to timestamptz,
    supersedes_override_id uuid REFERENCES "ob-poc".product_service_option_overrides(override_id),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT product_service_option_overrides_effective_range_check
        CHECK (effective_to IS NULL OR effective_to > effective_from),
    CONSTRAINT product_service_option_overrides_not_self_superseded_check
        CHECK (supersedes_override_id IS NULL OR supersedes_override_id <> override_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_product_service_option_overrides_current
    ON "ob-poc".product_service_option_overrides(product_id, service_id, service_option_def_id)
    WHERE effective_to IS NULL;

CREATE INDEX IF NOT EXISTS idx_product_service_option_overrides_condition
    ON "ob-poc".product_service_option_overrides(activation_condition_ref)
    WHERE activation_condition_ref IS NOT NULL;

COMMENT ON TABLE "ob-poc".product_service_option_overrides IS
    'Product-context overrides for service options. Product-level versioning is deferred; v1 uses row-level effective dating and supersession.';

-- ---------------------------------------------------------------------------
-- Resource eligibility constraints and fan-out rules.
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS "ob-poc".service_resource_option_constraints (
    constraint_id uuid PRIMARY KEY DEFAULT uuidv7(),
    service_id uuid NOT NULL REFERENCES "ob-poc".services(service_id) ON DELETE CASCADE,
    resource_id uuid NOT NULL REFERENCES "ob-poc".service_resource_types(resource_id) ON DELETE CASCADE,
    service_option_def_id uuid NOT NULL REFERENCES "ob-poc".service_option_defs(service_option_def_id) ON DELETE CASCADE,
    supported_values jsonb NOT NULL DEFAULT '{}'::jsonb,
    match_operator text NOT NULL DEFAULT 'intersect',
    priority integer NOT NULL DEFAULT 100,
    is_required_for_coverage boolean NOT NULL DEFAULT false,
    is_active boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT service_resource_option_constraints_match_operator_check
        CHECK (match_operator IN ('exact', 'subset', 'superset', 'intersect'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_service_resource_option_constraints_active
    ON "ob-poc".service_resource_option_constraints(service_id, resource_id, service_option_def_id, match_operator)
    WHERE is_active = true;

CREATE INDEX IF NOT EXISTS idx_service_resource_option_constraints_option
    ON "ob-poc".service_resource_option_constraints(service_option_def_id, priority);

COMMENT ON TABLE "ob-poc".service_resource_option_constraints IS
    'Eligibility constraints: which option values a service resource can serve. This formalises service_resource_capabilities.supported_options.';

CREATE TABLE IF NOT EXISTS "ob-poc".service_resource_fanout_rules (
    fanout_rule_id uuid PRIMARY KEY DEFAULT uuidv7(),
    service_id uuid NOT NULL REFERENCES "ob-poc".services(service_id) ON DELETE CASCADE,
    resource_id uuid NOT NULL REFERENCES "ob-poc".service_resource_types(resource_id) ON DELETE CASCADE,
    service_option_def_id uuid REFERENCES "ob-poc".service_option_defs(service_option_def_id) ON DELETE CASCADE,
    fanout_axis text NOT NULL,
    fanout_mode text NOT NULL,
    group_by_policy jsonb NOT NULL DEFAULT '{}'::jsonb,
    shared_when_null boolean NOT NULL DEFAULT true,
    priority integer NOT NULL DEFAULT 100,
    is_active boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT service_resource_fanout_rules_fanout_axis_check
        CHECK (fanout_axis IN ('none', 'market', 'currency', 'counterparty', 'account', 'fund', 'share_class', 'legal_entity', 'instruction_channel', 'jurisdiction', 'booking_principal')),
    CONSTRAINT service_resource_fanout_rules_fanout_mode_check
        CHECK (fanout_mode IN ('per_value', 'shared', 'grouped', 'conditional')),
    CONSTRAINT service_resource_fanout_rules_group_by_policy_object_check
        CHECK (jsonb_typeof(group_by_policy) = 'object')
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_service_resource_fanout_rules_active
    ON "ob-poc".service_resource_fanout_rules(service_id, resource_id, fanout_axis, COALESCE(service_option_def_id, '00000000-0000-0000-0000-000000000000'::uuid))
    WHERE is_active = true;

CREATE INDEX IF NOT EXISTS idx_service_resource_fanout_rules_option
    ON "ob-poc".service_resource_fanout_rules(service_option_def_id, priority)
    WHERE service_option_def_id IS NOT NULL;

COMMENT ON TABLE "ob-poc".service_resource_fanout_rules IS
    'Materialisation rules: whether a resource fans out per option value, remains shared, groups values, or follows conditional policy.';

-- ---------------------------------------------------------------------------
-- Runtime activation anchor, option bindings, and resource lineage.
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS "ob-poc".activation_runs (
    activation_run_id uuid PRIMARY KEY DEFAULT uuidv7(),
    cbu_id uuid NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    product_id uuid REFERENCES "ob-poc".products(product_id),
    run_kind text NOT NULL,
    status text NOT NULL DEFAULT 'started',
    triggered_by text,
    started_at timestamptz NOT NULL DEFAULT now(),
    completed_at timestamptz,
    failed_at timestamptz,
    failure_reason text,
    input_snapshot jsonb NOT NULL DEFAULT '{}'::jsonb,
    result_summary jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT activation_runs_run_kind_check
        CHECK (run_kind IN ('bind_options', 'validate_coverage', 'compute_fanout', 'activate', 'replay')),
    CONSTRAINT activation_runs_status_check
        CHECK (status IN ('started', 'succeeded', 'failed', 'cancelled')),
    CONSTRAINT activation_runs_terminal_timestamp_check
        CHECK (
            (status = 'succeeded' AND completed_at IS NOT NULL AND failed_at IS NULL)
            OR (status = 'failed' AND failed_at IS NOT NULL)
            OR (status IN ('started', 'cancelled'))
        )
);

CREATE INDEX IF NOT EXISTS idx_activation_runs_cbu_started
    ON "ob-poc".activation_runs(cbu_id, started_at DESC);

COMMENT ON TABLE "ob-poc".activation_runs IS
    'Runtime anchor for option binding, validation, fan-out, activation, and replay. Provides a stable activation_run_id for deterministic historical replay.';

CREATE TABLE IF NOT EXISTS "ob-poc".cbu_service_option_bindings (
    binding_id uuid PRIMARY KEY DEFAULT uuidv7(),
    cbu_id uuid NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    product_id uuid REFERENCES "ob-poc".products(product_id),
    service_id uuid NOT NULL REFERENCES "ob-poc".services(service_id) ON DELETE CASCADE,
    service_version_id uuid NOT NULL REFERENCES "ob-poc".service_versions(id),
    service_option_def_id uuid NOT NULL REFERENCES "ob-poc".service_option_defs(service_option_def_id),
    option_key text NOT NULL,
    value jsonb NOT NULL,
    source_kind text NOT NULL,
    source_ref jsonb,
    source_version text,
    value_hash text NOT NULL,
    coherence_status text NOT NULL DEFAULT 'clean',
    is_locked boolean NOT NULL DEFAULT false,
    valid_from timestamptz NOT NULL DEFAULT now(),
    valid_to timestamptz,
    supersedes_binding_id uuid REFERENCES "ob-poc".cbu_service_option_bindings(binding_id),
    activation_run_id uuid REFERENCES "ob-poc".activation_runs(activation_run_id),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT cbu_service_option_bindings_source_kind_check
        CHECK (source_kind IN ('derived', 'cbu_profile', 'instrument_matrix', 'legal_entity', 'document', 'product_option', 'manual', 'option_binding')),
    CONSTRAINT cbu_service_option_bindings_coherence_status_check
        CHECK (coherence_status IN ('clean', 'dirty', 'stale')),
    CONSTRAINT cbu_service_option_bindings_valid_range_check
        CHECK (valid_to IS NULL OR valid_to > valid_from),
    CONSTRAINT cbu_service_option_bindings_not_self_superseded_check
        CHECK (supersedes_binding_id IS NULL OR supersedes_binding_id <> binding_id),
    CONSTRAINT cbu_service_option_bindings_value_hash_check
        CHECK (value_hash ~ '^[a-f0-9]{64}$')
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_cbu_service_option_bindings_current
    ON "ob-poc".cbu_service_option_bindings(cbu_id, service_id, service_option_def_id)
    WHERE valid_to IS NULL;

CREATE INDEX IF NOT EXISTS idx_cbu_service_option_bindings_source
    ON "ob-poc".cbu_service_option_bindings(source_kind, coherence_status);

CREATE INDEX IF NOT EXISTS idx_cbu_service_option_bindings_source_ref
    ON "ob-poc".cbu_service_option_bindings USING gin (source_ref jsonb_path_ops)
    WHERE source_ref IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_cbu_service_option_bindings_run
    ON "ob-poc".cbu_service_option_bindings(activation_run_id)
    WHERE activation_run_id IS NOT NULL;

COMMENT ON TABLE "ob-poc".cbu_service_option_bindings IS
    'Versioned runtime option bindings for a CBU service. Rows supersede rather than overwrite so activation replay can target historical source versions.';

COMMENT ON COLUMN "ob-poc".cbu_service_option_bindings.value_hash IS
    'SHA-256 over canonical JSON. Canonicalization is implemented in application code and tested for key-order stability.';

CREATE TABLE IF NOT EXISTS "ob-poc".cbu_resource_instance_option_lineage (
    lineage_id uuid PRIMARY KEY DEFAULT uuidv7(),
    resource_instance_id uuid NOT NULL REFERENCES "ob-poc".cbu_resource_instances(instance_id) ON DELETE CASCADE,
    binding_id uuid NOT NULL REFERENCES "ob-poc".cbu_service_option_bindings(binding_id) ON DELETE CASCADE,
    contribution_type text NOT NULL,
    fanout_axis text,
    fanout_value jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT cbu_resource_instance_option_lineage_contribution_type_check
        CHECK (contribution_type IN ('eligibility', 'fanout', 'attribute_source')),
    CONSTRAINT cbu_resource_instance_option_lineage_fanout_axis_check
        CHECK (fanout_axis IS NULL OR fanout_axis IN ('none', 'market', 'currency', 'counterparty', 'account', 'fund', 'share_class', 'legal_entity', 'instruction_channel', 'jurisdiction', 'booking_principal'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_cbu_resource_instance_option_lineage_dedupe
    ON "ob-poc".cbu_resource_instance_option_lineage(
        resource_instance_id,
        binding_id,
        contribution_type,
        COALESCE(fanout_axis, ''),
        COALESCE(fanout_value, 'null'::jsonb)
    );

CREATE INDEX IF NOT EXISTS idx_cbu_resource_instance_option_lineage_binding
    ON "ob-poc".cbu_resource_instance_option_lineage(binding_id);

COMMENT ON TABLE "ob-poc".cbu_resource_instance_option_lineage IS
    'Reverse lineage from materialized resource instances to the option bindings that justified eligibility, fan-out, or attribute source values.';

-- ---------------------------------------------------------------------------
-- Compatibility-safe source policy restructuring.
-- ---------------------------------------------------------------------------

ALTER TABLE "ob-poc".resource_attribute_requirements
    ADD COLUMN IF NOT EXISTS source_kind text,
    ADD COLUMN IF NOT EXISTS source_fallback text[],
    ADD COLUMN IF NOT EXISTS derivation_input_type text,
    ADD COLUMN IF NOT EXISTS derivation_input_ref jsonb;

UPDATE "ob-poc".resource_attribute_requirements
SET
    source_kind = COALESCE(source_kind, source_policy ->> 0),
    source_fallback = COALESCE(
        source_fallback,
        ARRAY(
            SELECT source.value
            FROM jsonb_array_elements_text(source_policy) WITH ORDINALITY AS source(value, ordinality)
            WHERE source.ordinality > 1
        )
    )
WHERE source_policy IS NOT NULL
  AND jsonb_typeof(source_policy) = 'array';

ALTER TABLE "ob-poc".resource_attribute_requirements
    DROP CONSTRAINT IF EXISTS resource_attribute_requirements_source_kind_check;

ALTER TABLE "ob-poc".resource_attribute_requirements
    ADD CONSTRAINT resource_attribute_requirements_source_kind_check
    CHECK (
        source_kind IS NULL
        OR source_kind IN (
            'derived', 'cbu_profile', 'cbu', 'instrument_matrix', 'legal_entity',
            'entity', 'document', 'product_option', 'manual', 'option_binding'
        )
    );

COMMENT ON COLUMN "ob-poc".resource_attribute_requirements.source_policy IS
    'Deprecated compatibility JSONB. Prefer source_kind, source_fallback, derivation_input_type, and derivation_input_ref.';

COMMENT ON COLUMN "ob-poc".resource_attribute_requirements.source_kind IS
    'Structured primary source classification for this resource attribute. Compatibility values cbu/entity remain until source_policy retirement.';

COMMENT ON COLUMN "ob-poc".resource_attribute_requirements.source_fallback IS
    'Structured fallback source kinds, preserving the old source_policy order after the primary source_kind.';

COMMENT ON COLUMN "ob-poc".resource_attribute_requirements.derivation_input_type IS
    'Optional derivation input type, e.g. option_binding, when source_kind is derived.';

COMMENT ON COLUMN "ob-poc".resource_attribute_requirements.derivation_input_ref IS
    'Optional derivation input reference metadata, e.g. option binding key/path.';

COMMIT;
