use std::collections::HashMap;

use anyhow::{Context, Result};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::ast::{FieldValue, OverlayRow, ScopeData, SlotOverlayData};

fn is_missing_relation_error(error: &sqlx::Error, relation_name: &str) -> bool {
    let relation = format!("\"ob-poc\".{}", relation_name);
    matches!(error, sqlx::Error::Database(db_error)
        if db_error.code().as_deref() == Some("42P01")
            && (db_error.message().contains(&relation)
                || db_error.message().contains(relation_name)))
}

/// Fetch overlay data for a single slot using direct SQL queries.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool) -> anyhow::Result<()> {
/// use uuid::Uuid;
/// use ob_poc::state_reducer::fetch_slot_overlays;
///
/// let _ = fetch_slot_overlays(pool, Uuid::new_v4(), Uuid::new_v4(), None).await?;
/// # Ok(())
/// # }
/// ```
pub async fn fetch_slot_overlays(
    pool: &PgPool,
    cbu_id: Uuid,
    entity_id: Uuid,
    case_id: Option<Uuid>,
) -> Result<SlotOverlayData> {
    match fetch_slot_overlays_inner(pool, cbu_id, entity_id, case_id).await {
        Ok(data) => Ok(data),
        Err(e) => {
            tracing::warn!(error = %e, %cbu_id, %entity_id, "Overlay fetch failed — returning empty overlays");
            Ok(SlotOverlayData {
                sources: HashMap::new(),
                scope: ScopeData { fields: serde_json::json!({}) },
                slots: Vec::new(),
            })
        }
    }
}

async fn fetch_slot_overlays_inner(
    pool: &PgPool,
    cbu_id: Uuid,
    entity_id: Uuid,
    case_id: Option<Uuid>,
) -> Result<SlotOverlayData> {
    let mut data = SlotOverlayData {
        sources: HashMap::new(),
        scope: ScopeData {
            fields: serde_json::json!({}),
        },
        slots: Vec::new(),
    };

    let entity_roles = sqlx::query_as::<_, (Uuid, String)>(
        r#"
        SELECT cer.entity_id, r.name
        FROM "ob-poc".cbu_entity_roles cer
        JOIN "ob-poc".roles r ON r.role_id = cer.role_id
        WHERE cer.cbu_id = $1
          AND cer.entity_id = $2
        "#,
    )
    .bind(cbu_id)
    .bind(entity_id)
    .fetch_all(pool)
    .await
    .unwrap_or_else(|e| {
        tracing::warn!(error = %e, %cbu_id, %entity_id, "Failed to fetch entity role overlays — continuing with empty overlays");
        Vec::new()
    });

    if !entity_roles.is_empty() {
        data.sources.insert(
            "entity_ref".into(),
            entity_roles
                .into_iter()
                .map(|(entity_id, role_name)| OverlayRow {
                    fields: HashMap::from([
                        ("entity_id".into(), FieldValue::Str(entity_id.to_string())),
                        ("role".into(), FieldValue::Str(role_name)),
                    ]),
                })
                .collect(),
        );
    }

    if let Some(case_id) = case_id {
        let workstreams = sqlx::query_as::<_, (Uuid, String, Option<String>)>(
            r#"
            SELECT w.workstream_id, w.status, w.risk_rating
            FROM "ob-poc".entity_workstreams w
            WHERE w.case_id = $1
              AND w.entity_id = $2
            ORDER BY w.created_at DESC
            "#,
        )
        .bind(case_id)
        .bind(entity_id)
        .fetch_all(pool)
        .await
        .or_else(|err| {
            if is_missing_relation_error(&err, "entity_workstreams") {
                Ok(Vec::new())
            } else {
                Err(err)
            }
        })
        .context("failed to fetch workstream overlays")?;

        if !workstreams.is_empty() {
            data.sources.insert(
                "workstream".into(),
                workstreams
                    .iter()
                    .map(|(workstream_id, status, risk_rating)| OverlayRow {
                        fields: HashMap::from([
                            (
                                "workstream_id".into(),
                                FieldValue::Str(workstream_id.to_string()),
                            ),
                            ("status".into(), FieldValue::Str(status.clone())),
                            (
                                "risk_rating".into(),
                                risk_rating
                                    .as_ref()
                                    .map(|value| FieldValue::Str(value.clone()))
                                    .unwrap_or(FieldValue::Null),
                            ),
                        ]),
                    })
                    .collect(),
            );

            let workstream_ids: Vec<Uuid> = workstreams.iter().map(|(id, _, _)| *id).collect();
            fetch_screenings(pool, &mut data, &workstream_ids).await?;
            fetch_doc_requests(pool, &mut data, &workstream_ids).await?;
        }

        fetch_red_flags(pool, &mut data, case_id).await?;
    }

    fetch_evidence(pool, &mut data, entity_id).await?;

    Ok(data)
}

pub(crate) async fn fetch_slot_overlays_tx(
    tx: &mut Transaction<'_, Postgres>,
    cbu_id: Uuid,
    entity_id: Uuid,
    case_id: Option<Uuid>,
) -> Result<SlotOverlayData> {
    match fetch_slot_overlays_tx_inner(tx, cbu_id, entity_id, case_id).await {
        Ok(data) => Ok(data),
        Err(e) => {
            tracing::warn!(error = %e, %cbu_id, %entity_id, "Overlay fetch (tx) failed — returning empty overlays");
            Ok(SlotOverlayData {
                sources: HashMap::new(),
                scope: ScopeData { fields: serde_json::json!({}) },
                slots: Vec::new(),
            })
        }
    }
}

async fn fetch_slot_overlays_tx_inner(
    tx: &mut Transaction<'_, Postgres>,
    cbu_id: Uuid,
    entity_id: Uuid,
    case_id: Option<Uuid>,
) -> Result<SlotOverlayData> {
    let mut data = SlotOverlayData {
        sources: HashMap::new(),
        scope: ScopeData {
            fields: serde_json::json!({}),
        },
        slots: Vec::new(),
    };

    let entity_roles = sqlx::query_as::<_, (Uuid, String)>(
        r#"
        SELECT cer.entity_id, r.name
        FROM "ob-poc".cbu_entity_roles cer
        JOIN "ob-poc".roles r ON r.role_id = cer.role_id
        WHERE cer.cbu_id = $1
          AND cer.entity_id = $2
        "#,
    )
    .bind(cbu_id)
    .bind(entity_id)
    .fetch_all(&mut **tx)
    .await
    .unwrap_or_else(|e| {
        tracing::warn!(error = %e, %cbu_id, %entity_id, "Failed to fetch entity role overlays (tx) — continuing with empty overlays");
        Vec::new()
    });

    if !entity_roles.is_empty() {
        data.sources.insert(
            "entity_ref".into(),
            entity_roles
                .into_iter()
                .map(|(entity_id, role_name)| OverlayRow {
                    fields: HashMap::from([
                        ("entity_id".into(), FieldValue::Str(entity_id.to_string())),
                        ("role".into(), FieldValue::Str(role_name)),
                    ]),
                })
                .collect(),
        );
    }

    if let Some(case_id) = case_id {
        let workstreams = sqlx::query_as::<_, (Uuid, String, Option<String>)>(
            r#"
            SELECT w.workstream_id, w.status, w.risk_rating
            FROM "ob-poc".entity_workstreams w
            WHERE w.case_id = $1
              AND w.entity_id = $2
            ORDER BY w.created_at DESC
            "#,
        )
        .bind(case_id)
        .bind(entity_id)
        .fetch_all(&mut **tx)
        .await
        .or_else(|err| {
            if is_missing_relation_error(&err, "entity_workstreams") {
                Ok(Vec::new())
            } else {
                Err(err)
            }
        })
        .context("failed to fetch workstream overlays")?;

        if !workstreams.is_empty() {
            data.sources.insert(
                "workstream".into(),
                workstreams
                    .iter()
                    .map(|(workstream_id, status, risk_rating)| OverlayRow {
                        fields: HashMap::from([
                            (
                                "workstream_id".into(),
                                FieldValue::Str(workstream_id.to_string()),
                            ),
                            ("status".into(), FieldValue::Str(status.clone())),
                            (
                                "risk_rating".into(),
                                risk_rating
                                    .as_ref()
                                    .map(|value| FieldValue::Str(value.clone()))
                                    .unwrap_or(FieldValue::Null),
                            ),
                        ]),
                    })
                    .collect(),
            );

            let workstream_ids: Vec<Uuid> = workstreams.iter().map(|(id, _, _)| *id).collect();
            fetch_screenings_tx(tx, &mut data, &workstream_ids).await?;
            fetch_doc_requests_tx(tx, &mut data, &workstream_ids).await?;
        }

        fetch_red_flags_tx(tx, &mut data, case_id).await?;
    }

    fetch_evidence_tx(tx, &mut data, entity_id).await?;

    Ok(data)
}

async fn fetch_screenings(
    pool: &PgPool,
    data: &mut SlotOverlayData,
    workstream_ids: &[Uuid],
) -> Result<()> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>)>(
        r#"
        SELECT s.screening_type, s.status, s.result_summary
        FROM "ob-poc".screenings s
        WHERE s.workstream_id = ANY($1)
        "#,
    )
    .bind(workstream_ids)
    .fetch_all(pool)
    .await
    .or_else(|err| {
        if is_missing_relation_error(&err, "screenings") {
            Ok(Vec::new())
        } else {
            Err(err)
        }
    })
    .context("failed to fetch screening overlays")?;

    if !rows.is_empty() {
        data.sources.insert(
            "screenings".into(),
            rows.into_iter()
                .map(|(screening_type, status, result_summary)| OverlayRow {
                    fields: HashMap::from([
                        ("screening_type".into(), FieldValue::Str(screening_type)),
                        ("status".into(), FieldValue::Str(status)),
                        (
                            "result_summary".into(),
                            result_summary
                                .map(FieldValue::Str)
                                .unwrap_or(FieldValue::Null),
                        ),
                    ]),
                })
                .collect(),
        );
    }

    Ok(())
}

async fn fetch_evidence(pool: &PgPool, data: &mut SlotOverlayData, entity_id: Uuid) -> Result<()> {
    let rows = sqlx::query_as::<_, (String, String, Option<chrono::DateTime<chrono::Utc>>)>(
        r#"
        SELECT ue.evidence_type, ue.status, ue.verified_at
        FROM "ob-poc".kyc_ubo_evidence ue
        JOIN "ob-poc".ubo_registry ur ON ur.ubo_id = ue.ubo_id
        WHERE ur.subject_entity_id = $1
        "#,
    )
    .bind(entity_id)
    .fetch_all(pool)
    .await
    .or_else(|err| {
        if is_missing_relation_error(&err, "kyc_ubo_evidence")
            || is_missing_relation_error(&err, "ubo_registry")
        {
            Ok(Vec::new())
        } else {
            Err(err)
        }
    })
    .context("failed to fetch evidence overlays")?;

    if !rows.is_empty() {
        data.sources.insert(
            "evidence".into(),
            rows.into_iter()
                .map(|(evidence_type, status, verified_at)| OverlayRow {
                    fields: HashMap::from([
                        ("evidence_type".into(), FieldValue::Str(evidence_type)),
                        ("status".into(), FieldValue::Str(status)),
                        (
                            "verified_at".into(),
                            verified_at
                                .map(|ts| FieldValue::Str(ts.to_rfc3339()))
                                .unwrap_or(FieldValue::Null),
                        ),
                    ]),
                })
                .collect(),
        );
    }

    Ok(())
}

async fn fetch_red_flags(pool: &PgPool, data: &mut SlotOverlayData, case_id: Uuid) -> Result<()> {
    let rows = sqlx::query_as::<_, (String, String, String)>(
        r#"
        SELECT flag_type, severity, status
        FROM "ob-poc".red_flags
        WHERE case_id = $1
        "#,
    )
    .bind(case_id)
    .fetch_all(pool)
    .await
    .or_else(|err| {
        if is_missing_relation_error(&err, "red_flags") {
            Ok(Vec::new())
        } else {
            Err(err)
        }
    })
    .context("failed to fetch red flag overlays")?;

    if !rows.is_empty() {
        data.sources.insert(
            "red_flags".into(),
            rows.into_iter()
                .map(|(flag_type, severity, status)| OverlayRow {
                    fields: HashMap::from([
                        ("flag_type".into(), FieldValue::Str(flag_type)),
                        ("severity".into(), FieldValue::Str(severity)),
                        ("status".into(), FieldValue::Str(status)),
                    ]),
                })
                .collect(),
        );
    }

    Ok(())
}

async fn fetch_doc_requests(
    pool: &PgPool,
    data: &mut SlotOverlayData,
    workstream_ids: &[Uuid],
) -> Result<()> {
    let rows = sqlx::query_as::<_, (String, String)>(
        r#"
        SELECT dr.request_type, dr.status
        FROM "ob-poc".doc_requests dr
        WHERE dr.workstream_id = ANY($1)
        "#,
    )
    .bind(workstream_ids)
    .fetch_all(pool)
    .await
    .or_else(|err| {
        if is_missing_relation_error(&err, "doc_requests") {
            Ok(Vec::new())
        } else {
            Err(err)
        }
    })
    .context("failed to fetch document request overlays")?;

    if !rows.is_empty() {
        data.sources.insert(
            "doc_requests".into(),
            rows.into_iter()
                .map(|(request_type, status)| OverlayRow {
                    fields: HashMap::from([
                        ("request_type".into(), FieldValue::Str(request_type)),
                        ("status".into(), FieldValue::Str(status)),
                    ]),
                })
                .collect(),
        );
    }

    Ok(())
}

async fn fetch_screenings_tx(
    tx: &mut Transaction<'_, Postgres>,
    data: &mut SlotOverlayData,
    workstream_ids: &[Uuid],
) -> Result<()> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>)>(
        r#"
        SELECT s.screening_type, s.status, s.result_summary
        FROM "ob-poc".screenings s
        WHERE s.workstream_id = ANY($1)
        "#,
    )
    .bind(workstream_ids)
    .fetch_all(&mut **tx)
    .await
    .or_else(|err| {
        if is_missing_relation_error(&err, "screenings") {
            Ok(Vec::new())
        } else {
            Err(err)
        }
    })
    .context("failed to fetch screening overlays")?;

    if !rows.is_empty() {
        data.sources.insert(
            "screenings".into(),
            rows.into_iter()
                .map(|(screening_type, status, result_summary)| OverlayRow {
                    fields: HashMap::from([
                        ("screening_type".into(), FieldValue::Str(screening_type)),
                        ("status".into(), FieldValue::Str(status)),
                        (
                            "result_summary".into(),
                            result_summary
                                .map(FieldValue::Str)
                                .unwrap_or(FieldValue::Null),
                        ),
                    ]),
                })
                .collect(),
        );
    }

    Ok(())
}

async fn fetch_evidence_tx(
    tx: &mut Transaction<'_, Postgres>,
    data: &mut SlotOverlayData,
    entity_id: Uuid,
) -> Result<()> {
    let rows = sqlx::query_as::<_, (String, String, Option<chrono::DateTime<chrono::Utc>>)>(
        r#"
        SELECT ue.evidence_type, ue.status, ue.verified_at
        FROM "ob-poc".kyc_ubo_evidence ue
        JOIN "ob-poc".ubo_registry ur ON ur.ubo_id = ue.ubo_id
        WHERE ur.subject_entity_id = $1
        "#,
    )
    .bind(entity_id)
    .fetch_all(&mut **tx)
    .await
    .or_else(|err| {
        if is_missing_relation_error(&err, "kyc_ubo_evidence")
            || is_missing_relation_error(&err, "ubo_registry")
        {
            Ok(Vec::new())
        } else {
            Err(err)
        }
    })
    .context("failed to fetch evidence overlays")?;

    if !rows.is_empty() {
        data.sources.insert(
            "evidence".into(),
            rows.into_iter()
                .map(|(evidence_type, status, verified_at)| OverlayRow {
                    fields: HashMap::from([
                        ("evidence_type".into(), FieldValue::Str(evidence_type)),
                        ("status".into(), FieldValue::Str(status)),
                        (
                            "verified_at".into(),
                            verified_at
                                .map(|ts| FieldValue::Str(ts.to_rfc3339()))
                                .unwrap_or(FieldValue::Null),
                        ),
                    ]),
                })
                .collect(),
        );
    }

    Ok(())
}

async fn fetch_red_flags_tx(
    tx: &mut Transaction<'_, Postgres>,
    data: &mut SlotOverlayData,
    case_id: Uuid,
) -> Result<()> {
    let rows = sqlx::query_as::<_, (String, String, String)>(
        r#"
        SELECT flag_type, severity, status
        FROM "ob-poc".red_flags
        WHERE case_id = $1
        "#,
    )
    .bind(case_id)
    .fetch_all(&mut **tx)
    .await
    .or_else(|err| {
        if is_missing_relation_error(&err, "red_flags") {
            Ok(Vec::new())
        } else {
            Err(err)
        }
    })
    .context("failed to fetch red flag overlays")?;

    if !rows.is_empty() {
        data.sources.insert(
            "red_flags".into(),
            rows.into_iter()
                .map(|(flag_type, severity, status)| OverlayRow {
                    fields: HashMap::from([
                        ("flag_type".into(), FieldValue::Str(flag_type)),
                        ("severity".into(), FieldValue::Str(severity)),
                        ("status".into(), FieldValue::Str(status)),
                    ]),
                })
                .collect(),
        );
    }

    Ok(())
}

async fn fetch_doc_requests_tx(
    tx: &mut Transaction<'_, Postgres>,
    data: &mut SlotOverlayData,
    workstream_ids: &[Uuid],
) -> Result<()> {
    let rows = sqlx::query_as::<_, (String, String)>(
        r#"
        SELECT dr.request_type, dr.status
        FROM "ob-poc".doc_requests dr
        WHERE dr.workstream_id = ANY($1)
        "#,
    )
    .bind(workstream_ids)
    .fetch_all(&mut **tx)
    .await
    .or_else(|err| {
        if is_missing_relation_error(&err, "doc_requests") {
            Ok(Vec::new())
        } else {
            Err(err)
        }
    })
    .context("failed to fetch document request overlays")?;

    if !rows.is_empty() {
        data.sources.insert(
            "doc_requests".into(),
            rows.into_iter()
                .map(|(request_type, status)| OverlayRow {
                    fields: HashMap::from([
                        ("request_type".into(), FieldValue::Str(request_type)),
                        ("status".into(), FieldValue::Str(status)),
                    ]),
                })
                .collect(),
        );
    }

    Ok(())
}
