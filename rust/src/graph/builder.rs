//! CbuGraphBuilder - loads CBU data from PostgreSQL and builds a CbuGraph
//!
//! This builder queries the database for CBU data across multiple layers
//! (custody, KYC, UBO, services) and constructs a graph representation.

use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use super::types::*;

/// Builder for constructing CbuGraph from database
pub struct CbuGraphBuilder {
    cbu_id: Uuid,
    include_custody: bool,
    include_kyc: bool,
    include_ubo: bool,
    include_services: bool,
}

impl CbuGraphBuilder {
    /// Create a new builder for the given CBU
    pub fn new(cbu_id: Uuid) -> Self {
        Self {
            cbu_id,
            include_custody: true,
            include_kyc: false,
            include_ubo: false,
            include_services: false,
        }
    }

    /// Include custody layer (universe, SSI, booking rules, ISDA/CSA)
    pub fn with_custody(mut self, include: bool) -> Self {
        self.include_custody = include;
        self
    }

    /// Include KYC layer (documents, verifications)
    pub fn with_kyc(mut self, include: bool) -> Self {
        self.include_kyc = include;
        self
    }

    /// Include UBO layer (ownership chain)
    pub fn with_ubo(mut self, include: bool) -> Self {
        self.include_ubo = include;
        self
    }

    /// Include services layer (products, services, resources)
    pub fn with_services(mut self, include: bool) -> Self {
        self.include_services = include;
        self
    }

    /// Build the graph from database
    pub async fn build(self, pool: &PgPool) -> Result<CbuGraph> {
        // Load CBU base record
        let cbu_record = sqlx::query!(
            r#"SELECT cbu_id, name, jurisdiction, client_type
               FROM "ob-poc".cbus WHERE cbu_id = $1"#,
            self.cbu_id
        )
        .fetch_one(pool)
        .await?;

        let mut graph = CbuGraph::new(self.cbu_id, cbu_record.name.clone());

        // Add CBU root node
        graph.add_node(GraphNode {
            id: self.cbu_id.to_string(),
            node_type: NodeType::Cbu,
            layer: LayerType::Core,
            label: cbu_record.name,
            sublabel: Some(format!(
                "{} / {}",
                cbu_record.jurisdiction.as_deref().unwrap_or("N/A"),
                cbu_record.client_type.as_deref().unwrap_or("N/A")
            )),
            status: NodeStatus::Active,
            parent_id: None,
            data: serde_json::json!({
                "jurisdiction": cbu_record.jurisdiction,
                "client_type": cbu_record.client_type
            }),
        });

        // Always load linked entities (Core layer)
        self.load_entities(&mut graph, pool).await?;

        // Load custody layer
        if self.include_custody {
            self.load_custody_layer(&mut graph, pool).await?;
        }

        // Load KYC layer (stub - tables don't exist yet)
        if self.include_kyc {
            self.load_kyc_layer(&mut graph, pool).await?;
        }

        // Load UBO layer (stub - tables don't exist yet)
        if self.include_ubo {
            self.load_ubo_layer(&mut graph, pool).await?;
        }

        // Load Services layer
        if self.include_services {
            self.load_services_layer(&mut graph, pool).await?;
        }

        // Compute final stats
        graph.compute_stats();
        graph.build_layer_info();

        Ok(graph)
    }

    /// Load entities linked to the CBU via cbu_entity_roles
    async fn load_entities(&self, graph: &mut CbuGraph, pool: &PgPool) -> Result<()> {
        let entities = sqlx::query!(
            r#"SELECT
                cer.cbu_entity_role_id,
                cer.entity_id,
                e.name as entity_name,
                et.name as entity_type,
                r.name as role_name
               FROM "ob-poc".cbu_entity_roles cer
               JOIN "ob-poc".entities e ON e.entity_id = cer.entity_id
               JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
               JOIN "ob-poc".roles r ON r.role_id = cer.role_id
               WHERE cer.cbu_id = $1"#,
            self.cbu_id
        )
        .fetch_all(pool)
        .await?;

        for ent in entities {
            let entity_id = ent.entity_id.to_string();

            // Only add entity node if not already present (an entity can have multiple roles)
            if !graph.has_node(&entity_id) {
                graph.add_node(GraphNode {
                    id: entity_id.clone(),
                    node_type: NodeType::Entity,
                    layer: LayerType::Core,
                    label: ent.entity_name,
                    sublabel: Some(ent.entity_type),
                    status: NodeStatus::Active,
                    parent_id: None,
                    data: serde_json::json!({
                        "entity_id": ent.entity_id
                    }),
                });
            }

            // Add edge for each role
            graph.add_edge(GraphEdge {
                id: ent.cbu_entity_role_id.to_string(),
                source: self.cbu_id.to_string(),
                target: entity_id,
                edge_type: EdgeType::HasRole,
                label: Some(ent.role_name),
            });
        }

        Ok(())
    }

    async fn load_custody_layer(&self, graph: &mut CbuGraph, pool: &PgPool) -> Result<()> {
        // Track which markets we've created nodes for
        let mut market_nodes: std::collections::HashMap<Uuid, String> =
            std::collections::HashMap::new();

        // Load universe entries with joined names
        let universes = sqlx::query!(
            r#"SELECT
                u.universe_id,
                u.instrument_class_id,
                u.market_id,
                u.currencies,
                u.settlement_types,
                u.is_active,
                ic.name as "class_name?",
                m.name as "market_name?",
                m.mic as "mic?"
               FROM custody.cbu_instrument_universe u
               LEFT JOIN custody.instrument_classes ic ON ic.class_id = u.instrument_class_id
               LEFT JOIN custody.markets m ON m.market_id = u.market_id
               WHERE u.cbu_id = $1"#,
            self.cbu_id
        )
        .fetch_all(pool)
        .await?;

        for u in universes {
            let universe_id = u.universe_id.to_string();

            // Create market grouping node if not already created
            let market_node_id = if let Some(market_id) = u.market_id {
                if !market_nodes.contains_key(&market_id) {
                    let market_node_id = format!("market-{}", market_id);
                    let mic = u.mic.clone().unwrap_or_else(|| "N/A".to_string());
                    let market_name = u.market_name.clone().unwrap_or_else(|| mic.clone());

                    graph.add_node(GraphNode {
                        id: market_node_id.clone(),
                        node_type: NodeType::Market,
                        layer: LayerType::Custody,
                        label: mic.clone(),
                        sublabel: Some(market_name),
                        status: NodeStatus::Active,
                        parent_id: None,
                        data: serde_json::json!({
                            "market_id": market_id
                        }),
                    });

                    // Edge: CBU → Market
                    graph.add_edge(GraphEdge {
                        id: format!("cbu->{}", market_node_id),
                        source: self.cbu_id.to_string(),
                        target: market_node_id.clone(),
                        edge_type: EdgeType::Matches,
                        label: None,
                    });

                    market_nodes.insert(market_id, market_node_id.clone());
                }
                market_nodes.get(&market_id).cloned()
            } else {
                None
            };

            let class_name = u
                .class_name
                .clone()
                .unwrap_or_else(|| "Unknown".to_string());

            let sublabel = if u.currencies.is_empty() {
                None
            } else {
                Some(u.currencies.join(", "))
            };

            graph.add_node(GraphNode {
                id: universe_id.clone(),
                node_type: NodeType::Universe,
                layer: LayerType::Custody,
                label: class_name,
                sublabel,
                status: if u.is_active.unwrap_or(true) {
                    NodeStatus::Active
                } else {
                    NodeStatus::Suspended
                },
                parent_id: market_node_id.clone(),
                data: serde_json::json!({
                    "instrument_class_id": u.instrument_class_id,
                    "class_name": u.class_name,
                    "market_id": u.market_id,
                    "market_name": u.market_name,
                    "currencies": u.currencies,
                    "settlement_types": u.settlement_types
                }),
            });

            // Edge: Market → Universe (if grouped) or CBU → Universe (if no market)
            if let Some(ref parent_id) = market_node_id {
                graph.add_edge(GraphEdge {
                    id: format!("{}->{}", parent_id, universe_id),
                    source: parent_id.clone(),
                    target: universe_id,
                    edge_type: EdgeType::Matches,
                    label: None,
                });
            } else {
                graph.add_edge(GraphEdge {
                    id: format!("cbu->{}", universe_id),
                    source: self.cbu_id.to_string(),
                    target: universe_id,
                    edge_type: EdgeType::Matches,
                    label: None,
                });
            }
        }

        // Load SSIs with market info for grouping
        let ssis = sqlx::query!(
            r#"SELECT s.ssi_id, s.ssi_name, s.ssi_type, s.status, s.cash_currency,
                      s.safekeeping_account, s.safekeeping_bic, s.cash_account, s.cash_account_bic,
                      s.market_id,
                      m.mic as "mic?"
               FROM custody.cbu_ssi s
               LEFT JOIN custody.markets m ON m.market_id = s.market_id
               WHERE s.cbu_id = $1"#,
            self.cbu_id
        )
        .fetch_all(pool)
        .await?;

        for ssi in ssis {
            let ssi_id = ssi.ssi_id.to_string();

            // Create market node if needed and get parent_id
            let market_node_id = if let Some(market_id) = ssi.market_id {
                if !market_nodes.contains_key(&market_id) {
                    let market_node_id = format!("market-{}", market_id);
                    let mic = ssi.mic.clone().unwrap_or_else(|| "N/A".to_string());

                    graph.add_node(GraphNode {
                        id: market_node_id.clone(),
                        node_type: NodeType::Market,
                        layer: LayerType::Custody,
                        label: mic.clone(),
                        sublabel: None,
                        status: NodeStatus::Active,
                        parent_id: None,
                        data: serde_json::json!({
                            "market_id": market_id
                        }),
                    });

                    // Edge: CBU → Market
                    graph.add_edge(GraphEdge {
                        id: format!("cbu->{}", market_node_id),
                        source: self.cbu_id.to_string(),
                        target: market_node_id.clone(),
                        edge_type: EdgeType::Matches,
                        label: None,
                    });

                    market_nodes.insert(market_id, market_node_id.clone());
                }
                market_nodes.get(&market_id).cloned()
            } else {
                None
            };

            graph.add_node(GraphNode {
                id: ssi_id.clone(),
                node_type: NodeType::Ssi,
                layer: LayerType::Custody,
                label: ssi.ssi_name,
                sublabel: Some(format!(
                    "{} @ {}",
                    ssi.cash_currency.as_deref().unwrap_or("N/A"),
                    ssi.safekeeping_bic.as_deref().unwrap_or("N/A")
                )),
                status: match ssi.status.as_deref().unwrap_or("DRAFT") {
                    "ACTIVE" => NodeStatus::Active,
                    "PENDING" => NodeStatus::Pending,
                    "SUSPENDED" => NodeStatus::Suspended,
                    _ => NodeStatus::Draft,
                },
                parent_id: market_node_id.clone(),
                data: serde_json::json!({
                    "ssi_type": ssi.ssi_type,
                    "cash_currency": ssi.cash_currency,
                    "safekeeping_account": ssi.safekeeping_account,
                    "safekeeping_bic": ssi.safekeeping_bic,
                    "cash_account": ssi.cash_account,
                    "cash_account_bic": ssi.cash_account_bic,
                    "market_id": ssi.market_id
                }),
            });

            // Edge: Market → SSI (if grouped) or CBU → SSI
            if let Some(ref parent_id) = market_node_id {
                graph.add_edge(GraphEdge {
                    id: format!("{}->{}", parent_id, ssi_id),
                    source: parent_id.clone(),
                    target: ssi_id,
                    edge_type: EdgeType::SettlesAt,
                    label: None,
                });
            } else {
                graph.add_edge(GraphEdge {
                    id: format!("cbu->{}", ssi_id),
                    source: self.cbu_id.to_string(),
                    target: ssi_id,
                    edge_type: EdgeType::SettlesAt,
                    label: None,
                });
            }
        }

        // Load booking rules
        let rules = sqlx::query!(
            r#"SELECT r.rule_id, r.rule_name, r.priority, r.ssi_id,
                      r.instrument_class_id, r.market_id, r.currency, r.is_active,
                      ic.name as "class_name?",
                      m.mic as "mic?"
               FROM custody.ssi_booking_rules r
               LEFT JOIN custody.instrument_classes ic ON ic.class_id = r.instrument_class_id
               LEFT JOIN custody.markets m ON m.market_id = r.market_id
               WHERE r.cbu_id = $1
               ORDER BY r.priority"#,
            self.cbu_id
        )
        .fetch_all(pool)
        .await?;

        for rule in rules {
            let rule_id = rule.rule_id.to_string();

            // Create market node if needed and get parent_id
            let market_node_id = if let Some(market_id) = rule.market_id {
                if !market_nodes.contains_key(&market_id) {
                    let market_node_id = format!("market-{}", market_id);
                    let mic = rule.mic.clone().unwrap_or_else(|| "N/A".to_string());

                    graph.add_node(GraphNode {
                        id: market_node_id.clone(),
                        node_type: NodeType::Market,
                        layer: LayerType::Custody,
                        label: mic.clone(),
                        sublabel: None,
                        status: NodeStatus::Active,
                        parent_id: None,
                        data: serde_json::json!({
                            "market_id": market_id
                        }),
                    });

                    // Edge: CBU → Market
                    graph.add_edge(GraphEdge {
                        id: format!("cbu->{}", market_node_id),
                        source: self.cbu_id.to_string(),
                        target: market_node_id.clone(),
                        edge_type: EdgeType::Matches,
                        label: None,
                    });

                    market_nodes.insert(market_id, market_node_id.clone());
                }
                market_nodes.get(&market_id).cloned()
            } else {
                None
            };

            graph.add_node(GraphNode {
                id: rule_id.clone(),
                node_type: NodeType::BookingRule,
                layer: LayerType::Custody,
                label: rule.rule_name,
                sublabel: Some(format!("Priority {}", rule.priority)),
                status: if rule.is_active.unwrap_or(true) {
                    NodeStatus::Active
                } else {
                    NodeStatus::Suspended
                },
                parent_id: market_node_id,
                data: serde_json::json!({
                    "priority": rule.priority,
                    "instrument_class_id": rule.instrument_class_id,
                    "class_name": rule.class_name,
                    "market_id": rule.market_id,
                    "mic": rule.mic,
                    "currency": rule.currency
                }),
            });

            // Edge: Rule → SSI (routes to)
            graph.add_edge(GraphEdge {
                id: format!("{}->{}", rule_id, rule.ssi_id),
                source: rule_id,
                target: rule.ssi_id.to_string(),
                edge_type: EdgeType::RoutesTo,
                label: None,
            });
        }

        // Load ISDA agreements
        let isdas = sqlx::query!(
            r#"SELECT i.isda_id, i.counterparty_entity_id, i.governing_law,
                      i.agreement_date, i.is_active,
                      e.name as "counterparty_name?"
               FROM custody.isda_agreements i
               LEFT JOIN "ob-poc".entities e ON e.entity_id = i.counterparty_entity_id
               WHERE i.cbu_id = $1"#,
            self.cbu_id
        )
        .fetch_all(pool)
        .await?;

        for isda in isdas {
            let isda_id = isda.isda_id.to_string();

            let counterparty = isda
                .counterparty_name
                .clone()
                .unwrap_or_else(|| "Unknown".to_string());
            let label = format!("ISDA - {}", counterparty);

            graph.add_node(GraphNode {
                id: isda_id.clone(),
                node_type: NodeType::Isda,
                layer: LayerType::Custody,
                label,
                sublabel: isda.governing_law.as_ref().map(|g| format!("{} law", g)),
                status: if isda.is_active.unwrap_or(true) {
                    NodeStatus::Active
                } else {
                    NodeStatus::Suspended
                },
                parent_id: None,
                data: serde_json::json!({
                    "counterparty_entity_id": isda.counterparty_entity_id,
                    "counterparty_name": isda.counterparty_name,
                    "governing_law": isda.governing_law,
                    "agreement_date": isda.agreement_date
                }),
            });

            // Edge: CBU → ISDA
            graph.add_edge(GraphEdge {
                id: format!("cbu->{}", isda_id),
                source: self.cbu_id.to_string(),
                target: isda_id.clone(),
                edge_type: EdgeType::CoveredBy,
                label: None,
            });

            // Load CSAs for this ISDA
            let csas = sqlx::query!(
                r#"SELECT csa_id, csa_type, is_active
                   FROM custody.csa_agreements
                   WHERE isda_id = $1"#,
                isda.isda_id
            )
            .fetch_all(pool)
            .await?;

            for csa in csas {
                let csa_id = csa.csa_id.to_string();

                graph.add_node(GraphNode {
                    id: csa_id.clone(),
                    node_type: NodeType::Csa,
                    layer: LayerType::Custody,
                    label: format!("CSA ({})", csa.csa_type),
                    sublabel: None,
                    status: if csa.is_active.unwrap_or(true) {
                        NodeStatus::Active
                    } else {
                        NodeStatus::Suspended
                    },
                    parent_id: None,
                    data: serde_json::json!({
                        "csa_type": csa.csa_type
                    }),
                });

                // Edge: ISDA → CSA
                graph.add_edge(GraphEdge {
                    id: format!("{}->{}", isda_id, csa_id),
                    source: isda_id.clone(),
                    target: csa_id,
                    edge_type: EdgeType::SecuredBy,
                    label: None,
                });
            }
        }

        Ok(())
    }

    async fn load_kyc_layer(&self, graph: &mut CbuGraph, pool: &PgPool) -> Result<()> {
        // Load entity KYC statuses for this CBU
        let statuses = sqlx::query!(
            r#"SELECT
                ks.status_id,
                ks.entity_id,
                ks.kyc_status,
                ks.risk_rating,
                ks.next_review_date,
                e.name as "entity_name?"
               FROM "ob-poc".entity_kyc_status ks
               JOIN "ob-poc".entities e ON e.entity_id = ks.entity_id
               WHERE ks.cbu_id = $1"#,
            self.cbu_id
        )
        .fetch_all(pool)
        .await?;

        for ks in statuses {
            let status_id = format!("kyc-status-{}", ks.status_id);

            graph.add_node(GraphNode {
                id: status_id.clone(),
                node_type: NodeType::Verification,
                layer: LayerType::Kyc,
                label: format!("KYC: {}", ks.kyc_status.as_deref().unwrap_or("N/A")),
                sublabel: ks.risk_rating.clone(),
                status: match ks.kyc_status.as_deref() {
                    Some("APPROVED") => NodeStatus::Active,
                    Some("IN_PROGRESS") | Some("PENDING_REVIEW") => NodeStatus::Pending,
                    Some("REJECTED") => NodeStatus::Expired,
                    Some("EXPIRED") => NodeStatus::Expired,
                    _ => NodeStatus::Draft,
                },
                parent_id: None,
                data: serde_json::json!({
                    "kyc_status": ks.kyc_status,
                    "risk_rating": ks.risk_rating,
                    "next_review_date": ks.next_review_date
                }),
            });

            // Edge: Entity → KYC Status
            graph.add_edge(GraphEdge {
                id: format!("{}->{}", ks.entity_id, status_id),
                source: ks.entity_id.to_string(),
                target: status_id,
                edge_type: EdgeType::Validates,
                label: None,
            });
        }

        // Load document requirements from investigations linked to this CBU
        let doc_reqs = sqlx::query!(
            r#"SELECT
                dr.request_id,
                dr.document_type,
                dr.status,
                dr.requested_from_entity_id,
                e.name as "entity_name?"
               FROM "ob-poc".document_requests dr
               JOIN "ob-poc".kyc_investigations ki ON ki.investigation_id = dr.investigation_id
               LEFT JOIN "ob-poc".entities e ON e.entity_id = dr.requested_from_entity_id
               WHERE ki.cbu_id = $1"#,
            self.cbu_id
        )
        .fetch_all(pool)
        .await?;

        for dr in doc_reqs {
            let doc_req_id = format!("doc-req-{}", dr.request_id);

            graph.add_node(GraphNode {
                id: doc_req_id.clone(),
                node_type: NodeType::Document,
                layer: LayerType::Kyc,
                label: dr.document_type.clone(),
                sublabel: dr.status.clone(),
                status: match dr.status.as_deref() {
                    Some("RECEIVED") | Some("VERIFIED") => NodeStatus::Active,
                    Some("PENDING") => NodeStatus::Pending,
                    Some("REJECTED") => NodeStatus::Expired,
                    _ => NodeStatus::Draft,
                },
                parent_id: None,
                data: serde_json::json!({
                    "document_type": dr.document_type,
                    "status": dr.status
                }),
            });

            // Edge: Entity → Document Requirement (if entity specified)
            if let Some(entity_id) = dr.requested_from_entity_id {
                graph.add_edge(GraphEdge {
                    id: format!("{}->{}", entity_id, doc_req_id),
                    source: entity_id.to_string(),
                    target: doc_req_id,
                    edge_type: EdgeType::Requires,
                    label: None,
                });
            }
        }

        // Load screenings for entities linked to this CBU
        let screenings = sqlx::query!(
            r#"SELECT
                s.screening_id,
                s.entity_id,
                s.screening_type,
                s.result,
                s.resolution,
                e.name as "entity_name?"
               FROM "ob-poc".screenings s
               JOIN "ob-poc".cbu_entity_roles cer ON cer.entity_id = s.entity_id
               JOIN "ob-poc".entities e ON e.entity_id = s.entity_id
               WHERE cer.cbu_id = $1"#,
            self.cbu_id
        )
        .fetch_all(pool)
        .await?;

        for scr in screenings {
            let screening_id = format!("screening-{}", scr.screening_id);

            graph.add_node(GraphNode {
                id: screening_id.clone(),
                node_type: NodeType::Verification,
                layer: LayerType::Kyc,
                label: format!("{}", scr.screening_type),
                sublabel: scr.result.clone(),
                status: match scr.result.as_deref() {
                    Some("CLEAR") => NodeStatus::Active,
                    Some("MATCH") | Some("POTENTIAL_MATCH") => match scr.resolution.as_deref() {
                        Some("FALSE_POSITIVE") => NodeStatus::Active,
                        Some("CONFIRMED_MATCH") => NodeStatus::Expired,
                        _ => NodeStatus::Pending,
                    },
                    _ => NodeStatus::Draft,
                },
                parent_id: None,
                data: serde_json::json!({
                    "screening_type": scr.screening_type,
                    "result": scr.result,
                    "resolution": scr.resolution
                }),
            });

            // Edge: Entity → Screening
            graph.add_edge(GraphEdge {
                id: format!("{}->{}", scr.entity_id, screening_id),
                source: scr.entity_id.to_string(),
                target: screening_id,
                edge_type: EdgeType::Validates,
                label: None,
            });
        }

        Ok(())
    }

    async fn load_ubo_layer(&self, graph: &mut CbuGraph, pool: &PgPool) -> Result<()> {
        // Load UBO registry entries for this CBU
        let ubos = sqlx::query!(
            r#"SELECT
                u.ubo_id,
                u.subject_entity_id,
                u.ubo_proper_person_id,
                u.relationship_type,
                u.ownership_percentage,
                u.control_type,
                u.verification_status,
                se.name as "subject_name?",
                pe.name as "ubo_name?"
               FROM "ob-poc".ubo_registry u
               LEFT JOIN "ob-poc".entities se ON se.entity_id = u.subject_entity_id
               LEFT JOIN "ob-poc".entities pe ON pe.entity_id = u.ubo_proper_person_id
               WHERE u.cbu_id = $1"#,
            self.cbu_id
        )
        .fetch_all(pool)
        .await?;

        for ubo in &ubos {
            // Add UBO person node if not already present
            let ubo_id_str = ubo.ubo_proper_person_id.to_string();
            if !graph.has_node(&ubo_id_str) {
                let ubo_name = ubo
                    .ubo_name
                    .clone()
                    .unwrap_or_else(|| "Unknown UBO".to_string());
                let pct_str = ubo
                    .ownership_percentage
                    .as_ref()
                    .map(|p| format!("{}%", p))
                    .unwrap_or_else(|| "?%".to_string());
                graph.add_node(GraphNode {
                    id: ubo_id_str.clone(),
                    node_type: NodeType::Entity,
                    layer: LayerType::Ubo,
                    label: ubo_name,
                    sublabel: Some(pct_str),
                    status: match ubo.verification_status.as_deref() {
                        Some("VERIFIED") => NodeStatus::Active,
                        Some("PENDING") => NodeStatus::Pending,
                        _ => NodeStatus::Draft,
                    },
                    parent_id: None,
                    data: serde_json::json!({
                        "ownership_percentage": ubo.ownership_percentage,
                        "control_type": ubo.control_type,
                        "verification_status": ubo.verification_status
                    }),
                });
            }

            // Add ownership edge from subject to UBO
            let subject_id_str = ubo.subject_entity_id.to_string();
            let pct_label = ubo.ownership_percentage.as_ref().map(|p| format!("{}%", p));

            graph.add_edge(GraphEdge {
                id: ubo.ubo_id.to_string(),
                source: subject_id_str,
                target: ubo_id_str,
                edge_type: EdgeType::Owns,
                label: pct_label,
            });
        }

        // Also load direct ownership relationships for chain visualization
        let ownerships = sqlx::query!(
            r#"SELECT
                o.ownership_id,
                o.owner_entity_id,
                o.owned_entity_id,
                o.ownership_type,
                o.ownership_percent,
                owner.name as "owner_name?",
                owned.name as "owned_name?"
               FROM "ob-poc".ownership_relationships o
               LEFT JOIN "ob-poc".entities owner ON owner.entity_id = o.owner_entity_id
               LEFT JOIN "ob-poc".entities owned ON owned.entity_id = o.owned_entity_id
               WHERE o.owned_entity_id IN (
                   SELECT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1
               )
               OR o.owner_entity_id IN (
                   SELECT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1
               )"#,
            self.cbu_id
        )
        .fetch_all(pool)
        .await?;

        for own in &ownerships {
            let owner_id = own.owner_entity_id.to_string();
            let owned_id = own.owned_entity_id.to_string();

            // Add owner node if not present
            if !graph.has_node(&owner_id) {
                graph.add_node(GraphNode {
                    id: owner_id.clone(),
                    node_type: NodeType::Entity,
                    layer: LayerType::Ubo,
                    label: own
                        .owner_name
                        .clone()
                        .unwrap_or_else(|| "Unknown".to_string()),
                    sublabel: Some(own.ownership_type.clone()),
                    status: NodeStatus::Active,
                    parent_id: None,
                    data: serde_json::json!({}),
                });
            }

            // Add ownership edge
            let pct_label = Some(format!("{}%", own.ownership_percent));
            graph.add_edge(GraphEdge {
                id: own.ownership_id.to_string(),
                source: owner_id,
                target: owned_id,
                edge_type: EdgeType::Owns,
                label: pct_label,
            });
        }

        // Load control relationships (non-ownership control like board, trustee, etc.)
        let controls = sqlx::query!(
            r#"SELECT
                c.control_id,
                c.controller_entity_id,
                c.controlled_entity_id,
                c.control_type,
                c.description,
                c.is_active,
                controller.name as "controller_name?",
                controlled.name as "controlled_name?"
               FROM "ob-poc".control_relationships c
               LEFT JOIN "ob-poc".entities controller ON controller.entity_id = c.controller_entity_id
               LEFT JOIN "ob-poc".entities controlled ON controlled.entity_id = c.controlled_entity_id
               WHERE c.is_active = true
                 AND (c.controlled_entity_id IN (
                       SELECT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1
                     )
                  OR c.controller_entity_id IN (
                       SELECT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1
                     ))"#,
            self.cbu_id
        )
        .fetch_all(pool)
        .await?;

        for ctrl in &controls {
            let controller_id = ctrl.controller_entity_id.to_string();
            let controlled_id = ctrl.controlled_entity_id.to_string();

            // Add controller node if not present
            if !graph.has_node(&controller_id) {
                graph.add_node(GraphNode {
                    id: controller_id.clone(),
                    node_type: NodeType::Entity,
                    layer: LayerType::Ubo,
                    label: ctrl
                        .controller_name
                        .clone()
                        .unwrap_or_else(|| "Unknown".to_string()),
                    sublabel: Some(ctrl.control_type.clone()),
                    status: NodeStatus::Active,
                    parent_id: None,
                    data: serde_json::json!({}),
                });
            }

            // Add control edge
            graph.add_edge(GraphEdge {
                id: ctrl.control_id.to_string(),
                source: controller_id,
                target: controlled_id,
                edge_type: EdgeType::Controls,
                label: Some(ctrl.control_type.clone()),
            });
        }

        Ok(())
    }

    async fn load_services_layer(&self, graph: &mut CbuGraph, pool: &PgPool) -> Result<()> {
        // Load service resource instances from cbu_resource_instances
        let instances = sqlx::query!(
            r#"SELECT ri.instance_id, ri.status, ri.instance_name,
                      rt.name as type_name, rt.resource_type as category
               FROM "ob-poc".cbu_resource_instances ri
               JOIN "ob-poc".service_resource_types rt ON rt.resource_id = ri.resource_type_id
               WHERE ri.cbu_id = $1"#,
            self.cbu_id
        )
        .fetch_all(pool)
        .await;

        if let Ok(instances) = instances {
            for inst in instances {
                let inst_id = inst.instance_id.to_string();

                graph.add_node(GraphNode {
                    id: inst_id.clone(),
                    node_type: NodeType::Resource,
                    layer: LayerType::Services,
                    label: inst.type_name,
                    sublabel: inst.category,
                    status: match inst.status.as_str() {
                        "ACTIVE" => NodeStatus::Active,
                        "PENDING" => NodeStatus::Pending,
                        "SUSPENDED" => NodeStatus::Suspended,
                        _ => NodeStatus::Draft,
                    },
                    parent_id: None,
                    data: serde_json::json!({
                        "instance_name": inst.instance_name
                    }),
                });

                graph.add_edge(GraphEdge {
                    id: format!("cbu->{}", inst_id),
                    source: self.cbu_id.to_string(),
                    target: inst_id,
                    edge_type: EdgeType::Delivers,
                    label: None,
                });
            }
        }

        Ok(())
    }
}
