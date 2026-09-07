//! T7.2 (`EOP-PLAN-KYCUBO-KIT-T7` §2) — the plain-English ramp's tier-0
//! retrieval + deterministic disposition, v0 (no SLM — that's T7.3).
//!
//! Mirrors, does not adopt, `HybridVerbSearcher`'s semantic tier
//! (`crate::mcp::verb_search::search_patterns_constrained` /
//! `search_verb_patterns_semantic_constrained`) — same `verb_pattern_embeddings`
//! table, same pgvector cosine-distance query shape, but a new small function
//! with NO dependency on `crate::mcp`, per T0.1c mirror-not-adopt and T7 §1
//! I-5 (this module lives in `crate::repl`, imports workbook/substrate types
//! only, never the other direction).
//!
//! **I-1 Proposal-only.** This module never calls `KycWorkbook::stage()` /
//! `commit()` / `run()` — see `ramp_never_stages` below, a source-scan tooth
//! mirroring `zero_inference_assertion`'s discipline.
//!
//! **I-2 Board-restricted.** `rank_board_with_scores` only ever iterates
//! `board.moves` — a score for a verb absent from the board cannot appear in
//! its output, by construction (not by filtering after the fact).
//!
//! **I-3 Deterministic disposition.** `decide_disposition` is the sole
//! authority over select/clarify/abstain; the retrieval step produces only
//! `f32` scores, never a disposition.
//!
//! **Wired 2026-08-16.** `KycWorkbookCommand::Propose{utterance}`
//! (`super::kyc_workbook_surface`) is now the production caller of
//! `rank_board_with_scores`/`rank_placement_set`/`decide_disposition` below.
use std::collections::HashMap;

use ob_poc_kyc_substrate::{MoveId, PlacementSet, NONE_OF_THE_ABOVE};

/// One board move with its retrieval score. Ordered highest-first by
/// `rank_board_with_scores`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RankedMove {
    pub move_id: MoveId,
    pub score: f32,
}

/// Pure scoring step: given a board and a precomputed `verb_fqn -> score`
/// map (from wherever — live embeddings, a fixture, a fake), returns one
/// `RankedMove` per board move, sorted by score descending. A verb absent
/// from `verb_scores` (or absent from the board) scores 0.0 / is absent from
/// the output respectively — the output vector's `move_id`s are always a
/// subset of `board.moves`' ids, which is I-2 held by construction: this
/// function has no code path that can emit a `MoveId` it didn't read off
/// `board.moves`.
pub(crate) fn rank_board_with_scores(
    board: &PlacementSet,
    verb_scores: &HashMap<String, f32>,
) -> Vec<RankedMove> {
    let mut ranked: Vec<RankedMove> = board
        .moves
        .iter()
        .map(|m| {
            let score = if m.verb_fqn.as_str() == NONE_OF_THE_ABOVE {
                // The abstention candidate is never retrieval-scored — the
                // disposition policy (I-3), not similarity, is what decides
                // whether to fall back to it.
                0.0
            } else {
                verb_scores.get(m.verb_fqn.as_str()).copied().unwrap_or(0.0)
            };
            RankedMove {
                move_id: m.move_id.clone(),
                score,
            }
        })
        .collect();
    ranked.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    ranked
}

/// DB-backed tier-0 retrieval: embeds nothing itself (caller supplies the
/// utterance embedding — this module has no model-loading concern), queries
/// `"ob-poc".verb_pattern_embeddings` constrained to the board's own verb
/// fqns only (`verb_name = ANY($2)` — the SQL-level mirror of I-2, on top of
/// the construction-level guarantee `rank_board_with_scores` already gives),
/// then delegates scoring to the pure function above.
///
/// No production call site yet: `KycWorkbookCommand::Propose{utterance}`
/// (T7 §3 Q2a) — the REPL surface that would call this with a real
/// embedding and a real board — is deliberately its own follow-up step, not
/// bundled into this tranche. Exercised today by
/// `rank_placement_set_scopes_to_board` below (real pool, real table) so the
/// SQL-level half of I-2 is proven now rather than deferred untested.
#[cfg(feature = "database")]
pub(crate) async fn rank_placement_set(
    pool: &sqlx::PgPool,
    utterance_embedding: &[f32],
    board: &PlacementSet,
) -> Result<Vec<RankedMove>, sqlx::Error> {
    use pgvector::Vector;
    use std::collections::HashSet;

    let board_verb_fqns: Vec<String> = board
        .moves
        .iter()
        .map(|m| m.verb_fqn.as_str().to_string())
        .filter(|fqn| fqn != NONE_OF_THE_ABOVE)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    if board_verb_fqns.is_empty() {
        return Ok(rank_board_with_scores(board, &HashMap::new()));
    }

    let embedding_vec = Vector::from(utterance_embedding.to_vec());
    let rows: Vec<(String, f64)> = sqlx::query_as(
        r#"
        SELECT verb_name, MAX(1 - (embedding <=> $1::vector)) as best_similarity
        FROM "ob-poc".verb_pattern_embeddings
        WHERE embedding IS NOT NULL AND verb_name = ANY($2)
        GROUP BY verb_name
        "#,
    )
    .bind(&embedding_vec)
    .bind(&board_verb_fqns)
    .fetch_all(pool)
    .await?;

    let verb_scores: HashMap<String, f32> = rows
        .into_iter()
        .map(|(verb, sim)| (verb, sim as f32))
        .collect();

    Ok(rank_board_with_scores(board, &verb_scores))
}

/// I-3: the sole disposition authority. A model (T7.3) may only ever supply
/// scalars into `ranked` — this function, not the model, decides the
/// outcome.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DispositionOutcome {
    Select(MoveId),
    Clarify(Vec<MoveId>),
    Abstain,
}

/// v0 threshold policy (T7.2 §2 point 3): select the top candidate if it
/// clears `select_threshold` AND leads the runner-up by at least `margin`;
/// clarify on a genuine multi-peak (top clears the bar but the runner-up is
/// too close to call); abstain otherwise. These are v0 defaults, not gated
/// thresholds — T7.3's promotion criteria (charter §5) are a separate,
/// later ratification and do not reuse these constants.
pub(crate) const DEFAULT_SELECT_THRESHOLD: f32 = 0.55;
pub(crate) const DEFAULT_CLARIFY_MARGIN: f32 = 0.05;

pub(crate) fn decide_disposition(
    ranked: &[RankedMove],
    select_threshold: f32,
    clarify_margin: f32,
) -> DispositionOutcome {
    let Some(top) = ranked.first() else {
        return DispositionOutcome::Abstain;
    };
    if top.score < select_threshold {
        return DispositionOutcome::Abstain;
    }
    if let Some(second) = ranked.get(1) {
        if top.score - second.score < clarify_margin {
            return DispositionOutcome::Clarify(vec![top.move_id.clone(), second.move_id.clone()]);
        }
    }
    DispositionOutcome::Select(top.move_id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ob_poc_kyc_substrate::{LegalMove, SubjectId, TargetBinding, VerbFqn};
    use uuid::Uuid;

    fn fixture_board() -> PlacementSet {
        let subject = SubjectId(Uuid::new_v4());
        let moves = vec![
            LegalMove {
                move_id: MoveId("kyc_ubo.assert.subject.place::subject".to_string()),
                verb_fqn: VerbFqn("kyc_ubo.assert.subject.place".to_string()),
                target: TargetBinding::for_subject(subject),
                proposed_edge: None,
                proposed_entity_type: None,
            },
            LegalMove {
                move_id: MoveId("kyc_ubo.assert.subject.enquiry::subject".to_string()),
                verb_fqn: VerbFqn("kyc_ubo.assert.subject.enquiry".to_string()),
                target: TargetBinding::for_subject(subject),
                proposed_edge: None,
                proposed_entity_type: None,
            },
            LegalMove {
                move_id: PlacementSet::abstain_move_id(),
                verb_fqn: VerbFqn(NONE_OF_THE_ABOVE.to_string()),
                target: TargetBinding::default(),
                proposed_edge: None,
                proposed_entity_type: None,
            },
        ];
        PlacementSet {
            subject,
            board_hash: ob_poc_kyc_substrate::Hash::of_json(&serde_json::json!({"moves": []})),
            moves,
        }
    }

    /// I-2: even when the scores map names a verb that is NOT on the board
    /// (simulating what an unconstrained/global search might hand back),
    /// the ranked output must never contain it — the function has no code
    /// path capable of emitting a move id it didn't read off `board.moves`.
    #[test]
    fn ramp_only_proposes_board_moves() {
        let board = fixture_board();
        let mut scores = HashMap::new();
        scores.insert("kyc_ubo.assert.subject.place".to_string(), 0.9);
        // A verb NOT on this board at all — must never leak into the output.
        scores.insert("kyc_ubo.assert.edge.connect".to_string(), 0.99);

        let ranked = rank_board_with_scores(&board, &scores);

        let board_ids: std::collections::HashSet<&str> =
            board.moves.iter().map(|m| m.move_id.0.as_str()).collect();
        for r in &ranked {
            assert!(
                board_ids.contains(r.move_id.0.as_str()),
                "ranked output contained a move not on the board: {:?}",
                r.move_id
            );
        }
        assert_eq!(
            ranked.len(),
            board.moves.len(),
            "every board move must be scored, none dropped"
        );
        // The off-board score must have had zero opportunity to matter.
        assert!(!ranked
            .iter()
            .any(|r| r.move_id.0.contains("assert-control")));
    }

    /// I-3: two near-tied top candidates must clarify, never silently pick
    /// the marginally-higher one.
    #[test]
    fn multi_peak_clarifies_never_selects() {
        let board = fixture_board();
        let mut scores = HashMap::new();
        scores.insert("kyc_ubo.assert.subject.place".to_string(), 0.80);
        scores.insert("kyc_ubo.assert.subject.enquiry".to_string(), 0.79);

        let ranked = rank_board_with_scores(&board, &scores);
        let outcome = decide_disposition(&ranked, DEFAULT_SELECT_THRESHOLD, DEFAULT_CLARIFY_MARGIN);

        match outcome {
            DispositionOutcome::Clarify(candidates) => {
                assert_eq!(candidates.len(), 2);
            }
            other => panic!("expected Clarify on a near-tied board, got {other:?}"),
        }
    }

    #[test]
    fn clear_winner_selects() {
        let board = fixture_board();
        let mut scores = HashMap::new();
        scores.insert("kyc_ubo.assert.subject.place".to_string(), 0.90);
        scores.insert("kyc_ubo.assert.subject.enquiry".to_string(), 0.10);

        let ranked = rank_board_with_scores(&board, &scores);
        let outcome = decide_disposition(&ranked, DEFAULT_SELECT_THRESHOLD, DEFAULT_CLARIFY_MARGIN);

        assert_eq!(
            outcome,
            DispositionOutcome::Select(MoveId("kyc_ubo.assert.subject.place::subject".to_string()))
        );
    }

    #[test]
    fn below_threshold_abstains() {
        let board = fixture_board();
        let mut scores = HashMap::new();
        scores.insert("kyc_ubo.assert.subject.place".to_string(), 0.20);

        let ranked = rank_board_with_scores(&board, &scores);
        let outcome = decide_disposition(&ranked, DEFAULT_SELECT_THRESHOLD, DEFAULT_CLARIFY_MARGIN);

        assert_eq!(outcome, DispositionOutcome::Abstain);
    }

    /// SQL-layer half of I-2: `rank_placement_set`'s `verb_name = ANY(...)`
    /// constraint must hold against the real table too, not just the pure
    /// scoring function above. Smoke-level (no assertion on score values —
    /// that's tier-0 retrieval quality, not this tooth's concern): the
    /// query must execute and every returned move id must be on the board.
    #[cfg(feature = "database")]
    #[tokio::test]
    async fn rank_placement_set_scopes_to_board() {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql:///data_designer".to_string());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&database_url)
            .await
            .expect("connect to test DB");

        let board = fixture_board();
        let embedding = vec![0.01f32; 384];

        let ranked = rank_placement_set(&pool, &embedding, &board)
            .await
            .expect("rank_placement_set must succeed against a real pool");

        let board_ids: std::collections::HashSet<&str> =
            board.moves.iter().map(|m| m.move_id.0.as_str()).collect();
        for r in &ranked {
            assert!(board_ids.contains(r.move_id.0.as_str()));
        }
        assert_eq!(ranked.len(), board.moves.len());
    }

    /// I-1: this module must never call the workbook's mutation surface.
    /// Mirrors `zero_inference_assertion`'s source-scan discipline — a
    /// static, always-on tooth rather than trusting code review alone.
    #[test]
    fn ramp_never_stages() {
        let source = include_str!("kyc_ramp.rs");
        const FORBIDDEN: &[&str] = &[".stage(", ".commit(", ".run(", "append_in_scope"];
        for needle in FORBIDDEN {
            // Skip this test's own scan machinery (doc comments, the
            // FORBIDDEN list literal itself, and this filter/message) — it
            // necessarily mentions the forbidden strings to name them.
            let hits: Vec<&str> = source
                .lines()
                .filter(|l| {
                    let t = l.trim_start();
                    l.contains(needle)
                        && !t.starts_with("//")
                        && !t.starts_with("const FORBIDDEN")
                        && !t.contains("mutation surface")
                })
                .collect();
            assert!(
                hits.is_empty(),
                "kyc_ramp.rs must never call the workbook mutation surface — found {needle:?} in: {hits:#?}"
            );
        }
    }
}
