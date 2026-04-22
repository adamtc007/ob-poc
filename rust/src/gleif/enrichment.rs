//! GLEIF Enrichment Service
//!
//! Orchestrates the enrichment of entities with GLEIF data.
//!
//! Phase F.3b (2026-04-22): split into `fetch_all_for_enrich` (HTTP
//! only, pure reads) + `persist_enrichment` (DB writes, consumes the
//! fetched struct). The legacy `enrich_entity` is a thin wrapper that
//! calls both. This lets the `GleifEnrich` plugin op run the HTTP
//! phase in pre_fetch (outside the txn) and the DB phase in execute
//! (inside the caller's scope) — A1 invariant satisfied.

use super::{client::GleifClient, repository::GleifRepository, types::*};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

pub struct GleifEnrichmentService {
    client: GleifClient,
    repository: GleifRepository,
}

/// All HTTP results needed to run the `enrich_entity` persist pass.
///
/// Phase F.3b: produced by `fetch_all_for_enrich`, consumed by
/// `persist_enrichment`. JSON-round-trippable so the `GleifEnrich` op
/// can pass it through the pre_fetch → args → execute pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentFetch {
    pub record: LeiRecord,
    pub bics: Vec<BicMapping>,
    /// (parent_lei, parent_name) — None if no direct parent reported
    pub direct_parent: Option<(String, Option<String>)>,
    pub ultimate_parent: Option<(String, Option<String>)>,
    /// (manager_lei, manager_name) — None if not a fund or no manager
    pub fund_manager: Option<(String, String)>,
    pub umbrella: Option<(String, String)>,
    pub master: Option<(String, String)>,
}

impl GleifEnrichmentService {
    pub fn new(pool: Arc<PgPool>) -> Result<Self> {
        Ok(Self {
            client: GleifClient::new()?,
            repository: GleifRepository::new(pool),
        })
    }

    /// Phase F.3b (2026-04-22): HTTP-only fetch phase. Called by
    /// `GleifEnrich::pre_fetch` outside the transaction scope.
    /// Returns a self-contained struct; the caller (execute) does
    /// the DB writes via `persist_enrichment`.
    ///
    /// Pure reads against GLEIF — no DB writes, no side effects.
    pub async fn fetch_all_for_enrich(&self, lei: &str) -> Result<EnrichmentFetch> {
        let record = self
            .client
            .get_lei_record(lei)
            .await
            .context("Failed to fetch LEI record from GLEIF")?;

        let bics = self.client.get_bic_mappings(lei).await.unwrap_or_default();

        let direct_parent = match self.client.get_direct_parent(lei).await {
            Ok(Some(rel)) => {
                let parent_lei = rel.attributes.relationship.end_node.id.clone();
                let parent_name = self
                    .client
                    .get_lei_record(&parent_lei)
                    .await
                    .ok()
                    .map(|r| r.attributes.entity.legal_name.name.clone());
                Some((parent_lei, parent_name))
            }
            _ => None,
        };

        let ultimate_parent = match self.client.get_ultimate_parent(lei).await {
            Ok(Some(rel)) => {
                let parent_lei = rel.attributes.relationship.end_node.id.clone();
                let parent_name = self
                    .client
                    .get_lei_record(&parent_lei)
                    .await
                    .ok()
                    .map(|r| r.attributes.entity.legal_name.name.clone());
                Some((parent_lei, parent_name))
            }
            _ => None,
        };

        let fund_manager = match self.client.get_fund_manager(lei).await {
            Ok(Some(manager)) => manager.attributes.lei.clone().map(|manager_lei| {
                (
                    manager_lei,
                    manager.attributes.entity.legal_name.name.clone(),
                )
            }),
            _ => None,
        };

        let umbrella = match self.client.get_umbrella_fund(lei).await {
            Ok(Some(u)) => u.attributes.lei.clone().map(|umbrella_lei| {
                (umbrella_lei, u.attributes.entity.legal_name.name.clone())
            }),
            _ => None,
        };

        let master = match self.client.get_master_fund(lei).await {
            Ok(Some(m)) => m
                .attributes
                .lei
                .clone()
                .map(|master_lei| (master_lei, m.attributes.entity.legal_name.name.clone())),
            _ => None,
        };

        Ok(EnrichmentFetch {
            record,
            bics,
            direct_parent,
            ultimate_parent,
            fund_manager,
            umbrella,
            master,
        })
    }

    /// Legacy method — kept for backwards compat. Calls fetch + persist
    /// internally. Callers can migrate to the split methods as needed;
    /// under the new `GleifEnrich::pre_fetch` / `execute` flow the
    /// split is explicit.
    pub async fn enrich_entity(&self, entity_id: Uuid, lei: &str) -> Result<EnrichmentResult> {
        let fetched = self.fetch_all_for_enrich(lei).await?;
        self.persist_enrichment(entity_id, lei, fetched).await
    }

    /// Phase F.3b: DB-only persist phase. Called by
    /// `GleifEnrich::execute` inside the caller's transaction scope.
    /// All writes; no HTTP.
    #[allow(clippy::too_many_lines)]
    pub async fn persist_enrichment(
        &self,
        entity_id: Uuid,
        lei: &str,
        fetched: EnrichmentFetch,
    ) -> Result<EnrichmentResult> {
        // Unpack the pre-fetched HTTP results.
        let EnrichmentFetch {
            record,
            bics,
            direct_parent,
            ultimate_parent,
            fund_manager,
            umbrella,
            master,
        } = fetched;

        // Log sync start.
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

        // Update entity with GLEIF data.
        self.repository
            .update_entity_from_gleif(entity_id, &record)
            .await?;

        // Insert names.
        let names_added = self
            .repository
            .insert_entity_names(entity_id, &record)
            .await?;

        // Insert addresses.
        let addresses_added = self
            .repository
            .insert_entity_addresses(entity_id, &record)
            .await?;

        // Insert identifiers.
        let identifiers_added = self
            .repository
            .insert_entity_identifiers(entity_id, &record, &bics)
            .await?;

        // Insert lifecycle events.
        let events_added = self
            .repository
            .insert_lifecycle_events(entity_id, &record)
            .await?;

        // Parent relationships.
        let mut parent_relationships_added = 0;
        let direct_exception: Option<ReportingException> = None;
        let mut ultimate_exception: Option<ReportingException> = None;

        if let Some((parent_lei, parent_name_opt)) = direct_parent.as_ref() {
            self.repository
                .insert_parent_relationship(
                    entity_id,
                    parent_lei,
                    parent_name_opt.as_deref(),
                    "DIRECT_PARENT",
                    None,
                )
                .await?;
            parent_relationships_added += 1;

            sqlx::query(
                r#"UPDATE "ob-poc".entity_limited_companies
                   SET direct_parent_lei = $2
                   WHERE entity_id = $1"#,
            )
            .bind(entity_id)
            .bind(parent_lei)
            .execute(self.repository.pool.as_ref())
            .await?;
        }

        if let Some((parent_lei, parent_name_opt)) = ultimate_parent.as_ref() {
            self.repository
                .insert_parent_relationship(
                    entity_id,
                    parent_lei,
                    parent_name_opt.as_deref(),
                    "ULTIMATE_PARENT",
                    None,
                )
                .await?;
            parent_relationships_added += 1;

            sqlx::query(
                r#"UPDATE "ob-poc".entity_limited_companies
                   SET ultimate_parent_lei = $2
                   WHERE entity_id = $1"#,
            )
            .bind(entity_id)
            .bind(parent_lei)
            .execute(self.repository.pool.as_ref())
            .await?;
        }

        // Fund relationships.
        let mut fund_relationships_added = 0;

        if let Some((manager_lei, manager_name)) = fund_manager.as_ref() {
            self.repository
                .insert_parent_relationship(
                    entity_id,
                    manager_lei,
                    Some(manager_name.as_str()),
                    "FUND_MANAGER",
                    None,
                )
                .await?;
            fund_relationships_added += 1;
            tracing::info!(
                entity_id = %entity_id,
                %manager_lei,
                %manager_name,
                "Added fund manager relationship from GLEIF"
            );
        }

        if let Some((umbrella_lei, umbrella_name)) = umbrella.as_ref() {
            self.repository
                .insert_parent_relationship(
                    entity_id,
                    umbrella_lei,
                    Some(umbrella_name.as_str()),
                    "UMBRELLA_FUND",
                    None,
                )
                .await?;
            fund_relationships_added += 1;
            tracing::info!(
                entity_id = %entity_id,
                %umbrella_lei,
                %umbrella_name,
                "Added umbrella fund relationship from GLEIF"
            );
        }

        if let Some((master_lei, master_name)) = master.as_ref() {
            self.repository
                .insert_parent_relationship(
                    entity_id,
                    master_lei,
                    Some(master_name.as_str()),
                    "MASTER_FUND",
                    None,
                )
                .await?;
            fund_relationships_added += 1;
            tracing::info!(
                entity_id = %entity_id,
                %master_lei,
                %master_name,
                "Added master fund relationship from GLEIF"
            );
        }

        // Reporting exceptions based on the primary record.
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

    /// Phase F.3b-4 (2026-04-22): HTTP-only fetch for the corporate
    /// tree. Produces a `CorporateTreeResult` that `persist_corporate_tree`
    /// consumes. Exposed so `GleifImportTree::pre_fetch` can call it
    /// outside the txn.
    pub async fn fetch_corporate_tree_only(
        &self,
        root_lei: &str,
        max_depth: usize,
    ) -> Result<super::client::CorporateTreeResult> {
        self.client.fetch_corporate_tree(root_lei, max_depth).await
    }

    /// Phase F.3b-6 (2026-04-22): HTTP-only fetch for the enhanced
    /// corporate tree (with fund inclusion options). Used by
    /// `GleifImportToClientGroup::pre_fetch` when `include-funds=true`.
    pub async fn fetch_corporate_tree_with_options_only(
        &self,
        root_lei: &str,
        options: super::client::TreeFetchOptions,
    ) -> Result<super::client::CorporateTreeResult> {
        self.client
            .fetch_corporate_tree_with_options(root_lei, options)
            .await
    }

    /// Phase F.3b-4: DB-only persist phase. Takes the pre-fetched
    /// `CorporateTreeResult` and writes all entities + relationships.
    /// No HTTP.
    #[allow(clippy::too_many_lines)]
    pub async fn persist_corporate_tree(
        &self,
        root_lei: &str,
        tree_result: super::client::CorporateTreeResult,
    ) -> Result<TreeImportResult> {
        let mut entities_created = 0;
        let mut entities_updated = 0;
        let mut relationships_created = 0;
        let mut terminal_entities = Vec::new();

        let records = &tree_result.records;
        let discovered_relationships = &tree_result.relationships;

        for record in records {
            let lei = record.lei();

            let existing_id = self.repository.find_entity_by_lei(lei).await?;

            let entity_id = match existing_id {
                Some(id) => {
                    self.repository.update_entity_from_gleif(id, record).await?;
                    entities_updated += 1;
                    id
                }
                None => {
                    let id = self.repository.create_entity_from_gleif(record).await?;
                    entities_created += 1;
                    id
                }
            };

            self.repository
                .insert_entity_names(entity_id, record)
                .await?;
            self.repository
                .insert_entity_addresses(entity_id, record)
                .await?;
            self.repository
                .insert_entity_identifiers(entity_id, record, &[])
                .await?;

            if record.relationships.is_none()
                || (record
                    .relationships
                    .as_ref()
                    .map(|r| r.direct_parent.is_none() && r.ultimate_parent.is_none())
                    .unwrap_or(true))
            {
                if !record.is_fund() {
                    terminal_entities.push(TerminalEntity {
                        lei: lei.to_string(),
                        name: record.attributes.entity.legal_name.name.clone(),
                        exception: None,
                    });
                }
            }
        }

        for rel in discovered_relationships {
            let child_id = self.repository.find_entity_by_lei(&rel.child_lei).await?;

            if let Some(child_entity_id) = child_id {
                let parent_record = records.iter().find(|r| r.lei() == rel.parent_lei);
                let parent_name =
                    parent_record.map(|r| r.attributes.entity.legal_name.name.as_str());

                let db_rel_type = match rel.relationship_type.as_str() {
                    "DIRECT_PARENT" => "DIRECT_PARENT",
                    "IS_FUND-MANAGED_BY" => "FUND_MANAGER",
                    "IS_SUBFUND_OF" => "UMBRELLA_FUND",
                    "IS_FEEDER_TO" => "MASTER_FUND",
                    other => other,
                };

                self.repository
                    .insert_parent_relationship(
                        child_entity_id,
                        &rel.parent_lei,
                        parent_name,
                        db_rel_type,
                        None,
                    )
                    .await?;
                relationships_created += 1;
            }
        }

        let imported_leis: Vec<String> = records.iter().map(|r| r.lei().to_string()).collect();

        tracing::info!(
            root_lei = %root_lei,
            entities_created = entities_created,
            entities_updated = entities_updated,
            relationships_created = relationships_created,
            total_leis = imported_leis.len(),
            "Corporate tree persisted from pre-fetched bundle"
        );

        Ok(TreeImportResult {
            root_lei: root_lei.to_string(),
            entities_created,
            entities_updated,
            relationships_created,
            terminal_entities,
            imported_leis,
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

    /// Import a corporate tree with optional fund relationship expansion
    ///
    /// This enhanced method can load funds managed by entities in the tree
    /// (IS_FUND-MANAGED_BY) along with fund structure relationships.
    ///
    /// Returns an enhanced TreeImportResult with fund counts.
    pub async fn import_corporate_tree_with_options(
        &self,
        root_lei: &str,
        options: super::client::TreeFetchOptions,
    ) -> Result<TreeImportResult> {
        let mut entities_created = 0;
        let mut entities_updated = 0;
        let mut relationships_created = 0;
        let mut terminal_entities = Vec::new();

        // Fetch all records using enhanced traversal with fund support
        let tree_result = self
            .client
            .fetch_corporate_tree_with_options(root_lei, options)
            .await?;

        let records = &tree_result.records;
        let discovered_relationships = &tree_result.relationships;

        tracing::info!(
            root_lei = %root_lei,
            total_records = records.len(),
            fund_count = tree_result.fund_count,
            mancos_expanded = tree_result.mancos_expanded,
            relationships = discovered_relationships.len(),
            "Corporate tree fetch complete"
        );

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

            // Check if this is a terminal entity (no parent in ownership hierarchy)
            // Note: funds with only IM relationships are NOT terminal for UBO purposes
            if record.relationships.is_none()
                || (record
                    .relationships
                    .as_ref()
                    .map(|r| r.direct_parent.is_none() && r.ultimate_parent.is_none())
                    .unwrap_or(true))
            {
                // Only add non-funds as terminal entities (funds have ManCo, not parent)
                if !record.is_fund() {
                    terminal_entities.push(TerminalEntity {
                        lei: lei.to_string(),
                        name: record.attributes.entity.legal_name.name.clone(),
                        exception: None,
                    });
                }
            }
        }

        // Now create parent relationships from discovered relationships
        // This includes ownership AND IM relationships
        for rel in discovered_relationships {
            let child_id = self.repository.find_entity_by_lei(&rel.child_lei).await?;

            if let Some(child_entity_id) = child_id {
                // Find parent name from records if available
                let parent_record = records.iter().find(|r| r.lei() == rel.parent_lei);
                let parent_name =
                    parent_record.map(|r| r.attributes.entity.legal_name.name.as_str());

                // Map relationship type for DB storage
                let db_rel_type = match rel.relationship_type.as_str() {
                    "DIRECT_PARENT" => "DIRECT_PARENT",
                    "IS_FUND-MANAGED_BY" => "FUND_MANAGER",
                    "IS_SUBFUND_OF" => "UMBRELLA_FUND",
                    "IS_FEEDER_TO" => "MASTER_FUND",
                    other => other,
                };

                self.repository
                    .insert_parent_relationship(
                        child_entity_id,
                        &rel.parent_lei,
                        parent_name,
                        db_rel_type,
                        None,
                    )
                    .await?;
                relationships_created += 1;
            }
        }

        // Collect all LEIs from the imported records
        let imported_leis: Vec<String> = records.iter().map(|r| r.lei().to_string()).collect();

        tracing::info!(
            root_lei = %root_lei,
            entities_created = entities_created,
            entities_updated = entities_updated,
            relationships_created = relationships_created,
            total_leis = imported_leis.len(),
            "Corporate tree import complete"
        );

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
