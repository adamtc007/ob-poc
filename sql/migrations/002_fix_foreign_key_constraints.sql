-- Migration: Fix foreign key constraints to ensure referential integrity
-- This migration addresses missing or incorrect foreign key constraints across the schema

BEGIN;

-- ============================================================================
-- 1. Fix CBU-related foreign key constraints (already handled in 001, but ensuring completeness)
-- ============================================================================

-- Ensure dsl_ob.cbu_id properly references cbus table (handled in previous migration)
-- This is a safety check in case the constraint wasn't added
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_dsl_ob_cbu_id'
        AND table_name = 'dsl_ob'
        AND table_schema = 'ob-poc'
    ) THEN
        ALTER TABLE "ob-poc".dsl_ob
        ADD CONSTRAINT fk_dsl_ob_cbu_id
        FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;
    END IF;
END $$;

-- ============================================================================
-- 2. Fix attribute_values table foreign key constraints
-- ============================================================================

-- Ensure attribute_values.dsl_ob_id references dsl_ob table if the column exists
-- Note: This column is optional according to the schema
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'attribute_values'
        AND column_name = 'dsl_ob_id'
        AND table_schema = 'ob-poc'
    ) THEN
        -- Add foreign key constraint for dsl_ob_id if it doesn't exist
        IF NOT EXISTS (
            SELECT 1 FROM information_schema.table_constraints
            WHERE constraint_name = 'fk_attribute_values_dsl_ob_id'
            AND table_name = 'attribute_values'
            AND table_schema = 'ob-poc'
        ) THEN
            ALTER TABLE "ob-poc".attribute_values
            ADD CONSTRAINT fk_attribute_values_dsl_ob_id
            FOREIGN KEY (dsl_ob_id) REFERENCES "ob-poc".dsl_ob(version_id) ON DELETE SET NULL;
        END IF;
    END IF;
END $$;

-- ============================================================================
-- 3. Fix product requirements foreign key constraints
-- ============================================================================

-- Ensure product_requirements.product_id references products table
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_product_requirements_product_id'
        AND table_name = 'product_requirements'
        AND table_schema = 'ob-poc'
    ) THEN
        ALTER TABLE "ob-poc".product_requirements
        ADD CONSTRAINT fk_product_requirements_product_id
        FOREIGN KEY (product_id) REFERENCES "ob-poc".products(product_id) ON DELETE CASCADE;
    END IF;
END $$;

-- Ensure entity_product_mappings.product_id references products table
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_entity_product_mappings_product_id'
        AND table_name = 'entity_product_mappings'
        AND table_schema = 'ob-poc'
    ) THEN
        ALTER TABLE "ob-poc".entity_product_mappings
        ADD CONSTRAINT fk_entity_product_mappings_product_id
        FOREIGN KEY (product_id) REFERENCES "ob-poc".products(product_id) ON DELETE CASCADE;
    END IF;
END $$;

-- Ensure product_workflows.product_id references products table
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_product_workflows_product_id'
        AND table_name = 'product_workflows'
        AND table_schema = 'ob-poc'
    ) THEN
        ALTER TABLE "ob-poc".product_workflows
        ADD CONSTRAINT fk_product_workflows_product_id
        FOREIGN KEY (product_id) REFERENCES "ob-poc".products(product_id) ON DELETE CASCADE;
    END IF;
END $$;

-- ============================================================================
-- 4. Fix service and resource foreign key constraints
-- ============================================================================

-- Ensure service_resources.service_id references services table
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_service_resources_service_id'
        AND table_name = 'service_resources'
        AND table_schema = 'ob-poc'
    ) THEN
        ALTER TABLE "ob-poc".service_resources
        ADD CONSTRAINT fk_service_resources_service_id
        FOREIGN KEY (service_id) REFERENCES "ob-poc".services(service_id) ON DELETE CASCADE;
    END IF;
END $$;

-- Ensure service_resources.resource_id references prod_resources table
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_service_resources_resource_id'
        AND table_name = 'service_resources'
        AND table_schema = 'ob-poc'
    ) THEN
        ALTER TABLE "ob-poc".service_resources
        ADD CONSTRAINT fk_service_resources_resource_id
        FOREIGN KEY (resource_id) REFERENCES "ob-poc".prod_resources(resource_id) ON DELETE CASCADE;
    END IF;
END $$;

-- ============================================================================
-- 5. Fix entity relationship foreign key constraints
-- ============================================================================

-- Ensure entities.entity_type_id references entity_types table
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_entities_entity_type_id'
        AND table_name = 'entities'
        AND table_schema = 'ob-poc'
    ) THEN
        ALTER TABLE "ob-poc".entities
        ADD CONSTRAINT fk_entities_entity_type_id
        FOREIGN KEY (entity_type_id) REFERENCES "ob-poc".entity_types(entity_type_id) ON DELETE CASCADE;
    END IF;
END $$;

-- ============================================================================
-- 6. Fix trust-specific foreign key constraints
-- ============================================================================

-- Ensure trust_parties.trust_id references entity_trusts table
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_trust_parties_trust_id'
        AND table_name = 'trust_parties'
        AND table_schema = 'ob-poc'
    ) THEN
        ALTER TABLE "ob-poc".trust_parties
        ADD CONSTRAINT fk_trust_parties_trust_id
        FOREIGN KEY (trust_id) REFERENCES "ob-poc".entity_trusts(trust_id) ON DELETE CASCADE;
    END IF;
END $$;

-- Ensure trust_parties.entity_id references entities table
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_trust_parties_entity_id'
        AND table_name = 'trust_parties'
        AND table_schema = 'ob-poc'
    ) THEN
        ALTER TABLE "ob-poc".trust_parties
        ADD CONSTRAINT fk_trust_parties_entity_id
        FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;
    END IF;
END $$;

-- Ensure trust_beneficiary_classes.trust_id references entity_trusts table
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_trust_beneficiary_classes_trust_id'
        AND table_name = 'trust_beneficiary_classes'
        AND table_schema = 'ob-poc'
    ) THEN
        ALTER TABLE "ob-poc".trust_beneficiary_classes
        ADD CONSTRAINT fk_trust_beneficiary_classes_trust_id
        FOREIGN KEY (trust_id) REFERENCES "ob-poc".entity_trusts(trust_id) ON DELETE CASCADE;
    END IF;
END $$;

-- Ensure trust_protector_powers.trust_party_id references trust_parties table
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_trust_protector_powers_trust_party_id'
        AND table_name = 'trust_protector_powers'
        AND table_schema = 'ob-poc'
    ) THEN
        ALTER TABLE "ob-poc".trust_protector_powers
        ADD CONSTRAINT fk_trust_protector_powers_trust_party_id
        FOREIGN KEY (trust_party_id) REFERENCES "ob-poc".trust_parties(trust_party_id) ON DELETE CASCADE;
    END IF;
END $$;

-- ============================================================================
-- 7. Fix partnership-specific foreign key constraints
-- ============================================================================

-- Ensure partnership_interests.partnership_id references entity_partnerships table
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_partnership_interests_partnership_id'
        AND table_name = 'partnership_interests'
        AND table_schema = 'dsl-ob-poc'
    ) THEN
        ALTER TABLE "ob-poc".partnership_interests
        ADD CONSTRAINT fk_partnership_interests_partnership_id
        FOREIGN KEY (partnership_id) REFERENCES "ob-poc".entity_partnerships(partnership_id) ON DELETE CASCADE;
    END IF;
END $$;

-- Ensure partnership_interests.entity_id references entities table
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_partnership_interests_entity_id'
        AND table_name = 'partnership_interests'
        AND table_schema = 'dsl-ob-poc'
    ) THEN
        ALTER TABLE "ob-poc".partnership_interests
        ADD CONSTRAINT fk_partnership_interests_entity_id
        FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;
    END IF;
END $$;

-- Ensure partnership_control_mechanisms foreign keys
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_partnership_control_mechanisms_partnership_id'
        AND table_name = 'partnership_control_mechanisms'
        AND table_schema = 'dsl-ob-poc'
    ) THEN
        ALTER TABLE "ob-poc".partnership_control_mechanisms
        ADD CONSTRAINT fk_partnership_control_mechanisms_partnership_id
        FOREIGN KEY (partnership_id) REFERENCES "ob-poc".entity_partnerships(partnership_id) ON DELETE CASCADE;
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_partnership_control_mechanisms_entity_id'
        AND table_name = 'partnership_control_mechanisms'
        AND table_schema = 'dsl-ob-poc'
    ) THEN
        ALTER TABLE "ob-poc".partnership_control_mechanisms
        ADD CONSTRAINT fk_partnership_control_mechanisms_entity_id
        FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;
    END IF;
END $$;

-- ============================================================================
-- 8. Fix UBO registry foreign key constraints
-- ============================================================================

-- Ensure ubo_registry.subject_entity_id references entities table
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_ubo_registry_subject_entity_id'
        AND table_name = 'ubo_registry'
        AND table_schema = 'dsl-ob-poc'
    ) THEN
        ALTER TABLE "ob-poc".ubo_registry
        ADD CONSTRAINT fk_ubo_registry_subject_entity_id
        FOREIGN KEY (subject_entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;
    END IF;
END $$;

-- Ensure ubo_registry.ubo_proper_person_id references entities table
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_ubo_registry_ubo_proper_person_id'
        AND table_name = 'ubo_registry'
        AND table_schema = 'dsl-ob-poc'
    ) THEN
        ALTER TABLE "ob-poc".ubo_registry
        ADD CONSTRAINT fk_ubo_registry_ubo_proper_person_id
        FOREIGN KEY (ubo_proper_person_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;
    END IF;
END $$;

-- ============================================================================
-- 9. Fix orchestration session foreign key constraints
-- ============================================================================

-- Ensure orchestration_domain_sessions.orchestration_session_id references orchestration_sessions
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_orchestration_domain_sessions_orchestration_session_id'
        AND table_name = 'orchestration_domain_sessions'
        AND table_schema = 'dsl-ob-poc'
    ) THEN
        ALTER TABLE "ob-poc".orchestration_domain_sessions
        ADD CONSTRAINT fk_orchestration_domain_sessions_orchestration_session_id
        FOREIGN KEY (orchestration_session_id) REFERENCES "ob-poc".orchestration_sessions(session_id) ON DELETE CASCADE;
    END IF;
END $$;

-- Ensure orchestration_tasks.orchestration_session_id references orchestration_sessions
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_orchestration_tasks_orchestration_session_id'
        AND table_name = 'orchestration_tasks'
        AND table_schema = 'dsl-ob-poc'
    ) THEN
        ALTER TABLE "ob-poc".orchestration_tasks
        ADD CONSTRAINT fk_orchestration_tasks_orchestration_session_id
        FOREIGN KEY (orchestration_session_id) REFERENCES "ob-poc".orchestration_sessions(session_id) ON DELETE CASCADE;
    END IF;
END $$;

-- Ensure orchestration_state_history.orchestration_session_id references orchestration_sessions
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_orchestration_state_history_orchestration_session_id'
        AND table_name = 'orchestration_state_history'
        AND table_schema = 'dsl-ob-poc'
    ) THEN
        ALTER TABLE "ob-poc".orchestration_state_history
        ADD CONSTRAINT fk_orchestration_state_history_orchestration_session_id
        FOREIGN KEY (orchestration_session_id) REFERENCES "ob-poc".orchestration_sessions(session_id) ON DELETE CASCADE;
    END IF;
END $$;

-- ============================================================================
-- 10. Add missing constraints that were overlooked in the original schema
-- ============================================================================

-- Ensure cbu_entity_roles properly references all three tables
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_cbu_entity_roles_cbu_id'
        AND table_name = 'cbu_entity_roles'
        AND table_schema = 'dsl-ob-poc'
    ) THEN
        ALTER TABLE "ob-poc".cbu_entity_roles
        ADD CONSTRAINT fk_cbu_entity_roles_cbu_id
        FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE;
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_cbu_entity_roles_entity_id'
        AND table_name = 'cbu_entity_roles'
        AND table_schema = 'dsl-ob-poc'
    ) THEN
        ALTER TABLE "ob-poc".cbu_entity_roles
        ADD CONSTRAINT fk_cbu_entity_roles_entity_id
        FOREIGN KEY (entity_id) REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE;
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_cbu_entity_roles_role_id'
        AND table_name = 'cbu_entity_roles'
        AND table_schema = 'dsl-ob-poc'
    ) THEN
        ALTER TABLE "ob-poc".cbu_entity_roles
        ADD CONSTRAINT fk_cbu_entity_roles_role_id
        FOREIGN KEY (role_id) REFERENCES "ob-poc".roles(role_id) ON DELETE CASCADE;
    END IF;
END $$;

-- ============================================================================
-- 11. Add constraints with proper ON DELETE behavior for data integrity
-- ============================================================================

-- Update constraints that might need different deletion behaviors
-- For audit tables, we generally want to preserve records even if referenced entities are deleted

-- Update UBO registry to use SET NULL for CBU deletions (preserve UBO data for audit)
DO $$
BEGIN
    -- Drop and recreate with SET NULL behavior for cbu_id
    IF EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_ubo_registry_cbu_id'
        AND table_name = 'ubo_registry'
        AND table_schema = 'dsl-ob-poc'
    ) THEN
        ALTER TABLE "ob-poc".ubo_registry DROP CONSTRAINT fk_ubo_registry_cbu_id;
    END IF;

    ALTER TABLE "ob-poc".ubo_registry
    ADD CONSTRAINT fk_ubo_registry_cbu_id
    FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE SET NULL;
END $$;

-- ============================================================================
-- 12. Create validation views to monitor referential integrity
-- ============================================================================

-- Create a view to check for orphaned records
CREATE OR REPLACE VIEW "ob-poc".referential_integrity_check AS
WITH integrity_issues AS (
    -- Check for orphaned dsl_ob records
    SELECT 'dsl_ob' as table_name, 'cbu_id' as column_name, cbu_id::text as orphaned_value, 'missing CBU reference' as issue
    FROM "ob-poc".dsl_ob d
    WHERE NOT EXISTS (SELECT 1 FROM "ob-poc".cbus c WHERE c.cbu_id = d.cbu_id)

    UNION ALL

    -- Check for orphaned attribute_values records
    SELECT 'attribute_values' as table_name, 'cbu_id' as column_name, cbu_id::text as orphaned_value, 'missing CBU reference' as issue
    FROM "ob-poc".attribute_values av
    WHERE NOT EXISTS (SELECT 1 FROM "ob-poc".cbus c WHERE c.cbu_id = av.cbu_id)

    UNION ALL

    -- Check for orphaned attribute_values.attribute_id
    SELECT 'attribute_values' as table_name, 'attribute_id' as column_name, attribute_id::text as orphaned_value, 'missing dictionary reference' as issue
    FROM "ob-poc".attribute_values av
    WHERE NOT EXISTS (SELECT 1 FROM "ob-poc".dictionary d WHERE d.attribute_id = av.attribute_id)

    -- Add more checks as needed...
)
SELECT * FROM integrity_issues;

-- ============================================================================
-- 13. Add comments documenting the foreign key relationships
-- ============================================================================

COMMENT ON TABLE "ob-poc".dsl_ob IS 'DSL documents with enforced CBU referential integrity';
COMMENT ON TABLE "dsl-ob-poc".attribute_values IS 'Attribute values with enforced dictionary and CBU referential integrity';
COMMENT ON TABLE "dsl-ob-poc".ubo_registry IS 'UBO identification results with proper entity referential integrity';

-- Add completion log
INSERT INTO "dsl-ob-poc".dsl_ob (cbu_id, dsl_text)
SELECT
    c.cbu_id,
    '(system.migration (migration-id "002_fix_foreign_key_constraints") (status "completed") (timestamp "' || now()::text || '"))'
FROM "dsl-ob-poc".cbus c
LIMIT 1
ON CONFLICT DO NOTHING;

COMMIT;
