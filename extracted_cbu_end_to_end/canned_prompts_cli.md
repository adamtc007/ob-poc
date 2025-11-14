# SIMPLE CLI TEST RUNNER - Canned Prompts with ASCII Visualization
## Drop into Zed Claude: "Create this simple CLI test runner for immediate testing"

## Single File: `examples/canned_prompts_cli.rs`
```rust
//! Simple CLI Test Runner with Canned Prompts and ASCII Tree Visualization
//! Run with: cargo run --example canned_prompts_cli --features database
//! 
//! This provides immediate testing without REST API or complex setup

use sqlx::{PgPool, FromRow};
use uuid::Uuid;
use std::collections::HashMap;
use colored::*;
use anyhow::Result;

// ============================================================================
// CANNED PROMPTS DATABASE
// ============================================================================

struct CannedPrompts;

impl CannedPrompts {
    fn get_all_prompts() -> Vec<PromptSet> {
        vec![
            PromptSet {
                name: "Quick Test".to_string(),
                color: "green",
                prompts: vec![
                    "Create a CBU with Nature and Purpose 'Quick Test Services' and Source of funds 'Test Capital'",
                ],
            },
            
            PromptSet {
                name: "Hedge Fund Complete".to_string(),
                color: "blue",
                prompts: vec![
                    "Create a CBU with Nature and Purpose 'Hedge Fund Management for High Net Worth Individuals' and Source of funds 'Private Equity and Investment Returns'",
                    "Create entity John Smith as PERSON",
                    "Create entity Jane Doe as PERSON",
                    "Create entity Alpha Capital LLC as COMPANY",
                    "Connect entity John Smith to CBU {cbu_id} as Director",
                    "Connect entity Jane Doe to CBU {cbu_id} as Compliance Officer",
                    "Connect entity Alpha Capital LLC to CBU {cbu_id} as Fund Manager",
                ],
            },
            
            PromptSet {
                name: "Family Trust".to_string(),
                color: "cyan",
                prompts: vec![
                    "Create a CBU with Nature and Purpose 'Family Trust for Wealth Preservation' and Source of funds 'Family Assets and Inheritance'",
                    "Create entity Robert Johnson as PERSON",
                    "Create entity Mary Johnson as PERSON",
                    "Create entity Trust Services Inc as COMPANY",
                    "Connect entity Robert Johnson to CBU {cbu_id} as Trustee",
                    "Connect entity Mary Johnson to CBU {cbu_id} as Beneficiary",
                    "Connect entity Trust Services Inc to CBU {cbu_id} as Administrator",
                ],
            },
            
            PromptSet {
                name: "Investment Bank".to_string(),
                color: "yellow",
                prompts: vec![
                    "Create a CBU with Nature and Purpose 'Investment Banking and Advisory Services' and Source of funds 'M&A Fees and Trading Revenue'",
                    "Create entity Michael Bloomberg as PERSON",
                    "Create entity Global Markets Corp as COMPANY",
                    "Connect entity Michael Bloomberg to CBU {cbu_id} as CEO",
                    "Connect entity Global Markets Corp to CBU {cbu_id} as Parent Company",
                ],
            },
            
            PromptSet {
                name: "Pension Fund".to_string(),
                color: "magenta",
                prompts: vec![
                    "Create a CBU with Nature and Purpose 'Corporate Pension Fund Management' and Source of funds 'Employee and Employer Contributions'",
                    "Create entity Pension Manager as PERSON",
                    "Create entity Actuarial Services Ltd as COMPANY",
                    "Connect entity Pension Manager to CBU {cbu_id} as Fund Administrator",
                    "Connect entity Actuarial Services Ltd to CBU {cbu_id} as Advisor",
                ],
            },
            
            PromptSet {
                name: "Real Estate Trust".to_string(),
                color: "red",
                prompts: vec![
                    "Create a CBU with Nature and Purpose 'Real Estate Investment Trust' and Source of funds 'Rental Income and Property Sales'",
                    "Create entity Property Manager as PERSON",
                    "Create entity REIT Management Co as COMPANY",
                    "Connect entity Property Manager to CBU {cbu_id} as Manager",
                    "Connect entity REIT Management Co to CBU {cbu_id} as Management Company",
                ],
            },
        ]
    }
}

struct PromptSet {
    name: String,
    color: &'static str,
    prompts: Vec<&'static str>,
}

// ============================================================================
// SIMPLE DSL PARSER & EXECUTOR
// ============================================================================

#[derive(Debug)]
enum SimpleOperation {
    CreateCbu { nature: String, source: String },
    CreateEntity { name: String, entity_type: String },
    ConnectEntity { entity_name: String, cbu_id: Uuid, role: String },
}

struct SimpleExecutor {
    pool: PgPool,
    current_cbu_id: Option<Uuid>,
    entity_map: HashMap<String, Uuid>,
}

impl SimpleExecutor {
    fn new(pool: PgPool) -> Self {
        Self {
            pool,
            current_cbu_id: None,
            entity_map: HashMap::new(),
        }
    }
    
    fn parse(&self, prompt: &str) -> Result<SimpleOperation> {
        let lower = prompt.to_lowercase();
        
        if lower.contains("create a cbu") || lower.contains("create cbu") {
            // Extract nature and source
            let nature = self.extract_between(prompt, "Nature and Purpose", "and Source")
                .unwrap_or("General Services".to_string());
            let source = self.extract_after(prompt, "Source of funds")
                .unwrap_or("Corporate Operations".to_string());
            
            Ok(SimpleOperation::CreateCbu {
                nature: nature.trim().trim_matches('\'').trim_matches('"').to_string(),
                source: source.trim().trim_matches('\'').trim_matches('"').to_string(),
            })
        } else if lower.contains("create entity") {
            // Extract name and type
            let parts: Vec<&str> = prompt.split(" as ").collect();
            let name = parts[0].replace("Create entity", "").trim().to_string();
            let entity_type = parts.get(1).unwrap_or(&"ENTITY").to_string();
            
            Ok(SimpleOperation::CreateEntity { name, entity_type })
        } else if lower.contains("connect entity") {
            // Extract entity name, CBU ID, and role
            let entity_name = self.extract_between(prompt, "entity", "to")
                .unwrap_or("Unknown".to_string()).trim().to_string();
            
            let role = self.extract_after(prompt, " as ")
                .unwrap_or("Member".to_string()).trim().to_string();
            
            // Use current CBU ID or extract from prompt
            let cbu_id = self.current_cbu_id
                .ok_or_else(|| anyhow::anyhow!("No CBU ID available"))?;
            
            Ok(SimpleOperation::ConnectEntity { entity_name, cbu_id, role })
        } else {
            Err(anyhow::anyhow!("Unknown operation: {}", prompt))
        }
    }
    
    async fn execute(&mut self, prompt: &str) -> Result<String> {
        // Replace {cbu_id} placeholder if present
        let prompt = if let Some(cbu_id) = self.current_cbu_id {
            prompt.replace("{cbu_id}", &cbu_id.to_string())
        } else {
            prompt.to_string()
        };
        
        let op = self.parse(&prompt)?;
        
        match op {
            SimpleOperation::CreateCbu { nature, source } => {
                let cbu_id = Uuid::new_v4();
                let name = format!("CBU-{}", &cbu_id.to_string()[..8]);
                
                sqlx::query!(
                    r#"
                    INSERT INTO "ob-poc".cbus (cbu_id, name, nature_purpose, description)
                    VALUES ($1, $2, $3, $4)
                    "#,
                    cbu_id,
                    name,
                    nature,
                    source
                )
                .execute(&self.pool)
                .await?;
                
                self.current_cbu_id = Some(cbu_id);
                Ok(format!("Created CBU: {} ({})", name, cbu_id))
            }
            
            SimpleOperation::CreateEntity { name, entity_type } => {
                let entity_id = Uuid::new_v4();
                
                sqlx::query!(
                    r#"
                    INSERT INTO "ob-poc".entities (entity_id, name, entity_type)
                    VALUES ($1, $2, $3)
                    ON CONFLICT (entity_id) DO NOTHING
                    "#,
                    entity_id,
                    name,
                    entity_type
                )
                .execute(&self.pool)
                .await?;
                
                self.entity_map.insert(name.clone(), entity_id);
                Ok(format!("Created {}: {} ({})", entity_type, name, entity_id))
            }
            
            SimpleOperation::ConnectEntity { entity_name, cbu_id, role } => {
                // Get entity ID from map or create new
                let entity_id = if let Some(id) = self.entity_map.get(&entity_name) {
                    *id
                } else {
                    // Auto-create entity if not exists
                    let id = Uuid::new_v4();
                    sqlx::query!(
                        r#"
                        INSERT INTO "ob-poc".entities (entity_id, name, entity_type)
                        VALUES ($1, $2, 'ENTITY')
                        ON CONFLICT (entity_id) DO NOTHING
                        "#,
                        id,
                        entity_name
                    )
                    .execute(&self.pool)
                    .await?;
                    self.entity_map.insert(entity_name.clone(), id);
                    id
                };
                
                let role_id = Uuid::new_v5(&Uuid::NAMESPACE_DNS, role.as_bytes());
                let connection_id = Uuid::new_v4();
                
                sqlx::query!(
                    r#"
                    INSERT INTO "ob-poc".cbu_entity_roles (cbu_entity_role_id, cbu_id, entity_id, role_id)
                    VALUES ($1, $2, $3, $4)
                    "#,
                    connection_id,
                    cbu_id,
                    entity_id,
                    role_id
                )
                .execute(&self.pool)
                .await?;
                
                Ok(format!("Connected {} to CBU as {}", entity_name, role))
            }
        }
    }
    
    fn extract_between(&self, text: &str, start: &str, end: &str) -> Option<String> {
        let start_pos = text.find(start)?;
        let after_start = &text[start_pos + start.len()..];
        let end_pos = after_start.find(end)?;
        Some(after_start[..end_pos].trim().to_string())
    }
    
    fn extract_after(&self, text: &str, marker: &str) -> Option<String> {
        let pos = text.find(marker)?;
        Some(text[pos + marker.len()..].trim().to_string())
    }
}

// ============================================================================
// ASCII TREE VISUALIZATION
// ============================================================================

#[derive(FromRow)]
struct CbuInfo {
    cbu_id: Uuid,
    name: String,
    nature_purpose: Option<String>,
}

#[derive(FromRow)]
struct EntityInfo {
    entity_id: Uuid,
    name: String,
    entity_type: String,
}

async fn visualize_cbu_ascii(pool: &PgPool, cbu_id: Uuid) -> Result<()> {
    // Fetch CBU
    let cbu = sqlx::query_as::<_, CbuInfo>(
        r#"SELECT cbu_id, name, nature_purpose FROM "ob-poc".cbus WHERE cbu_id = $1"#
    )
    .bind(cbu_id)
    .fetch_one(pool)
    .await?;
    
    // Fetch connected entities
    let entities = sqlx::query_as::<_, EntityInfo>(
        r#"
        SELECT e.entity_id, e.name, e.entity_type
        FROM "ob-poc".cbu_entity_roles cer
        JOIN "ob-poc".entities e ON e.entity_id = cer.entity_id
        WHERE cer.cbu_id = $1
        "#
    )
    .bind(cbu_id)
    .fetch_all(pool)
    .await?;
    
    // Draw ASCII tree
    println!("\n{}", "📊 CBU STRUCTURE:".bold().cyan());
    println!("{}",   "═".repeat(60).cyan());
    
    // Root CBU node
    println!("🏢 {}", cbu.name.bold().green());
    if let Some(purpose) = cbu.nature_purpose {
        println!("   {}", format!("Purpose: {}", purpose).dimmed());
    }
    println!("   {}", format!("ID: {}", cbu.cbu_id).dimmed());
    
    // Entity branches
    if !entities.is_empty() {
        println!("   │");
        for (i, entity) in entities.iter().enumerate() {
            let is_last = i == entities.len() - 1;
            let branch = if is_last { "└──" } else { "├──" };
            let icon = match entity.entity_type.as_str() {
                "PERSON" => "👤",
                "COMPANY" => "🏢",
                "TRUST" => "🏛️",
                _ => "📄",
            };
            
            println!("   {} {} {} ({})", 
                branch, 
                icon, 
                entity.name.yellow(),
                entity.entity_type.dimmed()
            );
        }
    }
    
    println!("{}", "═".repeat(60).cyan());
    
    Ok(())
}

// ============================================================================
// MAIN CLI RUNNER
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    println!("{}", "🚀 AGENTIC DSL CRUD - CANNED PROMPTS CLI TEST".bold().green());
    println!("{}", "=".repeat(80).dimmed());
    
    // Connect to database
    println!("\n📦 Connecting to database...");
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost:5432/ob-poc".to_string());
    let pool = PgPool::connect(&database_url).await?;
    println!("✅ Connected to database");
    
    // Get prompt sets
    let prompt_sets = CannedPrompts::get_all_prompts();
    
    // Interactive menu
    loop {
        println!("\n{}", "📋 AVAILABLE TEST SCENARIOS:".bold().blue());
        println!("{}", "-".repeat(60).dimmed());
        
        for (i, set) in prompt_sets.iter().enumerate() {
            let color = match set.color {
                "green" => set.name.green(),
                "blue" => set.name.blue(),
                "cyan" => set.name.cyan(),
                "yellow" => set.name.yellow(),
                "magenta" => set.name.magenta(),
                "red" => set.name.red(),
                _ => set.name.normal(),
            };
            println!("  {}. {}", i + 1, color.bold());
        }
        println!("  {}. Run All Scenarios", prompt_sets.len() + 1);
        println!("  {}. Exit", prompt_sets.len() + 2);
        
        // Get user choice
        print!("\n{}", "Select scenario (number): ".cyan());
        use std::io::{self, Write};
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        let choice: usize = match input.trim().parse() {
            Ok(n) => n,
            Err(_) => continue,
        };
        
        if choice == prompt_sets.len() + 2 {
            println!("\n{}", "👋 Goodbye!".green());
            break;
        }
        
        // Run selected scenarios
        let scenarios_to_run = if choice == prompt_sets.len() + 1 {
            // Run all
            prompt_sets.iter().collect()
        } else if choice > 0 && choice <= prompt_sets.len() {
            // Run selected
            vec![&prompt_sets[choice - 1]]
        } else {
            println!("{}", "❌ Invalid choice".red());
            continue;
        };
        
        for scenario in scenarios_to_run {
            println!("\n{}", format!("▶️  RUNNING: {}", scenario.name).bold().blue());
            println!("{}", "─".repeat(60).dimmed());
            
            let mut executor = SimpleExecutor::new(pool.clone());
            
            for prompt in &scenario.prompts {
                println!("\n📝 {}", prompt.dimmed());
                
                match executor.execute(prompt).await {
                    Ok(result) => {
                        println!("   ✅ {}", result.green());
                    }
                    Err(e) => {
                        println!("   ❌ Error: {}", e.to_string().red());
                    }
                }
                
                // Small delay for visual effect
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
            
            // Visualize the created structure
            if let Some(cbu_id) = executor.current_cbu_id {
                if let Err(e) = visualize_cbu_ascii(&pool, cbu_id).await {
                    println!("   ⚠️  Visualization error: {}", e.to_string().yellow());
                }
            }
        }
        
        println!("\n{}", "Press Enter to continue...".dimmed());
        let mut _input = String::new();
        io::stdin().read_line(&mut _input)?;
    }
    
    Ok(())
}
```

## Usage Instructions

### 1. Quick Setup
```bash
# Ensure entities table exists
psql -d ob_poc -c "
CREATE TABLE IF NOT EXISTS \"ob-poc\".entities (
    entity_id UUID PRIMARY KEY,
    name VARCHAR(255),
    entity_type VARCHAR(50)
);"

# Run the CLI
cargo run --example canned_prompts_cli --features database
```

### 2. Interactive Menu
```
🚀 AGENTIC DSL CRUD - CANNED PROMPTS CLI TEST
================================================================================

📋 AVAILABLE TEST SCENARIOS:
------------------------------------------------------------
  1. Quick Test
  2. Hedge Fund Complete
  3. Family Trust
  4. Investment Bank
  5. Pension Fund
  6. Real Estate Trust
  7. Run All Scenarios
  8. Exit

Select scenario (number): 2
```

### 3. Example Output
```
▶️  RUNNING: Hedge Fund Complete
────────────────────────────────────────────────────────

📝 Create a CBU with Nature and Purpose 'Hedge Fund Management...'
   ✅ Created CBU: CBU-a1b2c3d4 (a1b2c3d4-...)

📝 Create entity John Smith as PERSON
   ✅ Created PERSON: John Smith (e1f2g3h4-...)

📝 Connect entity John Smith to CBU {cbu_id} as Director
   ✅ Connected John Smith to CBU as Director

📊 CBU STRUCTURE:
════════════════════════════════════════════════════════════
🏢 CBU-a1b2c3d4
   Purpose: Hedge Fund Management for High Net Worth Individuals
   ID: a1b2c3d4-...
   │
   ├── 👤 John Smith (PERSON)
   ├── 👤 Jane Doe (PERSON)
   └── 🏢 Alpha Capital LLC (COMPANY)
════════════════════════════════════════════════════════════
```

## Features

1. **Interactive Menu** - Choose which scenario to run
2. **Canned Prompts** - 6 complete scenarios ready to test
3. **ASCII Visualization** - See the CBU structure immediately
4. **Colored Output** - Easy to read results
5. **Error Handling** - Graceful failure reporting
6. **No Dependencies** - Works with just the database

## Cargo.toml Dependencies
```toml
[dependencies]
sqlx = { version = "0.7", features = ["postgres", "runtime-tokio-native-tls", "uuid"] }
tokio = { version = "1", features = ["full"] }
uuid = { version = "1", features = ["v4", "v5", "serde"] }
anyhow = "1"
colored = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

## Summary

This simple CLI test runner provides:
- ✅ **Immediate testing** without REST API
- ✅ **6 realistic scenarios** covering different business cases
- ✅ **Interactive menu** for easy selection
- ✅ **ASCII tree visualization** showing CBU structure
- ✅ **Colored output** for better readability
- ✅ **Complete end-to-end flow** from prompt to visualization

Just run `cargo run --example canned_prompts_cli --features database` and start testing immediately!

**Drop into Zed Claude and say**: "Create this simple CLI test runner for immediate testing"
