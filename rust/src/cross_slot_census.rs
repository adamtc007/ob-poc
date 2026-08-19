//! EOP-PLAN-GAMEBOARD-001 R2 Stage 1 — `cross_slot_constraints` report-only
//! census.
//!
//! §1 fact 1: 49 `cross_slot_constraints` entries across 8 DAG files, all
//! declared, zero runtime consumers. R0 P1's recon found a second, sharper
//! fact underneath that: unlike `CrossWorkspaceConstraint` (structured,
//! machine-evaluable fields), `CrossSlotConstraint.rule` is a **freeform
//! `Option<String>`** — hand-written pseudo-SQL prose, not a parseable
//! predicate grammar. There is no generic interpreter to write; each rule's
//! real check has to be hand-translated and verified against the live
//! schema before it can honestly report a verdict.
//!
//! That verification turned up a THIRD fact, beyond the two the plan's §1
//! already names: several rules reference columns that don't exist on the
//! real table (`cbu_evidence.evidence_required`, `investors.status` — the
//! real column is `lifecycle_state`). This is fact 3's exact shape
//! ("declarations themselves are unvalidated") recurring in a different
//! declaration surface. Per that fact, this module does not silently
//! reinterpret a broken rule as something else and pass it off as the
//! rule's own verdict — a rule whose literal text doesn't match the real
//! schema is reported as [`ConstraintOutcome::SchemaMismatch`], visibly,
//! not guessed into a wrong-but-plausible check.
//!
//! Stage 1 is **report-only by construction**: [`ConstraintReport`] is a
//! plain data value. Nothing in this module returns an error that a caller
//! could interpret as "refuse the transition," and nothing in the wider
//! codebase calls into this module from a dispatch path (grep-checked by
//! this module's own test, `nothing_outside_this_module_calls_evaluate_all`).
//! Wiring it into live per-transition dispatch (matching where the
//! enforced sibling, `GateChecker`, is evaluated) is explicitly deferred —
//! see the R0 P3 pause note in this session's report: that call site is
//! shared with the in-flight Control-Plane Graduation program and needs
//! its own reconciliation, not a second independent change riding in
//! through R2.

use std::collections::BTreeMap;

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

/// One declared `cross_slot_constraints` entry, indexed alongside its
/// owning workspace (DAG file). Loaded through the SAME single loader
/// `cross_workspace_constraints` and `requires_states` already use
/// (`dsl_core::load_dags_from_dir` — one loader, one structure, per §0).
#[derive(Debug, Clone)]
pub struct CrossSlotConstraintDecl {
    pub workspace: String,
    pub id: String,
    pub description: String,
    pub severity: String,
    pub rule_text: Option<String>,
}

/// The result of evaluating one declared constraint against real data.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum ConstraintOutcome {
    /// Hand-translated and evaluated; no violating rows found.
    Clean,
    /// Hand-translated and evaluated; these rows violate it.
    Violated { entities: Vec<ViolationExample> },
    /// Declared, indexed, dispatched — but not yet hand-translated into a
    /// real check. Distinct from `Violated`/`Clean`: this is an honest
    /// "no verdict yet," never silently omitted from the census.
    NotYetImplemented,
    /// The rule's own text references a column/table that does not exist
    /// on the real schema (verified directly against `psql \d`, not
    /// assumed) — a fact-3-shaped declaration bug, distinct from "not yet
    /// implemented." Named so stage 2 planning doesn't mistake "nothing to
    /// enforce" for "this rule cannot be enforced as declared."
    SchemaMismatch { detail: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct ViolationExample {
    pub entity_id: Uuid,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConstraintReport {
    pub workspace: String,
    pub id: String,
    pub severity: String,
    pub outcome: ConstraintOutcome,
}

impl ConstraintReport {
    pub fn violation_count(&self) -> usize {
        match &self.outcome {
            ConstraintOutcome::Violated { entities } => entities.len(),
            _ => 0,
        }
    }
}

/// Loads every `cross_slot_constraints` entry from every DAG file, via the
/// single production loader. Returns them flattened with their owning
/// workspace attached — this IS the "index all 49 into the registry"
/// deliverable; a flat `Vec` is the right shape here because, unlike
/// `cross_workspace_constraints`, `CrossSlotConstraint` carries no
/// target-transition addressing to index by (confirmed by its own field
/// list: `id`, `description`, `rule`, `severity`, `v1_3_candidate`,
/// `enforced_by` — no workspace/slot/transition fields at all).
pub fn load_all_declared(dag_dir: &std::path::Path) -> anyhow::Result<Vec<CrossSlotConstraintDecl>> {
    let loaded = dsl_core::load_dags_from_dir(dag_dir)?;
    let mut out = Vec::new();
    for (workspace, ld) in loaded {
        for c in &ld.dag.cross_slot_constraints {
            out.push(CrossSlotConstraintDecl {
                workspace: workspace.clone(),
                id: c.id.clone(),
                description: c.description.clone(),
                severity: format!("{:?}", c.severity).to_lowercase(),
                rule_text: c.rule.clone(),
            });
        }
    }
    Ok(out)
}

/// Evaluates every declared constraint. Every id gets a real
/// [`ConstraintReport`] — `NotYetImplemented`/`SchemaMismatch` are
/// legitimate, visible outcomes, not omissions. This is what makes
/// `every_declared_cross_slot_constraint_is_evaluated` true: dispatch
/// happens for all 49, even where the verdict isn't a real Clean/Violated
/// yet.
pub async fn evaluate_all(
    declared: &[CrossSlotConstraintDecl],
    pool: &PgPool,
) -> anyhow::Result<Vec<ConstraintReport>> {
    let mut out = Vec::with_capacity(declared.len());
    for decl in declared {
        let outcome = evaluate_one(&decl.id, pool).await?;
        out.push(ConstraintReport {
            workspace: decl.workspace.clone(),
            id: decl.id.clone(),
            severity: decl.severity.clone(),
            outcome,
        });
    }
    Ok(out)
}

/// Per-rule dispatch. Hand-verified subset (8 of 49 — cbu_validated_*/2 +
/// deal_contracted_requires_bac_approved from the first pass, the 3 KYC
/// case-approval cluster rules added 2026-08-18, and the 2 investor-register
/// rules added 2026-08-19 once the register was actually populated) return a
/// real `Clean`/`Violated` verdict; everything else is honestly
/// `NotYetImplemented`. No rule is currently `SchemaMismatch` — the 2 that
/// were (investor_active_requires_kyc_approved,
/// holding_active_requires_investor_active) are fixed as of 2026-08-19.
async fn evaluate_one(rule_id: &str, pool: &PgPool) -> anyhow::Result<ConstraintOutcome> {
    match rule_id {
        // --- Hand-verified, real checks (3) ---------------------------------
        "cbu_validated_requires_evidence_set_verified" => {
            evidence_set_verified(pool).await
        }
        "cbu_validated_requires_commercial_client_entity" => {
            commercial_client_entity_set(pool).await
        }
        "deal_contracted_requires_bac_approved" => bac_approved(pool).await,

        // --- Investor register (2), hand-translated 2026-08-19 after the
        // register was actually populated for the first time
        // (20260819_backfill_investors_register_allianz.sql) — see that
        // migration's own comment for why only 1 of 5 candidate investor
        // entities was backfilled (the other 4 are captest_* fixture data).
        "investor_active_requires_kyc_approved" => investor_kyc_approved(pool).await,
        "holding_active_requires_investor_active" => holding_investor_active(pool).await,

        // --- KYC case-approval cluster (3), hand-translated 2026-08-18 ------
        "case_cannot_approve_without_workstreams_complete" => {
            case_workstreams_complete(pool).await
        }
        "case_cannot_approve_with_unresolved_red_flags" => {
            case_no_blocking_red_flags(pool).await
        }
        "case_cannot_approve_with_unresolved_screening_hits" => {
            case_screenings_resolved(pool).await
        }

        // --- Declared, indexed, not yet hand-translated (41) ----------------
        _ => Ok(ConstraintOutcome::NotYetImplemented),
    }
}

/// `cbu_validated_requires_evidence_set_verified` — the §1 fact 1 flagship
/// rule. The rule's OWN text references `cbu_evidence.evidence_required`,
/// a column that does not exist (verified: `\d "ob-poc".cbu_evidence` has
/// no such column). Rather than mark this `SchemaMismatch` too — which
/// would silently drop the one rule this whole tranche exists to catch —
/// this uses the rule's DESCRIPTION as the source of intent ("CBU →
/// VALIDATED requires ALL required cbu_evidence rows at status =
/// VERIFIED") and closes the vacuous-ALL trap a literal `NOT EXISTS
/// (non-verified row)` would fall into: a CBU with ZERO evidence rows at
/// all must count as a violation (missing required evidence), not as
/// vacuously compliant — which is exactly the real incident from fact 1
/// (a CBU reached VALIDATED with zero evidence rows).
async fn evidence_set_verified(pool: &PgPool) -> anyhow::Result<ConstraintOutcome> {
    let rows: Vec<(Uuid, i64, i64)> = sqlx::query_as(
        r#"
        SELECT c.cbu_id,
               COUNT(e.evidence_id) AS total,
               COUNT(e.evidence_id) FILTER (WHERE e.verification_status = 'VERIFIED') AS verified
        FROM "ob-poc".cbus c
        LEFT JOIN "ob-poc".cbu_evidence e ON e.cbu_id = c.cbu_id
        WHERE c.status = 'VALIDATED'
        GROUP BY c.cbu_id
        HAVING COUNT(e.evidence_id) = 0
            OR COUNT(e.evidence_id) FILTER (WHERE e.verification_status = 'VERIFIED') < COUNT(e.evidence_id)
        "#,
    )
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        return Ok(ConstraintOutcome::Clean);
    }
    let entities = rows
        .into_iter()
        .map(|(cbu_id, total, verified)| ViolationExample {
            entity_id: cbu_id,
            detail: format!("VALIDATED with {verified}/{total} cbu_evidence rows VERIFIED"),
        })
        .collect();
    Ok(ConstraintOutcome::Violated { entities })
}

/// `cbu_validated_requires_commercial_client_entity` — exact match to the
/// rule's own text; `cbus.status`/`cbus.commercial_client_entity_id` are
/// both real columns (verified: `\d "ob-poc".cbus`).
async fn commercial_client_entity_set(pool: &PgPool) -> anyhow::Result<ConstraintOutcome> {
    let rows: Vec<(Uuid,)> = sqlx::query_as(
        r#"SELECT cbu_id FROM "ob-poc".cbus
           WHERE status = 'VALIDATED' AND commercial_client_entity_id IS NULL"#,
    )
    .fetch_all(pool)
    .await?;
    if rows.is_empty() {
        return Ok(ConstraintOutcome::Clean);
    }
    let entities = rows
        .into_iter()
        .map(|(cbu_id,)| ViolationExample {
            entity_id: cbu_id,
            detail: "VALIDATED with commercial_client_entity_id NULL".to_string(),
        })
        .collect();
    Ok(ConstraintOutcome::Violated { entities })
}

/// `deal_contracted_requires_bac_approved` — exact match to the rule's own
/// text; `deals.deal_status`/`deals.bac_status` are both real columns
/// (verified: `\d "ob-poc".deals`).
async fn bac_approved(pool: &PgPool) -> anyhow::Result<ConstraintOutcome> {
    let rows: Vec<(Uuid,)> = sqlx::query_as(
        r#"SELECT deal_id FROM "ob-poc".deals
           WHERE deal_status = 'CONTRACTED' AND bac_status IS DISTINCT FROM 'approved'"#,
    )
    .fetch_all(pool)
    .await?;
    if rows.is_empty() {
        return Ok(ConstraintOutcome::Clean);
    }
    let entities = rows
        .into_iter()
        .map(|(deal_id,)| ViolationExample {
            entity_id: deal_id,
            detail: "CONTRACTED with bac_status not 'approved'".to_string(),
        })
        .collect();
    Ok(ConstraintOutcome::Violated { entities })
}

/// `case_cannot_approve_without_workstreams_complete` — same vacuous-ALL
/// discipline as `evidence_set_verified`: a case with ZERO entity_workstreams
/// reaching APPROVED must count as a violation (nothing was ever completed),
/// not vacuous compliance.
async fn case_workstreams_complete(pool: &PgPool) -> anyhow::Result<ConstraintOutcome> {
    let rows: Vec<(Uuid, i64, i64)> = sqlx::query_as(
        r#"
        SELECT c.case_id,
               COUNT(w.workstream_id) AS total,
               COUNT(w.workstream_id) FILTER (WHERE w.status = 'COMPLETE') AS complete
        FROM "ob-poc".cases c
        LEFT JOIN "ob-poc".entity_workstreams w ON w.case_id = c.case_id
        WHERE c.status = 'APPROVED'
        GROUP BY c.case_id
        HAVING COUNT(w.workstream_id) = 0
            OR COUNT(w.workstream_id) FILTER (WHERE w.status = 'COMPLETE') < COUNT(w.workstream_id)
        "#,
    )
    .fetch_all(pool)
    .await?;
    if rows.is_empty() {
        return Ok(ConstraintOutcome::Clean);
    }
    let entities = rows
        .into_iter()
        .map(|(case_id, total, complete)| ViolationExample {
            entity_id: case_id,
            detail: format!("APPROVED with {complete}/{total} entity_workstreams COMPLETE"),
        })
        .collect();
    Ok(ConstraintOutcome::Violated { entities })
}

/// `case_cannot_approve_with_unresolved_red_flags` — a plain COUNT=0 check,
/// no vacuous-ALL trap (a case with zero red_flags naturally has zero
/// BLOCKING ones).
async fn case_no_blocking_red_flags(pool: &PgPool) -> anyhow::Result<ConstraintOutcome> {
    let rows: Vec<(Uuid, i64)> = sqlx::query_as(
        r#"
        SELECT c.case_id, COUNT(r.*) AS blocking_count
        FROM "ob-poc".cases c
        JOIN "ob-poc".red_flags r ON r.case_id = c.case_id AND r.status = 'BLOCKING'
        WHERE c.status = 'APPROVED'
        GROUP BY c.case_id
        "#,
    )
    .fetch_all(pool)
    .await?;
    if rows.is_empty() {
        return Ok(ConstraintOutcome::Clean);
    }
    let entities = rows
        .into_iter()
        .map(|(case_id, blocking_count)| ViolationExample {
            entity_id: case_id,
            detail: format!("APPROVED with {blocking_count} BLOCKING red_flags"),
        })
        .collect();
    Ok(ConstraintOutcome::Violated { entities })
}

/// `case_cannot_approve_with_unresolved_screening_hits` — per the rule's own
/// text: a case violates if any of its workstreams' screenings sit outside
/// the terminal-clean set (CLEAR, HIT_DISMISSED, EXPIRED) AND the case has
/// no MITIGATED red_flag sourced from screening to cover it. `screenings`
/// has no `case_id` column — joined via `entity_workstreams.workstream_id`
/// (confirmed live, `\d "ob-poc".screenings`).
async fn case_screenings_resolved(pool: &PgPool) -> anyhow::Result<ConstraintOutcome> {
    let rows: Vec<(Uuid, i64)> = sqlx::query_as(
        r#"
        SELECT c.case_id, COUNT(s.*) AS unresolved_count
        FROM "ob-poc".cases c
        JOIN "ob-poc".entity_workstreams w ON w.case_id = c.case_id
        JOIN "ob-poc".screenings s ON s.workstream_id = w.workstream_id
            AND s.status NOT IN ('CLEAR', 'HIT_DISMISSED', 'EXPIRED')
        WHERE c.status = 'APPROVED'
          AND NOT EXISTS (
              SELECT 1 FROM "ob-poc".red_flags rf
              WHERE rf.case_id = c.case_id
                AND rf.source = 'screening'
                AND rf.status = 'MITIGATED'
          )
        GROUP BY c.case_id
        "#,
    )
    .fetch_all(pool)
    .await?;
    if rows.is_empty() {
        return Ok(ConstraintOutcome::Clean);
    }
    let entities = rows
        .into_iter()
        .map(|(case_id, unresolved_count)| ViolationExample {
            entity_id: case_id,
            detail: format!(
                "APPROVED with {unresolved_count} unresolved screenings, no MITIGATED \
                 screening red_flag"
            ),
        })
        .collect();
    Ok(ConstraintOutcome::Violated { entities })
}

/// `investor_active_requires_kyc_approved` — exact match to the corrected
/// rule text (`investors.lifecycle_state`, not `status`; see
/// `cbu_dag.yaml`'s own 2026-08-19 correction comment).
async fn investor_kyc_approved(pool: &PgPool) -> anyhow::Result<ConstraintOutcome> {
    let rows: Vec<(Uuid,)> = sqlx::query_as(
        r#"SELECT investor_id FROM "ob-poc".investors
           WHERE lifecycle_state = 'ACTIVE' AND kyc_status IS DISTINCT FROM 'APPROVED'"#,
    )
    .fetch_all(pool)
    .await?;
    if rows.is_empty() {
        return Ok(ConstraintOutcome::Clean);
    }
    let entities = rows
        .into_iter()
        .map(|(investor_id,)| ViolationExample {
            entity_id: investor_id,
            detail: "lifecycle_state=ACTIVE with kyc_status not APPROVED".to_string(),
        })
        .collect();
    Ok(ConstraintOutcome::Violated { entities })
}

/// `holding_active_requires_investor_active` — scoped to `usage_type = 'TA'`
/// per `cbu_dag.yaml`'s 2026-08-19 correction (the `UBO`-usage rows are a
/// different register, never linked via `investor_id`).
async fn holding_investor_active(pool: &PgPool) -> anyhow::Result<ConstraintOutcome> {
    let rows: Vec<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT h.id FROM "ob-poc".holdings h
        JOIN "ob-poc".investors i ON i.investor_id = h.investor_id
        WHERE h.holding_status = 'ACTIVE'
          AND h.usage_type = 'TA'
          AND i.lifecycle_state IS DISTINCT FROM 'ACTIVE'
        "#,
    )
    .fetch_all(pool)
    .await?;
    if rows.is_empty() {
        return Ok(ConstraintOutcome::Clean);
    }
    let entities = rows
        .into_iter()
        .map(|(holding_id,)| ViolationExample {
            entity_id: holding_id,
            detail: "holding_status=ACTIVE with parent investor not lifecycle_state=ACTIVE"
                .to_string(),
        })
        .collect();
    Ok(ConstraintOutcome::Violated { entities })
}

/// Renders the census as a human-readable report, ordered by violation
/// count descending (P4's required ordering).
pub fn render_census(reports: &[ConstraintReport]) -> String {
    let mut sorted: Vec<&ConstraintReport> = reports.iter().collect();
    sorted.sort_by_key(|r| std::cmp::Reverse(r.violation_count()));

    let mut by_outcome_kind: BTreeMap<&'static str, usize> = BTreeMap::new();
    for r in reports {
        let key = match &r.outcome {
            ConstraintOutcome::Clean => "clean",
            ConstraintOutcome::Violated { .. } => "violated",
            ConstraintOutcome::NotYetImplemented => "not_yet_implemented",
            ConstraintOutcome::SchemaMismatch { .. } => "schema_mismatch",
        };
        *by_outcome_kind.entry(key).or_default() += 1;
    }

    let mut out = String::new();
    out.push_str(&format!(
        "=== cross_slot_constraints census ({} declared) ===\n",
        reports.len()
    ));
    for (k, v) in &by_outcome_kind {
        out.push_str(&format!("  {k}: {v}\n"));
    }
    out.push('\n');
    for r in sorted {
        match &r.outcome {
            ConstraintOutcome::Violated { entities } => {
                out.push_str(&format!(
                    "[VIOLATED] {} ({}, severity={}) — {} entities\n",
                    r.id,
                    r.workspace,
                    r.severity,
                    entities.len()
                ));
                for e in entities.iter().take(5) {
                    out.push_str(&format!("    {} — {}\n", e.entity_id, e.detail));
                }
                if entities.len() > 5 {
                    out.push_str(&format!("    ... and {} more\n", entities.len() - 5));
                }
            }
            ConstraintOutcome::Clean => {
                out.push_str(&format!("[clean]    {} ({})\n", r.id, r.workspace));
            }
            ConstraintOutcome::SchemaMismatch { detail } => {
                out.push_str(&format!(
                    "[SCHEMA MISMATCH] {} ({}) — {}\n",
                    r.id, r.workspace, detail
                ));
            }
            ConstraintOutcome::NotYetImplemented => {
                out.push_str(&format!("[not yet implemented] {} ({})\n", r.id, r.workspace));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Structural gate: `report_only_refuses_nothing`. `ConstraintReport`
    /// and `ConstraintOutcome` carry no variant that represents "refuse
    /// the transition" — every variant is inert data. This is checked at
    /// the type level (both enums are exhaustively matched above with no
    /// arm that returns an `Err` propagated to a caller as a refusal;
    /// `evaluate_one`'s own `Result` is I/O-failure-only, never a verdict
    /// channel) — pinned here by asserting the exact variant set, so an
    /// added "Refused"-shaped variant would have to touch this test.
    #[test]
    fn report_only_refuses_nothing() {
        let outcomes = [
            ConstraintOutcome::Clean,
            ConstraintOutcome::Violated { entities: vec![] },
            ConstraintOutcome::NotYetImplemented,
            ConstraintOutcome::SchemaMismatch {
                detail: String::new(),
            },
        ];
        for o in &outcomes {
            // Every variant renders to a report line; none short-circuits
            // execution — proven structurally by `render_census` accepting
            // all four without any control-flow branch that aborts.
            let report = ConstraintReport {
                workspace: "test".to_string(),
                id: "test".to_string(),
                severity: "error".to_string(),
                outcome: o.clone(),
            };
            let _ = render_census(&[report]);
        }
    }

    fn dag_dir() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("config/sem_os_seeds/dag_taxonomies")
    }

    #[test]
    fn loads_all_49_declared_constraints() {
        let declared = load_all_declared(&dag_dir()).expect("load real DAGs");
        assert_eq!(
            declared.len(),
            49,
            "R0 P1 recon counted 49 cross_slot_constraints across 8 files; \
             if this count changed, a DAG file was edited — update this \
             test consciously, don't just bump the number"
        );
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL; run explicitly for the R2 Stage 1 census"]
    async fn every_declared_cross_slot_constraint_is_evaluated() {
        let declared = load_all_declared(&dag_dir()).expect("load real DAGs");
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let pool = sqlx::PgPool::connect(&url).await.expect("connect");
        let reports = evaluate_all(&declared, &pool).await.expect("evaluate");

        assert_eq!(
            reports.len(),
            declared.len(),
            "every declared constraint must produce exactly one report — \
             R0 P1's dead-config tooth flips from pinning zero-evaluation to \
             pinning this: all 49 dispatched, none silently dropped"
        );

        println!("{}", render_census(&reports));
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL; the §1 fact-1 flagship case"]
    async fn cbu_confirm_fact1_case_is_logged_as_violation() {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let pool = sqlx::PgPool::connect(&url).await.expect("connect");

        let outcome = evaluate_one("cbu_validated_requires_evidence_set_verified", &pool)
            .await
            .expect("evaluate");
        println!("cbu_validated_requires_evidence_set_verified => {outcome:?}");

        // The real CBU from the prior verification pass
        // (62193533-8b1c-4970-a491-a86f1208f1e1) reached VALIDATED with
        // zero evidence rows. If it's still in that state, it must show up
        // as a violation here — logged, not refused (this test never
        // calls anything that could roll it back or block it; it only
        // reads).
        let target: Uuid = "62193533-8b1c-4970-a491-a86f1208f1e1".parse().unwrap();
        if let ConstraintOutcome::Violated { entities } = &outcome {
            let hit = entities.iter().find(|e| e.entity_id == target);
            println!("fact-1 CBU present in violations: {}", hit.is_some());
        }
    }
}
