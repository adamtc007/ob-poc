//! UBO prong analysis and classification

use super::traversal::OwnershipPath;

#[derive(Debug, Clone)]
pub(crate) struct UboProng {
    pub prong_id: String,
    pub path: Vec<String>,
    pub ownership_percent: f64,
    pub effective_ownership: f64,
    pub status: ProngStatus,
    pub prong_type: ProngType,
}

#[derive(Debug, Clone)]
pub(crate) enum ProngStatus {
    Identified,
    BelowThreshold,
    BlockedBlindTrust,
    HighOpacity,
    DataConflict,
}

#[derive(Debug, Clone)]
pub(crate) enum ProngType {
    Ownership,
    Control,
    Mixed,
}

pub(crate) fn analyze_prongs(paths: Vec<OwnershipPath>, threshold: f64) -> Vec<UboProng> {
    paths
        .into_iter()
        .enumerate()
        .map(|(i, path)| {
            let status = if path.effective_ownership >= threshold {
                ProngStatus::Identified
            } else {
                ProngStatus::BelowThreshold
            };

            UboProng {
                prong_id: format!("prong-{}", i + 1),
                path: path.nodes,
                ownership_percent: path.effective_ownership,
                effective_ownership: path.effective_ownership,
                status,
                prong_type: ProngType::Ownership,
            }
        })
        .collect()
}
