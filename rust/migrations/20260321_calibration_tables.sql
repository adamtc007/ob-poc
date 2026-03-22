ALTER TABLE "ob-poc".utterance_traces
    ADD COLUMN IF NOT EXISTS is_synthetic BOOLEAN NOT NULL DEFAULT false;

CREATE INDEX IF NOT EXISTS idx_utterance_traces_is_synthetic
    ON "ob-poc".utterance_traces (is_synthetic);

CREATE TABLE IF NOT EXISTS "ob-poc".calibration_scenarios (
    scenario_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    scenario_name TEXT NOT NULL,
    created_by TEXT NOT NULL,
    governance_status TEXT NOT NULL DEFAULT 'Draft',
    constellation_template_id TEXT NOT NULL,
    constellation_template_version TEXT NOT NULL,
    situation_signature TEXT,
    situation_signature_hash BIGINT,
    operational_phase TEXT,
    target_entity_type TEXT NOT NULL,
    target_entity_state TEXT NOT NULL,
    linked_entity_states JSONB NOT NULL DEFAULT '[]'::jsonb,
    target_verb TEXT NOT NULL,
    legal_verb_set_snapshot JSONB NOT NULL DEFAULT '[]'::jsonb,
    verb_taxonomy_tag TEXT,
    excluded_neighbours JSONB NOT NULL DEFAULT '[]'::jsonb,
    near_neighbour_verbs JSONB NOT NULL DEFAULT '[]'::jsonb,
    expected_margin_threshold REAL NOT NULL DEFAULT 0.0,
    execution_shape TEXT NOT NULL DEFAULT 'Singleton',
    gold_utterances JSONB NOT NULL DEFAULT '[]'::jsonb,
    admitted_synthetic_set_id UUID,
    seed_data JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_cal_scenario_verb
    ON "ob-poc".calibration_scenarios (target_verb);

CREATE INDEX IF NOT EXISTS idx_cal_scenario_template
    ON "ob-poc".calibration_scenarios (constellation_template_id);

CREATE INDEX IF NOT EXISTS idx_cal_scenario_phase
    ON "ob-poc".calibration_scenarios (operational_phase);

CREATE INDEX IF NOT EXISTS idx_cal_scenario_status
    ON "ob-poc".calibration_scenarios (governance_status);

CREATE INDEX IF NOT EXISTS idx_cal_scenario_signature_hash
    ON "ob-poc".calibration_scenarios (situation_signature_hash)
    WHERE situation_signature_hash IS NOT NULL;

CREATE TABLE IF NOT EXISTS "ob-poc".calibration_utterances (
    utterance_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    scenario_id UUID NOT NULL REFERENCES "ob-poc".calibration_scenarios(scenario_id),
    text TEXT NOT NULL,
    calibration_mode TEXT NOT NULL,
    negative_type TEXT,
    lifecycle_status TEXT NOT NULL DEFAULT 'Generated',
    expected_outcome JSONB NOT NULL,
    generation_rationale TEXT,
    pre_screen JSONB,
    pre_screen_stratum TEXT,
    reviewed_by TEXT,
    admitted_at TIMESTAMPTZ,
    deprecated_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_cal_utt_scenario
    ON "ob-poc".calibration_utterances (scenario_id, lifecycle_status);

CREATE INDEX IF NOT EXISTS idx_cal_utt_mode
    ON "ob-poc".calibration_utterances (calibration_mode);

CREATE INDEX IF NOT EXISTS idx_cal_utt_stratum
    ON "ob-poc".calibration_utterances (pre_screen_stratum)
    WHERE pre_screen_stratum IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_cal_utt_admitted
    ON "ob-poc".calibration_utterances (scenario_id)
    WHERE lifecycle_status = 'Admitted';

CREATE TABLE IF NOT EXISTS "ob-poc".calibration_runs (
    run_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    scenario_id UUID NOT NULL REFERENCES "ob-poc".calibration_scenarios(scenario_id),
    triggered_by TEXT NOT NULL,
    surface_versions JSONB NOT NULL,
    utterance_count INTEGER NOT NULL,
    positive_count INTEGER NOT NULL DEFAULT 0,
    negative_count INTEGER NOT NULL DEFAULT 0,
    boundary_count INTEGER NOT NULL DEFAULT 0,
    metrics JSONB NOT NULL,
    drift JSONB,
    prior_run_id UUID REFERENCES "ob-poc".calibration_runs(run_id),
    trace_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
    run_start TIMESTAMPTZ NOT NULL,
    run_end TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_cal_run_scenario
    ON "ob-poc".calibration_runs (scenario_id, run_start DESC);

CREATE INDEX IF NOT EXISTS idx_cal_run_prior
    ON "ob-poc".calibration_runs (prior_run_id)
    WHERE prior_run_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS "ob-poc".calibration_outcomes (
    outcome_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id UUID NOT NULL REFERENCES "ob-poc".calibration_runs(run_id),
    utterance_id UUID NOT NULL REFERENCES "ob-poc".calibration_utterances(utterance_id),
    trace_id UUID NOT NULL REFERENCES "ob-poc".utterance_traces(trace_id),
    calibration_mode TEXT NOT NULL,
    negative_type TEXT,
    expected_outcome JSONB NOT NULL,
    verdict TEXT NOT NULL,
    actual_resolved_verb TEXT,
    actual_halt_reason TEXT,
    failure_phase SMALLINT,
    failure_detail JSONB,
    top1_score REAL,
    top2_score REAL,
    margin REAL,
    margin_stable BOOLEAN,
    latency_total_ms INTEGER,
    latency_per_phase JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_cal_out_run
    ON "ob-poc".calibration_outcomes (run_id, verdict);

CREATE INDEX IF NOT EXISTS idx_cal_out_trace
    ON "ob-poc".calibration_outcomes (trace_id);

CREATE INDEX IF NOT EXISTS idx_cal_out_fragile
    ON "ob-poc".calibration_outcomes (margin_stable)
    WHERE margin_stable = false;

CREATE INDEX IF NOT EXISTS idx_cal_out_failures
    ON "ob-poc".calibration_outcomes (verdict, failure_phase)
    WHERE verdict <> 'Pass';
