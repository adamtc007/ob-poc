//! Agent plans and plan steps.
//!
//! An `AgentPlan` represents a structured sequence of actions the agent
//! intends to execute. Each plan contains ordered `PlanStep` entries that
//! pin verb snapshot IDs for provenance.
//!
//! Plans are INSERT-only (immutable). Status transitions are UPDATE-only
//! on the `status` column for progress tracking.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

// ── Plan Status ───────────────────────────────────────────────

/// Lifecycle status for an agent plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentPlanStatus {
    Draft,
    Active,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for AgentPlanStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        };
        write!(f, "{}", s)
    }
}

// ── Plan Step Status ──────────────────────────────────────────

/// Lifecycle status for a plan step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStepStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

impl std::fmt::Display for PlanStepStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        };
        write!(f, "{}", s)
    }
}

// ── Agent Plan ────────────────────────────────────────────────

/// A structured plan of actions the agent intends to execute.
///
/// Plans are created as `Draft`, activated when confirmed, and
/// transition to `Completed`/`Failed`/`Cancelled` as execution proceeds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPlan {
    /// Unique plan identifier.
    pub plan_id: Uuid,
    /// Case or subject this plan relates to.
    #[serde(default)]
    pub case_id: Option<Uuid>,
    /// High-level goal description.
    pub goal: String,
    /// Reference to the context resolution that informed this plan.
    #[serde(default)]
    pub context_resolution_ref: Option<serde_json::Value>,
    /// Ordered steps in this plan.
    pub steps: Vec<PlanStep>,
    /// Assumptions made when creating the plan.
    #[serde(default)]
    pub assumptions: Vec<String>,
    /// Identified risk flags.
    #[serde(default)]
    pub risk_flags: Vec<String>,
    /// Required security clearance level for executing this plan.
    #[serde(default)]
    pub security_clearance: Option<String>,
    /// Current plan status.
    pub status: AgentPlanStatus,
    /// Who created this plan.
    pub created_by: String,
    /// When this plan was created.
    pub created_at: DateTime<Utc>,
    /// When this plan was last updated.
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

// ── Plan Step ─────────────────────────────────────────────────

/// A single step in an agent plan.
///
/// Each step pins a `verb_snapshot_id` for provenance — the exact version
/// of the verb contract that was planned against.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    /// Unique step identifier.
    pub step_id: Uuid,
    /// Plan this step belongs to.
    pub plan_id: Uuid,
    /// Sequence number (0-based ordering).
    pub seq: i32,
    /// Verb object ID.
    pub verb_id: Uuid,
    /// Pinned verb snapshot ID (exact version planned against).
    pub verb_snapshot_id: Uuid,
    /// Verb FQN for display.
    pub verb_fqn: String,
    /// Parameters for this step.
    #[serde(default)]
    pub params: serde_json::Value,
    /// Expected postconditions after execution.
    #[serde(default)]
    pub expected_postconditions: Vec<String>,
    /// Fallback step IDs if this step fails.
    #[serde(default)]
    pub fallback_steps: Vec<Uuid>,
    /// Step IDs this step depends on (must complete first).
    #[serde(default)]
    pub depends_on_steps: Vec<Uuid>,
    /// Current step status.
    pub status: PlanStepStatus,
    /// Execution result (populated after execution).
    #[serde(default)]
    pub result: Option<serde_json::Value>,
    /// Error message (if failed).
    #[serde(default)]
    pub error: Option<String>,
}

// ── Plan Store ────────────────────────────────────────────────

/// Database operations for agent plans and steps.
pub struct PlanStore;

impl PlanStore {
    /// Insert a new agent plan (immutable INSERT).
    pub async fn insert_plan(pool: &PgPool, plan: &AgentPlan) -> Result<Uuid> {
        let steps_json = serde_json::to_value(&plan.steps)?;
        let assumptions_json = serde_json::to_value(&plan.assumptions)?;
        let risk_flags_json = serde_json::to_value(&plan.risk_flags)?;

        let plan_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO sem_reg.agent_plans (
                plan_id, case_id, goal, context_resolution_ref,
                steps, assumptions, risk_flags, security_clearance,
                status, created_by
            ) VALUES (
                $1, $2, $3, $4,
                $5, $6, $7, $8,
                $9, $10
            )
            RETURNING plan_id
            "#,
        )
        .bind(plan.plan_id)
        .bind(plan.case_id)
        .bind(&plan.goal)
        .bind(&plan.context_resolution_ref)
        .bind(&steps_json)
        .bind(&assumptions_json)
        .bind(&risk_flags_json)
        .bind(&plan.security_clearance)
        .bind(plan.status.to_string())
        .bind(&plan.created_by)
        .fetch_one(pool)
        .await?;

        Ok(plan_id)
    }

    /// Insert a plan step.
    pub async fn insert_step(pool: &PgPool, step: &PlanStep) -> Result<Uuid> {
        let params_json = &step.params;
        let postconditions_json = serde_json::to_value(&step.expected_postconditions)?;
        let fallback_json = serde_json::to_value(&step.fallback_steps)?;
        let depends_json = serde_json::to_value(&step.depends_on_steps)?;

        let step_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO sem_reg.plan_steps (
                step_id, plan_id, seq, verb_id, verb_snapshot_id,
                verb_fqn, params, expected_postconditions,
                fallback_steps, depends_on_steps, status
            ) VALUES (
                $1, $2, $3, $4, $5,
                $6, $7, $8,
                $9, $10, $11
            )
            RETURNING step_id
            "#,
        )
        .bind(step.step_id)
        .bind(step.plan_id)
        .bind(step.seq)
        .bind(step.verb_id)
        .bind(step.verb_snapshot_id)
        .bind(&step.verb_fqn)
        .bind(params_json)
        .bind(&postconditions_json)
        .bind(&fallback_json)
        .bind(&depends_json)
        .bind(step.status.to_string())
        .fetch_one(pool)
        .await?;

        Ok(step_id)
    }

    /// Update plan status.
    pub async fn update_plan_status(
        pool: &PgPool,
        plan_id: Uuid,
        status: AgentPlanStatus,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE sem_reg.agent_plans
            SET status = $2, updated_at = now()
            WHERE plan_id = $1
            "#,
        )
        .bind(plan_id)
        .bind(status.to_string())
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Update step status and optionally set result or error.
    pub async fn update_step_status(
        pool: &PgPool,
        step_id: Uuid,
        status: PlanStepStatus,
        result: Option<&serde_json::Value>,
        error: Option<&str>,
    ) -> Result<u64> {
        let rows = sqlx::query(
            r#"
            UPDATE sem_reg.plan_steps
            SET status = $2, result = $3, error = $4, updated_at = now()
            WHERE step_id = $1
            "#,
        )
        .bind(step_id)
        .bind(status.to_string())
        .bind(result)
        .bind(error)
        .execute(pool)
        .await?;
        Ok(rows.rows_affected())
    }

    /// Load a plan by ID with its steps.
    pub async fn load_plan(pool: &PgPool, plan_id: Uuid) -> Result<Option<AgentPlan>> {
        let row = sqlx::query_as::<_, PlanRow>(
            r#"
            SELECT plan_id, case_id, goal, context_resolution_ref,
                   steps, assumptions, risk_flags, security_clearance,
                   status, created_by, created_at, updated_at
            FROM sem_reg.agent_plans
            WHERE plan_id = $1
            "#,
        )
        .bind(plan_id)
        .fetch_optional(pool)
        .await?;

        match row {
            Some(r) => Ok(Some(r.into_plan()?)),
            None => Ok(None),
        }
    }

    /// List plans for a case, newest first.
    pub async fn list_plans_for_case(
        pool: &PgPool,
        case_id: Uuid,
        limit: i64,
    ) -> Result<Vec<AgentPlan>> {
        let rows = sqlx::query_as::<_, PlanRow>(
            r#"
            SELECT plan_id, case_id, goal, context_resolution_ref,
                   steps, assumptions, risk_flags, security_clearance,
                   status, created_by, created_at, updated_at
            FROM sem_reg.agent_plans
            WHERE case_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(case_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        rows.into_iter().map(|r| r.into_plan()).collect()
    }

    /// Load steps for a plan, ordered by sequence.
    pub async fn load_steps(pool: &PgPool, plan_id: Uuid) -> Result<Vec<PlanStep>> {
        let rows = sqlx::query_as::<_, StepRow>(
            r#"
            SELECT step_id, plan_id, seq, verb_id, verb_snapshot_id,
                   verb_fqn, params, expected_postconditions,
                   fallback_steps, depends_on_steps, status,
                   result, error
            FROM sem_reg.plan_steps
            WHERE plan_id = $1
            ORDER BY seq
            "#,
        )
        .bind(plan_id)
        .fetch_all(pool)
        .await?;

        rows.into_iter().map(|r| r.into_step()).collect()
    }
}

// ── Internal DB row types ─────────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
struct PlanRow {
    plan_id: Uuid,
    case_id: Option<Uuid>,
    goal: String,
    context_resolution_ref: Option<serde_json::Value>,
    steps: serde_json::Value,
    assumptions: serde_json::Value,
    risk_flags: serde_json::Value,
    security_clearance: Option<String>,
    status: String,
    created_by: String,
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
}

impl PlanRow {
    fn into_plan(self) -> Result<AgentPlan> {
        let steps: Vec<PlanStep> = serde_json::from_value(self.steps)?;
        let assumptions: Vec<String> = serde_json::from_value(self.assumptions)?;
        let risk_flags: Vec<String> = serde_json::from_value(self.risk_flags)?;
        let status = match self.status.as_str() {
            "draft" => AgentPlanStatus::Draft,
            "active" => AgentPlanStatus::Active,
            "completed" => AgentPlanStatus::Completed,
            "failed" => AgentPlanStatus::Failed,
            "cancelled" => AgentPlanStatus::Cancelled,
            _ => AgentPlanStatus::Draft,
        };

        Ok(AgentPlan {
            plan_id: self.plan_id,
            case_id: self.case_id,
            goal: self.goal,
            context_resolution_ref: self.context_resolution_ref,
            steps,
            assumptions,
            risk_flags,
            security_clearance: self.security_clearance,
            status,
            created_by: self.created_by,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
struct StepRow {
    step_id: Uuid,
    plan_id: Uuid,
    seq: i32,
    verb_id: Uuid,
    verb_snapshot_id: Uuid,
    verb_fqn: String,
    params: serde_json::Value,
    expected_postconditions: serde_json::Value,
    fallback_steps: serde_json::Value,
    depends_on_steps: serde_json::Value,
    status: String,
    result: Option<serde_json::Value>,
    error: Option<String>,
}

impl StepRow {
    fn into_step(self) -> Result<PlanStep> {
        let expected_postconditions: Vec<String> =
            serde_json::from_value(self.expected_postconditions)?;
        let fallback_steps: Vec<Uuid> = serde_json::from_value(self.fallback_steps)?;
        let depends_on_steps: Vec<Uuid> = serde_json::from_value(self.depends_on_steps)?;
        let status = match self.status.as_str() {
            "pending" => PlanStepStatus::Pending,
            "running" => PlanStepStatus::Running,
            "completed" => PlanStepStatus::Completed,
            "failed" => PlanStepStatus::Failed,
            "skipped" => PlanStepStatus::Skipped,
            _ => PlanStepStatus::Pending,
        };

        Ok(PlanStep {
            step_id: self.step_id,
            plan_id: self.plan_id,
            seq: self.seq,
            verb_id: self.verb_id,
            verb_snapshot_id: self.verb_snapshot_id,
            verb_fqn: self.verb_fqn,
            params: self.params,
            expected_postconditions,
            fallback_steps,
            depends_on_steps,
            status,
            result: self.result,
            error: self.error,
        })
    }
}

// ── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_status_display() {
        assert_eq!(AgentPlanStatus::Draft.to_string(), "draft");
        assert_eq!(AgentPlanStatus::Active.to_string(), "active");
        assert_eq!(AgentPlanStatus::Completed.to_string(), "completed");
        assert_eq!(AgentPlanStatus::Failed.to_string(), "failed");
        assert_eq!(AgentPlanStatus::Cancelled.to_string(), "cancelled");
    }

    #[test]
    fn test_step_status_display() {
        assert_eq!(PlanStepStatus::Pending.to_string(), "pending");
        assert_eq!(PlanStepStatus::Running.to_string(), "running");
        assert_eq!(PlanStepStatus::Completed.to_string(), "completed");
        assert_eq!(PlanStepStatus::Failed.to_string(), "failed");
        assert_eq!(PlanStepStatus::Skipped.to_string(), "skipped");
    }

    #[test]
    fn test_plan_serde_roundtrip() {
        let plan = AgentPlan {
            plan_id: Uuid::new_v4(),
            case_id: Some(Uuid::new_v4()),
            goal: "Discover UBO structure for Allianz".into(),
            context_resolution_ref: Some(serde_json::json!({"resolution_id": "abc"})),
            steps: vec![PlanStep {
                step_id: Uuid::new_v4(),
                plan_id: Uuid::new_v4(),
                seq: 0,
                verb_id: Uuid::new_v4(),
                verb_snapshot_id: Uuid::new_v4(),
                verb_fqn: "ubo.discover".into(),
                params: serde_json::json!({"entity_id": "uuid-123"}),
                expected_postconditions: vec!["ubo_graph_populated".into()],
                fallback_steps: vec![],
                depends_on_steps: vec![],
                status: PlanStepStatus::Pending,
                result: None,
                error: None,
            }],
            assumptions: vec!["Entity exists in registry".into()],
            risk_flags: vec!["complex_corporate_structure".into()],
            security_clearance: Some("confidential".into()),
            status: AgentPlanStatus::Draft,
            created_by: "agent-1".into(),
            created_at: Utc::now(),
            updated_at: None,
        };

        let json = serde_json::to_value(&plan).unwrap();
        let round: AgentPlan = serde_json::from_value(json).unwrap();
        assert_eq!(round.plan_id, plan.plan_id);
        assert_eq!(round.goal, "Discover UBO structure for Allianz");
        assert_eq!(round.steps.len(), 1);
        assert_eq!(round.steps[0].verb_fqn, "ubo.discover");
        assert_eq!(round.status, AgentPlanStatus::Draft);
    }

    #[test]
    fn test_step_with_dependencies() {
        let step1_id = Uuid::new_v4();
        let step2 = PlanStep {
            step_id: Uuid::new_v4(),
            plan_id: Uuid::new_v4(),
            seq: 1,
            verb_id: Uuid::new_v4(),
            verb_snapshot_id: Uuid::new_v4(),
            verb_fqn: "kyc.open-case".into(),
            params: serde_json::json!({}),
            expected_postconditions: vec!["case_opened".into()],
            fallback_steps: vec![],
            depends_on_steps: vec![step1_id],
            status: PlanStepStatus::Pending,
            result: None,
            error: None,
        };

        let json = serde_json::to_value(&step2).unwrap();
        let round: PlanStep = serde_json::from_value(json).unwrap();
        assert_eq!(round.depends_on_steps.len(), 1);
        assert_eq!(round.depends_on_steps[0], step1_id);
    }

    #[test]
    fn test_plan_row_status_parsing() {
        let row = PlanRow {
            plan_id: Uuid::new_v4(),
            case_id: None,
            goal: "Test".into(),
            context_resolution_ref: None,
            steps: serde_json::json!([]),
            assumptions: serde_json::json!([]),
            risk_flags: serde_json::json!([]),
            security_clearance: None,
            status: "active".into(),
            created_by: "test".into(),
            created_at: Utc::now(),
            updated_at: None,
        };
        let plan = row.into_plan().unwrap();
        assert_eq!(plan.status, AgentPlanStatus::Active);
    }
}
