//! Color palettes for the graph visualization
//!
//! Provides consistent colors for risk ratings, KYC status, entity types, etc.

#![allow(dead_code)]

use egui::Color32;

use super::types::{EntityType, PrimaryRole};

// =============================================================================
// RISK RATING COLORS
// =============================================================================

/// Risk rating levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskRating {
    Unrated,
    Standard,
    Low,
    Medium,
    High,
    Prohibited,
}

impl std::str::FromStr for RiskRating {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_uppercase().as_str() {
            "UNRATED" | "NOT_RATED" => Self::Unrated,
            "STANDARD" => Self::Standard,
            "LOW" => Self::Low,
            "MEDIUM" => Self::Medium,
            "HIGH" => Self::High,
            "PROHIBITED" | "VERY_HIGH" => Self::Prohibited,
            _ => Self::Unrated,
        })
    }
}

/// Get color for risk rating
pub fn risk_color(rating: RiskRating) -> Color32 {
    match rating {
        RiskRating::Unrated => Color32::from_rgb(158, 158, 158), // Gray
        RiskRating::Standard => Color32::from_rgb(76, 175, 80),  // Green
        RiskRating::Low => Color32::from_rgb(139, 195, 74),      // Light green
        RiskRating::Medium => Color32::from_rgb(255, 193, 7),    // Amber
        RiskRating::High => Color32::from_rgb(255, 87, 34),      // Deep orange
        RiskRating::Prohibited => Color32::from_rgb(33, 33, 33), // Near black
    }
}

// =============================================================================
// KYC STATUS COLORS
// =============================================================================

/// KYC status levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KycStatus {
    NotStarted,
    Pending,
    InProgress,
    Verified,
    Rejected,
    Expired,
}

impl std::str::FromStr for KycStatus {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_uppercase().replace('-', "_").as_str() {
            "NOT_STARTED" | "NOTSTARTED" => Self::NotStarted,
            "PENDING" => Self::Pending,
            "IN_PROGRESS" | "INPROGRESS" => Self::InProgress,
            "VERIFIED" | "APPROVED" | "COMPLETE" => Self::Verified,
            "REJECTED" | "FAILED" => Self::Rejected,
            "EXPIRED" => Self::Expired,
            _ => Self::NotStarted,
        })
    }
}

/// Get color for KYC status
pub fn kyc_status_color(status: KycStatus) -> Color32 {
    match status {
        KycStatus::NotStarted => Color32::from_rgb(158, 158, 158), // Gray
        KycStatus::Pending => Color32::from_rgb(255, 193, 7),      // Amber
        KycStatus::InProgress => Color32::from_rgb(33, 150, 243),  // Blue
        KycStatus::Verified => Color32::from_rgb(76, 175, 80),     // Green
        KycStatus::Rejected => Color32::from_rgb(244, 67, 54),     // Red
        KycStatus::Expired => Color32::from_rgb(121, 85, 72),      // Brown
    }
}

// =============================================================================
// ENTITY TYPE COLORS
// =============================================================================

/// Get fill color for entity type (neutral palette)
pub fn entity_type_fill(entity_type: EntityType) -> Color32 {
    match entity_type {
        EntityType::ProperPerson => Color32::from_rgb(100, 181, 246), // Light blue
        EntityType::LimitedCompany => Color32::from_rgb(144, 164, 174), // Blue-gray
        EntityType::Partnership => Color32::from_rgb(129, 199, 132),  // Light green
        EntityType::Trust => Color32::from_rgb(206, 147, 216),        // Light purple
        EntityType::Fund => Color32::from_rgb(178, 223, 219),         // Teal
        EntityType::Product => Color32::from_rgb(168, 85, 247),       // Purple (service layer)
        EntityType::Service => Color32::from_rgb(96, 165, 250),       // Blue (service layer)
        EntityType::Resource => Color32::from_rgb(74, 222, 128),      // Green (service layer)
        EntityType::Unknown => Color32::from_rgb(176, 190, 197),      // Gray
    }
}

/// Get border color for entity type
pub fn entity_type_border(entity_type: EntityType) -> Color32 {
    match entity_type {
        EntityType::ProperPerson => Color32::from_rgb(25, 118, 210), // Blue
        EntityType::LimitedCompany => Color32::from_rgb(69, 90, 100), // Blue-gray dark
        EntityType::Partnership => Color32::from_rgb(56, 142, 60),   // Green
        EntityType::Trust => Color32::from_rgb(142, 36, 170),        // Purple
        EntityType::Fund => Color32::from_rgb(0, 137, 123),          // Teal dark
        EntityType::Product => Color32::from_rgb(88, 28, 135),       // Purple dark (service layer)
        EntityType::Service => Color32::from_rgb(30, 58, 138),       // Blue dark (service layer)
        EntityType::Resource => Color32::from_rgb(20, 83, 45),       // Green dark (service layer)
        EntityType::Unknown => Color32::from_rgb(96, 125, 139),      // Gray dark
    }
}

// =============================================================================
// ROLE COLORS
// =============================================================================

/// Get color for primary role
pub fn role_color(role: PrimaryRole) -> Color32 {
    match role {
        // Ownership/Control - Green family (KYC focus)
        PrimaryRole::UltimateBeneficialOwner => Color32::from_rgb(76, 175, 80), // Green
        PrimaryRole::BeneficialOwner => Color32::from_rgb(102, 187, 106),       // Light green
        PrimaryRole::Shareholder => Color32::from_rgb(139, 195, 74),            // Lime
        PrimaryRole::GeneralPartner => Color32::from_rgb(67, 160, 71),          // Green darker
        PrimaryRole::LimitedPartner => Color32::from_rgb(129, 199, 132),        // Green lighter
        // Governance - Blue family (KYC focus)
        PrimaryRole::Director => Color32::from_rgb(33, 150, 243), // Blue
        PrimaryRole::Officer => Color32::from_rgb(3, 169, 244),   // Light blue
        PrimaryRole::ConductingOfficer => Color32::from_rgb(0, 188, 212), // Cyan
        PrimaryRole::ChiefComplianceOfficer => Color32::from_rgb(0, 151, 167), // Cyan dark
        PrimaryRole::Trustee => Color32::from_rgb(121, 85, 72),   // Brown
        PrimaryRole::Protector => Color32::from_rgb(96, 125, 139), // Blue-gray
        PrimaryRole::Beneficiary => Color32::from_rgb(77, 208, 225), // Cyan light
        PrimaryRole::Settlor => Color32::from_rgb(233, 30, 99),   // Pink
        // Fund structure - Purple/Orange family (Trading focus)
        PrimaryRole::Principal => Color32::from_rgb(156, 39, 176), // Purple
        PrimaryRole::AssetOwner => Color32::from_rgb(171, 71, 188), // Purple light
        PrimaryRole::MasterFund => Color32::from_rgb(123, 31, 162), // Purple dark
        PrimaryRole::FeederFund => Color32::from_rgb(186, 104, 200), // Purple lighter
        PrimaryRole::SegregatedPortfolio => Color32::from_rgb(149, 117, 205), // Deep purple
        PrimaryRole::ManagementCompany => Color32::from_rgb(255, 152, 0), // Orange
        PrimaryRole::InvestmentManager => Color32::from_rgb(255, 167, 38), // Orange light
        PrimaryRole::InvestmentAdvisor => Color32::from_rgb(255, 183, 77), // Orange lighter
        PrimaryRole::Sponsor => Color32::from_rgb(251, 140, 0),    // Orange dark
        // Service providers - Teal/Gray family
        PrimaryRole::Administrator => Color32::from_rgb(0, 150, 136), // Teal
        PrimaryRole::Custodian => Color32::from_rgb(38, 166, 154),    // Teal light
        PrimaryRole::Depositary => Color32::from_rgb(0, 137, 123),    // Teal dark
        PrimaryRole::TransferAgent => Color32::from_rgb(77, 182, 172), // Teal lighter
        PrimaryRole::Distributor => Color32::from_rgb(128, 203, 196), // Teal lightest
        PrimaryRole::PrimeBroker => Color32::from_rgb(255, 87, 34),   // Deep orange
        PrimaryRole::Auditor => Color32::from_rgb(120, 144, 156),     // Blue-gray
        PrimaryRole::LegalCounsel => Color32::from_rgb(84, 110, 122), // Blue-gray dark
        // Other
        PrimaryRole::AuthorizedSignatory => Color32::from_rgb(63, 81, 181), // Indigo
        PrimaryRole::ContactPerson => Color32::from_rgb(158, 158, 158),     // Gray
        PrimaryRole::CommercialClient => Color32::from_rgb(255, 193, 7),    // Amber
        PrimaryRole::Unknown => Color32::from_rgb(117, 117, 117),           // Dark gray
    }
}

/// Get role badge background color (muted version)
pub fn role_badge_background(role: PrimaryRole) -> Color32 {
    let base = role_color(role);
    // Lighten and reduce saturation
    Color32::from_rgba_unmultiplied(
        (base.r() as u16 + 200).min(255) as u8 / 2 + 100,
        (base.g() as u16 + 200).min(255) as u8 / 2 + 100,
        (base.b() as u16 + 200).min(255) as u8 / 2 + 100,
        40,
    )
}

// =============================================================================
// EDGE COLORS
// =============================================================================

use super::types::EdgeType;

/// Get color for edge type
pub fn edge_color(edge_type: EdgeType) -> Color32 {
    match edge_type {
        EdgeType::HasRole => Color32::from_rgb(107, 114, 128), // Gray
        EdgeType::Owns => Color32::from_rgb(34, 197, 94),      // Green
        EdgeType::Controls => Color32::from_rgb(251, 191, 36), // Amber
        EdgeType::Other => Color32::from_rgb(156, 163, 175),   // Light gray
    }
}

// =============================================================================
// FOCUS COLORS
// =============================================================================

/// Color for focused node highlight
pub fn focus_highlight_color() -> Color32 {
    Color32::from_rgb(59, 130, 246) // Blue-500
}

/// Color for connected node highlight
pub fn connected_highlight_color() -> Color32 {
    Color32::from_rgb(147, 197, 253) // Blue-300
}

/// Opacity multiplier for non-focused elements
pub const BLUR_OPACITY: f32 = 0.25;

// =============================================================================
// UI CHROME COLORS
// =============================================================================

/// Background color for info panels
pub fn panel_background() -> Color32 {
    Color32::from_rgba_unmultiplied(255, 255, 255, 245)
}

/// Border color for panels
pub fn panel_border() -> Color32 {
    Color32::from_rgb(229, 231, 235)
}

/// Text color for labels
pub fn label_text_color() -> Color32 {
    Color32::from_rgb(107, 114, 128)
}

/// Text color for primary content
pub fn primary_text_color() -> Color32 {
    Color32::from_rgb(31, 41, 55)
}

/// Text color for secondary content
pub fn secondary_text_color() -> Color32 {
    Color32::from_rgb(156, 163, 175)
}
