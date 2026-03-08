BEGIN;

ALTER TYPE feedback.actor_type SET SCHEMA "ob-poc";
ALTER TYPE feedback.audit_action SET SCHEMA "ob-poc";
ALTER TYPE feedback.error_type SET SCHEMA "ob-poc";
ALTER TYPE feedback.issue_status SET SCHEMA "ob-poc";
ALTER TYPE feedback.remediation_path SET SCHEMA "ob-poc";

ALTER TABLE IF EXISTS feedback.failures SET SCHEMA "ob-poc";
ALTER TABLE IF EXISTS feedback.occurrences SET SCHEMA "ob-poc";
ALTER TABLE IF EXISTS feedback.audit_log SET SCHEMA "ob-poc";

ALTER FUNCTION feedback.cleanup_old_resolved() SET SCHEMA "ob-poc";
ALTER FUNCTION feedback.record_occurrence(text, uuid, timestamptz, uuid, text, bigint, text, text) SET SCHEMA "ob-poc";
ALTER FUNCTION feedback.update_timestamps() SET SCHEMA "ob-poc";

ALTER VIEW IF EXISTS feedback.active_issues SET SCHEMA "ob-poc";
ALTER VIEW IF EXISTS feedback.ready_for_todo SET SCHEMA "ob-poc";
ALTER VIEW IF EXISTS feedback.recent_activity SET SCHEMA "ob-poc";

DROP SCHEMA feedback;

COMMIT;
