use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug)]
pub struct SeedState {
    pub prefix: String,
    pub client_group_id: Uuid,
    pub entity_id: Uuid,
    pub cbu_id: Uuid,
    pub deal_id: Uuid,
    pub case_id: Uuid,
    pub workstream_id: Uuid,
    pub doc_id: Uuid,
}

/// Connect to the configured test database.
///
/// # Examples
///
/// ```ignore
/// let pool = semtaxonomy_seed::get_pool().await?;
/// ```
pub async fn get_pool() -> Result<PgPool> {
    let url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string());
    Ok(PgPool::connect(&url).await?)
}

async fn ensure_entity_type_id(pool: &PgPool, type_code: &str) -> Result<Uuid> {
    if let Some(id) = sqlx::query_scalar(
        r#"
        SELECT entity_type_id
        FROM "ob-poc".entity_types
        WHERE type_code = $1
        LIMIT 1
        "#,
    )
    .bind(type_code)
    .fetch_optional(pool)
    .await?
    {
        return Ok(id);
    }

    let id = sqlx::query_scalar(
        r#"
        SELECT entity_type_id
        FROM "ob-poc".entity_types
        WHERE type_code IN ('LIMITED_COMPANY_PRIVATE', 'limited_company', 'LEGAL_ENTITY')
        ORDER BY CASE
            WHEN type_code = 'LIMITED_COMPANY_PRIVATE' THEN 0
            WHEN type_code = 'limited_company' THEN 1
            ELSE 2
        END
        LIMIT 1
        "#,
    )
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// Seed a minimal client-group/deal/onboarding/KYC/document/screening state.
///
/// # Examples
///
/// ```ignore
/// let state = semtaxonomy_seed::seed_state(&pool).await?;
/// ```
pub async fn seed_state(pool: &PgPool) -> Result<SeedState> {
    let prefix = format!("SeedCap-{}", &Uuid::new_v4().simple().to_string()[..8]);
    let client_group_id = Uuid::new_v4();
    let entity_id = Uuid::new_v4();
    let cbu_id = Uuid::new_v4();
    let deal_id = Uuid::new_v4();
    let case_id = Uuid::new_v4();
    let workstream_id = Uuid::new_v4();
    let doc_id = Uuid::new_v4();
    let entity_type_id = ensure_entity_type_id(pool, "LIMITED_COMPANY_PRIVATE").await?;

    sqlx::query(
        r#"
        INSERT INTO "ob-poc".client_group (id, canonical_name, discovery_status)
        VALUES ($1, $2, 'complete')
        "#,
    )
    .bind(client_group_id)
    .bind(format!("{prefix} Allianz"))
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO "ob-poc".client_group_alias (group_id, alias, alias_norm, source, is_primary)
        VALUES ($1, $2, LOWER($2), 'test', true)
        "#,
    )
    .bind(client_group_id)
    .bind(format!("{prefix} Allianz"))
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name, name_norm)
        VALUES ($1, $2, $3, LOWER($3))
        "#,
    )
    .bind(entity_id)
    .bind(entity_type_id)
    .bind(format!("{prefix} Management Ltd"))
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO "ob-poc".cbus (cbu_id, name, jurisdiction, client_type, commercial_client_entity_id, status)
        VALUES ($1, $2, 'LU', 'FUND', $3, 'DISCOVERED')
        "#,
    )
    .bind(cbu_id)
    .bind(format!("{prefix} SICAV"))
    .bind(entity_id)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO "ob-poc".client_group_entity (group_id, entity_id, membership_type, added_by, review_status, cbu_id)
        VALUES ($1, $2, 'confirmed', 'test', 'confirmed', $3)
        "#,
    )
    .bind(client_group_id)
    .bind(entity_id)
    .bind(cbu_id)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO "ob-poc".deals (deal_id, deal_name, deal_reference, primary_client_group_id, deal_status)
        VALUES ($1, $2, $3, $4, 'ONBOARDING')
        "#,
    )
    .bind(deal_id)
    .bind(format!("{prefix} Prime Services"))
    .bind(format!("{prefix}-D1"))
    .bind(client_group_id)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO "ob-poc".onboarding_requests (request_id, cbu_id, request_state, current_phase, created_by)
        VALUES ($1, $2, 'services_configured', 'kyc', 'test')
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(cbu_id)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO "ob-poc".cases (case_id, cbu_id, case_ref, client_group_id, deal_id, subject_entity_id, status, case_type)
        VALUES ($1, $2, $3, $4, $5, $6, 'DISCOVERY', 'NEW_CLIENT')
        "#,
    )
    .bind(case_id)
    .bind(cbu_id)
    .bind(format!("{prefix}-CASE"))
    .bind(client_group_id)
    .bind(deal_id)
    .bind(entity_id)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO "ob-poc".entity_workstreams (workstream_id, case_id, entity_id, status, blocker_type, blocker_message)
        VALUES ($1, $2, $3, 'SCREEN', 'SCREENING_HIT', 'Sanctions screening hit pending review')
        "#,
    )
    .bind(workstream_id)
    .bind(case_id)
    .bind(entity_id)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO "ob-poc".screenings (screening_id, workstream_id, screening_type, status, match_count)
        VALUES ($1, $2, 'SANCTIONS', 'HIT_PENDING_REVIEW', 1)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(workstream_id)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO "ob-poc".document_catalog (doc_id, document_name, document_type_code, status, cbu_id, entity_id)
        VALUES ($1, $2, 'PASSPORT', 'active', $3, $4)
        "#,
    )
    .bind(doc_id)
    .bind(format!("{prefix} Passport"))
    .bind(cbu_id)
    .bind(entity_id)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO "ob-poc".deal_documents (deal_id, document_id, document_type, document_status)
        VALUES ($1, $2, 'PASSPORT', 'UNDER_REVIEW')
        "#,
    )
    .bind(deal_id)
    .bind(doc_id)
    .execute(pool)
    .await?;

    Ok(SeedState {
        prefix,
        client_group_id,
        entity_id,
        cbu_id,
        deal_id,
        case_id,
        workstream_id,
        doc_id,
    })
}

/// Remove the seeded test state.
///
/// # Examples
///
/// ```ignore
/// semtaxonomy_seed::cleanup_state(&pool, &state).await;
/// ```
pub async fn cleanup_state(pool: &PgPool, state: &SeedState) {
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".deal_documents WHERE deal_id = $1"#)
        .bind(state.deal_id)
        .execute(pool)
        .await;
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".document_catalog WHERE doc_id = $1"#)
        .bind(state.doc_id)
        .execute(pool)
        .await;
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".screenings WHERE workstream_id = $1"#)
        .bind(state.workstream_id)
        .execute(pool)
        .await;
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".entity_workstreams WHERE workstream_id = $1"#)
        .bind(state.workstream_id)
        .execute(pool)
        .await;
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".cases WHERE case_id = $1"#)
        .bind(state.case_id)
        .execute(pool)
        .await;
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".onboarding_requests WHERE cbu_id = $1"#)
        .bind(state.cbu_id)
        .execute(pool)
        .await;
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".deals WHERE deal_id = $1"#)
        .bind(state.deal_id)
        .execute(pool)
        .await;
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".client_group_entity WHERE group_id = $1"#)
        .bind(state.client_group_id)
        .execute(pool)
        .await;
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".cbus WHERE cbu_id = $1"#)
        .bind(state.cbu_id)
        .execute(pool)
        .await;
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".entities WHERE entity_id = $1"#)
        .bind(state.entity_id)
        .execute(pool)
        .await;
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".client_group_alias WHERE group_id = $1"#)
        .bind(state.client_group_id)
        .execute(pool)
        .await;
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".client_group WHERE id = $1"#)
        .bind(state.client_group_id)
        .execute(pool)
        .await;
}
