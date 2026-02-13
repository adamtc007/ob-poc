//! View definition types for the semantic registry.
//!
//! A view definition describes a context projection — a lens through
//! which a set of registry objects is presented. Views define which
//! columns (attributes) to show, how to filter, and how to sort.
//!
//! This is the semantic equivalent of a SQL view: a named, reusable
//! projection that can be applied to any entity type.

use serde::{Deserialize, Serialize};

/// Body for a view definition snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewDefBody {
    /// Fully qualified name, e.g. `"cbu.trading-overview"`
    pub fqn: String,
    /// Human-readable name
    pub name: String,
    /// Description
    pub description: String,
    /// Owning domain
    pub domain: String,
    /// The entity type this view projects over
    pub base_entity_type: String,
    /// Columns (attributes) to include in the projection
    #[serde(default)]
    pub columns: Vec<ViewColumn>,
    /// Default filters applied to this view
    #[serde(default)]
    pub filters: Vec<ViewFilter>,
    /// Default sort order
    #[serde(default)]
    pub sort_order: Vec<ViewSortField>,
}

/// A column in a view definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewColumn {
    /// Attribute FQN this column references
    pub attribute_fqn: String,
    /// Display label override (uses attribute name if absent)
    #[serde(default)]
    pub label: Option<String>,
    /// Display width hint (for UI)
    #[serde(default)]
    pub width: Option<u32>,
    /// Whether this column is visible by default
    #[serde(default = "default_true")]
    pub visible: bool,
    /// Format hint: `text`, `number`, `currency`, `date`, `boolean`, `badge`
    #[serde(default)]
    pub format: Option<String>,
}

fn default_true() -> bool {
    true
}

/// A filter condition in a view definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewFilter {
    /// Attribute FQN to filter on
    pub attribute_fqn: String,
    /// Operator: `eq`, `ne`, `in`, `not_in`, `gt`, `lt`, `is_null`, `is_not_null`
    pub operator: String,
    /// Filter value (can be a scalar, array, or null for is_null/is_not_null)
    #[serde(default)]
    pub value: Option<serde_json::Value>,
    /// Whether this filter is user-removable
    #[serde(default = "default_true")]
    pub removable: bool,
}

/// A sort field in a view definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewSortField {
    /// Attribute FQN to sort by
    pub attribute_fqn: String,
    /// Sort direction
    #[serde(default)]
    pub direction: SortDirection,
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    #[default]
    Ascending,
    Descending,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_def_serde() {
        let body = ViewDefBody {
            fqn: "cbu.trading-overview".into(),
            name: "Trading Overview".into(),
            description: "Default trading view for CBUs".into(),
            domain: "cbu".into(),
            base_entity_type: "cbu".into(),
            columns: vec![
                ViewColumn {
                    attribute_fqn: "cbu.name".into(),
                    label: Some("Structure Name".into()),
                    width: Some(200),
                    visible: true,
                    format: Some("text".into()),
                },
                ViewColumn {
                    attribute_fqn: "cbu.jurisdiction".into(),
                    label: None,
                    width: None,
                    visible: true,
                    format: Some("badge".into()),
                },
            ],
            filters: vec![ViewFilter {
                attribute_fqn: "cbu.status".into(),
                operator: "eq".into(),
                value: Some(serde_json::json!("active")),
                removable: true,
            }],
            sort_order: vec![ViewSortField {
                attribute_fqn: "cbu.name".into(),
                direction: SortDirection::Ascending,
            }],
        };
        let json = serde_json::to_value(&body).unwrap();
        let round: ViewDefBody = serde_json::from_value(json).unwrap();
        assert_eq!(round.fqn, "cbu.trading-overview");
        assert_eq!(round.columns.len(), 2);
        assert_eq!(round.filters.len(), 1);
        assert_eq!(round.sort_order.len(), 1);
    }

    #[test]
    fn test_view_column_defaults() {
        let json = serde_json::json!({
            "attribute_fqn": "cbu.name"
        });
        let col: ViewColumn = serde_json::from_value(json).unwrap();
        assert!(col.visible); // default true
        assert!(col.label.is_none());
        assert!(col.width.is_none());
        assert!(col.format.is_none());
    }

    #[test]
    fn test_sort_direction_default() {
        let field = ViewSortField {
            attribute_fqn: "cbu.name".into(),
            direction: Default::default(),
        };
        assert_eq!(field.direction, SortDirection::Ascending);
    }
}
