//! CLI governance checker — detects drift between `#[governed_query]` annotations
//! and the live Semantic OS registry.
//!
//! Hard errors (fail CI with `--strict`):
//!   GC001  Verb not found in registry (unknown FQN)
//!   GC002  Verb is deprecated (must migrate)
//!   GC003  Cache stale (missing, count drift >5%, or older than 7 days)
//!
//! Soft warnings (informational):
//!   GC010  Verb approaching deprecation (active with successor)
//!   GC011  Unused PII authorization (allow_pii but no PII label)
//!
//! Usage:
//!   cargo x governed-check           # print findings, always exit 0
//!   cargo x governed-check --strict  # exit non-zero on hard errors (CI mode)

use anyhow::{Context, Result};
use sqlx::PgPool;
use std::collections::HashMap;
use std::path::PathBuf;

// ── Finding model ────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug)]
pub struct Finding {
    pub code: &'static str,
    pub severity: Severity,
    pub file: String,
    pub line: usize,
    pub verb: String,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct CheckResult {
    pub findings: Vec<Finding>,
}

impl CheckResult {
    pub fn error_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == Severity::Warning)
            .count()
    }

    pub fn info_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == Severity::Info)
            .count()
    }
}

// ── Annotation scanning ──────────────────────────────────────────

#[derive(Debug)]
struct AnnotationSite {
    file: PathBuf,
    line: usize,
    verb: String,
    attrs: Vec<String>,
    allow_pii: bool,
}

// ── DB row type ──────────────────────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
struct SnapshotWithSuccessor {
    fqn: String,
    #[allow(dead_code)]
    object_type: String,
    status: String,
    has_successor: bool,
    pii: bool,
}

// ── Entry point ──────────────────────────────────────────────────

pub async fn run_check(strict: bool) -> Result<()> {
    let pool = connect().await?;

    // 1. Scan source files for annotations
    let annotations = scan_annotations()?;

    println!(
        "Found {} #[governed_query] annotation(s) across {} file(s).",
        annotations.len(),
        annotations
            .iter()
            .map(|a| a.file.display().to_string())
            .collect::<std::collections::HashSet<_>>()
            .len()
    );

    // 2. Query DB for current state
    let db_state = query_db_state(&pool).await?;
    println!("Registry: {} active/deprecated FQNs in database.", db_state.len());

    // 3. Run all checks
    let mut result = CheckResult::default();

    // 3a. Annotation-level checks (only if annotations exist)
    check_annotations(&annotations, &db_state, &mut result);

    // 3b. Cache staleness check (always runs)
    check_cache_staleness(&pool, &mut result).await;

    // 4. Print results
    print_results(&result);

    // 5. Exit code
    if strict && result.error_count() > 0 {
        anyhow::bail!(
            "Governance check failed: {} error(s) found. Fix errors or update the cache.",
            result.error_count()
        );
    }

    Ok(())
}

// ── Annotation checks ────────────────────────────────────────────

fn check_annotations(
    annotations: &[AnnotationSite],
    db_state: &HashMap<String, SnapshotWithSuccessor>,
    result: &mut CheckResult,
) {
    for ann in annotations {
        match db_state.get(&ann.verb) {
            None => {
                // GC001: Verb not found in registry
                result.findings.push(Finding {
                    code: "GC001",
                    severity: Severity::Error,
                    file: ann.file.display().to_string(),
                    line: ann.line,
                    verb: ann.verb.clone(),
                    message: format!(
                        "verb `{}` not found in registry — no active or deprecated snapshot exists",
                        ann.verb
                    ),
                });
            }
            Some(entry) => {
                // GC002: Verb deprecated
                if entry.status == "deprecated" {
                    result.findings.push(Finding {
                        code: "GC002",
                        severity: Severity::Error,
                        file: ann.file.display().to_string(),
                        line: ann.line,
                        verb: ann.verb.clone(),
                        message: format!(
                            "verb `{}` is deprecated — migrate to its successor",
                            ann.verb
                        ),
                    });
                }

                // GC010: Approaching deprecation (active with successor)
                if entry.status == "active" && entry.has_successor {
                    result.findings.push(Finding {
                        code: "GC010",
                        severity: Severity::Info,
                        file: ann.file.display().to_string(),
                        line: ann.line,
                        verb: ann.verb.clone(),
                        message: format!(
                            "verb `{}` has a successor — consider migrating before deprecation",
                            ann.verb
                        ),
                    });
                }

                // GC011: Unused PII authorization
                if ann.allow_pii && !entry.pii {
                    result.findings.push(Finding {
                        code: "GC011",
                        severity: Severity::Warning,
                        file: ann.file.display().to_string(),
                        line: ann.line,
                        verb: ann.verb.clone(),
                        message: format!(
                            "verb `{}` does not carry PII label — `allow_pii = true` is unnecessary",
                            ann.verb
                        ),
                    });
                }
            }
        }

        // Check referenced attributes
        for attr_fqn in &ann.attrs {
            match db_state.get(attr_fqn) {
                None => {
                    result.findings.push(Finding {
                        code: "GC001",
                        severity: Severity::Error,
                        file: ann.file.display().to_string(),
                        line: ann.line,
                        verb: ann.verb.clone(),
                        message: format!(
                            "attribute `{attr_fqn}` not found in registry"
                        ),
                    });
                }
                Some(entry) => {
                    if entry.status == "deprecated" {
                        result.findings.push(Finding {
                            code: "GC002",
                            severity: Severity::Error,
                            file: ann.file.display().to_string(),
                            line: ann.line,
                            verb: ann.verb.clone(),
                            message: format!(
                                "attribute `{attr_fqn}` is deprecated — migrate to its successor"
                            ),
                        });
                    }
                    if entry.status == "active" && entry.has_successor {
                        result.findings.push(Finding {
                            code: "GC010",
                            severity: Severity::Info,
                            file: ann.file.display().to_string(),
                            line: ann.line,
                            verb: ann.verb.clone(),
                            message: format!(
                                "attribute `{attr_fqn}` has a successor — consider migrating"
                            ),
                        });
                    }
                }
            }
        }
    }
}

// ── Cache staleness check ────────────────────────────────────────

async fn check_cache_staleness(pool: &PgPool, result: &mut CheckResult) {
    let cache_path = default_cache_path();

    // Load cache file
    let cache_bytes = match std::fs::read(&cache_path) {
        Ok(b) => b,
        Err(_) => {
            result.findings.push(Finding {
                code: "GC003",
                severity: Severity::Error,
                file: cache_path.display().to_string(),
                line: 0,
                verb: String::new(),
                message: format!(
                    "cache file missing at {} — run `cargo x governed-cache refresh`",
                    cache_path.display()
                ),
            });
            return;
        }
    };

    let cache: governed_cache_types::GovernedCache = match bincode::deserialize(&cache_bytes) {
        Ok(c) => c,
        Err(e) => {
            result.findings.push(Finding {
                code: "GC003",
                severity: Severity::Error,
                file: cache_path.display().to_string(),
                line: 0,
                verb: String::new(),
                message: format!("cache file corrupt — {e}. Run `cargo x governed-cache refresh`"),
            });
            return;
        }
    };

    // Query DB for active snapshot count
    let db_count: i64 = match sqlx::query_scalar(
        "SELECT COUNT(*) FROM sem_reg.snapshots WHERE status = 'active' AND definition ->> 'fqn' IS NOT NULL"
    )
    .fetch_one(pool)
    .await
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("  WARN: could not query DB for cache staleness check: {e}");
            return;
        }
    };

    let cache_count = cache.entries.len() as i64;
    let drift_pct = if db_count > 0 {
        ((cache_count - db_count).abs() as f64) / (db_count as f64)
    } else {
        0.0
    };

    // Count drift > 5%
    if drift_pct > 0.05 {
        result.findings.push(Finding {
            code: "GC003",
            severity: Severity::Error,
            file: cache_path.display().to_string(),
            line: 0,
            verb: String::new(),
            message: format!(
                "cache has {cache_count} entries but DB has {db_count} active snapshots ({:.1}% drift) — run `cargo x governed-cache refresh`",
                drift_pct * 100.0
            ),
        });
    }

    // Age check: older than 7 days
    if let Ok(generated_at) = chrono::DateTime::parse_from_rfc3339(&cache.generated_at) {
        let age = chrono::Utc::now() - generated_at.with_timezone(&chrono::Utc);
        if age.num_days() > 7 {
            result.findings.push(Finding {
                code: "GC003",
                severity: Severity::Error,
                file: cache_path.display().to_string(),
                line: 0,
                verb: String::new(),
                message: format!(
                    "cache is {} days old (generated {}) — run `cargo x governed-cache refresh`",
                    age.num_days(),
                    cache.generated_at
                ),
            });
        }
    }
}

// ── Output formatting ────────────────────────────────────────────

fn print_results(result: &CheckResult) {
    if result.findings.is_empty() {
        println!("\nGovernance check passed — no findings.");
        return;
    }

    println!("\nGovernance Check Results:");
    println!("========================");

    // Print errors first, then warnings, then info
    for severity in [Severity::Error, Severity::Warning, Severity::Info] {
        for f in result.findings.iter().filter(|f| f.severity == severity) {
            let prefix = match f.severity {
                Severity::Error => "ERROR",
                Severity::Warning => " WARN",
                Severity::Info => " INFO",
            };
            if f.line > 0 {
                println!("  [{prefix}] {code} {file}:{line}  verb={verb}",
                    code = f.code, file = f.file, line = f.line, verb = f.verb);
            } else {
                println!("  [{prefix}] {code} {file}",
                    code = f.code, file = f.file);
            }
            println!("         {}", f.message);
        }
    }

    println!(
        "\n{} error(s), {} warning(s), {} info(s).",
        result.error_count(),
        result.warning_count(),
        result.info_count()
    );
}

// ── Source scanning ──────────────────────────────────────────────

fn scan_annotations() -> Result<Vec<AnnotationSite>> {
    let mut results = Vec::new();

    let patterns = &["src/**/*.rs", "crates/**/*.rs"];
    let workspace_root = workspace_root();

    for pattern in patterns {
        let full_pattern = workspace_root.join(pattern).display().to_string();
        for entry in glob::glob(&full_pattern)? {
            let path = entry?;
            if let Ok(content) = std::fs::read_to_string(&path) {
                parse_annotations_from_file(&path, &content, &mut results);
            }
        }
    }

    Ok(results)
}

fn parse_annotations_from_file(
    path: &std::path::Path,
    content: &str,
    results: &mut Vec<AnnotationSite>,
) {
    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if !trimmed.starts_with("#[governed_query(") {
            continue;
        }

        if let Some(verb) = extract_string_arg(trimmed, "verb") {
            let attrs = extract_string_array(trimmed, "attrs");
            let allow_pii = trimmed.contains("allow_pii = true");

            results.push(AnnotationSite {
                file: path.to_path_buf(),
                line: line_num + 1,
                verb,
                attrs,
                allow_pii,
            });
        }
    }
}

fn extract_string_arg(line: &str, key: &str) -> Option<String> {
    let pattern = format!("{key} = \"");
    let start = line.find(&pattern)? + pattern.len();
    let end = line[start..].find('"')? + start;
    Some(line[start..end].to_string())
}

fn extract_string_array(line: &str, key: &str) -> Vec<String> {
    let pattern = format!("{key} = [");
    let Some(start) = line.find(&pattern) else {
        return Vec::new();
    };
    let start = start + pattern.len();
    let Some(end) = line[start..].find(']') else {
        return Vec::new();
    };
    let inner = &line[start..start + end];
    inner
        .split(',')
        .filter_map(|s| {
            let s = s.trim().trim_matches('"');
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        })
        .collect()
}

// ── DB + cache helpers ───────────────────────────────────────────

async fn query_db_state(pool: &PgPool) -> Result<HashMap<String, SnapshotWithSuccessor>> {
    let rows: Vec<SnapshotWithSuccessor> = sqlx::query_as(
        r#"
        SELECT
            s.definition ->> 'fqn' AS fqn,
            s.object_type::text,
            s.status::text,
            EXISTS (
                SELECT 1 FROM sem_reg.snapshots s2
                WHERE s2.predecessor_id = s.snapshot_id
            ) AS has_successor,
            COALESCE((s.security_label->>'pii')::boolean, false) AS pii
        FROM sem_reg.snapshots s
        WHERE s.status IN ('active', 'deprecated')
          AND s.definition ->> 'fqn' IS NOT NULL
        ORDER BY s.definition ->> 'fqn'
        "#,
    )
    .fetch_all(pool)
    .await
    .context("Failed to query snapshot state")?;

    let map: HashMap<String, SnapshotWithSuccessor> =
        rows.into_iter().map(|r| (r.fqn.clone(), r)).collect();

    Ok(map)
}

fn workspace_root() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(&manifest_dir)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn default_cache_path() -> PathBuf {
    workspace_root().join("assets").join("governed_cache.bin")
}

async fn connect() -> Result<PgPool> {
    let url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".into());
    PgPool::connect(&url)
        .await
        .context("Failed to connect to database")
}

// ── Cache types (shared with governed_cache.rs) ──────────────────
//
// We import via a private module to avoid duplicating the struct definitions.
// The canonical types live in governed_cache.rs; we re-use them for deserialization only.

mod governed_cache_types {
    use serde::Deserialize;
    use std::collections::HashMap;

    // Mirror types must match governed_cache.rs exactly for bincode deserialization.

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum GovernanceTier { Governed, Operational }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum TrustClass { Proof, DecisionSupport, Convenience }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum SnapshotStatus { Draft, Active, Deprecated, Retired }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum ObjectType {
        AttributeDef, EntityTypeDef, RelationshipTypeDef, VerbContract,
        TaxonomyDef, TaxonomyNode, MembershipRule, ViewDef, PolicyRule,
        EvidenceRequirement, DocumentTypeDef, ObservationDef, DerivationSpec,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum Classification { Public, Internal, Confidential, Restricted }

    #[derive(Debug, Deserialize)]
    pub struct GovernedCache {
        #[allow(dead_code)]
        pub version: u32,
        pub generated_at: String,
        pub entries: HashMap<String, CacheEntry>,
    }

    #[derive(Debug, Deserialize)]
    pub struct CacheEntry {
        #[allow(dead_code)]
        pub fqn: String,
        #[allow(dead_code)]
        pub object_type: ObjectType,
        #[allow(dead_code)]
        pub status: SnapshotStatus,
        #[allow(dead_code)]
        pub governance_tier: GovernanceTier,
        #[allow(dead_code)]
        pub trust_class: TrustClass,
        #[allow(dead_code)]
        pub pii: bool,
        #[allow(dead_code)]
        pub classification: Classification,
    }
}
