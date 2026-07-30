//! Template Service — instantiate stewardship templates into changesets.
//!
//! Templates are versioned, domain-scoped objects (spec §9.5) that pre-populate
//! changeset entries. This module provides template instantiation logic.

use anyhow::{anyhow, Result};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use super::store::StewardshipStore;
use super::types::*;

/// Instantiate a template into a changeset, creating entries from template items.
///
/// Returns the list of created `ChangesetEntryRow` records.
pub async fn instantiate_template(
    pool: &PgPool,
    changeset_id: Uuid,
    template_fqn: &str,
    actor_id: &str,
    overrides: &serde_json::Value,
) -> Result<Vec<ChangesetEntryRow>> {
    // 1. Load the active template
    let template = StewardshipStore::get_active_template(pool, template_fqn)
        .await?
        .ok_or_else(|| anyhow!("No active template found for FQN '{}'", template_fqn))?;

    // 2. Create entries from template items
    let mut entries = Vec::new();
    for item in &template.items {
        let fqn = apply_fqn_pattern(&item.fqn_pattern, overrides);
        let payload = merge_payload(item.default_payload.as_ref(), overrides);

        let entry = ChangesetEntryRow {
            entry_id: Uuid::new_v4(),
            changeset_id,
            object_fqn: fqn,
            object_type: item.object_type.clone(),
            change_kind: item.action.as_str().to_string(),
            draft_payload: payload,
            base_snapshot_id: None,
            created_at: Utc::now(),
            action: item.action.clone(),
            predecessor_id: None,
            revision: 1,
            reasoning: Some(format!(
                "Created from template '{}' v{}",
                template.fqn, template.version
            )),
            guardrail_log: serde_json::json!([]),
        };
        entries.push(entry);
    }

    // 3. Emit audit event
    let event = StewardshipRecord {
        event_id: Uuid::new_v4(),
        changeset_id,
        event_type: StewardshipEventType::ChangesetCreated,
        actor_id: actor_id.to_string(),
        payload: serde_json::json!({
            "template_fqn": template.fqn,
            "template_version": template.version.to_string(),
            "items_created": entries.len(),
        }),
        viewport_manifest_id: None,
        created_at: Utc::now(),
    };
    StewardshipStore::append_event(pool, &event).await?;

    Ok(entries)
}

/// Apply variable substitution to an FQN pattern.
/// Pattern: `{domain}.{name}` where `{name}` can be substituted from overrides.
fn apply_fqn_pattern(pattern: &str, overrides: &serde_json::Value) -> String {
    let mut result = pattern.to_string();
    if let Some(obj) = overrides.as_object() {
        for (key, value) in obj {
            if let Some(s) = value.as_str() {
                let placeholder = format!("{{{}}}", key);
                result = result.replace(&placeholder, s);
            }
        }
    }
    result
}

/// Merge a default payload with overrides. Override keys win.
fn merge_payload(
    default: Option<&serde_json::Value>,
    overrides: &serde_json::Value,
) -> serde_json::Value {
    match (default, overrides.as_object()) {
        (Some(serde_json::Value::Object(base)), Some(ovr)) => {
            let mut merged = base.clone();
            for (key, value) in ovr {
                merged.insert(key.clone(), value.clone());
            }
            serde_json::Value::Object(merged)
        }
        (Some(base), _) => base.clone(),
        (None, _) => overrides.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_fqn_pattern() {
        let pattern = "kyc.{entity_type}.name";
        let overrides = serde_json::json!({ "entity_type": "natural_person" });
        assert_eq!(
            apply_fqn_pattern(pattern, &overrides),
            "kyc.natural_person.name"
        );
    }

    #[test]
    fn test_apply_fqn_pattern_no_match() {
        let pattern = "kyc.static_name";
        let overrides = serde_json::json!({ "entity_type": "natural_person" });
        assert_eq!(apply_fqn_pattern(pattern, &overrides), "kyc.static_name");
    }

    #[test]
    fn test_merge_payload_override_wins() {
        let default = serde_json::json!({ "data_type": "string", "required": true });
        let overrides = serde_json::json!({ "required": false, "description": "override" });
        let merged = merge_payload(Some(&default), &overrides);
        assert_eq!(merged["data_type"], "string");
        assert_eq!(merged["required"], false);
        assert_eq!(merged["description"], "override");
    }

    #[test]
    fn test_merge_payload_no_default() {
        let overrides = serde_json::json!({ "description": "only" });
        let merged = merge_payload(None, &overrides);
        assert_eq!(merged["description"], "only");
    }

}
