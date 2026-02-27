//! xtask commands for the Semantic Registry.
//!
//! Usage: `cargo x sem-reg <subcommand>`

use anyhow::{Context, Result};
use sqlx::PgPool;

use ob_poc::sem_reg::types::{
    pg_rows_to_snapshot_rows, ChangeType, PgSnapshotRow, SnapshotMeta, SnapshotRow,
};
use ob_poc::sem_reg::{ObjectType, RegistryService, SnapshotStore};

/// Show registry statistics (counts by object type).
pub async fn stats() -> Result<()> {
    let pool = connect().await?;
    let counts = RegistryService::stats(&pool).await?;

    if counts.is_empty() {
        println!("Registry is empty — no active snapshots found.");
        println!("\nHint: Run migrations first, then `cargo x sem-reg scan` to bootstrap.");
        return Ok(());
    }

    println!("Semantic Registry Statistics");
    println!("============================");
    let mut total = 0i64;
    for (obj_type, count) in &counts {
        println!("  {:<25} {:>6}", obj_type.to_string(), count);
        total += count;
    }
    println!("  {:<25} {:>6}", "TOTAL", total);

    Ok(())
}

/// Describe an attribute definition by FQN.
pub async fn attr_describe(fqn: &str) -> Result<()> {
    let pool = connect().await?;
    match RegistryService::resolve_attribute_def_by_fqn(&pool, fqn).await? {
        Some((row, body)) => {
            println!("Attribute Definition: {}", body.fqn);
            println!("  Name:           {}", body.name);
            println!("  Domain:         {}", body.domain);
            println!("  Description:    {}", body.description);
            println!("  Data Type:      {:?}", body.data_type);
            println!("  Version:        {}", row.version_string());
            println!("  Tier:           {:?}", row.governance_tier);
            println!("  Trust:          {:?}", row.trust_class);
            println!("  Created By:     {}", row.created_by);
            println!("  Effective From: {}", row.effective_from);
            if let Some(source) = &body.source {
                if let Some(verb) = &source.producing_verb {
                    println!("  Producing Verb: {}", verb);
                }
            }
            if !body.sinks.is_empty() {
                println!("  Sinks:");
                for sink in &body.sinks {
                    println!("    - {} (arg: {})", sink.consuming_verb, sink.arg_name);
                }
            }
        }
        None => {
            println!("No active attribute definition found with FQN: {}", fqn);
        }
    }
    Ok(())
}

/// List active attribute definitions.
pub async fn attr_list(limit: i64) -> Result<()> {
    let pool = connect().await?;
    let rows = SnapshotStore::list_active(&pool, ObjectType::AttributeDef, limit, 0).await?;
    if rows.is_empty() {
        println!("No active attribute definitions found.");
        return Ok(());
    }
    println!(
        "{:<40} {:<15} {:<10} {:<12}",
        "FQN", "DOMAIN", "VERSION", "TIER"
    );
    println!("{}", "-".repeat(80));
    for row in &rows {
        let fqn = row
            .definition
            .get("fqn")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let domain = row
            .definition
            .get("domain")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        println!(
            "{:<40} {:<15} {:<10} {:<12?}",
            fqn,
            domain,
            row.version_string(),
            row.governance_tier,
        );
    }
    println!("\n{} attribute definitions", rows.len());
    Ok(())
}

/// Describe an entity type definition by FQN.
pub async fn entity_type_describe(fqn: &str) -> Result<()> {
    let pool = connect().await?;
    match RegistryService::resolve_entity_type_def_by_fqn(&pool, fqn).await? {
        Some((row, body)) => {
            println!("Entity Type Definition: {}", body.fqn);
            println!("  Name:           {}", body.name);
            println!("  Domain:         {}", body.domain);
            println!("  Description:    {}", body.description);
            println!("  Version:        {}", row.version_string());
            println!("  Tier:           {:?}", row.governance_tier);
            println!("  Trust:          {:?}", row.trust_class);
            if let Some(parent) = &body.parent_type {
                println!("  Parent Type:    {}", parent);
            }
            if let Some(db) = &body.db_table {
                println!("  DB Table:       {}.{}", db.schema, db.table);
            }
            if !body.required_attributes.is_empty() {
                println!("  Required Attrs: {}", body.required_attributes.join(", "));
            }
            if !body.lifecycle_states.is_empty() {
                println!("  States:");
                for state in &body.lifecycle_states {
                    let transitions: Vec<&str> =
                        state.transitions.iter().map(|t| t.to.as_str()).collect();
                    if transitions.is_empty() {
                        println!("    - {} (terminal: {})", state.name, state.terminal);
                    } else {
                        println!("    - {} -> [{}]", state.name, transitions.join(", "));
                    }
                }
            }
        }
        None => {
            println!("No active entity type definition found with FQN: {}", fqn);
        }
    }
    Ok(())
}

/// Describe a verb contract by FQN.
pub async fn verb_describe(fqn: &str) -> Result<()> {
    let pool = connect().await?;
    match RegistryService::resolve_verb_contract_by_fqn(&pool, fqn).await? {
        Some((row, body)) => {
            println!("Verb Contract: {}", body.fqn);
            println!("  Domain:         {}", body.domain);
            println!("  Action:         {}", body.action);
            println!("  Description:    {}", body.description);
            println!("  Behavior:       {}", body.behavior);
            println!("  Version:        {}", row.version_string());
            println!("  Tier:           {:?}", row.governance_tier);
            println!("  Trust:          {:?}", row.trust_class);
            println!("  Created By:     {}", row.created_by);

            if !body.args.is_empty() {
                println!("  Arguments:");
                for arg in &body.args {
                    let req = if arg.required { "required" } else { "optional" };
                    println!("    - {} ({}, {})", arg.name, arg.arg_type, req);
                    if let Some(desc) = &arg.description {
                        println!("      {}", desc);
                    }
                }
            }

            if let Some(ret) = &body.returns {
                println!("  Returns:        {}", ret.return_type);
            }

            if let Some(produces) = &body.produces {
                println!(
                    "  Produces:       {} (resolved: {})",
                    produces.entity_type, produces.resolved
                );
            }

            if !body.preconditions.is_empty() {
                println!("  Preconditions:");
                for pre in &body.preconditions {
                    println!("    - {}: {}", pre.kind, pre.value);
                }
            }

            if !body.invocation_phrases.is_empty() {
                let shown: Vec<&str> = body
                    .invocation_phrases
                    .iter()
                    .take(5)
                    .map(|s| s.as_str())
                    .collect();
                println!("  Phrases:        {}", shown.join(", "));
                if body.invocation_phrases.len() > 5 {
                    println!(
                        "                  ... and {} more",
                        body.invocation_phrases.len() - 5
                    );
                }
            }
        }
        None => {
            println!("No active verb contract found with FQN: {}", fqn);
        }
    }
    Ok(())
}

/// List active verb contracts.
pub async fn verb_list(limit: i64) -> Result<()> {
    let pool = connect().await?;
    let rows = SnapshotStore::list_active(&pool, ObjectType::VerbContract, limit, 0).await?;
    if rows.is_empty() {
        println!("No active verb contracts found.");
        return Ok(());
    }
    println!(
        "{:<35} {:<12} {:<10} {:<12} {:<10}",
        "FQN", "BEHAVIOR", "VERSION", "TIER", "ARGS"
    );
    println!("{}", "-".repeat(82));
    for row in &rows {
        let fqn = row
            .definition
            .get("fqn")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let behavior = row
            .definition
            .get("behavior")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let args_count = row
            .definition
            .get("args")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        println!(
            "{:<35} {:<12} {:<10} {:<12?} {:<10}",
            fqn,
            behavior,
            row.version_string(),
            row.governance_tier,
            args_count,
        );
    }
    println!("\n{} verb contracts", rows.len());
    Ok(())
}

/// Show snapshot history for an object.
pub async fn history(object_type_str: &str, fqn: &str) -> Result<()> {
    let pool = connect().await?;

    let object_type = parse_object_type(object_type_str)?;

    // Find the object by FQN first
    let current =
        SnapshotStore::find_active_by_definition_field(&pool, object_type, "fqn", fqn).await?;

    match current {
        Some(row) => {
            let history = SnapshotStore::load_history(&pool, object_type, row.object_id).await?;
            println!("History for {} ({}):", fqn, object_type_str);
            println!(
                "{:<38} {:<10} {:<12} {:<15} {:<20}",
                "SNAPSHOT_ID", "VERSION", "STATUS", "CHANGE_TYPE", "EFFECTIVE_FROM"
            );
            println!("{}", "-".repeat(98));
            for h in &history {
                println!(
                    "{:<38} {:<10} {:<12?} {:<15?} {:<20}",
                    h.snapshot_id,
                    h.version_string(),
                    h.status,
                    h.change_type,
                    h.effective_from.format("%Y-%m-%d %H:%M:%S"),
                );
            }
            println!("\n{} snapshots", history.len());
        }
        None => {
            println!("No active {} found with FQN: {}", object_type_str, fqn);
        }
    }
    Ok(())
}

/// Run the onboarding scan (bootstrap registry from verb YAML).
pub async fn scan(dry_run: bool, verbose: bool) -> Result<()> {
    let pool = connect().await?;
    println!("Scanning verb YAML definitions...\n");
    let report = ob_poc::sem_reg::scanner::run_onboarding_scan(&pool, dry_run, verbose).await?;
    println!("\n{}", report);
    Ok(())
}

/// Describe a derivation spec by FQN.
pub async fn derivation_describe(fqn: &str) -> Result<()> {
    let pool = connect().await?;
    match RegistryService::resolve_derivation_spec_by_fqn(&pool, fqn).await? {
        Some((row, body)) => {
            println!("Derivation Spec: {}", body.fqn);
            println!("  Name:              {}", body.name);
            println!("  Description:       {}", body.description);
            println!("  Output Attribute:  {}", body.output_attribute_fqn);
            println!("  Version:           {}", row.version_string());
            println!("  Tier:              {:?}", row.governance_tier);
            println!("  Trust:             {:?}", row.trust_class);
            println!("  Evidence Grade:    {:?}", body.evidence_grade);
            println!("  Null Semantics:    {:?}", body.null_semantics);
            println!("  Security Inherit:  {:?}", body.security_inheritance);
            if let Some(ref freshness) = body.freshness_rule {
                println!("  Max Age:           {}s", freshness.max_age_seconds);
            }
            println!("  Expression:        {:?}", body.expression);
            if !body.inputs.is_empty() {
                println!("  Inputs:");
                for input in &body.inputs {
                    let req = if input.required {
                        "required"
                    } else {
                        "optional"
                    };
                    println!(
                        "    - {} (role: {}, {})",
                        input.attribute_fqn, input.role, req
                    );
                }
            }
            if !body.tests.is_empty() {
                println!("  Test Cases:        {}", body.tests.len());
            }
        }
        None => {
            println!("No active derivation spec found with FQN: {}", fqn);
        }
    }
    Ok(())
}

/// Backfill security labels on existing snapshots using heuristics.
pub async fn backfill_labels(dry_run: bool) -> Result<()> {
    let pool = connect().await?;

    // Query snapshots with default/empty security labels
    let pg_rows = sqlx::query_as::<_, PgSnapshotRow>(
        "SELECT * FROM sem_reg.snapshots \
         WHERE status = 'active' \
           AND effective_until IS NULL \
           AND (security_label IS NULL \
                OR security_label = '{}'::jsonb \
                OR security_label = '{\"classification\":\"internal\"}'::jsonb) \
         ORDER BY object_type, definition->>'fqn'",
    )
    .fetch_all(&pool)
    .await?;
    let rows: Vec<SnapshotRow> =
        pg_rows_to_snapshot_rows(pg_rows).context("Failed to parse snapshot rows")?;

    if rows.is_empty() {
        println!("All active snapshots already have non-default security labels.");
        return Ok(());
    }

    println!(
        "Found {} snapshots with default security labels",
        rows.len()
    );
    if dry_run {
        println!("(dry run — no changes will be written)\n");
    }

    let mut updated = 0;
    for row in &rows {
        let fqn = row
            .definition
            .get("fqn")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let domain = row
            .definition
            .get("domain")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let tags: Vec<String> = row
            .definition
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let suggested = ob_poc::sem_reg::scanner::suggest_security_label(fqn, domain, &tags);
        println!(
            "  {} ({:?}) → classification={:?}, pii={}",
            fqn, row.object_type, suggested.classification, suggested.pii
        );

        if !dry_run {
            // Publish a successor snapshot with the new security label.
            // This preserves immutability — the original snapshot is superseded.
            let meta = SnapshotMeta {
                object_type: row.object_type,
                object_id: row.object_id,
                version_major: row.version_major,
                version_minor: row.version_minor + 1,
                status: row.status,
                governance_tier: row.governance_tier,
                trust_class: row.trust_class,
                security_label: suggested,
                predecessor_id: Some(row.snapshot_id),
                change_type: ChangeType::NonBreaking,
                change_rationale: Some("Security label backfill".into()),
                created_by: "xtask-backfill".into(),
                approved_by: None,
            };
            SnapshotStore::publish_snapshot(&pool, &meta, &row.definition, None).await?;
            updated += 1;
        }
    }

    if dry_run {
        println!(
            "\nDry run complete. {} snapshots would be updated.",
            rows.len()
        );
    } else {
        println!("\nUpdated {} snapshots.", updated);
    }
    Ok(())
}

/// Run all publish gates against active snapshots and report results.
pub async fn validate(enforce: bool) -> Result<()> {
    use ob_poc::sem_reg::gates::{evaluate_publish_gates, GateMode};
    use ob_poc::sem_reg::gates_technical::check_security_label_presence;

    let pool = connect().await?;
    let mode = if enforce {
        GateMode::Enforce
    } else {
        GateMode::ReportOnly
    };

    let pg_rows = sqlx::query_as::<_, PgSnapshotRow>(
        "SELECT * FROM sem_reg.snapshots \
         WHERE status = 'active' AND effective_until IS NULL \
         ORDER BY object_type, definition->>'fqn'",
    )
    .fetch_all(&pool)
    .await?;
    let rows: Vec<SnapshotRow> =
        pg_rows_to_snapshot_rows(pg_rows).context("Failed to parse snapshot rows")?;

    if rows.is_empty() {
        println!("No active snapshots found.");
        return Ok(());
    }

    println!(
        "Validating {} active snapshots (mode: {:?})\n",
        rows.len(),
        mode
    );

    let mut total_errors = 0;
    let mut total_warnings = 0;

    for row in &rows {
        let fqn = row
            .definition
            .get("fqn")
            .and_then(|v| v.as_str())
            .unwrap_or("?");

        // Standard publish gates (using a synthetic SnapshotMeta from the row)
        let meta = ob_poc::sem_reg::SnapshotMeta {
            object_type: row.object_type,
            object_id: row.object_id,
            version_major: row.version_major,
            version_minor: row.version_minor,
            status: row.status,
            governance_tier: row.governance_tier,
            trust_class: row.trust_class,
            security_label: row.parse_security_label().unwrap_or_default(),
            change_type: row.change_type,
            change_rationale: row.change_rationale.clone(),
            created_by: row.created_by.clone(),
            approved_by: row.approved_by.clone(),
            predecessor_id: row.predecessor_id,
        };
        let standard = evaluate_publish_gates(&meta, None);
        if !standard.all_passed() {
            for msg in standard.failure_messages() {
                println!("  ERROR  {}: {}", fqn, msg);
                total_errors += 1;
            }
        }

        // Security label presence check
        let label_failures = check_security_label_presence(row);
        for f in &label_failures {
            println!("  {:?}  {}: {}", f.severity, fqn, f.message);
            match f.severity {
                ob_poc::sem_reg::gates::GateSeverity::Error => total_errors += 1,
                ob_poc::sem_reg::gates::GateSeverity::Warning => total_warnings += 1,
            }
        }
    }

    println!("\nValidation complete:");
    println!("  {} snapshots checked", rows.len());
    println!("  {} errors", total_errors);
    println!("  {} warnings", total_warnings);

    if enforce && total_errors > 0 {
        anyhow::bail!("{} gate errors in enforce mode", total_errors);
    }

    Ok(())
}

/// Resolve context for a subject using the 12-step pipeline.
pub async fn ctx_resolve(
    subject_str: &str,
    subject_type: &str,
    actor_type: &str,
    mode_str: &str,
    as_of_str: Option<&str>,
    json_output: bool,
) -> Result<()> {
    use ob_poc::sem_reg::context_resolution::{ContextResolutionRequest, EvidenceMode, SubjectRef};
    use ob_poc::sem_reg::{abac::ActorContext, resolve_context};

    let pool = connect().await?;

    // Parse subject ID
    let subject_id: uuid::Uuid = subject_str
        .parse()
        .context("Invalid subject UUID. Provide a valid UUID.")?;

    // Parse subject type
    let subject = match subject_type {
        "case" => SubjectRef::CaseId(subject_id),
        "entity" => SubjectRef::EntityId(subject_id),
        "document" => SubjectRef::DocumentId(subject_id),
        "task" => SubjectRef::TaskId(subject_id),
        "view" => SubjectRef::ViewId(subject_id),
        _ => anyhow::bail!(
            "Unknown subject type: '{}'. Valid: case, entity, document, task, view",
            subject_type
        ),
    };

    // Parse evidence mode
    let evidence_mode = match mode_str {
        "strict" => EvidenceMode::Strict,
        "normal" => EvidenceMode::Normal,
        "exploratory" => EvidenceMode::Exploratory,
        "governance" => EvidenceMode::Governance,
        _ => anyhow::bail!(
            "Unknown mode: '{}'. Valid: strict, normal, exploratory, governance",
            mode_str
        ),
    };

    // Parse point-in-time
    let point_in_time = match as_of_str {
        Some(s) => {
            let dt = chrono::DateTime::parse_from_rfc3339(s)
                .context("Invalid as-of date. Use ISO 8601 format (e.g., 2024-01-15T00:00:00Z).")?;
            Some(dt.with_timezone(&chrono::Utc))
        }
        None => None,
    };

    // Build actor context from actor type string
    let actor = match actor_type {
        "agent" => ActorContext {
            actor_id: "cli-agent".to_string(),
            roles: vec!["agent".to_string()],
            department: Some("operations".to_string()),
            clearance: Some(ob_poc::sem_reg::Classification::Internal),
            jurisdictions: vec![],
        },
        "analyst" => ActorContext {
            actor_id: "cli-analyst".to_string(),
            roles: vec!["analyst".to_string()],
            department: Some("compliance".to_string()),
            clearance: Some(ob_poc::sem_reg::Classification::Confidential),
            jurisdictions: vec![],
        },
        "governance" => ActorContext {
            actor_id: "cli-governance".to_string(),
            roles: vec!["governance".to_string(), "steward".to_string()],
            department: Some("governance".to_string()),
            clearance: Some(ob_poc::sem_reg::Classification::Restricted),
            jurisdictions: vec![],
        },
        _ => anyhow::bail!(
            "Unknown actor type: '{}'. Valid: agent, analyst, governance",
            actor_type
        ),
    };

    let req = ContextResolutionRequest {
        subject,
        intent: None,
        actor,
        goals: vec![],
        constraints: Default::default(),
        evidence_mode,
        point_in_time,
        entity_kind: None,
    };

    let response = resolve_context(&pool, &req).await?;

    if json_output {
        let json = serde_json::to_string_pretty(&response)?;
        println!("{}", json);
    } else {
        println!("Context Resolution Results");
        println!("==========================");
        println!(
            "  As-of:       {}",
            response.as_of_time.format("%Y-%m-%d %H:%M:%S UTC")
        );
        println!(
            "  Resolved at: {}",
            response.resolved_at.format("%Y-%m-%d %H:%M:%S UTC")
        );
        println!("  Confidence:  {:.2}", response.confidence);

        // Views
        println!(
            "\n  Applicable Views ({}):",
            response.applicable_views.len()
        );
        for view in &response.applicable_views {
            println!(
                "    - {} (overlap: {:.2}, entity_type: {})",
                view.fqn, view.overlap_score, view.body.base_entity_type
            );
        }

        // Verbs
        println!("\n  Candidate Verbs ({}):", response.candidate_verbs.len());
        for verb in &response.candidate_verbs {
            let proof_tag = if verb.usable_for_proof {
                " [proof]"
            } else {
                ""
            };
            println!(
                "    - {} (rank: {:.2}, tier: {:?}, access: {:?}){}",
                verb.fqn, verb.rank_score, verb.governance_tier, verb.access_decision, proof_tag
            );
        }

        // Attributes
        println!(
            "\n  Candidate Attributes ({}):",
            response.candidate_attributes.len()
        );
        for attr in &response.candidate_attributes {
            let req_tag = if attr.required { " [required]" } else { "" };
            println!(
                "    - {} (rank: {:.2}, tier: {:?}){}",
                attr.fqn, attr.rank_score, attr.governance_tier, req_tag
            );
        }

        // Policies
        if !response.policy_verdicts.is_empty() {
            println!("\n  Policy Verdicts ({}):", response.policy_verdicts.len());
            for verdict in &response.policy_verdicts {
                let status = if verdict.allowed { "allow" } else { "deny" };
                println!(
                    "    - {} [{}]: {}",
                    verdict.policy_fqn, status, verdict.reason
                );
            }
        }

        // Governance signals
        if !response.governance_signals.is_empty() {
            println!(
                "\n  Governance Signals ({}):",
                response.governance_signals.len()
            );
            for signal in &response.governance_signals {
                println!(
                    "    - [{:?}] {:?}: {}",
                    signal.severity, signal.kind, signal.message
                );
            }
        }

        // Disambiguation
        if !response.disambiguation_questions.is_empty() {
            println!(
                "\n  Disambiguation Questions ({}):",
                response.disambiguation_questions.len()
            );
            for q in &response.disambiguation_questions {
                println!("    - {}", q.question);
                for opt in &q.options {
                    println!("      * {} — {}", opt.id, opt.label);
                }
            }
        }

        // Access decision
        println!("\n  Security Handling: {:?}", response.security_handling);
    }

    Ok(())
}

/// List available Semantic Registry MCP tools.
pub async fn agent_tools() -> Result<()> {
    let specs = ob_poc::sem_reg::all_tool_specs();

    println!("Semantic Registry MCP Tools ({} tools)", specs.len());
    println!("==========================================\n");

    // Group by category (first word of description heuristic, or name prefix)
    let mut current_category = String::new();
    for spec in &specs {
        let category = spec.name.split('_').nth(2).unwrap_or("other");
        let cat = match category {
            "describe" | "search" | "list" => "Registry Query",
            "taxonomy" | "classify" => "Taxonomy",
            "verb" | "attribute" => {
                if spec.name.contains("surface") || spec.name.contains("producers") {
                    "Impact / Lineage"
                } else {
                    "Registry Query"
                }
            }
            "impact" | "lineage" | "regulation" => "Impact / Lineage",
            "resolve" | "apply" => "Context Resolution",
            "create" | "add" | "validate" | "execute" | "record" => "Planning / Decisions",
            "check" | "identify" => "Evidence",
            _ => "Other",
        };

        if cat != current_category {
            if !current_category.is_empty() {
                println!();
            }
            println!("── {} ──", cat);
            current_category = cat.to_string();
        }

        let param_count = spec.parameters.len();
        let required = spec.parameters.iter().filter(|p| p.required).count();
        println!(
            "  {:<40} params: {} ({} required)",
            spec.name, param_count, required
        );
        println!("    {}", spec.description);
    }

    println!("\n{} tools total", specs.len());
    Ok(())
}

/// Show governance coverage report.
pub async fn coverage(tier_str: &str, json_output: bool) -> Result<()> {
    let pool = connect().await?;

    let tier_filter = match tier_str {
        "governed" => Some("governed".to_string()),
        "operational" => Some("operational".to_string()),
        "all" => None,
        _ => anyhow::bail!(
            "Unknown tier: '{}'. Valid: governed, operational, all",
            tier_str
        ),
    };

    let report =
        ob_poc::sem_reg::MetricsStore::coverage_report(&pool, tier_filter.as_deref()).await?;

    if json_output {
        let json = serde_json::to_string_pretty(&report)?;
        println!("{}", json);
    } else {
        println!("Governance Coverage Report");
        println!("==========================");
        if let Some(ref tier) = report.filter_tier {
            println!("  Filter:                         {}", tier);
        }
        println!(
            "  Snapshot Volume:                {}",
            report.snapshot_volume
        );
        println!(
            "  Tier Distribution:              governed={}, operational={}",
            report.tier_distribution.governed, report.tier_distribution.operational
        );
        println!(
            "\n  Classification Coverage:        {:.1}%",
            report.classification_coverage_pct
        );
        println!(
            "  Stewardship Coverage:           {:.1}%",
            report.stewardship_coverage_pct
        );
        println!(
            "  Policy Attachment:              {:.1}%",
            report.policy_attachment_pct
        );
        println!(
            "  Evidence Freshness:             {:.1}%",
            report.evidence_freshness_pct
        );
        println!(
            "  Security Label Completeness:    {:.1}%",
            report.security_label_completeness_pct
        );
        println!(
            "  Retention Compliance:           {}",
            if report.retention_compliance {
                "PASS"
            } else {
                "FAIL"
            }
        );
        println!(
            "  Proof Rule Compliance:          {}",
            if report.proof_rule_compliance {
                "PASS"
            } else {
                "FAIL"
            }
        );
    }

    Ok(())
}

// ── Onboarding Pipeline ──────────────────────────────────────

const DEFAULT_MANIFEST_PATH: &str = "data/onboarding-manifest.json";

/// Run the 5-step extraction pipeline and write an onboarding manifest.
pub async fn onboard_scan(verbose: bool) -> Result<()> {
    use ob_poc::sem_reg::onboarding::{entity_infer, manifest, schema_extract, verb_extract, xref};

    println!("Registry Onboarding Scan");
    println!("========================\n");

    // Step 1: Extract verb signatures from DSL YAML
    println!("Step 1/5: Extracting verb signatures from YAML...");
    let loader = dsl_core::config::loader::ConfigLoader::from_env();
    let verbs_config = loader
        .load_verbs()
        .context("Failed to load verb configuration from YAML")?;
    let verb_extracts = verb_extract::extract_verbs(&verbs_config);
    println!("  → {} verbs extracted", verb_extracts.len());

    if verbose {
        for v in &verb_extracts {
            println!(
                "    {} ({:?}, {} inputs, {} side-effects)",
                v.fqn,
                v.behavior,
                v.inputs.len(),
                v.side_effects.len()
            );
        }
    }

    // Step 2: Introspect PostgreSQL schema
    println!("\nStep 2/5: Introspecting PostgreSQL schema...");
    let pool = connect().await?;
    let tables = schema_extract::extract_schema(&pool, schema_extract::DEFAULT_SCHEMAS)
        .await
        .context("Failed to extract schema from database")?;

    let total_columns: usize = tables.iter().map(|t| t.columns.len()).sum();
    println!("  → {} tables, {} columns", tables.len(), total_columns);

    if verbose {
        for t in &tables {
            println!(
                "    {}.{} ({} cols, {} PKs, {} FKs)",
                t.schema,
                t.table_name,
                t.columns.len(),
                t.primary_keys.len(),
                t.foreign_keys.len()
            );
        }
    }

    // Step 3: Cross-reference verbs ↔ schema
    println!("\nStep 3/5: Cross-referencing verbs ↔ schema columns...");
    let xref_result = xref::cross_reference(&verb_extracts, &tables);
    println!(
        "  → {} candidates: {} verb-connected, {} framework, {} orphans, {} dead",
        xref_result.candidates.len(),
        xref_result.verb_connected,
        xref_result.framework,
        xref_result.operational_orphans,
        xref_result.dead_schema,
    );

    // Step 4: Infer entity types and relationships
    println!("\nStep 4/5: Inferring entity types and relationships...");
    let entity_types = entity_infer::infer_entity_types(&xref_result.candidates, &tables);
    let relationships = entity_infer::infer_relationships(&tables);
    println!(
        "  → {} entity types, {} relationships",
        entity_types.len(),
        relationships.len()
    );

    if verbose {
        for et in &entity_types {
            println!(
                "    {} ({} attrs, {} verb-connected, {} orphans)",
                et.fqn,
                et.attribute_fqns.len(),
                et.verb_connected_count,
                et.orphan_count
            );
        }
    }

    // Step 5: Assemble manifest
    println!("\nStep 5/5: Assembling onboarding manifest...");
    let onboarding_manifest = manifest::assemble_manifest(
        &std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".into()),
        tables.len(),
        total_columns,
        verb_extracts,
        xref_result,
        entity_types,
        relationships,
    );

    // Write manifest
    let manifest_path = std::path::Path::new(DEFAULT_MANIFEST_PATH);
    manifest::write_manifest(&onboarding_manifest, manifest_path)
        .context("Failed to write manifest")?;
    println!("  → Manifest written to {}", DEFAULT_MANIFEST_PATH);

    // Print summary
    println!("\n── Summary ────────────────────────────────────────");
    println!(
        "  Tables scanned:       {}",
        onboarding_manifest.tables_scanned
    );
    println!(
        "  Columns scanned:      {}",
        onboarding_manifest.columns_scanned
    );
    println!(
        "  Verbs extracted:      {}",
        onboarding_manifest.verbs_extracted
    );
    println!(
        "  Verb-connected attrs: {}",
        onboarding_manifest.verb_connected_attrs
    );
    println!(
        "  Framework columns:    {}",
        onboarding_manifest.framework_columns
    );
    println!(
        "  Operational orphans:  {}",
        onboarding_manifest.operational_orphans
    );
    println!(
        "  Dead schema:          {}",
        onboarding_manifest.dead_schema
    );
    println!(
        "  Entity types:         {}",
        onboarding_manifest.entity_type_candidates.len()
    );
    println!(
        "  Relationships:        {}",
        onboarding_manifest.relationship_candidates.len()
    );
    println!(
        "  Wiring completeness:  {:.1}% ({} fully / {} partial / {} unwired)",
        onboarding_manifest.wiring_pct,
        onboarding_manifest.verbs_fully_wired,
        onboarding_manifest.verbs_partially_wired,
        onboarding_manifest.verbs_unwired,
    );

    if onboarding_manifest.wiring_pct < 80.0 {
        println!("\n  ⚠ Wiring below 80% target — review orphan columns and unmapped verbs");
    }

    Ok(())
}

/// Display a summary report from an onboarding manifest file.
pub async fn onboard_report(manifest_path: Option<&str>) -> Result<()> {
    use ob_poc::sem_reg::onboarding::manifest;

    let path_str = manifest_path.unwrap_or(DEFAULT_MANIFEST_PATH);
    let path = std::path::Path::new(path_str);

    let m = manifest::read_manifest(path)
        .context(format!("Failed to read manifest from {}", path_str))?;

    println!("Onboarding Manifest Report");
    println!("==========================\n");
    println!("  Source DB:            {}", m.source_db);
    println!("  Extracted at:         {}", m.extracted_at);
    println!("  Tables scanned:       {}", m.tables_scanned);
    println!("  Columns scanned:      {}", m.columns_scanned);
    println!("  Verbs extracted:      {}", m.verbs_extracted);

    println!("\n── Column Classification ───────────────────────────");
    println!(
        "  Verb-connected:       {} (seeded as AttributeDef)",
        m.verb_connected_attrs
    );
    println!(
        "  Framework:            {} (NOT seeded)",
        m.framework_columns
    );
    println!(
        "  Operational orphans:  {} (seeded with verb_orphan=true)",
        m.operational_orphans
    );
    println!("  Dead schema:          {} (NOT seeded)", m.dead_schema);

    println!("\n── Wiring Completeness ─────────────────────────────");
    println!("  Fully wired:          {}", m.verbs_fully_wired);
    println!("  Partially wired:      {}", m.verbs_partially_wired);
    println!("  Unwired:              {}", m.verbs_unwired);
    println!("  Wiring percentage:    {:.1}%", m.wiring_pct);

    println!(
        "\n── Entity Types ({}) ──────────────────────────────",
        m.entity_type_candidates.len()
    );
    for et in &m.entity_type_candidates {
        println!(
            "  {:<40} {} attrs ({} verb, {} orphan)",
            et.fqn,
            et.attribute_fqns.len(),
            et.verb_connected_count,
            et.orphan_count,
        );
    }

    println!(
        "\n── Relationships ({}) ─────────────────────────────",
        m.relationship_candidates.len()
    );
    for rel in &m.relationship_candidates {
        println!(
            "  {:<50} {:?} ({:?})",
            rel.fqn, rel.edge_class, rel.cardinality,
        );
    }

    println!("\n── Bootstrap Seed Summary ──────────────────────────");
    let seedable_attrs = m
        .attribute_candidates
        .iter()
        .filter(|c| {
            matches!(
                c.classification,
                ob_poc::sem_reg::onboarding::xref::ColumnClassification::VerbConnected
                    | ob_poc::sem_reg::onboarding::xref::ColumnClassification::OperationalOrphan
            )
        })
        .count();
    println!("  AttributeDefs to seed:          {}", seedable_attrs);
    println!(
        "  VerbContracts to seed:          {}",
        m.verb_extracts.len()
    );
    println!(
        "  EntityTypeDefs to seed:         {}",
        m.entity_type_candidates.len()
    );
    println!(
        "  RelationshipTypeDefs to seed:   {}",
        m.relationship_candidates.len()
    );

    println!("\n  NOT seeded (Phase B2+):");
    println!("    PolicyRules, EvidenceRequirements, TaxonomyMemberships,");
    println!("    SecurityLabels, Templates, VerbBindings");

    Ok(())
}

/// Apply bootstrap seed from manifest to sem_reg.snapshots.
pub async fn onboard_apply(manifest_path: Option<&str>) -> Result<()> {
    use ob_poc::sem_reg::onboarding::{manifest, seed};

    let path_str = manifest_path.unwrap_or(DEFAULT_MANIFEST_PATH);
    let path = std::path::Path::new(path_str);

    let m = manifest::read_manifest(path)
        .context(format!("Failed to read manifest from {}", path_str))?;

    println!("Registry Bootstrap Seed");
    println!("=======================\n");
    println!("  Manifest: {}", path_str);
    println!("  Source DB: {}", m.source_db);
    println!("  Extracted at: {}", m.extracted_at);

    let pool = connect().await?;
    let report = seed::apply_bootstrap(&pool, &m)
        .await
        .context("Bootstrap seed failed")?;

    println!("\n── Bootstrap Report ────────────────────────────────");
    println!(
        "  AttributeDefs written:          {}",
        report.attribute_defs_written
    );
    println!(
        "  AttributeDefs skipped:          {}",
        report.attribute_defs_skipped
    );
    println!(
        "  VerbContracts written:          {}",
        report.verb_contracts_written
    );
    println!(
        "  VerbContracts skipped:          {}",
        report.verb_contracts_skipped
    );
    println!(
        "  EntityTypeDefs written:         {}",
        report.entity_type_defs_written
    );
    println!(
        "  EntityTypeDefs skipped:         {}",
        report.entity_type_defs_skipped
    );
    println!(
        "  RelationshipTypeDefs written:   {}",
        report.relationship_type_defs_written
    );
    println!(
        "  RelationshipTypeDefs skipped:   {}",
        report.relationship_type_defs_skipped
    );
    println!(
        "  Total snapshots:                {}",
        report.total_written()
    );

    if report.total_written() == 0 {
        println!("\n  ℹ No new snapshots written — bootstrap may have already been applied.");
    }

    Ok(())
}

// ── Authoring Pipeline CLI ────────────────────────────────────

/// List authoring pipeline ChangeSets with optional status filter.
pub async fn authoring_list(status: Option<&str>, limit: i64) -> Result<()> {
    let pool = connect().await?;
    let store = sem_os_postgres::PgAuthoringStore::new(pool);

    let status_filter = match status {
        Some(s) => {
            let parsed =
                sem_os_core::authoring::types::ChangeSetStatus::parse(s).ok_or_else(|| {
                    anyhow::anyhow!(
                        "Invalid status '{}'. Valid: draft, validated, rejected, \
                     dry_run_passed, dry_run_failed, published, superseded",
                        s
                    )
                })?;
            Some(parsed)
        }
        None => None,
    };

    use sem_os_core::authoring::ports::AuthoringStore;
    let changesets = store
        .list_change_sets(status_filter, limit)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    if changesets.is_empty() {
        println!("No ChangeSets found.");
        return Ok(());
    }

    println!("Authoring Pipeline ChangeSets");
    println!("=============================");
    println!(
        "  {:<38} {:<16} {:<30} {}",
        "ID", "STATUS", "TITLE", "CREATED"
    );
    println!("  {}", "-".repeat(100));
    for cs in &changesets {
        println!(
            "  {:<38} {:<16} {:<30} {}",
            cs.change_set_id,
            cs.status.as_str(),
            truncate_str(&cs.title, 28),
            cs.created_at.format("%Y-%m-%d %H:%M"),
        );
    }
    println!("\n  Total: {}", changesets.len());
    Ok(())
}

/// Get details for a single ChangeSet.
pub async fn authoring_get(id: &str) -> Result<()> {
    let pool = connect().await?;
    let store = sem_os_postgres::PgAuthoringStore::new(pool);
    let cs_id = uuid::Uuid::parse_str(id).context("Invalid UUID")?;

    use sem_os_core::authoring::ports::AuthoringStore;
    let cs = store
        .get_change_set(cs_id)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    println!("ChangeSet: {}", cs.change_set_id);
    println!("  Status:        {}", cs.status.as_str());
    println!("  Title:         {}", cs.title);
    if let Some(ref r) = cs.rationale {
        println!("  Rationale:     {}", r);
    }
    println!("  Content Hash:  {}", cs.content_hash);
    println!("  Hash Version:  {}", cs.hash_version);
    println!("  Created By:    {}", cs.created_by);
    println!("  Created At:    {}", cs.created_at);
    if !cs.depends_on.is_empty() {
        println!("  Depends On:    {:?}", cs.depends_on);
    }
    if let Some(sup) = cs.supersedes_change_set_id {
        println!("  Supersedes:    {}", sup);
    }
    if let Some(by) = cs.superseded_by {
        println!("  Superseded By: {}", by);
    }
    if let Some(eval) = cs.evaluated_against_snapshot_set_id {
        println!("  Evaluated Against: {}", eval);
    }

    // Show artifacts
    let artifacts = store
        .get_artifacts(cs_id)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    if !artifacts.is_empty() {
        println!("\n  Artifacts ({}):", artifacts.len());
        for a in &artifacts {
            println!(
                "    [{}] {} ({})",
                a.ordinal,
                a.path.as_deref().unwrap_or("(inline)"),
                a.artifact_type,
            );
        }
    }

    // Show validation reports
    let reports = store
        .get_validation_reports(cs_id)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    if !reports.is_empty() {
        println!("\n  Validation Reports ({}):", reports.len());
        for (report_id, stage, ok, _report_json) in &reports {
            let status = if *ok { "PASS" } else { "FAIL" };
            println!("    {} {:?} → {}", report_id, stage, status);
        }
    }

    Ok(())
}

/// Validate a ChangeSet (Stage 1 — Draft → Validated/Rejected).
pub async fn authoring_validate(id: &str) -> Result<()> {
    let pool = connect().await?;
    let store = sem_os_postgres::PgAuthoringStore::new(pool.clone());
    let scratch = sem_os_postgres::PgScratchSchemaRunner::new(pool);
    let cs_id = uuid::Uuid::parse_str(id).context("Invalid UUID")?;

    let service =
        sem_os_core::authoring::governance_verbs::GovernanceVerbService::new(&store, &scratch);
    let report = service
        .validate(cs_id)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    println!("Validation Report for {}", cs_id);
    println!("  Result: {}", if report.ok { "PASS" } else { "FAIL" });
    if !report.errors.is_empty() {
        println!("  Errors ({}):", report.errors.len());
        for e in &report.errors {
            println!("    [{:?}] {}: {}", e.severity, e.code, e.message);
        }
    }
    if !report.warnings.is_empty() {
        println!("  Warnings ({}):", report.warnings.len());
        for w in &report.warnings {
            println!("    [{:?}] {}: {}", w.severity, w.code, w.message);
        }
    }
    Ok(())
}

/// Dry-run a ChangeSet (Stage 2 — Validated → DryRunPassed/DryRunFailed).
pub async fn authoring_dry_run(id: &str) -> Result<()> {
    let pool = connect().await?;
    let store = sem_os_postgres::PgAuthoringStore::new(pool.clone());
    let scratch = sem_os_postgres::PgScratchSchemaRunner::new(pool);
    let cs_id = uuid::Uuid::parse_str(id).context("Invalid UUID")?;

    let service =
        sem_os_core::authoring::governance_verbs::GovernanceVerbService::new(&store, &scratch);
    let report = service
        .dry_run(cs_id)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    println!("Dry-Run Report for {}", cs_id);
    println!("  Result: {}", if report.ok { "PASS" } else { "FAIL" });
    if let Some(ms) = report.scratch_schema_apply_ms {
        println!("  Scratch apply: {}ms", ms);
    }
    if !report.errors.is_empty() {
        println!("  Errors ({}):", report.errors.len());
        for e in &report.errors {
            println!("    [{:?}] {}: {}", e.severity, e.code, e.message);
        }
    }
    if !report.warnings.is_empty() {
        println!("  Warnings ({}):", report.warnings.len());
        for w in &report.warnings {
            println!("    [{:?}] {}: {}", w.severity, w.code, w.message);
        }
    }
    if let Some(ref diff) = report.diff_summary {
        print_diff_summary(diff);
    }
    Ok(())
}

/// Generate a publish plan (diff) for a ChangeSet. Read-only.
pub async fn authoring_plan(id: &str) -> Result<()> {
    let pool = connect().await?;
    let store = sem_os_postgres::PgAuthoringStore::new(pool.clone());
    let scratch = sem_os_postgres::PgScratchSchemaRunner::new(pool);
    let cs_id = uuid::Uuid::parse_str(id).context("Invalid UUID")?;

    let service =
        sem_os_core::authoring::governance_verbs::GovernanceVerbService::new(&store, &scratch);
    let diff = service
        .plan_publish(cs_id)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    println!("Publish Plan for {}", cs_id);
    println!("  Status:            {}", diff.status);
    println!("  Breaking changes:  {}", diff.breaking_change_count);
    println!("  Migration count:   {}", diff.migration_count);
    print_diff_summary(&diff.diff);
    Ok(())
}

/// Publish a ChangeSet (DryRunPassed → Published).
pub async fn authoring_publish(id: &str, publisher: &str) -> Result<()> {
    let pool = connect().await?;
    let store = sem_os_postgres::PgAuthoringStore::new(pool.clone());
    let scratch = sem_os_postgres::PgScratchSchemaRunner::new(pool);
    let cs_id = uuid::Uuid::parse_str(id).context("Invalid UUID")?;

    let service =
        sem_os_core::authoring::governance_verbs::GovernanceVerbService::new(&store, &scratch);
    let batch = service
        .publish(cs_id, publisher)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    println!("Published ChangeSet {}", cs_id);
    println!("  Batch ID:       {}", batch.batch_id);
    println!("  Snapshot Set:   {}", batch.snapshot_set_id);
    println!("  Published At:   {}", batch.published_at);
    println!("  Publisher:      {}", batch.publisher);
    Ok(())
}

/// Publish multiple ChangeSets atomically in topological order.
pub async fn authoring_publish_batch(ids: &[String], publisher: &str) -> Result<()> {
    let pool = connect().await?;
    let store = sem_os_postgres::PgAuthoringStore::new(pool.clone());
    let scratch = sem_os_postgres::PgScratchSchemaRunner::new(pool);

    let cs_ids: Vec<uuid::Uuid> = ids
        .iter()
        .map(|s| uuid::Uuid::parse_str(s).context("Invalid UUID"))
        .collect::<Result<Vec<_>>>()?;

    let service =
        sem_os_core::authoring::governance_verbs::GovernanceVerbService::new(&store, &scratch);
    let batch = service
        .publish_batch(&cs_ids, publisher)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    println!(
        "Published {} ChangeSets in batch {}",
        cs_ids.len(),
        batch.batch_id
    );
    println!("  Snapshot Set:   {}", batch.snapshot_set_id);
    println!("  Published At:   {}", batch.published_at);
    println!("  Publisher:      {}", batch.publisher);
    for cs_id in &batch.change_set_ids {
        println!("  - {}", cs_id);
    }
    Ok(())
}

/// Compute structural diff between two ChangeSets.
pub async fn authoring_diff(base_id: &str, target_id: &str) -> Result<()> {
    let pool = connect().await?;
    let store = sem_os_postgres::PgAuthoringStore::new(pool.clone());
    let scratch = sem_os_postgres::PgScratchSchemaRunner::new(pool);

    let base = uuid::Uuid::parse_str(base_id).context("Invalid base UUID")?;
    let target = uuid::Uuid::parse_str(target_id).context("Invalid target UUID")?;

    let service =
        sem_os_core::authoring::governance_verbs::GovernanceVerbService::new(&store, &scratch);
    let diff = service
        .diff(base, target)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    println!("Diff: {} → {}", base_id, target_id);
    print_diff_summary(&diff);
    Ok(())
}

/// Show authoring pipeline health (pending changesets, stale dry-runs).
pub async fn authoring_health() -> Result<()> {
    let pool = connect().await?;
    let store = sem_os_postgres::PgAuthoringStore::new(pool);

    use sem_os_core::authoring::ports::AuthoringStore;

    // Pending changeset counts by status
    let status_counts = store
        .count_by_status()
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut total_pending: i64 = 0;

    println!("Authoring Pipeline Health");
    println!("=========================");
    println!("\n  Pending ChangeSets by Status:");
    if status_counts.is_empty() {
        println!("    (none)");
    } else {
        for (status, count) in &status_counts {
            println!("    {:<20} {:>6}", status.as_str(), count);
            if !status.is_terminal() {
                total_pending += count;
            }
        }
        println!("    {:<20} {:>6}", "TOTAL (non-terminal)", total_pending);
    }

    // Stale dry-runs
    let stale = store
        .find_stale_dry_runs()
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("\n  Stale Dry-Runs: {}", stale.len());
    for cs in &stale {
        println!(
            "    {} (status: {}, title: {})",
            cs.change_set_id,
            cs.status.as_str(),
            truncate_str(&cs.title, 40),
        );
    }

    Ok(())
}

/// Propose a ChangeSet from a bundle directory or inline YAML.
pub async fn authoring_propose(bundle_path: &str) -> Result<()> {
    use sem_os_core::authoring::bundle::{build_bundle, parse_manifest};
    use sem_os_core::principal::Principal;

    let pool = connect().await?;
    let store = sem_os_postgres::PgAuthoringStore::new(pool.clone());
    let scratch = sem_os_postgres::PgScratchSchemaRunner::new(pool);

    // Read manifest from bundle directory
    let bundle_dir = std::path::Path::new(bundle_path);
    let manifest_path = bundle_dir.join("changeset.yaml");
    let manifest_yaml = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("Failed to read {}", manifest_path.display()))?;
    let raw = parse_manifest(&manifest_yaml).map_err(|e| anyhow::anyhow!("{e}"))?;

    let bundle = build_bundle(&raw, |_type_str, path| {
        let full_path = bundle_dir.join(path);
        std::fs::read_to_string(&full_path)
            .map_err(|e| format!("Failed to read {}: {e}", full_path.display()))
    })
    .map_err(|e| anyhow::anyhow!("{e}"))?;

    let principal = Principal {
        actor_id: "cli".to_string(),
        roles: vec!["operator".to_string()],
        claims: std::collections::HashMap::new(),
        tenancy: None,
    };

    let service =
        sem_os_core::authoring::governance_verbs::GovernanceVerbService::new(&store, &scratch);
    let cs = service
        .propose(&bundle, &principal)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    println!("Proposed ChangeSet: {}", cs.change_set_id);
    println!("  Status:       {}", cs.status.as_str());
    println!("  Title:        {}", cs.title);
    println!("  Content Hash: {}", cs.content_hash);
    println!("  Artifacts:    {}", bundle.artifacts.len());
    Ok(())
}

/// Run cleanup to archive old terminal/orphan ChangeSets.
pub async fn authoring_cleanup(terminal_days: Option<u32>, orphan_days: Option<u32>) -> Result<()> {
    let pool = connect().await?;

    let policy = sem_os_core::authoring::cleanup::CleanupPolicy {
        terminal_retention_days: terminal_days.unwrap_or(90),
        orphan_retention_days: orphan_days.unwrap_or(30),
    };

    println!("Cleanup Policy:");
    println!(
        "  Terminal retention: {} days",
        policy.terminal_retention_days
    );
    println!(
        "  Orphan retention:   {} days",
        policy.orphan_retention_days
    );

    let cleanup_store = sem_os_postgres::PgCleanupStore::new(pool);
    let report = sem_os_core::authoring::cleanup::run_cleanup(&cleanup_store, &policy)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    println!("\nCleanup Results:");
    println!("  Terminal archived: {}", report.terminal_archived);
    println!("  Orphan archived:   {}", report.orphan_archived);
    Ok(())
}

fn print_diff_summary(diff: &sem_os_core::authoring::types::DiffSummary) {
    if diff.added.is_empty()
        && diff.modified.is_empty()
        && diff.removed.is_empty()
        && diff.breaking_changes.is_empty()
    {
        println!("  (no changes)");
        return;
    }
    if !diff.added.is_empty() {
        println!("  Added ({}):", diff.added.len());
        for e in &diff.added {
            println!("    + {} [{}]", e.fqn, e.object_type);
        }
    }
    if !diff.modified.is_empty() {
        println!("  Modified ({}):", diff.modified.len());
        for e in &diff.modified {
            println!("    ~ {} [{}]", e.fqn, e.object_type);
        }
    }
    if !diff.removed.is_empty() {
        println!("  Removed ({}):", diff.removed.len());
        for e in &diff.removed {
            println!("    - {} [{}]", e.fqn, e.object_type);
        }
    }
    if !diff.breaking_changes.is_empty() {
        println!("  Breaking Changes ({}):", diff.breaking_changes.len());
        for e in &diff.breaking_changes {
            println!(
                "    ! {} [{}] — {}",
                e.fqn,
                e.object_type,
                e.detail.as_deref().unwrap_or("")
            );
        }
    }
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}

// ── Helpers ───────────────────────────────────────────────────

async fn connect() -> Result<PgPool> {
    let url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".into());
    PgPool::connect(&url)
        .await
        .context("Failed to connect to database")
}

fn parse_object_type(s: &str) -> Result<ObjectType> {
    match s {
        "attribute_def" | "attr" => Ok(ObjectType::AttributeDef),
        "entity_type_def" | "entity-type" => Ok(ObjectType::EntityTypeDef),
        "verb_contract" | "verb" => Ok(ObjectType::VerbContract),
        "taxonomy_def" | "taxonomy" => Ok(ObjectType::TaxonomyDef),
        "taxonomy_node" => Ok(ObjectType::TaxonomyNode),
        "membership_rule" | "membership" => Ok(ObjectType::MembershipRule),
        "view_def" | "view" => Ok(ObjectType::ViewDef),
        "policy_rule" | "policy" => Ok(ObjectType::PolicyRule),
        "evidence_requirement" | "evidence" => Ok(ObjectType::EvidenceRequirement),
        "document_type_def" | "doc-type" => Ok(ObjectType::DocumentTypeDef),
        "observation_def" | "observation" => Ok(ObjectType::ObservationDef),
        "derivation_spec" | "derivation" => Ok(ObjectType::DerivationSpec),
        _ => anyhow::bail!(
            "Unknown object type: '{}'. Valid types: attr, entity-type, verb, taxonomy, \
             membership, view, policy, evidence, doc-type, observation, derivation",
            s
        ),
    }
}
