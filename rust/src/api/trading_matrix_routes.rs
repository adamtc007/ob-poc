//! Trading Matrix API Routes
//!
//! Provides endpoints to fetch Trading Matrix data for CBU visualization.
//! The Trading Matrix shows a hierarchical drill-down view of trading configuration:
//!
//! CBU → Instrument Classes → Markets/Counterparties → Universe Entries → Resources
//!
//! Resources include: SSIs, Booking Rules, Settlement Chains, Tax Config, ISDA/CSA

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

// =============================================================================
// API RESPONSE TYPES
// =============================================================================

/// Status color for visual indicators
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusColor {
    Green,
    Yellow,
    Red,
    Gray,
}

/// Node type with type-specific metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TradingMatrixNodeType {
    Cbu {
        cbu_id: String,
        cbu_name: String,
    },
    InstrumentClass {
        class_code: String,
        cfi_prefix: Option<String>,
        is_otc: bool,
    },
    Market {
        mic: String,
        market_name: String,
        country_code: String,
    },
    Counterparty {
        entity_id: String,
        entity_name: String,
        lei: Option<String>,
    },
    UniverseEntry {
        universe_id: String,
        currencies: Vec<String>,
        settlement_types: Vec<String>,
        is_held: bool,
        is_traded: bool,
    },
    Ssi {
        ssi_id: String,
        ssi_name: String,
        ssi_type: String,
        status: String,
        safekeeping_account: Option<String>,
        safekeeping_bic: Option<String>,
        cash_account: Option<String>,
        cash_bic: Option<String>,
    },
    BookingRule {
        rule_id: String,
        rule_name: String,
        priority: i32,
        specificity_score: i32,
        is_active: bool,
    },
    SettlementChain {
        chain_id: String,
        chain_name: String,
        hop_count: usize,
        is_active: bool,
    },
    SettlementHop {
        hop_id: String,
        sequence: i32,
        intermediary_bic: Option<String>,
        intermediary_name: Option<String>,
        role: String,
    },
    TaxConfig {
        status_id: String,
        investor_type: String,
        tax_exempt: bool,
        documentation_status: Option<String>,
    },
    TaxJurisdiction {
        jurisdiction_id: String,
        jurisdiction_code: String,
        jurisdiction_name: String,
        default_withholding_rate: Option<f64>,
        reclaim_available: bool,
    },
    IsdaAgreement {
        isda_id: String,
        counterparty_name: String,
        governing_law: Option<String>,
        agreement_date: Option<String>,
    },
    CsaAgreement {
        csa_id: String,
        csa_type: String,
        threshold_currency: Option<String>,
        threshold_amount: Option<f64>,
    },
    Category {
        name: String,
    },
}

/// A node in the trading matrix tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingMatrixNode {
    /// Unique path-based identifier
    pub id: Vec<String>,
    /// Node type with metadata
    pub node_type: TradingMatrixNodeType,
    /// Display label
    pub label: String,
    /// Optional sublabel
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sublabel: Option<String>,
    /// Child nodes
    pub children: Vec<TradingMatrixNode>,
    /// Leaf count (computed)
    pub leaf_count: usize,
    /// Status color indicator
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_color: Option<StatusColor>,
}

impl TradingMatrixNode {
    fn new(id: Vec<String>, node_type: TradingMatrixNodeType, label: &str) -> Self {
        Self {
            id,
            node_type,
            label: label.to_string(),
            sublabel: None,
            children: Vec::new(),
            leaf_count: 0,
            status_color: None,
        }
    }

    fn with_sublabel(mut self, sublabel: &str) -> Self {
        if !sublabel.is_empty() {
            self.sublabel = Some(sublabel.to_string());
        }
        self
    }

    fn with_status(mut self, status: StatusColor) -> Self {
        self.status_color = Some(status);
        self
    }
}

/// The complete trading matrix response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingMatrixResponse {
    pub cbu_id: String,
    pub cbu_name: String,
    pub children: Vec<TradingMatrixNode>,
    pub total_leaf_count: usize,
}

// =============================================================================
// DATABASE ROW TYPES
// =============================================================================

#[derive(Debug, sqlx::FromRow)]
struct UniverseEntryRow {
    universe_id: Uuid,
    instrument_class_id: Uuid,
    instrument_class_code: String,
    market_id: Option<Uuid>,
    market_mic: Option<String>,
    market_name: Option<String>,
    market_country: Option<String>,
    counterparty_id: Option<Uuid>,
    counterparty_name: Option<String>,
    currencies: Vec<String>,
    settlement_types: Option<Vec<String>>,
    is_held: Option<bool>,
    is_traded: Option<bool>,
    is_active: Option<bool>,
}

#[derive(Debug, sqlx::FromRow)]
struct SsiRow {
    ssi_id: Uuid,
    ssi_name: String,
    ssi_type: String,
    status: Option<String>,
    safekeeping_account: Option<String>,
    safekeeping_bic: Option<String>,
    cash_account: Option<String>,
    cash_bic: Option<String>,
    cash_currency: Option<String>,
    pset_bic: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct BookingRuleRow {
    rule_id: Uuid,
    ssi_id: Uuid,
    rule_name: String,
    priority: i32,
    instrument_class_id: Option<Uuid>,
    market_id: Option<Uuid>,
    currency: Option<String>,
    settlement_type: Option<String>,
    specificity_score: Option<i32>,
    is_active: Option<bool>,
}

#[derive(Debug, sqlx::FromRow)]
struct SettlementChainRow {
    chain_id: Uuid,
    chain_name: String,
    instrument_class_id: Option<Uuid>,
    market_id: Option<Uuid>,
    currency: Option<String>,
    is_active: Option<bool>,
}

#[derive(Debug, sqlx::FromRow)]
struct SettlementHopRow {
    hop_id: Uuid,
    chain_id: Uuid,
    hop_sequence: i32,
    role: String,
    intermediary_bic: Option<String>,
    intermediary_name: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct TaxJurisdictionRow {
    jurisdiction_id: Uuid,
    jurisdiction_code: String,
    jurisdiction_name: String,
    default_withholding_rate: Option<f64>,
    reclaim_available: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct TaxStatusRow {
    status_id: Uuid,
    tax_jurisdiction_id: Uuid,
    investor_type: String,
    tax_exempt: bool,
    documentation_status: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct IsdaRow {
    isda_id: Uuid,
    counterparty_entity_id: Uuid,
    counterparty_name: String,
    governing_law: Option<String>,
    agreement_date: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct CsaRow {
    csa_id: Uuid,
    isda_id: Uuid,
    csa_type: String,
    threshold_currency: Option<String>,
    threshold_amount: Option<f64>,
}

// =============================================================================
// API ENDPOINT
// =============================================================================

/// GET /api/cbu/{cbu_id}/trading-matrix
///
/// Returns the complete Trading Matrix tree for a CBU.
/// The response is a hierarchical structure suitable for drill-down visualization.
pub async fn get_trading_matrix(
    State(pool): State<PgPool>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<TradingMatrixResponse>, (StatusCode, String)> {
    // Get CBU info
    let cbu = sqlx::query!(
        r#"SELECT cbu_id, name FROM "ob-poc".cbus WHERE cbu_id = $1"#,
        cbu_id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or_else(|| (StatusCode::NOT_FOUND, format!("CBU not found: {}", cbu_id)))?;

    // Load universe entries with instrument classes and markets
    let universe_entries = sqlx::query_as!(
        UniverseEntryRow,
        r#"
        SELECT
            u.universe_id,
            u.instrument_class_id,
            ic.code as instrument_class_code,
            u.market_id,
            m.mic as market_mic,
            m.name as market_name,
            m.country_code as market_country,
            u.counterparty_entity_id as counterparty_id,
            e.name as counterparty_name,
            u.currencies,
            u.settlement_types,
            u.is_held,
            u.is_traded,
            u.is_active
        FROM custody.cbu_instrument_universe u
        JOIN custody.instrument_classes ic ON ic.class_id = u.instrument_class_id
        LEFT JOIN custody.markets m ON m.market_id = u.market_id
        LEFT JOIN "ob-poc".entities e ON e.entity_id = u.counterparty_entity_id
        WHERE u.cbu_id = $1
        ORDER BY ic.code, m.mic, e.name
        "#,
        cbu_id
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Load SSIs
    let ssis = sqlx::query_as!(
        SsiRow,
        r#"
        SELECT
            ssi_id,
            ssi_name,
            ssi_type,
            status,
            safekeeping_account,
            safekeeping_bic,
            cash_account,
            cash_account_bic as cash_bic,
            cash_currency,
            pset_bic
        FROM custody.cbu_ssi
        WHERE cbu_id = $1
        ORDER BY ssi_name
        "#,
        cbu_id
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Load booking rules
    let booking_rules = sqlx::query_as!(
        BookingRuleRow,
        r#"
        SELECT
            rule_id,
            ssi_id,
            rule_name,
            priority,
            instrument_class_id,
            market_id,
            currency,
            settlement_type,
            specificity_score,
            is_active
        FROM custody.ssi_booking_rules
        WHERE cbu_id = $1
        ORDER BY priority, rule_name
        "#,
        cbu_id
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Load settlement chains
    let chains = sqlx::query_as!(
        SettlementChainRow,
        r#"
        SELECT
            chain_id,
            chain_name,
            instrument_class_id,
            market_id,
            currency,
            is_active
        FROM custody.cbu_settlement_chains
        WHERE cbu_id = $1
        ORDER BY chain_name
        "#,
        cbu_id
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Load settlement hops
    let hops = sqlx::query_as!(
        SettlementHopRow,
        r#"
        SELECT
            h.hop_id,
            h.chain_id,
            h.hop_sequence,
            h.role,
            h.intermediary_bic,
            h.intermediary_name
        FROM custody.settlement_chain_hops h
        JOIN custody.cbu_settlement_chains c ON c.chain_id = h.chain_id
        WHERE c.cbu_id = $1
        ORDER BY h.chain_id, h.hop_sequence
        "#,
        cbu_id
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Load tax jurisdictions
    let tax_jurisdictions = sqlx::query_as!(
        TaxJurisdictionRow,
        r#"
        SELECT
            j.jurisdiction_id,
            j.jurisdiction_code,
            j.jurisdiction_name,
            j.default_withholding_rate::float8 as default_withholding_rate,
            j.reclaim_available
        FROM custody.tax_jurisdictions j
        WHERE j.jurisdiction_id IN (
            SELECT DISTINCT ts.tax_jurisdiction_id
            FROM custody.cbu_tax_status ts
            WHERE ts.cbu_id = $1
        )
        ORDER BY j.jurisdiction_code
        "#,
        cbu_id
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Load tax status
    let tax_statuses = sqlx::query_as!(
        TaxStatusRow,
        r#"
        SELECT
            status_id,
            tax_jurisdiction_id,
            investor_type,
            tax_exempt,
            documentation_status
        FROM custody.cbu_tax_status
        WHERE cbu_id = $1
        ORDER BY investor_type
        "#,
        cbu_id
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Load ISDA agreements
    let isdas = sqlx::query_as!(
        IsdaRow,
        r#"
        SELECT
            i.isda_id,
            i.counterparty_entity_id,
            e.name as counterparty_name,
            i.governing_law,
            i.agreement_date::text as agreement_date
        FROM custody.isda_agreements i
        JOIN "ob-poc".entities e ON e.entity_id = i.counterparty_entity_id
        WHERE i.cbu_id = $1
        ORDER BY e.name
        "#,
        cbu_id
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Load CSA agreements
    let csas = sqlx::query_as!(
        CsaRow,
        r#"
        SELECT
            csa_id,
            isda_id,
            csa_type,
            threshold_currency,
            threshold_amount::float8 as threshold_amount
        FROM custody.csa_agreements
        WHERE isda_id IN (SELECT isda_id FROM custody.isda_agreements WHERE cbu_id = $1)
        ORDER BY csa_type
        "#,
        cbu_id
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Build the tree structure
    let children = build_trading_matrix_tree(
        &universe_entries,
        &ssis,
        &booking_rules,
        &chains,
        &hops,
        &tax_jurisdictions,
        &tax_statuses,
        &isdas,
        &csas,
    );

    // Compute total leaf count
    let total_leaf_count = children.iter().map(|c| count_leaves(c)).sum();

    let response = TradingMatrixResponse {
        cbu_id: cbu_id.to_string(),
        cbu_name: cbu.name,
        children,
        total_leaf_count,
    };

    Ok(Json(response))
}

fn count_leaves(node: &TradingMatrixNode) -> usize {
    if node.children.is_empty() {
        1
    } else {
        node.children.iter().map(|c| count_leaves(c)).sum()
    }
}

/// Build the trading matrix tree from database rows
fn build_trading_matrix_tree(
    universe_entries: &[UniverseEntryRow],
    ssis: &[SsiRow],
    booking_rules: &[BookingRuleRow],
    chains: &[SettlementChainRow],
    hops: &[SettlementHopRow],
    tax_jurisdictions: &[TaxJurisdictionRow],
    tax_statuses: &[TaxStatusRow],
    isdas: &[IsdaRow],
    csas: &[CsaRow],
) -> Vec<TradingMatrixNode> {
    let mut result = Vec::new();

    // Group universe entries by instrument class
    let mut class_groups: HashMap<String, Vec<&UniverseEntryRow>> = HashMap::new();
    for entry in universe_entries {
        class_groups
            .entry(entry.instrument_class_code.clone())
            .or_default()
            .push(entry);
    }

    // Group booking rules by SSI
    let mut rules_by_ssi: HashMap<Uuid, Vec<&BookingRuleRow>> = HashMap::new();
    for rule in booking_rules {
        rules_by_ssi.entry(rule.ssi_id).or_default().push(rule);
    }

    // Group hops by chain
    let mut hops_by_chain: HashMap<Uuid, Vec<&SettlementHopRow>> = HashMap::new();
    for hop in hops {
        hops_by_chain.entry(hop.chain_id).or_default().push(hop);
    }

    // Group CSAs by ISDA
    let mut csas_by_isda: HashMap<Uuid, Vec<&CsaRow>> = HashMap::new();
    for csa in csas {
        csas_by_isda.entry(csa.isda_id).or_default().push(csa);
    }

    // Group tax statuses by jurisdiction
    let mut statuses_by_jurisdiction: HashMap<Uuid, Vec<&TaxStatusRow>> = HashMap::new();
    for status in tax_statuses {
        statuses_by_jurisdiction
            .entry(status.tax_jurisdiction_id)
            .or_default()
            .push(status);
    }

    // Build instrument class nodes (Trading Universe section)
    if !class_groups.is_empty() {
        let mut universe_category = TradingMatrixNode::new(
            vec!["_UNIVERSE".to_string()],
            TradingMatrixNodeType::Category {
                name: "Trading Universe".to_string(),
            },
            "Trading Universe",
        );

        for (class_code, entries) in &class_groups {
            let is_otc = class_code.starts_with("OTC_") || class_code.contains("SWAP");

            let class_id = vec!["_UNIVERSE".to_string(), class_code.clone()];
            let mut class_node = TradingMatrixNode::new(
                class_id.clone(),
                TradingMatrixNodeType::InstrumentClass {
                    class_code: class_code.clone(),
                    cfi_prefix: None,
                    is_otc,
                },
                class_code,
            );

            // Group by market or counterparty
            let mut market_groups: HashMap<String, Vec<&UniverseEntryRow>> = HashMap::new();

            for entry in entries {
                let key = if let Some(ref mic) = entry.market_mic {
                    mic.clone()
                } else if let Some(ref name) = entry.counterparty_name {
                    format!("CP:{}", name)
                } else {
                    "GLOBAL".to_string()
                };
                market_groups.entry(key).or_default().push(entry);
            }

            for (market_key, market_entries) in &market_groups {
                let first_entry = market_entries.first().unwrap();

                let mut market_id = class_id.clone();
                market_id.push(market_key.clone());

                let mut market_node = if market_key.starts_with("CP:") {
                    // Counterparty node
                    TradingMatrixNode::new(
                        market_id.clone(),
                        TradingMatrixNodeType::Counterparty {
                            entity_id: first_entry
                                .counterparty_id
                                .map(|id| id.to_string())
                                .unwrap_or_default(),
                            entity_name: first_entry.counterparty_name.clone().unwrap_or_default(),
                            lei: None,
                        },
                        first_entry
                            .counterparty_name
                            .as_deref()
                            .unwrap_or("Unknown"),
                    )
                } else {
                    // Market node
                    TradingMatrixNode::new(
                        market_id.clone(),
                        TradingMatrixNodeType::Market {
                            mic: first_entry.market_mic.clone().unwrap_or_default(),
                            market_name: first_entry.market_name.clone().unwrap_or_default(),
                            country_code: first_entry.market_country.clone().unwrap_or_default(),
                        },
                        market_key,
                    )
                    .with_sublabel(first_entry.market_name.as_deref().unwrap_or(""))
                };

                // Add universe entries under market
                for entry in market_entries {
                    let mut entry_id = market_id.clone();
                    entry_id.push(entry.universe_id.to_string());
                    let currencies_str = entry.currencies.join(", ");

                    let entry_node = TradingMatrixNode::new(
                        entry_id,
                        TradingMatrixNodeType::UniverseEntry {
                            universe_id: entry.universe_id.to_string(),
                            currencies: entry.currencies.clone(),
                            settlement_types: entry.settlement_types.clone().unwrap_or_default(),
                            is_held: entry.is_held.unwrap_or(true),
                            is_traded: entry.is_traded.unwrap_or(true),
                        },
                        &currencies_str,
                    )
                    .with_status(if entry.is_active.unwrap_or(true) {
                        StatusColor::Green
                    } else {
                        StatusColor::Gray
                    });

                    market_node.children.push(entry_node);
                }

                class_node.children.push(market_node);
            }

            universe_category.children.push(class_node);
        }

        result.push(universe_category);
    }

    // Add SSI section
    if !ssis.is_empty() {
        let mut ssi_category = TradingMatrixNode::new(
            vec!["_SSI".to_string()],
            TradingMatrixNodeType::Category {
                name: "Standing Settlement Instructions".to_string(),
            },
            "Standing Settlement Instructions",
        );

        for ssi in ssis {
            let ssi_node_id = vec!["_SSI".to_string(), ssi.ssi_id.to_string()];
            let status = match ssi.status.as_deref() {
                Some("ACTIVE") => StatusColor::Green,
                Some("PENDING") => StatusColor::Yellow,
                Some("SUSPENDED") => StatusColor::Red,
                _ => StatusColor::Gray,
            };

            let mut ssi_node = TradingMatrixNode::new(
                ssi_node_id.clone(),
                TradingMatrixNodeType::Ssi {
                    ssi_id: ssi.ssi_id.to_string(),
                    ssi_name: ssi.ssi_name.clone(),
                    ssi_type: ssi.ssi_type.clone(),
                    status: ssi.status.clone().unwrap_or_else(|| "PENDING".to_string()),
                    safekeeping_account: ssi.safekeeping_account.clone(),
                    safekeeping_bic: ssi.safekeeping_bic.clone(),
                    cash_account: ssi.cash_account.clone(),
                    cash_bic: ssi.cash_bic.clone(),
                },
                &ssi.ssi_name,
            )
            .with_sublabel(&ssi.ssi_type)
            .with_status(status);

            // Add booking rules under SSI
            if let Some(rules) = rules_by_ssi.get(&ssi.ssi_id) {
                for rule in rules {
                    let mut rule_node_id = ssi_node_id.clone();
                    rule_node_id.push(rule.rule_id.to_string());

                    let rule_node = TradingMatrixNode::new(
                        rule_node_id,
                        TradingMatrixNodeType::BookingRule {
                            rule_id: rule.rule_id.to_string(),
                            rule_name: rule.rule_name.clone(),
                            priority: rule.priority,
                            specificity_score: rule.specificity_score.unwrap_or(0),
                            is_active: rule.is_active.unwrap_or(true),
                        },
                        &rule.rule_name,
                    )
                    .with_sublabel(&format!("P{}", rule.priority))
                    .with_status(if rule.is_active.unwrap_or(true) {
                        StatusColor::Green
                    } else {
                        StatusColor::Gray
                    });

                    ssi_node.children.push(rule_node);
                }
            }

            ssi_category.children.push(ssi_node);
        }

        result.push(ssi_category);
    }

    // Add Settlement Chains section
    if !chains.is_empty() {
        let mut chain_category = TradingMatrixNode::new(
            vec!["_CHAINS".to_string()],
            TradingMatrixNodeType::Category {
                name: "Settlement Chains".to_string(),
            },
            "Settlement Chains",
        );

        for chain in chains {
            let chain_node_id = vec!["_CHAINS".to_string(), chain.chain_id.to_string()];
            let hop_count = hops_by_chain
                .get(&chain.chain_id)
                .map(|h| h.len())
                .unwrap_or(0);

            let mut chain_node = TradingMatrixNode::new(
                chain_node_id.clone(),
                TradingMatrixNodeType::SettlementChain {
                    chain_id: chain.chain_id.to_string(),
                    chain_name: chain.chain_name.clone(),
                    hop_count,
                    is_active: chain.is_active.unwrap_or(true),
                },
                &chain.chain_name,
            )
            .with_sublabel(&format!("{} hops", hop_count))
            .with_status(if chain.is_active.unwrap_or(true) {
                StatusColor::Green
            } else {
                StatusColor::Gray
            });

            // Add hops under chain
            if let Some(chain_hops) = hops_by_chain.get(&chain.chain_id) {
                for hop in chain_hops {
                    let mut hop_node_id = chain_node_id.clone();
                    hop_node_id.push(hop.hop_id.to_string());

                    let hop_label = format!(
                        "{}. {}",
                        hop.hop_sequence,
                        hop.intermediary_name.as_deref().unwrap_or(&hop.role)
                    );

                    let hop_node = TradingMatrixNode::new(
                        hop_node_id,
                        TradingMatrixNodeType::SettlementHop {
                            hop_id: hop.hop_id.to_string(),
                            sequence: hop.hop_sequence,
                            intermediary_bic: hop.intermediary_bic.clone(),
                            intermediary_name: hop.intermediary_name.clone(),
                            role: hop.role.clone(),
                        },
                        &hop_label,
                    )
                    .with_sublabel(hop.intermediary_bic.as_deref().unwrap_or(""));

                    chain_node.children.push(hop_node);
                }
            }

            chain_category.children.push(chain_node);
        }

        result.push(chain_category);
    }

    // Add Tax Configuration section
    if !tax_jurisdictions.is_empty() {
        let mut tax_category = TradingMatrixNode::new(
            vec!["_TAX".to_string()],
            TradingMatrixNodeType::Category {
                name: "Tax Configuration".to_string(),
            },
            "Tax Configuration",
        );

        for jurisdiction in tax_jurisdictions {
            let jurisdiction_node_id =
                vec!["_TAX".to_string(), jurisdiction.jurisdiction_id.to_string()];

            let rate_str = jurisdiction
                .default_withholding_rate
                .map(|r| format!("{:.1}%", r))
                .unwrap_or_else(|| "N/A".to_string());

            let mut jurisdiction_node = TradingMatrixNode::new(
                jurisdiction_node_id.clone(),
                TradingMatrixNodeType::TaxJurisdiction {
                    jurisdiction_id: jurisdiction.jurisdiction_id.to_string(),
                    jurisdiction_code: jurisdiction.jurisdiction_code.clone(),
                    jurisdiction_name: jurisdiction.jurisdiction_name.clone(),
                    default_withholding_rate: jurisdiction.default_withholding_rate,
                    reclaim_available: jurisdiction.reclaim_available,
                },
                &jurisdiction.jurisdiction_name,
            )
            .with_sublabel(&rate_str);

            // Add tax statuses for this jurisdiction
            if let Some(statuses) = statuses_by_jurisdiction.get(&jurisdiction.jurisdiction_id) {
                for status in statuses {
                    let mut status_node_id = jurisdiction_node_id.clone();
                    status_node_id.push(status.status_id.to_string());

                    let status_node = TradingMatrixNode::new(
                        status_node_id,
                        TradingMatrixNodeType::TaxConfig {
                            status_id: status.status_id.to_string(),
                            investor_type: status.investor_type.clone(),
                            tax_exempt: status.tax_exempt,
                            documentation_status: status.documentation_status.clone(),
                        },
                        &status.investor_type,
                    )
                    .with_sublabel(if status.tax_exempt {
                        "Exempt"
                    } else {
                        "Taxable"
                    })
                    .with_status(
                        match status.documentation_status.as_deref() {
                            Some("VALIDATED") => StatusColor::Green,
                            Some("SUBMITTED") => StatusColor::Yellow,
                            Some("EXPIRED") => StatusColor::Red,
                            _ => StatusColor::Gray,
                        },
                    );

                    jurisdiction_node.children.push(status_node);
                }
            }

            tax_category.children.push(jurisdiction_node);
        }

        result.push(tax_category);
    }

    // Add ISDA/CSA section
    if !isdas.is_empty() {
        let mut isda_category = TradingMatrixNode::new(
            vec!["_ISDA".to_string()],
            TradingMatrixNodeType::Category {
                name: "ISDA Agreements".to_string(),
            },
            "ISDA Agreements",
        );

        for isda in isdas {
            let isda_node_id = vec!["_ISDA".to_string(), isda.isda_id.to_string()];

            let mut isda_node = TradingMatrixNode::new(
                isda_node_id.clone(),
                TradingMatrixNodeType::IsdaAgreement {
                    isda_id: isda.isda_id.to_string(),
                    counterparty_name: isda.counterparty_name.clone(),
                    governing_law: isda.governing_law.clone(),
                    agreement_date: isda.agreement_date.clone(),
                },
                &isda.counterparty_name,
            )
            .with_sublabel(isda.governing_law.as_deref().unwrap_or(""));

            // Add CSAs under ISDA
            if let Some(isda_csas) = csas_by_isda.get(&isda.isda_id) {
                for csa in isda_csas {
                    let mut csa_node_id = isda_node_id.clone();
                    csa_node_id.push(csa.csa_id.to_string());

                    let csa_node = TradingMatrixNode::new(
                        csa_node_id,
                        TradingMatrixNodeType::CsaAgreement {
                            csa_id: csa.csa_id.to_string(),
                            csa_type: csa.csa_type.clone(),
                            threshold_currency: csa.threshold_currency.clone(),
                            threshold_amount: csa.threshold_amount,
                        },
                        &format!("{} CSA", csa.csa_type),
                    )
                    .with_sublabel(csa.threshold_currency.as_deref().unwrap_or(""));

                    isda_node.children.push(csa_node);
                }
            }

            isda_category.children.push(isda_node);
        }

        result.push(isda_category);
    }

    result
}

/// Create the trading matrix router
pub fn create_trading_matrix_router(pool: PgPool) -> Router {
    Router::new()
        .route("/api/cbu/:cbu_id/trading-matrix", get(get_trading_matrix))
        .with_state(pool)
}
