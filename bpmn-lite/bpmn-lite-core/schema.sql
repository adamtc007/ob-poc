-- =============================================================================
-- BPMN-Lite Master Schema
-- Generated: 2026-02-10
-- Source: migrations/001 through 013
--
-- This file is the consolidated schema for the bpmn-lite-core database.
-- For incremental changes, use the individual migration files.
-- =============================================================================

-- ---------------------------------------------------------------------------
-- 001: Process Instances
-- ---------------------------------------------------------------------------
CREATE TABLE process_instances (
    instance_id       UUID PRIMARY KEY,
    process_key       TEXT NOT NULL,
    bytecode_version  BYTEA NOT NULL,
    domain_payload    TEXT NOT NULL,
    domain_payload_hash BYTEA NOT NULL,
    flags             JSONB NOT NULL DEFAULT '{}',
    counters          JSONB NOT NULL DEFAULT '{}',
    join_expected     JSONB NOT NULL DEFAULT '{}',
    state             JSONB NOT NULL,
    correlation_id    TEXT NOT NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_instances_process_key ON process_instances (process_key);
CREATE INDEX idx_instances_correlation ON process_instances (correlation_id);

-- ---------------------------------------------------------------------------
-- 002: Fibers
-- ---------------------------------------------------------------------------
CREATE TABLE fibers (
    instance_id  UUID NOT NULL REFERENCES process_instances(instance_id) ON DELETE CASCADE,
    fiber_id     UUID NOT NULL,
    pc           INTEGER NOT NULL,
    stack        JSONB NOT NULL DEFAULT '[]',
    regs         JSONB NOT NULL DEFAULT '[]',
    wait_state   JSONB NOT NULL,
    loop_epoch   INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (instance_id, fiber_id)
);

-- ---------------------------------------------------------------------------
-- 003: Join Barriers
-- ---------------------------------------------------------------------------
CREATE TABLE join_barriers (
    instance_id  UUID NOT NULL REFERENCES process_instances(instance_id) ON DELETE CASCADE,
    join_id      INTEGER NOT NULL,
    arrive_count SMALLINT NOT NULL DEFAULT 0,
    PRIMARY KEY (instance_id, join_id)
);

-- ---------------------------------------------------------------------------
-- 004: Dedupe Cache
-- ---------------------------------------------------------------------------
CREATE TABLE dedupe_cache (
    job_key     TEXT PRIMARY KEY,
    completion  JSONB NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ---------------------------------------------------------------------------
-- 005: Job Queue
-- ---------------------------------------------------------------------------
CREATE TABLE job_queue (
    job_key              TEXT PRIMARY KEY,
    process_instance_id  UUID NOT NULL,
    task_type            TEXT NOT NULL,
    service_task_id      TEXT NOT NULL,
    domain_payload       TEXT NOT NULL,
    domain_payload_hash  BYTEA NOT NULL,
    orch_flags           JSONB NOT NULL DEFAULT '{}',
    retries_remaining    INTEGER NOT NULL DEFAULT 3,
    status               TEXT NOT NULL DEFAULT 'pending',
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    claimed_at           TIMESTAMPTZ
);
CREATE INDEX idx_jobs_pending ON job_queue (task_type, created_at) WHERE status = 'pending';
CREATE INDEX idx_jobs_instance ON job_queue (process_instance_id);

-- ---------------------------------------------------------------------------
-- 006: Compiled Programs
-- ---------------------------------------------------------------------------
CREATE TABLE compiled_programs (
    bytecode_version  BYTEA PRIMARY KEY,
    program           JSONB NOT NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ---------------------------------------------------------------------------
-- 007: Dead Letter Queue
-- ---------------------------------------------------------------------------
CREATE TABLE dead_letter_queue (
    name       INTEGER NOT NULL,
    corr_key   TEXT NOT NULL,
    payload    BYTEA NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (name, corr_key)
);

-- ---------------------------------------------------------------------------
-- 008: Event Sequences
-- ---------------------------------------------------------------------------
CREATE TABLE event_sequences (
    instance_id  UUID PRIMARY KEY,
    next_seq     BIGINT NOT NULL DEFAULT 0
);

-- ---------------------------------------------------------------------------
-- 009: Event Log
-- ---------------------------------------------------------------------------
CREATE TABLE event_log (
    instance_id  UUID NOT NULL,
    seq          BIGINT NOT NULL,
    event        JSONB NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (instance_id, seq)
);

-- ---------------------------------------------------------------------------
-- 010: Payload History
-- ---------------------------------------------------------------------------
CREATE TABLE payload_history (
    instance_id      UUID NOT NULL,
    payload_hash     BYTEA NOT NULL,
    domain_payload   TEXT NOT NULL,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (instance_id, payload_hash)
);

-- ---------------------------------------------------------------------------
-- 011: Incidents
-- ---------------------------------------------------------------------------
CREATE TABLE incidents (
    incident_id          UUID PRIMARY KEY,
    process_instance_id  UUID NOT NULL REFERENCES process_instances(instance_id),
    fiber_id             UUID NOT NULL,
    service_task_id      TEXT NOT NULL,
    bytecode_addr        INTEGER NOT NULL,
    error_class          JSONB NOT NULL,
    message              TEXT NOT NULL,
    retry_count          INTEGER NOT NULL DEFAULT 0,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at          TIMESTAMPTZ,
    resolution           TEXT
);
CREATE INDEX idx_incidents_instance ON incidents (process_instance_id);

-- ---------------------------------------------------------------------------
-- 012: Updated-At Trigger
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_instances_updated_at
    BEFORE UPDATE ON process_instances
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- ---------------------------------------------------------------------------
-- 013: Workflow Templates (Authoring Pipeline — Phase D)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS workflow_templates (
    template_key     VARCHAR(255) NOT NULL,
    template_version INTEGER NOT NULL,
    process_key      VARCHAR(255) NOT NULL,
    bytecode_version VARCHAR(64) NOT NULL,
    state            VARCHAR(20) NOT NULL DEFAULT 'draft'
                     CHECK (state IN ('draft', 'published', 'retired')),
    source_format    VARCHAR(20) NOT NULL DEFAULT 'yaml',
    dto_snapshot     JSONB NOT NULL,
    task_manifest    JSONB NOT NULL DEFAULT '[]',
    bpmn_xml         TEXT,
    summary_md       TEXT,
    verb_registry_hash VARCHAR(64),
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    published_at     TIMESTAMPTZ,
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (template_key, template_version)
);

CREATE INDEX IF NOT EXISTS idx_wf_templates_latest
    ON workflow_templates (template_key, state, template_version DESC)
    WHERE state = 'published';

-- Immutability guard: published content cannot change, retired cannot revert.
CREATE OR REPLACE FUNCTION enforce_template_immutability()
RETURNS TRIGGER AS $$
BEGIN
    -- Published templates: only state change to 'retired' is allowed
    IF OLD.state = 'published' THEN
        IF NEW.state NOT IN ('published', 'retired') THEN
            RAISE EXCEPTION 'Published template cannot revert to %', NEW.state;
        END IF;
        IF NEW.dto_snapshot IS DISTINCT FROM OLD.dto_snapshot
           OR NEW.bytecode_version IS DISTINCT FROM OLD.bytecode_version
           OR NEW.task_manifest IS DISTINCT FROM OLD.task_manifest THEN
            RAISE EXCEPTION 'Published template content is immutable';
        END IF;
    END IF;

    -- Retired templates: no state changes allowed
    IF OLD.state = 'retired' THEN
        IF NEW.state != 'retired' THEN
            RAISE EXCEPTION 'Retired template cannot transition to %', NEW.state;
        END IF;
    END IF;

    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_template_immutability ON workflow_templates;
CREATE TRIGGER trg_template_immutability
    BEFORE UPDATE ON workflow_templates
    FOR EACH ROW
    EXECUTE FUNCTION enforce_template_immutability();
