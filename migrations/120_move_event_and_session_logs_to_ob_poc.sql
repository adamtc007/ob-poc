BEGIN;

ALTER TABLE events.log RENAME CONSTRAINT log_pkey TO event_log_pkey;
ALTER TABLE events.log RENAME CONSTRAINT valid_event_type TO event_log_valid_event_type;
ALTER TABLE events.log RENAME CONSTRAINT log_id_not_null TO event_log_id_not_null;
ALTER TABLE events.log RENAME CONSTRAINT log_timestamp_not_null TO event_log_timestamp_not_null;
ALTER TABLE events.log RENAME CONSTRAINT log_event_type_not_null TO event_log_event_type_not_null;
ALTER TABLE events.log RENAME CONSTRAINT log_payload_not_null TO event_log_payload_not_null;
ALTER INDEX events.idx_events_log_timestamp RENAME TO idx_ob_poc_event_log_timestamp;
ALTER INDEX events.idx_events_log_session RENAME TO idx_ob_poc_event_log_session;
ALTER INDEX events.idx_events_log_failures RENAME TO idx_ob_poc_event_log_failures;

ALTER TABLE sessions.log RENAME CONSTRAINT log_pkey TO session_log_pkey;
ALTER TABLE sessions.log RENAME CONSTRAINT valid_entry_type TO session_log_valid_entry_type;
ALTER TABLE sessions.log RENAME CONSTRAINT valid_source TO session_log_valid_source;
ALTER TABLE sessions.log RENAME CONSTRAINT log_id_not_null TO session_log_id_not_null;
ALTER TABLE sessions.log RENAME CONSTRAINT log_session_id_not_null TO session_log_session_id_not_null;
ALTER TABLE sessions.log RENAME CONSTRAINT log_timestamp_not_null TO session_log_timestamp_not_null;
ALTER TABLE sessions.log RENAME CONSTRAINT log_entry_type_not_null TO session_log_entry_type_not_null;
ALTER TABLE sessions.log RENAME CONSTRAINT log_content_not_null TO session_log_content_not_null;
ALTER TABLE sessions.log RENAME CONSTRAINT log_source_not_null TO session_log_source_not_null;
ALTER TABLE sessions.log RENAME CONSTRAINT log_event_id_fkey TO session_log_event_id_fkey;
ALTER INDEX sessions.idx_sessions_log_session RENAME TO idx_ob_poc_session_log_session;
ALTER INDEX sessions.idx_sessions_log_event RENAME TO idx_ob_poc_session_log_event;

ALTER TABLE events.log RENAME TO event_log;
ALTER TABLE sessions.log RENAME TO session_log;
ALTER SEQUENCE events.log_id_seq RENAME TO event_log_id_seq;
ALTER SEQUENCE sessions.log_id_seq RENAME TO session_log_id_seq;

ALTER TABLE events.event_log SET SCHEMA "ob-poc";
ALTER TABLE sessions.session_log SET SCHEMA "ob-poc";

ALTER VIEW IF EXISTS events.recent_failures SET SCHEMA "ob-poc";
ALTER VIEW IF EXISTS events.session_summary SET SCHEMA "ob-poc";

ALTER FUNCTION events.cleanup_old_events(integer) SET SCHEMA "ob-poc";
ALTER FUNCTION sessions.cleanup_old_logs(integer) RENAME TO cleanup_old_session_logs;
ALTER FUNCTION sessions.cleanup_old_session_logs(integer) SET SCHEMA "ob-poc";

DROP SCHEMA events;
DROP SCHEMA sessions;

COMMIT;
