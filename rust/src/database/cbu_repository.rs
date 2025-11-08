//! CBU composition repository: link CBUs to entities by role and list participants

use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Clone)]
pub struct CbuRepository {
    pool: PgPool,
}

impl CbuRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn create_role(&self, name: &str, description: &str) -> Result<Uuid, sqlx::Error> {
        sqlx::query_scalar::<_, Uuid>(
            r#"INSERT INTO "ob-poc".roles (name, description)
               VALUES ($1, $2)
               ON CONFLICT (name) DO UPDATE SET description = EXCLUDED.description
               RETURNING role_id"#,
        )
        .bind(name)
        .bind(description)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn link_cbu_entity_role(
        &self,
        cbu_id: Uuid,
        entity_id: Uuid,
        role_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"INSERT INTO "ob-poc".cbu_entity_roles (cbu_entity_role_id, cbu_id, entity_id, role_id, created_at)
               VALUES (gen_random_uuid(), $1, $2, $3, now())
               ON CONFLICT (cbu_id, entity_id, role_id) DO NOTHING"#,
        )
        .bind(cbu_id)
        .bind(entity_id)
        .bind(role_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Composite view of CBU participants by role with resolved entity type and display name
    pub async fn list_cbu_entities_with_roles(
        &self,
        cbu_id: Uuid,
    ) -> Result<Vec<CbuParticipant>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT r.name AS role_name,
                   et.name AS entity_type,
                   COALESCE(lc.company_name,
                            p.partnership_name,
                            pp.last_name || ', ' || pp.first_name) AS entity_name,
                   e.entity_id
            FROM "ob-poc".cbu_entity_roles cer
              JOIN "ob-poc".roles r ON r.role_id = cer.role_id
              JOIN "ob-poc".entities e ON e.entity_id = cer.entity_id
              JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
              LEFT JOIN "ob-poc".entity_limited_companies lc ON lc.limited_company_id = e.external_id::uuid
              LEFT JOIN "ob-poc".entity_partnerships p ON p.partnership_id = e.external_id::uuid
              LEFT JOIN "ob-poc".entity_proper_persons pp ON pp.proper_person_id = e.external_id::uuid
            WHERE cer.cbu_id = $1
            ORDER BY r.name
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(CbuParticipant {
                role_name: row.get("role_name"),
                entity_type: row.get("entity_type"),
                entity_name: row.get("entity_name"),
                entity_id: row.get("entity_id"),
            });
        }
        Ok(out)
    }
}

#[derive(Debug, Clone)]
pub struct CbuParticipant {
    pub role_name: String,
    pub entity_type: String,
    pub entity_name: String,
    pub entity_id: Uuid,
}
