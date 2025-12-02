//! KYC Case visualization builder
//!
//! Builds a tree visualization of a KYC case showing:
//! - Case summary with status, risk rating, escalation level
//! - Workstream tree (entities discovered during KYC)
//! - Red flags and their status
//! - Document and screening statistics

use crate::database::visualization_repository::VisualizationRepository;
use anyhow::Result;
use serde::Serialize;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

/// Builds case visualization trees
pub struct CaseTreeBuilder {
    repo: VisualizationRepository,
}

/// Complete case visualization
#[derive(Debug, Clone, Serialize)]
pub struct CaseVisualization {
    pub case_id: Uuid,
    pub cbu_id: Uuid,
    pub cbu_name: String,
    pub status: String,
    pub escalation_level: String,
    pub risk_rating: Option<String>,
    pub case_type: Option<String>,
    pub sla_deadline: Option<String>,
    pub workstream_tree: Vec<WorkstreamNode>,
    pub case_red_flags: Vec<RedFlagInfo>,
    pub stats: CaseStats,
}

/// Node in the workstream tree
#[derive(Debug, Clone, Serialize)]
pub struct WorkstreamNode {
    pub workstream_id: Uuid,
    pub entity_id: Uuid,
    pub entity_name: String,
    pub entity_type: String,
    pub jurisdiction: Option<String>,
    pub status: String,
    pub risk_rating: Option<String>,
    pub is_ubo: bool,
    pub ownership_percentage: Option<f64>,
    pub requires_enhanced_dd: bool,
    pub discovery_reason: Option<String>,
    pub discovery_depth: i32,
    pub red_flags: Vec<RedFlagInfo>,
    pub doc_stats: DocStats,
    pub screening_stats: ScreeningStats,
    pub children: Vec<WorkstreamNode>,
}

/// Red flag information
#[derive(Debug, Clone, Serialize)]
pub struct RedFlagInfo {
    pub red_flag_id: Uuid,
    pub flag_type: String,
    pub severity: String,
    pub status: String,
    pub description: String,
    pub source: Option<String>,
    pub raised_at: String,
}

/// Document statistics for a workstream
#[derive(Debug, Clone, Serialize, Default)]
pub struct DocStats {
    pub pending: i64,
    pub received: i64,
    pub verified: i64,
    pub rejected: i64,
}

/// Screening statistics for a workstream
#[derive(Debug, Clone, Serialize, Default)]
pub struct ScreeningStats {
    pub clear: i64,
    pub pending_review: i64,
    pub confirmed_hits: i64,
}

/// Overall case statistics
#[derive(Debug, Clone, Serialize)]
pub struct CaseStats {
    pub total_workstreams: usize,
    pub completed_workstreams: usize,
    pub blocked_workstreams: usize,
    pub open_red_flags: usize,
    pub hard_stops: usize,
    pub pending_docs: usize,
    pub pending_screenings: usize,
}

impl CaseTreeBuilder {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: VisualizationRepository::new(pool),
        }
    }

    /// Build visualization for a case
    pub async fn build(&self, case_id: Uuid) -> Result<CaseVisualization> {
        // Load case info
        let case_info = self.repo.get_case(case_id).await?;

        // Load CBU name
        let cbu = self.repo.get_cbu_for_tree(case_info.cbu_id).await?;

        // Load all workstreams for this case
        let workstreams = self.repo.get_case_workstreams(case_id).await?;

        // Load all red flags for this case
        let red_flags = self.repo.get_case_red_flags(case_id).await?;

        // Build workstream nodes with stats
        let mut nodes: HashMap<Uuid, WorkstreamNode> = HashMap::new();
        let mut root_ids: Vec<Uuid> = Vec::new();

        for ws in &workstreams {
            // Get document stats for this workstream
            let doc_stats = self.repo.get_workstream_doc_stats(ws.workstream_id).await?;

            // Get screening stats for this workstream
            let screening_stats = self
                .repo
                .get_workstream_screening_stats(ws.workstream_id)
                .await?;

            // Get red flags for this workstream
            let ws_flags: Vec<RedFlagInfo> = red_flags
                .iter()
                .filter(|rf| rf.workstream_id == Some(ws.workstream_id))
                .map(|rf| RedFlagInfo {
                    red_flag_id: rf.red_flag_id,
                    flag_type: rf.flag_type.clone(),
                    severity: rf.severity.clone(),
                    status: rf.status.clone(),
                    description: rf.description.clone(),
                    source: rf.source.clone(),
                    raised_at: rf.raised_at.to_rfc3339(),
                })
                .collect();

            let node = WorkstreamNode {
                workstream_id: ws.workstream_id,
                entity_id: ws.entity_id,
                entity_name: ws.entity_name.clone(),
                entity_type: ws.entity_type.clone(),
                jurisdiction: ws.jurisdiction.clone(),
                status: ws.status.clone(),
                risk_rating: ws.risk_rating.clone(),
                is_ubo: ws.is_ubo,
                ownership_percentage: ws.ownership_percentage,
                requires_enhanced_dd: ws.requires_enhanced_dd,
                discovery_reason: ws.discovery_reason.clone(),
                discovery_depth: ws.discovery_depth,
                red_flags: ws_flags,
                doc_stats: DocStats {
                    pending: doc_stats.pending,
                    received: doc_stats.received,
                    verified: doc_stats.verified,
                    rejected: doc_stats.rejected,
                },
                screening_stats: ScreeningStats {
                    clear: screening_stats.clear,
                    pending_review: screening_stats.pending_review,
                    confirmed_hits: screening_stats.confirmed_hits,
                },
                children: vec![],
            };

            nodes.insert(ws.workstream_id, node);

            if ws.discovery_source_workstream_id.is_none() {
                root_ids.push(ws.workstream_id);
            }
        }

        // Build tree by linking children to parents
        for ws in &workstreams {
            if let Some(parent_id) = ws.discovery_source_workstream_id {
                if let Some(child_node) = nodes.get(&ws.workstream_id).cloned() {
                    if let Some(parent_node) = nodes.get_mut(&parent_id) {
                        parent_node.children.push(child_node);
                    }
                }
            }
        }

        // Get root nodes only (remove children from main map)
        let workstream_tree: Vec<WorkstreamNode> = root_ids
            .iter()
            .filter_map(|id| nodes.get(id).cloned())
            .collect();

        // Case-level red flags (not tied to workstream)
        let case_red_flags: Vec<RedFlagInfo> = red_flags
            .iter()
            .filter(|rf| rf.workstream_id.is_none())
            .map(|rf| RedFlagInfo {
                red_flag_id: rf.red_flag_id,
                flag_type: rf.flag_type.clone(),
                severity: rf.severity.clone(),
                status: rf.status.clone(),
                description: rf.description.clone(),
                source: rf.source.clone(),
                raised_at: rf.raised_at.to_rfc3339(),
            })
            .collect();

        // Calculate stats
        let stats = CaseStats {
            total_workstreams: workstreams.len(),
            completed_workstreams: workstreams
                .iter()
                .filter(|w| w.status == "COMPLETE")
                .count(),
            blocked_workstreams: workstreams.iter().filter(|w| w.status == "BLOCKED").count(),
            open_red_flags: red_flags
                .iter()
                .filter(|r| r.status == "OPEN" || r.status == "BLOCKING")
                .count(),
            hard_stops: red_flags
                .iter()
                .filter(|r| {
                    r.severity == "HARD_STOP"
                        && r.status != "MITIGATED"
                        && r.status != "WAIVED"
                        && r.status != "CLOSED"
                })
                .count(),
            pending_docs: workstreams
                .iter()
                .map(|w| {
                    nodes
                        .get(&w.workstream_id)
                        .map(|n| n.doc_stats.pending)
                        .unwrap_or(0)
                })
                .sum::<i64>() as usize,
            pending_screenings: workstreams
                .iter()
                .map(|w| {
                    nodes
                        .get(&w.workstream_id)
                        .map(|n| n.screening_stats.pending_review)
                        .unwrap_or(0)
                })
                .sum::<i64>() as usize,
        };

        Ok(CaseVisualization {
            case_id,
            cbu_id: case_info.cbu_id,
            cbu_name: cbu.name,
            status: case_info.status,
            escalation_level: case_info.escalation_level,
            risk_rating: case_info.risk_rating,
            case_type: case_info.case_type,
            sla_deadline: case_info.sla_deadline.map(|d| d.to_rfc3339()),
            workstream_tree,
            case_red_flags,
            stats,
        })
    }

    /// List all cases for a CBU
    pub async fn list_cases_for_cbu(&self, cbu_id: Uuid) -> Result<Vec<CaseSummary>> {
        self.repo.list_cases_for_cbu(cbu_id).await
    }
}

/// Summary of a case for listing
#[derive(Debug, Clone, Serialize)]
pub struct CaseSummary {
    pub case_id: Uuid,
    pub cbu_id: Uuid,
    pub status: String,
    pub escalation_level: String,
    pub risk_rating: Option<String>,
    pub case_type: Option<String>,
    pub opened_at: String,
    pub workstream_count: i64,
    pub open_red_flags: i64,
}
