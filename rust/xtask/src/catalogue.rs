//! `cargo x catalogue` subcommands — Tranche 3 Phase 3.B (2026-04-26).
//!
//! Command-line ergonomics for catalogue authorship. Wraps the four
//! authorship verbs (`catalogue.{propose,commit,rollback,list}`) into a
//! CLI surface so authors can invoke them without going through the REPL.
//!
//! These commands talk directly to Postgres rather than dispatching
//! through the orchestrator — they're a developer-tools surface, not a
//! production authorship path. Production authorship uses Sage / REPL,
//! which dispatches through the verb runtime + ABAC + audit trail.
//!
//! Per v1.2 §8.4 DoD item 9.

use anyhow::{anyhow, Context, Result};
use clap::Subcommand;
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use uuid::Uuid;

#[derive(Subcommand)]
pub enum CatalogueAction {
    /// Propose a new or updated verb declaration (DRAFT row insert).
    ///
    /// The proposed declaration JSON is read from --file or --json. The
    /// verb FQN is the verb being authored / amended.
    Propose {
        /// Verb FQN (e.g. "deal.cancel").
        #[arg(long)]
        verb_fqn: String,
        /// Path to a JSON file containing the proposed declaration.
        #[arg(long, conflicts_with = "json")]
        file: Option<String>,
        /// Inline JSON containing the proposed declaration.
        #[arg(long, conflicts_with = "file")]
        json: Option<String>,
        /// Optional rationale for the proposal.
        #[arg(long)]
        rationale: Option<String>,
        /// Proposing principal (e.g. email or actor_id). Defaults to $USER.
        #[arg(long)]
        proposed_by: Option<String>,
    },

    /// Commit a STAGED proposal — promotes to authoritative catalogue.
    ///
    /// Two-eye rule: the committing principal MUST differ from the
    /// proposing principal. Enforced by a DB CHECK constraint and by
    /// pre-flight verb handler logic; this CLI surfaces a clean error.
    Commit {
        /// Proposal UUID to commit.
        #[arg(long)]
        proposal_id: Uuid,
        /// Approving principal. Must differ from the proposer.
        #[arg(long)]
        approver: String,
    },

    /// Roll back a DRAFT or STAGED proposal.
    Rollback {
        /// Proposal UUID to roll back.
        #[arg(long)]
        proposal_id: Uuid,
        /// Reason for rollback.
        #[arg(long)]
        reason: String,
        /// Acting principal. Defaults to $USER.
        #[arg(long)]
        acting_by: Option<String>,
    },

    /// List proposals filtered by status.
    List {
        /// Filter: pending (DRAFT|STAGED), committed, rolled_back, all.
        #[arg(long, default_value = "pending")]
        status: String,
    },
}

fn user_default() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "anonymous".to_string())
}

async fn pool() -> Result<PgPool> {
    let url = std::env::var("DATABASE_URL")
        .context("DATABASE_URL must be set for `cargo x catalogue` commands")?;
    PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .context("failed to connect to Postgres")
}

pub async fn run(action: CatalogueAction) -> Result<()> {
    let pool = pool().await?;
    match action {
        CatalogueAction::Propose {
            verb_fqn,
            file,
            json,
            rationale,
            proposed_by,
        } => propose(&pool, verb_fqn, file, json, rationale, proposed_by).await,
        CatalogueAction::Commit {
            proposal_id,
            approver,
        } => commit(&pool, proposal_id, approver).await,
        CatalogueAction::Rollback {
            proposal_id,
            reason,
            acting_by,
        } => rollback(&pool, proposal_id, reason, acting_by).await,
        CatalogueAction::List { status } => list(&pool, status).await,
    }
}

async fn propose(
    pool: &PgPool,
    verb_fqn: String,
    file: Option<String>,
    inline_json: Option<String>,
    rationale: Option<String>,
    proposed_by: Option<String>,
) -> Result<()> {
    let raw = match (file, inline_json) {
        (Some(path), None) => {
            std::fs::read_to_string(&path).with_context(|| format!("reading {}", path))?
        }
        (None, Some(s)) => s,
        (None, None) => {
            return Err(anyhow!("Must supply either --file or --json"));
        }
        _ => unreachable!("clap conflicts_with covers this"),
    };
    let declaration: serde_json::Value =
        serde_json::from_str(&raw).context("proposed declaration must be valid JSON")?;
    let proposed_by = proposed_by.unwrap_or_else(user_default);

    let proposal_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO "ob-poc".catalogue_proposals
           (verb_fqn, proposed_declaration, rationale, status, proposed_by)
           VALUES ($1, $2, $3, 'DRAFT', $4)
           RETURNING proposal_id"#,
    )
    .bind(&verb_fqn)
    .bind(&declaration)
    .bind(&rationale)
    .bind(&proposed_by)
    .fetch_one(pool)
    .await
    .context("INSERT into catalogue_proposals failed")?;

    println!("✓ Proposal created");
    println!("  proposal_id: {}", proposal_id);
    println!("  verb_fqn:    {}", verb_fqn);
    println!("  proposed_by: {}", proposed_by);
    println!("  status:      DRAFT");
    println!();
    println!("Next: stage by running validator on the declaration; then");
    println!(
        "      cargo x catalogue commit --proposal-id {} --approver <other-author>",
        proposal_id
    );
    Ok(())
}

async fn commit(pool: &PgPool, proposal_id: Uuid, approver: String) -> Result<()> {
    // Pre-flight: load current row + check two-eye rule.
    let row = sqlx::query(
        r#"SELECT status, proposed_by, verb_fqn, proposed_declaration
           FROM "ob-poc".catalogue_proposals WHERE proposal_id = $1"#,
    )
    .bind(proposal_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("Proposal not found: {}", proposal_id))?;

    let status: String = row.try_get("status")?;
    let proposed_by: String = row.try_get("proposed_by")?;
    let verb_fqn: String = row.try_get("verb_fqn")?;
    let declaration: serde_json::Value = row.try_get("proposed_declaration")?;

    if status != "STAGED" {
        return Err(anyhow!(
            "Cannot commit proposal in state '{}'; must be STAGED",
            status
        ));
    }
    if approver == proposed_by {
        return Err(anyhow!(
            "Two-eye rule violation: approver ({}) must differ from proposer ({})",
            approver,
            proposed_by
        ));
    }

    let mut tx = pool.begin().await?;
    sqlx::query(
        r#"UPDATE "ob-poc".catalogue_proposals
           SET status = 'COMMITTED', committed_by = $2, committed_at = now()
           WHERE proposal_id = $1"#,
    )
    .bind(proposal_id)
    .bind(&approver)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"INSERT INTO "ob-poc".catalogue_committed_verbs
           (verb_fqn, declaration, committed_proposal_id)
           VALUES ($1, $2, $3)
           ON CONFLICT (verb_fqn) DO UPDATE
             SET declaration = EXCLUDED.declaration,
                 committed_proposal_id = EXCLUDED.committed_proposal_id,
                 committed_at = now()"#,
    )
    .bind(&verb_fqn)
    .bind(&declaration)
    .bind(proposal_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    println!("✓ Proposal committed");
    println!("  proposal_id:  {}", proposal_id);
    println!("  verb_fqn:     {}", verb_fqn);
    println!("  proposed_by:  {}", proposed_by);
    println!("  committed_by: {}", approver);
    println!("  status:       COMMITTED");
    Ok(())
}

async fn rollback(
    pool: &PgPool,
    proposal_id: Uuid,
    reason: String,
    acting_by: Option<String>,
) -> Result<()> {
    let acting_by = acting_by.unwrap_or_else(user_default);

    let current_status: Option<String> = sqlx::query_scalar(
        r#"SELECT status FROM "ob-poc".catalogue_proposals WHERE proposal_id = $1"#,
    )
    .bind(proposal_id)
    .fetch_optional(pool)
    .await?;

    let status = current_status.ok_or_else(|| anyhow!("Proposal not found: {}", proposal_id))?;
    if status != "STAGED" && status != "DRAFT" {
        return Err(anyhow!(
            "Cannot roll back proposal in state '{}'; must be DRAFT or STAGED",
            status
        ));
    }

    sqlx::query(
        r#"UPDATE "ob-poc".catalogue_proposals
           SET status = 'ROLLED_BACK',
               rolled_back_by = $2,
               rolled_back_at = now(),
               rolled_back_reason = $3
           WHERE proposal_id = $1"#,
    )
    .bind(proposal_id)
    .bind(&acting_by)
    .bind(&reason)
    .execute(pool)
    .await?;

    println!("✓ Proposal rolled back");
    println!("  proposal_id: {}", proposal_id);
    println!("  acting_by:   {}", acting_by);
    println!("  reason:      {}", reason);
    Ok(())
}

async fn list(pool: &PgPool, status_filter: String) -> Result<()> {
    let sql = match status_filter.as_str() {
        "pending" => {
            r#"SELECT proposal_id, verb_fqn, status, proposed_by, created_at
               FROM "ob-poc".catalogue_proposals
               WHERE status IN ('DRAFT', 'STAGED')
               ORDER BY created_at DESC LIMIT 100"#
        }
        "committed" => {
            r#"SELECT proposal_id, verb_fqn, status, proposed_by, committed_at AS created_at
               FROM "ob-poc".catalogue_proposals
               WHERE status = 'COMMITTED'
               ORDER BY committed_at DESC LIMIT 100"#
        }
        "rolled_back" => {
            r#"SELECT proposal_id, verb_fqn, status, proposed_by, rolled_back_at AS created_at
               FROM "ob-poc".catalogue_proposals
               WHERE status = 'ROLLED_BACK'
               ORDER BY rolled_back_at DESC LIMIT 100"#
        }
        _ => {
            r#"SELECT proposal_id, verb_fqn, status, proposed_by, created_at
               FROM "ob-poc".catalogue_proposals
               ORDER BY created_at DESC LIMIT 100"#
        }
    };

    let rows = sqlx::query(sql).fetch_all(pool).await?;
    if rows.is_empty() {
        println!("(no proposals matching '{}')", status_filter);
        return Ok(());
    }

    println!(
        "{:<38} {:<35} {:<14} {:<25}",
        "proposal_id", "verb_fqn", "status", "proposed_by"
    );
    println!("{}", "─".repeat(115));
    for r in rows {
        let proposal_id: Uuid = r.try_get("proposal_id")?;
        let verb_fqn: String = r.try_get("verb_fqn")?;
        let status: String = r.try_get("status")?;
        let proposed_by: String = r.try_get("proposed_by")?;
        println!(
            "{:<38} {:<35} {:<14} {:<25}",
            proposal_id, verb_fqn, status, proposed_by
        );
    }
    Ok(())
}
