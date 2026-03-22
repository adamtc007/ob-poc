//! Pure view definition seed builders for the semantic registry.
//!
//! All functions are **pure** (no DB, no I/O). The DB-publishing orchestrator
//! remains in `ob-poc/src/sem_reg/seeds/view_seeds.rs`.

use sem_os_core::view_def::{SortDirection, ViewColumn, ViewDefBody, ViewFilter, ViewSortField};

/// Core view definitions to bootstrap.
pub fn core_views() -> Vec<ViewDefBody> {
    vec![
        trading_overview_view(),
        kyc_case_view(),
        governance_review_view(),
        entity_detail_view(),
        onboarding_view(),
        deal_management_view(),
        screening_view(),
        document_collection_view(),
    ]
}

pub fn trading_overview_view() -> ViewDefBody {
    ViewDefBody {
        fqn: "view.trading-overview".into(),
        name: "Trading Overview".into(),
        description: "Default CBU trading view showing instruments, markets, and settlement".into(),
        domain: "trading".into(),
        base_entity_type: "entity.cbu".into(),
        columns: vec![
            col("cbu.name", Some("Name"), Some(200), true, None),
            col(
                "cbu.jurisdiction_code",
                Some("Jurisdiction"),
                Some(80),
                true,
                Some("badge"),
            ),
            col("cbu.status", Some("Status"), Some(80), true, Some("badge")),
            col(
                "trading-profile.instrument_class",
                Some("Instruments"),
                Some(150),
                true,
                None,
            ),
            col(
                "trading-profile.market_code",
                Some("Markets"),
                Some(120),
                true,
                None,
            ),
            col(
                "custody.settlement_type",
                Some("Settlement"),
                Some(100),
                true,
                None,
            ),
        ],
        filters: vec![],
        sort_order: vec![ViewSortField {
            attribute_fqn: "cbu.name".into(),
            direction: SortDirection::Ascending,
        }],
        includes_operational: true,
    }
}

pub fn kyc_case_view() -> ViewDefBody {
    ViewDefBody {
        fqn: "view.kyc-case".into(),
        name: "KYC Case".into(),
        description: "KYC case management view showing ownership, screening, and document status"
            .into(),
        domain: "kyc".into(),
        base_entity_type: "entity.cbu".into(),
        columns: vec![
            col("cbu.name", Some("Structure"), Some(200), true, None),
            col(
                "kyc.case_status",
                Some("Case Status"),
                Some(100),
                true,
                Some("badge"),
            ),
            col(
                "kyc.risk_level",
                Some("Risk"),
                Some(80),
                true,
                Some("badge"),
            ),
            col(
                "kyc.ubo_coverage_pct",
                Some("UBO Coverage"),
                Some(100),
                true,
                Some("number"),
            ),
            col(
                "kyc.screening_status",
                Some("Screening"),
                Some(100),
                true,
                Some("badge"),
            ),
            col(
                "kyc.document_completeness_pct",
                Some("Documents"),
                Some(100),
                true,
                Some("number"),
            ),
        ],
        filters: vec![ViewFilter {
            attribute_fqn: "kyc.case_status".into(),
            operator: "ne".into(),
            value: Some(serde_json::json!("closed")),
            removable: true,
        }],
        sort_order: vec![ViewSortField {
            attribute_fqn: "kyc.risk_level".into(),
            direction: SortDirection::Descending,
        }],
        includes_operational: true,
    }
}

pub fn governance_review_view() -> ViewDefBody {
    ViewDefBody {
        fqn: "view.governance-review".into(),
        name: "Governance Review".into(),
        description:
            "Governance review view showing both governed and operational objects for audit".into(),
        domain: "governance".into(),
        base_entity_type: "entity.cbu".into(),
        columns: vec![
            col("cbu.name", Some("Structure"), Some(200), true, None),
            col(
                "cbu.governance_tier",
                Some("Tier"),
                Some(80),
                true,
                Some("badge"),
            ),
            col(
                "cbu.trust_class",
                Some("Trust"),
                Some(80),
                true,
                Some("badge"),
            ),
            col("cbu.steward", Some("Steward"), Some(120), true, None),
            col(
                "cbu.review_deadline",
                Some("Review Due"),
                Some(100),
                true,
                Some("date"),
            ),
        ],
        filters: vec![],
        sort_order: vec![ViewSortField {
            attribute_fqn: "cbu.review_deadline".into(),
            direction: SortDirection::Ascending,
        }],
        includes_operational: true,
    }
}

pub fn entity_detail_view() -> ViewDefBody {
    ViewDefBody {
        fqn: "view.entity-detail".into(),
        name: "Entity Detail".into(),
        description: "Detailed entity view showing all attributes and relationships".into(),
        domain: "entity".into(),
        base_entity_type: "entity.legal_entity".into(),
        columns: vec![
            col("entity.name", Some("Name"), Some(200), true, None),
            col(
                "entity.entity_type",
                Some("Type"),
                Some(100),
                true,
                Some("badge"),
            ),
            col(
                "entity.jurisdiction_code",
                Some("Jurisdiction"),
                Some(80),
                true,
                Some("badge"),
            ),
            col("entity.lei_code", Some("LEI"), Some(160), true, None),
            col(
                "entity.status",
                Some("Status"),
                Some(80),
                true,
                Some("badge"),
            ),
        ],
        filters: vec![],
        sort_order: vec![ViewSortField {
            attribute_fqn: "entity.name".into(),
            direction: SortDirection::Ascending,
        }],
        includes_operational: false,
    }
}

pub fn onboarding_view() -> ViewDefBody {
    ViewDefBody {
        fqn: "view.onboarding".into(),
        name: "Client Onboarding".into(),
        description: "Onboarding workflow view showing CBU setup progress, role assignments, and KYC readiness".into(),
        domain: "onboarding".into(),
        base_entity_type: "entity.cbu".into(),
        columns: vec![
            col("cbu.name", Some("Structure"), Some(200), true, None),
            col("cbu.status", Some("Status"), Some(80), true, Some("badge")),
            col("cbu.jurisdiction_code", Some("Jurisdiction"), Some(80), true, Some("badge")),
            col("cbu.client_type", Some("Type"), Some(100), true, None),
            col("kyc-case.status", Some("KYC Status"), Some(100), true, Some("badge")),
        ],
        filters: vec![],
        sort_order: vec![ViewSortField {
            attribute_fqn: "cbu.name".into(),
            direction: SortDirection::Ascending,
        }],
        includes_operational: true,
    }
}

pub fn deal_management_view() -> ViewDefBody {
    ViewDefBody {
        fqn: "view.deal-management".into(),
        name: "Deal Management".into(),
        description:
            "Commercial deal pipeline view showing deal status, rate cards, and onboarding progress"
                .into(),
        domain: "deal".into(),
        base_entity_type: "entity.deal".into(),
        columns: vec![
            col("deal.deal_name", Some("Deal"), Some(200), true, None),
            col(
                "deal.status",
                Some("Status"),
                Some(100),
                true,
                Some("badge"),
            ),
            col(
                "deal.sales_owner",
                Some("Sales Owner"),
                Some(120),
                true,
                None,
            ),
            col(
                "deal.created_at",
                Some("Created"),
                Some(100),
                true,
                Some("date"),
            ),
        ],
        filters: vec![],
        sort_order: vec![ViewSortField {
            attribute_fqn: "deal.created_at".into(),
            direction: SortDirection::Descending,
        }],
        includes_operational: true,
    }
}

pub fn screening_view() -> ViewDefBody {
    ViewDefBody {
        fqn: "view.screening".into(),
        name: "Screening Dashboard".into(),
        description:
            "Compliance screening view showing PEP, sanctions, and adverse media check status"
                .into(),
        domain: "screening".into(),
        base_entity_type: "entity.legal_entity".into(),
        columns: vec![
            col("entity.name", Some("Entity"), Some(200), true, None),
            col(
                "entity.entity_type",
                Some("Type"),
                Some(80),
                true,
                Some("badge"),
            ),
            col(
                "screening.pep_status",
                Some("PEP"),
                Some(80),
                true,
                Some("badge"),
            ),
            col(
                "screening.sanctions_status",
                Some("Sanctions"),
                Some(80),
                true,
                Some("badge"),
            ),
            col(
                "screening.adverse_media_status",
                Some("Media"),
                Some(80),
                true,
                Some("badge"),
            ),
        ],
        filters: vec![],
        sort_order: vec![ViewSortField {
            attribute_fqn: "entity.name".into(),
            direction: SortDirection::Ascending,
        }],
        includes_operational: true,
    }
}

pub fn document_collection_view() -> ViewDefBody {
    ViewDefBody {
        fqn: "view.document-collection".into(),
        name: "Document Collection".into(),
        description: "Document solicitation view showing outstanding requirements, submissions, and QA status".into(),
        domain: "document".into(),
        base_entity_type: "entity.legal_entity".into(),
        columns: vec![
            col("entity.name", Some("Entity"), Some(200), true, None),
            col("document.doc_type", Some("Document Type"), Some(150), true, None),
            col("document.status", Some("Status"), Some(80), true, Some("badge")),
            col("document.requested_at", Some("Requested"), Some(100), true, Some("date")),
        ],
        filters: vec![],
        sort_order: vec![ViewSortField {
            attribute_fqn: "entity.name".into(),
            direction: SortDirection::Ascending,
        }],
        includes_operational: true,
    }
}

/// Helper to build a view column.
fn col(
    attribute_fqn: &str,
    label: Option<&str>,
    width: Option<u32>,
    visible: bool,
    format: Option<&str>,
) -> ViewColumn {
    ViewColumn {
        attribute_fqn: attribute_fqn.into(),
        label: label.map(Into::into),
        width,
        visible,
        format: format.map(Into::into),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_views_well_formed() {
        let views = core_views();
        assert_eq!(views.len(), 8, "Expected 8 core views");

        for v in &views {
            assert!(v.fqn.starts_with("view."), "Bad FQN: {}", v.fqn);
            assert!(!v.name.is_empty());
            assert!(!v.description.is_empty());
            assert!(!v.base_entity_type.is_empty());
            assert!(!v.columns.is_empty(), "View {} has no columns", v.fqn);
        }
    }

    #[test]
    fn test_cbu_instance_views_include_operational() {
        let views = core_views();
        for fqn in [
            "view.trading-overview",
            "view.kyc-case",
            "view.governance-review",
        ] {
            let view = views
                .iter()
                .find(|v| v.fqn == fqn)
                .unwrap_or_else(|| panic!("missing {fqn}"));
            assert!(
                view.includes_operational,
                "View {} should include operational tier",
                fqn
            );
        }

        let entity_detail = views
            .iter()
            .find(|v| v.fqn == "view.entity-detail")
            .unwrap();
        assert!(
            !entity_detail.includes_operational,
            "Entity detail should remain governed-only"
        );
    }

    #[test]
    fn test_trading_view_columns() {
        let view = trading_overview_view();
        assert_eq!(view.columns.len(), 6);
        assert_eq!(view.columns[0].attribute_fqn, "cbu.name");
        assert!(view.columns[0].visible);
    }

    #[test]
    fn test_view_serde_round_trip() {
        for v in &core_views() {
            let json = serde_json::to_value(v).unwrap();
            let back: ViewDefBody = serde_json::from_value(json).unwrap();
            assert_eq!(back.fqn, v.fqn);
            assert_eq!(back.columns.len(), v.columns.len());
            assert_eq!(back.includes_operational, v.includes_operational);
        }
    }
}
