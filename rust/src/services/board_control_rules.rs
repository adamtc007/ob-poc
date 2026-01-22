//! Board Control Rules Engine
//!
//! Computes who controls the board of a CBU's entities using a priority-ordered
//! rules engine. The derivation is auditable and explains why a controller was selected.
//!
//! ## Rules Priority (evaluated in order)
//!
//! 1. **Rule A - Board Appointment Rights**: Entity that can appoint/remove majority of directors
//! 2. **Rule B - Voting Rights Majority**: Entity with >50% voting power
//! 3. **Rule C - Special Instrument**: Golden share, GP authority, trustee powers (can override A/B)
//! 4. **Rule D - No Single Controller**: No entity meets threshold, return top candidates
//!
//! ## Evidence Sources
//!
//! - GLEIF Relationship Records
//! - BODS Ownership Statements
//! - Investor Register (share class → votes per share)
//! - Governance Documents (articles, shareholder agreements)
//! - Special Instruments (golden share, GP/LP agreements)

use chrono::{NaiveDate, Utc};
use ob_poc_types::control::{
    BoardControlExplanation, BoardControlMethod, BoardControllerEdge, ControlCandidate,
    ControlConfidence, ControlEdgeType, ControlScore, EvidenceRef, EvidenceSource,
};
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

/// Row struct for board controller DB queries
#[derive(Debug, sqlx::FromRow)]
struct BoardControllerRow {
    id: Uuid,
    cbu_id: Uuid,
    controller_entity_id: Option<Uuid>,
    controller_name: Option<String>,
    method: String,
    confidence: String,
    score: Decimal,
    as_of: NaiveDate,
    explanation: serde_json::Value,
}

/// Configuration for the rules engine
#[derive(Debug, Clone)]
pub struct RulesEngineConfig {
    /// Threshold for board appointment majority (default 50%)
    pub board_appointment_threshold: f32,
    /// Threshold for voting rights majority (default 50%)
    pub voting_rights_threshold: f32,
    /// PSC threshold for significant control (default 25%)
    pub psc_threshold: f32,
    /// Maximum depth to walk control chains
    pub max_chain_depth: u32,
}

impl Default for RulesEngineConfig {
    fn default() -> Self {
        Self {
            board_appointment_threshold: 50.0,
            voting_rights_threshold: 50.0,
            psc_threshold: 25.0,
            max_chain_depth: 10,
        }
    }
}

/// Result of a board control computation
#[derive(Debug, Clone)]
pub struct BoardControlResult {
    pub controller_entity_id: Option<Uuid>,
    pub controller_name: Option<String>,
    pub method: BoardControlMethod,
    pub confidence: ControlConfidence,
    pub score: f32,
    pub explanation: BoardControlExplanation,
}

/// Raw control edge from database
#[derive(Debug, Clone)]
struct RawControlEdge {
    id: Uuid,
    from_entity_id: Uuid,
    from_entity_name: String,
    #[allow(dead_code)]
    from_entity_type: String,
    #[allow(dead_code)]
    to_entity_id: Uuid,
    edge_type: String,
    percentage: Option<f32>,
    #[allow(dead_code)]
    is_direct: bool,
    source_register: Option<String>,
    effective_date: Option<NaiveDate>,
}

/// The board control rules engine
pub struct BoardControlRulesEngine {
    pool: PgPool,
    config: RulesEngineConfig,
}

impl BoardControlRulesEngine {
    /// Create a new rules engine
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            config: RulesEngineConfig::default(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(pool: PgPool, config: RulesEngineConfig) -> Self {
        Self { pool, config }
    }

    /// Compute board controller for a CBU
    ///
    /// This is the main entry point. It:
    /// 1. Finds the CBU's issuer entity (the fund legal entity)
    /// 2. Walks the control graph upward
    /// 3. Applies rules A → B → C → D in priority order
    /// 4. Returns the computed controller with full explanation
    pub async fn compute_for_cbu(&self, cbu_id: Uuid) -> Result<BoardControlResult, String> {
        let as_of = Utc::now().date_naive();

        // 1. Get the CBU's issuer entity (anchor with role 'issuer')
        let issuer_entity_id = self.get_cbu_issuer_entity(cbu_id).await?;

        let Some(target_entity_id) = issuer_entity_id else {
            // No issuer entity linked - can't compute
            return Ok(BoardControlResult {
                controller_entity_id: None,
                controller_name: None,
                method: BoardControlMethod::NoSingleController,
                confidence: ControlConfidence::Low,
                score: 0.0,
                explanation: BoardControlExplanation {
                    as_of: Some(as_of),
                    rule_fired: Some(BoardControlMethod::NoSingleController),
                    candidates: vec![],
                    evidence_refs: vec![],
                    data_gaps: vec!["No issuer entity linked to CBU".to_string()],
                },
            });
        };

        // 2. Load control edges pointing TO the target entity
        let edges = self.load_control_edges(target_entity_id).await?;

        if edges.is_empty() {
            return Ok(BoardControlResult {
                controller_entity_id: None,
                controller_name: None,
                method: BoardControlMethod::NoSingleController,
                confidence: ControlConfidence::Low,
                score: 0.0,
                explanation: BoardControlExplanation {
                    as_of: Some(as_of),
                    rule_fired: Some(BoardControlMethod::NoSingleController),
                    candidates: vec![],
                    evidence_refs: vec![],
                    data_gaps: vec!["No control edges found for issuer entity".to_string()],
                },
            });
        }

        // 3. Build candidate scores
        let mut candidates: HashMap<Uuid, CandidateBuilder> = HashMap::new();
        let mut evidence_refs: Vec<EvidenceRef> = vec![];
        let mut data_gaps: Vec<String> = vec![];

        for edge in &edges {
            let candidate = candidates.entry(edge.from_entity_id).or_insert_with(|| {
                CandidateBuilder::new(edge.from_entity_id, edge.from_entity_name.clone())
            });

            let edge_type = ControlEdgeType::from_db_str(&edge.edge_type);

            match edge_type {
                Some(ControlEdgeType::AppointsBoard) => {
                    // Rule A: Board appointment rights
                    let pct = edge.percentage.unwrap_or(100.0);
                    candidate.add_appointment_rights(pct);
                    candidate.add_reason(format!("Has {}% board appointment rights", pct));
                    evidence_refs.push(EvidenceRef {
                        source_type: EvidenceSource::GovernanceDoc,
                        source_id: edge.id.to_string(),
                        description: format!("Board appointment rights: {}%", pct),
                        as_of: edge.effective_date,
                    });
                }
                Some(ControlEdgeType::HoldsVotingRights) => {
                    // Rule B: Voting rights
                    if let Some(pct) = edge.percentage {
                        candidate.add_voting_rights(pct);
                        candidate.add_reason(format!("Holds {}% voting rights", pct));
                        evidence_refs.push(EvidenceRef {
                            source_type: if edge.source_register.as_deref()
                                == Some("investor_register")
                            {
                                EvidenceSource::InvestorRegister
                            } else {
                                EvidenceSource::BodsStatement
                            },
                            source_id: edge.id.to_string(),
                            description: format!("Voting rights: {}%", pct),
                            as_of: edge.effective_date,
                        });
                    }
                }
                Some(ControlEdgeType::HoldsShares) => {
                    // Shares imply voting unless voting rights edge overrides
                    if let Some(pct) = edge.percentage {
                        // Only use shares for voting if no explicit voting rights
                        if candidate.s_vote < 0.01 {
                            candidate.add_voting_rights(pct * 0.8); // Discount: shares ≠ votes always
                            candidate.add_reason(format!("Holds {}% shares (implied voting)", pct));
                        }
                        evidence_refs.push(EvidenceRef {
                            source_type: EvidenceSource::InvestorRegister,
                            source_id: edge.id.to_string(),
                            description: format!("Shareholding: {}%", pct),
                            as_of: edge.effective_date,
                        });
                    }
                }
                Some(ControlEdgeType::ExercisesInfluence) => {
                    // Rule C: Special influence (could be golden share, etc.)
                    candidate.add_special_influence();
                    candidate.add_reason("Exercises significant influence or control".to_string());
                    evidence_refs.push(EvidenceRef {
                        source_type: EvidenceSource::SpecialInstrument,
                        source_id: edge.id.to_string(),
                        description: "Significant influence or control".to_string(),
                        as_of: edge.effective_date,
                    });
                }
                Some(ControlEdgeType::IsTrustee) => {
                    // Trustee has control in trust structures
                    candidate.add_special_override();
                    candidate.add_reason("Is trustee of trust arrangement".to_string());
                    evidence_refs.push(EvidenceRef {
                        source_type: EvidenceSource::SpecialInstrument,
                        source_id: edge.id.to_string(),
                        description: "Trustee control".to_string(),
                        as_of: edge.effective_date,
                    });
                }
                Some(ControlEdgeType::ManagedBy) => {
                    // ManCo relationship - governance anchor
                    candidate.add_affiliation(0.3);
                    candidate.add_reason("Is management company (GLEIF relationship)".to_string());
                    evidence_refs.push(EvidenceRef {
                        source_type: EvidenceSource::GleifRr,
                        source_id: edge.id.to_string(),
                        description: "IS_FUND_MANAGED_BY relationship".to_string(),
                        as_of: edge.effective_date,
                    });
                }
                _ => {
                    // Other edge types contribute weak affiliation signal
                    candidate.add_affiliation(0.1);
                }
            }
        }

        // Check for data gaps
        if !candidates.values().any(|c| c.s_appoint > 0.0) {
            data_gaps.push("No explicit board appointment rights documented".to_string());
        }
        if !candidates.values().any(|c| c.s_vote > 0.0) {
            data_gaps.push("No voting rights data from investor register".to_string());
        }

        // 4. Apply rules in priority order
        let mut final_candidates: Vec<ControlCandidate> =
            candidates.into_values().map(|b| b.build()).collect();

        // Sort by total score descending
        final_candidates.sort_by(|a, b| {
            b.total_score
                .partial_cmp(&a.total_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Determine winner and method
        let (controller, method) = self.apply_rules(&final_candidates);

        // Calculate overall confidence
        let data_coverage = self.calculate_data_coverage(&evidence_refs, &data_gaps);
        let top_score = final_candidates
            .first()
            .map(|c| c.total_score)
            .unwrap_or(0.0);

        let confidence = if controller.is_some() {
            if data_coverage > 0.8 && top_score > 0.7 {
                ControlConfidence::High
            } else if data_coverage > 0.5 && top_score > 0.5 {
                ControlConfidence::Medium
            } else {
                ControlConfidence::Low
            }
        } else {
            ControlConfidence::Low
        };

        let explanation = BoardControlExplanation {
            as_of: Some(as_of),
            rule_fired: Some(method),
            candidates: final_candidates,
            evidence_refs,
            data_gaps,
        };

        Ok(BoardControlResult {
            controller_entity_id: controller.as_ref().map(|c| c.entity_id),
            controller_name: controller.map(|c| c.entity_name),
            method,
            confidence,
            score: top_score,
            explanation,
        })
    }

    /// Apply rules A → B → C → D to find the controller
    fn apply_rules(
        &self,
        candidates: &[ControlCandidate],
    ) -> (Option<ControlCandidate>, BoardControlMethod) {
        if candidates.is_empty() {
            return (None, BoardControlMethod::NoSingleController);
        }

        // Rule A: Check for board appointment rights majority
        for candidate in candidates {
            if candidate.score.s_appoint >= self.config.board_appointment_threshold / 100.0 {
                return (
                    Some(candidate.clone()),
                    BoardControlMethod::BoardAppointmentRights,
                );
            }
        }

        // Rule C: Check for special override (golden share, trustee, etc.)
        // This can supersede Rule B
        for candidate in candidates {
            if candidate.score.s_override > 0.0 {
                return (
                    Some(candidate.clone()),
                    BoardControlMethod::SpecialInstrument,
                );
            }
        }

        // Rule B: Check for voting rights majority
        for candidate in candidates {
            if candidate.score.s_vote >= self.config.voting_rights_threshold / 100.0 {
                return (
                    Some(candidate.clone()),
                    BoardControlMethod::VotingRightsMajority,
                );
            }
        }

        // Rule D: No single controller - check if top candidate has significant score
        let top = &candidates[0];
        if top.total_score > 0.3 {
            // Has significant presence but not majority control
            (Some(top.clone()), BoardControlMethod::NoSingleController)
        } else {
            (None, BoardControlMethod::NoSingleController)
        }
    }

    /// Get the issuer entity for a CBU (from control anchors)
    async fn get_cbu_issuer_entity(&self, cbu_id: Uuid) -> Result<Option<Uuid>, String> {
        // First try control anchors
        let anchor_result = sqlx::query_scalar::<_, Uuid>(
            r#"SELECT entity_id FROM "ob-poc".cbu_control_anchors
               WHERE cbu_id = $1 AND anchor_role = 'issuer'
               LIMIT 1"#,
        )
        .bind(cbu_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Failed to get issuer anchor: {}", e))?;

        if anchor_result.is_some() {
            return Ok(anchor_result);
        }

        // Fallback: look for fund entity with same name/ID
        // This is a heuristic when anchors aren't set up
        let fallback_result = sqlx::query_scalar::<_, Uuid>(
            r#"SELECT e.entity_id
               FROM "ob-poc".cbus c
               JOIN "ob-poc".entity_limited_companies e
                 ON LOWER(e.company_name) = LOWER(c.name)
               WHERE c.cbu_id = $1
               LIMIT 1"#,
        )
        .bind(cbu_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Failed to get fallback issuer: {}", e))?;

        Ok(fallback_result)
    }

    /// Load control edges pointing to an entity
    async fn load_control_edges(
        &self,
        target_entity_id: Uuid,
    ) -> Result<Vec<RawControlEdge>, String> {
        let rows = sqlx::query_as::<
            _,
            (
                Uuid,
                Uuid,
                String,
                String,
                Uuid,
                String,
                Option<rust_decimal::Decimal>,
                bool,
                Option<String>,
                Option<NaiveDate>,
            ),
        >(
            r#"SELECT
                ce.id,
                ce.from_entity_id,
                COALESCE(ep.search_name, elc.company_name, 'Unknown') as from_entity_name,
                COALESCE(
                    CASE WHEN ep.entity_id IS NOT NULL THEN 'Person' ELSE NULL END,
                    CASE WHEN elc.entity_id IS NOT NULL THEN 'LegalEntity' ELSE NULL END,
                    'Unknown'
                ) as from_entity_type,
                ce.to_entity_id,
                ce.edge_type,
                ce.percentage,
                ce.is_direct,
                ce.source_register,
                ce.effective_date
            FROM "ob-poc".control_edges ce
            LEFT JOIN "ob-poc".entity_proper_persons ep ON ep.entity_id = ce.from_entity_id
            LEFT JOIN "ob-poc".entity_limited_companies elc ON elc.entity_id = ce.from_entity_id
            WHERE ce.to_entity_id = $1
              AND ce.end_date IS NULL
            ORDER BY ce.percentage DESC NULLS LAST"#,
        )
        .bind(target_entity_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Failed to load control edges: {}", e))?;

        Ok(rows
            .into_iter()
            .map(
                |(
                    id,
                    from_id,
                    from_name,
                    from_type,
                    to_id,
                    edge_type,
                    pct,
                    is_direct,
                    source,
                    eff_date,
                )| {
                    RawControlEdge {
                        id,
                        from_entity_id: from_id,
                        from_entity_name: from_name,
                        from_entity_type: from_type,
                        to_entity_id: to_id,
                        edge_type,
                        percentage: pct.map(|d| d.to_string().parse().unwrap_or(0.0)),
                        is_direct,
                        source_register: source,
                        effective_date: eff_date,
                    }
                },
            )
            .collect())
    }

    /// Calculate data coverage score
    fn calculate_data_coverage(&self, evidence: &[EvidenceRef], gaps: &[String]) -> f32 {
        let evidence_score = (evidence.len() as f32 * 0.2).min(1.0);
        let gap_penalty = (gaps.len() as f32 * 0.2).min(0.5);
        (evidence_score - gap_penalty).max(0.0)
    }

    /// Store the computed board controller in the database
    pub async fn store_result(
        &self,
        cbu_id: Uuid,
        result: &BoardControlResult,
        triggered_by: &str,
    ) -> Result<Uuid, String> {
        let explanation_json = serde_json::to_value(&result.explanation)
            .map_err(|e| format!("Failed to serialize explanation: {}", e))?;

        // Upsert the board controller
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"INSERT INTO "ob-poc".cbu_board_controller
               (cbu_id, controller_entity_id, controller_name, method, confidence, score, as_of, explanation, computed_by)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
               ON CONFLICT (cbu_id) DO UPDATE SET
                 controller_entity_id = EXCLUDED.controller_entity_id,
                 controller_name = EXCLUDED.controller_name,
                 method = EXCLUDED.method,
                 confidence = EXCLUDED.confidence,
                 score = EXCLUDED.score,
                 as_of = EXCLUDED.as_of,
                 explanation = EXCLUDED.explanation,
                 computed_at = NOW(),
                 computed_by = EXCLUDED.computed_by
               RETURNING id"#,
        )
        .bind(cbu_id)
        .bind(result.controller_entity_id)
        .bind(&result.controller_name)
        .bind(result.method.to_db_str())
        .bind(result.confidence.to_db_str())
        .bind(result.score)
        .bind(result.explanation.as_of.unwrap_or_else(|| Utc::now().date_naive()))
        .bind(explanation_json)
        .bind(triggered_by)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("Failed to store board controller: {}", e))?;

        // Store evidence references
        for evidence in &result.explanation.evidence_refs {
            let _ = sqlx::query(
                r#"INSERT INTO "ob-poc".board_control_evidence
                   (cbu_board_controller_id, source_type, source_id, description, as_of)
                   VALUES ($1, $2, $3, $4, $5)"#,
            )
            .bind(id)
            .bind(evidence.source_type.to_db_str())
            .bind(&evidence.source_id)
            .bind(&evidence.description)
            .bind(evidence.as_of)
            .execute(&self.pool)
            .await;
        }

        Ok(id)
    }

    /// Load existing board controller for a CBU
    pub async fn load_for_cbu(&self, cbu_id: Uuid) -> Result<Option<BoardControllerEdge>, String> {
        let row: Option<BoardControllerRow> = sqlx::query_as(
            r#"SELECT id, cbu_id, controller_entity_id, controller_name, method, confidence, score, as_of, explanation
               FROM "ob-poc".cbu_board_controller
               WHERE cbu_id = $1"#,
        )
        .bind(cbu_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Failed to load board controller: {}", e))?;

        match row {
            Some(r) => {
                let explanation: BoardControlExplanation =
                    serde_json::from_value(r.explanation).unwrap_or_default();

                Ok(Some(BoardControllerEdge {
                    id: r.id,
                    cbu_id: r.cbu_id,
                    controller_entity_id: r.controller_entity_id,
                    controller_name: r.controller_name,
                    method: BoardControlMethod::from_db_str(&r.method)
                        .unwrap_or(BoardControlMethod::NoSingleController),
                    confidence: ControlConfidence::from_db_str(&r.confidence)
                        .unwrap_or(ControlConfidence::Low),
                    score: r.score.to_string().parse().unwrap_or(0.0),
                    as_of: r.as_of,
                    explanation,
                }))
            }
            None => Ok(None),
        }
    }
}

/// Builder for accumulating candidate scores
struct CandidateBuilder {
    entity_id: Uuid,
    entity_name: String,
    s_appoint: f32,
    s_vote: f32,
    s_affiliation: f32,
    s_override: f32,
    reasons: Vec<String>,
}

impl CandidateBuilder {
    fn new(entity_id: Uuid, entity_name: String) -> Self {
        Self {
            entity_id,
            entity_name,
            s_appoint: 0.0,
            s_vote: 0.0,
            s_affiliation: 0.0,
            s_override: 0.0,
            reasons: vec![],
        }
    }

    fn add_appointment_rights(&mut self, pct: f32) {
        self.s_appoint = (self.s_appoint + pct / 100.0).min(1.0);
    }

    fn add_voting_rights(&mut self, pct: f32) {
        self.s_vote = (self.s_vote + pct / 100.0).min(1.0);
    }

    fn add_affiliation(&mut self, score: f32) {
        self.s_affiliation = (self.s_affiliation + score).min(1.0);
    }

    fn add_special_influence(&mut self) {
        self.s_affiliation = (self.s_affiliation + 0.5).min(1.0);
    }

    fn add_special_override(&mut self) {
        self.s_override = 1.0;
    }

    fn add_reason(&mut self, reason: String) {
        self.reasons.push(reason);
    }

    fn build(self) -> ControlCandidate {
        let score = ControlScore {
            s_appoint: self.s_appoint,
            s_vote: self.s_vote,
            s_affiliation: self.s_affiliation,
            s_override: self.s_override,
            data_coverage: 0.7, // Default coverage, adjusted later
        };
        let total_score = score.total();

        ControlCandidate {
            entity_id: self.entity_id,
            entity_name: self.entity_name,
            score,
            total_score,
            why: self.reasons,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_control_score_calculation() {
        let score = ControlScore {
            s_appoint: 0.6,
            s_vote: 0.4,
            s_affiliation: 0.2,
            s_override: 0.0,
            data_coverage: 0.8,
        };

        // 0.70 * 0.6 + 0.25 * 0.4 + 0.05 * 0.2 = 0.42 + 0.1 + 0.01 = 0.53
        // * 0.8 = 0.424
        let total = score.total();
        assert!((total - 0.424).abs() < 0.001);
    }

    #[test]
    fn test_override_trumps_everything() {
        let score = ControlScore {
            s_appoint: 0.0,
            s_vote: 0.0,
            s_affiliation: 0.0,
            s_override: 1.0,
            data_coverage: 0.5,
        };

        assert_eq!(score.total(), 1.0);
    }

    #[test]
    fn test_confidence_levels() {
        // High: data_coverage > 0.8 AND total() > 0.7
        // total = (0.70*s_appoint + 0.25*s_vote + 0.05*s_affiliation) * data_coverage
        // For high: total = (0.70*1.0 + 0.25*0.8 + 0.05*0.5) * 0.9 = 0.925 * 0.9 = 0.8325
        let high = ControlScore {
            s_appoint: 1.0,
            s_vote: 0.8,
            s_affiliation: 0.5,
            s_override: 0.0,
            data_coverage: 0.9,
        };
        assert_eq!(high.confidence(), ControlConfidence::High);

        // Medium: data_coverage > 0.5 AND total() > 0.5
        // total = (0.70*0.7 + 0.25*0.6 + 0.05*0.3) * 0.7 = 0.655 * 0.7 = 0.4585
        // Adjust to get above 0.5: (0.70*0.8 + 0.25*0.6 + 0.05*0.3) * 0.8 = 0.725 * 0.8 = 0.58
        let medium = ControlScore {
            s_appoint: 0.8,
            s_vote: 0.6,
            s_affiliation: 0.3,
            s_override: 0.0,
            data_coverage: 0.8,
        };
        assert_eq!(medium.confidence(), ControlConfidence::Medium);

        // Low: either data_coverage <= 0.5 or total() <= 0.5
        let low = ControlScore {
            s_appoint: 0.2,
            s_vote: 0.1,
            s_affiliation: 0.1,
            s_override: 0.0,
            data_coverage: 0.3,
        };
        assert_eq!(low.confidence(), ControlConfidence::Low);
    }
}
