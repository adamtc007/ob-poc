use anyhow::{anyhow, Context, Result};
use clap::{Subcommand, ValueEnum};
use ob_poc::agent::learning::embedder::CandleEmbedder;
use ob_poc::api::agent_service::AgentService;
use ob_poc::calibration::{
    build_generation_prompt, build_scenario_seed, classify_outcome, compute_drift, compute_metrics,
    execute_calibration_utterance, generate_proposed_gaps, generate_suggested_clarifications,
    load_trace, parse_generated_utterances, pre_screen_utterances, CalibrationExecutionShape,
    CalibrationFixtureTransition, CalibrationFixtures, CalibrationRun, CalibrationStore,
    CalibrationWriteThroughSummary,
};
use ob_poc::sem_reg::abac::ActorContext;
use ob_poc::sem_reg::agent::mcp_tools::build_sem_os_service;
use ob_poc::sem_reg::types::Classification;
use ob_poc::session::UnifiedSession;
use sem_os_client::inprocess::InProcessClient;
use sqlx::PgPool;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
    Yaml,
}

#[derive(Subcommand)]
pub enum CalibrationAction {
    /// Build and persist one scenario seed from live runtime metadata.
    Seed {
        #[arg(long)]
        scenario_name: String,
        #[arg(long)]
        template_id: String,
        #[arg(long)]
        target_entity_type: String,
        #[arg(long)]
        target_entity_state: String,
        #[arg(long)]
        target_verb: String,
        #[arg(long, default_value_t = 0.08)]
        margin_threshold: f32,
    },
    /// Print the generation prompt for an existing scenario.
    Prompt {
        #[arg(long)]
        scenario_id: Uuid,
    },
    /// Export one scenario and its utterances to a bundle file.
    ExportScenario {
        #[arg(long)]
        scenario_id: Uuid,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    /// Import one scenario bundle file.
    ImportScenario {
        #[arg(long)]
        input: PathBuf,
    },
    /// Ingest generated utterances from a JSON file, pre-screen them, and persist them.
    Generate {
        #[arg(long)]
        scenario_id: Uuid,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        admit: bool,
    },
    /// List utterances awaiting or undergoing review.
    ListPending {
        #[arg(long)]
        scenario_id: Option<Uuid>,
        #[arg(long, default_value = "Screened")]
        lifecycle_status: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
    /// Admit or reject one generated utterance.
    Review {
        #[arg(long)]
        utterance_id: Uuid,
        #[arg(long)]
        reviewer: String,
        #[arg(long)]
        admit: bool,
        #[arg(long)]
        reject: bool,
    },
    /// Execute a stored scenario's utterances through the live chat pipeline.
    Run {
        #[arg(long)]
        scenario_id: Uuid,
        #[arg(long, default_value = "xtask.calibration")]
        triggered_by: String,
        #[arg(long, default_value = "Admitted")]
        lifecycle_status: String,
        #[arg(long)]
        output_dir: Option<PathBuf>,
    },
    /// Print a compact summary for one completed run.
    Report {
        #[arg(long)]
        run_id: Uuid,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
        #[arg(long)]
        output_dir: Option<PathBuf>,
    },
    /// Print drift between the latest two runs for a scenario.
    Drift {
        #[arg(long)]
        scenario_id: Uuid,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
        #[arg(long)]
        output_dir: Option<PathBuf>,
    },
    /// Print a portfolio summary across all calibration scenarios.
    Portfolio {
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
        #[arg(long)]
        output_dir: Option<PathBuf>,
    },
    /// Print draft Loop 1 gap proposals from one completed run.
    Gaps {
        #[arg(long)]
        run_id: Uuid,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
        #[arg(long)]
        output_dir: Option<PathBuf>,
    },
    /// Print draft Loop 2 clarification suggestions from one completed run.
    Clarifications {
        #[arg(long)]
        run_id: Uuid,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
        #[arg(long)]
        output_dir: Option<PathBuf>,
    },
}

pub async fn run(action: CalibrationAction) -> Result<()> {
    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL must be set for calibration xtask")?;
    let pool = PgPool::connect(&database_url)
        .await
        .context("connect calibration PgPool")?;
    let store = CalibrationStore::new(pool.clone());

    match action {
        CalibrationAction::Seed {
            scenario_name,
            template_id,
            target_entity_type,
            target_entity_state,
            target_verb,
            margin_threshold,
        } => {
            let embedder = CandleEmbedder::new().context("initialize CandleEmbedder")?;
            let scenario = build_scenario_seed(
                &pool,
                &embedder,
                &scenario_name,
                &template_id,
                &target_entity_type,
                &target_entity_state,
                &target_verb,
                Vec::new(),
                CalibrationExecutionShape::Singleton,
                margin_threshold,
            )
            .await?;
            store.upsert_scenario(&scenario).await?;
            println!(
                "seeded scenario {} ({})",
                scenario.scenario_id, scenario.scenario_name
            );
        }
        CalibrationAction::Prompt { scenario_id } => {
            let scenario = store
                .get_scenario(scenario_id)
                .await?
                .ok_or_else(|| anyhow!("scenario not found: {}", scenario_id))?;
            println!("{}", build_generation_prompt(&scenario));
        }
        CalibrationAction::ExportScenario {
            scenario_id,
            output,
            format,
        } => {
            let bundle = store
                .export_scenario_bundle(scenario_id)
                .await?
                .ok_or_else(|| anyhow!("scenario not found: {}", scenario_id))?;
            write_bundle(&bundle, &output, format)?;
            println!(
                "exported scenario bundle {} to {}",
                scenario_id,
                output.display()
            );
        }
        CalibrationAction::ImportScenario { input } => {
            let bundle = read_bundle(&input)?;
            store.import_scenario_bundle(&bundle).await?;
            println!(
                "imported scenario bundle {} from {}",
                bundle.scenario.scenario_id,
                input.display()
            );
        }
        CalibrationAction::Generate {
            scenario_id,
            input,
            admit,
        } => {
            let scenario = store
                .get_scenario(scenario_id)
                .await?
                .ok_or_else(|| anyhow!("scenario not found: {}", scenario_id))?;
            let raw = std::fs::read_to_string(&input)
                .with_context(|| format!("read generated utterance file {}", input.display()))?;
            let utterances = parse_generated_utterances(&raw)?;
            if utterances.is_empty() {
                return Err(anyhow!("generated utterance file contained no utterances"));
            }

            let embedder = CandleEmbedder::new().context("initialize CandleEmbedder")?;
            let pre_screen = pre_screen_utterances(&utterances, &scenario, &embedder).await?;
            let utterance_ids = store
                .insert_generated_utterances(scenario_id, &utterances)
                .await
                .context("persist generated utterances")?;
            let lifecycle_status = if admit { "Admitted" } else { "Screened" };
            for (utterance_id, pre_screen) in utterance_ids.iter().zip(pre_screen.iter()) {
                store
                    .update_pre_screen(*utterance_id, pre_screen, lifecycle_status)
                    .await
                    .with_context(|| format!("update pre-screen for utterance {}", utterance_id))?;
            }
            println!(
                "persisted {} generated utterances for scenario {} with lifecycle_status={}",
                utterance_ids.len(),
                scenario_id,
                lifecycle_status
            );
        }
        CalibrationAction::ListPending {
            scenario_id,
            lifecycle_status,
            format,
        } => {
            let utterances = store
                .list_review_utterances(scenario_id, Some(&lifecycle_status))
                .await?;
            print_review_rows(&utterances, format)?;
        }
        CalibrationAction::Review {
            utterance_id,
            reviewer,
            admit,
            reject,
        } => {
            if admit == reject {
                return Err(anyhow!(
                    "choose exactly one of --admit or --reject for review"
                ));
            }
            store
                .review_utterance(utterance_id, admit, &reviewer)
                .await?;
            println!(
                "reviewed utterance {} as {} by {}",
                utterance_id,
                if admit { "Admitted" } else { "Deprecated" },
                reviewer
            );
        }
        CalibrationAction::Run {
            scenario_id,
            triggered_by,
            lifecycle_status,
            output_dir,
        } => {
            let scenario = store
                .get_scenario(scenario_id)
                .await?
                .ok_or_else(|| anyhow!("scenario not found: {}", scenario_id))?;
            let mut utterances = store
                .list_utterances(scenario_id, Some(&lifecycle_status))
                .await?;
            if utterances.is_empty() {
                utterances = store.list_utterances(scenario_id, None).await?;
            }
            if utterances.is_empty() {
                return Err(anyhow!(
                    "no calibration utterances found for scenario {}",
                    scenario_id
                ));
            }

            let embedder = Arc::new(CandleEmbedder::new().context("initialize CandleEmbedder")?);
            let sem_os_service = build_sem_os_service(&pool);
            let sem_os_client = Arc::new(InProcessClient::new(sem_os_service));
            let service = AgentService::new(pool.clone(), embedder, None, None)
                .with_sem_os_client(sem_os_client);
            let actor = ActorContext {
                actor_id: "xtask.calibration".to_string(),
                roles: vec!["admin".to_string(), "ops".to_string()],
                department: Some("calibration".to_string()),
                clearance: Some(Classification::Restricted),
                jurisdictions: vec!["*".to_string()],
            };
            let mut session = UnifiedSession::new_for_entity(
                None,
                &scenario.target_entity_type,
                None,
                Some(scenario.target_entity_type.clone()),
            );
            session.target_entity_type = Some(scenario.target_entity_type.clone());
            session.context.domain_hint = Some(scenario.target_entity_type.clone());
            session.context.stage_focus =
                infer_stage_focus_for_scenario(&scenario).map(str::to_string);
            let mut fixtures = CalibrationFixtures::new(session);
            fixtures.prime_for_scenario(&scenario);
            fixtures
                .hydrate_persisted_subjects(&pool, &scenario)
                .await?;
            let run_id = Uuid::new_v4();
            let run_start = chrono::Utc::now();
            let mut outcomes = Vec::with_capacity(utterances.len());
            let mut trace_ids = Vec::with_capacity(utterances.len());
            let mut fixture_transitions = Vec::with_capacity(utterances.len());
            let mut surface_versions = None;

            for utterance in &utterances {
                let trace_id = execute_calibration_utterance(
                    &service,
                    &mut fixtures,
                    &actor,
                    &utterance.text,
                    &pool,
                )
                .await
                .with_context(|| format!("execute calibration utterance '{}'", utterance.text))?;
                let trace = load_trace(&pool, trace_id).await?;
                if surface_versions.is_none() {
                    surface_versions = Some(trace.surface_versions.clone());
                }
                fixtures.apply_trace_transition(&trace);
                let outcome = classify_outcome(&trace, utterance, &scenario);
                fixture_transitions.push(CalibrationFixtureTransition {
                    utterance_id: utterance.utterance_id,
                    trace_id,
                    fixture_state: fixtures.snapshot_state(),
                });
                trace_ids.push(trace_id);
                outcomes.push(outcome);
            }

            let metrics = compute_metrics(&outcomes);
            let prior_run = store.latest_run_for_scenario(scenario_id).await?;
            let run = CalibrationRun {
                run_id,
                scenario_id,
                triggered_by,
                surface_versions: surface_versions.unwrap_or_default(),
                utterance_count: outcomes.len(),
                positive_count: outcomes
                    .iter()
                    .filter(|outcome| {
                        outcome.calibration_mode == ob_poc::calibration::CalibrationMode::Positive
                    })
                    .count(),
                negative_count: outcomes
                    .iter()
                    .filter(|outcome| {
                        outcome.calibration_mode == ob_poc::calibration::CalibrationMode::Negative
                    })
                    .count(),
                boundary_count: outcomes
                    .iter()
                    .filter(|outcome| {
                        outcome.calibration_mode == ob_poc::calibration::CalibrationMode::Boundary
                    })
                    .count(),
                metrics,
                outcomes: outcomes.clone(),
                prior_run_id: prior_run.as_ref().map(|run| run.run_id),
                drift: None,
                trace_ids,
                run_start,
                run_end: Some(chrono::Utc::now()),
            };
            let run = CalibrationRun {
                drift: prior_run.as_ref().map(|prior| compute_drift(prior, &run)),
                ..run
            };
            store.insert_run(&run).await?;
            store.insert_outcomes(run_id, &outcomes).await?;
            store
                .insert_fixture_transitions(run_id, &fixture_transitions)
                .await?;
            let gaps = generate_proposed_gaps(&scenario, &run.outcomes);
            let clarifications = generate_suggested_clarifications(&scenario, &run.outcomes);
            let write_through = store.write_through_learning(&gaps, &clarifications).await?;
            let artifact_dir = run_artifact_dir(output_dir.as_deref(), run_id);
            write_run_artifacts(
                &artifact_dir,
                &scenario,
                &run,
                &gaps,
                &clarifications,
                &fixture_transitions,
                &write_through,
            )?;
            println!(
                "completed run {} for scenario {} with {} utterances",
                run_id, scenario_id, run.utterance_count
            );
            println!("loop1_upserts: {}", write_through.loop1_candidates_upserted);
            println!(
                "loop2_phrase_upserts: {}",
                write_through.loop2_phrases_upserted
            );
            println!("artifacts: {}", artifact_dir.display());
            if let Some(drift) = &run.drift {
                println!(
                    "drift vs prior {}: accuracy {:+.2}%, fallback {:+.2}%",
                    drift.prior_run_id,
                    drift.overall_accuracy_delta * 100.0,
                    drift.fallback_rate_delta * 100.0
                );
            }
        }
        CalibrationAction::Report {
            run_id,
            format,
            output_dir,
        } => {
            let run = store
                .get_run(run_id)
                .await?
                .ok_or_else(|| anyhow!("run not found: {}", run_id))?;
            let scenario = store
                .get_scenario(run.scenario_id)
                .await?
                .ok_or_else(|| anyhow!("scenario not found: {}", run.scenario_id))?;
            let gaps = generate_proposed_gaps(&scenario, &run.outcomes);
            let clarifications = generate_suggested_clarifications(&scenario, &run.outcomes);
            let transitions = store.list_fixture_transitions(run_id).await?;
            print_run_report(&scenario, &run, &gaps, &clarifications, format)?;
            if let Some(output_dir) = output_dir {
                write_run_artifacts(
                    &output_dir,
                    &scenario,
                    &run,
                    &gaps,
                    &clarifications,
                    &transitions,
                    &CalibrationWriteThroughSummary::default(),
                )?;
            }
        }
        CalibrationAction::Drift {
            scenario_id,
            format,
            output_dir,
        } => {
            let current = store
                .latest_run_for_scenario(scenario_id)
                .await?
                .ok_or_else(|| anyhow!("no calibration runs found for scenario {}", scenario_id))?;
            let Some(prior_run_id) = current.prior_run_id else {
                return Err(anyhow!(
                    "scenario {} has only one run; drift requires a prior run",
                    scenario_id
                ));
            };
            let prior = store
                .get_run(prior_run_id)
                .await?
                .ok_or_else(|| anyhow!("prior run not found: {}", prior_run_id))?;
            let drift = current
                .drift
                .clone()
                .unwrap_or_else(|| compute_drift(&prior, &current));
            print_drift_report(scenario_id, &prior, &current, &drift, format)?;
            if let Some(output_dir) = output_dir {
                write_named_payload(
                    &output_dir,
                    "drift",
                    &serde_json::json!({
                        "scenario_id": scenario_id,
                        "prior_run": prior,
                        "current_run": current,
                        "drift": drift,
                    }),
                    &format!(
                        "scenario_id: {}\nprior_run_id: {}\ncurrent_run_id: {}\n",
                        scenario_id, prior.run_id, current.run_id
                    ),
                )?;
            }
        }
        CalibrationAction::Portfolio { format, output_dir } => {
            let rows = store.build_portfolio_summary().await?;
            print_portfolio(&rows, format)?;
            if let Some(output_dir) = output_dir {
                write_named_payload(
                    &output_dir,
                    "portfolio",
                    &serde_json::to_value(&rows)?,
                    &rows
                        .iter()
                        .map(|row| format!("{} {}", row.scenario_id, row.scenario_name))
                        .collect::<Vec<_>>()
                        .join("\n"),
                )?;
            }
        }
        CalibrationAction::Gaps {
            run_id,
            format,
            output_dir,
        } => {
            let run = store
                .get_run(run_id)
                .await?
                .ok_or_else(|| anyhow!("run not found: {}", run_id))?;
            let scenario = store
                .get_scenario(run.scenario_id)
                .await?
                .ok_or_else(|| anyhow!("scenario not found: {}", run.scenario_id))?;
            let gaps = generate_proposed_gaps(&scenario, &run.outcomes);
            print_gaps(&gaps, format)?;
            if let Some(output_dir) = output_dir {
                write_named_payload(
                    &output_dir,
                    "gaps",
                    &serde_json::to_value(&gaps)?,
                    &gaps
                        .iter()
                        .map(|row| format!("{} {}", row.code, row.utterance))
                        .collect::<Vec<_>>()
                        .join("\n"),
                )?;
            }
        }
        CalibrationAction::Clarifications {
            run_id,
            format,
            output_dir,
        } => {
            let run = store
                .get_run(run_id)
                .await?
                .ok_or_else(|| anyhow!("run not found: {}", run_id))?;
            let scenario = store
                .get_scenario(run.scenario_id)
                .await?
                .ok_or_else(|| anyhow!("scenario not found: {}", run.scenario_id))?;
            let suggestions = generate_suggested_clarifications(&scenario, &run.outcomes);
            print_clarifications(&suggestions, format)?;
            if let Some(output_dir) = output_dir {
                write_named_payload(
                    &output_dir,
                    "clarifications",
                    &serde_json::to_value(&suggestions)?,
                    &suggestions
                        .iter()
                        .map(|row| format!("{} -> {}", row.trigger_phrase, row.suggested_prompt))
                        .collect::<Vec<_>>()
                        .join("\n"),
                )?;
            }
        }
    }

    Ok(())
}

fn run_artifact_dir(base: Option<&std::path::Path>, run_id: Uuid) -> PathBuf {
    match base {
        Some(path) => path.join(format!("run-{}", run_id)),
        None => PathBuf::from("artifacts")
            .join("calibration")
            .join(format!("run-{}", run_id)),
    }
}

fn infer_stage_focus_for_scenario(
    scenario: &ob_poc::calibration::CalibrationScenario,
) -> Option<&'static str> {
    if scenario.target_entity_type == "cbu"
        && matches!(
            scenario.target_verb.as_str(),
            "cbu.read" | "cbu.list" | "cbu.parties"
        )
    {
        return Some("semos-calibration");
    }
    if scenario.operational_phase == "KYCBlocked" {
        Some("semos-kyc")
    } else if scenario.target_entity_type == "cbu"
        && matches!(
            scenario.target_entity_state.as_str(),
            "DISCOVERED" | "VALIDATION_FAILED" | "VALIDATION_PENDING" | "VALIDATED"
        )
    {
        Some("semos-onboarding")
    } else if matches!(
        scenario.target_entity_type.as_str(),
        "document" | "requirement" | "ubo" | "screening"
    ) {
        Some("semos-kyc")
    } else if matches!(
        scenario.target_entity_type.as_str(),
        "deal" | "contract" | "billing" | "product" | "registry"
    ) {
        Some("semos-data-management")
    } else {
        None
    }
}

fn print_run_report(
    scenario: &ob_poc::calibration::CalibrationScenario,
    run: &CalibrationRun,
    gaps: &[ob_poc::calibration::ProposedGapEntry],
    clarifications: &[ob_poc::calibration::SuggestedClarification],
    format: OutputFormat,
) -> Result<()> {
    match format {
        OutputFormat::Text => {
            println!(
                "scenario: {} ({})",
                scenario.scenario_name, scenario.scenario_id
            );
            println!("target_verb: {}", scenario.target_verb);
            println!(
                "entity: {} state={}",
                scenario.target_entity_type, scenario.target_entity_state
            );
            println!("run_id: {}", run.run_id);
            println!("triggered_by: {}", run.triggered_by);
            println!("utterances: {}", run.utterance_count);
            println!("accuracy: {:.2}%", run.metrics.overall_accuracy * 100.0);
            println!(
                "fallback_rate: {:.2}%",
                run.metrics.phase4_fallback_rate * 100.0
            );
            if let Some(margin) = run.metrics.phase4_avg_margin {
                println!("avg_margin: {:.4}", margin);
            }
            println!("fragile_boundaries: {}", run.metrics.fragile_boundary_count);
            println!("run_start: {}", run.run_start);
            if let Some(run_end) = run.run_end {
                println!("run_end: {}", run_end);
            }
            println!("loop1_gaps: {}", gaps.len());
            println!("loop2_clarifications: {}", clarifications.len());
            println!("outcomes:");
            for outcome in &run.outcomes {
                println!(
                    "  - {} => {:?} verb={:?} halt={:?}",
                    outcome.utterance_id,
                    outcome.verdict,
                    outcome.actual_resolved_verb,
                    outcome.actual_halt_reason
                );
            }
            Ok(())
        }
        OutputFormat::Json => print_json(&serde_json::json!({
            "scenario": scenario,
            "run": run,
            "gaps": gaps,
            "clarifications": clarifications,
        })),
        OutputFormat::Yaml => print_yaml(&serde_json::json!({
            "scenario": scenario,
            "run": run,
            "gaps": gaps,
            "clarifications": clarifications,
        })),
    }
}

fn print_drift_report(
    scenario_id: Uuid,
    prior: &CalibrationRun,
    current: &CalibrationRun,
    drift: &ob_poc::calibration::CalibrationDrift,
    format: OutputFormat,
) -> Result<()> {
    let payload = serde_json::json!({
        "scenario_id": scenario_id,
        "prior_run_id": prior.run_id,
        "current_run_id": current.run_id,
        "drift": drift,
    });
    match format {
        OutputFormat::Text => {
            println!("scenario_id: {}", scenario_id);
            println!("prior_run_id: {}", prior.run_id);
            println!("current_run_id: {}", current.run_id);
            println!(
                "accuracy_delta: {:+.2}%",
                drift.overall_accuracy_delta * 100.0
            );
            println!(
                "fallback_rate_delta: {:+.2}%",
                drift.fallback_rate_delta * 100.0
            );
            if let Some(avg_margin_delta) = drift.avg_margin_delta {
                println!("avg_margin_delta: {:+.4}", avg_margin_delta);
            }
            println!(
                "newly_failing: {} newly_passing: {}",
                drift.newly_failing_utterances.len(),
                drift.newly_passing_utterances.len()
            );
            Ok(())
        }
        OutputFormat::Json => print_json(&payload),
        OutputFormat::Yaml => print_yaml(&payload),
    }
}

fn print_review_rows(
    rows: &[ob_poc::calibration::CalibrationUtteranceReviewRow],
    format: OutputFormat,
) -> Result<()> {
    match format {
        OutputFormat::Text => {
            for row in rows {
                println!(
                    "{} {} {} {:?} reviewer={:?}",
                    row.utterance_id,
                    row.lifecycle_status,
                    row.scenario_id,
                    row.calibration_mode,
                    row.reviewed_by
                );
                println!("  {}", row.text);
            }
            Ok(())
        }
        OutputFormat::Json => print_json(rows),
        OutputFormat::Yaml => print_yaml(rows),
    }
}

fn print_portfolio(
    rows: &[ob_poc::calibration::CalibrationPortfolioEntry],
    format: OutputFormat,
) -> Result<()> {
    match format {
        OutputFormat::Text => {
            for row in rows {
                println!(
                    "{} {} verb={} admitted={} last_run={:?} accuracy={}",
                    row.scenario_id,
                    row.scenario_name,
                    row.target_verb,
                    row.admitted_utterance_count,
                    row.last_run_id,
                    row.overall_accuracy
                        .map(|value| format!("{:.2}%", value * 100.0))
                        .unwrap_or_else(|| "n/a".to_string())
                );
            }
            Ok(())
        }
        OutputFormat::Json => print_json(rows),
        OutputFormat::Yaml => print_yaml(rows),
    }
}

fn print_gaps(rows: &[ob_poc::calibration::ProposedGapEntry], format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Text => {
            for row in rows {
                println!(
                    "{} source={} entity={} state={} verb={} halt={:?}",
                    row.code,
                    row.source,
                    row.entity_type,
                    row.entity_state,
                    row.target_verb,
                    row.actual_halt_reason
                );
                println!("  {}", row.utterance);
            }
            Ok(())
        }
        OutputFormat::Json => print_json(rows),
        OutputFormat::Yaml => print_yaml(rows),
    }
}

fn print_clarifications(
    rows: &[ob_poc::calibration::SuggestedClarification],
    format: OutputFormat,
) -> Result<()> {
    match format {
        OutputFormat::Text => {
            for row in rows {
                println!("{} :: {} vs {}", row.trigger_phrase, row.verb_a, row.verb_b);
                println!("  {}", row.suggested_prompt);
            }
            Ok(())
        }
        OutputFormat::Json => print_json(rows),
        OutputFormat::Yaml => print_yaml(rows),
    }
}

fn print_json<T: serde::Serialize + ?Sized>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn print_yaml<T: serde::Serialize + ?Sized>(value: &T) -> Result<()> {
    println!("{}", serde_yaml::to_string(value)?);
    Ok(())
}

fn write_run_artifacts(
    output_dir: &std::path::Path,
    scenario: &ob_poc::calibration::CalibrationScenario,
    run: &CalibrationRun,
    gaps: &[ob_poc::calibration::ProposedGapEntry],
    clarifications: &[ob_poc::calibration::SuggestedClarification],
    fixture_transitions: &[CalibrationFixtureTransition],
    write_through: &CalibrationWriteThroughSummary,
) -> Result<()> {
    let payload = serde_json::json!({
        "scenario": scenario,
        "run": run,
        "gaps": gaps,
        "clarifications": clarifications,
        "fixture_transitions": fixture_transitions,
        "write_through": write_through,
    });
    let summary = format!(
        "scenario: {} ({})\nrun_id: {}\nutterances: {}\naccuracy: {:.2}%\nloop1_candidates_upserted: {}\nloop2_phrases_upserted: {}\nloop2_blocklist_upserts: {}\n",
        scenario.scenario_name,
        scenario.scenario_id,
        run.run_id,
        run.utterance_count,
        run.metrics.overall_accuracy * 100.0,
        write_through.loop1_candidates_upserted,
        write_through.loop2_phrases_upserted,
        write_through.loop2_blocklist_upserts,
    );
    write_named_payload(output_dir, "run_report", &payload, &summary)?;
    write_named_payload(
        output_dir,
        "gaps",
        &serde_json::to_value(gaps)?,
        &gaps
            .iter()
            .map(|row| format!("{} {}", row.code, row.utterance))
            .collect::<Vec<_>>()
            .join("\n"),
    )?;
    write_named_payload(
        output_dir,
        "clarifications",
        &serde_json::to_value(clarifications)?,
        &clarifications
            .iter()
            .map(|row| format!("{} -> {}", row.trigger_phrase, row.suggested_prompt))
            .collect::<Vec<_>>()
            .join("\n"),
    )?;
    write_named_payload(
        output_dir,
        "fixture_transitions",
        &serde_json::to_value(fixture_transitions)?,
        &fixture_transitions
            .iter()
            .map(|row| format!("{} {}", row.utterance_id, row.trace_id))
            .collect::<Vec<_>>()
            .join("\n"),
    )?;
    write_named_payload(
        output_dir,
        "write_through",
        &serde_json::to_value(write_through)?,
        &format!(
            "loop1_candidates_upserted: {}\nloop2_phrases_upserted: {}\nloop2_blocklist_upserts: {}\n",
            write_through.loop1_candidates_upserted,
            write_through.loop2_phrases_upserted,
            write_through.loop2_blocklist_upserts,
        ),
    )?;
    Ok(())
}

fn write_named_payload(
    output_dir: &std::path::Path,
    stem: &str,
    payload: &serde_json::Value,
    text: &str,
) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;
    std::fs::write(
        output_dir.join(format!("{stem}.json")),
        serde_json::to_string_pretty(payload)?,
    )?;
    std::fs::write(
        output_dir.join(format!("{stem}.yaml")),
        serde_yaml::to_string(payload)?,
    )?;
    std::fs::write(output_dir.join(format!("{stem}.txt")), text)?;
    Ok(())
}

fn write_bundle(
    bundle: &ob_poc::calibration::CalibrationScenarioBundle,
    output: &std::path::Path,
    format: OutputFormat,
) -> Result<()> {
    let body = match format {
        OutputFormat::Text => serde_json::to_string_pretty(bundle)?,
        OutputFormat::Json => serde_json::to_string_pretty(bundle)?,
        OutputFormat::Yaml => serde_yaml::to_string(bundle)?,
    };
    std::fs::write(output, body)?;
    Ok(())
}

fn read_bundle(input: &std::path::Path) -> Result<ob_poc::calibration::CalibrationScenarioBundle> {
    let raw = std::fs::read_to_string(input)?;
    match input.extension().and_then(|value| value.to_str()) {
        Some("yaml" | "yml") => Ok(serde_yaml::from_str(&raw)?),
        _ => Ok(serde_json::from_str(&raw)?),
    }
}
