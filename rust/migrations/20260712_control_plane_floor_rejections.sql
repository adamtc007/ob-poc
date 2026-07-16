-- T11.F.2 (EOP-PLAN-CONTROLPLANE-002): the definitional floor's audit
-- record. Extends control_plane_shadow_decisions rather than a new table
-- (T11.F.2 design doc §4) -- a floor rejection carries the same shape a
-- shadow observation does (session/entry/verb/gate outcome), it just also
-- actually blocked dispatch.
--
-- floor_rejected defaults false: every pre-existing and every ordinary
-- shadow-only row is unaffected. floor_gate/floor_reason are NULL unless
-- floor_rejected is true -- a floor rejection's audit record is required
-- to name which gate and why (T11.F.2 design doc §4's "controlled work
-- item, not an uncontrolled failure" requirement), enforced by the CHECK
-- constraint below rather than left to application discipline alone.
ALTER TABLE "ob-poc".control_plane_shadow_decisions
    ADD COLUMN floor_rejected BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN floor_gate TEXT,
    ADD COLUMN floor_reason TEXT;

ALTER TABLE "ob-poc".control_plane_shadow_decisions
    ADD CONSTRAINT control_plane_shadow_decisions_floor_reason_check
    CHECK (
        (floor_rejected = false AND floor_gate IS NULL AND floor_reason IS NULL)
        OR (floor_rejected = true AND floor_gate IS NOT NULL AND floor_reason IS NOT NULL)
    );

CREATE INDEX idx_control_plane_shadow_decisions_floor_rejected
    ON "ob-poc".control_plane_shadow_decisions (floor_rejected, decided_at DESC)
    WHERE floor_rejected;

COMMENT ON COLUMN "ob-poc".control_plane_shadow_decisions.floor_rejected IS
    'T11.F.2: true iff the definitional floor (G1 registry-absence, G3 MissingPack/AmbiguousPack, G4 topological/blocking_violations) actually blocked this dispatch -- distinguishes a real hard rejection from an ordinary shadow-only observation in the same table.';
COMMENT ON COLUMN "ob-poc".control_plane_shadow_decisions.floor_gate IS
    'Which gate fired the floor rejection: "G1" | "G3" | "G4". NULL unless floor_rejected.';
COMMENT ON COLUMN "ob-poc".control_plane_shadow_decisions.floor_reason IS
    'Human-readable reason the floor fired (e.g. the missing verb_fqn, the pack-resolution outcome, the blocking_violations join). NULL unless floor_rejected.';
