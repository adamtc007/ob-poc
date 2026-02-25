//! Permission harness tests (Stage 2.3).
//!
//! Tests that the DB boundary is enforced:
//! - ob_app role CANNOT read sem_reg.snapshots.
//! - ob_app role CAN read sem_reg_pub.active_verb_contracts.

use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use std::str::FromStr;

/// Run the permission scenario suite.
///
/// `admin_pool` is connected as a superuser/owner (used to set up roles + privileges).
/// `admin_url` is the base connection string (used to derive role-specific connections).
///
/// **Prerequisites:** The roles and privileges SQL scripts must have been applied to the
/// test database before running these tests. The caller (test harness) is responsible for
/// running `sem_os_roles.sql` and `sem_os_privileges.sql` against the test DB.
pub async fn run_permission_scenario_suite(admin_pool: &PgPool, admin_url: &str, dbname: &str) {
    // First, ensure the roles exist and privileges are applied in the test DB.
    setup_roles_and_privileges(admin_pool).await;

    // Test: ob_app cannot read sem_reg.snapshots.
    test_ob_app_cannot_read_sem_reg(admin_pool, admin_url, dbname).await;

    // Test: ob_app can read sem_reg_pub.active_verb_contracts.
    test_ob_app_can_read_sem_reg_pub(admin_pool, admin_url, dbname).await;
}

async fn setup_roles_and_privileges(pool: &PgPool) {
    // Create roles if they don't exist.
    sqlx::query(
        r#"
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
            -- Also create a login role for ob_app testing.
            IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'ob_app_login') THEN
                CREATE ROLE ob_app_login LOGIN;
            END IF;
        END
        $$;
        "#,
    )
    .execute(pool)
    .await
    .expect("role creation failed");

    // Grant ob_app to ob_app_login so we can connect as that role.
    sqlx::query("GRANT ob_app TO ob_app_login")
        .execute(pool)
        .await
        .expect("GRANT ob_app TO ob_app_login failed");

    // Apply privilege assignments.
    let privilege_statements = [
        "ALTER SCHEMA sem_reg OWNER TO sem_os_owner",
        "ALTER SCHEMA sem_reg_pub OWNER TO sem_os_owner",
        "GRANT USAGE ON SCHEMA sem_reg TO sem_os_app",
        "GRANT ALL ON ALL TABLES IN SCHEMA sem_reg TO sem_os_app",
        "GRANT USAGE ON SCHEMA sem_reg_pub TO sem_os_app",
        "GRANT ALL ON ALL TABLES IN SCHEMA sem_reg_pub TO sem_os_app",
        "REVOKE ALL ON SCHEMA sem_reg FROM ob_app",
        "REVOKE ALL ON ALL TABLES IN SCHEMA sem_reg FROM ob_app",
        "GRANT USAGE ON SCHEMA sem_reg_pub TO ob_app",
        "GRANT SELECT ON ALL TABLES IN SCHEMA sem_reg_pub TO ob_app",
    ];

    for stmt in &privilege_statements {
        sqlx::query(stmt)
            .execute(pool)
            .await
            .unwrap_or_else(|e| panic!("privilege statement failed: {stmt}: {e}"));
    }
}

/// Connect as ob_app_login role and assert that SELECT from sem_reg.snapshots fails.
async fn test_ob_app_cannot_read_sem_reg(_admin_pool: &PgPool, admin_url: &str, dbname: &str) {
    tracing::info!("test_ob_app_cannot_read_sem_reg: starting");

    let ob_pool = connect_as_role(admin_url, dbname, "ob_app_login").await;

    let result = sqlx::query("SELECT 1 FROM sem_reg.snapshots LIMIT 1")
        .fetch_optional(&ob_pool)
        .await;

    match result {
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("permission denied") || msg.contains("does not exist"),
                "expected permission denied, got: {msg}"
            );
            tracing::info!("test_ob_app_cannot_read_sem_reg: correctly denied ({msg})");
        }
        Ok(_) => {
            panic!("test_ob_app_cannot_read_sem_reg: SECURITY BUG — ob_app could read sem_reg.snapshots!");
        }
    }

    ob_pool.close().await;
    tracing::info!("test_ob_app_cannot_read_sem_reg: passed");
}

/// Connect as ob_app_login role and assert that SELECT from sem_reg_pub.active_verb_contracts succeeds.
async fn test_ob_app_can_read_sem_reg_pub(_admin_pool: &PgPool, admin_url: &str, dbname: &str) {
    tracing::info!("test_ob_app_can_read_sem_reg_pub: starting");

    let ob_pool = connect_as_role(admin_url, dbname, "ob_app_login").await;

    let result = sqlx::query("SELECT 1 FROM sem_reg_pub.active_verb_contracts LIMIT 1")
        .fetch_optional(&ob_pool)
        .await;

    match result {
        Ok(_) => {
            tracing::info!("test_ob_app_can_read_sem_reg_pub: correctly allowed");
        }
        Err(e) => {
            panic!("test_ob_app_can_read_sem_reg_pub: ob_app should be able to read sem_reg_pub but got: {e}");
        }
    }

    ob_pool.close().await;
    tracing::info!("test_ob_app_can_read_sem_reg_pub: passed");
}

/// Connect to a specific database as a specific role.
async fn connect_as_role(admin_url: &str, dbname: &str, role: &str) -> PgPool {
    let opts = PgConnectOptions::from_str(admin_url)
        .expect("admin_url parse failed")
        .database(dbname)
        .username(role);

    PgPoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap_or_else(|e| panic!("connect as {role} failed: {e}"))
}
