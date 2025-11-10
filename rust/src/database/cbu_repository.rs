//! CBU repository: comprehensive CBU creation, management, and composition
//!
//! This repository handles all CBU-related operations including creation,
//! updates, document tracking, and entity role management.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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

    /// Create a new CBU with comprehensive information
    pub async fn create_cbu(&self, cbu_data: &CbuCreateData) -> Result<Uuid, sqlx::Error> {
        sqlx::query_scalar::<_, Uuid>(
            r#"INSERT INTO "ob-poc".cbus (
                   name, description, nature_purpose, source_of_funds,
                   customer_type, jurisdiction, channel, risk_rating, status
               ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'ACTIVE')
               RETURNING cbu_id"#,
        )
        .bind(&cbu_data.name)
        .bind(&cbu_data.description)
        .bind(&cbu_data.nature_purpose)
        .bind(&cbu_data.source_of_funds)
        .bind(&cbu_data.customer_type)
        .bind(&cbu_data.jurisdiction)
        .bind(&cbu_data.channel)
        .bind(&cbu_data.risk_rating)
        .fetch_one(&self.pool)
        .await
    }

    /// Create a simple CBU with just name and description (legacy method)
    pub async fn create_cbu_simple(
        &self,
        name: &str,
        description: &str,
    ) -> Result<Uuid, sqlx::Error> {
        sqlx::query_scalar::<_, Uuid>(
            r#"INSERT INTO "ob-poc".cbus (name, description)
               VALUES ($1, $2)
               ON CONFLICT (name) DO UPDATE SET description = EXCLUDED.description
               RETURNING cbu_id"#,
        )
        .bind(name)
        .bind(description)
        .fetch_one(&self.pool)
        .await
    }

    /// Get CBU by ID with comprehensive information
    pub async fn get_cbu(&self, cbu_id: Uuid) -> Result<Option<CbuData>, sqlx::Error> {
        let row = sqlx::query(
            r#"SELECT cbu_id, name, description, nature_purpose, source_of_funds,
                      customer_type, jurisdiction, channel, risk_rating, status,
                      created_at, updated_at
               FROM "ob-poc".cbus
               WHERE cbu_id = $1"#,
        )
        .bind(cbu_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(CbuData {
                cbu_id: row.get("cbu_id"),
                name: row.get("name"),
                description: row.get("description"),
                nature_purpose: row.get("nature_purpose"),
                source_of_funds: row.get("source_of_funds"),
                customer_type: row.get("customer_type"),
                jurisdiction: row.get("jurisdiction"),
                channel: row.get("channel"),
                risk_rating: row.get("risk_rating"),
                status: row.get("status"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            }))
        } else {
            Ok(None)
        }
    }

    /// Get CBU by name
    pub async fn get_cbu_by_name(&self, name: &str) -> Result<Option<CbuData>, sqlx::Error> {
        let row = sqlx::query(
            r#"SELECT cbu_id, name, description, nature_purpose, source_of_funds,
                      customer_type, jurisdiction, channel, risk_rating, status,
                      created_at, updated_at
               FROM "ob-poc".cbus
               WHERE name = $1"#,
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(CbuData {
                cbu_id: row.get("cbu_id"),
                name: row.get("name"),
                description: row.get("description"),
                nature_purpose: row.get("nature_purpose"),
                source_of_funds: row.get("source_of_funds"),
                customer_type: row.get("customer_type"),
                jurisdiction: row.get("jurisdiction"),
                channel: row.get("channel"),
                risk_rating: row.get("risk_rating"),
                status: row.get("status"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            }))
        } else {
            Ok(None)
        }
    }

    /// List all CBUs with optional filtering
    pub async fn list_cbus(
        &self,
        customer_type: Option<&str>,
        risk_rating: Option<&str>,
        limit: Option<i32>,
    ) -> Result<Vec<CbuData>, sqlx::Error> {
        let mut query = String::from(
            r#"SELECT cbu_id, name, description, nature_purpose, source_of_funds,
                      customer_type, jurisdiction, channel, risk_rating, status,
                      created_at, updated_at
               FROM "ob-poc".cbus
               WHERE status = 'ACTIVE'"#,
        );

        let mut bind_count = 0;
        if customer_type.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND customer_type = ${}", bind_count));
        }
        if risk_rating.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND risk_rating = ${}", bind_count));
        }

        query.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = limit {
            bind_count += 1;
            query.push_str(&format!(" LIMIT ${}", bind_count));
        }

        let mut db_query = sqlx::query(&query);

        if let Some(ct) = customer_type {
            db_query = db_query.bind(ct);
        }
        if let Some(rr) = risk_rating {
            db_query = db_query.bind(rr);
        }
        if let Some(l) = limit {
            db_query = db_query.bind(l);
        }

        let rows = db_query.fetch_all(&self.pool).await?;

        let mut cbus = Vec::with_capacity(rows.len());
        for row in rows {
            cbus.push(CbuData {
                cbu_id: row.get("cbu_id"),
                name: row.get("name"),
                description: row.get("description"),
                nature_purpose: row.get("nature_purpose"),
                source_of_funds: row.get("source_of_funds"),
                customer_type: row.get("customer_type"),
                jurisdiction: row.get("jurisdiction"),
                channel: row.get("channel"),
                risk_rating: row.get("risk_rating"),
                status: row.get("status"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            });
        }

        Ok(cbus)
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

/// Data structure for creating a new CBU
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuCreateData {
    pub name: String,
    pub description: Option<String>,
    pub nature_purpose: String,
    pub source_of_funds: String,
    pub customer_type: String,
    pub jurisdiction: String,
    pub channel: String,
    pub risk_rating: String,
}

/// Complete CBU data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuData {
    pub cbu_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub nature_purpose: Option<String>,
    pub source_of_funds: Option<String>,
    pub customer_type: Option<String>,
    pub jurisdiction: Option<String>,
    pub channel: Option<String>,
    pub risk_rating: Option<String>,
    pub status: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}
