//! xtask commands for the Semantic Registry.
//!
//! Usage: `cargo x sem-reg <subcommand>`

use anyhow::{Context, Result};
use sqlx::PgPool;

use ob_poc::sem_reg::types::{ChangeType, SnapshotMeta};
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
    let rows = sqlx::query_as::<_, ob_poc::sem_reg::SnapshotRow>(
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

    let rows = sqlx::query_as::<_, ob_poc::sem_reg::SnapshotRow>(
        "SELECT * FROM sem_reg.snapshots \
         WHERE status = 'active' AND effective_until IS NULL \
         ORDER BY object_type, definition->>'fqn'",
    )
    .fetch_all(&pool)
    .await?;

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
