//! Board Operations - Board composition and control analysis
//!
//! Plugin handlers for board.yaml verbs that require custom logic.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

use super::helpers::get_required_uuid;
use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

// ============================================================================
// BoardAnalyzeControlOp - Analyze who controls a board through appointments
// ============================================================================

/// Analyzes board composition to determine who controls an entity through
/// board appointments, removal rights, and veto powers.
pub struct BoardAnalyzeControlOp;

#[async_trait]
impl CustomOperation for BoardAnalyzeControlOp {
    fn domain(&self) -> &'static str {
        "board"
    }

    fn verb(&self) -> &'static str {
        "analyze-control"
    }

    fn rationale(&self) -> &'static str {
        "Board control analysis requires aggregating appointments by appointer, checking appointment rights, and determining majority control"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let entity_id = get_required_uuid(verb_call, "entity-id")?;

        // Get all current board compositions (using runtime query to avoid compile-time schema check)
        let appointments: Vec<(
            Uuid,
            Uuid,
            String,
            Uuid,
            String,
            Option<Uuid>,
            Option<String>,
            Option<chrono::NaiveDate>,
            bool,
        )> = sqlx::query_as(
            r#"
            SELECT
                bc.id,
                bc.person_entity_id,
                p.name as person_name,
                bc.role_id,
                r.name as role_name,
                bc.appointed_by_entity_id,
                ap.name as appointer_name,
                bc.appointment_date,
                bc.is_active
            FROM kyc.board_compositions bc
            JOIN "ob-poc".entities p ON bc.person_entity_id = p.entity_id
            JOIN "ob-poc".roles r ON bc.role_id = r.role_id
            LEFT JOIN "ob-poc".entities ap ON bc.appointed_by_entity_id = ap.entity_id
            WHERE bc.entity_id = $1
              AND bc.is_active = true
              AND (bc.resignation_date IS NULL OR bc.resignation_date > CURRENT_DATE)
            ORDER BY bc.appointment_date
            "#,
        )
        .bind(entity_id)
        .fetch_all(pool)
        .await?;

        let total_board_size = appointments.len();

        // Aggregate by appointer
        let mut appointer_counts: std::collections::HashMap<String, (String, i32)> =
            std::collections::HashMap::new();

        for (_, _, _, _, _, appointer_id, appointer_name, _, _) in &appointments {
            if let Some(appt_id) = appointer_id {
                let key = appt_id.to_string();
                let entry = appointer_counts
                    .entry(key)
                    .or_insert_with(|| (appointer_name.clone().unwrap_or_default(), 0));
                entry.1 += 1;
            }
        }

        // Check appointment rights
        let appointment_rights: Vec<(Uuid, String, String, Option<i32>)> = sqlx::query_as(
            r#"
            SELECT
                ar.holder_entity_id,
                e.name as holder_name,
                ar.right_type,
                ar.max_appointments
            FROM kyc.appointment_rights ar
            JOIN "ob-poc".entities e ON ar.holder_entity_id = e.entity_id
            WHERE ar.target_entity_id = $1
              AND ar.is_active = true
              AND (ar.effective_to IS NULL OR ar.effective_to > CURRENT_DATE)
            "#,
        )
        .bind(entity_id)
        .fetch_all(pool)
        .await?;

        // Determine who has majority control
        let majority_threshold = if total_board_size > 0 {
            (total_board_size as f64 / 2.0).ceil() as i32
        } else {
            1
        };
        let mut controllers: Vec<serde_json::Value> = Vec::new();

        for (appointer_id, (appointer_name, count)) in &appointer_counts {
            if *count >= majority_threshold {
                controllers.push(json!({
                    "controller_id": appointer_id,
                    "controller_name": appointer_name,
                    "appointments": count,
                    "has_majority": true,
                    "control_strength": if total_board_size > 0 { (*count as f64) / (total_board_size as f64) } else { 0.0 }
                }));
            }
        }

        // Build analysis result
        let board_members: Vec<serde_json::Value> = appointments
            .iter()
            .map(
                |(
                    id,
                    person_id,
                    person_name,
                    role_id,
                    role_name,
                    appointer_id,
                    appointer_name,
                    appointed_at,
                    _,
                )| {
                    json!({
                        "composition_id": id.to_string(),
                        "person_id": person_id.to_string(),
                        "person_name": person_name,
                        "role_id": role_id.to_string(),
                        "role_name": role_name,
                        "appointer_id": appointer_id.map(|id| id.to_string()),
                        "appointer_name": appointer_name,
                        "appointed_at": appointed_at.map(|d| d.to_string())
                    })
                },
            )
            .collect();

        let rights: Vec<serde_json::Value> = appointment_rights
            .iter()
            .map(|(holder_id, holder_name, right_type, max_appts)| {
                json!({
                    "holder_id": holder_id.to_string(),
                    "holder_name": holder_name,
                    "right_type": right_type,
                    "max_appointments": max_appts
                })
            })
            .collect();

        let result = json!({
            "entity_id": entity_id.to_string(),
            "board_size": total_board_size,
            "majority_threshold": majority_threshold,
            "board_members": board_members,
            "appointer_breakdown": appointer_counts.iter().map(|(k, (n, c))| {
                json!({
                    "appointer_id": k,
                    "appointer_name": n,
                    "appointments": c,
                    "percentage": if total_board_size > 0 { *c as f64 / total_board_size as f64 * 100.0 } else { 0.0 }
                })
            }).collect::<Vec<_>>(),
            "appointment_rights": rights,
            "controllers": controllers,
            "control_type": if controllers.len() == 1 { "single" }
                           else if controllers.len() > 1 { "joint" }
                           else { "diffuse" },
            "analysis_timestamp": chrono::Utc::now().to_rfc3339()
        });

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_board_analyze_control_metadata() {
        let op = BoardAnalyzeControlOp;
        assert_eq!(op.domain(), "board");
        assert_eq!(op.verb(), "analyze-control");
        assert!(!op.rationale().is_empty());
    }
}
