//! Registry metadata enrichment for SemOS attribute reconciliation.

use anyhow::Result;
use serde_json::json;
use sqlx::PgPool;

struct RegistryBridgeSeed {
    registry_id: &'static str,
    primary_fqn: &'static str,
    aliases: &'static [&'static str],
    lineage_plane: &'static str,
}

const SEMOS_BRIDGE_SEEDS: &[RegistryBridgeSeed] = &[
    RegistryBridgeSeed {
        registry_id: "attr.ubo.ownership_percentage",
        primary_fqn: "ubo.total_ownership_pct_value",
        aliases: &["ubo.direct_holding_pct", "ubo.indirect_holding_pct"],
        lineage_plane: "below_line",
    },
    RegistryBridgeSeed {
        registry_id: "attr.regulatory.risk_rating",
        primary_fqn: "risk.composite_score_value",
        aliases: &[],
        lineage_plane: "below_line",
    },
    RegistryBridgeSeed {
        registry_id: "attr.financial.aum",
        primary_fqn: "trading.aggregate_aum_value",
        aliases: &["trading.cbu_aum"],
        lineage_plane: "below_line",
    },
];

pub(crate) async fn ensure_semos_registry_bridge(pool: &PgPool) -> Result<()> {
    for seed in SEMOS_BRIDGE_SEEDS {
        let aliases = serde_json::to_value(seed.aliases)?;
        let semos_patch = json!({
            "attribute_fqn": seed.primary_fqn,
            "aliases": aliases,
            "lineage_plane": seed.lineage_plane,
            "bridge_source": "slice4_registry_enrichment",
        });

        sqlx::query(
            r#"
            UPDATE "ob-poc".attribute_registry
            SET metadata = COALESCE(metadata, '{}'::jsonb) || jsonb_build_object('sem_os', $2::jsonb),
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(seed.registry_id)
        .bind(semos_patch)
        .execute(pool)
        .await?;
    }

    Ok(())
}
