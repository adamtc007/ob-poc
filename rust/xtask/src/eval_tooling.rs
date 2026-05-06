use anyhow::{bail, Context, Result};
use clap::Subcommand;
use ob_poc_eval_fixtures::{
    drop_eval_fixture_schema_sql, eval_fixture_schema_name, list_eval_fixture_schemas_sql,
    parse_seed_bundle_manifest_yaml, seed_bundle_path, SEED_BUNDLE_DIR,
};
use sqlx::{PgPool, Row};
use std::path::{Path, PathBuf};

#[derive(Subcommand)]
pub enum EvalDbAction {
    /// Drop all schemas matching eval_fixture_*.
    Cleanout {
        /// Show schemas that would be dropped without dropping them.
        #[arg(long)]
        dry_run: bool,
    },
    /// Run database migrations. Wraps the existing migration mechanism later.
    Migrate,
    /// Reset the development database. Wraps the existing reset mechanism later.
    Reset,
    /// Apply an immutable seed bundle to the current database.
    ApplyBundle {
        /// Seed bundle identifier.
        id: String,
    },
    /// Verify the current state snapshot id.
    VerifySnapshot {
        /// Expected SemOS state snapshot identifier.
        expected_state_snapshot_id: String,
    },
    /// Capture a development snapshot. Not used by eval runs.
    Snapshot {
        /// Snapshot name.
        name: String,
    },
    /// Restore a development snapshot. Not used by eval runs.
    Restore {
        /// Snapshot name.
        name: String,
    },
}

#[derive(Subcommand)]
pub enum EvalBundleAction {
    /// Create a local seed-bundle directory skeleton.
    New {
        /// Seed bundle identifier.
        id: String,
    },
    /// Freeze a seed bundle by computing and recording its hash.
    Freeze {
        /// Seed bundle identifier.
        id: String,
    },
    /// List seed bundles.
    List,
    /// Inspect one seed-bundle manifest.
    Inspect {
        /// Seed bundle identifier.
        id: String,
    },
}

#[derive(Subcommand)]
pub enum EvalFixtureAction {
    /// Spin up an eval fixture from a seed bundle.
    SpinUp {
        /// Seed bundle identifier.
        #[arg(long)]
        bundle: String,
        /// Human fixture name, normalized to eval_fixture_<name>.
        #[arg(long)]
        name: String,
    },
    /// Tear down one eval fixture schema.
    TearDown {
        /// Human fixture name or eval_fixture_<name> schema.
        #[arg(long)]
        name: String,
    },
    /// List active eval fixture schemas.
    List,
    /// Drop all schemas matching eval_fixture_*.
    Cleanout {
        /// Show schemas that would be dropped without dropping them.
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
pub enum EvalProbeAction {
    /// Record a live probe response for later deterministic replay.
    Record {
        /// Probe name, such as lei_lookup.
        #[arg(long)]
        probe: String,
        /// Probe arguments, stored as command-line strings for Phase 2a.
        args: Vec<String>,
    },
    /// List recorded probe stubs.
    StubList,
    /// Validate probe stubs referenced by one seed bundle.
    StubValidate {
        /// Seed bundle identifier.
        bundle_id: String,
    },
}

pub async fn run_db(action: EvalDbAction) -> Result<()> {
    match action {
        EvalDbAction::Cleanout { dry_run } => cleanout(dry_run).await,
        EvalDbAction::Migrate => phase_2b_stub("db migrate"),
        EvalDbAction::Reset => phase_2b_stub("db reset"),
        EvalDbAction::ApplyBundle { id } => phase_2b_stub(&format!("db apply-bundle {id}")),
        EvalDbAction::VerifySnapshot {
            expected_state_snapshot_id,
        } => phase_2b_stub(&format!("db verify-snapshot {expected_state_snapshot_id}")),
        EvalDbAction::Snapshot { name } => phase_2b_stub(&format!("db snapshot {name}")),
        EvalDbAction::Restore { name } => phase_2b_stub(&format!("db restore {name}")),
    }
}

pub fn run_bundle(action: EvalBundleAction) -> Result<()> {
    match action {
        EvalBundleAction::New { id } => phase_2b_stub(&format!("bundle new {id}")),
        EvalBundleAction::Freeze { id } => phase_2b_stub(&format!("bundle freeze {id}")),
        EvalBundleAction::List => list_bundles(Path::new(SEED_BUNDLE_DIR)),
        EvalBundleAction::Inspect { id } => inspect_bundle(&id),
    }
}

pub async fn run_fixture(action: EvalFixtureAction) -> Result<()> {
    match action {
        EvalFixtureAction::SpinUp { bundle, name } => {
            let schema = eval_fixture_schema_name(&name)?;
            phase_2b_stub(&format!(
                "fixture spin-up --bundle {bundle} --name {schema}"
            ))
        }
        EvalFixtureAction::TearDown { name } => tear_down(&name).await,
        EvalFixtureAction::List => list_fixtures().await,
        EvalFixtureAction::Cleanout { dry_run } => cleanout(dry_run).await,
    }
}

pub fn run_probe(action: EvalProbeAction) -> Result<()> {
    match action {
        EvalProbeAction::Record { probe, args } => {
            phase_2b_stub(&format!("probe record --probe {probe} {}", args.join(" ")))
        }
        EvalProbeAction::StubList => phase_2b_stub("probe stub list"),
        EvalProbeAction::StubValidate { bundle_id } => {
            phase_2b_stub(&format!("probe stub validate {bundle_id}"))
        }
    }
}

fn phase_2b_stub(command: &str) -> Result<()> {
    bail!(
        "`cargo x {command}` is registered for Sage Eval Harness Phase 2b; \
         the underlying fixture library implementation lands in Phase 2a"
    )
}

fn list_bundles(seed_bundle_dir: &Path) -> Result<()> {
    if !seed_bundle_dir.exists() {
        println!("No seed bundles found.");
        return Ok(());
    }

    let mut bundle_ids = Vec::new();
    for entry in std::fs::read_dir(seed_bundle_dir)
        .with_context(|| format!("failed to read {}", seed_bundle_dir.display()))?
    {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            bundle_ids.push(entry.file_name().to_string_lossy().to_string());
        }
    }

    bundle_ids.sort();
    for bundle_id in &bundle_ids {
        println!("{bundle_id}");
    }
    if bundle_ids.is_empty() {
        println!("No seed bundles found.");
    }
    Ok(())
}

fn inspect_bundle(bundle_id: &str) -> Result<()> {
    let manifest_path = seed_bundle_path(bundle_id).join("manifest.yaml");
    let contents = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest = parse_seed_bundle_manifest_yaml(&contents)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;

    println!("bundle_id: {}", manifest.bundle_id);
    println!("configuration_version: {}", manifest.configuration_version);
    println!(
        "domain_pack: {}@{}",
        manifest.domain_pack_id, manifest.domain_pack_version
    );
    println!("state_snapshot_id: {}", manifest.state_snapshot_id);
    println!("frozen: {}", manifest.frozen);
    if let Some(checksum) = manifest.checksum {
        println!("checksum: {checksum}");
    }
    Ok(())
}

async fn list_fixtures() -> Result<()> {
    let pool = connect_pool().await?;
    let schemas = list_fixture_schemas(&pool).await?;
    for schema in &schemas {
        println!("{schema}");
    }
    if schemas.is_empty() {
        println!("No eval fixtures found.");
    }
    Ok(())
}

async fn tear_down(name: &str) -> Result<()> {
    let schema = eval_fixture_schema_name(name)?;
    let pool = connect_pool().await?;
    drop_schema(&pool, &schema).await?;
    println!("Dropped {schema}");
    Ok(())
}

async fn cleanout(dry_run: bool) -> Result<()> {
    let pool = connect_pool().await?;
    let schemas = list_fixture_schemas(&pool).await?;
    if schemas.is_empty() {
        println!("No eval fixtures found.");
        return Ok(());
    }

    for schema in &schemas {
        if dry_run {
            println!("Would drop {schema}");
        } else {
            drop_schema(&pool, schema).await?;
            println!("Dropped {schema}");
        }
    }
    Ok(())
}

async fn connect_pool() -> Result<PgPool> {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".into());
    PgPool::connect(&database_url)
        .await
        .with_context(|| format!("failed to connect to {database_url}"))
}

async fn list_fixture_schemas(pool: &PgPool) -> Result<Vec<String>> {
    let rows = sqlx::query(list_eval_fixture_schemas_sql())
        .fetch_all(pool)
        .await?;
    let mut schemas = Vec::with_capacity(rows.len());
    for row in rows {
        schemas.push(row.try_get::<String, _>("nspname")?);
    }
    Ok(schemas)
}

async fn drop_schema(pool: &PgPool, schema: &str) -> Result<()> {
    let sql = drop_eval_fixture_schema_sql(schema)?;
    sqlx::query(&sql).execute(pool).await?;
    Ok(())
}

#[allow(dead_code)]
fn eval_workspace_root() -> Result<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .context("xtask manifest dir is not under rust/xtask")?;
    Ok(repo_root.join("eval"))
}
