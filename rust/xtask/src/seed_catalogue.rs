use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};
use ob_poc::sequencer_tx::PgTransactionScope;
use sem_os_core::principal::Principal;
use sem_os_postgres::ops::{build_registry, SemOsVerbOpRegistry};
use serde_json::{json, Map, Value};
use sqlx::PgPool;

const SEED_TABLES: &[&str] = &[
    "resource_owner_principals",
    "service_resource_types",
    "services",
    "products",
    "service_versions",
    "product_services",
    "service_resource_capabilities",
];

/// Replay the Phase 1 catalogue genesis snapshot through verb dispatch.
///
/// # Examples
///
/// ```text
/// cargo x seed-catalogue --dry-run
/// DATABASE_URL=postgresql:///data_designer cargo x seed-catalogue
/// ```
pub(crate) async fn run(seed_dir: Option<PathBuf>, dry_run: bool) -> Result<()> {
    let seed_dir = seed_dir.unwrap_or_else(default_seed_dir);
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".into());
    let pool = PgPool::connect(&database_url)
        .await
        .with_context(|| format!("connecting to {database_url}"))?;
    let registry = registry();
    let principal = Principal::in_process(
        "catalogue-seed",
        vec![
            "admin".to_string(),
            "resource_owner".to_string(),
            "compliance_admin".to_string(),
        ],
    );
    let mut ctx = VerbExecutionContext::new(principal);
    let mut scope = PgTransactionScope::begin(&pool).await?;

    let resource_owner_principals = load_copy_table(
        &seed_dir.join("resource_owner_principals.sql"),
        "resource_owner_principals",
    )?;
    let service_resource_types = load_copy_table(
        &seed_dir.join("service_resource_types.sql"),
        "service_resource_types",
    )?;
    let services = load_copy_table(&seed_dir.join("services.sql"), "services")?;
    let products = load_copy_table(&seed_dir.join("products.sql"), "products")?;
    let service_versions =
        load_copy_table(&seed_dir.join("service_versions.sql"), "service_versions")?;
    let product_services =
        load_copy_table(&seed_dir.join("product_services.sql"), "product_services")?;
    let service_resource_capabilities = load_copy_table(
        &seed_dir.join("service_resource_capabilities.sql"),
        "service_resource_capabilities",
    )?;

    replay_rows(
        &registry,
        "resource-owner.assign",
        resource_owner_principals
            .rows
            .iter()
            .map(resource_owner_args),
        &mut ctx,
        &mut scope,
    )
    .await
    .context("replaying resource_owner_principals through resource-owner.assign")?;
    replay_rows(
        &registry,
        "service-resource.define-type",
        service_resource_types
            .rows
            .iter()
            .map(service_resource_type_args),
        &mut ctx,
        &mut scope,
    )
    .await
    .context("replaying service_resource_types through service-resource.define-type")?;
    replay_rows(
        &registry,
        "service.define",
        services.rows.iter().map(service_args),
        &mut ctx,
        &mut scope,
    )
    .await
    .context("replaying services through service.define")?;
    replay_rows(
        &registry,
        "product.define",
        products.rows.iter().map(product_args),
        &mut ctx,
        &mut scope,
    )
    .await
    .context("replaying products through product.define")?;
    replay_rows(
        &registry,
        "service-version.draft",
        service_versions.rows.iter().map(service_version_args),
        &mut ctx,
        &mut scope,
    )
    .await
    .context("replaying service_versions through service-version.draft")?;
    replay_rows(
        &registry,
        "product-service.link",
        product_services.rows.iter().map(product_service_args),
        &mut ctx,
        &mut scope,
    )
    .await
    .context("replaying product_services through product-service.link")?;
    replay_rows(
        &registry,
        "service-resource.add-capability",
        service_resource_capabilities
            .rows
            .iter()
            .map(service_resource_capability_args),
        &mut ctx,
        &mut scope,
    )
    .await
    .context("replaying service_resource_capabilities through service-resource.add-capability")?;

    println!("catalogue seed input rows:");
    println!(
        "  resource_owner_principals: {}",
        resource_owner_principals.rows.len()
    );
    println!(
        "  service_resource_types: {}",
        service_resource_types.rows.len()
    );
    println!("  services: {}", services.rows.len());
    println!("  products: {}", products.rows.len());
    println!("  service_versions: {}", service_versions.rows.len());
    println!("  product_services: {}", product_services.rows.len());
    println!(
        "  service_resource_capabilities: {}",
        service_resource_capabilities.rows.len()
    );

    if dry_run {
        scope.rollback().await?;
        println!("dry run: transaction rolled back");
        return Ok(());
    }

    scope.commit().await?;
    println!("committed catalogue seed replay");
    println!("live row counts:");
    for table in SEED_TABLES {
        let count = live_count(&pool, table).await?;
        println!("  {table}: {count}");
    }

    Ok(())
}

fn default_seed_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask manifest dir has a parent rust directory")
        .join("seeds/phase1_genesis")
}

fn registry() -> SemOsVerbOpRegistry {
    let mut registry = build_registry();
    ob_poc::domain_ops::extend_registry(&mut registry);
    registry
}

async fn replay_rows<I>(
    registry: &SemOsVerbOpRegistry,
    fqn: &str,
    rows: I,
    ctx: &mut VerbExecutionContext,
    scope: &mut PgTransactionScope,
) -> Result<()>
where
    I: IntoIterator<Item = Result<Value>>,
{
    let op = registry
        .get(fqn)
        .ok_or_else(|| anyhow!("missing registered op {fqn}"))?;
    for row in rows {
        let args = row?;
        match op.execute(&args, ctx, scope).await? {
            VerbExecutionOutcome::Uuid(_)
            | VerbExecutionOutcome::Record(_)
            | VerbExecutionOutcome::Affected(_)
            | VerbExecutionOutcome::Void => {}
            VerbExecutionOutcome::RecordSet(records) => {
                return Err(anyhow!(
                    "{fqn} returned {} records while replaying a seed row",
                    records.len()
                ));
            }
        }
    }
    Ok(())
}

async fn live_count(pool: &PgPool, table: &str) -> Result<i64> {
    let sql = match table {
        "resource_owner_principals" => r#"SELECT count(*) FROM "ob-poc".resource_owner_principals"#,
        "service_resource_types" => r#"SELECT count(*) FROM "ob-poc".service_resource_types"#,
        "services" => r#"SELECT count(*) FROM "ob-poc".services"#,
        "products" => r#"SELECT count(*) FROM "ob-poc".products"#,
        "service_versions" => r#"SELECT count(*) FROM "ob-poc".service_versions"#,
        "product_services" => r#"SELECT count(*) FROM "ob-poc".product_services"#,
        "service_resource_capabilities" => {
            r#"SELECT count(*) FROM "ob-poc".service_resource_capabilities"#
        }
        other => return Err(anyhow!("unsupported seed table {other}")),
    };
    Ok(sqlx::query_scalar(sql).fetch_one(pool).await?)
}

#[derive(Debug)]
struct CopyTable {
    rows: Vec<CopyRow>,
}

#[derive(Debug)]
struct CopyRow {
    values: BTreeMap<String, Option<String>>,
}

impl CopyRow {
    fn field(&self, column: &str) -> Result<Option<&str>> {
        self.values
            .get(column)
            .map(Option::as_deref)
            .ok_or_else(|| anyhow!("seed row missing column {column}"))
    }

    fn required_field(&self, column: &str) -> Result<&str> {
        self.field(column)?
            .ok_or_else(|| anyhow!("seed row has NULL required column {column}"))
    }
}

fn load_copy_table(path: &Path, table_name: &str) -> Result<CopyTable> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut lines = content.lines();

    let copy_line = lines
        .by_ref()
        .find(|line| line.starts_with("COPY "))
        .ok_or_else(|| anyhow!("{} does not contain a COPY block", path.display()))?;
    if !copy_line.contains(table_name) {
        return Err(anyhow!(
            "{} COPY block does not target {table_name}: {copy_line}",
            path.display()
        ));
    }
    let columns = parse_copy_columns(copy_line)?;
    let mut rows = Vec::new();
    for line in lines {
        if line == r"\." {
            break;
        }
        let fields = line
            .split('\t')
            .map(decode_copy_field)
            .collect::<Result<Vec<_>>>()?;
        if fields.len() != columns.len() {
            return Err(anyhow!(
                "{} row has {} fields for {} columns",
                path.display(),
                fields.len(),
                columns.len()
            ));
        }
        let values = columns.iter().cloned().zip(fields).collect();
        rows.push(CopyRow { values });
    }

    Ok(CopyTable { rows })
}

fn parse_copy_columns(copy_line: &str) -> Result<Vec<String>> {
    let open = copy_line
        .find('(')
        .ok_or_else(|| anyhow!("COPY line has no column list: {copy_line}"))?;
    let close = copy_line
        .find(") FROM stdin")
        .ok_or_else(|| anyhow!("COPY line has no FROM stdin suffix: {copy_line}"))?;
    Ok(copy_line[open + 1..close]
        .split(',')
        .map(str::trim)
        .map(ToOwned::to_owned)
        .collect())
}

fn decode_copy_field(raw: &str) -> Result<Option<String>> {
    if raw == r"\N" {
        return Ok(None);
    }
    let mut decoded = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            decoded.push(ch);
            continue;
        }
        match chars.next() {
            Some('b') => decoded.push('\u{0008}'),
            Some('f') => decoded.push('\u{000c}'),
            Some('n') => decoded.push('\n'),
            Some('r') => decoded.push('\r'),
            Some('t') => decoded.push('\t'),
            Some('\\') => decoded.push('\\'),
            Some(other) => decoded.push(other),
            None => decoded.push('\\'),
        }
    }
    Ok(Some(decoded))
}

fn resource_owner_args(row: &CopyRow) -> Result<Value> {
    let mut args = Map::new();
    put_string(
        &mut args,
        "owner-principal-fqn",
        row.field("owner_principal_fqn")?,
    );
    put_string(
        &mut args,
        "owner-system",
        Some(row.required_field("owner_system")?),
    );
    put_string(
        &mut args,
        "display-name",
        Some(row.required_field("display_name")?),
    );
    put_string(
        &mut args,
        "dispatch-endpoint",
        row.field("dispatch_endpoint")?,
    );
    put_string(&mut args, "status", row.field("status")?);
    put_json(&mut args, "metadata", row.field("metadata")?)?;
    put_json(
        &mut args,
        "principal-capabilities",
        row.field("principal_capabilities")?,
    )?;
    put_bool(
        &mut args,
        "dispatch-enabled",
        row.field("dispatch_enabled")?,
    )?;
    Ok(Value::Object(args))
}

fn service_resource_type_args(row: &CopyRow) -> Result<Value> {
    let mut args = Map::new();
    put_string(&mut args, "resource-id", row.field("resource_id")?);
    put_string(&mut args, "name", Some(row.required_field("name")?));
    put_string(&mut args, "description", row.field("description")?);
    put_string(&mut args, "owner", Some(row.required_field("owner")?));
    put_string(
        &mut args,
        "dictionary-group",
        row.field("dictionary_group")?,
    );
    put_string(&mut args, "resource-code", row.field("resource_code")?);
    put_string(&mut args, "resource-type", row.field("resource_type")?);
    put_string(&mut args, "vendor", row.field("vendor")?);
    put_string(&mut args, "version", row.field("version")?);
    put_string(&mut args, "api-endpoint", row.field("api_endpoint")?);
    put_string(&mut args, "api-version", row.field("api_version")?);
    put_string(
        &mut args,
        "authentication-method",
        row.field("authentication_method")?,
    );
    put_bool(&mut args, "is-active", row.field("is_active")?)?;
    put_string(
        &mut args,
        "owner-principal-fqn",
        row.field("owner_principal_fqn")?,
    );
    put_string(
        &mut args,
        "governance-status",
        row.field("governance_status")?,
    );
    Ok(Value::Object(args))
}

fn service_args(row: &CopyRow) -> Result<Value> {
    let mut args = Map::new();
    put_string(&mut args, "service-id", row.field("service_id")?);
    put_string(&mut args, "name", Some(row.required_field("name")?));
    put_string(&mut args, "description", row.field("description")?);
    put_string(&mut args, "service-code", row.field("service_code")?);
    put_string(
        &mut args,
        "service-category",
        row.field("service_category")?,
    );
    put_json(&mut args, "sla-definition", row.field("sla_definition")?)?;
    put_bool(&mut args, "is-active", row.field("is_active")?)?;
    put_string(&mut args, "lifecycle-tags", row.field("lifecycle_tags")?);
    put_string(
        &mut args,
        "lifecycle-status",
        row.field("lifecycle_status")?,
    );
    put_string(&mut args, "governance-status", Some("active"));
    put_string(&mut args, "created-at", row.field("created_at")?);
    put_string(&mut args, "updated-at", row.field("updated_at")?);
    Ok(Value::Object(args))
}

fn product_args(row: &CopyRow) -> Result<Value> {
    let mut args = Map::new();
    put_string(&mut args, "product-id", row.field("product_id")?);
    put_string(&mut args, "name", Some(row.required_field("name")?));
    put_string(&mut args, "description", row.field("description")?);
    put_string(&mut args, "product-code", row.field("product_code")?);
    put_string(
        &mut args,
        "product-category",
        row.field("product_category")?,
    );
    put_string(
        &mut args,
        "regulatory-framework",
        row.field("regulatory_framework")?,
    );
    put_string(
        &mut args,
        "min-asset-requirement",
        row.field("min_asset_requirement")?,
    );
    put_bool(&mut args, "is-active", row.field("is_active")?)?;
    put_json(&mut args, "metadata", row.field("metadata")?)?;
    put_string(&mut args, "kyc-risk-rating", row.field("kyc_risk_rating")?);
    put_string(&mut args, "kyc-context", row.field("kyc_context")?);
    put_bool(&mut args, "requires-kyc", row.field("requires_kyc")?)?;
    put_string(&mut args, "product-family", row.field("product_family")?);
    put_string(&mut args, "effective-from", row.field("effective_from")?);
    put_string(&mut args, "effective-to", row.field("effective_to")?);
    put_string(&mut args, "governance-status", Some("active"));
    Ok(Value::Object(args))
}

fn service_version_args(row: &CopyRow) -> Result<Value> {
    let mut args = Map::new();
    put_string(&mut args, "version-id", row.field("id")?);
    put_string(&mut args, "service-id", row.field("service_id")?);
    put_string(&mut args, "version", Some(row.required_field("version")?));
    put_string(
        &mut args,
        "lifecycle-status",
        row.field("lifecycle_status")?,
    );
    put_json(&mut args, "spec", row.field("spec")?)?;
    put_string(&mut args, "drafted-at", row.field("drafted_at")?);
    put_string(&mut args, "reviewed-at", row.field("reviewed_at")?);
    put_string(&mut args, "published-at", row.field("published_at")?);
    put_string(&mut args, "superseded-at", row.field("superseded_at")?);
    put_string(&mut args, "retired-at", row.field("retired_at")?);
    put_string(&mut args, "notes", row.field("notes")?);
    put_string(&mut args, "created-at", row.field("created_at")?);
    put_string(&mut args, "updated-at", row.field("updated_at")?);
    Ok(Value::Object(args))
}

fn product_service_args(row: &CopyRow) -> Result<Value> {
    let mut args = Map::new();
    put_string(&mut args, "product-id", row.field("product_id")?);
    put_string(&mut args, "service-id", row.field("service_id")?);
    put_bool(&mut args, "is-mandatory", row.field("is_mandatory")?)?;
    put_bool(&mut args, "is-default", row.field("is_default")?)?;
    put_int(&mut args, "display-order", row.field("display_order")?)?;
    put_json(&mut args, "configuration", row.field("configuration")?)?;
    Ok(Value::Object(args))
}

fn service_resource_capability_args(row: &CopyRow) -> Result<Value> {
    let mut args = Map::new();
    put_string(&mut args, "capability-id", row.field("capability_id")?);
    put_string(&mut args, "service-id", row.field("service_id")?);
    put_string(&mut args, "resource-id", row.field("resource_id")?);
    put_json(
        &mut args,
        "supported-options",
        row.field("supported_options")?,
    )?;
    put_int(&mut args, "priority", row.field("priority")?)?;
    put_string(&mut args, "cost-factor", row.field("cost_factor")?);
    put_int(
        &mut args,
        "performance-rating",
        row.field("performance_rating")?,
    )?;
    put_json(&mut args, "resource-config", row.field("resource_config")?)?;
    put_bool(&mut args, "is-active", row.field("is_active")?)?;
    put_bool(&mut args, "is-required", row.field("is_required")?)?;
    Ok(Value::Object(args))
}

fn put_string(args: &mut Map<String, Value>, name: &str, value: Option<&str>) {
    if let Some(value) = value {
        args.insert(name.to_string(), Value::String(value.to_string()));
    }
}

fn put_bool(args: &mut Map<String, Value>, name: &str, value: Option<&str>) -> Result<()> {
    let Some(value) = value else {
        return Ok(());
    };
    let parsed = match value {
        "t" | "true" => true,
        "f" | "false" => false,
        other => return Err(anyhow!("{name} has invalid boolean value {other}")),
    };
    args.insert(name.to_string(), Value::Bool(parsed));
    Ok(())
}

fn put_int(args: &mut Map<String, Value>, name: &str, value: Option<&str>) -> Result<()> {
    let Some(value) = value else {
        return Ok(());
    };
    let parsed = value
        .parse::<i64>()
        .with_context(|| format!("{name} has invalid integer value {value}"))?;
    args.insert(name.to_string(), json!(parsed));
    Ok(())
}

fn put_json(args: &mut Map<String, Value>, name: &str, value: Option<&str>) -> Result<()> {
    let Some(value) = value else {
        return Ok(());
    };
    let parsed = serde_json::from_str(value)
        .with_context(|| format!("{name} has invalid JSON value {value}"))?;
    args.insert(name.to_string(), parsed);
    Ok(())
}
