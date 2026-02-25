-- sem_os_privileges.sql — Assign privileges for Semantic OS boundary enforcement.
--
-- Requires: sem_os_roles.sql must be run first.
-- Requires: schemas sem_reg and sem_reg_pub must exist (created by migrations).
--
-- Usage: psql -d data_designer -f sql/sem_os_privileges.sql

-- sem_os_owner owns the schemas.
ALTER SCHEMA sem_reg OWNER TO sem_os_owner;
ALTER SCHEMA sem_reg_pub OWNER TO sem_os_owner;

-- sem_os_app: full access to sem_reg; write access to sem_reg_pub.
GRANT USAGE ON SCHEMA sem_reg TO sem_os_app;
GRANT ALL ON ALL TABLES IN SCHEMA sem_reg TO sem_os_app;
GRANT USAGE ON ALL SEQUENCES IN SCHEMA sem_reg TO sem_os_app;

GRANT USAGE ON SCHEMA sem_reg_pub TO sem_os_app;
GRANT ALL ON ALL TABLES IN SCHEMA sem_reg_pub TO sem_os_app;
GRANT USAGE ON ALL SEQUENCES IN SCHEMA sem_reg_pub TO sem_os_app;

-- Default privileges so future tables get the same grants.
ALTER DEFAULT PRIVILEGES IN SCHEMA sem_reg GRANT ALL ON TABLES TO sem_os_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA sem_reg_pub GRANT ALL ON TABLES TO sem_os_app;

-- ob_app: NO access to sem_reg (not even SELECT).
REVOKE ALL ON SCHEMA sem_reg FROM ob_app;
REVOKE ALL ON ALL TABLES IN SCHEMA sem_reg FROM ob_app;

-- ob_app: read-only on sem_reg_pub.
GRANT USAGE ON SCHEMA sem_reg_pub TO ob_app;
GRANT SELECT ON ALL TABLES IN SCHEMA sem_reg_pub TO ob_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA sem_reg_pub GRANT SELECT ON TABLES TO ob_app;
