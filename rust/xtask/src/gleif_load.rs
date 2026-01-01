//! Load Allianz GLEIF data into the database via DSL
//!
//! This module generates DSL from the GLEIF JSON files and executes it.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Entity data from GLEIF Level 2
#[derive(Debug, Deserialize)]
pub struct GleifEntity {
    pub name: String,
    pub lei: String,
    pub jurisdiction: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub legal_form: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub registration_number: Option<String>,
    #[serde(default)]
    pub validation_level: Option<String>,
    #[serde(default)]
    pub next_renewal: Option<String>,
    #[serde(default)]
    pub parent_exception: Option<String>,
    #[serde(default)]
    pub address: Option<GleifAddress>,
    #[serde(default)]
    pub direct_parent: Option<ParentRelationship>,
    #[serde(default)]
    pub ultimate_parent: Option<ParentRelationship>,
}

#[derive(Debug, Deserialize)]
pub struct GleifAddress {
    #[serde(default)]
    pub city: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ParentRelationship {
    pub parent_lei: String,
    #[serde(default)]
    pub relationship_type: Option<String>,
    #[serde(default)]
    pub corroboration: Option<String>,
}

/// Level 2 data file structure
#[derive(Debug, Deserialize)]
pub struct Level2Data {
    pub entities: HashMap<String, GleifEntity>,
}

/// Ownership chain file structure
#[derive(Debug, Deserialize)]
pub struct OwnershipChain {
    pub ownership_chain: Vec<OwnershipChainEntity>,
    pub relationships: Vec<Relationship>,
    pub subsidiaries: Vec<Subsidiary>,
    #[serde(default)]
    pub managed_funds_count: usize,
    #[serde(default)]
    pub managed_funds_sample: Vec<ManagedFund>,
}

#[derive(Debug, Deserialize)]
pub struct OwnershipChainEntity {
    pub lei: String,
    pub legal_name: String,
    pub jurisdiction: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub legal_form_code: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub registration_number: Option<String>,
    #[serde(default)]
    pub city: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub ubo_terminus: Option<bool>,
    #[serde(default)]
    pub terminus_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Relationship {
    pub child_lei: String,
    pub parent_lei: String,
    #[serde(rename = "type")]
    pub relationship_type: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub corroboration: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Subsidiary {
    pub lei: String,
    pub legal_name: String,
    pub jurisdiction: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub legal_form_code: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub registration_number: Option<String>,
    #[serde(default)]
    pub city: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub parent_lei: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ManagedFund {
    pub manco_lei: String,
    pub fund_lei: String,
    pub fund_name: String,
    pub fund_jurisdiction: String,
}

// ============================================================================
// New types for complete funds file (allianzgi_funds_complete.json)
// ============================================================================

/// Complete funds file structure with umbrella relationships
#[derive(Debug, Deserialize)]
pub struct FundsComplete {
    pub investment_manager: EntityRef,
    pub ultimate_client: EntityRef,
    pub total_funds: usize,
    pub umbrella_count: usize,
    #[serde(default)]
    pub unique_umbrella_leis: Vec<String>,
    #[serde(default)]
    pub by_jurisdiction: HashMap<String, usize>,
    pub funds: Vec<CompleteFund>,
}

/// Reference to a key entity (IM, Ultimate Client)
#[derive(Debug, Deserialize)]
pub struct EntityRef {
    pub lei: String,
    pub name: String,
}

/// Fund with umbrella relationship info
#[derive(Debug, Deserialize)]
pub struct CompleteFund {
    pub lei: String,
    pub name: String,
    pub jurisdiction: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub legal_form: Option<String>,
    #[serde(default)]
    pub city: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub umbrella_lei: Option<String>,
    #[serde(default)]
    pub umbrella_name: Option<String>,
    #[serde(default)]
    pub is_umbrella: bool,
}

/// Corporate tree file structure
#[derive(Debug, Deserialize)]
pub struct CorporateTree {
    pub parent: CorporateTreeParent,
    pub direct_children_count: usize,
    pub by_jurisdiction: HashMap<String, usize>,
    pub direct_children: Vec<CorporateTreeChild>,
}

#[derive(Debug, Deserialize)]
pub struct CorporateTreeParent {
    pub lei: String,
    pub name: String,
    pub jurisdiction: String,
}

#[derive(Debug, Deserialize)]
pub struct CorporateTreeChild {
    pub lei: String,
    pub name: String,
    pub jurisdiction: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub legal_form: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub city: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
}

/// Convert LEI to a safe DSL binding alias
/// Uses full LEI to avoid collisions (LEIs are 20 chars, globally unique)
fn lei_to_alias(lei: &str) -> String {
    format!("@lei_{}", lei.to_lowercase())
}

/// Escape string for DSL
fn escape_dsl_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ")
        .replace('\r', "")
}

/// Generate DSL for a parent entity (Allianz SE, AllianzGI)
fn generate_parent_entity_dsl(entity: &GleifEntity) -> String {
    let alias = lei_to_alias(&entity.lei);
    let mut lines = vec![
        format!(";; {}", entity.name),
        "(entity.ensure-limited-company".to_string(),
        format!("    :name \"{}\"", escape_dsl_string(&entity.name)),
        format!("    :lei \"{}\"", entity.lei),
        format!("    :jurisdiction \"{}\"", entity.jurisdiction),
    ];

    if let Some(ref reg) = entity.registration_number {
        lines.push(format!(
            "    :registration-number \"{}\"",
            escape_dsl_string(reg)
        ));
    }
    if let Some(ref addr) = entity.address {
        if let Some(ref city) = addr.city {
            lines.push(format!("    :city \"{}\"", escape_dsl_string(city)));
        }
    }
    if let Some(ref status) = entity.status {
        lines.push(format!("    :gleif-status \"{}\"", status));
    }
    if let Some(ref category) = entity.category {
        lines.push(format!("    :gleif-category \"{}\"", category));
    }
    if let Some(ref legal_form) = entity.legal_form {
        lines.push(format!("    :legal-form-code \"{}\"", legal_form));
    }
    if let Some(ref val) = entity.validation_level {
        lines.push(format!("    :gleif-validation-level \"{}\"", val));
    }
    if let Some(ref dp) = entity.direct_parent {
        lines.push(format!("    :direct-parent-lei \"{}\"", dp.parent_lei));
    }
    if let Some(ref up) = entity.ultimate_parent {
        lines.push(format!("    :ultimate-parent-lei \"{}\"", up.parent_lei));
    }
    if let Some(ref exc) = entity.parent_exception {
        lines.push(format!("    :parent-exception \"{}\"", exc));
    }

    lines.push(format!("    :as {})", alias));
    lines.join("\n")
}

/// Generate DSL for a subsidiary
fn generate_subsidiary_dsl(sub: &Subsidiary) -> String {
    let alias = lei_to_alias(&sub.lei);
    let mut lines = vec![
        format!(";; {}", sub.legal_name),
        "(entity.ensure-limited-company".to_string(),
        format!("    :name \"{}\"", escape_dsl_string(&sub.legal_name)),
        format!("    :lei \"{}\"", sub.lei),
        format!("    :jurisdiction \"{}\"", sub.jurisdiction),
    ];

    if let Some(ref reg) = sub.registration_number {
        lines.push(format!(
            "    :registration-number \"{}\"",
            escape_dsl_string(reg)
        ));
    }
    if let Some(ref city) = sub.city {
        lines.push(format!("    :city \"{}\"", escape_dsl_string(city)));
    }
    if let Some(ref status) = sub.status {
        lines.push(format!("    :gleif-status \"{}\"", status));
    }
    if let Some(ref category) = sub.category {
        lines.push(format!("    :gleif-category \"{}\"", category));
    }
    if let Some(ref lf) = sub.legal_form_code {
        lines.push(format!("    :legal-form-code \"{}\"", lf));
    }
    if let Some(ref parent) = sub.parent_lei {
        lines.push(format!("    :direct-parent-lei \"{}\"", parent));
    }

    lines.push(format!("    :as {})", alias));
    lines.join("\n")
}

/// Generate DSL for a managed fund entity + CBU (OLD - for legacy managed_funds_sample)
fn generate_fund_dsl(fund: &ManagedFund, im_alias: &str) -> String {
    let entity_alias = lei_to_alias(&fund.fund_lei);
    let cbu_alias = format!("@cbu_{}", fund.fund_lei.to_lowercase());

    // Truncate name if too long
    let name = if fund.fund_name.len() > 200 {
        format!("{}...", &fund.fund_name[..197])
    } else {
        fund.fund_name.clone()
    };

    let mut lines = vec![
        format!(";; Fund: {}", name),
        format!(""),
        format!(";; Step 1: Create fund entity"),
        "(entity.ensure-limited-company".to_string(),
        format!("    :name \"{}\"", escape_dsl_string(&name)),
        format!("    :lei \"{}\"", fund.fund_lei),
        format!("    :jurisdiction \"{}\"", fund.fund_jurisdiction),
        "    :gleif-category \"FUND\"".to_string(),
        format!("    :as {})", entity_alias),
        format!(""),
        format!(";; Step 2: Create CBU for fund onboarding"),
        "(cbu.ensure".to_string(),
        format!("    :name \"{}\"", escape_dsl_string(&name)),
        "    :client-type \"FUND\"".to_string(),
        format!("    :jurisdiction \"{}\"", fund.fund_jurisdiction),
        format!("    :as {})", cbu_alias),
        format!(""),
        format!(";; Step 3: Assign Investment Manager role"),
        "(cbu.assign-role".to_string(),
        format!("    :cbu-id {}", cbu_alias),
        format!("    :entity-id {}", im_alias),
        "    :role \"INVESTMENT_MANAGER\")".to_string(),
    ];

    // Add ManCo role (same as IM for self-managed)
    lines.extend(vec![
        format!(""),
        format!(";; Step 4: Assign ManCo role (self-managed)"),
        "(cbu.assign-role".to_string(),
        format!("    :cbu-id {}", cbu_alias),
        format!("    :entity-id {}", im_alias),
        "    :role \"MANAGEMENT_COMPANY\")".to_string(),
    ]);

    // Note: SICAV is a fund structure type, not a role - handled via entity_funds.fund_structure_type

    lines.join("\n")
}

// ============================================================================
// NEW: Complete fund DSL generation with correct role structure
// ============================================================================

/// Generate DSL for a fund entity from CompleteFund data
fn generate_complete_fund_entity_dsl(fund: &CompleteFund) -> String {
    let alias = lei_to_alias(&fund.lei);

    // Truncate name if too long
    let name = if fund.name.len() > 200 {
        format!("{}...", &fund.name[..197])
    } else {
        fund.name.clone()
    };

    let mut lines = vec![
        format!(";; {}", name),
        "(entity.ensure-limited-company".to_string(),
        format!("    :name \"{}\"", escape_dsl_string(&name)),
        format!("    :lei \"{}\"", fund.lei),
        format!("    :jurisdiction \"{}\"", fund.jurisdiction),
        "    :gleif-category \"FUND\"".to_string(),
    ];

    if let Some(ref status) = fund.status {
        lines.push(format!("    :gleif-status \"{}\"", status));
    }
    if let Some(ref legal_form) = fund.legal_form {
        lines.push(format!("    :legal-form-code \"{}\"", legal_form));
    }
    if let Some(ref city) = fund.city {
        lines.push(format!("    :city \"{}\"", escape_dsl_string(city)));
    }

    lines.push(format!("    :as {})", alias));
    lines.join("\n")
}

/// Generate DSL for a fund CBU with correct role assignments
/// - SICAV role: Only for sub-funds, points to umbrella entity (not the fund itself!)
/// - Ultimate Client role: Allianz SE for all funds
fn generate_fund_cbu_dsl(
    fund: &CompleteFund,
    im: &EntityRef,
    ultimate_client: &EntityRef,
) -> String {
    let entity_alias = lei_to_alias(&fund.lei);
    let cbu_alias = format!("@cbu_{}", fund.lei.to_lowercase());
    let im_alias = lei_to_alias(&im.lei);
    let uc_alias = lei_to_alias(&ultimate_client.lei);

    // Truncate name if too long
    let name = if fund.name.len() > 200 {
        format!("{}...", &fund.name[..197])
    } else {
        fund.name.clone()
    };

    let mut lines = vec![
        format!(";; CBU: {}", name),
        format!(""),
        // Create CBU
        "(cbu.ensure".to_string(),
        format!("    :name \"{}\"", escape_dsl_string(&name)),
        "    :client-type \"FUND\"".to_string(),
        format!("    :jurisdiction \"{}\"", fund.jurisdiction),
        format!("    :as {})", cbu_alias),
        format!(""),
        // Asset Owner role - the fund itself
        "(cbu.assign-role".to_string(),
        format!("    :cbu-id {}", cbu_alias),
        format!("    :entity-id {}", entity_alias),
        "    :role \"ASSET_OWNER\")".to_string(),
        format!(""),
        // Investment Manager role - AllianzGI
        "(cbu.assign-role".to_string(),
        format!("    :cbu-id {}", cbu_alias),
        format!("    :entity-id {}", im_alias),
        "    :role \"INVESTMENT_MANAGER\")".to_string(),
        format!(""),
        // ManCo role - AllianzGI (self-managed)
        "(cbu.assign-role".to_string(),
        format!("    :cbu-id {}", cbu_alias),
        format!("    :entity-id {}", im_alias),
        "    :role \"MANAGEMENT_COMPANY\")".to_string(),
    ];

    // SICAV role - ONLY for sub-funds with umbrella, NOT for umbrellas themselves
    // The SICAV role points to the umbrella entity, not the fund itself!
    if let Some(ref umbrella_lei) = fund.umbrella_lei {
        if !fund.is_umbrella {
            let sicav_alias = lei_to_alias(umbrella_lei);
            lines.extend(vec![
                format!(""),
                format!(
                    ";; SICAV: {} (umbrella)",
                    fund.umbrella_name.as_deref().unwrap_or("Unknown")
                ),
                "(cbu.assign-role".to_string(),
                format!("    :cbu-id {}", cbu_alias),
                format!("    :entity-id {}", sicav_alias),
                "    :role \"SICAV\")".to_string(),
            ]);
        }
    }

    // Ultimate Client role - Allianz SE
    lines.extend(vec![
        format!(""),
        format!(";; Ultimate Client: {}", ultimate_client.name),
        "(cbu.assign-role".to_string(),
        format!("    :cbu-id {}", cbu_alias),
        format!("    :entity-id {}", uc_alias),
        "    :role \"ULTIMATE_CLIENT\")".to_string(),
    ]);

    lines.join("\n")
}

/// Generate full DSL from the complete funds file (allianzgi_funds_complete.json)
pub fn generate_complete_funds_dsl(
    funds_path: &Path,
    level2_path: &Path,
    fund_limit: Option<usize>,
) -> Result<String> {
    use std::collections::HashSet;

    // Load complete funds data
    let funds_content = std::fs::read_to_string(funds_path)
        .with_context(|| format!("Failed to read {}", funds_path.display()))?;
    let funds_data: FundsComplete = serde_json::from_str(&funds_content)
        .with_context(|| format!("Failed to parse {}", funds_path.display()))?;

    // Load Level 2 data for parent entities (Allianz SE, AllianzGI)
    let level2_content = std::fs::read_to_string(level2_path)
        .with_context(|| format!("Failed to read {}", level2_path.display()))?;
    let level2: Level2Data = serde_json::from_str(&level2_content)
        .with_context(|| format!("Failed to parse {}", level2_path.display()))?;

    // Track already-defined LEIs to avoid duplicates
    let mut defined_leis: HashSet<String> = HashSet::new();

    // Build DSL
    let mut dsl_parts = vec![
        ";; ============================================================================"
            .to_string(),
        ";; ALLIANZ GLEIF COMPLETE FUND DATA LOAD".to_string(),
        format!(";; Generated: {}", chrono::Utc::now().to_rfc3339()),
        ";; Source: GLEIF API (api.gleif.org)".to_string(),
        format!(";; Total funds: {}", funds_data.total_funds),
        format!(";; Umbrella SICAVs: {}", funds_data.umbrella_count),
        ";; ============================================================================"
            .to_string(),
        String::new(),
    ];

    // Phase 1: Parent entities from Level 2 data (Allianz SE → AllianzGI)
    dsl_parts.extend(vec![
        ";; ============================================================================"
            .to_string(),
        ";; PHASE 1: Parent Entities (Allianz SE → AllianzGI)".to_string(),
        ";; ============================================================================"
            .to_string(),
        String::new(),
    ]);

    let allianz_se_lei = &funds_data.ultimate_client.lei;
    let allianzgi_lei = &funds_data.investment_manager.lei;

    // Allianz SE
    if let Some(allianz_se) = level2.entities.get(allianz_se_lei) {
        dsl_parts.push(generate_parent_entity_dsl(allianz_se));
        dsl_parts.push(String::new());
        defined_leis.insert(allianz_se_lei.clone());
    } else {
        // Fallback: Create minimal entity if not in Level 2 data
        dsl_parts.push(format!(";; {}", funds_data.ultimate_client.name));
        dsl_parts.push("(entity.ensure-limited-company".to_string());
        dsl_parts.push(format!(
            "    :name \"{}\"",
            escape_dsl_string(&funds_data.ultimate_client.name)
        ));
        dsl_parts.push(format!("    :lei \"{}\"", allianz_se_lei));
        dsl_parts.push("    :jurisdiction \"DE\"".to_string());
        dsl_parts.push(format!("    :as {})", lei_to_alias(allianz_se_lei)));
        dsl_parts.push(String::new());
        defined_leis.insert(allianz_se_lei.clone());
    }

    // AllianzGI
    if let Some(allianzgi) = level2.entities.get(allianzgi_lei) {
        dsl_parts.push(generate_parent_entity_dsl(allianzgi));
        dsl_parts.push(String::new());
        defined_leis.insert(allianzgi_lei.clone());
    } else {
        // Fallback
        dsl_parts.push(format!(";; {}", funds_data.investment_manager.name));
        dsl_parts.push("(entity.ensure-limited-company".to_string());
        dsl_parts.push(format!(
            "    :name \"{}\"",
            escape_dsl_string(&funds_data.investment_manager.name)
        ));
        dsl_parts.push(format!("    :lei \"{}\"", allianzgi_lei));
        dsl_parts.push("    :jurisdiction \"DE\"".to_string());
        dsl_parts.push(format!("    :as {})", lei_to_alias(allianzgi_lei)));
        dsl_parts.push(String::new());
        defined_leis.insert(allianzgi_lei.clone());
    }

    // Ownership relationship: Allianz SE → AllianzGI
    dsl_parts.extend(vec![
        ";; Ownership: Allianz SE → AllianzGI".to_string(),
        "(ubo.add-ownership".to_string(),
        format!("    :owner-entity-id {}", lei_to_alias(allianz_se_lei)),
        format!("    :owned-entity-id {}", lei_to_alias(allianzgi_lei)),
        "    :percentage 100.0".to_string(),
        "    :ownership-type \"DIRECT\")".to_string(),
        String::new(),
    ]);

    // Phase 2: Umbrella SICAV entities FIRST (must exist before sub-funds reference them)
    let umbrella_funds: Vec<_> = funds_data.funds.iter().filter(|f| f.is_umbrella).collect();

    dsl_parts.extend(vec![
        ";; ============================================================================"
            .to_string(),
        format!(
            ";; PHASE 2: Umbrella SICAV Entities ({} umbrellas)",
            umbrella_funds.len()
        ),
        ";; MUST be created before sub-funds reference them via SICAV role".to_string(),
        ";; ============================================================================"
            .to_string(),
        String::new(),
    ]);

    for umbrella in &umbrella_funds {
        dsl_parts.push(generate_complete_fund_entity_dsl(umbrella));
        dsl_parts.push(String::new());
        defined_leis.insert(umbrella.lei.clone());
    }

    // Phase 2.5: External umbrella entities (referenced but not in fund list)
    // Some sub-funds reference umbrella LEIs that are not in our fund list
    // We need to create placeholder entities for these so SICAV role can reference them
    let mut external_umbrellas: Vec<(&str, &str)> = Vec::new();
    for fund in &funds_data.funds {
        if let Some(ref umbrella_lei) = fund.umbrella_lei {
            if !defined_leis.contains(umbrella_lei) {
                let umbrella_name = fund.umbrella_name.as_deref().unwrap_or("Unknown Umbrella");
                external_umbrellas.push((umbrella_lei.as_str(), umbrella_name));
            }
        }
    }

    // Deduplicate external umbrellas
    external_umbrellas.sort_by_key(|(lei, _)| *lei);
    external_umbrellas.dedup_by_key(|(lei, _)| *lei);

    if !external_umbrellas.is_empty() {
        dsl_parts.extend(vec![
            ";; ============================================================================"
                .to_string(),
            format!(
                ";; PHASE 2.5: External Umbrella Entities ({} external)",
                external_umbrellas.len()
            ),
            ";; These umbrellas are referenced by sub-funds but not in our fund list".to_string(),
            ";; Creating placeholder entities so SICAV role can reference them".to_string(),
            ";; ============================================================================"
                .to_string(),
            String::new(),
        ]);

        for (lei, name) in &external_umbrellas {
            let alias = lei_to_alias(lei);
            // Truncate name if too long
            let display_name = if name.len() > 200 {
                format!("{}...", &name[..197])
            } else {
                (*name).to_string()
            };

            dsl_parts.push(format!(";; External umbrella: {}", display_name));
            dsl_parts.push("(entity.ensure-limited-company".to_string());
            dsl_parts.push(format!(
                "    :name \"{}\"",
                escape_dsl_string(&display_name)
            ));
            dsl_parts.push(format!("    :lei \"{}\"", lei));
            dsl_parts.push("    :jurisdiction \"LU\"".to_string()); // Default to LU for fund umbrellas
            dsl_parts.push("    :gleif-category \"FUND\"".to_string());
            dsl_parts.push(format!("    :as {})", alias));
            dsl_parts.push(String::new());
            defined_leis.insert((*lei).to_string());
        }
    }

    // Phase 3: All remaining fund entities (sub-funds and standalone)
    let remaining_funds: Vec<_> = funds_data.funds.iter().filter(|f| !f.is_umbrella).collect();

    let funds_to_process: Vec<_> = if let Some(limit) = fund_limit {
        remaining_funds.into_iter().take(limit).collect()
    } else {
        remaining_funds
    };

    dsl_parts.extend(vec![
        ";; ============================================================================"
            .to_string(),
        format!(
            ";; PHASE 3: Sub-Fund and Standalone Fund Entities ({})",
            funds_to_process.len()
        ),
        ";; ============================================================================"
            .to_string(),
        String::new(),
    ]);

    for fund in &funds_to_process {
        if !defined_leis.contains(&fund.lei) {
            dsl_parts.push(generate_complete_fund_entity_dsl(fund));
            dsl_parts.push(String::new());
            defined_leis.insert(fund.lei.clone());
        }
    }

    // Phase 4: CBUs with correct role assignments
    // Process ALL funds (umbrellas + remaining)
    let all_funds_for_cbus: Vec<_> = if let Some(limit) = fund_limit {
        // Include umbrellas + limited remaining
        umbrella_funds
            .iter()
            .chain(funds_to_process.iter())
            .take(limit)
            .collect()
    } else {
        umbrella_funds
            .iter()
            .chain(funds_to_process.iter())
            .collect()
    };

    dsl_parts.extend(vec![
        ";; ============================================================================"
            .to_string(),
        format!(
            ";; PHASE 4: CBUs with Role Assignments ({})",
            all_funds_for_cbus.len()
        ),
        ";; Roles: ASSET_OWNER, INVESTMENT_MANAGER, MANAGEMENT_COMPANY, SICAV*, ULTIMATE_CLIENT"
            .to_string(),
        ";; *SICAV only for sub-funds, points to umbrella entity (not fund itself!)".to_string(),
        ";; ============================================================================"
            .to_string(),
        String::new(),
    ]);

    for fund in all_funds_for_cbus {
        dsl_parts.push(generate_fund_cbu_dsl(
            fund,
            &funds_data.investment_manager,
            &funds_data.ultimate_client,
        ));
        dsl_parts.push(String::new());
    }

    dsl_parts.extend(vec![
        ";; ============================================================================"
            .to_string(),
        ";; END OF ALLIANZ GLEIF COMPLETE FUND DATA LOAD".to_string(),
        ";; ============================================================================"
            .to_string(),
    ]);

    Ok(dsl_parts.join("\n"))
}

/// Generate DSL for a corporate tree child (Allianz SE subsidiary)
fn generate_corp_child_dsl(child: &CorporateTreeChild, parent_lei: &str) -> String {
    let alias = lei_to_alias(&child.lei);
    let mut lines = vec![
        format!(";; {}", child.name),
        "(entity.ensure-limited-company".to_string(),
        format!("    :name \"{}\"", escape_dsl_string(&child.name)),
        format!("    :lei \"{}\"", child.lei),
        format!("    :jurisdiction \"{}\"", child.jurisdiction),
        format!("    :direct-parent-lei \"{}\"", parent_lei),
    ];

    if let Some(ref city) = child.city {
        lines.push(format!("    :city \"{}\"", escape_dsl_string(city)));
    }
    if let Some(ref status) = child.status {
        lines.push(format!("    :gleif-status \"{}\"", status));
    }
    if let Some(ref category) = child.category {
        lines.push(format!("    :gleif-category \"{}\"", category));
    }
    if let Some(ref lf) = child.legal_form {
        lines.push(format!("    :legal-form-code \"{}\"", lf));
    }

    lines.push(format!("    :as {})", alias));
    lines.join("\n")
}

/// Generate ownership relationship DSL
fn generate_ownership_dsl(rel: &Relationship) -> String {
    let owner_alias = lei_to_alias(&rel.parent_lei);
    let owned_alias = lei_to_alias(&rel.child_lei);

    let mut lines = vec![
        "(ubo.add-ownership".to_string(),
        format!("    :owner-entity-id {}", owner_alias),
        format!("    :owned-entity-id {}", owned_alias),
        "    :percentage 100.0".to_string(), // GLEIF consolidation implies 100%
        format!(
            "    :ownership-type \"{}\"",
            rel.relationship_type.replace("IS_", "").replace("_BY", "")
        ),
    ];

    if let Some(ref corr) = rel.corroboration {
        lines.push(format!("    :corroboration \"{}\"", corr));
    }

    lines.push(")".to_string());
    lines.join("\n")
}

/// Generate full DSL file from all sources
pub fn generate_full_dsl(
    level2_path: &Path,
    ownership_path: &Path,
    corp_tree_path: &Path,
    fund_limit: Option<usize>,
    corp_limit: Option<usize>,
) -> Result<String> {
    use std::collections::HashSet;

    // Load data files
    let level2_content = std::fs::read_to_string(level2_path)
        .with_context(|| format!("Failed to read {}", level2_path.display()))?;
    let level2: Level2Data = serde_json::from_str(&level2_content)
        .with_context(|| format!("Failed to parse {}", level2_path.display()))?;

    let ownership_content = std::fs::read_to_string(ownership_path)
        .with_context(|| format!("Failed to read {}", ownership_path.display()))?;
    let ownership: OwnershipChain = serde_json::from_str(&ownership_content)
        .with_context(|| format!("Failed to parse {}", ownership_path.display()))?;

    let corp_content = std::fs::read_to_string(corp_tree_path)
        .with_context(|| format!("Failed to read {}", corp_tree_path.display()))?;
    let corp_tree: CorporateTree = serde_json::from_str(&corp_content)
        .with_context(|| format!("Failed to parse {}", corp_tree_path.display()))?;

    // Track already-defined LEIs to avoid duplicates
    let mut defined_leis: HashSet<String> = HashSet::new();

    // Build DSL
    let mut dsl_parts = vec![
        format!(";; ============================================================================"),
        format!(";; ALLIANZ GLEIF DATA LOAD"),
        format!(";; Generated: {}", chrono::Utc::now().to_rfc3339()),
        format!(";; Source: GLEIF API (api.gleif.org)"),
        format!(";; ============================================================================"),
        format!(""),
        format!(";; ============================================================================"),
        format!(";; PHASE 1: Parent Entities (Allianz SE → AllianzGI hierarchy)"),
        format!(";; ============================================================================"),
        format!(""),
    ];

    // Phase 1: Parent entities from Level 2 data
    // Order matters - Allianz SE first, then AllianzGI
    let allianz_se_lei = "529900K9B0N5BT694847";
    let allianzgi_lei = "OJ2TIQSVQND4IZYYK658";

    if let Some(allianz_se) = level2.entities.get(allianz_se_lei) {
        dsl_parts.push(generate_parent_entity_dsl(allianz_se));
        dsl_parts.push(String::new());
        defined_leis.insert(allianz_se_lei.to_string());
    }

    if let Some(allianzgi) = level2.entities.get(allianzgi_lei) {
        dsl_parts.push(generate_parent_entity_dsl(allianzgi));
        dsl_parts.push(String::new());
        defined_leis.insert(allianzgi_lei.to_string());
    }

    // Phase 1b: Ownership relationships
    dsl_parts.extend(vec![format!(";; Ownership relationships"), format!("")]);

    for rel in &ownership.relationships {
        // Only include relationships where both entities exist
        if level2.entities.contains_key(&rel.parent_lei)
            || rel.parent_lei == allianz_se_lei
            || rel.parent_lei == allianzgi_lei
        {
            dsl_parts.push(generate_ownership_dsl(rel));
            dsl_parts.push(String::new());
        }
    }

    // Phase 2: AllianzGI Subsidiaries
    dsl_parts.extend(vec![
        format!(";; ============================================================================"),
        format!(";; PHASE 2: AllianzGI Subsidiaries"),
        format!(";; ============================================================================"),
        format!(""),
    ]);

    for sub in &ownership.subsidiaries {
        dsl_parts.push(generate_subsidiary_dsl(sub));
        dsl_parts.push(String::new());
        defined_leis.insert(sub.lei.clone());
    }

    // Phase 3: Managed Funds → CBUs
    let funds: Vec<_> = if let Some(limit) = fund_limit {
        ownership.managed_funds_sample.iter().take(limit).collect()
    } else {
        ownership.managed_funds_sample.iter().collect()
    };

    dsl_parts.extend(vec![
        format!(";; ============================================================================"),
        format!(";; PHASE 3: Managed Funds → CBUs with IM/ManCo roles"),
        format!(";; Total funds: {}", funds.len()),
        format!(";; ============================================================================"),
        format!(""),
    ]);

    let im_alias = lei_to_alias(allianzgi_lei);
    for fund in funds {
        dsl_parts.push(generate_fund_dsl(fund, &im_alias));
        dsl_parts.push(String::new());
    }

    // Phase 4: Allianz SE Direct Subsidiaries
    let children: Vec<_> = if let Some(limit) = corp_limit {
        corp_tree.direct_children.iter().take(limit).collect()
    } else {
        corp_tree.direct_children.iter().collect()
    };

    dsl_parts.extend(vec![
        format!(";; ============================================================================"),
        format!(";; PHASE 4: Allianz SE Direct Subsidiaries"),
        format!(";; Total: {}", children.len()),
        format!(";; ============================================================================"),
        format!(""),
    ]);

    let mut skipped_count = 0;
    for child in children {
        // Skip entities already defined in earlier phases
        if defined_leis.contains(&child.lei) {
            skipped_count += 1;
            continue;
        }
        dsl_parts.push(generate_corp_child_dsl(child, allianz_se_lei));
        dsl_parts.push(String::new());
    }
    if skipped_count > 0 {
        dsl_parts.insert(
            dsl_parts.len() - 3, // Before "END OF" comment
            format!(
                ";; Skipped {} entities already defined in earlier phases",
                skipped_count
            ),
        );
    }

    dsl_parts.extend(vec![
        format!(";; ============================================================================"),
        format!(";; END OF ALLIANZ GLEIF DATA LOAD"),
        format!(";; ============================================================================"),
    ]);

    Ok(dsl_parts.join("\n"))
}

/// Main entry point for the gleif-load command
pub async fn gleif_load(
    output_file: Option<std::path::PathBuf>,
    fund_limit: Option<usize>,
    _corp_limit: Option<usize>, // Kept for backward compatibility
    dry_run: bool,
    execute: bool,
    use_complete: bool, // NEW: Use complete funds file
) -> Result<()> {
    use sqlx::PgPool;

    println!("===========================================");
    println!("  Load Allianz GLEIF Data");
    println!("===========================================\n");

    // Paths to source files
    let base_path = std::path::Path::new("/Users/adamtc007/Developer/ob-poc/data/derived/gleif");
    let level2_path = base_path.join("allianz_level2_data.json");
    let complete_funds_path = base_path.join("allianzgi_funds_complete.json");

    // Check Level 2 exists (always needed for parent entities)
    if !level2_path.exists() {
        anyhow::bail!("Source file not found: {}", level2_path.display());
    }

    // Generate DSL based on mode
    let dsl = if use_complete {
        // NEW: Use complete funds file with correct CBU structure
        if !complete_funds_path.exists() {
            anyhow::bail!(
                "Complete funds file not found: {}\nRun the GLEIF data scraper first.",
                complete_funds_path.display()
            );
        }

        println!("Source files:");
        println!("  Level 2:        {}", level2_path.display());
        println!("  Complete Funds: {}", complete_funds_path.display());
        println!();

        println!("Generating DSL from complete funds data...");
        generate_complete_funds_dsl(&complete_funds_path, &level2_path, fund_limit)?
    } else {
        // OLD: Legacy mode using ownership chain sample data
        let ownership_path = base_path.join("allianzgi_ownership_chain.json");
        let corp_tree_path = base_path.join("allianz_se_corporate_tree.json");

        for path in [&ownership_path, &corp_tree_path] {
            if !path.exists() {
                anyhow::bail!("Source file not found: {}", path.display());
            }
        }

        println!("Source files (LEGACY MODE - limited data):");
        println!("  Level 2:    {}", level2_path.display());
        println!("  Ownership:  {}", ownership_path.display());
        println!("  Corporate:  {}", corp_tree_path.display());
        println!();
        println!("NOTE: Use --complete flag for full fund data with correct CBU structure");
        println!();

        println!("Generating DSL...");
        generate_full_dsl(
            &level2_path,
            &ownership_path,
            &corp_tree_path,
            fund_limit,
            _corp_limit,
        )?
    };

    // Count statements
    let stmt_count = dsl.lines().filter(|l| l.trim().starts_with('(')).count();
    println!("  Generated {} DSL statements", stmt_count);
    println!();

    // Output file
    let output = output_file.unwrap_or_else(|| {
        if use_complete {
            std::path::PathBuf::from(
                "/Users/adamtc007/Developer/ob-poc/data/derived/dsl/allianz_complete_load.dsl",
            )
        } else {
            std::path::PathBuf::from(
                "/Users/adamtc007/Developer/ob-poc/data/derived/dsl/allianz_gleif_load.dsl",
            )
        }
    });

    // Write DSL file
    std::fs::create_dir_all(output.parent().unwrap())?;
    std::fs::write(&output, &dsl)?;
    println!("Wrote DSL to: {}", output.display());

    if dry_run {
        println!("\nDRY RUN - DSL generated but not executed");
        println!("\nFirst 100 lines:");
        for line in dsl.lines().take(100) {
            println!("{}", line);
        }
        return Ok(());
    }

    if !execute {
        println!("\nTo execute, run with --execute flag");
        return Ok(());
    }

    // Execute DSL
    println!("\nExecuting DSL...");
    let db_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string());
    let _pool = PgPool::connect(&db_url).await?;

    // For now, we'll parse and validate. Full execution would require
    // importing the ob_poc crate which creates a circular dependency.
    // Instead, we output the DSL file and let dsl_cli execute it.

    println!("\nTo execute the generated DSL:");
    println!(
        "  cargo run --bin dsl_cli --features database,cli -- execute --file {}",
        output.display()
    );

    Ok(())
}
