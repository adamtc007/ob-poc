-- Migration 098: Stewardship Phase 1 — Show Loop tables
--
-- Depends on: 097_stewardship_phase0.sql (stewardship schema)
-- Spec refs: §9.14.1 (FocusState), §9.4 (ViewportManifest)

-- 1. Focus state — server-side shared truth (spec §9.14.1)
CREATE TABLE IF NOT EXISTS stewardship.focus_states (
  session_id         UUID PRIMARY KEY,
  changeset_id       UUID REFERENCES sem_reg.changesets(changeset_id),
  overlay_mode       VARCHAR(20) NOT NULL DEFAULT 'active_only'
    CHECK (overlay_mode IN ('active_only','draft_overlay')),
  overlay_changeset_id UUID,  -- populated when overlay_mode = 'draft_overlay'
  object_refs        JSONB NOT NULL DEFAULT '[]',   -- Vec<ObjectRef> (multiple selection)
  taxonomy_focus     JSONB,                          -- Optional TaxonomyFocus
  resolution_context JSONB,                          -- Optional ResolutionContext
  updated_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_by         VARCHAR(20) NOT NULL DEFAULT 'agent'
    CHECK (updated_by IN ('agent','user_navigation'))
);

-- 2. Viewport manifests — immutable audit records (spec §9.4)
CREATE TABLE IF NOT EXISTS stewardship.viewport_manifests (
  manifest_id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  session_id         UUID NOT NULL,
  changeset_id       UUID,
  focus_state        JSONB NOT NULL,              -- snapshot of FocusState at capture time
  overlay_mode       VARCHAR(20) NOT NULL,
  assumed_principal   VARCHAR(200),                -- ABAC impersonation context (§2.3.4)
  viewport_refs      JSONB NOT NULL DEFAULT '[]', -- Vec<ViewportRef> with data_hash + registry_version
  created_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_viewport_manifests_session
  ON stewardship.viewport_manifests (session_id, created_at DESC);
