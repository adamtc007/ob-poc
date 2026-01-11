//! Normalization functions for GLEIF data
//!
//! Converts GLEIF API types to normalized structures.

use crate::gleif::types::{Address, EntityCategory, EntityStatus, LeiRecord, RelationshipRecord};
use crate::research::sources::normalized::{
    EntityStatus as NormEntityStatus, EntityType, NormalizedAddress, NormalizedEntity,
    NormalizedRelationship, RelationshipType,
};

/// Normalize a GLEIF LEI record to a NormalizedEntity
pub fn normalize_lei_record(record: &LeiRecord, include_raw: bool) -> NormalizedEntity {
    let entity = &record.attributes.entity;

    NormalizedEntity {
        source_key: record.lei().to_string(),
        source_name: "GLEIF".to_string(),
        name: entity.legal_name.name.clone(),
        lei: Some(record.lei().to_string()),
        registration_number: entity.registered_as.clone(),
        tax_id: None,
        entity_type: entity.category.as_deref().map(map_entity_category),
        jurisdiction: entity.jurisdiction.clone(),
        status: entity.status.as_deref().map(map_entity_status),
        incorporated_date: entity
            .creation_date
            .as_deref()
            .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()),
        dissolved_date: entity
            .expiration_date
            .as_deref()
            .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()),
        registered_address: Some(normalize_address(&entity.legal_address)),
        business_address: entity.headquarters_address.as_ref().map(normalize_address),
        raw_response: if include_raw {
            serde_json::to_value(record).ok()
        } else {
            None
        },
    }
}

/// Map GLEIF entity category to our EntityType
fn map_entity_category(category: &str) -> EntityType {
    match EntityCategory::parse(category) {
        EntityCategory::Fund => EntityType::Fund,
        EntityCategory::General => EntityType::LimitedCompany,
        EntityCategory::Branch => EntityType::Branch,
        EntityCategory::SoleProprietor => EntityType::SoleProprietor,
        EntityCategory::Unknown(s) => EntityType::Unknown(s),
    }
}

/// Map GLEIF entity status to our EntityStatus
fn map_entity_status(status: &str) -> NormEntityStatus {
    match EntityStatus::parse(status) {
        EntityStatus::Active => NormEntityStatus::Active,
        EntityStatus::Inactive => NormEntityStatus::Inactive,
        EntityStatus::Unknown(s) => NormEntityStatus::Unknown(s),
    }
}

/// Normalize a GLEIF address to NormalizedAddress
fn normalize_address(addr: &Address) -> NormalizedAddress {
    let mut lines = addr.address_lines.clone();

    // Add address number if present
    if let Some(ref num) = addr.address_number {
        if !num.is_empty() && lines.first().is_none_or(|l| !l.starts_with(num)) {
            if let Some(first) = lines.first_mut() {
                *first = format!("{} {}", num, first);
            }
        }
    }

    NormalizedAddress {
        lines,
        city: addr.city.clone(),
        region: addr.region.clone(),
        postal_code: addr.postal_code.clone(),
        country: addr.country.clone(),
    }
}

/// Normalize a GLEIF relationship record to NormalizedRelationship
pub fn normalize_relationship(
    rel: &RelationshipRecord,
    child_name: &str,
    parent_name: &str,
) -> NormalizedRelationship {
    let rel_type = match rel.attributes.relationship.relationship_type.as_str() {
        "IS_DIRECTLY_CONSOLIDATED_BY" => RelationshipType::DirectParent,
        "IS_ULTIMATELY_CONSOLIDATED_BY" => RelationshipType::UltimateParent,
        "IS_FUND-MANAGED_BY" => RelationshipType::FundManagedBy,
        "IS_SUBFUND_OF" => RelationshipType::SubfundOf,
        _ => RelationshipType::DirectParent, // Default for unknown types
    };

    let is_direct = matches!(rel_type, RelationshipType::DirectParent);

    NormalizedRelationship {
        parent_key: rel.attributes.relationship.end_node.id.clone(),
        parent_name: parent_name.to_string(),
        child_key: rel.attributes.relationship.start_node.id.clone(),
        child_name: child_name.to_string(),
        relationship_type: rel_type,
        ownership_pct: None, // GLEIF doesn't provide ownership percentages
        is_direct,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_entity_category() {
        assert!(matches!(map_entity_category("FUND"), EntityType::Fund));
        assert!(matches!(
            map_entity_category("GENERAL"),
            EntityType::LimitedCompany
        ));
        assert!(matches!(map_entity_category("BRANCH"), EntityType::Branch));
        assert!(matches!(
            map_entity_category("UNKNOWN_TYPE"),
            EntityType::Unknown(_)
        ));
    }

    #[test]
    fn test_map_entity_status() {
        assert!(matches!(
            map_entity_status("ACTIVE"),
            NormEntityStatus::Active
        ));
        assert!(matches!(
            map_entity_status("INACTIVE"),
            NormEntityStatus::Inactive
        ));
        assert!(matches!(
            map_entity_status("PENDING"),
            NormEntityStatus::Unknown(_)
        ));
    }
}
