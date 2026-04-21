//! Board domain verbs (1 plugin verb) — SemOS-side YAML-first
//! re-implementation of the plugin subset of
//! `rust/config/verbs/kyc/board.yaml`.
//!
//! `board.analyze-control` aggregates board appointments by
//! appointer, surfaces majority-controllers, joins appointment
//! rights, and classifies the control regime (single / joint /
//! diffuse). Every other verb in the domain is `behavior: crud`.

use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::json_get_required_uuid;
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

pub struct AnalyzeControl;

#[async_trait]
impl SemOsVerbOp for AnalyzeControl {
    fn fqn(&self) -> &str {
        "board.analyze-control"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_get_required_uuid(args, "entity-id")?;

        type BoardRow = (
            Uuid,
            Uuid,
            String,
            Uuid,
            String,
            Option<Uuid>,
            Option<String>,
            Option<chrono::NaiveDate>,
            bool,
        );

        let appointments: Vec<BoardRow> = sqlx::query_as(
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
            FROM "ob-poc".board_compositions bc
            JOIN "ob-poc".entities p ON bc.person_entity_id = p.entity_id
            JOIN "ob-poc".roles r ON bc.role_id = r.role_id
            LEFT JOIN "ob-poc".entities ap ON bc.appointed_by_entity_id = ap.entity_id
            WHERE bc.entity_id = $1
              AND p.deleted_at IS NULL
              AND (ap.entity_id IS NULL OR ap.deleted_at IS NULL)
              AND bc.is_active = true
              AND (bc.resignation_date IS NULL OR bc.resignation_date > CURRENT_DATE)
            ORDER BY bc.appointment_date
            "#,
        )
        .bind(entity_id)
        .fetch_all(scope.executor())
        .await?;

        let total_board_size = appointments.len();

        let mut appointer_counts: HashMap<String, (String, i32)> = HashMap::new();
        for (_, _, _, _, _, appointer_id, appointer_name, _, _) in &appointments {
            if let Some(appt_id) = appointer_id {
                let key = appt_id.to_string();
                let entry = appointer_counts
                    .entry(key)
                    .or_insert_with(|| (appointer_name.clone().unwrap_or_default(), 0));
                entry.1 += 1;
            }
        }

        let appointment_rights: Vec<(Uuid, String, String, Option<i32>)> = sqlx::query_as(
            r#"
            SELECT
                ar.holder_entity_id,
                e.name as holder_name,
                ar.right_type,
                ar.max_appointments
            FROM "ob-poc".appointment_rights ar
            JOIN "ob-poc".entities e ON ar.holder_entity_id = e.entity_id
            WHERE ar.target_entity_id = $1
              AND e.deleted_at IS NULL
              AND ar.is_active = true
              AND (ar.effective_to IS NULL OR ar.effective_to > CURRENT_DATE)
            "#,
        )
        .bind(entity_id)
        .fetch_all(scope.executor())
        .await?;

        let majority_threshold = if total_board_size > 0 {
            (total_board_size as f64 / 2.0).ceil() as i32
        } else {
            1
        };

        let mut controllers: Vec<Value> = Vec::new();
        for (appointer_id, (appointer_name, count)) in &appointer_counts {
            if *count >= majority_threshold {
                controllers.push(json!({
                    "controller_id": appointer_id,
                    "controller_name": appointer_name,
                    "appointments": count,
                    "has_majority": true,
                    "control_strength": if total_board_size > 0 {
                        (*count as f64) / (total_board_size as f64)
                    } else { 0.0 }
                }));
            }
        }

        let board_members: Vec<Value> = appointments
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
                        "appointed_at": appointed_at.map(|d| d.to_string()),
                    })
                },
            )
            .collect();

        let rights: Vec<Value> = appointment_rights
            .iter()
            .map(|(holder_id, holder_name, right_type, max_appts)| {
                json!({
                    "holder_id": holder_id.to_string(),
                    "holder_name": holder_name,
                    "right_type": right_type,
                    "max_appointments": max_appts,
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
                    "percentage": if total_board_size > 0 {
                        *c as f64 / total_board_size as f64 * 100.0
                    } else { 0.0 }
                })
            }).collect::<Vec<_>>(),
            "appointment_rights": rights,
            "controllers": controllers,
            "control_type": if controllers.len() == 1 { "single" }
                           else if controllers.len() > 1 { "joint" }
                           else { "diffuse" },
            "analysis_timestamp": chrono::Utc::now().to_rfc3339(),
        });

        Ok(VerbExecutionOutcome::Record(result))
    }
}
