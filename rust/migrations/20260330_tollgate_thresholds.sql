-- Tollgate thresholds — configurable pass/fail criteria per evaluation type.
-- Referenced by: tollgate_ops.rs (TollgateEvaluateOp)

CREATE TABLE IF NOT EXISTS "ob-poc".tollgate_thresholds (
    threshold_id        uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    threshold_name      varchar(100) NOT NULL,
    metric_type         varchar(50) NOT NULL,
    comparison          varchar(10) NOT NULL DEFAULT 'gte',
    threshold_value     numeric(10,2),
    is_blocking         boolean DEFAULT true NOT NULL,
    weight              numeric(5,2),
    applies_to_case_types text[],
    description         text,
    created_at          timestamptz DEFAULT now() NOT NULL,
    CONSTRAINT tollgate_thresholds_chk_comparison CHECK (
        comparison IN ('eq', 'neq', 'gt', 'gte', 'lt', 'lte')
    )
);

-- Seed default thresholds for standard evaluation types
INSERT INTO "ob-poc".tollgate_thresholds (threshold_name, metric_type, comparison, threshold_value, is_blocking, applies_to_case_types) VALUES
    ('Ownership Graph Coverage', 'ownership_coverage_pct', 'gte', 70.0, true, NULL),
    ('Identity Documents Complete', 'identity_doc_coverage_pct', 'gte', 100.0, true, NULL),
    ('Screening Clear', 'screening_clear_pct', 'gte', 100.0, true, NULL),
    ('UBO Determination', 'ubo_determination_count', 'gte', 1.0, true, NULL),
    ('Red Flags Resolved', 'unresolved_red_flag_count', 'eq', 0.0, true, NULL),
    ('Evidence Verified', 'evidence_verified_pct', 'gte', 80.0, false, NULL)
ON CONFLICT DO NOTHING;
