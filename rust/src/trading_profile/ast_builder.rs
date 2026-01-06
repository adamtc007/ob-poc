//! AST Builder for TradingMatrixDocument
//!
//! This module provides functions to build the TradingMatrixDocument AST incrementally.
//! Each function corresponds to a DSL verb operation and modifies the tree in place.
//!
//! ## Design Philosophy
//!
//! The document IS the AST. These functions:
//! 1. Take a mutable reference to `TradingMatrixDocument`
//! 2. Apply a semantic operation (add node, remove node, update status)
//! 3. Return a result indicating success or the specific error
//!
//! No SQL tables are touched - all state lives in the document.

use ob_poc_types::trading_matrix::{
    categories, BookingMatchCriteria, StatusColor, TradingMatrixDocument, TradingMatrixNode,
    TradingMatrixNodeId, TradingMatrixNodeType, TradingMatrixOp,
};

/// Errors that can occur during AST building
#[derive(Debug, thiserror::Error)]
pub enum AstBuildError {
    #[error("Node already exists: {path}")]
    NodeAlreadyExists { path: String },

    #[error("Node not found: {path}")]
    NodeNotFound { path: String },

    #[error("Parent node not found: {path}")]
    ParentNotFound { path: String },

    #[error("Invalid operation: {message}")]
    InvalidOperation { message: String },

    #[error("Reference not found: {ref_type} '{ref_value}'")]
    ReferenceNotFound { ref_type: String, ref_value: String },
}

/// Result type for AST building operations
pub type AstBuildResult<T> = Result<T, AstBuildError>;

// ============================================================================
// DOCUMENT LIFECYCLE
// ============================================================================

/// Create a new empty TradingMatrixDocument for a CBU
pub fn create_document(cbu_id: &str, cbu_name: &str) -> TradingMatrixDocument {
    let mut doc = TradingMatrixDocument::new(cbu_id, cbu_name);
    doc.created_at = Some(chrono::Utc::now().to_rfc3339());
    doc.updated_at = doc.created_at.clone();
    doc
}

/// Initialize standard categories in a document
pub fn initialize_categories(doc: &mut TradingMatrixDocument) {
    doc.ensure_category(categories::UNIVERSE);
    doc.ensure_category(categories::SSI);
    doc.ensure_category(categories::CHAINS);
    doc.ensure_category(categories::TAX);
    doc.ensure_category(categories::ISDA);
    doc.ensure_category(categories::PRICING);
    doc.ensure_category(categories::MANAGERS);
}

/// Mark document as modified
fn mark_modified(doc: &mut TradingMatrixDocument) {
    doc.updated_at = Some(chrono::Utc::now().to_rfc3339());
}

// ============================================================================
// OPERATION DISPATCHER
// ============================================================================

/// Apply a TradingMatrixOp to the document
pub fn apply_op(doc: &mut TradingMatrixDocument, op: TradingMatrixOp) -> AstBuildResult<()> {
    match op {
        TradingMatrixOp::AddInstrumentClass {
            class_code,
            cfi_prefix,
            is_otc,
        } => add_instrument_class(doc, &class_code, cfi_prefix.as_deref(), is_otc),

        TradingMatrixOp::AddMarket {
            parent_class,
            mic,
            market_name,
            country_code,
        } => add_market(doc, &parent_class, &mic, &market_name, &country_code),

        TradingMatrixOp::AddCounterparty {
            parent_class,
            entity_id,
            entity_name,
            lei,
        } => add_counterparty(doc, &parent_class, &entity_id, &entity_name, lei.as_deref()),

        TradingMatrixOp::AddUniverseEntry {
            parent_class,
            parent_market_or_counterparty,
            universe_id,
            currencies,
            settlement_types,
            is_held,
            is_traded,
        } => add_universe_entry(
            doc,
            &parent_class,
            &parent_market_or_counterparty,
            &universe_id,
            currencies,
            settlement_types,
            is_held,
            is_traded,
        ),

        TradingMatrixOp::AddSsi {
            ssi_id,
            ssi_name,
            ssi_type,
            safekeeping_account,
            safekeeping_bic,
            cash_account,
            cash_bic,
            cash_currency,
            pset_bic,
        } => add_ssi(
            doc,
            &ssi_id,
            &ssi_name,
            &ssi_type,
            safekeeping_account.as_deref(),
            safekeeping_bic.as_deref(),
            cash_account.as_deref(),
            cash_bic.as_deref(),
            cash_currency.as_deref(),
            pset_bic.as_deref(),
        ),

        TradingMatrixOp::ActivateSsi { ssi_id } => set_ssi_status(doc, &ssi_id, "ACTIVE"),

        TradingMatrixOp::SuspendSsi { ssi_id } => set_ssi_status(doc, &ssi_id, "SUSPENDED"),

        TradingMatrixOp::AddBookingRule {
            ssi_ref,
            rule_id,
            rule_name,
            priority,
            match_criteria,
        } => add_booking_rule(
            doc,
            &ssi_ref,
            &rule_id,
            &rule_name,
            priority,
            match_criteria,
        ),

        TradingMatrixOp::AddSettlementChain {
            chain_id,
            chain_name,
            mic,
            currency,
        } => add_settlement_chain(
            doc,
            &chain_id,
            &chain_name,
            mic.as_deref(),
            currency.as_deref(),
        ),

        TradingMatrixOp::AddSettlementHop {
            chain_ref,
            hop_id,
            sequence,
            role,
            intermediary_bic,
            intermediary_name,
        } => add_settlement_hop(
            doc,
            &chain_ref,
            &hop_id,
            sequence,
            &role,
            intermediary_bic.as_deref(),
            intermediary_name.as_deref(),
        ),

        TradingMatrixOp::AddIsda {
            isda_id,
            counterparty_entity_id,
            counterparty_name,
            counterparty_lei,
            governing_law,
            agreement_date,
        } => add_isda(
            doc,
            &isda_id,
            &counterparty_entity_id,
            &counterparty_name,
            counterparty_lei.as_deref(),
            governing_law.as_deref(),
            agreement_date.as_deref(),
        ),

        TradingMatrixOp::AddCsa {
            isda_ref,
            csa_id,
            csa_type,
            threshold_currency,
            threshold_amount,
            minimum_transfer_amount,
            collateral_ssi_ref,
        } => add_csa(
            doc,
            &isda_ref,
            &csa_id,
            &csa_type,
            threshold_currency.as_deref(),
            threshold_amount,
            minimum_transfer_amount,
            collateral_ssi_ref.as_deref(),
        ),

        TradingMatrixOp::AddProductCoverage {
            isda_ref,
            coverage_id,
            asset_class,
            base_products,
        } => add_product_coverage(doc, &isda_ref, &coverage_id, &asset_class, base_products),

        TradingMatrixOp::AddTaxJurisdiction {
            jurisdiction_id,
            jurisdiction_code,
            jurisdiction_name,
            default_withholding_rate,
            reclaim_available,
        } => add_tax_jurisdiction(
            doc,
            &jurisdiction_id,
            &jurisdiction_code,
            &jurisdiction_name,
            default_withholding_rate,
            reclaim_available,
        ),

        TradingMatrixOp::AddTaxConfig {
            jurisdiction_ref,
            status_id,
            investor_type,
            tax_exempt,
            documentation_status,
            treaty_rate,
        } => add_tax_config(
            doc,
            &jurisdiction_ref,
            &status_id,
            &investor_type,
            tax_exempt,
            documentation_status.as_deref(),
            treaty_rate,
        ),

        TradingMatrixOp::AddImMandate {
            manager_id,
            manager_entity_id,
            manager_name,
            manager_lei,
            priority,
            role,
            can_trade,
            can_settle,
            scope_instrument_classes,
            scope_markets,
            scope_currencies,
        } => add_im_mandate(
            doc,
            &manager_id,
            &manager_entity_id,
            &manager_name,
            manager_lei.as_deref(),
            priority,
            &role,
            can_trade,
            can_settle,
            scope_instrument_classes,
            scope_markets,
            scope_currencies,
        ),

        TradingMatrixOp::UpdateImScope {
            manager_ref,
            scope_instrument_classes,
            scope_markets,
            scope_currencies,
        } => update_im_scope(
            doc,
            &manager_ref,
            scope_instrument_classes,
            scope_markets,
            scope_currencies,
        ),

        TradingMatrixOp::AddCsaEligibleCollateral {
            isda_ref,
            csa_ref,
            collateral_id,
            collateral_type,
            currency,
            haircut_pct,
            concentration_limit_pct,
        } => add_csa_eligible_collateral(
            doc,
            &isda_ref,
            &csa_ref,
            &collateral_id,
            &collateral_type,
            currency.as_deref(),
            haircut_pct,
            concentration_limit_pct,
        ),

        TradingMatrixOp::LinkCsaSsi {
            isda_ref,
            csa_ref,
            ssi_ref,
        } => link_csa_ssi(doc, &isda_ref, &csa_ref, &ssi_ref),

        TradingMatrixOp::SetBaseCurrency { currency } => set_base_currency(doc, &currency),

        TradingMatrixOp::AddAllowedCurrency { currency } => add_allowed_currency(doc, &currency),

        TradingMatrixOp::RemoveNode { node_id } => remove_node(doc, &node_id),

        TradingMatrixOp::SetNodeStatus { node_id, status } => {
            set_node_status(doc, &node_id, status)
        }
    }
}

// ============================================================================
// UNIVERSE OPERATIONS
// ============================================================================

/// Add an instrument class to the Trading Universe category
pub fn add_instrument_class(
    doc: &mut TradingMatrixDocument,
    class_code: &str,
    cfi_prefix: Option<&str>,
    is_otc: bool,
) -> AstBuildResult<()> {
    let universe = doc.ensure_category(categories::UNIVERSE);
    let node_id = universe.id.child(class_code);

    // Check if already exists
    if universe.children.iter().any(|c| c.id == node_id) {
        return Err(AstBuildError::NodeAlreadyExists {
            path: format!("{}/{}", categories::UNIVERSE, class_code),
        });
    }

    let node = TradingMatrixNode::new(
        node_id,
        TradingMatrixNodeType::InstrumentClass {
            class_code: class_code.to_string(),
            cfi_prefix: cfi_prefix.map(|s| s.to_string()),
            is_otc,
        },
        class_code,
    )
    .with_sublabel(if is_otc { "OTC" } else { "Exchange Traded" });

    universe.add_child(node);
    mark_modified(doc);
    Ok(())
}

/// Add a market under an instrument class
pub fn add_market(
    doc: &mut TradingMatrixDocument,
    parent_class: &str,
    mic: &str,
    market_name: &str,
    country_code: &str,
) -> AstBuildResult<()> {
    let universe = doc.ensure_category(categories::UNIVERSE);
    let parent_id = universe.id.child(parent_class);

    let parent = universe
        .children
        .iter_mut()
        .find(|c| c.id == parent_id)
        .ok_or_else(|| AstBuildError::ParentNotFound {
            path: format!("{}/{}", categories::UNIVERSE, parent_class),
        })?;

    let node_id = parent_id.child(mic);

    // Check if already exists
    if parent.children.iter().any(|c| c.id == node_id) {
        return Err(AstBuildError::NodeAlreadyExists {
            path: format!("{}/{}/{}", categories::UNIVERSE, parent_class, mic),
        });
    }

    let node = TradingMatrixNode::new(
        node_id,
        TradingMatrixNodeType::Market {
            mic: mic.to_string(),
            market_name: market_name.to_string(),
            country_code: country_code.to_string(),
        },
        mic,
    )
    .with_sublabel(market_name);

    parent.add_child(node);
    mark_modified(doc);
    Ok(())
}

/// Add a counterparty under an instrument class (for OTC)
pub fn add_counterparty(
    doc: &mut TradingMatrixDocument,
    parent_class: &str,
    entity_id: &str,
    entity_name: &str,
    lei: Option<&str>,
) -> AstBuildResult<()> {
    let universe = doc.ensure_category(categories::UNIVERSE);
    let parent_id = universe.id.child(parent_class);

    let parent = universe
        .children
        .iter_mut()
        .find(|c| c.id == parent_id)
        .ok_or_else(|| AstBuildError::ParentNotFound {
            path: format!("{}/{}", categories::UNIVERSE, parent_class),
        })?;

    // Use entity_id as the path segment (not name, since names can have special chars)
    let node_id = parent_id.child(entity_id);

    // Check if already exists
    if parent.children.iter().any(|c| c.id == node_id) {
        return Err(AstBuildError::NodeAlreadyExists {
            path: format!("{}/{}/{}", categories::UNIVERSE, parent_class, entity_name),
        });
    }

    let node = TradingMatrixNode::new(
        node_id,
        TradingMatrixNodeType::Counterparty {
            entity_id: entity_id.to_string(),
            entity_name: entity_name.to_string(),
            lei: lei.map(|s| s.to_string()),
        },
        entity_name,
    )
    .with_sublabel(lei.unwrap_or(""));

    parent.add_child(node);
    mark_modified(doc);
    Ok(())
}

/// Add a universe entry under a market or counterparty
#[allow(clippy::too_many_arguments)]
pub fn add_universe_entry(
    doc: &mut TradingMatrixDocument,
    parent_class: &str,
    parent_market_or_counterparty: &str,
    universe_id: &str,
    currencies: Vec<String>,
    settlement_types: Vec<String>,
    is_held: bool,
    is_traded: bool,
) -> AstBuildResult<()> {
    let universe = doc.ensure_category(categories::UNIVERSE);
    let class_id = universe.id.child(parent_class);

    let class_node = universe
        .children
        .iter_mut()
        .find(|c| c.id == class_id)
        .ok_or_else(|| AstBuildError::ParentNotFound {
            path: format!("{}/{}", categories::UNIVERSE, parent_class),
        })?;

    let parent_id = class_id.child(parent_market_or_counterparty);

    let parent = class_node
        .children
        .iter_mut()
        .find(|c| c.id == parent_id)
        .ok_or_else(|| AstBuildError::ParentNotFound {
            path: format!(
                "{}/{}/{}",
                categories::UNIVERSE,
                parent_class,
                parent_market_or_counterparty
            ),
        })?;

    let node_id = parent_id.child(universe_id);

    // Check if already exists
    if parent.children.iter().any(|c| c.id == node_id) {
        return Err(AstBuildError::NodeAlreadyExists {
            path: format!(
                "{}/{}/{}/{}",
                categories::UNIVERSE,
                parent_class,
                parent_market_or_counterparty,
                universe_id
            ),
        });
    }

    let label = currencies.join(", ");
    let sublabel = settlement_types.join(", ");

    let node = TradingMatrixNode::new(
        node_id,
        TradingMatrixNodeType::UniverseEntry {
            universe_id: universe_id.to_string(),
            currencies,
            settlement_types,
            is_held,
            is_traded,
        },
        label,
    )
    .with_sublabel(sublabel)
    .with_status(if is_traded {
        StatusColor::Green
    } else {
        StatusColor::Gray
    });

    parent.add_child(node);
    mark_modified(doc);
    Ok(())
}

// ============================================================================
// SSI OPERATIONS
// ============================================================================

/// Add a Standing Settlement Instruction
#[allow(clippy::too_many_arguments)]
pub fn add_ssi(
    doc: &mut TradingMatrixDocument,
    ssi_id: &str,
    ssi_name: &str,
    ssi_type: &str,
    safekeeping_account: Option<&str>,
    safekeeping_bic: Option<&str>,
    cash_account: Option<&str>,
    cash_bic: Option<&str>,
    cash_currency: Option<&str>,
    pset_bic: Option<&str>,
) -> AstBuildResult<()> {
    let ssi_category = doc.ensure_category(categories::SSI);
    let node_id = ssi_category.id.child(ssi_name);

    // Check if already exists
    if ssi_category.children.iter().any(|c| c.id == node_id) {
        return Err(AstBuildError::NodeAlreadyExists {
            path: format!("{}/{}", categories::SSI, ssi_name),
        });
    }

    let sublabel = match (safekeeping_bic, cash_currency) {
        (Some(bic), Some(ccy)) => format!("{} / {}", bic, ccy),
        (Some(bic), None) => bic.to_string(),
        (None, Some(ccy)) => ccy.to_string(),
        (None, None) => String::new(),
    };

    let node = TradingMatrixNode::new(
        node_id,
        TradingMatrixNodeType::Ssi {
            ssi_id: ssi_id.to_string(),
            ssi_name: ssi_name.to_string(),
            ssi_type: ssi_type.to_string(),
            status: "PENDING".to_string(),
            safekeeping_account: safekeeping_account.map(|s| s.to_string()),
            safekeeping_bic: safekeeping_bic.map(|s| s.to_string()),
            cash_account: cash_account.map(|s| s.to_string()),
            cash_bic: cash_bic.map(|s| s.to_string()),
            pset_bic: pset_bic.map(|s| s.to_string()),
            cash_currency: cash_currency.map(|s| s.to_string()),
        },
        ssi_name,
    )
    .with_sublabel(sublabel)
    .with_status(StatusColor::Yellow); // PENDING

    ssi_category.add_child(node);
    mark_modified(doc);
    Ok(())
}

/// Set SSI status (activate or suspend)
fn set_ssi_status(
    doc: &mut TradingMatrixDocument,
    ssi_id: &str,
    status: &str,
) -> AstBuildResult<()> {
    let ssi_category = doc.ensure_category(categories::SSI);

    // Find SSI by ID in node_type
    let ssi_node = ssi_category.children.iter_mut().find(
        |c| matches!(&c.node_type, TradingMatrixNodeType::Ssi { ssi_id: id, .. } if id == ssi_id),
    );

    let Some(node) = ssi_node else {
        return Err(AstBuildError::NodeNotFound {
            path: format!("SSI with id {}", ssi_id),
        });
    };

    // Update status in node_type
    if let TradingMatrixNodeType::Ssi {
        status: ref mut s, ..
    } = node.node_type
    {
        *s = status.to_string();
    }

    // Update visual status
    node.status_color = Some(match status {
        "ACTIVE" => StatusColor::Green,
        "SUSPENDED" => StatusColor::Red,
        _ => StatusColor::Yellow,
    });

    mark_modified(doc);
    Ok(())
}

/// Add a booking rule under an SSI (referenced by name)
pub fn add_booking_rule(
    doc: &mut TradingMatrixDocument,
    ssi_ref: &str,
    rule_id: &str,
    rule_name: &str,
    priority: i32,
    match_criteria: BookingMatchCriteria,
) -> AstBuildResult<()> {
    let ssi_category = doc.ensure_category(categories::SSI);
    let parent_id = ssi_category.id.child(ssi_ref);

    let parent = ssi_category
        .children
        .iter_mut()
        .find(|c| c.id == parent_id)
        .ok_or_else(|| AstBuildError::ReferenceNotFound {
            ref_type: "SSI".to_string(),
            ref_value: ssi_ref.to_string(),
        })?;

    let node_id = parent_id.child(rule_id);

    // Check if already exists
    if parent.children.iter().any(|c| c.id == node_id) {
        return Err(AstBuildError::NodeAlreadyExists {
            path: format!("{}/{}/{}", categories::SSI, ssi_ref, rule_name),
        });
    }

    // Compute specificity score (count of non-None criteria)
    let specificity_score = [
        match_criteria.instrument_class.is_some(),
        match_criteria.security_type.is_some(),
        match_criteria.mic.is_some(),
        match_criteria.currency.is_some(),
        match_criteria.settlement_type.is_some(),
        match_criteria.counterparty_entity_id.is_some(),
    ]
    .iter()
    .filter(|&&b| b)
    .count() as i32;

    let sublabel = format!("Priority: {}, Specificity: {}", priority, specificity_score);

    let node = TradingMatrixNode::new(
        node_id,
        TradingMatrixNodeType::BookingRule {
            rule_id: rule_id.to_string(),
            rule_name: rule_name.to_string(),
            priority,
            specificity_score,
            is_active: true,
            match_criteria: Some(match_criteria),
        },
        rule_name,
    )
    .with_sublabel(sublabel)
    .with_status(StatusColor::Green);

    parent.add_child(node);
    mark_modified(doc);
    Ok(())
}

// ============================================================================
// SETTLEMENT CHAIN OPERATIONS
// ============================================================================

/// Add a settlement chain
pub fn add_settlement_chain(
    doc: &mut TradingMatrixDocument,
    chain_id: &str,
    chain_name: &str,
    mic: Option<&str>,
    currency: Option<&str>,
) -> AstBuildResult<()> {
    let chains = doc.ensure_category(categories::CHAINS);
    let node_id = chains.id.child(chain_name);

    // Check if already exists
    if chains.children.iter().any(|c| c.id == node_id) {
        return Err(AstBuildError::NodeAlreadyExists {
            path: format!("{}/{}", categories::CHAINS, chain_name),
        });
    }

    let sublabel = match (mic, currency) {
        (Some(m), Some(c)) => format!("{} / {}", m, c),
        (Some(m), None) => m.to_string(),
        (None, Some(c)) => c.to_string(),
        (None, None) => String::new(),
    };

    let node = TradingMatrixNode::new(
        node_id,
        TradingMatrixNodeType::SettlementChain {
            chain_id: chain_id.to_string(),
            chain_name: chain_name.to_string(),
            hop_count: 0,
            is_active: true,
            mic: mic.map(|s| s.to_string()),
            currency: currency.map(|s| s.to_string()),
        },
        chain_name,
    )
    .with_sublabel(sublabel)
    .with_status(StatusColor::Green);

    chains.add_child(node);
    mark_modified(doc);
    Ok(())
}

/// Add a hop to a settlement chain
pub fn add_settlement_hop(
    doc: &mut TradingMatrixDocument,
    chain_ref: &str,
    hop_id: &str,
    sequence: i32,
    role: &str,
    intermediary_bic: Option<&str>,
    intermediary_name: Option<&str>,
) -> AstBuildResult<()> {
    let chains = doc.ensure_category(categories::CHAINS);
    let parent_id = chains.id.child(chain_ref);

    let parent = chains
        .children
        .iter_mut()
        .find(|c| c.id == parent_id)
        .ok_or_else(|| AstBuildError::ReferenceNotFound {
            ref_type: "SettlementChain".to_string(),
            ref_value: chain_ref.to_string(),
        })?;

    let node_id = parent_id.child(hop_id);

    // Check if already exists
    if parent.children.iter().any(|c| c.id == node_id) {
        return Err(AstBuildError::NodeAlreadyExists {
            path: format!("{}/{}/{}", categories::CHAINS, chain_ref, hop_id),
        });
    }

    let label = format!("Hop {} - {}", sequence, role);
    let sublabel = intermediary_bic
        .or(intermediary_name)
        .unwrap_or("")
        .to_string();

    let node = TradingMatrixNode::new(
        node_id,
        TradingMatrixNodeType::SettlementHop {
            hop_id: hop_id.to_string(),
            sequence,
            intermediary_bic: intermediary_bic.map(|s| s.to_string()),
            intermediary_name: intermediary_name.map(|s| s.to_string()),
            role: role.to_string(),
        },
        label,
    )
    .with_sublabel(sublabel);

    parent.add_child(node);

    // Update hop count in parent
    if let TradingMatrixNodeType::SettlementChain {
        ref mut hop_count, ..
    } = parent.node_type
    {
        *hop_count = parent.children.len();
    }

    mark_modified(doc);
    Ok(())
}

// ============================================================================
// ISDA OPERATIONS
// ============================================================================

/// Add an ISDA agreement
#[allow(clippy::too_many_arguments)]
pub fn add_isda(
    doc: &mut TradingMatrixDocument,
    isda_id: &str,
    counterparty_entity_id: &str,
    counterparty_name: &str,
    counterparty_lei: Option<&str>,
    governing_law: Option<&str>,
    agreement_date: Option<&str>,
) -> AstBuildResult<()> {
    let isda_category = doc.ensure_category(categories::ISDA);
    let node_id = isda_category.id.child(counterparty_name);

    // Check if already exists
    if isda_category.children.iter().any(|c| c.id == node_id) {
        return Err(AstBuildError::NodeAlreadyExists {
            path: format!("{}/{}", categories::ISDA, counterparty_name),
        });
    }

    let sublabel = governing_law
        .map(|gl| format!("{} Law", gl))
        .unwrap_or_default();

    let node = TradingMatrixNode::new(
        node_id,
        TradingMatrixNodeType::IsdaAgreement {
            isda_id: isda_id.to_string(),
            counterparty_name: counterparty_name.to_string(),
            governing_law: governing_law.map(|s| s.to_string()),
            agreement_date: agreement_date.map(|s| s.to_string()),
            counterparty_entity_id: Some(counterparty_entity_id.to_string()),
            counterparty_lei: counterparty_lei.map(|s| s.to_string()),
        },
        counterparty_name,
    )
    .with_sublabel(sublabel)
    .with_status(StatusColor::Green);

    isda_category.add_child(node);
    mark_modified(doc);
    Ok(())
}

/// Add a CSA under an ISDA (referenced by counterparty name)
#[allow(clippy::too_many_arguments)]
pub fn add_csa(
    doc: &mut TradingMatrixDocument,
    isda_ref: &str,
    csa_id: &str,
    csa_type: &str,
    threshold_currency: Option<&str>,
    threshold_amount: Option<f64>,
    minimum_transfer_amount: Option<f64>,
    collateral_ssi_ref: Option<&str>,
) -> AstBuildResult<()> {
    let isda_category = doc.ensure_category(categories::ISDA);
    let parent_id = isda_category.id.child(isda_ref);

    let parent = isda_category
        .children
        .iter_mut()
        .find(|c| c.id == parent_id)
        .ok_or_else(|| AstBuildError::ReferenceNotFound {
            ref_type: "ISDA".to_string(),
            ref_value: isda_ref.to_string(),
        })?;

    let node_id = parent_id.child(csa_id);

    // Check if already exists
    if parent.children.iter().any(|c| c.id == node_id) {
        return Err(AstBuildError::NodeAlreadyExists {
            path: format!("{}/{}/{}", categories::ISDA, isda_ref, csa_type),
        });
    }

    let sublabel = match (threshold_currency.as_ref(), threshold_amount) {
        (Some(ccy), Some(amt)) => format!("{} {} threshold", ccy, amt),
        _ => String::new(),
    };

    let node = TradingMatrixNode::new(
        node_id,
        TradingMatrixNodeType::CsaAgreement {
            csa_id: csa_id.to_string(),
            csa_type: csa_type.to_string(),
            threshold_currency: threshold_currency.map(|s| s.to_string()),
            threshold_amount,
            minimum_transfer_amount,
            collateral_ssi_ref: collateral_ssi_ref.map(|s| s.to_string()),
        },
        format!("{} CSA", csa_type),
    )
    .with_sublabel(sublabel)
    .with_status(StatusColor::Green);

    parent.add_child(node);
    mark_modified(doc);
    Ok(())
}

/// Add product coverage to an ISDA
pub fn add_product_coverage(
    doc: &mut TradingMatrixDocument,
    isda_ref: &str,
    coverage_id: &str,
    asset_class: &str,
    base_products: Vec<String>,
) -> AstBuildResult<()> {
    let isda_category = doc.ensure_category(categories::ISDA);
    let parent_id = isda_category.id.child(isda_ref);

    let parent = isda_category
        .children
        .iter_mut()
        .find(|c| c.id == parent_id)
        .ok_or_else(|| AstBuildError::ReferenceNotFound {
            ref_type: "ISDA".to_string(),
            ref_value: isda_ref.to_string(),
        })?;

    let node_id = parent_id.child(coverage_id);

    // Check if already exists
    if parent.children.iter().any(|c| c.id == node_id) {
        return Err(AstBuildError::NodeAlreadyExists {
            path: format!("{}/{}/{}", categories::ISDA, isda_ref, asset_class),
        });
    }

    let sublabel = base_products.join(", ");

    let node = TradingMatrixNode::new(
        node_id,
        TradingMatrixNodeType::ProductCoverage {
            coverage_id: coverage_id.to_string(),
            asset_class: asset_class.to_string(),
            base_products,
        },
        asset_class,
    )
    .with_sublabel(sublabel);

    parent.add_child(node);
    mark_modified(doc);
    Ok(())
}

// ============================================================================
// TAX OPERATIONS
// ============================================================================

/// Add a tax jurisdiction
pub fn add_tax_jurisdiction(
    doc: &mut TradingMatrixDocument,
    jurisdiction_id: &str,
    jurisdiction_code: &str,
    jurisdiction_name: &str,
    default_withholding_rate: Option<f64>,
    reclaim_available: bool,
) -> AstBuildResult<()> {
    let tax = doc.ensure_category(categories::TAX);
    let node_id = tax.id.child(jurisdiction_code);

    // Check if already exists
    if tax.children.iter().any(|c| c.id == node_id) {
        return Err(AstBuildError::NodeAlreadyExists {
            path: format!("{}/{}", categories::TAX, jurisdiction_code),
        });
    }

    let sublabel = match default_withholding_rate {
        Some(rate) => format!("{}% WHT", rate),
        None => String::new(),
    };

    let node = TradingMatrixNode::new(
        node_id,
        TradingMatrixNodeType::TaxJurisdiction {
            jurisdiction_id: jurisdiction_id.to_string(),
            jurisdiction_code: jurisdiction_code.to_string(),
            jurisdiction_name: jurisdiction_name.to_string(),
            default_withholding_rate,
            reclaim_available,
        },
        jurisdiction_name,
    )
    .with_sublabel(sublabel)
    .with_status(if reclaim_available {
        StatusColor::Green
    } else {
        StatusColor::Gray
    });

    tax.add_child(node);
    mark_modified(doc);
    Ok(())
}

/// Add tax config under a jurisdiction
pub fn add_tax_config(
    doc: &mut TradingMatrixDocument,
    jurisdiction_ref: &str,
    status_id: &str,
    investor_type: &str,
    tax_exempt: bool,
    documentation_status: Option<&str>,
    treaty_rate: Option<f64>,
) -> AstBuildResult<()> {
    let tax = doc.ensure_category(categories::TAX);
    let parent_id = tax.id.child(jurisdiction_ref);

    let parent = tax
        .children
        .iter_mut()
        .find(|c| c.id == parent_id)
        .ok_or_else(|| AstBuildError::ReferenceNotFound {
            ref_type: "TaxJurisdiction".to_string(),
            ref_value: jurisdiction_ref.to_string(),
        })?;

    let node_id = parent_id.child(status_id);

    // Check if already exists
    if parent.children.iter().any(|c| c.id == node_id) {
        return Err(AstBuildError::NodeAlreadyExists {
            path: format!("{}/{}/{}", categories::TAX, jurisdiction_ref, investor_type),
        });
    }

    let sublabel = match (tax_exempt, treaty_rate) {
        (true, _) => "Exempt".to_string(),
        (false, Some(rate)) => format!("{}% treaty rate", rate),
        (false, None) => "Standard rate".to_string(),
    };

    let status = match documentation_status {
        Some("VALIDATED") => StatusColor::Green,
        Some("SUBMITTED") => StatusColor::Yellow,
        Some("EXPIRED") => StatusColor::Red,
        _ => StatusColor::Gray,
    };

    let node = TradingMatrixNode::new(
        node_id,
        TradingMatrixNodeType::TaxConfig {
            status_id: status_id.to_string(),
            investor_type: investor_type.to_string(),
            tax_exempt,
            documentation_status: documentation_status.map(|s| s.to_string()),
            treaty_rate,
        },
        investor_type,
    )
    .with_sublabel(sublabel)
    .with_status(status);

    parent.add_child(node);
    mark_modified(doc);
    Ok(())
}

// ============================================================================
// INVESTMENT MANAGER OPERATIONS
// ============================================================================

/// Add an investment manager mandate
pub fn add_im_mandate(
    doc: &mut TradingMatrixDocument,
    manager_id: &str,
    manager_entity_id: &str,
    manager_name: &str,
    manager_lei: Option<&str>,
    priority: i32,
    role: &str,
    can_trade: bool,
    can_settle: bool,
    _scope_instrument_classes: Vec<String>,
    _scope_markets: Vec<String>,
    _scope_currencies: Vec<String>,
) -> AstBuildResult<()> {
    let managers = doc.ensure_category(categories::MANAGERS);
    let node_id = managers.id.child(manager_id);

    // Check if already exists
    if managers.children.iter().any(|c| c.id == node_id) {
        return Err(AstBuildError::NodeAlreadyExists {
            path: format!("{}/{}", categories::MANAGERS, manager_id),
        });
    }

    let sublabel = format!("{} (P{})", role, priority);

    let node = TradingMatrixNode::new(
        node_id,
        TradingMatrixNodeType::InvestmentManagerMandate {
            mandate_id: manager_id.to_string(),
            manager_entity_id: manager_entity_id.to_string(),
            manager_name: manager_name.to_string(),
            manager_lei: manager_lei.map(|s| s.to_string()),
            priority,
            role: role.to_string(),
            can_trade,
            can_settle,
        },
        manager_name,
    )
    .with_sublabel(sublabel)
    .with_status(StatusColor::Green);

    // Store scope as children (optional - could also add to node type)
    // For now the scope is in the op data, we could add scope nodes later

    managers.add_child(node);
    mark_modified(doc);
    Ok(())
}

/// Update an investment manager's scope
pub fn update_im_scope(
    doc: &mut TradingMatrixDocument,
    manager_ref: &str,
    _scope_instrument_classes: Option<Vec<String>>,
    _scope_markets: Option<Vec<String>>,
    _scope_currencies: Option<Vec<String>>,
) -> AstBuildResult<()> {
    let managers = doc.ensure_category(categories::MANAGERS);

    // Find manager by name or ID
    let manager = managers
        .children
        .iter_mut()
        .find(|c| {
            if let TradingMatrixNodeType::InvestmentManagerMandate {
                mandate_id,
                manager_name,
                ..
            } = &c.node_type
            {
                mandate_id == manager_ref || manager_name == manager_ref
            } else {
                false
            }
        })
        .ok_or_else(|| AstBuildError::NodeNotFound {
            path: format!("{}/{}", categories::MANAGERS, manager_ref),
        })?;

    // For now, just update sublabel to indicate scope was modified
    // A fuller implementation would store scope in the node type or as children
    if let Some(ref current_sublabel) = manager.sublabel {
        manager.sublabel = Some(format!("{} (scope updated)", current_sublabel));
    }

    mark_modified(doc);
    Ok(())
}

// ============================================================================
// CSA COLLATERAL OPERATIONS
// ============================================================================

/// Add eligible collateral to a CSA
pub fn add_csa_eligible_collateral(
    doc: &mut TradingMatrixDocument,
    isda_ref: &str,
    csa_ref: &str,
    collateral_id: &str,
    collateral_type: &str,
    currency: Option<&str>,
    haircut_pct: Option<f64>,
    _concentration_limit_pct: Option<f64>,
) -> AstBuildResult<()> {
    let isda_cat = doc.ensure_category(categories::ISDA);

    // Find ISDA by counterparty name
    let isda_node = isda_cat
        .children
        .iter_mut()
        .find(|c| {
            if let TradingMatrixNodeType::IsdaAgreement {
                counterparty_name, ..
            } = &c.node_type
            {
                counterparty_name == isda_ref
            } else {
                false
            }
        })
        .ok_or_else(|| AstBuildError::ParentNotFound {
            path: format!("{}/{}", categories::ISDA, isda_ref),
        })?;

    // Find CSA under ISDA
    let csa_node = isda_node
        .children
        .iter_mut()
        .find(|c| {
            if let TradingMatrixNodeType::CsaAgreement { csa_type, .. } = &c.node_type {
                csa_type == csa_ref
            } else {
                false
            }
        })
        .ok_or_else(|| AstBuildError::ParentNotFound {
            path: format!("{}/{}/{}", categories::ISDA, isda_ref, csa_ref),
        })?;

    // Add collateral as a child node (using a generic approach for now)
    let node_id = csa_node.id.child(collateral_id);

    if csa_node.children.iter().any(|c| c.id == node_id) {
        return Err(AstBuildError::NodeAlreadyExists {
            path: format!(
                "{}/{}/{}/{}",
                categories::ISDA,
                isda_ref,
                csa_ref,
                collateral_id
            ),
        });
    }

    let sublabel = match (currency, haircut_pct) {
        (Some(ccy), Some(hc)) => format!("{} ({}% haircut)", ccy, hc),
        (Some(ccy), None) => ccy.to_string(),
        (None, Some(hc)) => format!("{}% haircut", hc),
        (None, None) => String::new(),
    };

    // Use a Category node type for now - could add specific CollateralType later
    let node = TradingMatrixNode::new(
        node_id,
        TradingMatrixNodeType::Category {
            name: format!("Collateral: {}", collateral_type),
        },
        collateral_type,
    )
    .with_sublabel(sublabel);

    csa_node.add_child(node);
    mark_modified(doc);
    Ok(())
}

/// Link an SSI to a CSA for collateral movements
pub fn link_csa_ssi(
    doc: &mut TradingMatrixDocument,
    isda_ref: &str,
    csa_ref: &str,
    ssi_ref: &str,
) -> AstBuildResult<()> {
    let isda_cat = doc.ensure_category(categories::ISDA);

    // Find ISDA by counterparty name
    let isda_node = isda_cat
        .children
        .iter_mut()
        .find(|c| {
            if let TradingMatrixNodeType::IsdaAgreement {
                counterparty_name, ..
            } = &c.node_type
            {
                counterparty_name == isda_ref
            } else {
                false
            }
        })
        .ok_or_else(|| AstBuildError::ParentNotFound {
            path: format!("{}/{}", categories::ISDA, isda_ref),
        })?;

    // Find CSA under ISDA
    let csa_node = isda_node
        .children
        .iter_mut()
        .find(|c| {
            if let TradingMatrixNodeType::CsaAgreement { csa_type, .. } = &c.node_type {
                csa_type == csa_ref
            } else {
                false
            }
        })
        .ok_or_else(|| AstBuildError::ParentNotFound {
            path: format!("{}/{}/{}", categories::ISDA, isda_ref, csa_ref),
        })?;

    // Update the CSA's collateral_ssi_ref if it's a CsaAgreement
    if let TradingMatrixNodeType::CsaAgreement {
        ref mut collateral_ssi_ref,
        ..
    } = csa_node.node_type
    {
        *collateral_ssi_ref = Some(ssi_ref.to_string());
    }

    mark_modified(doc);
    Ok(())
}

// ============================================================================
// CURRENCY CONFIGURATION
// ============================================================================

/// Set the base currency for the trading profile
pub fn set_base_currency(doc: &mut TradingMatrixDocument, currency: &str) -> AstBuildResult<()> {
    // Store base currency in metadata
    doc.metadata.notes = Some(format!(
        "Base currency: {}{}",
        currency,
        doc.metadata
            .notes
            .as_ref()
            .map(|n| format!("; {}", n))
            .unwrap_or_default()
    ));
    mark_modified(doc);
    Ok(())
}

/// Add an allowed currency to the profile
pub fn add_allowed_currency(doc: &mut TradingMatrixDocument, currency: &str) -> AstBuildResult<()> {
    // For now, store in metadata - could add a dedicated Currency category later
    let current_notes = doc.metadata.notes.clone().unwrap_or_default();
    if current_notes.contains(&format!("allowed:{}", currency)) {
        return Err(AstBuildError::NodeAlreadyExists {
            path: format!("currency/{}", currency),
        });
    }

    doc.metadata.notes = Some(format!("{}; allowed:{}", current_notes, currency));
    mark_modified(doc);
    Ok(())
}

// ============================================================================
// GENERIC NODE OPERATIONS
// ============================================================================

/// Remove a node by ID (recursive search)
pub fn remove_node(
    doc: &mut TradingMatrixDocument,
    node_id: &TradingMatrixNodeId,
) -> AstBuildResult<()> {
    fn remove_from_children(
        children: &mut Vec<TradingMatrixNode>,
        node_id: &TradingMatrixNodeId,
    ) -> bool {
        // First try to remove directly
        let initial_len = children.len();
        children.retain(|c| &c.id != node_id);
        if children.len() < initial_len {
            return true;
        }

        // Recursively search children
        for child in children.iter_mut() {
            if remove_from_children(&mut child.children, node_id) {
                return true;
            }
        }

        false
    }

    if remove_from_children(&mut doc.children, node_id) {
        mark_modified(doc);
        Ok(())
    } else {
        Err(AstBuildError::NodeNotFound {
            path: node_id.0.join("/"),
        })
    }
}

/// Set a node's status color
pub fn set_node_status(
    doc: &mut TradingMatrixDocument,
    node_id: &TradingMatrixNodeId,
    status: StatusColor,
) -> AstBuildResult<()> {
    let node = doc
        .find_by_id_mut(node_id)
        .ok_or_else(|| AstBuildError::NodeNotFound {
            path: node_id.0.join("/"),
        })?;

    node.status_color = Some(status);
    mark_modified(doc);
    Ok(())
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_document() {
        let doc = create_document("cbu-123", "Test CBU");
        assert_eq!(doc.cbu_id, "cbu-123");
        assert_eq!(doc.cbu_name, "Test CBU");
        assert_eq!(doc.version, 1);
        assert_eq!(doc.status, DocumentStatus::Draft);
    }

    #[test]
    fn test_add_instrument_class() {
        let mut doc = create_document("cbu-123", "Test CBU");

        add_instrument_class(&mut doc, "EQUITY", Some("ES"), false).unwrap();

        let universe = doc
            .find_by_id(&TradingMatrixNodeId::category("UNIVERSE"))
            .unwrap();
        assert_eq!(universe.children.len(), 1);
        assert_eq!(universe.children[0].label, "EQUITY");
    }

    #[test]
    fn test_add_market_under_class() {
        let mut doc = create_document("cbu-123", "Test CBU");

        add_instrument_class(&mut doc, "EQUITY", None, false).unwrap();
        add_market(&mut doc, "EQUITY", "XNYS", "New York Stock Exchange", "US").unwrap();

        let universe = doc
            .find_by_id(&TradingMatrixNodeId::category("UNIVERSE"))
            .unwrap();
        let equity = &universe.children[0];
        assert_eq!(equity.children.len(), 1);
        assert_eq!(equity.children[0].label, "XNYS");
    }

    #[test]
    fn test_add_ssi_and_booking_rule() {
        let mut doc = create_document("cbu-123", "Test CBU");

        add_ssi(
            &mut doc,
            "ssi-001",
            "US Equities",
            "SECURITIES",
            Some("SAFE001"),
            Some("CITIUS33"),
            Some("CASH001"),
            Some("CITIUS33"),
            Some("USD"),
            Some("DTCYUS33"),
        )
        .unwrap();

        add_booking_rule(
            &mut doc,
            "US Equities",
            "rule-001",
            "US Equity DVP",
            10,
            BookingMatchCriteria {
                instrument_class: Some("EQUITY".to_string()),
                mic: Some("XNYS".to_string()),
                currency: Some("USD".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        let ssi = doc
            .find_by_id(&TradingMatrixNodeId::category("SSI"))
            .unwrap();
        let ssi_node = &ssi.children[0];
        assert_eq!(ssi_node.label, "US Equities");
        assert_eq!(ssi_node.children.len(), 1);
        assert_eq!(ssi_node.children[0].label, "US Equity DVP");
    }

    #[test]
    fn test_duplicate_node_error() {
        let mut doc = create_document("cbu-123", "Test CBU");

        add_instrument_class(&mut doc, "EQUITY", None, false).unwrap();
        let result = add_instrument_class(&mut doc, "EQUITY", None, false);

        assert!(matches!(
            result,
            Err(AstBuildError::NodeAlreadyExists { .. })
        ));
    }

    #[test]
    fn test_apply_op() {
        let mut doc = create_document("cbu-123", "Test CBU");

        apply_op(
            &mut doc,
            TradingMatrixOp::AddInstrumentClass {
                class_code: "EQUITY".to_string(),
                cfi_prefix: Some("ES".to_string()),
                is_otc: false,
            },
        )
        .unwrap();

        apply_op(
            &mut doc,
            TradingMatrixOp::AddMarket {
                parent_class: "EQUITY".to_string(),
                mic: "XLON".to_string(),
                market_name: "London Stock Exchange".to_string(),
                country_code: "GB".to_string(),
            },
        )
        .unwrap();

        let universe = doc
            .find_by_id(&TradingMatrixNodeId::category("UNIVERSE"))
            .unwrap();
        assert_eq!(universe.children.len(), 1);
        assert_eq!(universe.children[0].children.len(), 1);
    }

    #[test]
    fn test_remove_node() {
        let mut doc = create_document("cbu-123", "Test CBU");

        add_instrument_class(&mut doc, "EQUITY", None, false).unwrap();
        add_market(&mut doc, "EQUITY", "XNYS", "NYSE", "US").unwrap();

        let node_id = TradingMatrixNodeId::category("UNIVERSE")
            .child("EQUITY")
            .child("XNYS");

        remove_node(&mut doc, &node_id).unwrap();

        let equity = doc
            .find_by_id(&TradingMatrixNodeId::category("UNIVERSE").child("EQUITY"))
            .unwrap();
        assert_eq!(equity.children.len(), 0);
    }
}
