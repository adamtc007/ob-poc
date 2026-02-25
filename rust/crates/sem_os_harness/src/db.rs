//! Isolated test database helpers (SC-4).
//!
//! Each harness run creates a temporary database via CREATE DATABASE,
//! runs only the sem_reg migrations into it, and drops it on cleanup.

use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use std::path::Path;
use std::str::FromStr;

/// Holds the test database pool, name, and admin connection for cleanup.
pub struct IsolatedDb {
    /// Pool connected to the isolated test database.
    pub pool: PgPool,
    /// Name of the temporary database.
    pub dbname: String,
    /// Admin pool connected to the control database (for CREATE/DROP).
    admin: PgPool,
}

/// Migration filename prefixes that belong to the sem_reg subsystem.
/// This avoids running the full ob-poc migration set (which requires
/// `ob-poc`, `agent`, etc. schemas) for Semantic OS harness tests.
const SEM_REG_PREFIXES: &[&str] = &[
    "078_", "079_", "081_", "082_", "083_", "084_", "085_", "086_", "090_", "091_", "092_", "093_",
    "094_", "095_",
];

fn is_sem_reg_migration(filename: &str) -> bool {
    SEM_REG_PREFIXES.iter().any(|p| filename.starts_with(p))
}

/// Create an isolated test database, run sem_reg migrations, and return handles.
///
/// `admin_url` should point to a database that allows CREATE/DROP DATABASE
/// (typically `postgresql:///postgres` or `postgresql:///data_designer`).
pub async fn isolated_db(admin_url: &str) -> IsolatedDb {
    let dbname = format!("sem_os_test_{}", uuid::Uuid::new_v4().simple());

    let admin_opts = PgConnectOptions::from_str(admin_url).expect("admin_url parse failed");
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(admin_opts)
        .await
        .expect("admin connect failed");

    sqlx::query(&format!(r#"CREATE DATABASE "{}""#, dbname))
        .execute(&admin)
        .await
        .expect("CREATE DATABASE failed");

    let test_opts = PgConnectOptions::from_str(admin_url)
        .expect("admin_url parse failed")
        .database(&dbname);

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect_with(test_opts)
        .await
        .expect("test db connect failed");

    // Only run sem_reg migrations — the harness tests don't need the full
    // ob-poc schema (which has non-standard filenames and cross-schema deps).
    let migrations_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../migrations");
    run_sem_reg_migrations(&pool, &migrations_dir).await;

    IsolatedDb {
        pool,
        dbname,
        admin,
    }
}

/// Run only sem_reg `.sql` migration files in sorted order.
async fn run_sem_reg_migrations(pool: &PgPool, dir: &Path) {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read migrations dir {:?}: {}", dir, e))
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".sql") && is_sem_reg_migration(&name) {
                Some((name, entry.path()))
            } else {
                None
            }
        })
        .collect();

    files.sort_by(|a, b| a.0.cmp(&b.0));

    for (name, path) in &files {
        let sql = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("cannot read migration {}: {}", name, e));
        sqlx::raw_sql(&sql)
            .execute(pool)
            .await
            .unwrap_or_else(|e| panic!("migration {} failed: {}", name, e));
    }
}

/// Drop the isolated test database. Call this in cleanup, even on failure.
pub async fn drop_db(iso: IsolatedDb) {
    // Close the test pool first so connections don't block the DROP.
    iso.pool.close().await;

    // Postgres 13+ supports FORCE to drop even if connections linger.
    let drop_sql = format!(r#"DROP DATABASE IF EXISTS "{}" WITH (FORCE)"#, iso.dbname);
    let _ = sqlx::query(&drop_sql).execute(&iso.admin).await;

    iso.admin.close().await;
}
