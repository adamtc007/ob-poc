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
        // Trading layer types - Orange/Amber family
        EntityType::TradingProfile => Color32::from_rgb(251, 191, 36), // Amber-400
        EntityType::InstrumentMatrix => Color32::from_rgb(253, 224, 71), // Yellow-300
        EntityType::InstrumentClass => Color32::from_rgb(254, 215, 170), // Orange-200
        EntityType::Market => Color32::from_rgb(134, 239, 172),        // Green-300 (exchanges)
        EntityType::Counterparty => Color32::from_rgb(196, 181, 253),  // Violet-300
        EntityType::IsdaAgreement => Color32::from_rgb(252, 165, 165), // Red-300 (legal)
        EntityType::CsaAgreement => Color32::from_rgb(253, 186, 186),  // Red-200 (legal, lighter)
        // Control layer - hexagon portal (default color, actual color from confidence)
        EntityType::ControlPortal => Color32::from_rgb(147, 197, 253), // Blue-300 (neutral)
        EntityType::Unknown => Color32::from_rgb(176, 190, 197),       // Gray
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
        // Trading layer types - darker versions
        EntityType::TradingProfile => Color32::from_rgb(217, 119, 6), // Amber-600
        EntityType::InstrumentMatrix => Color32::from_rgb(202, 138, 4), // Yellow-600
        EntityType::InstrumentClass => Color32::from_rgb(234, 88, 12), // Orange-600
        EntityType::Market => Color32::from_rgb(22, 163, 74),         // Green-600
        EntityType::Counterparty => Color32::from_rgb(124, 58, 237),  // Violet-600
        EntityType::IsdaAgreement => Color32::from_rgb(220, 38, 38),  // Red-600
        EntityType::CsaAgreement => Color32::from_rgb(239, 68, 68),   // Red-500
        // Control layer
        EntityType::ControlPortal => Color32::from_rgb(30, 64, 175), // Blue-800
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
        // Core edge types
        EdgeType::HasRole => Color32::from_rgb(107, 114, 128), // Gray
        EdgeType::Owns => Color32::from_rgb(34, 197, 94),      // Green
        EdgeType::Controls => Color32::from_rgb(251, 191, 36), // Amber
        EdgeType::UboTerminus => Color32::from_rgb(239, 68, 68), // Red - terminus marker
        // Service layer edge types
        EdgeType::UsesProduct => Color32::from_rgb(168, 85, 247), // Purple
        EdgeType::DeliversService => Color32::from_rgb(96, 165, 250), // Blue
        EdgeType::ProvidesResource => Color32::from_rgb(74, 222, 128), // Green
        // Trading layer edge types
        EdgeType::HasTradingProfile => Color32::from_rgb(217, 119, 6), // Amber-600
        EdgeType::HasMatrix => Color32::from_rgb(202, 138, 4),         // Yellow-600
        EdgeType::IncludesClass => Color32::from_rgb(234, 88, 12),     // Orange-600
        EdgeType::TradedOn => Color32::from_rgb(22, 163, 74),          // Green-600
        EdgeType::OtcCounterparty => Color32::from_rgb(124, 58, 237),  // Violet-600 (dashed)
        EdgeType::CoveredByIsda => Color32::from_rgb(220, 38, 38),     // Red-600 (dashed)
        EdgeType::HasCsa => Color32::from_rgb(239, 68, 68),            // Red-500
        EdgeType::ImMandate => Color32::from_rgb(59, 130, 246),        // Blue-500 (dashed)
        // Control layer
        EdgeType::BoardController => Color32::from_rgb(168, 85, 247), // Purple-500
        EdgeType::Other => Color32::from_rgb(156, 163, 175),          // Light gray
    }
}

// =============================================================================
// CONTROL PORTAL COLORS (confidence-based)
// =============================================================================

/// Confidence level for control computation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlConfidence {
    High,
    Medium,
    Low,
}

impl std::str::FromStr for ControlConfidence {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "high" => Self::High,
            "medium" => Self::Medium,
            "low" => Self::Low,
            _ => Self::Medium, // Default to medium for unknown
        })
    }
}

/// Get fill color for control portal based on confidence level
/// High = green, Medium = amber, Low = red
pub fn control_portal_fill(confidence: ControlConfidence) -> Color32 {
    match confidence {
        ControlConfidence::High => Color32::from_rgb(34, 197, 94), // Green-500
        ControlConfidence::Medium => Color32::from_rgb(251, 191, 36), // Amber-400
        ControlConfidence::Low => Color32::from_rgb(239, 68, 68),  // Red-500
    }
}

/// Get border color for control portal based on confidence level
pub fn control_portal_border(confidence: ControlConfidence) -> Color32 {
    match confidence {
        ControlConfidence::High => Color32::from_rgb(22, 163, 74), // Green-600
        ControlConfidence::Medium => Color32::from_rgb(217, 119, 6), // Amber-600
        ControlConfidence::Low => Color32::from_rgb(220, 38, 38),  // Red-600
    }
}

/// Get glow color for control portal (used for hover/focus effects)
pub fn control_portal_glow(confidence: ControlConfidence) -> Color32 {
    match confidence {
        ControlConfidence::High => Color32::from_rgba_unmultiplied(34, 197, 94, 80),
        ControlConfidence::Medium => Color32::from_rgba_unmultiplied(251, 191, 36, 80),
        ControlConfidence::Low => Color32::from_rgba_unmultiplied(239, 68, 68, 80),
    }
}

// =============================================================================
// VERIFICATION STATUS COLORS
// =============================================================================

/// Edge style based on verification status
#[derive(Debug, Clone, Copy)]
pub struct VerificationEdgeStyle {
    pub color: Color32,
    pub width_multiplier: f32,
    pub dashed: bool,
}

/// Get edge style based on verification status
/// - "proven": solid green, normal width
/// - "alleged": dashed amber, normal width
/// - "disputed": solid red, thicker
/// - "pending": dashed gray, thinner
/// - None: use default edge type color
pub fn verification_edge_style(status: Option<&str>) -> Option<VerificationEdgeStyle> {
    status.map(|s| match s.to_lowercase().as_str() {
        "proven" => VerificationEdgeStyle {
            color: Color32::from_rgb(34, 197, 94), // Green-500
            width_multiplier: 1.0,
            dashed: false,
        },
        "alleged" => VerificationEdgeStyle {
            color: Color32::from_rgb(251, 191, 36), // Amber-400
            width_multiplier: 1.0,
            dashed: true,
        },
        "disputed" => VerificationEdgeStyle {
            color: Color32::from_rgb(239, 68, 68), // Red-500
            width_multiplier: 1.5,
            dashed: false,
        },
        "pending" => VerificationEdgeStyle {
            color: Color32::from_rgb(156, 163, 175), // Gray-400
            width_multiplier: 0.8,
            dashed: true,
        },
        _ => VerificationEdgeStyle {
            color: Color32::from_rgb(107, 114, 128), // Gray-500
            width_multiplier: 1.0,
            dashed: false,
        },
    })
}

/// Get edge width based on ownership weight (0-100)
/// Returns multiplier: 1.0 for 0%, up to 2.5 for 100%
pub fn edge_width_for_weight(weight: Option<f32>) -> f32 {
    match weight {
        Some(w) => {
            let clamped = w.clamp(0.0, 100.0);
            1.0 + (clamped / 100.0) * 1.5 // 1.0 to 2.5
        }
        None => 1.0,
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

// =============================================================================
// CAPITAL STRUCTURE COLORS
// =============================================================================

/// Get color for share class by instrument kind
pub fn share_class_color(instrument_kind: &str, is_voting: bool) -> Color32 {
    if !is_voting {
        return Color32::from_rgb(156, 163, 175); // Gray for non-voting
    }
    match instrument_kind.to_uppercase().as_str() {
        "ORDINARY_EQUITY" => Color32::from_rgb(100, 149, 237), // Cornflower blue
        "PREFERENCE_EQUITY" => Color32::from_rgb(255, 215, 0), // Gold
        "FUND_UNIT" => Color32::from_rgb(144, 238, 144),       // Light green
        "FUND_SHARE" => Color32::from_rgb(134, 239, 172),      // Green-300
        "LP_INTEREST" => Color32::from_rgb(221, 160, 221),     // Plum
        "GP_INTEREST" => Color32::from_rgb(255, 182, 193),     // Light pink
        "DEBT" => Color32::from_rgb(176, 190, 197),            // Blue-gray
        "CONVERTIBLE" | "CONVERTIBLE_NOTE" => Color32::from_rgb(255, 165, 0), // Orange
        "WARRANT" => Color32::from_rgb(253, 224, 71),          // Yellow-300
        "SAFE" => Color32::from_rgb(251, 191, 36),             // Amber-400
        _ => Color32::from_rgb(192, 192, 192),                 // Silver
    }
}

/// Get color for control edge based on source and status
pub fn control_edge_color(
    has_control: bool,
    has_significant_influence: bool,
    derived_from: &str,
) -> Color32 {
    match (
        has_control,
        has_significant_influence,
        derived_from.to_uppercase().as_str(),
    ) {
        (true, _, "REGISTER") => Color32::from_rgb(220, 38, 38), // Red-600 - proven control
        (true, _, "BODS") => Color32::from_rgb(255, 140, 0),     // Dark orange
        (true, _, "GLEIF") => Color32::from_rgb(234, 179, 8),    // Yellow-500
        (true, _, _) => Color32::from_rgb(239, 68, 68),          // Red-500
        (false, true, _) => Color32::from_rgb(251, 146, 60),     // Orange-400 - significant
        (false, false, _) => Color32::from_rgb(156, 163, 175),   // Gray-400
    }
}

/// Get color for special right indicator
pub fn special_right_color(right_type: &str) -> Color32 {
    match right_type.to_uppercase().as_str() {
        "BOARD_APPOINTMENT" | "BOARD_OBSERVER" => Color32::from_rgb(138, 43, 226), // Blue violet
        "VETO_MA" | "VETO_FUNDRAISE" | "VETO_BUDGET" => Color32::from_rgb(220, 20, 60), // Crimson
        "CONSENT_REQUIRED" => Color32::from_rgb(255, 87, 34),                      // Deep orange
        "ANTI_DILUTION" => Color32::from_rgb(255, 193, 7),                         // Amber
        "DRAG_ALONG" | "TAG_ALONG" => Color32::from_rgb(59, 130, 246),             // Blue-500
        "PREEMPTION" | "ROFR" => Color32::from_rgb(16, 185, 129),                  // Emerald-500
        _ => Color32::from_rgb(75, 0, 130),                                        // Indigo
    }
}

/// Get icon for share class by instrument kind
pub fn share_class_icon(instrument_kind: &str) -> &'static str {
    match instrument_kind.to_uppercase().as_str() {
        "ORDINARY_EQUITY" => "🏛️",
        "PREFERENCE_EQUITY" => "⭐",
        "FUND_UNIT" => "📊",
        "FUND_SHARE" => "📈",
        "LP_INTEREST" => "🤝",
        "GP_INTEREST" => "👔",
        "DEBT" => "📜",
        "CONVERTIBLE" | "CONVERTIBLE_NOTE" => "🔄",
        "WARRANT" => "📋",
        "SAFE" => "🛡️",
        _ => "📄",
    }
}

/// Get control indicator icons
pub fn control_indicator(
    has_control: bool,
    has_board_rights: bool,
    has_veto: bool,
) -> &'static str {
    match (has_control, has_board_rights, has_veto) {
        (true, true, true) => "⚡🪑🚫", // Control + board + veto
        (true, true, false) => "⚡🪑",  // Control + board
        (true, false, true) => "⚡🚫",  // Control + veto
        (true, false, false) => "⚡",   // Control only
        (false, true, true) => "🪑🚫",  // Board + veto
        (false, true, false) => "🪑",   // Board only
        (false, false, true) => "🚫",   // Veto only
        (false, false, false) => "",    // None
    }
}

/// Get color for dilution instrument type
pub fn dilution_instrument_color(instrument_type: &str) -> Color32 {
    match instrument_type.to_uppercase().as_str() {
        "STOCK_OPTION" => Color32::from_rgb(59, 130, 246), // Blue-500
        "RSU" => Color32::from_rgb(99, 102, 241),          // Indigo-500
        "WARRANT" => Color32::from_rgb(234, 179, 8),       // Yellow-500
        "CONVERTIBLE_NOTE" => Color32::from_rgb(249, 115, 22), // Orange-500
        "SAFE" => Color32::from_rgb(251, 191, 36),         // Amber-400
        "CONVERTIBLE_PREFERRED" => Color32::from_rgb(168, 85, 247), // Purple-500
        "PHANTOM_STOCK" | "SAR" => Color32::from_rgb(156, 163, 175), // Gray-400
        _ => Color32::from_rgb(107, 114, 128),             // Gray-500
    }
}

/// Get color for reconciliation finding severity
pub fn reconciliation_severity_color(severity: &str) -> Color32 {
    match severity.to_uppercase().as_str() {
        "CRITICAL" => Color32::from_rgb(220, 38, 38), // Red-600
        "HIGH" => Color32::from_rgb(239, 68, 68),     // Red-500
        "MEDIUM" => Color32::from_rgb(251, 191, 36),  // Amber-400
        "LOW" => Color32::from_rgb(234, 179, 8),      // Yellow-500
        "INFO" => Color32::from_rgb(59, 130, 246),    // Blue-500
        _ => Color32::from_rgb(156, 163, 175),        // Gray-400
    }
}
