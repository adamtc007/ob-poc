BEGIN;

ALTER TABLE IF EXISTS teams.teams SET SCHEMA "ob-poc";
ALTER TABLE IF EXISTS teams.memberships SET SCHEMA "ob-poc";
ALTER TABLE IF EXISTS teams.team_cbu_access SET SCHEMA "ob-poc";
ALTER TABLE IF EXISTS teams.team_service_entitlements SET SCHEMA "ob-poc";
ALTER TABLE IF EXISTS teams.access_review_campaigns SET SCHEMA "ob-poc";
ALTER TABLE IF EXISTS teams.access_review_items SET SCHEMA "ob-poc";
ALTER TABLE IF EXISTS teams.access_attestations SET SCHEMA "ob-poc";

ALTER FUNCTION teams.generate_attestation_signature(uuid, uuid, uuid[], text, timestamptz) SET SCHEMA "ob-poc";
ALTER FUNCTION teams.get_user_access_domains(uuid) SET SCHEMA "ob-poc";
ALTER FUNCTION teams.get_user_cbu_access(uuid) SET SCHEMA "ob-poc";
ALTER FUNCTION teams.log_membership_change() RENAME TO teams_log_membership_change;
ALTER FUNCTION teams.teams_log_membership_change() SET SCHEMA "ob-poc";
ALTER FUNCTION teams.update_timestamp() RENAME TO teams_update_timestamp;
ALTER FUNCTION teams.teams_update_timestamp() SET SCHEMA "ob-poc";
ALTER FUNCTION teams.user_can_access_cbu(uuid, uuid) SET SCHEMA "ob-poc";
ALTER FUNCTION teams.user_has_domain(uuid, character varying) SET SCHEMA "ob-poc";

ALTER VIEW IF EXISTS teams.v_campaign_dashboard SET SCHEMA "ob-poc";
ALTER VIEW IF EXISTS teams.v_flagged_items_summary SET SCHEMA "ob-poc";
ALTER VIEW IF EXISTS teams.v_reviewer_workload SET SCHEMA "ob-poc";

DROP SCHEMA teams;

COMMIT;
