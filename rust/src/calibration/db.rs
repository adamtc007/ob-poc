//! Database persistence helpers for calibration artifacts.

use anyhow::{Context, Result};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::classifier::CalibrationUtteranceRow;
use super::types::{
    CalibrationDrift, CalibrationFixtureTransition, CalibrationMetrics, CalibrationMode,
    CalibrationOutcome, CalibrationPortfolioEntry, CalibrationRun, CalibrationScenario,
    CalibrationScenarioBundle, CalibrationUtteranceReviewRow, CalibrationVerdict,
    CalibrationWriteThroughSummary, ExpectedOutcome, GeneratedUtterance, GovernanceStatus,
    NegativeType, ProposedGapEntry, SuggestedClarification,
};

/// Minimal persistence facade for calibration tables.
pub struct CalibrationStore {
    pool: PgPool,
}

impl CalibrationStore {
    /// Create a calibration store from a Postgres pool.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use ob_poc::calibration::CalibrationStore;
    ///
    /// # fn demo(pool: sqlx::PgPool) {
    /// let _store = CalibrationStore::new(pool);
    /// # }
    /// ```
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Return a clone of the underlying Postgres pool.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use ob_poc::calibration::CalibrationStore;
    ///
    /// # fn demo(pool: sqlx::PgPool) {
    /// let store = CalibrationStore::new(pool);
    /// let _pool = store.pool();
    /// # }
    /// ```
    pub fn pool(&self) -> PgPool {
        self.pool.clone()
    }

    /// Insert or update a scenario seed.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use ob_poc::calibration::{CalibrationExecutionShape, CalibrationScenario, CalibrationStore, GovernanceStatus};
    /// use uuid::Uuid;
    ///
    /// # async fn demo(store: CalibrationStore) -> anyhow::Result<()> {
    /// let scenario = CalibrationScenario {
    ///     scenario_id: Uuid::new_v4(),
    ///     scenario_name: "demo".into(),
    ///     created_by: "demo".into(),
    ///     governance_status: GovernanceStatus::Draft,
    ///     constellation_template_id: "struct.demo".into(),
    ///     constellation_template_version: "v1".into(),
    ///     situation_signature: "entity:ACTIVE".into(),
    ///     situation_signature_hash: None,
    ///     operational_phase: "Active".into(),
    ///     target_entity_type: "entity".into(),
    ///     target_entity_state: "ACTIVE".into(),
    ///     linked_entity_states: vec![],
    ///     target_verb: "entity.read".into(),
    ///     legal_verb_set_snapshot: vec![],
    ///     verb_taxonomy_tag: "read".into(),
    ///     excluded_neighbours: vec![],
    ///     near_neighbour_verbs: vec![],
    ///     expected_margin_threshold: 0.1,
    ///     execution_shape: CalibrationExecutionShape::Singleton,
    ///     gold_utterances: vec![],
    ///     admitted_synthetic_set_id: None,
    /// };
    /// store.upsert_scenario(&scenario).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn upsert_scenario(&self, scenario: &CalibrationScenario) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".calibration_scenarios (
                scenario_id, scenario_name, created_by, governance_status,
                constellation_template_id, constellation_template_version,
                situation_signature, situation_signature_hash, operational_phase,
                target_entity_type, target_entity_state, linked_entity_states,
                target_verb, legal_verb_set_snapshot, verb_taxonomy_tag,
                excluded_neighbours, near_neighbour_verbs, expected_margin_threshold,
                execution_shape, gold_utterances, admitted_synthetic_set_id, seed_data
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                $13, $14, $15, $16, $17, $18, $19, $20, $21, $22
            )
            ON CONFLICT (scenario_id) DO UPDATE SET
                scenario_name = EXCLUDED.scenario_name,
                governance_status = EXCLUDED.governance_status,
                constellation_template_version = EXCLUDED.constellation_template_version,
                situation_signature = EXCLUDED.situation_signature,
                situation_signature_hash = EXCLUDED.situation_signature_hash,
                operational_phase = EXCLUDED.operational_phase,
                target_entity_type = EXCLUDED.target_entity_type,
                target_entity_state = EXCLUDED.target_entity_state,
                linked_entity_states = EXCLUDED.linked_entity_states,
                target_verb = EXCLUDED.target_verb,
                legal_verb_set_snapshot = EXCLUDED.legal_verb_set_snapshot,
                verb_taxonomy_tag = EXCLUDED.verb_taxonomy_tag,
                excluded_neighbours = EXCLUDED.excluded_neighbours,
                near_neighbour_verbs = EXCLUDED.near_neighbour_verbs,
                expected_margin_threshold = EXCLUDED.expected_margin_threshold,
                execution_shape = EXCLUDED.execution_shape,
                gold_utterances = EXCLUDED.gold_utterances,
                admitted_synthetic_set_id = EXCLUDED.admitted_synthetic_set_id,
                seed_data = EXCLUDED.seed_data,
                updated_at = now()
            "#,
        )
        .bind(scenario.scenario_id)
        .bind(&scenario.scenario_name)
        .bind(&scenario.created_by)
        .bind(serde_json::to_string(&scenario.governance_status)?)
        .bind(&scenario.constellation_template_id)
        .bind(&scenario.constellation_template_version)
        .bind(&scenario.situation_signature)
        .bind(scenario.situation_signature_hash)
        .bind(&scenario.operational_phase)
        .bind(&scenario.target_entity_type)
        .bind(&scenario.target_entity_state)
        .bind(serde_json::to_value(&scenario.linked_entity_states)?)
        .bind(&scenario.target_verb)
        .bind(serde_json::to_value(&scenario.legal_verb_set_snapshot)?)
        .bind(&scenario.verb_taxonomy_tag)
        .bind(serde_json::to_value(&scenario.excluded_neighbours)?)
        .bind(serde_json::to_value(&scenario.near_neighbour_verbs)?)
        .bind(scenario.expected_margin_threshold)
        .bind(serde_json::to_string(&scenario.execution_shape)?)
        .bind(serde_json::to_value(&scenario.gold_utterances)?)
        .bind(scenario.admitted_synthetic_set_id)
        .bind(serde_json::to_value(scenario)?)
        .execute(&self.pool)
        .await
        .context("upsert calibration scenario")?;
        Ok(())
    }

    /// Persist generated utterances for a scenario.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use ob_poc::calibration::CalibrationStore;
    /// use uuid::Uuid;
    ///
    /// # async fn demo(store: CalibrationStore, utterances: Vec<ob_poc::calibration::GeneratedUtterance>) -> anyhow::Result<()> {
    /// store.insert_generated_utterances(Uuid::new_v4(), &utterances).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn insert_generated_utterances(
        &self,
        scenario_id: Uuid,
        utterances: &[GeneratedUtterance],
    ) -> Result<Vec<Uuid>> {
        let mut ids = Vec::with_capacity(utterances.len());
        for utterance in utterances {
            let utterance_id = Uuid::new_v4();
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".calibration_utterances (
                    utterance_id, scenario_id, text, calibration_mode, negative_type,
                    expected_outcome, generation_rationale
                ) VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
            )
            .bind(utterance_id)
            .bind(scenario_id)
            .bind(&utterance.text)
            .bind(serde_json::to_string(&utterance.calibration_mode)?)
            .bind(
                utterance
                    .negative_type
                    .map(|value| serde_json::to_string(&value))
                    .transpose()?,
            )
            .bind(serde_json::to_value(&utterance.expected_outcome)?)
            .bind(&utterance.generation_rationale)
            .execute(&self.pool)
            .await
            .context("insert calibration utterance")?;
            ids.push(utterance_id);
        }
        Ok(ids)
    }

    /// Load a scenario seed by ID.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use ob_poc::calibration::CalibrationStore;
    /// use uuid::Uuid;
    ///
    /// # async fn demo(store: CalibrationStore, scenario_id: Uuid) -> anyhow::Result<()> {
    /// let _scenario = store.get_scenario(scenario_id).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_scenario(&self, scenario_id: Uuid) -> Result<Option<CalibrationScenario>> {
        let row = sqlx::query_scalar::<_, serde_json::Value>(
            r#"
            SELECT seed_data
            FROM "ob-poc".calibration_scenarios
            WHERE scenario_id = $1
            "#,
        )
        .bind(scenario_id)
        .fetch_optional(&self.pool)
        .await
        .context("load calibration scenario")?;
        row.map(|value| serde_json::from_value(value).context("decode calibration scenario"))
            .transpose()
    }

    /// List utterances for a scenario, optionally filtering by lifecycle status.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use ob_poc::calibration::CalibrationStore;
    /// use uuid::Uuid;
    ///
    /// # async fn demo(store: CalibrationStore, scenario_id: Uuid) -> anyhow::Result<()> {
    /// let _rows = store.list_utterances(scenario_id, Some("Admitted")).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list_utterances(
        &self,
        scenario_id: Uuid,
        lifecycle_status: Option<&str>,
    ) -> Result<Vec<CalibrationUtteranceRow>> {
        let rows = sqlx::query(
            r#"
            SELECT utterance_id, text, calibration_mode, negative_type, expected_outcome, pre_screen
            FROM "ob-poc".calibration_utterances
            WHERE scenario_id = $1
              AND ($2::TEXT IS NULL OR lifecycle_status = $2)
            ORDER BY created_at ASC
            "#,
        )
        .bind(scenario_id)
        .bind(lifecycle_status)
        .fetch_all(&self.pool)
        .await
        .context("list calibration utterances")?;

        rows.into_iter()
            .map(|row| {
                let calibration_mode = serde_json::from_str::<CalibrationMode>(
                    &row.get::<String, _>("calibration_mode"),
                )
                .context("decode calibration_mode")?;
                let negative_type = row
                    .get::<Option<String>, _>("negative_type")
                    .map(|value| serde_json::from_str::<NegativeType>(&value))
                    .transpose()
                    .context("decode negative_type")?;
                let expected_outcome = serde_json::from_value::<ExpectedOutcome>(
                    row.get::<serde_json::Value, _>("expected_outcome"),
                )
                .context("decode expected_outcome")?;
                let pre_screen = row
                    .get::<Option<serde_json::Value>, _>("pre_screen")
                    .map(serde_json::from_value)
                    .transpose()
                    .context("decode pre_screen")?;

                Ok(CalibrationUtteranceRow {
                    utterance_id: row.get("utterance_id"),
                    text: row.get("text"),
                    calibration_mode,
                    negative_type,
                    expected_outcome,
                    pre_screen,
                })
            })
            .collect()
    }

    /// List utterances with review metadata for operator curation.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use ob_poc::calibration::CalibrationStore;
    /// use uuid::Uuid;
    ///
    /// # async fn demo(store: CalibrationStore, scenario_id: Uuid) -> anyhow::Result<()> {
    /// let _rows = store.list_review_utterances(Some(scenario_id), Some("Screened")).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list_review_utterances(
        &self,
        scenario_id: Option<Uuid>,
        lifecycle_status: Option<&str>,
    ) -> Result<Vec<CalibrationUtteranceReviewRow>> {
        let rows = sqlx::query(
            r#"
            SELECT utterance_id, scenario_id, text, calibration_mode, negative_type,
                   lifecycle_status, expected_outcome, pre_screen, generation_rationale,
                   reviewed_by, admitted_at, deprecated_at, created_at
            FROM "ob-poc".calibration_utterances
            WHERE ($1::UUID IS NULL OR scenario_id = $1)
              AND ($2::TEXT IS NULL OR lifecycle_status = $2)
            ORDER BY created_at ASC
            "#,
        )
        .bind(scenario_id)
        .bind(lifecycle_status)
        .fetch_all(&self.pool)
        .await
        .context("list calibration review utterances")?;

        rows.into_iter()
            .map(|row| {
                Ok(CalibrationUtteranceReviewRow {
                    utterance_id: row.get("utterance_id"),
                    scenario_id: row.get("scenario_id"),
                    text: row.get("text"),
                    calibration_mode: serde_json::from_str::<CalibrationMode>(
                        &row.get::<String, _>("calibration_mode"),
                    )
                    .context("decode review calibration_mode")?,
                    negative_type: row
                        .get::<Option<String>, _>("negative_type")
                        .map(|value| serde_json::from_str::<NegativeType>(&value))
                        .transpose()
                        .context("decode review negative_type")?,
                    lifecycle_status: row.get("lifecycle_status"),
                    expected_outcome: serde_json::from_value::<ExpectedOutcome>(
                        row.get::<serde_json::Value, _>("expected_outcome"),
                    )
                    .context("decode review expected_outcome")?,
                    pre_screen: row
                        .get::<Option<serde_json::Value>, _>("pre_screen")
                        .map(serde_json::from_value)
                        .transpose()
                        .context("decode review pre_screen")?,
                    generation_rationale: row.get("generation_rationale"),
                    reviewed_by: row.get("reviewed_by"),
                    admitted_at: row.get("admitted_at"),
                    deprecated_at: row.get("deprecated_at"),
                    created_at: row.get("created_at"),
                })
            })
            .collect()
    }

    /// Update one utterance row with pre-screen output and lifecycle status.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use ob_poc::calibration::{CalibrationStore, EmbeddingPreScreen, PreScreenStratum};
    /// use uuid::Uuid;
    ///
    /// # async fn demo(store: CalibrationStore, utterance_id: Uuid) -> anyhow::Result<()> {
    /// let pre_screen = EmbeddingPreScreen {
    ///     utterance: "demo".into(),
    ///     target_verb_distance: 0.1,
    ///     nearest_neighbour_distance: 0.2,
    ///     nearest_neighbour_verb: "entity.read".into(),
    ///     margin: 0.1,
    ///     stratum: PreScreenStratum::ClearMatch { distance: 0.1 },
    /// };
    /// store.update_pre_screen(utterance_id, &pre_screen, "Screened").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn update_pre_screen(
        &self,
        utterance_id: Uuid,
        pre_screen: &super::types::EmbeddingPreScreen,
        lifecycle_status: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE "ob-poc".calibration_utterances
            SET pre_screen = $2,
                pre_screen_stratum = $3,
                lifecycle_status = $4
            WHERE utterance_id = $1
            "#,
        )
        .bind(utterance_id)
        .bind(serde_json::to_value(pre_screen)?)
        .bind(serde_json::to_string(&pre_screen.stratum)?)
        .bind(lifecycle_status)
        .execute(&self.pool)
        .await
        .context("update calibration utterance pre_screen")?;
        Ok(())
    }

    /// Mark one utterance as admitted or deprecated during review.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use ob_poc::calibration::CalibrationStore;
    /// use uuid::Uuid;
    ///
    /// # async fn demo(store: CalibrationStore, utterance_id: Uuid) -> anyhow::Result<()> {
    /// store.review_utterance(utterance_id, true, "reviewer").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn review_utterance(
        &self,
        utterance_id: Uuid,
        admit: bool,
        reviewer: &str,
    ) -> Result<()> {
        let lifecycle_status = if admit { "Admitted" } else { "Deprecated" };
        sqlx::query(
            r#"
            UPDATE "ob-poc".calibration_utterances
            SET lifecycle_status = $2,
                reviewed_by = $3,
                admitted_at = CASE WHEN $2 = 'Admitted' THEN now() ELSE admitted_at END,
                deprecated_at = CASE WHEN $2 = 'Deprecated' THEN now() ELSE deprecated_at END
            WHERE utterance_id = $1
            "#,
        )
        .bind(utterance_id)
        .bind(lifecycle_status)
        .bind(reviewer)
        .execute(&self.pool)
        .await
        .context("review calibration utterance")?;
        Ok(())
    }

    /// Build a lightweight portfolio summary across all scenarios.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use ob_poc::calibration::CalibrationStore;
    ///
    /// # async fn demo(store: CalibrationStore) -> anyhow::Result<()> {
    /// let _rows = store.build_portfolio_summary().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn build_portfolio_summary(&self) -> Result<Vec<CalibrationPortfolioEntry>> {
        let rows = sqlx::query(
            r#"
            SELECT s.scenario_id,
                   s.scenario_name,
                   s.target_verb,
                   s.governance_status,
                   (
                       SELECT COUNT(*)
                       FROM "ob-poc".calibration_utterances u
                       WHERE u.scenario_id = s.scenario_id
                         AND u.lifecycle_status = 'Admitted'
                   ) AS admitted_utterance_count,
                   r.run_id AS last_run_id,
                   r.metrics AS last_metrics,
                   r.run_start AS last_run_start
            FROM "ob-poc".calibration_scenarios s
            LEFT JOIN LATERAL (
                SELECT run_id, metrics, run_start
                FROM "ob-poc".calibration_runs
                WHERE scenario_id = s.scenario_id
                ORDER BY run_start DESC
                LIMIT 1
            ) r ON true
            ORDER BY s.scenario_name ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("build calibration portfolio summary")?;

        rows.into_iter()
            .map(|row| {
                let metrics = row
                    .get::<Option<serde_json::Value>, _>("last_metrics")
                    .map(serde_json::from_value::<CalibrationMetrics>)
                    .transpose()
                    .context("decode portfolio metrics")?;
                Ok(CalibrationPortfolioEntry {
                    scenario_id: row.get("scenario_id"),
                    scenario_name: row.get("scenario_name"),
                    target_verb: row.get("target_verb"),
                    governance_status: serde_json::from_str::<GovernanceStatus>(
                        &row.get::<String, _>("governance_status"),
                    )
                    .context("decode portfolio governance_status")?,
                    admitted_utterance_count: row.get::<i64, _>("admitted_utterance_count")
                        as usize,
                    last_run_id: row.get("last_run_id"),
                    overall_accuracy: metrics.as_ref().map(|value| value.overall_accuracy),
                    fallback_rate: metrics.as_ref().map(|value| value.phase4_fallback_rate),
                    fragile_boundary_count: metrics
                        .as_ref()
                        .map(|value| value.fragile_boundary_count),
                    last_run_start: row.get("last_run_start"),
                })
            })
            .collect()
    }

    /// Export a portable bundle for one scenario and its utterances.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use ob_poc::calibration::CalibrationStore;
    /// use uuid::Uuid;
    ///
    /// # async fn demo(store: CalibrationStore, scenario_id: Uuid) -> anyhow::Result<()> {
    /// let _bundle = store.export_scenario_bundle(scenario_id).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn export_scenario_bundle(
        &self,
        scenario_id: Uuid,
    ) -> Result<Option<CalibrationScenarioBundle>> {
        let Some(scenario) = self.get_scenario(scenario_id).await? else {
            return Ok(None);
        };
        let utterances = self.list_review_utterances(Some(scenario_id), None).await?;
        Ok(Some(CalibrationScenarioBundle {
            exported_at: chrono::Utc::now(),
            scenario,
            utterances,
        }))
    }

    /// Import a portable scenario bundle.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use ob_poc::calibration::{CalibrationScenarioBundle, CalibrationStore};
    ///
    /// # async fn demo(store: CalibrationStore, bundle: CalibrationScenarioBundle) -> anyhow::Result<()> {
    /// store.import_scenario_bundle(&bundle).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn import_scenario_bundle(&self, bundle: &CalibrationScenarioBundle) -> Result<()> {
        self.upsert_scenario(&bundle.scenario).await?;

        for utterance in &bundle.utterances {
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".calibration_utterances (
                    utterance_id, scenario_id, text, calibration_mode, negative_type,
                    lifecycle_status, expected_outcome, generation_rationale, pre_screen,
                    reviewed_by, admitted_at, deprecated_at, created_at
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13
                )
                ON CONFLICT (utterance_id) DO UPDATE SET
                    text = EXCLUDED.text,
                    calibration_mode = EXCLUDED.calibration_mode,
                    negative_type = EXCLUDED.negative_type,
                    lifecycle_status = EXCLUDED.lifecycle_status,
                    expected_outcome = EXCLUDED.expected_outcome,
                    generation_rationale = EXCLUDED.generation_rationale,
                    pre_screen = EXCLUDED.pre_screen,
                    reviewed_by = EXCLUDED.reviewed_by,
                    admitted_at = EXCLUDED.admitted_at,
                    deprecated_at = EXCLUDED.deprecated_at
                "#,
            )
            .bind(utterance.utterance_id)
            .bind(utterance.scenario_id)
            .bind(&utterance.text)
            .bind(serde_json::to_string(&utterance.calibration_mode)?)
            .bind(
                utterance
                    .negative_type
                    .map(|value| serde_json::to_string(&value))
                    .transpose()?,
            )
            .bind(&utterance.lifecycle_status)
            .bind(serde_json::to_value(&utterance.expected_outcome)?)
            .bind(&utterance.generation_rationale)
            .bind(
                utterance
                    .pre_screen
                    .as_ref()
                    .map(serde_json::to_value)
                    .transpose()?,
            )
            .bind(&utterance.reviewed_by)
            .bind(utterance.admitted_at)
            .bind(utterance.deprecated_at)
            .bind(utterance.created_at)
            .execute(&self.pool)
            .await
            .context("import calibration utterance bundle row")?;
        }

        Ok(())
    }

    /// Persist per-utterance fixture snapshots for one completed run.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use ob_poc::calibration::{CalibrationFixtureTransition, CalibrationStore};
    /// use uuid::Uuid;
    ///
    /// # async fn demo(store: CalibrationStore, rows: Vec<CalibrationFixtureTransition>) -> anyhow::Result<()> {
    /// store.insert_fixture_transitions(Uuid::new_v4(), &rows).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn insert_fixture_transitions(
        &self,
        run_id: Uuid,
        rows: &[CalibrationFixtureTransition],
    ) -> Result<()> {
        for row in rows {
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".calibration_fixture_transitions (
                    transition_id, run_id, utterance_id, trace_id, fixture_state
                ) VALUES ($1, $2, $3, $4, $5)
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(run_id)
            .bind(row.utterance_id)
            .bind(row.trace_id)
            .bind(serde_json::to_value(&row.fixture_state)?)
            .execute(&self.pool)
            .await
            .context("insert calibration fixture transition")?;
        }
        Ok(())
    }

    /// Persist a completed calibration run.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use ob_poc::calibration::CalibrationStore;
    ///
    /// # async fn demo(store: CalibrationStore, run: ob_poc::calibration::CalibrationRun) -> anyhow::Result<()> {
    /// store.insert_run(&run).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn insert_run(&self, run: &CalibrationRun) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".calibration_runs (
                run_id, scenario_id, triggered_by, surface_versions, utterance_count,
                positive_count, negative_count, boundary_count, metrics, drift,
                prior_run_id, trace_ids, run_start, run_end
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14
            )
            "#,
        )
        .bind(run.run_id)
        .bind(run.scenario_id)
        .bind(&run.triggered_by)
        .bind(serde_json::to_value(&run.surface_versions)?)
        .bind(run.utterance_count as i32)
        .bind(run.positive_count as i32)
        .bind(run.negative_count as i32)
        .bind(run.boundary_count as i32)
        .bind(serde_json::to_value(&run.metrics)?)
        .bind(run.drift.as_ref().map(serde_json::to_value).transpose()?)
        .bind(run.prior_run_id)
        .bind(serde_json::to_value(&run.trace_ids)?)
        .bind(run.run_start)
        .bind(run.run_end)
        .execute(&self.pool)
        .await
        .context("insert calibration run")?;
        Ok(())
    }

    /// Load a completed calibration run by ID, including its outcomes.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use ob_poc::calibration::CalibrationStore;
    /// use uuid::Uuid;
    ///
    /// # async fn demo(store: CalibrationStore, run_id: Uuid) -> anyhow::Result<()> {
    /// let _run = store.get_run(run_id).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_run(&self, run_id: Uuid) -> Result<Option<CalibrationRun>> {
        let row = sqlx::query(
            r#"
            SELECT run_id, scenario_id, triggered_by, surface_versions, utterance_count,
                   positive_count, negative_count, boundary_count, metrics, drift,
                   prior_run_id, trace_ids, run_start, run_end
            FROM "ob-poc".calibration_runs
            WHERE run_id = $1
            "#,
        )
        .bind(run_id)
        .fetch_optional(&self.pool)
        .await
        .context("load calibration run")?;

        let Some(row) = row else {
            return Ok(None);
        };
        self.decode_run_row(row).await.map(Some)
    }

    /// Load all persisted fixture snapshots for one completed run.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use ob_poc::calibration::CalibrationStore;
    /// use uuid::Uuid;
    ///
    /// # async fn demo(store: CalibrationStore, run_id: Uuid) -> anyhow::Result<()> {
    /// let _rows = store.list_fixture_transitions(run_id).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list_fixture_transitions(
        &self,
        run_id: Uuid,
    ) -> Result<Vec<CalibrationFixtureTransition>> {
        let rows = sqlx::query(
            r#"
            SELECT utterance_id, trace_id, fixture_state
            FROM "ob-poc".calibration_fixture_transitions
            WHERE run_id = $1
            ORDER BY created_at ASC
            "#,
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await
        .context("load calibration fixture transitions")?;

        rows.into_iter()
            .map(|row| {
                Ok(CalibrationFixtureTransition {
                    utterance_id: row.get("utterance_id"),
                    trace_id: row.get("trace_id"),
                    fixture_state: serde_json::from_value(row.get("fixture_state"))
                        .context("decode calibration fixture_state")?,
                })
            })
            .collect()
    }

    /// Load the most recent completed run for a scenario, including outcomes.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use ob_poc::calibration::CalibrationStore;
    /// use uuid::Uuid;
    ///
    /// # async fn demo(store: CalibrationStore, scenario_id: Uuid) -> anyhow::Result<()> {
    /// let _run = store.latest_run_for_scenario(scenario_id).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn latest_run_for_scenario(
        &self,
        scenario_id: Uuid,
    ) -> Result<Option<CalibrationRun>> {
        let row = sqlx::query(
            r#"
            SELECT run_id, scenario_id, triggered_by, surface_versions, utterance_count,
                   positive_count, negative_count, boundary_count, metrics, drift,
                   prior_run_id, trace_ids, run_start, run_end
            FROM "ob-poc".calibration_runs
            WHERE scenario_id = $1
            ORDER BY run_start DESC
            LIMIT 1
            "#,
        )
        .bind(scenario_id)
        .fetch_optional(&self.pool)
        .await
        .context("load latest calibration run")?;

        let Some(row) = row else {
            return Ok(None);
        };
        self.decode_run_row(row).await.map(Some)
    }

    /// Persist classified outcomes for a run.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use ob_poc::calibration::CalibrationStore;
    /// use uuid::Uuid;
    ///
    /// # async fn demo(store: CalibrationStore, outcomes: Vec<ob_poc::calibration::CalibrationOutcome>) -> anyhow::Result<()> {
    /// store.insert_outcomes(Uuid::new_v4(), &outcomes).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn insert_outcomes(
        &self,
        run_id: Uuid,
        outcomes: &[CalibrationOutcome],
    ) -> Result<()> {
        for outcome in outcomes {
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".calibration_outcomes (
                    outcome_id, run_id, utterance_id, trace_id, calibration_mode,
                    negative_type, expected_outcome, verdict, actual_resolved_verb,
                    actual_halt_reason, failure_phase, failure_detail, top1_score,
                    top2_score, margin, margin_stable, latency_total_ms, latency_per_phase
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
                    $14, $15, $16, $17, $18
                )
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(run_id)
            .bind(outcome.utterance_id)
            .bind(outcome.trace_id)
            .bind(serde_json::to_string(&outcome.calibration_mode)?)
            .bind(
                outcome
                    .negative_type
                    .map(|value| serde_json::to_string(&value))
                    .transpose()?,
            )
            .bind(serde_json::to_value(&outcome.expected_outcome)?)
            .bind(serde_json::to_string(&outcome.verdict)?)
            .bind(&outcome.actual_resolved_verb)
            .bind(&outcome.actual_halt_reason)
            .bind(outcome.failure_phase.map(i16::from))
            .bind(&outcome.failure_detail)
            .bind(outcome.top1_score)
            .bind(outcome.top2_score)
            .bind(outcome.margin)
            .bind(outcome.margin_stable)
            .bind(outcome.latency_total_ms.map(|value| value as i32))
            .bind(
                outcome
                    .latency_per_phase
                    .as_ref()
                    .map(serde_json::to_value)
                    .transpose()?,
            )
            .execute(&self.pool)
            .await
            .context("insert calibration outcome")?;
        }
        Ok(())
    }

    /// Write calibration Loop 1 / Loop 2 outputs into the live learning stores.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use ob_poc::calibration::CalibrationStore;
    ///
    /// # async fn demo(store: CalibrationStore) -> anyhow::Result<()> {
    /// let summary = store.write_through_learning(&[], &[]).await?;
    /// assert_eq!(summary.loop1_candidates_upserted, 0);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn write_through_learning(
        &self,
        gaps: &[ProposedGapEntry],
        clarifications: &[SuggestedClarification],
    ) -> Result<CalibrationWriteThroughSummary> {
        let mut summary = CalibrationWriteThroughSummary::default();

        for gap in gaps {
            let fingerprint = format!(
                "calibration_gap:{}:{}:{}",
                gap.target_verb,
                gap.entity_state.to_lowercase(),
                gap.utterance.to_lowercase().trim()
            );
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".learning_candidates (
                    fingerprint, learning_type, input_pattern, suggested_output,
                    risk_level, auto_applicable, domain_hint
                )
                VALUES ($1, 'invocation_phrase', $2, $3, 'medium', false, $4)
                ON CONFLICT (fingerprint) DO UPDATE SET
                    occurrence_count = "ob-poc".learning_candidates.occurrence_count + 1,
                    last_seen = NOW(),
                    domain_hint = EXCLUDED.domain_hint,
                    updated_at = NOW()
                "#,
            )
            .bind(&fingerprint)
            .bind(&gap.utterance)
            .bind(&gap.target_verb)
            .bind(format!(
                "loopback_calibration:{}:{}:{}:{}",
                gap.code,
                gap.entity_type,
                gap.entity_state,
                gap.actual_halt_reason.as_deref().unwrap_or("unknown")
            ))
            .execute(&self.pool)
            .await
            .context("upsert calibration Loop 1 learning candidate")?;
            summary.loop1_candidates_upserted += 1;
        }

        for clarification in clarifications {
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".user_learned_phrases (
                    user_id, phrase, verb, occurrence_count, confidence, source, created_at, updated_at
                )
                VALUES ($1, $2, $3, 1, 0.80, 'loopback_calibration', NOW(), NOW())
                ON CONFLICT (user_id, phrase) DO UPDATE SET
                    occurrence_count = "ob-poc".user_learned_phrases.occurrence_count + 1,
                    confidence = GREATEST("ob-poc".user_learned_phrases.confidence, 0.80),
                    verb = EXCLUDED.verb,
                    updated_at = NOW()
                "#,
            )
            .bind(Uuid::nil())
            .bind(&clarification.trigger_phrase)
            .bind(&clarification.verb_a)
            .execute(&self.pool)
            .await
            .context("upsert calibration Loop 2 learned phrase")?;
            summary.loop2_phrases_upserted += 1;

            for variant in phrase_variants(&clarification.trigger_phrase) {
                if variant == clarification.trigger_phrase {
                    continue;
                }
                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".user_learned_phrases (
                        user_id, phrase, verb, occurrence_count, confidence, source, created_at, updated_at
                    )
                    VALUES ($1, $2, $3, 1, 0.72, 'loopback_calibration_variant', NOW(), NOW())
                    ON CONFLICT (user_id, phrase) DO UPDATE SET
                        occurrence_count = "ob-poc".user_learned_phrases.occurrence_count + 1,
                        confidence = GREATEST("ob-poc".user_learned_phrases.confidence, 0.72),
                        updated_at = NOW()
                    "#,
                )
                .bind(Uuid::nil())
                .bind(&variant)
                .bind(&clarification.verb_a)
                .execute(&self.pool)
                .await
                .context("upsert calibration Loop 2 learned phrase variant")?;
                summary.loop2_phrases_upserted += 1;
            }

            if clarification.verb_b != clarification.verb_a {
                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".phrase_blocklist (
                        phrase, blocked_verb, reason, created_at
                    )
                    VALUES ($1, $2, $3, NOW())
                    ON CONFLICT (phrase, blocked_verb, COALESCE(user_id, '00000000-0000-0000-0000-000000000000'::uuid))
                    DO NOTHING
                    "#,
                )
                .bind(&clarification.trigger_phrase)
                .bind(&clarification.verb_b)
                .bind(format!(
                    "loopback_calibration prefers {} over {}",
                    clarification.verb_a, clarification.verb_b
                ))
                .execute(&self.pool)
                .await
                .context("upsert calibration phrase blocklist")?;
                summary.loop2_blocklist_upserts += 1;
            }
        }

        Ok(summary)
    }

    async fn decode_run_row(&self, row: sqlx::postgres::PgRow) -> Result<CalibrationRun> {
        let run_id = row.get("run_id");
        Ok(CalibrationRun {
            run_id,
            scenario_id: row.get("scenario_id"),
            triggered_by: row.get("triggered_by"),
            surface_versions: serde_json::from_value(
                row.get::<serde_json::Value, _>("surface_versions"),
            )
            .context("decode calibration run surface_versions")?,
            utterance_count: row.get::<i32, _>("utterance_count") as usize,
            positive_count: row.get::<i32, _>("positive_count") as usize,
            negative_count: row.get::<i32, _>("negative_count") as usize,
            boundary_count: row.get::<i32, _>("boundary_count") as usize,
            metrics: serde_json::from_value::<CalibrationMetrics>(
                row.get::<serde_json::Value, _>("metrics"),
            )
            .context("decode calibration run metrics")?,
            outcomes: self.list_outcomes(run_id).await?,
            prior_run_id: row.get("prior_run_id"),
            drift: row
                .get::<Option<serde_json::Value>, _>("drift")
                .map(serde_json::from_value::<CalibrationDrift>)
                .transpose()
                .context("decode calibration run drift")?,
            trace_ids: serde_json::from_value(row.get::<serde_json::Value, _>("trace_ids"))
                .context("decode calibration run trace_ids")?,
            run_start: row.get("run_start"),
            run_end: row.get("run_end"),
        })
    }

    async fn list_outcomes(&self, run_id: Uuid) -> Result<Vec<CalibrationOutcome>> {
        let rows = sqlx::query(
            r#"
            SELECT o.utterance_id, u.text, o.trace_id, o.calibration_mode, o.negative_type,
                   o.expected_outcome, o.verdict, o.actual_resolved_verb, o.actual_halt_reason,
                   o.failure_phase, o.failure_detail, o.top1_score, o.top2_score, o.margin,
                   o.margin_stable, o.latency_total_ms, o.latency_per_phase, u.pre_screen
            FROM "ob-poc".calibration_outcomes o
            JOIN "ob-poc".calibration_utterances u
              ON u.utterance_id = o.utterance_id
            WHERE o.run_id = $1
            ORDER BY o.created_at ASC
            "#,
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await
        .context("list calibration outcomes")?;

        rows.into_iter()
            .map(|row| {
                Ok(CalibrationOutcome {
                    utterance_id: row.get("utterance_id"),
                    utterance_text: row.get("text"),
                    calibration_mode: serde_json::from_str::<CalibrationMode>(
                        &row.get::<String, _>("calibration_mode"),
                    )
                    .context("decode outcome calibration_mode")?,
                    negative_type: row
                        .get::<Option<String>, _>("negative_type")
                        .map(|value| serde_json::from_str::<NegativeType>(&value))
                        .transpose()
                        .context("decode outcome negative_type")?,
                    pre_screen: row
                        .get::<Option<serde_json::Value>, _>("pre_screen")
                        .map(serde_json::from_value)
                        .transpose()
                        .context("decode outcome pre_screen")?,
                    expected_outcome: serde_json::from_value::<ExpectedOutcome>(
                        row.get::<serde_json::Value, _>("expected_outcome"),
                    )
                    .context("decode outcome expected_outcome")?,
                    trace_id: row.get("trace_id"),
                    actual_resolved_verb: row.get("actual_resolved_verb"),
                    actual_halt_reason: row.get("actual_halt_reason"),
                    verdict: serde_json::from_str::<CalibrationVerdict>(
                        &row.get::<String, _>("verdict"),
                    )
                    .context("decode outcome verdict")?,
                    failure_phase: row
                        .get::<Option<i16>, _>("failure_phase")
                        .map(|value| value as u8),
                    failure_detail: row.get("failure_detail"),
                    top1_score: row.get("top1_score"),
                    top2_score: row.get("top2_score"),
                    margin: row.get("margin"),
                    margin_stable: row.get("margin_stable"),
                    latency_total_ms: row.get::<Option<i32>, _>("latency_total_ms").map(i64::from),
                    latency_per_phase: row
                        .get::<Option<serde_json::Value>, _>("latency_per_phase")
                        .map(serde_json::from_value::<Vec<(u8, i64)>>)
                        .transpose()
                        .context("decode outcome latency_per_phase")?,
                })
            })
            .collect()
    }
}

fn phrase_variants(phrase: &str) -> Vec<String> {
    let lower = phrase.to_lowercase();
    let mut variants = vec![lower.clone()];
    let swaps = [
        ("show", "list"),
        ("list", "show"),
        ("get", "show"),
        ("open", "show"),
        ("find", "show"),
    ];
    for (from, to) in swaps {
        if lower.contains(from) {
            let swapped = lower.replace(from, to);
            if !variants.contains(&swapped) {
                variants.push(swapped);
            }
        }
    }
    if lower.contains("cbus") {
        let singular = lower.replace("cbus", "cbu");
        if !variants.contains(&singular) {
            variants.push(singular);
        }
    }
    variants
}
