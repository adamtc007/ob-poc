-- EOP-PLAN-KYCUBO-KIT-T7 §7/§8 gameboard-vocabulary adoption — widens
-- kyc_ramp_capture.user_action from the charter's original 3-value scope
-- (accepted/edited/rejected) to semantic-decision-contracts::MoveAttemptOutcome's
-- full 10-value closed set, per EOP-DD-KYCUBO-CAPTURE-CHARTER v0.1 §1
-- Amendment (2026-08-17) — this widening is charter-governed, not a silent
-- ALTER TABLE (I-6).
--
-- Only 3 of the 10 values are produced by any call site today
-- (applied/corrected/compiler_refused — see kyc_workbook_surface.rs's Stage
-- arm); the rest are adopted-but-unpopulated, matching the "list the whole
-- taxonomy even when not every arm has code behind it yet" convention
-- already used for verb-YAML valid_values (CLAUDE.md).

ALTER TABLE "ob-poc".kyc_ramp_capture
    DROP CONSTRAINT kyc_ramp_capture_user_action_check;

ALTER TABLE "ob-poc".kyc_ramp_capture
    ADD CONSTRAINT kyc_ramp_capture_user_action_check
    CHECK (user_action IN (
        'applied',
        'incomplete',
        'ambiguous',
        'inapplicable',
        'disclosure_safe_refusal',
        'stale',
        'compiler_refused',
        'rejected_by_user',
        'corrected',
        'system_failure'
    ));

COMMENT ON COLUMN "ob-poc".kyc_ramp_capture.staged_move_id IS
    'Joins back to the governed append-protocol record it produced, without duplicating that record. NULL unless user_action IN (''applied'', ''corrected'') — both are successful stages, per EOP-DD-KYCUBO-CAPTURE-CHARTER v0.1 §1 Amendment (2026-08-17).';
