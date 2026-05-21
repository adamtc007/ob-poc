-- Journey-persisted bpmn-lite runtime schema (Tranche 6)
-- All tables prefixed with dsl_ to avoid conflicts with existing ob-poc tables.
-- These tables are schema-unqualified (no "ob-poc". prefix) because the
-- journey runtime is a standalone subsystem that runs its own schema.

CREATE TABLE dsl_workflow_instance (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    journey_name    TEXT NOT NULL,
    version         INTEGER NOT NULL DEFAULT 1,
    status          TEXT NOT NULL DEFAULT 'active', -- active | completed | failed | cancelled
    started_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at    TIMESTAMPTZ,
    data            JSONB NOT NULL DEFAULT '{}'
);

-- Append-only audit log of every state transition
CREATE TABLE dsl_journey_log (
    id              BIGSERIAL PRIMARY KEY,
    instance_id     UUID NOT NULL REFERENCES dsl_workflow_instance(id),
    token_id        UUID,
    event_kind      TEXT NOT NULL,
    from_node       TEXT,
    to_node         TEXT,
    data_delta      JSONB,
    recorded_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX dsl_journey_log_instance ON dsl_journey_log(instance_id, id);

-- One row per live token (a token = one active path of execution)
CREATE TABLE dsl_active_token (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instance_id     UUID NOT NULL REFERENCES dsl_workflow_instance(id),
    current_node    TEXT NOT NULL,
    fork_ref        UUID,             -- gateway that spawned this token (parallel/inclusive fork)
    branch_lineage  TEXT[],           -- ordered list of fork gateway names from root
    write_log       JSONB NOT NULL DEFAULT '[]',  -- array of {location, value} objects this token has written
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX dsl_active_token_instance ON dsl_active_token(instance_id);

-- Versioned key-value store for instance application data
CREATE TABLE dsl_instance_data (
    instance_id     UUID NOT NULL REFERENCES dsl_workflow_instance(id),
    key             TEXT NOT NULL,
    value           JSONB,
    version         INTEGER NOT NULL DEFAULT 1,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (instance_id, key)
);

-- Instances waiting for an external event
CREATE TABLE dsl_pending_wait (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instance_id     UUID NOT NULL REFERENCES dsl_workflow_instance(id),
    token_id        UUID NOT NULL REFERENCES dsl_active_token(id),
    wait_kind       TEXT NOT NULL,   -- timer | message | human_task | switch_decision
    node_name       TEXT NOT NULL,
    correlation_key TEXT,
    timeout_at      TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX dsl_pending_wait_correlation ON dsl_pending_wait(wait_kind, correlation_key) WHERE correlation_key IS NOT NULL;

-- Scheduled timers
CREATE TABLE dsl_pending_timer (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instance_id     UUID NOT NULL REFERENCES dsl_workflow_instance(id),
    wait_id         UUID NOT NULL REFERENCES dsl_pending_wait(id),
    fires_at        TIMESTAMPTZ NOT NULL,
    fired           BOOLEAN NOT NULL DEFAULT FALSE
);
CREATE INDEX dsl_pending_timer_fires_at ON dsl_pending_timer(fires_at) WHERE NOT fired;

-- Gateway decision requests (switch adaptor protocol)
CREATE TABLE dsl_switch_decision_request (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instance_id     UUID NOT NULL REFERENCES dsl_workflow_instance(id),
    token_id        UUID NOT NULL REFERENCES dsl_active_token(id),
    gateway_name    TEXT NOT NULL,
    gateway_kind    TEXT NOT NULL,
    context_data    JSONB NOT NULL DEFAULT '{}',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Inbound event queue (FOR UPDATE SKIP LOCKED)
CREATE TABLE dsl_event_queue (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instance_id     UUID NOT NULL REFERENCES dsl_workflow_instance(id),
    event_kind      TEXT NOT NULL,
    payload         JSONB NOT NULL DEFAULT '{}',
    enqueued_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    claimed_at      TIMESTAMPTZ,
    claim_token     UUID,
    attempts        INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX dsl_event_queue_unclaimed ON dsl_event_queue(enqueued_at) WHERE claimed_at IS NULL;

-- Parallel join arrival tracking
CREATE TABLE dsl_join_arrival (
    join_name       TEXT NOT NULL,
    instance_id     UUID NOT NULL REFERENCES dsl_workflow_instance(id),
    token_id        UUID NOT NULL REFERENCES dsl_active_token(id),
    arrived_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (join_name, instance_id, token_id)
);
