-- sem_os_roles.sql — Create DB roles for Semantic OS boundary enforcement.
--
-- Three roles:
--   sem_os_owner — owns sem_reg and sem_reg_pub schemas (DDL, admin)
--   sem_os_app   — full access to sem_reg; write access to sem_reg_pub (server process)
--   ob_app       — NO access to sem_reg; read-only on sem_reg_pub (ob-poc process)
--
-- Usage: psql -d data_designer -f sql/sem_os_roles.sql

DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'sem_os_owner') THEN
        CREATE ROLE sem_os_owner NOLOGIN;
    END IF;
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'sem_os_app') THEN
        CREATE ROLE sem_os_app NOLOGIN;
    END IF;
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'ob_app') THEN
        CREATE ROLE ob_app NOLOGIN;
    END IF;
END
$$;
