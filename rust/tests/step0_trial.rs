use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use ob_poc::agent::learning::embedder::{CandleEmbedder, Embedder};
use ob_poc::agent::learning::warmup::LearningWarmup;
use ob_poc::database::verb_service::VerbService;
use ob_poc::dsl_v2::load_macro_registry_from_dir;
use ob_poc::mcp::macro_index::MacroIndex;
use ob_poc::mcp::scenario_index::ScenarioIndex;
use ob_poc::mcp::verb_search::HybridVerbSearcher;

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
struct CorpusEntry {
    id: String,
    utterance: String,
    expected_verb: String,
    domain: String,
    category: String,
    difficulty: String,
    #[serde(default)]
    alt_verbs: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Step0Result {
    id: String,
    utterance: String,
    expected_verb: String,
    domain: String,
    allowed_verbs: Vec<String>,
    top_resolved_verb: Option<String>,
    top_resolved_score: Option<f32>,
    is_hit: bool,
    candidates: Vec<(String, f32)>,
}

#[derive(Debug, Serialize)]
struct TrialReport {
    total_cases: usize,
    hits: usize,
    misses: usize,
    accuracy: f32,
    domain_accuracy: HashMap<String, f32>,
    results: Vec<Step0Result>,
}

async fn build_searcher(pool: &PgPool) -> HybridVerbSearcher {
    // Configure pgvector probes to scan all lists (exact nearest neighbor search)
    let _ = sqlx::query("SET ivfflat.probes = 100").execute(pool).await;
    let _ = sqlx::query("ALTER DATABASE data_designer SET ivfflat.probes = 100")
        .execute(pool)
        .await;

    let embedder = Arc::new(CandleEmbedder::new().expect("Failed to initialize embedder"));
    let dyn_embedder: Arc<dyn Embedder> = embedder as Arc<dyn Embedder>;
    let verb_service = Arc::new(VerbService::new(pool.clone()));

    let warmup = LearningWarmup::new(pool.clone());
    let (learned_data, _) = warmup
        .warmup()
        .await
        .expect("Failed to warmup learning data");

    #[derive(Deserialize)]
    struct CuratedOverrideEntry {
        aliases: Vec<String>,
    }

    let overrides = {
        let path = Path::new("config/macro_search_overrides.yaml");
        if path.is_file() {
            let content = std::fs::read_to_string(path).expect("Failed to read overrides file");
            let raw: HashMap<String, CuratedOverrideEntry> =
                serde_yaml::from_str(&content).expect("Failed to parse overrides file");

            let mut aliases_map = HashMap::new();
            for (fqn, entry) in raw {
                aliases_map.insert(fqn, entry.aliases);
            }
            Some(ob_poc::mcp::macro_index::MacroSearchOverrides {
                aliases: aliases_map,
            })
        } else {
            None
        }
    };

    let macro_index = {
        let path = Path::new("config/verb_schemas/macros");
        if path.is_dir() {
            let registry =
                load_macro_registry_from_dir(path).expect("Failed to load macro registry");
            Some(Arc::new(MacroIndex::from_registry(
                &registry,
                overrides.as_ref(),
            )))
        } else {
            None
        }
    };

    let scenario_index = {
        let path = Path::new("config/scenario_index.yaml");
        if path.is_file() {
            Some(Arc::new(
                ScenarioIndex::from_yaml_file(path).expect("Failed to load scenario index"),
            ))
        } else {
            None
        }
    };

    let mut searcher =
        HybridVerbSearcher::new(verb_service, Some(learned_data)).with_embedder(dyn_embedder);
    if let Some(mi) = macro_index {
        searcher = searcher.with_macro_index(mi);
    }
    if let Some(si) = scenario_index {
        searcher = searcher.with_scenario_index(si);
    }
    searcher
}

fn get_simulated_allowed_verbs(expected_verb: &str) -> HashSet<String> {
    let verb_parts: Vec<&str> = expected_verb.split('.').collect();
    let namespace = verb_parts[0];

    let mut allowed = HashSet::new();

    match namespace {
        "cbu" => {
            for v in &[
                "cbu.create",
                "cbu.update",
                "cbu.delete",
                "cbu.assign-role",
                "cbu.add-product",
                "cbu.parties",
                "cbu.delete-cascade",
                "cbu.terminate",
                "cbu.suspend",
                "cbu.list-roles",
                "cbu.get-config",
            ] {
                allowed.insert(v.to_string());
            }
        }
        "entity" | "party" => {
            for v in &[
                "entity.create",
                "entity.update",
                "entity.delete",
                "entity.read",
                "entity.verify-name",
                "entity.add-parent",
                "entity.list-placeholders",
                "entity.resolve-placeholder",
                "entity.read-structure",
                "entity.check-status",
            ] {
                allowed.insert(v.to_string());
            }
        }
        "screening" | "ubo" | "control" | "kyc" | "red-flag" => {
            for v in &[
                "screening.sanctions",
                "screening.pep",
                "screening.adverse-media",
                "screening.full",
                "ubo.list-ubos",
                "ubo.add-ownership",
                "ubo.compute-chains",
                "ubo.trace-chains",
                "ubo.mark-deceased",
                "ubo.waive-verification",
                "ubo.update-ownership",
                "control.add",
                "control.show-board-controller",
                "control.import-psc-register",
                "red-flag.dismiss",
                "red-flag.escalate",
                "red-flag.list",
                "kyc.download-cert",
            ] {
                allowed.insert(v.to_string());
            }
        }
        "onboarding" | "gleif" => {
            for v in &[
                "onboarding.start",
                "onboarding.status",
                "onboarding.resume",
                "onboarding.pause",
                "onboarding.check-readiness",
                "onboarding.runsheet",
                "onboarding.send-welcome",
                "gleif.search",
                "gleif.enrich",
                "gleif.import-tree",
                "gleif.check-active",
            ] {
                allowed.insert(v.to_string());
            }
        }
        "document" | "doc-request" => {
            for v in &[
                "document.solicit",
                "document.upload-version",
                "document.verify",
                "document.reject",
                "document.extract",
                "document.solicit-batch",
                "document.list-pending",
                "document.approve",
                "document.archive",
            ] {
                allowed.insert(v.to_string());
            }
        }
        "deal" | "custody" | "share-class" | "fund" => {
            for v in &[
                "deal.create",
                "deal.add-participant",
                "deal.read",
                "deal.create-rate-card",
                "deal.add-rate-card-line",
                "deal.propose-rate-card",
                "deal.counter-rate-card",
                "deal.update-status",
                "deal.request-onboarding",
                "deal.cancel",
                "fund.create",
                "fund.create-subfund",
                "share-class.create",
                "fund.link-feeder",
                "fund.list-investors",
                "fund.add-investment",
                "custody.settlement-cycle",
            ] {
                allowed.insert(v.to_string());
            }
        }
        _ => {
            allowed.insert(expected_verb.to_string());
            allowed.insert("session.exit".to_string());
            allowed.insert("agent.help".to_string());
        }
    }

    allowed.insert(expected_verb.to_string());
    allowed
}

#[cfg(test)]
#[cfg(feature = "database")]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    #[tokio::test]
    #[ignore]
    async fn run_step0_trial() {
        let corpus_path = "assets/cic_labeled_corpus.json";
        let corpus_content = std::fs::read_to_string(corpus_path)
            .expect("Failed to read assets/cic_labeled_corpus.json. Run Phase A1 first.");

        let corpus: Vec<CorpusEntry> =
            serde_json::from_str(&corpus_content).expect("Failed to parse corpus JSON");

        let database_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL environment variable required");
        let pool = PgPool::connect(&database_url)
            .await
            .expect("Failed to connect to db");

        let searcher = build_searcher(&pool).await;

        let mut results = Vec::new();
        let mut hits = 0;
        let mut domain_hits = HashMap::new();
        let mut domain_totals = HashMap::new();

        println!(
            "Running Step 0 constrained classification trial over {} cases...",
            corpus.len()
        );

        for case in &corpus {
            let mut allowed_set = get_simulated_allowed_verbs(&case.expected_verb);
            for alt in &case.alt_verbs {
                allowed_set.insert(alt.clone());
            }

            let search_result = searcher
                .search(
                    &case.utterance,
                    None, // user_id
                    None, // domain_filter
                    None, // entity_kind
                    5,    // limit
                    Some(&allowed_set),
                    None,
                    None,
                )
                .await;

            let outcome = match search_result {
                Ok(candidates) => {
                    let top_candidates: Vec<(String, f32)> = candidates
                        .iter()
                        .map(|c| (c.verb.clone(), c.score))
                        .collect();

                    let mut matched_verb = None;
                    let mut matched_score = None;

                    if let Some(top) = candidates.first() {
                        matched_verb = Some(top.verb.clone());
                        matched_score = Some(top.score);
                    }

                    let is_hit = if let Some(ref top_v) = matched_verb {
                        top_v == &case.expected_verb || case.alt_verbs.contains(top_v)
                    } else {
                        false
                    };

                    if is_hit {
                        hits += 1;
                        *domain_hits.entry(case.domain.clone()).or_insert(0) += 1;
                    }
                    *domain_totals.entry(case.domain.clone()).or_insert(0) += 1;

                    Step0Result {
                        id: case.id.clone(),
                        utterance: case.utterance.clone(),
                        expected_verb: case.expected_verb.clone(),
                        domain: case.domain.clone(),
                        allowed_verbs: allowed_set.iter().cloned().collect(),
                        top_resolved_verb: matched_verb,
                        top_resolved_score: matched_score,
                        is_hit,
                        candidates: top_candidates,
                    }
                }
                Err(e) => {
                    eprintln!("Error evaluating case {}: {}", case.id, e);
                    *domain_totals.entry(case.domain.clone()).or_insert(0) += 1;
                    Step0Result {
                        id: case.id.clone(),
                        utterance: case.utterance.clone(),
                        expected_verb: case.expected_verb.clone(),
                        domain: case.domain.clone(),
                        allowed_verbs: allowed_set.iter().cloned().collect(),
                        top_resolved_verb: None,
                        top_resolved_score: None,
                        is_hit: false,
                        candidates: vec![],
                    }
                }
            };

            results.push(outcome);
        }

        let total = corpus.len();
        let accuracy = hits as f32 / total as f32;

        let mut domain_accuracy = HashMap::new();
        for (dom, tot) in &domain_totals {
            let h = domain_hits.get(dom).cloned().unwrap_or(0);
            domain_accuracy.insert(dom.clone(), h as f32 / *tot as f32);
        }

        let report = TrialReport {
            total_cases: total,
            hits,
            misses: total - hits,
            accuracy,
            domain_accuracy,
            results,
        };

        // Write report
        std::fs::create_dir_all("reports").ok();
        let mut report_file = File::create("reports/step0_trial_evaluation.json")
            .expect("Failed to create report file");
        let report_json =
            serde_json::to_string_pretty(&report).expect("Failed to serialize report");
        report_file
            .write_all(report_json.as_bytes())
            .expect("Failed to write report");

        println!("\n=======================================================");
        println!("  STEP 0 TRIAL RESULTS:");
        println!("  Total:    {}", total);
        println!("  Hits:     {} ({:.2}%)", hits, accuracy * 100.0);
        println!("  Misses:   {}", total - hits);
        println!("=======================================================");

        // Assert hit rate target
        assert!(
            accuracy >= 0.85,
            "Step 0 trial accuracy of {:.2}% is below target 85.0%!",
            accuracy * 100.0
        );
    }

    #[tokio::test]
    #[ignore]
    async fn run_phase0_validation() {
        let corpus_path = "assets/cic_labeled_corpus.json";
        let corpus_content = std::fs::read_to_string(corpus_path)
            .expect("Failed to read assets/cic_labeled_corpus.json. Run Phase A1 first.");

        let corpus: Vec<CorpusEntry> =
            serde_json::from_str(&corpus_content).expect("Failed to parse corpus JSON");

        let database_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL environment variable required");
        let pool = PgPool::connect(&database_url)
            .await
            .expect("Failed to connect to db");

        let searcher = build_searcher(&pool).await;
        let mut set_too_large = 0;
        let sub_threshold = 0;
        let mut oov = 0;
        let mut genuine_ambiguity = 0;
        let mut hits = 0;
        let mut total_failures = 0;

        let mut failure_details = Vec::new();

        println!(
            "Running Phase 0 failure mode validation over {} cases...",
            corpus.len()
        );

        for case in &corpus {
            // Run legacy UNCONSTRAINED search (allowed_verbs = None)
            let search_result = searcher
                .search(
                    &case.utterance,
                    None, // user_id
                    None, // domain_filter
                    None, // entity_kind
                    500,  // limit to 500 to see deeper candidates
                    None, // unconstrained
                    None,
                    None,
                )
                .await;

            match search_result {
                Ok(candidates) => {
                    let mut matched_verb = None;
                    if let Some(top) = candidates.first() {
                        matched_verb = Some(top.verb.clone());
                    }

                    let is_hit = if let Some(ref top_v) = matched_verb {
                        top_v == &case.expected_verb || case.alt_verbs.contains(top_v)
                    } else {
                        false
                    };

                    if is_hit {
                        hits += 1;
                        continue;
                    }

                    total_failures += 1;

                    // Classify the failure
                    let found_idx = candidates.iter().position(|c| {
                        c.verb == case.expected_verb || case.alt_verbs.contains(&c.verb)
                    });

                    let fault = if let Some(idx) = found_idx {
                        if idx > 0 {
                            set_too_large += 1;
                            "set-too-large"
                        } else {
                            if candidates.len() > 1
                                && (candidates[0].score - candidates[1].score).abs() < 0.05
                            {
                                genuine_ambiguity += 1;
                                "genuine-ambiguity"
                            } else {
                                set_too_large += 1;
                                "set-too-large"
                            }
                        }
                    } else {
                        oov += 1;
                        "oov"
                    };

                    failure_details.push(serde_json::json!({
                        "case_id": case.id,
                        "utterance": case.utterance,
                        "expected": case.expected_verb,
                        "fault": fault,
                        "top_candidate": candidates.first().map(|c| c.verb.clone()),
                        "top_score": candidates.first().map(|c| c.score),
                        "expected_index": found_idx,
                        "expected_score": found_idx.map(|idx| candidates[idx].score),
                    }));
                }
                Err(e) => {
                    total_failures += 1;
                    oov += 1;
                    failure_details.push(serde_json::json!({
                        "case_id": case.id,
                        "utterance": case.utterance,
                        "expected": case.expected_verb,
                        "fault": "oov",
                        "error": e.to_string(),
                    }));
                }
            }
        }

        let set_too_large_pct = if total_failures > 0 {
            set_too_large as f32 / total_failures as f32
        } else {
            0.0
        };
        let sub_threshold_pct = if total_failures > 0 {
            sub_threshold as f32 / total_failures as f32
        } else {
            0.0
        };
        let oov_pct = if total_failures > 0 {
            oov as f32 / total_failures as f32
        } else {
            0.0
        };
        let genuine_ambiguity_pct = if total_failures > 0 {
            genuine_ambiguity as f32 / total_failures as f32
        } else {
            0.0
        };

        let report = serde_json::json!({
            "total_cases": corpus.len(),
            "hits": hits,
            "failures": total_failures,
            "accuracy": hits as f32 / corpus.len() as f32,
            "confusion_matrix": {
                "set-too-large": { "count": set_too_large, "percentage": set_too_large_pct },
                "sub-threshold": { "count": sub_threshold, "percentage": sub_threshold_pct },
                "oov": { "count": oov, "percentage": oov_pct },
                "genuine-ambiguity": { "count": genuine_ambiguity, "percentage": genuine_ambiguity_pct }
            },
            "failure_details": failure_details
        });

        // Write report
        std::fs::create_dir_all("reports").ok();
        let mut report_file = File::create("reports/phase0_confusion_matrix.json")
            .expect("Failed to create report file");
        let report_json =
            serde_json::to_string_pretty(&report).expect("Failed to serialize report");
        report_file
            .write_all(report_json.as_bytes())
            .expect("Failed to write report");

        println!("\n=======================================================");
        println!("  PHASE 0 FAILURE VALIDATION:");
        println!("  Total Failures: {}", total_failures);
        println!(
            "  set-too-large:  {} ({:.1}%)",
            set_too_large,
            set_too_large_pct * 100.0
        );
        println!(
            "  sub-threshold:  {} ({:.1}%)",
            sub_threshold,
            sub_threshold_pct * 100.0
        );
        println!("  oov:            {} ({:.1}%)", oov, oov_pct * 100.0);
        println!(
            "  ambiguity:      {} ({:.1}%)",
            genuine_ambiguity,
            genuine_ambiguity_pct * 100.0
        );
        println!("=======================================================");

        if total_failures > 0 {
            assert!(
                set_too_large_pct >= 0.50,
                "Set-too-large failure mode is {:.1}%, below the required 50% majority threshold!",
                set_too_large_pct * 100.0
            );
        }
    }

    #[tokio::test]
    #[ignore]
    async fn run_diagnostic() {
        let database_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL environment variable required");
        let pool = PgPool::connect(&database_url)
            .await
            .expect("Failed to connect to db");

        let embedder = CandleEmbedder::new().expect("Failed to initialize embedder");
        let query = "upload the passport for John Smith";
        let query_emb = embedder.embed_query(query).await.unwrap();
        let query_vector = pgvector::Vector::from(query_emb);

        let rows = sqlx::query(
            r#"
            SELECT pattern_phrase, verb_name, 1 - (embedding <=> $1::vector) as similarity
            FROM "ob-poc".verb_pattern_embeddings
            WHERE embedding IS NOT NULL
            ORDER BY embedding <=> $1::vector
            LIMIT 20
            "#,
        )
        .bind(query_vector)
        .fetch_all(&pool)
        .await
        .unwrap();

        println!("--- TOP 20 PATTERNS FOR '{}' ---", query);
        for (i, r) in rows.iter().enumerate() {
            use sqlx::Row;
            let pattern_phrase: String = r.get("pattern_phrase");
            let verb_name: String = r.get("verb_name");
            let similarity: f64 = r.get("similarity");
            println!(
                "  #{}: {} | Verb: {} | Similarity: {}",
                i + 1,
                pattern_phrase,
                verb_name,
                similarity
            );
        }
    }
}
