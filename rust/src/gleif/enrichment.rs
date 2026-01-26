//! GLEIF Enrichment Service
//!
//! Orchestrates the enrichment of entities with GLEIF data.

use super::{client::GleifClient, repository::GleifRepository, types::*};
use anyhow::{Context, Result};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

pub struct GleifEnrichmentService {
    client: GleifClient,
    repository: GleifRepository,
}

impl GleifEnrichmentService {
    pub fn new(pool: Arc<PgPool>) -> Result<Self> {
        Ok(Self {
            client: GleifClient::new()?,
            repository: GleifRepository::new(pool),
        })
    }

    /// Enrich an existing entity with GLEIF data by LEI
    pub async fn enrich_entity(&self, entity_id: Uuid, lei: &str) -> Result<EnrichmentResult> {
        // Log sync start
        let _sync_id = self
            .repository
            .log_sync(
                Some(entity_id),
                Some(lei),
                "FULL",
                "IN_PROGRESS",
                0,
                0,
                0,
                None,
            )
            .await?;

        // Fetch LEI record from GLEIF API
        let record = self
            .client
            .get_lei_record(lei)
            .await
            .context("Failed to fetch LEI record from GLEIF")?;

        // Fetch BIC mappings
        let bics = self.client.get_bic_mappings(lei).await.unwrap_or_default();

        // Update entity with GLEIF data
        self.repository
            .update_entity_from_gleif(entity_id, &record)
            .await?;

        // Insert names
        let names_added = self
            .repository
            .insert_entity_names(entity_id, &record)
            .await?;

        // Insert addresses
        let addresses_added = self
            .repository
            .insert_entity_addresses(entity_id, &record)
            .await?;

        // Insert identifiers
        let identifiers_added = self
            .repository
            .insert_entity_identifiers(entity_id, &record, &bics)
            .await?;

        // Insert lifecycle events
        let events_added = self
            .repository
            .insert_lifecycle_events(entity_id, &record)
            .await?;

        // Fetch and insert parent relationships
        let mut parent_relationships_added = 0;
        let direct_exception: Option<ReportingException> = None;
        let mut ultimate_exception: Option<ReportingException> = None;

        // Fetch direct parent
        if let Ok(Some(direct_parent)) = self.client.get_direct_parent(lei).await {
            let parent_lei = &direct_parent.attributes.relationship.end_node.id;
            let parent_record = self.client.get_lei_record(parent_lei).await.ok();
            let parent_name = parent_record
                .as_ref()
                .map(|r| r.attributes.entity.legal_name.name.as_str());

            self.repository
                .insert_parent_relationship(
                    entity_id,
                    parent_lei,
                    parent_name,
                    "DIRECT_PARENT",
                    None,
                )
                .await?;
            parent_relationships_added += 1;

            // Update the entity with direct parent LEI
            sqlx::query(
                r#"
                UPDATE "ob-poc".entity_limited_companies
                SET direct_parent_lei = $2
                WHERE entity_id = $1
            "#,
            )
            .bind(entity_id)
            .bind(parent_lei)
            .execute(self.repository.pool.as_ref())
            .await?;
        }

        // Fetch ultimate parent
        if let Ok(Some(ultimate_parent)) = self.client.get_ultimate_parent(lei).await {
            let parent_lei = &ultimate_parent.attributes.relationship.end_node.id;
            let parent_record = self.client.get_lei_record(parent_lei).await.ok();
            let parent_name = parent_record
                .as_ref()
                .map(|r| r.attributes.entity.legal_name.name.as_str());

            self.repository
                .insert_parent_relationship(
                    entity_id,
                    parent_lei,
                    parent_name,
                    "ULTIMATE_PARENT",
                    None,
                )
                .await?;
            parent_relationships_added += 1;

            // Update the entity with ultimate parent LEI
            sqlx::query(
                r#"
                UPDATE "ob-poc".entity_limited_companies
                SET ultimate_parent_lei = $2
                WHERE entity_id = $1
            "#,
            )
            .bind(entity_id)
            .bind(parent_lei)
            .execute(self.repository.pool.as_ref())
            .await?;
        }

        // Fetch and insert fund relationships (for fund entities)
        // These are trading-relevant: fund manager, umbrella fund, master fund
        let mut fund_relationships_added = 0;

        // Fund manager (IS_FUND-MANAGED_BY) -> INVESTMENT_MANAGER role
        if let Ok(Some(fund_manager)) = self.client.get_fund_manager(lei).await {
            if let Some(ref manager_lei) = fund_manager.attributes.lei {
                let manager_name = fund_manager.attributes.entity.legal_name.name.as_str();

                self.repository
                    .insert_parent_relationship(
                        entity_id,
                        manager_lei,
                        Some(manager_name),
                        "FUND_MANAGER",
                        None,
                    )
                    .await?;
                fund_relationships_added += 1;
                tracing::info!(
                    entity_id = %entity_id,
                    manager_lei = %manager_lei,
                    manager_name = %manager_name,
                    "Added fund manager relationship from GLEIF"
                );
            }
        }

        // Umbrella fund (IS_SUBFUND_OF) -> fund structure
        if let Ok(Some(umbrella)) = self.client.get_umbrella_fund(lei).await {
            if let Some(ref umbrella_lei) = umbrella.attributes.lei {
                let umbrella_name = umbrella.attributes.entity.legal_name.name.as_str();

                self.repository
                    .insert_parent_relationship(
                        entity_id,
                        umbrella_lei,
                        Some(umbrella_name),
                        "UMBRELLA_FUND",
                        None,
                    )
                    .await?;
                fund_relationships_added += 1;
                tracing::info!(
                    entity_id = %entity_id,
                    umbrella_lei = %umbrella_lei,
                    umbrella_name = %umbrella_name,
                    "Added umbrella fund relationship from GLEIF"
                );
            }
        }

        // Master fund (IS_FEEDER_TO) -> master-feeder structure
        if let Ok(Some(master)) = self.client.get_master_fund(lei).await {
            if let Some(ref master_lei) = master.attributes.lei {
                let master_name = master.attributes.entity.legal_name.name.as_str();

                self.repository
                    .insert_parent_relationship(
                        entity_id,
                        master_lei,
                        Some(master_name),
                        "MASTER_FUND",
                        None,
                    )
                    .await?;
                fund_relationships_added += 1;
                tracing::info!(
                    entity_id = %entity_id,
                    master_lei = %master_lei,
                    master_name = %master_name,
                    "Added master fund relationship from GLEIF"
                );
            }
        }

        // Check for reporting exceptions (when no parent is found)
        // This is simplified - in reality we'd need to check the exception endpoint
        if let Some(ref rels) = record.relationships {
            if rels.direct_parent.is_none() && rels.ultimate_parent.is_none() {
                // Check entity category for public float indicators
                if record.attributes.entity.category.as_deref() == Some("FUND")
                    || record.attributes.entity.sub_category.as_deref() == Some("FUND")
                {
                    // Funds often have fund manager relationships instead
                } else if record
                    .attributes
                    .entity
                    .legal_form
                    .as_ref()
                    .and_then(|lf| lf.id.as_deref())
                    == Some("UDY2")
                {
                    // Societas Europaea - likely publicly traded
                    ultimate_exception = Some(ReportingException::NoKnownPerson);
                }
            }
        }

        // Update parent exceptions
        self.repository
            .update_parent_exceptions(
                entity_id,
                direct_exception.as_ref().map(|e| e.as_str()),
                ultimate_exception.as_ref().map(|e| e.as_str()),
            )
            .await?;

        // Update UBO status based on exceptions
        let ubo_status = if ultimate_exception
            .as_ref()
            .map(|e| e.is_public_float())
            .unwrap_or(false)
        {
            "PUBLIC_FLOAT"
        } else {
            // Either requires BODS lookup or no exception info yet
            "PENDING"
        };

        self.repository
            .update_ubo_status(entity_id, ubo_status)
            .await?;

        // Log sync completion
        self.repository
            .log_sync(Some(entity_id), Some(lei), "FULL", "SUCCESS", 1, 1, 0, None)
            .await?;

        Ok(EnrichmentResult {
            entity_id,
            lei: lei.to_string(),
            names_added,
            addresses_added,
            identifiers_added,
            parent_relationships_added,
            fund_relationships_added,
            events_added,
            direct_parent_exception: direct_exception,
            ultimate_parent_exception: ultimate_exception,
        })
    }

    /// Import a corporate tree starting from a root LEI
    pub async fn import_corporate_tree(
        &self,
        root_lei: &str,
        max_depth: usize,
    ) -> Result<TreeImportResult> {
        let mut entities_created = 0;
        let mut entities_updated = 0;
        let mut relationships_created = 0;
        let mut terminal_entities = Vec::new();

        // Fetch all records in the tree (now includes discovered relationships)
        let tree_result = self
            .client
            .fetch_corporate_tree(root_lei, max_depth)
            .await?;

        let records = &tree_result.records;
        let discovered_relationships = &tree_result.relationships;

        for record in records {
            let lei = record.lei();

            // Check if entity exists
            let existing_id = self.repository.find_entity_by_lei(lei).await?;

            let entity_id = match existing_id {
                Some(id) => {
                    // Update existing entity
                    self.repository.update_entity_from_gleif(id, record).await?;
                    entities_updated += 1;
                    id
                }
                None => {
                    // Create new entity
                    let id = self.repository.create_entity_from_gleif(record).await?;
                    entities_created += 1;
                    id
                }
            };

            // Insert names, addresses, identifiers
            self.repository
                .insert_entity_names(entity_id, record)
                .await?;
            self.repository
                .insert_entity_addresses(entity_id, record)
                .await?;
            self.repository
                .insert_entity_identifiers(entity_id, record, &[])
                .await?;

            // Check if this is a terminal entity (no parent)
            if record.relationships.is_none()
                || (record
                    .relationships
                    .as_ref()
                    .map(|r| r.direct_parent.is_none() && r.ultimate_parent.is_none())
                    .unwrap_or(true))
            {
                terminal_entities.push(TerminalEntity {
                    lei: lei.to_string(),
                    name: record.attributes.entity.legal_name.name.clone(),
                    exception: None, // Would need to fetch from exception endpoint
                });
            }
        }

        // Now create parent relationships from discovered relationships
        for rel in discovered_relationships {
            let child_id = self.repository.find_entity_by_lei(&rel.child_lei).await?;

            if let Some(child_entity_id) = child_id {
                // Find parent name from records if available
                let parent_record = records.iter().find(|r| r.lei() == rel.parent_lei);
                let parent_name =
                    parent_record.map(|r| r.attributes.entity.legal_name.name.as_str());

                self.repository
                    .insert_parent_relationship(
                        child_entity_id,
                        &rel.parent_lei,
                        parent_name,
                        &rel.relationship_type,
                        None,
                    )
                    .await?;
                relationships_created += 1;
            }
        }

        // Collect all LEIs from the imported records
        let imported_leis: Vec<String> = records.iter().map(|r| r.lei().to_string()).collect();

        Ok(TreeImportResult {
            root_lei: root_lei.to_string(),
            entities_created,
            entities_updated,
            relationships_created,
            terminal_entities,
            imported_leis,
        })
    }

    /// Refresh GLEIF data for an entity
    pub async fn refresh_entity(&self, entity_id: Uuid) -> Result<EnrichmentResult> {
        // Get the LEI for this entity
        let lei: Option<String> = sqlx::query_scalar(
            r#"
            SELECT lei FROM "ob-poc".entity_limited_companies
            WHERE entity_id = $1
        "#,
        )
        .bind(entity_id)
        .fetch_optional(self.repository.pool.as_ref())
        .await?
        .flatten();

        match lei {
            Some(lei) => self.enrich_entity(entity_id, &lei).await,
            None => anyhow::bail!("Entity {} has no LEI", entity_id),
        }
    }
}
