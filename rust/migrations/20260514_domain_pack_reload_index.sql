CREATE TABLE IF NOT EXISTS sem_reg.domain_pack_reload_index (
    pack_id text PRIMARY KEY,
    source_fingerprints jsonb NOT NULL DEFAULT '[]'::jsonb,
    surface_hash text NOT NULL,
    snapshot_set_id uuid,
    last_checked_at timestamptz NOT NULL DEFAULT now(),
    last_loaded_at timestamptz,
    status text NOT NULL,
    diagnostics jsonb NOT NULL DEFAULT '[]'::jsonb,
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT domain_pack_reload_index_status_chk CHECK (
        status IN ('clean', 'loaded', 'index_only', 'publish_required')
    )
);
