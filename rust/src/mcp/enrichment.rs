//! Entity Enrichment for Disambiguation
//!
//! Fetches rich context for entities to help agents make confident resolution decisions.
//! Context includes roles, relationships, dates, and other distinguishing details.

use serde::Serialize;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

/// Entity type for enrichment queries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityType {
    ProperPerson,
    LegalEntity,
    Cbu,
}

impl EntityType {
    /// Parse from string (case-insensitive)
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "person" | "proper_person" | "properperson" => Some(Self::ProperPerson),
            "company" | "legal_entity" | "legalentity" | "limited_company" => {
                Some(Self::LegalEntity)
            }
            "cbu" => Some(Self::Cbu),
            "entity" => Some(Self::ProperPerson), // Default to person for generic entity
            _ => None,
        }
    }
}

/// Enriches entities with contextual information for disambiguation
pub struct EntityEnricher {
    pool: PgPool,
}

impl EntityEnricher {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Enrich a batch of entity IDs with context
    pub async fn enrich(
        &self,
        entity_type: EntityType,
        ids: &[Uuid],
    ) -> Result<HashMap<Uuid, EntityContext>, sqlx::Error> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }

        match entity_type {
            EntityType::ProperPerson => self.enrich_persons(ids).await,
            EntityType::LegalEntity => self.enrich_legal_entities(ids).await,
            EntityType::Cbu => self.enrich_cbus(ids).await,
        }
    }

    /// Enrich proper persons with nationality, DOB, roles
    async fn enrich_persons(
        &self,
        ids: &[Uuid],
    ) -> Result<HashMap<Uuid, EntityContext>, sqlx::Error> {
        let mut results = HashMap::new();

        // Fetch basic person info
        let rows = sqlx::query!(
            r#"
            SELECT
                e.entity_id as "entity_id!: Uuid",
                pp.nationality,
                pp.date_of_birth,
                e.created_at,
                e.updated_at
            FROM "ob-poc".entities e
            LEFT JOIN "ob-poc".entity_proper_persons pp ON e.entity_id = pp.entity_id
            WHERE e.entity_id = ANY($1)
            "#,
            ids
        )
        .fetch_all(&self.pool)
        .await?;

        for row in rows {
            results.insert(
                row.entity_id,
                EntityContext {
                    nationality: row.nationality.clone(),
                    date_of_birth: row.date_of_birth.map(|d| d.to_string()),
                    jurisdiction: None,
                    registration_number: None,
                    roles: Vec::new(),
                    ownership: Vec::new(),
                    created_at: row.created_at.map(|t| t.to_string()).unwrap_or_default(),
                    last_activity: row.updated_at.map(|t| t.to_string()).unwrap_or_default(),
                },
            );
        }

        // Fetch roles for all entities
        let roles = sqlx::query!(
            r#"
            SELECT
                cer.entity_id as "entity_id!: Uuid",
                r.name as role_name,
                c.name as cbu_name,
                cer.created_at as since
            FROM "ob-poc".cbu_entity_roles cer
            JOIN "ob-poc".roles r ON cer.role_id = r.role_id
            JOIN "ob-poc".cbus c ON cer.cbu_id = c.cbu_id
            WHERE cer.entity_id = ANY($1)
            ORDER BY cer.created_at DESC
            "#,
            ids
        )
        .fetch_all(&self.pool)
        .await?;

        for role_row in roles {
            if let Some(ctx) = results.get_mut(&role_row.entity_id) {
                ctx.roles.push(RoleContext {
                    role: role_row.role_name,
                    cbu_name: role_row.cbu_name,
                    since: role_row.since.map(|t| t.to_string()),
                });
            }
        }

        // Fetch ownership relationships
        let ownership = sqlx::query!(
            r#"
            SELECT
                r.from_entity_id as "entity_id!: Uuid",
                e.name as owned_name,
                r.percentage as ownership_percent,
                r.ownership_type
            FROM "ob-poc".entity_relationships r
            JOIN "ob-poc".entities e ON r.to_entity_id = e.entity_id
            WHERE r.from_entity_id = ANY($1)
            AND r.relationship_type = 'ownership'
            AND (r.effective_to IS NULL OR r.effective_to > CURRENT_DATE)
            "#,
            ids
        )
        .fetch_all(&self.pool)
        .await?;

        for own_row in ownership {
            if let Some(ctx) = results.get_mut(&own_row.entity_id) {
                ctx.ownership.push(OwnershipContext {
                    owned_name: own_row.owned_name,
                    percentage: own_row
                        .ownership_percent
                        .map(|p| p.to_string().parse().unwrap_or(0.0))
                        .unwrap_or(0.0),
                    ownership_type: own_row.ownership_type.clone(),
                });
            }
        }

        Ok(results)
    }

    /// Enrich legal entities with jurisdiction, registration, roles
    async fn enrich_legal_entities(
        &self,
        ids: &[Uuid],
    ) -> Result<HashMap<Uuid, EntityContext>, sqlx::Error> {
        let mut results = HashMap::new();

        // Fetch basic company info
        let rows = sqlx::query!(
            r#"
            SELECT
                e.entity_id as "entity_id!: Uuid",
                lc.jurisdiction,
                lc.registration_number,
                e.created_at,
                e.updated_at
            FROM "ob-poc".entities e
            LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
            WHERE e.entity_id = ANY($1)
            "#,
            ids
        )
        .fetch_all(&self.pool)
        .await?;

        for row in rows {
            results.insert(
                row.entity_id,
                EntityContext {
                    nationality: None,
                    date_of_birth: None,
                    jurisdiction: row.jurisdiction.clone(),
                    registration_number: row.registration_number.clone(),
                    roles: Vec::new(),
                    ownership: Vec::new(),
                    created_at: row.created_at.map(|t| t.to_string()).unwrap_or_default(),
                    last_activity: row.updated_at.map(|t| t.to_string()).unwrap_or_default(),
                },
            );
        }

        // Fetch roles (same as persons)
        let roles = sqlx::query!(
            r#"
            SELECT
                cer.entity_id as "entity_id!: Uuid",
                r.name as role_name,
                c.name as cbu_name,
                cer.created_at as since
            FROM "ob-poc".cbu_entity_roles cer
            JOIN "ob-poc".roles r ON cer.role_id = r.role_id
            JOIN "ob-poc".cbus c ON cer.cbu_id = c.cbu_id
            WHERE cer.entity_id = ANY($1)
            ORDER BY cer.created_at DESC
            "#,
            ids
        )
        .fetch_all(&self.pool)
        .await?;

        for role_row in roles {
            if let Some(ctx) = results.get_mut(&role_row.entity_id) {
                ctx.roles.push(RoleContext {
                    role: role_row.role_name,
                    cbu_name: role_row.cbu_name,
                    since: role_row.since.map(|t| t.to_string()),
                });
            }
        }

        Ok(results)
    }

    /// Enrich CBUs with jurisdiction, type, entity/role counts
    async fn enrich_cbus(&self, ids: &[Uuid]) -> Result<HashMap<Uuid, EntityContext>, sqlx::Error> {
        let mut results = HashMap::new();

        let rows = sqlx::query!(
            r#"
            SELECT
                c.cbu_id as "cbu_id!: Uuid",
                c.jurisdiction,
                c.client_type,
                c.created_at,
                c.updated_at,
                (SELECT COUNT(*) FROM "ob-poc".cbu_entity_roles cer WHERE cer.cbu_id = c.cbu_id)::int as role_count
            FROM "ob-poc".cbus c
            WHERE c.cbu_id = ANY($1)
            "#,
            ids
        )
        .fetch_all(&self.pool)
        .await?;

        for row in rows {
            let cbu_id = row.cbu_id;
            let role_count = row.role_count.unwrap_or(0);

            results.insert(
                cbu_id,
                EntityContext {
                    nationality: None,
                    date_of_birth: None,
                    jurisdiction: row.jurisdiction.clone(),
                    registration_number: row.client_type.clone(), // Reuse field for client_type
                    roles: vec![RoleContext {
                        role: format!("{} entities", role_count),
                        cbu_name: String::new(),
                        since: None,
                    }],
                    ownership: Vec::new(),
                    created_at: row.created_at.map(|t| t.to_string()).unwrap_or_default(),
                    last_activity: row.updated_at.map(|t| t.to_string()).unwrap_or_default(),
                },
            );
        }

        Ok(results)
    }
}

/// Rich context for entity disambiguation
#[derive(Debug, Clone, Default, Serialize)]
pub struct EntityContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nationality: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_of_birth: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jurisdiction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration_number: Option<String>,
    pub roles: Vec<RoleContext>,
    pub ownership: Vec<OwnershipContext>,
    pub created_at: String,
    pub last_activity: String,
}

/// Role context for a specific CBU
#[derive(Debug, Clone, Serialize)]
pub struct RoleContext {
    pub role: String,
    pub cbu_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
}

/// Ownership context
#[derive(Debug, Clone, Serialize)]
pub struct OwnershipContext {
    pub owned_name: String,
    pub percentage: f64,
    pub ownership_type: String,
}

impl EntityContext {
    /// Build human-readable disambiguation label
    pub fn disambiguation_label(&self, display: &str, entity_type: EntityType) -> String {
        match entity_type {
            EntityType::ProperPerson => {
                let mut parts = vec![display.to_string()];

                // Add nationality if available
                if let Some(nat) = &self.nationality {
                    parts.push(format!("({})", nat));
                }

                // Add birth year if available
                if let Some(dob) = &self.date_of_birth {
                    if let Some(year) = dob.split('-').next() {
                        parts.push(format!("b.{}", year));
                    }
                }

                // Add roles (first 2)
                if !self.roles.is_empty() {
                    let role_str = self
                        .roles
                        .iter()
                        .take(2)
                        .filter(|r| !r.cbu_name.is_empty())
                        .map(|r| format!("{} at {}", r.role, r.cbu_name))
                        .collect::<Vec<_>>()
                        .join(", ");
                    if !role_str.is_empty() {
                        parts.push(format!("- {}", role_str));
                    } else {
                        parts.push("- No current roles".to_string());
                    }
                } else {
                    parts.push("- No current roles".to_string());
                }

                parts.join(" ")
            }
            EntityType::LegalEntity => {
                let mut parts = vec![display.to_string()];

                if let Some(j) = &self.jurisdiction {
                    parts.push(format!("({})", j));
                }

                if let Some(reg) = &self.registration_number {
                    parts.push(format!("#{}", reg));
                }

                if !self.roles.is_empty() {
                    parts.push(format!("- {} roles", self.roles.len()));
                }

                parts.join(" ")
            }
            EntityType::Cbu => {
                let mut parts = vec![display.to_string()];

                if let Some(j) = &self.jurisdiction {
                    parts.push(format!("({})", j));
                }

                // client_type is stored in registration_number field for CBUs
                if let Some(ct) = &self.registration_number {
                    parts.push(format!("[{}]", ct));
                }

                parts.join(" ")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_type_from_str() {
        assert_eq!(EntityType::parse("person"), Some(EntityType::ProperPerson));
        assert_eq!(EntityType::parse("PERSON"), Some(EntityType::ProperPerson));
        assert_eq!(EntityType::parse("company"), Some(EntityType::LegalEntity));
        assert_eq!(
            EntityType::parse("legal_entity"),
            Some(EntityType::LegalEntity)
        );
        assert_eq!(EntityType::parse("cbu"), Some(EntityType::Cbu));
        assert_eq!(EntityType::parse("unknown"), None);
    }

    #[test]
    fn test_disambiguation_label_person() {
        let ctx = EntityContext {
            nationality: Some("US".to_string()),
            date_of_birth: Some("1975-03-15".to_string()),
            roles: vec![RoleContext {
                role: "Director".to_string(),
                cbu_name: "Apex Fund".to_string(),
                since: None,
            }],
            ..Default::default()
        };

        let label = ctx.disambiguation_label("John Smith", EntityType::ProperPerson);
        assert!(label.contains("John Smith"));
        assert!(label.contains("US"));
        assert!(label.contains("b.1975"));
        assert!(label.contains("Director at Apex Fund"));
    }

    #[test]
    fn test_disambiguation_label_company() {
        let ctx = EntityContext {
            jurisdiction: Some("LU".to_string()),
            registration_number: Some("B123456".to_string()),
            roles: vec![RoleContext {
                role: "Shareholder".to_string(),
                cbu_name: "Fund".to_string(),
                since: None,
            }],
            ..Default::default()
        };

        let label = ctx.disambiguation_label("Holdings Ltd", EntityType::LegalEntity);
        assert!(label.contains("Holdings Ltd"));
        assert!(label.contains("LU"));
        assert!(label.contains("#B123456"));
        assert!(label.contains("1 roles"));
    }

    #[test]
    fn test_disambiguation_label_no_roles() {
        let ctx = EntityContext {
            nationality: Some("GB".to_string()),
            date_of_birth: Some("1982-11-20".to_string()),
            roles: vec![],
            ..Default::default()
        };

        let label = ctx.disambiguation_label("John Smith", EntityType::ProperPerson);
        assert!(label.contains("No current roles"));
    }
}
