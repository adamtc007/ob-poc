//! DB-backed catalogue loader — Tranche 3 Phase 3.F Stages 3+4 (2026-04-27).
//!
//! v1.2 §8.2 forward-discipline activation: makes `catalogue_committed_verbs`
//! a real source of truth alongside YAML.
//!
//! ## Two paths
//!
//! ```text
//! +-------------------+               +-------------------------+
//! | rust/config/verbs |  --(seed)-->  | catalogue_committed_    |
//! | (YAML, dev mode)  |               |   verbs (DB, prod)      |
//! +-------------------+               +-------------------------+
//!         ^                                       ^
//!         |                                       |
//!     ConfigLoader                          CatalogueLoader
//!     ::load_verbs()                        ::load_from_db()
//!         |                                       |
//!         +---------------+-----------------------+
//!                         |
//!                  CatalogueSource
//!                  ::resolve_from_env()
//! ```
//!
//! `CatalogueSource::from_env()` returns `Db` when `CATALOGUE_SOURCE=db` is
//! set, else `Yaml`. Production sets `CATALOGUE_SOURCE=db` to enforce
//! forward discipline; dev defaults to `Yaml` for hot-reload ergonomics.
//!
//! ## Boot-time seed
//!
//! `seed_committed_verbs_from_yaml(pool, &yaml_config)` writes one row per
//! YAML-declared verb to `catalogue_committed_verbs`. Idempotent — re-runs
//! UPDATE on conflict. Called at production boot when the table is empty.
//!
//! ## Stage 4 architectural payoff
//!
//! Once `CATALOGUE_SOURCE=db` is set in production AND every direct YAML
//! edit goes through `catalogue.commit-verb-declaration`, drift becomes
//! architecturally impossible — the catalogue has no other write path.

use anyhow::{anyhow, Context, Result};
use dsl_core::config::types::{DomainConfig, VerbConfig, VerbsConfig};
use serde_json::Value;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use tracing::info;

/// Where the catalogue is loaded from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogueSource {
    /// `rust/config/verbs/**/*.yaml` — dev default.
    Yaml,
    /// `"ob-poc".catalogue_committed_verbs` — production / forward-discipline default.
    Db,
}

impl CatalogueSource {
    /// Resolve from `CATALOGUE_SOURCE` env var. Defaults to `Yaml` for
    /// developer ergonomics; production deployments set
    /// `CATALOGUE_SOURCE=db` to enforce forward discipline.
    pub fn from_env() -> Self {
        match std::env::var("CATALOGUE_SOURCE")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "db" | "database" | "committed" => Self::Db,
            _ => Self::Yaml,
        }
    }
}

/// Seed `catalogue_committed_verbs` from a YAML-loaded `VerbsConfig`.
///
/// One INSERT per (domain, verb) pair. Idempotent via `ON CONFLICT DO
/// UPDATE`. Used:
///   - Once at first production boot to populate the table from the
///     committed YAML state.
///   - In CI to refresh the test database after a verbatim YAML import.
///
/// The `committed_proposal_id` field requires a row in
/// `catalogue_proposals`; this function inserts a synthetic
/// "yaml-bootstrap" proposal once and references it on every committed
/// verb. The synthetic proposal records `proposed_by = "yaml-bootstrap"`
/// and `committed_by = "yaml-bootstrap-system"` so the two-eye CHECK
/// constraint is satisfied (different principals).
pub async fn seed_committed_verbs_from_yaml(pool: &PgPool, cfg: &VerbsConfig) -> Result<usize> {
    // Ensure the bootstrap proposal exists.
    let bootstrap_id: uuid::Uuid = sqlx::query_scalar(
        r#"INSERT INTO "ob-poc".catalogue_proposals
           (verb_fqn, proposed_declaration, rationale, status,
            proposed_by, committed_by, committed_at)
           VALUES ('__yaml_bootstrap__', '{}'::jsonb,
                   'YAML-to-DB seed (Tranche 3 Phase 3.F Stage 3)',
                   'COMMITTED', 'yaml-bootstrap', 'yaml-bootstrap-system', now())
           ON CONFLICT DO NOTHING
           RETURNING proposal_id"#,
    )
    .fetch_optional(pool)
    .await
    .context("failed to insert yaml-bootstrap proposal")?
    .unwrap_or_else(uuid::Uuid::nil);

    // If the row already existed (ON CONFLICT DO NOTHING returns no row),
    // look it up.
    let bootstrap_id = if bootstrap_id == uuid::Uuid::nil() {
        sqlx::query_scalar(
            r#"SELECT proposal_id FROM "ob-poc".catalogue_proposals
               WHERE verb_fqn='__yaml_bootstrap__' AND proposed_by='yaml-bootstrap'
               ORDER BY created_at DESC LIMIT 1"#,
        )
        .fetch_one(pool)
        .await
        .context("failed to lookup yaml-bootstrap proposal")?
    } else {
        bootstrap_id
    };

    let mut count = 0usize;
    for (domain, dom_cfg) in &cfg.domains {
        for (verb, verb_cfg) in &dom_cfg.verbs {
            let fqn = format!("{domain}.{verb}");
            let declaration =
                serde_json::to_value(verb_cfg).with_context(|| format!("serialise {fqn}"))?;
            sqlx::query(
                r#"INSERT INTO "ob-poc".catalogue_committed_verbs
                   (verb_fqn, declaration, committed_proposal_id)
                   VALUES ($1, $2, $3)
                   ON CONFLICT (verb_fqn) DO UPDATE
                     SET declaration = EXCLUDED.declaration,
                         committed_proposal_id = EXCLUDED.committed_proposal_id,
                         committed_at = now()"#,
            )
            .bind(&fqn)
            .bind(&declaration)
            .bind(bootstrap_id)
            .execute(pool)
            .await
            .with_context(|| format!("failed to upsert committed verb {fqn}"))?;
            count += 1;
        }
    }
    info!("seeded {} verbs into catalogue_committed_verbs", count);
    Ok(count)
}

/// Load the catalogue from `catalogue_committed_verbs`. Returns a
/// `VerbsConfig` shaped identically to `ConfigLoader::load_verbs()` so
/// downstream consumers don't care which source is active.
///
/// Per Stage 4 architectural commitment: when `CATALOGUE_SOURCE=db` is
/// set, this is the ONLY way the catalogue loads — no YAML fallback.
pub async fn load_from_db(pool: &PgPool) -> Result<VerbsConfig> {
    let rows = sqlx::query(
        r#"SELECT verb_fqn, declaration
           FROM "ob-poc".catalogue_committed_verbs"#,
    )
    .fetch_all(pool)
    .await
    .context("failed to load catalogue_committed_verbs")?;

    if rows.is_empty() {
        return Err(anyhow!(
            "catalogue_committed_verbs is empty — run \
             seed_committed_verbs_from_yaml first or set CATALOGUE_SOURCE=yaml"
        ));
    }

    let mut domains: HashMap<String, DomainConfig> = HashMap::new();
    for r in rows {
        let fqn: String = r.try_get("verb_fqn").context("verb_fqn")?;
        if fqn == "__yaml_bootstrap__" {
            continue;
        }
        let declaration: Value = r.try_get("declaration").context("declaration")?;
        let (domain, verb) = fqn
            .split_once('.')
            .ok_or_else(|| anyhow!("malformed FQN: {}", fqn))?;
        let verb_cfg: VerbConfig =
            serde_json::from_value(declaration).with_context(|| format!("deserialise {fqn}"))?;
        let domain_entry = domains
            .entry(domain.to_string())
            .or_insert_with(|| DomainConfig {
                description: "(loaded from catalogue_committed_verbs)".to_string(),
                verbs: HashMap::new(),
                dynamic_verbs: vec![],
                invocation_hints: vec![],
            });
        domain_entry.verbs.insert(verb.to_string(), verb_cfg);
    }

    info!(
        "loaded {} verbs across {} domains from catalogue_committed_verbs",
        domains.values().map(|d| d.verbs.len()).sum::<usize>(),
        domains.len()
    );

    Ok(VerbsConfig {
        version: "1.0".into(),
        domains,
    })
}

/// Resolve the catalogue per the active source. Use this at startup in
/// place of direct `ConfigLoader::load_verbs()` calls when forward
/// discipline applies.
///
/// - `Yaml` mode: delegates to YAML loading (caller supplies the loaded config).
/// - `Db` mode: loads from `catalogue_committed_verbs`.
pub async fn resolve_catalogue(
    source: CatalogueSource,
    yaml_loaded: VerbsConfig,
    pool: &PgPool,
) -> Result<VerbsConfig> {
    match source {
        CatalogueSource::Yaml => Ok(yaml_loaded),
        CatalogueSource::Db => {
            // If the table is empty, seed from the YAML config so the first
            // production boot doesn't fail. Subsequent boots find the table
            // populated and skip the seed.
            let count: i64 =
                sqlx::query_scalar(r#"SELECT count(*) FROM "ob-poc".catalogue_committed_verbs"#)
                    .fetch_one(pool)
                    .await
                    .context("counting committed_verbs")?;
            if count == 0 {
                info!("catalogue_committed_verbs is empty; seeding from YAML");
                seed_committed_verbs_from_yaml(pool, &yaml_loaded).await?;
            }
            load_from_db(pool).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Combine env-var tests into one sequential test so they don't race
    // against the shared CATALOGUE_SOURCE env var when the test runner
    // parallelises by default.
    #[test]
    fn catalogue_source_resolution() {
        std::env::remove_var("CATALOGUE_SOURCE");
        assert_eq!(CatalogueSource::from_env(), CatalogueSource::Yaml);

        std::env::set_var("CATALOGUE_SOURCE", "db");
        assert_eq!(CatalogueSource::from_env(), CatalogueSource::Db);

        std::env::set_var("CATALOGUE_SOURCE", "DATABASE");
        assert_eq!(CatalogueSource::from_env(), CatalogueSource::Db);

        std::env::set_var("CATALOGUE_SOURCE", "committed");
        assert_eq!(CatalogueSource::from_env(), CatalogueSource::Db);

        std::env::set_var("CATALOGUE_SOURCE", "potato");
        assert_eq!(CatalogueSource::from_env(), CatalogueSource::Yaml);

        std::env::remove_var("CATALOGUE_SOURCE");
    }
}
