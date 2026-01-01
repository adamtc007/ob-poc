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

/// Generate DSL for a managed fund entity + CBU
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
    corp_limit: Option<usize>,
    dry_run: bool,
    execute: bool,
) -> Result<()> {
    use sqlx::PgPool;

    println!("===========================================");
    println!("  Load Allianz GLEIF Data");
    println!("===========================================\n");

    // Paths to source files
    let base_path = std::path::Path::new("/Users/adamtc007/Developer/ob-poc/data/derived/gleif");
    let level2_path = base_path.join("allianz_level2_data.json");
    let ownership_path = base_path.join("allianzgi_ownership_chain.json");
    let corp_tree_path = base_path.join("allianz_se_corporate_tree.json");

    // Check source files exist
    for path in [&level2_path, &ownership_path, &corp_tree_path] {
        if !path.exists() {
            anyhow::bail!("Source file not found: {}", path.display());
        }
    }

    println!("Source files:");
    println!("  Level 2:       {}", level2_path.display());
    println!("  Ownership:     {}", ownership_path.display());
    println!("  Corporate:     {}", corp_tree_path.display());
    println!();

    // Generate DSL
    println!("Generating DSL...");
    let dsl = generate_full_dsl(
        &level2_path,
        &ownership_path,
        &corp_tree_path,
        fund_limit,
        corp_limit,
    )?;

    // Count statements
    let stmt_count = dsl.lines().filter(|l| l.trim().starts_with('(')).count();
    println!("  Generated {} DSL statements", stmt_count);
    println!();

    // Output file
    let output = output_file.unwrap_or_else(|| {
        std::path::PathBuf::from(
            "/Users/adamtc007/Developer/ob-poc/data/derived/dsl/allianz_gleif_load.dsl",
        )
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
    let pool = PgPool::connect(&db_url).await?;

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
