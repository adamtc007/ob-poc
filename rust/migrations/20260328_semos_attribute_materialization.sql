-- Materialization trigger: SemOS AttributeDef snapshots → attribute_registry
--
-- Any active AttributeDef snapshot publish automatically projects to the
-- operational store.  This ensures attribute_registry is always consistent
-- with SemOS regardless of the code path that published the snapshot
-- (verb handler, governance pipeline, bulk reconcile, etc.).

CREATE OR REPLACE FUNCTION sem_reg.materialize_attribute_def_to_registry()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    v_fqn          text;
    v_uuid         uuid;
    v_name         text;
    v_domain       text;
    v_data_type    text;
    v_category     text;
    v_evidence     text;
    v_is_derived   boolean;
    v_deriv_fqn    text;
    v_val_rules    jsonb;
    v_applicability jsonb;
BEGIN
    -- Only fire for active attribute_def snapshots that are not yet superseded
    IF NEW.object_type::text <> 'attribute_def'
       OR NEW.status::text <> 'active'
       OR NEW.effective_until IS NOT NULL
    THEN
        RETURN NEW;
    END IF;

    -- Extract fields from the snapshot definition
    v_fqn          := NEW.definition ->> 'fqn';
    v_name         := NEW.definition ->> 'name';
    v_domain       := NEW.definition ->> 'domain';
    v_data_type    := NEW.definition ->> 'data_type';
    v_category     := NEW.definition ->> 'category';
    v_evidence     := COALESCE(NEW.definition ->> 'evidence_grade', 'none');
    v_is_derived   := COALESCE((NEW.definition ->> 'is_derived')::boolean, false);
    v_deriv_fqn    := NEW.definition ->> 'derivation_spec_fqn';
    v_val_rules    := NEW.definition -> 'validation_rules';
    v_applicability := NEW.definition -> 'applicability';

    -- Cannot materialize without an FQN
    IF v_fqn IS NULL THEN
        RETURN NEW;
    END IF;

    -- Deterministic UUID from the object_id (same logic as Rust object_id_for)
    v_uuid := NEW.object_id;

    INSERT INTO "ob-poc".attribute_registry (
        id, uuid, display_name, category, value_type, domain,
        validation_rules, applicability, evidence_grade,
        is_derived, derivation_spec_fqn, sem_reg_snapshot_id,
        metadata, created_at, updated_at
    )
    VALUES (
        v_fqn,
        v_uuid,
        COALESCE(v_name, v_fqn),
        COALESCE(v_category, 'entity'),
        COALESCE(v_data_type, 'string'),
        v_domain,
        COALESCE(v_val_rules, '{}'::jsonb),
        COALESCE(v_applicability, '{}'::jsonb),
        v_evidence,
        v_is_derived,
        v_deriv_fqn,
        NEW.snapshot_id,
        jsonb_build_object('sem_os', jsonb_build_object(
            'snapshot_id', NEW.snapshot_id,
            'object_id', NEW.object_id,
            'attribute_fqn', v_fqn
        )),
        NOW(),
        NOW()
    )
    ON CONFLICT (id) DO UPDATE SET
        display_name        = EXCLUDED.display_name,
        category            = EXCLUDED.category,
        value_type          = EXCLUDED.value_type,
        domain              = EXCLUDED.domain,
        validation_rules    = COALESCE(EXCLUDED.validation_rules, "ob-poc".attribute_registry.validation_rules),
        applicability       = COALESCE(EXCLUDED.applicability, "ob-poc".attribute_registry.applicability),
        evidence_grade      = EXCLUDED.evidence_grade,
        is_derived          = EXCLUDED.is_derived,
        derivation_spec_fqn = EXCLUDED.derivation_spec_fqn,
        sem_reg_snapshot_id = EXCLUDED.sem_reg_snapshot_id,
        metadata            = jsonb_set(
            COALESCE("ob-poc".attribute_registry.metadata, '{}'::jsonb),
            '{sem_os}',
            COALESCE("ob-poc".attribute_registry.metadata -> 'sem_os', '{}'::jsonb)
                || jsonb_build_object(
                    'snapshot_id', NEW.snapshot_id,
                    'object_id', NEW.object_id,
                    'attribute_fqn', v_fqn
                ),
            true
        ),
        updated_at = NOW();

    RETURN NEW;
END;
$$;

-- Drop if exists to allow re-running
DROP TRIGGER IF EXISTS trg_materialize_attribute_def ON sem_reg.snapshots;

CREATE TRIGGER trg_materialize_attribute_def
    AFTER INSERT ON sem_reg.snapshots
    FOR EACH ROW
    EXECUTE FUNCTION sem_reg.materialize_attribute_def_to_registry();
