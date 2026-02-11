//! Booking Principal Repository — Database access layer
//!
//! Provides all database queries for the booking principal selection capability.
//! Covers CRUD for core entities, rule gathering, evaluation persistence,
//! and coverage view reads.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::booking_principal_types::*;

/// Repository for booking principal database operations
pub struct BookingPrincipalRepository;

// ============================================================================
// Private FromRow types — converted to public API types via Into
// ============================================================================

#[derive(sqlx::FromRow)]
struct LegalEntityRow {
    legal_entity_id: Uuid,
    lei: Option<String>,
    name: String,
    incorporation_jurisdiction: String,
    status: String,
    entity_id: Option<Uuid>,
    metadata: Option<serde_json::Value>,
    created_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
}

impl From<LegalEntityRow> for LegalEntity {
    fn from(r: LegalEntityRow) -> Self {
        Self {
            legal_entity_id: r.legal_entity_id,
            lei: r.lei,
            name: r.name,
            incorporation_jurisdiction: r.incorporation_jurisdiction,
            status: r.status,
            entity_id: r.entity_id,
            metadata: r.metadata,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct BookingLocationRow {
    booking_location_id: Uuid,
    country_code: String,
    region_code: Option<String>,
    regulatory_regime_tags: Option<Vec<String>>,
    jurisdiction_code: Option<String>,
    metadata: Option<serde_json::Value>,
    created_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
}

impl From<BookingLocationRow> for BookingLocation {
    fn from(r: BookingLocationRow) -> Self {
        Self {
            booking_location_id: r.booking_location_id,
            country_code: r.country_code,
            region_code: r.region_code,
            regulatory_regime_tags: r.regulatory_regime_tags.unwrap_or_default(),
            jurisdiction_code: r.jurisdiction_code,
            metadata: r.metadata,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct BookingPrincipalRow {
    booking_principal_id: Uuid,
    legal_entity_id: Uuid,
    booking_location_id: Option<Uuid>,
    principal_code: String,
    book_code: Option<String>,
    status: String,
    effective_from: DateTime<Utc>,
    effective_to: Option<DateTime<Utc>>,
    metadata: Option<serde_json::Value>,
    created_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
}

impl From<BookingPrincipalRow> for BookingPrincipal {
    fn from(r: BookingPrincipalRow) -> Self {
        Self {
            booking_principal_id: r.booking_principal_id,
            legal_entity_id: r.legal_entity_id,
            booking_location_id: r.booking_location_id,
            principal_code: r.principal_code,
            book_code: r.book_code,
            status: r.status,
            effective_from: r.effective_from,
            effective_to: r.effective_to,
            metadata: r.metadata,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct RulesetRow {
    ruleset_id: Uuid,
    owner_type: String,
    owner_id: Option<Uuid>,
    name: String,
    ruleset_boundary: String,
    version: i32,
    effective_from: DateTime<Utc>,
    effective_to: Option<DateTime<Utc>>,
    status: String,
    metadata: Option<serde_json::Value>,
    created_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
}

impl From<RulesetRow> for Ruleset {
    fn from(r: RulesetRow) -> Self {
        Self {
            ruleset_id: r.ruleset_id,
            owner_type: r.owner_type,
            owner_id: r.owner_id,
            name: r.name,
            ruleset_boundary: r.ruleset_boundary,
            version: r.version,
            effective_from: r.effective_from,
            effective_to: r.effective_to,
            status: r.status,
            metadata: r.metadata,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct RuleRow {
    rule_id: Uuid,
    ruleset_id: Uuid,
    name: String,
    kind: String,
    when_expr: serde_json::Value,
    then_effect: serde_json::Value,
    explain: Option<String>,
    priority: i32,
    metadata: Option<serde_json::Value>,
    created_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
}

impl From<RuleRow> for Rule {
    fn from(r: RuleRow) -> Self {
        Self {
            rule_id: r.rule_id,
            ruleset_id: r.ruleset_id,
            name: r.name,
            kind: r.kind,
            when_expr: r.when_expr,
            then_effect: r.then_effect,
            explain: r.explain,
            priority: r.priority,
            metadata: r.metadata,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct ServiceAvailabilityRow {
    service_availability_id: Uuid,
    booking_principal_id: Uuid,
    service_id: Uuid,
    regulatory_status: String,
    regulatory_constraints: Option<serde_json::Value>,
    commercial_status: String,
    commercial_constraints: Option<serde_json::Value>,
    operational_status: String,
    delivery_model: Option<String>,
    operational_constraints: Option<serde_json::Value>,
    effective_from: DateTime<Utc>,
    effective_to: Option<DateTime<Utc>>,
    metadata: Option<serde_json::Value>,
    created_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
}

impl From<ServiceAvailabilityRow> for ServiceAvailabilityRecord {
    fn from(r: ServiceAvailabilityRow) -> Self {
        Self {
            service_availability_id: r.service_availability_id,
            booking_principal_id: r.booking_principal_id,
            service_id: r.service_id,
            regulatory_status: r.regulatory_status,
            regulatory_constraints: r.regulatory_constraints,
            commercial_status: r.commercial_status,
            commercial_constraints: r.commercial_constraints,
            operational_status: r.operational_status,
            delivery_model: r.delivery_model,
            operational_constraints: r.operational_constraints,
            effective_from: r.effective_from,
            effective_to: r.effective_to,
            metadata: r.metadata,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct ClientPrincipalRelationshipRow {
    client_principal_relationship_id: Uuid,
    client_group_id: Uuid,
    booking_principal_id: Uuid,
    product_offering_id: Uuid,
    relationship_status: String,
    contract_ref: Option<String>,
    onboarded_at: Option<DateTime<Utc>>,
    effective_from: DateTime<Utc>,
    effective_to: Option<DateTime<Utc>>,
    metadata: Option<serde_json::Value>,
    created_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
}

impl From<ClientPrincipalRelationshipRow> for ClientPrincipalRelationship {
    fn from(r: ClientPrincipalRelationshipRow) -> Self {
        Self {
            client_principal_relationship_id: r.client_principal_relationship_id,
            client_group_id: r.client_group_id,
            booking_principal_id: r.booking_principal_id,
            product_offering_id: r.product_offering_id,
            relationship_status: r.relationship_status,
            contract_ref: r.contract_ref,
            onboarded_at: r.onboarded_at,
            effective_from: r.effective_from,
            effective_to: r.effective_to,
            metadata: r.metadata,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct GapRow {
    offering_code: Option<String>,
    jurisdiction: String,
    principal_code: Option<String>,
    gap_type: String,
    detail: String,
    delivery_model: Option<String>,
}

impl From<GapRow> for GapReportEntry {
    fn from(r: GapRow) -> Self {
        Self {
            offering_code: r.offering_code,
            jurisdiction: r.jurisdiction,
            principal_code: r.principal_code,
            gap_type: r.gap_type,
            detail: r.detail,
            delivery_model: r.delivery_model,
        }
    }
}

impl BookingPrincipalRepository {
    // ========================================================================
    // Legal Entity
    // ========================================================================

    pub async fn insert_legal_entity(
        pool: &PgPool,
        name: &str,
        incorporation_jurisdiction: &str,
        lei: Option<&str>,
        entity_id: Option<Uuid>,
    ) -> Result<Uuid> {
        let row = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO "ob-poc".legal_entity (name, incorporation_jurisdiction, lei, entity_id)
            VALUES ($1, $2, $3, $4)
            RETURNING legal_entity_id
            "#,
        )
        .bind(name)
        .bind(incorporation_jurisdiction)
        .bind(lei)
        .bind(entity_id)
        .fetch_one(pool)
        .await
        .context("Failed to insert legal entity")?;

        Ok(row)
    }

    pub async fn get_legal_entity(
        pool: &PgPool,
        legal_entity_id: Uuid,
    ) -> Result<Option<LegalEntity>> {
        let row = sqlx::query_as::<_, LegalEntityRow>(
            r#"SELECT * FROM "ob-poc".legal_entity WHERE legal_entity_id = $1"#,
        )
        .bind(legal_entity_id)
        .fetch_optional(pool)
        .await
        .context("Failed to fetch legal entity")?;

        Ok(row.map(Into::into))
    }

    pub async fn list_legal_entities(pool: &PgPool) -> Result<Vec<LegalEntity>> {
        let rows = sqlx::query_as::<_, LegalEntityRow>(
            r#"SELECT * FROM "ob-poc".legal_entity WHERE status = 'active' ORDER BY name"#,
        )
        .fetch_all(pool)
        .await
        .context("Failed to list legal entities")?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    // ========================================================================
    // Booking Location
    // ========================================================================

    pub async fn insert_booking_location(
        pool: &PgPool,
        country_code: &str,
        region_code: Option<&str>,
        regulatory_regime_tags: &[String],
        jurisdiction_code: Option<&str>,
    ) -> Result<Uuid> {
        let row = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO "ob-poc".booking_location
                (country_code, region_code, regulatory_regime_tags, jurisdiction_code)
            VALUES ($1, $2, $3, $4)
            RETURNING booking_location_id
            "#,
        )
        .bind(country_code)
        .bind(region_code)
        .bind(regulatory_regime_tags)
        .bind(jurisdiction_code)
        .fetch_one(pool)
        .await
        .context("Failed to insert booking location")?;

        Ok(row)
    }

    pub async fn get_booking_location(
        pool: &PgPool,
        booking_location_id: Uuid,
    ) -> Result<Option<BookingLocation>> {
        let row = sqlx::query_as::<_, BookingLocationRow>(
            r#"SELECT * FROM "ob-poc".booking_location WHERE booking_location_id = $1"#,
        )
        .bind(booking_location_id)
        .fetch_optional(pool)
        .await
        .context("Failed to fetch booking location")?;

        Ok(row.map(Into::into))
    }

    // ========================================================================
    // Booking Principal
    // ========================================================================

    pub async fn insert_booking_principal(
        pool: &PgPool,
        legal_entity_id: Uuid,
        booking_location_id: Option<Uuid>,
        principal_code: &str,
        book_code: Option<&str>,
    ) -> Result<Uuid> {
        let row = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO "ob-poc".booking_principal
                (legal_entity_id, booking_location_id, principal_code, book_code)
            VALUES ($1, $2, $3, $4)
            RETURNING booking_principal_id
            "#,
        )
        .bind(legal_entity_id)
        .bind(booking_location_id)
        .bind(principal_code)
        .bind(book_code)
        .fetch_one(pool)
        .await
        .context("Failed to insert booking principal")?;

        Ok(row)
    }

    pub async fn get_booking_principal(
        pool: &PgPool,
        booking_principal_id: Uuid,
    ) -> Result<Option<BookingPrincipal>> {
        let row = sqlx::query_as::<_, BookingPrincipalRow>(
            r#"SELECT * FROM "ob-poc".booking_principal WHERE booking_principal_id = $1"#,
        )
        .bind(booking_principal_id)
        .fetch_optional(pool)
        .await
        .context("Failed to fetch booking principal")?;

        Ok(row.map(Into::into))
    }

    /// List all active booking principals (with legal entity name for display)
    pub async fn list_active_principals(pool: &PgPool) -> Result<Vec<BookingPrincipal>> {
        let rows = sqlx::query_as::<_, BookingPrincipalRow>(
            r#"
            SELECT bp.*
            FROM "ob-poc".booking_principal bp
            WHERE bp.status = 'active'
              AND (bp.effective_to IS NULL OR bp.effective_to > now())
            ORDER BY bp.principal_code
            "#,
        )
        .fetch_all(pool)
        .await
        .context("Failed to list active principals")?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn retire_booking_principal(
        pool: &PgPool,
        booking_principal_id: Uuid,
    ) -> Result<i64> {
        // Count active relationships first (returned for caller to check)
        let count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM "ob-poc".client_principal_relationship
            WHERE booking_principal_id = $1 AND relationship_status = 'active'
            "#,
        )
        .bind(booking_principal_id)
        .fetch_one(pool)
        .await
        .context("Failed to count active relationships")?;

        sqlx::query(
            r#"
            UPDATE "ob-poc".booking_principal
            SET status = 'inactive', effective_to = now(), updated_at = now()
            WHERE booking_principal_id = $1
            "#,
        )
        .bind(booking_principal_id)
        .execute(pool)
        .await
        .context("Failed to retire booking principal")?;

        Ok(count)
    }

    // ========================================================================
    // Client Profile + Classifications (snapshot)
    // ========================================================================

    pub async fn insert_client_profile(
        pool: &PgPool,
        client_group_id: Uuid,
        segment: &str,
        domicile_country: &str,
        entity_types: &[String],
        risk_flags: Option<&serde_json::Value>,
    ) -> Result<Uuid> {
        let row = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO "ob-poc".client_profile
                (client_group_id, segment, domicile_country, entity_types, risk_flags)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING client_profile_id
            "#,
        )
        .bind(client_group_id)
        .bind(segment)
        .bind(domicile_country)
        .bind(entity_types)
        .bind(risk_flags)
        .fetch_one(pool)
        .await
        .context("Failed to insert client profile")?;

        Ok(row)
    }

    pub async fn insert_client_classification(
        pool: &PgPool,
        client_profile_id: Uuid,
        classification_scheme: &str,
        classification_value: &str,
        jurisdiction_scope: Option<&str>,
        source: Option<&str>,
    ) -> Result<Uuid> {
        let row = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO "ob-poc".client_classification
                (client_profile_id, classification_scheme, classification_value,
                 jurisdiction_scope, source)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING client_classification_id
            "#,
        )
        .bind(client_profile_id)
        .bind(classification_scheme)
        .bind(classification_value)
        .bind(jurisdiction_scope)
        .bind(source)
        .fetch_one(pool)
        .await
        .context("Failed to insert client classification")?;

        Ok(row)
    }

    pub async fn get_classifications_for_profile(
        pool: &PgPool,
        client_profile_id: Uuid,
    ) -> Result<Vec<ClientClassification>> {
        #[derive(sqlx::FromRow)]
        struct Row {
            client_classification_id: Uuid,
            client_profile_id: Uuid,
            classification_scheme: String,
            classification_value: String,
            jurisdiction_scope: Option<String>,
            effective_from: Option<DateTime<Utc>>,
            effective_to: Option<DateTime<Utc>>,
            source: Option<String>,
            metadata: Option<serde_json::Value>,
            created_at: Option<DateTime<Utc>>,
        }

        let rows = sqlx::query_as::<_, Row>(
            r#"
            SELECT * FROM "ob-poc".client_classification
            WHERE client_profile_id = $1
            ORDER BY classification_scheme
            "#,
        )
        .bind(client_profile_id)
        .fetch_all(pool)
        .await
        .context("Failed to fetch classifications")?;

        Ok(rows
            .into_iter()
            .map(|r| ClientClassification {
                client_classification_id: r.client_classification_id,
                client_profile_id: r.client_profile_id,
                classification_scheme: r.classification_scheme,
                classification_value: r.classification_value,
                jurisdiction_scope: r.jurisdiction_scope,
                effective_from: r.effective_from,
                effective_to: r.effective_to,
                source: r.source,
                metadata: r.metadata,
                created_at: r.created_at,
            })
            .collect())
    }

    // ========================================================================
    // Service Availability
    // ========================================================================

    #[allow(clippy::too_many_arguments)]
    pub async fn set_service_availability(
        pool: &PgPool,
        booking_principal_id: Uuid,
        service_id: Uuid,
        regulatory_status: &str,
        regulatory_constraints: Option<&serde_json::Value>,
        commercial_status: &str,
        commercial_constraints: Option<&serde_json::Value>,
        operational_status: &str,
        delivery_model: Option<&str>,
        operational_constraints: Option<&serde_json::Value>,
        effective_from: Option<DateTime<Utc>>,
        effective_to: Option<DateTime<Utc>>,
    ) -> Result<Uuid> {
        let eff_from = effective_from.unwrap_or_else(Utc::now);

        let row = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO "ob-poc".service_availability
                (booking_principal_id, service_id,
                 regulatory_status, regulatory_constraints,
                 commercial_status, commercial_constraints,
                 operational_status, delivery_model, operational_constraints,
                 effective_from, effective_to)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING service_availability_id
            "#,
        )
        .bind(booking_principal_id)
        .bind(service_id)
        .bind(regulatory_status)
        .bind(regulatory_constraints)
        .bind(commercial_status)
        .bind(commercial_constraints)
        .bind(operational_status)
        .bind(delivery_model)
        .bind(operational_constraints)
        .bind(eff_from)
        .bind(effective_to)
        .fetch_one(pool)
        .await
        .context("Failed to set service availability")?;

        Ok(row)
    }

    /// Get active service availability for a principal
    pub async fn list_availability_for_principal(
        pool: &PgPool,
        booking_principal_id: Uuid,
    ) -> Result<Vec<ServiceAvailabilityRecord>> {
        let rows = sqlx::query_as::<_, ServiceAvailabilityRow>(
            r#"
            SELECT * FROM "ob-poc".service_availability
            WHERE booking_principal_id = $1
              AND now() BETWEEN effective_from AND COALESCE(effective_to, 'infinity'::timestamptz)
            ORDER BY service_id
            "#,
        )
        .bind(booking_principal_id)
        .fetch_all(pool)
        .await
        .context("Failed to list service availability")?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Get availability for a principal x service (for delivery checking)
    pub async fn get_availability(
        pool: &PgPool,
        booking_principal_id: Uuid,
        service_id: Uuid,
    ) -> Result<Option<ServiceAvailabilityRecord>> {
        let row = sqlx::query_as::<_, ServiceAvailabilityRow>(
            r#"
            SELECT * FROM "ob-poc".service_availability
            WHERE booking_principal_id = $1
              AND service_id = $2
              AND now() BETWEEN effective_from AND COALESCE(effective_to, 'infinity'::timestamptz)
            "#,
        )
        .bind(booking_principal_id)
        .bind(service_id)
        .fetch_optional(pool)
        .await
        .context("Failed to fetch service availability")?;

        Ok(row.map(Into::into))
    }

    // ========================================================================
    // Client-Principal Relationship
    // ========================================================================

    pub async fn record_relationship(
        pool: &PgPool,
        client_group_id: Uuid,
        booking_principal_id: Uuid,
        product_offering_id: Uuid,
        contract_ref: Option<&str>,
    ) -> Result<Uuid> {
        let row = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO "ob-poc".client_principal_relationship
                (client_group_id, booking_principal_id, product_offering_id,
                 contract_ref, onboarded_at)
            VALUES ($1, $2, $3, $4, now())
            RETURNING client_principal_relationship_id
            "#,
        )
        .bind(client_group_id)
        .bind(booking_principal_id)
        .bind(product_offering_id)
        .bind(contract_ref)
        .fetch_one(pool)
        .await
        .context("Failed to record relationship")?;

        Ok(row)
    }

    pub async fn terminate_relationship(pool: &PgPool, relationship_id: Uuid) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".client_principal_relationship
            SET relationship_status = 'terminated',
                effective_to = now(),
                updated_at = now()
            WHERE client_principal_relationship_id = $1
              AND relationship_status = 'active'
            "#,
        )
        .bind(relationship_id)
        .execute(pool)
        .await
        .context("Failed to terminate relationship")?;

        Ok(result.rows_affected())
    }

    pub async fn list_relationships(
        pool: &PgPool,
        client_group_id: Uuid,
        status_filter: Option<&str>,
    ) -> Result<Vec<ClientPrincipalRelationship>> {
        let rows = sqlx::query_as::<_, ClientPrincipalRelationshipRow>(
            r#"
            SELECT * FROM "ob-poc".client_principal_relationship
            WHERE client_group_id = $1
              AND ($2::text IS NULL OR relationship_status = $2)
            ORDER BY created_at DESC
            "#,
        )
        .bind(client_group_id)
        .bind(status_filter)
        .fetch_all(pool)
        .await
        .context("Failed to list relationships")?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Get existing active relationships for a client group (for scoring)
    pub async fn get_active_relationships_for_client(
        pool: &PgPool,
        client_group_id: Uuid,
    ) -> Result<Vec<ClientPrincipalRelationship>> {
        let rows = sqlx::query_as::<_, ClientPrincipalRelationshipRow>(
            r#"
            SELECT * FROM "ob-poc".client_principal_relationship
            WHERE client_group_id = $1
              AND relationship_status = 'active'
            "#,
        )
        .bind(client_group_id)
        .fetch_all(pool)
        .await
        .context("Failed to fetch active relationships")?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    // ========================================================================
    // Ruleset + Rules
    // ========================================================================

    pub async fn create_ruleset(
        pool: &PgPool,
        owner_type: &str,
        owner_id: Option<Uuid>,
        name: &str,
        ruleset_boundary: &str,
        effective_from: Option<DateTime<Utc>>,
        effective_to: Option<DateTime<Utc>>,
    ) -> Result<Uuid> {
        let eff_from = effective_from.unwrap_or_else(Utc::now);

        let row = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO "ob-poc".ruleset
                (owner_type, owner_id, name, ruleset_boundary,
                 effective_from, effective_to)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING ruleset_id
            "#,
        )
        .bind(owner_type)
        .bind(owner_id)
        .bind(name)
        .bind(ruleset_boundary)
        .bind(eff_from)
        .bind(effective_to)
        .fetch_one(pool)
        .await
        .context("Failed to create ruleset")?;

        Ok(row)
    }

    pub async fn publish_ruleset(pool: &PgPool, ruleset_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".ruleset
            SET status = 'active', updated_at = now()
            WHERE ruleset_id = $1 AND status = 'draft'
            "#,
        )
        .bind(ruleset_id)
        .execute(pool)
        .await
        .context("Failed to publish ruleset")?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn retire_ruleset(pool: &PgPool, ruleset_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".ruleset
            SET status = 'retired', effective_to = now(), updated_at = now()
            WHERE ruleset_id = $1 AND status = 'active'
            "#,
        )
        .bind(ruleset_id)
        .execute(pool)
        .await
        .context("Failed to retire ruleset")?;

        Ok(result.rows_affected() > 0)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn add_rule(
        pool: &PgPool,
        ruleset_id: Uuid,
        name: &str,
        kind: &str,
        when_expr: &serde_json::Value,
        then_effect: &serde_json::Value,
        explain: Option<&str>,
        priority: Option<i32>,
    ) -> Result<Uuid> {
        let prio = priority.unwrap_or(100);

        let row = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO "ob-poc".rule
                (ruleset_id, name, kind, when_expr, then_effect, explain, priority)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING rule_id
            "#,
        )
        .bind(ruleset_id)
        .bind(name)
        .bind(kind)
        .bind(when_expr)
        .bind(then_effect)
        .bind(explain)
        .bind(prio)
        .fetch_one(pool)
        .await
        .context("Failed to add rule")?;

        Ok(row)
    }

    /// Gather all active rulesets + rules applicable to the evaluation.
    /// Returns (ruleset, rules) pairs for: global + per-offering + per-principal.
    pub async fn gather_rules_for_evaluation(
        pool: &PgPool,
        offering_ids: &[Uuid],
        principal_ids: &[Uuid],
    ) -> Result<Vec<(Ruleset, Vec<Rule>)>> {
        // Get all active rulesets that apply
        let rulesets = sqlx::query_as::<_, RulesetRow>(
            r#"
            SELECT * FROM "ob-poc".ruleset
            WHERE status = 'active'
              AND now() BETWEEN effective_from AND COALESCE(effective_to, 'infinity'::timestamptz)
              AND (
                  owner_type = 'global'
                  OR (owner_type = 'offering' AND owner_id = ANY($1))
                  OR (owner_type = 'principal' AND owner_id = ANY($2))
              )
            ORDER BY
                CASE owner_type
                    WHEN 'global' THEN 0
                    WHEN 'offering' THEN 1
                    WHEN 'principal' THEN 2
                END,
                ruleset_boundary
            "#,
        )
        .bind(offering_ids)
        .bind(principal_ids)
        .fetch_all(pool)
        .await
        .context("Failed to gather rulesets")?;

        let mut result = Vec::with_capacity(rulesets.len());
        for rs_row in rulesets {
            let rules = sqlx::query_as::<_, RuleRow>(
                r#"
                SELECT * FROM "ob-poc".rule
                WHERE ruleset_id = $1
                ORDER BY priority, created_at
                "#,
            )
            .bind(rs_row.ruleset_id)
            .fetch_all(pool)
            .await
            .context("Failed to fetch rules for ruleset")?;

            let rs: Ruleset = rs_row.into();
            let rules: Vec<Rule> = rules.into_iter().map(Into::into).collect();
            result.push((rs, rules));
        }

        Ok(result)
    }

    // ========================================================================
    // Rule Field Dictionary
    // ========================================================================

    pub async fn get_field_dictionary(pool: &PgPool) -> Result<Vec<RuleFieldDictionaryEntry>> {
        #[derive(sqlx::FromRow)]
        struct Row {
            field_key: String,
            field_type: String,
            description: Option<String>,
            source_table: Option<String>,
            added_in_version: i32,
        }

        let rows = sqlx::query_as::<_, Row>(
            r#"SELECT * FROM "ob-poc".rule_field_dictionary ORDER BY field_key"#,
        )
        .fetch_all(pool)
        .await
        .context("Failed to fetch field dictionary")?;

        Ok(rows
            .into_iter()
            .map(|r| RuleFieldDictionaryEntry {
                field_key: r.field_key,
                field_type: r.field_type,
                description: r.description,
                source_table: r.source_table,
                added_in_version: r.added_in_version,
            })
            .collect())
    }

    pub async fn register_field(
        pool: &PgPool,
        field_key: &str,
        field_type: &str,
        description: Option<&str>,
        source_table: Option<&str>,
    ) -> Result<RuleFieldDictionaryEntry> {
        #[derive(sqlx::FromRow)]
        struct Row {
            field_key: String,
            field_type: String,
            description: Option<String>,
            source_table: Option<String>,
            added_in_version: i32,
        }

        let row = sqlx::query_as::<_, Row>(
            r#"
            INSERT INTO "ob-poc".rule_field_dictionary (field_key, field_type, description, source_table)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (field_key) DO UPDATE
                SET field_type = EXCLUDED.field_type,
                    description = COALESCE(EXCLUDED.description, "ob-poc".rule_field_dictionary.description),
                    source_table = COALESCE(EXCLUDED.source_table, "ob-poc".rule_field_dictionary.source_table)
            RETURNING field_key, field_type, description, source_table, added_in_version
            "#,
        )
        .bind(field_key)
        .bind(field_type)
        .bind(description)
        .bind(source_table)
        .fetch_one(pool)
        .await
        .context("Failed to register rule field")?;

        Ok(RuleFieldDictionaryEntry {
            field_key: row.field_key,
            field_type: row.field_type,
            description: row.description,
            source_table: row.source_table,
            added_in_version: row.added_in_version,
        })
    }

    // ========================================================================
    // Contract Pack
    // ========================================================================

    pub async fn create_contract_pack(
        pool: &PgPool,
        code: &str,
        name: &str,
        description: Option<&str>,
    ) -> Result<Uuid> {
        let row = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO "ob-poc".contract_pack (code, name, description)
            VALUES ($1, $2, $3)
            RETURNING contract_pack_id
            "#,
        )
        .bind(code)
        .bind(name)
        .bind(description)
        .fetch_one(pool)
        .await
        .context("Failed to create contract pack")?;

        Ok(row)
    }

    pub async fn add_contract_template(
        pool: &PgPool,
        contract_pack_id: Uuid,
        template_type: &str,
        template_ref: Option<&str>,
    ) -> Result<Uuid> {
        let row = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO "ob-poc".contract_template
                (contract_pack_id, template_type, template_ref)
            VALUES ($1, $2, $3)
            RETURNING contract_template_id
            "#,
        )
        .bind(contract_pack_id)
        .bind(template_type)
        .bind(template_ref)
        .fetch_one(pool)
        .await
        .context("Failed to add contract template")?;

        Ok(row)
    }

    pub async fn get_contract_pack_by_code(
        pool: &PgPool,
        code: &str,
    ) -> Result<Option<ContractPack>> {
        #[derive(sqlx::FromRow)]
        struct Row {
            contract_pack_id: Uuid,
            code: String,
            name: String,
            description: Option<String>,
            effective_from: DateTime<Utc>,
            effective_to: Option<DateTime<Utc>>,
            metadata: Option<serde_json::Value>,
            created_at: Option<DateTime<Utc>>,
            updated_at: Option<DateTime<Utc>>,
        }

        let row =
            sqlx::query_as::<_, Row>(r#"SELECT * FROM "ob-poc".contract_pack WHERE code = $1"#)
                .bind(code)
                .fetch_optional(pool)
                .await
                .context("Failed to fetch contract pack")?;

        Ok(row.map(|r| ContractPack {
            contract_pack_id: r.contract_pack_id,
            code: r.code,
            name: r.name,
            description: r.description,
            effective_from: r.effective_from,
            effective_to: r.effective_to,
            metadata: r.metadata,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }))
    }

    // ========================================================================
    // Eligibility Evaluation (append-only)
    // ========================================================================

    #[allow(clippy::too_many_arguments)]
    pub async fn insert_evaluation(
        pool: &PgPool,
        client_profile_id: Uuid,
        client_group_id: Uuid,
        product_offering_ids: &[Uuid],
        requested_by: &str,
        policy_snapshot: &serde_json::Value,
        evaluation_context: Option<&serde_json::Value>,
        result: &serde_json::Value,
        explain: &serde_json::Value,
    ) -> Result<Uuid> {
        let row = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO "ob-poc".eligibility_evaluation
                (client_profile_id, client_group_id, product_offering_ids,
                 requested_by, policy_snapshot, evaluation_context,
                 result, explain)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING eligibility_evaluation_id
            "#,
        )
        .bind(client_profile_id)
        .bind(client_group_id)
        .bind(product_offering_ids)
        .bind(requested_by)
        .bind(policy_snapshot)
        .bind(evaluation_context)
        .bind(result)
        .bind(explain)
        .fetch_one(pool)
        .await
        .context("Failed to insert evaluation")?;

        Ok(row)
    }

    pub async fn select_principal_on_evaluation(
        pool: &PgPool,
        evaluation_id: Uuid,
        principal_id: Uuid,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".eligibility_evaluation
            SET selected_principal_id = $2, selected_at = now()
            WHERE eligibility_evaluation_id = $1
              AND selected_principal_id IS NULL
            "#,
        )
        .bind(evaluation_id)
        .bind(principal_id)
        .execute(pool)
        .await
        .context("Failed to select principal")?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn get_evaluation(
        pool: &PgPool,
        evaluation_id: Uuid,
    ) -> Result<Option<EligibilityEvaluation>> {
        #[derive(sqlx::FromRow)]
        struct Row {
            eligibility_evaluation_id: Uuid,
            client_profile_id: Uuid,
            client_group_id: Uuid,
            product_offering_ids: Vec<Uuid>,
            requested_at: DateTime<Utc>,
            requested_by: String,
            policy_snapshot: serde_json::Value,
            evaluation_context: Option<serde_json::Value>,
            result: serde_json::Value,
            explain: serde_json::Value,
            selected_principal_id: Option<Uuid>,
            selected_at: Option<DateTime<Utc>>,
            runbook_entry_id: Option<Uuid>,
        }

        let row = sqlx::query_as::<_, Row>(
            r#"
            SELECT * FROM "ob-poc".eligibility_evaluation
            WHERE eligibility_evaluation_id = $1
            "#,
        )
        .bind(evaluation_id)
        .fetch_optional(pool)
        .await
        .context("Failed to fetch evaluation")?;

        Ok(row.map(|r| EligibilityEvaluation {
            eligibility_evaluation_id: r.eligibility_evaluation_id,
            client_profile_id: r.client_profile_id,
            client_group_id: r.client_group_id,
            product_offering_ids: r.product_offering_ids,
            requested_at: r.requested_at,
            requested_by: r.requested_by,
            policy_snapshot: r.policy_snapshot,
            evaluation_context: r.evaluation_context,
            result: r.result,
            explain: r.explain,
            selected_principal_id: r.selected_principal_id,
            selected_at: r.selected_at,
            runbook_entry_id: r.runbook_entry_id,
        }))
    }

    // ========================================================================
    // Coverage Views (read-only)
    // ========================================================================

    pub async fn get_regulatory_gaps(pool: &PgPool) -> Result<Vec<GapReportEntry>> {
        let rows = sqlx::query_as::<_, GapRow>(
            r#"
            SELECT offering_code, jurisdiction, NULL::text as principal_code,
                   gap_type, detail, NULL::text as delivery_model
            FROM "ob-poc".v_regulatory_gaps
            "#,
        )
        .fetch_all(pool)
        .await
        .context("Failed to fetch regulatory gaps")?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn get_commercial_gaps(pool: &PgPool) -> Result<Vec<GapReportEntry>> {
        let rows = sqlx::query_as::<_, GapRow>(
            r#"
            SELECT offering_code, jurisdiction, principal_code,
                   gap_type, detail, NULL::text as delivery_model
            FROM "ob-poc".v_commercial_gaps
            "#,
        )
        .fetch_all(pool)
        .await
        .context("Failed to fetch commercial gaps")?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn get_operational_gaps(pool: &PgPool) -> Result<Vec<GapReportEntry>> {
        let rows = sqlx::query_as::<_, GapRow>(
            r#"
            SELECT offering_code, jurisdiction, principal_code,
                   gap_type, detail, delivery_model
            FROM "ob-poc".v_operational_gaps
            "#,
        )
        .fetch_all(pool)
        .await
        .context("Failed to fetch operational gaps")?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Get services linked to a product (via product_services join table)
    pub async fn get_services_for_product(pool: &PgPool, product_id: Uuid) -> Result<Vec<Uuid>> {
        let rows = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT service_id
            FROM "ob-poc".product_services
            WHERE product_id = $1
            "#,
        )
        .bind(product_id)
        .fetch_all(pool)
        .await
        .context("Failed to fetch services for product")?;

        Ok(rows)
    }
}
