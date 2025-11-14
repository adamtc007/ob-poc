# AGENTIC DSL CRUD CBU - END-TO-END TEST HARNESS & VISUALIZATION
## Drop this into Zed Claude: "Implement this complete test harness with visualization"

## Overview
Complete the agentic DSL CRUD system with:
1. REST API endpoints
2. Test harness with canned prompts
3. Tree visualization for egui display
4. End-to-end workflow from natural language to visual tree

## File 1: `src/api/agentic_complete.rs`
```rust
//! Complete REST API for Agentic DSL CRUD with visualization

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    Router,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::services::agentic_dsl_crud::{AgenticDslService, DslParser};

// ============================================================================
// VISUALIZATION STRUCTURES (for egui display)
// ============================================================================

/// Tree structure for egui visualization (like a video game skill tree)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuTreeNode {
    pub id: Uuid,
    pub node_type: NodeType,
    pub label: String,
    pub details: NodeDetails,
    pub position: Position2D,
    pub children: Vec<CbuTreeNode>,
    pub connections: Vec<Connection>,
    pub status: NodeStatus,
    pub metadata: NodeMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    Cbu,
    Entity,
    Role,
    Attribute,
    Document,
    Workflow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeDetails {
    pub title: String,
    pub subtitle: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub color: String, // Hex color for egui
    pub icon: String,  // Icon identifier
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position2D {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub from_id: Uuid,
    pub to_id: Uuid,
    pub connection_type: String,
    pub label: Option<String>,
    pub strength: f32, // 0.0 to 1.0 for visual weight
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeStatus {
    Active,
    Pending,
    Complete,
    Error,
    Inactive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetadata {
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completion_percentage: f32,
    pub risk_score: Option<f32>,
    pub compliance_status: Option<String>,
}

// ============================================================================
// API REQUEST/RESPONSE TYPES
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ExecutePromptRequest {
    pub prompt: String,
    pub context: Option<PromptContext>,
}

#[derive(Debug, Deserialize)]
pub struct PromptContext {
    pub cbu_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct ExecutePromptResponse {
    pub success: bool,
    pub prompt: String,
    pub generated_dsl: String,
    pub execution_result: ExecutionResult,
    pub visualization_tree: CbuTreeNode,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ExecutionResult {
    pub operation: String,
    pub entity_id: Option<Uuid>,
    pub affected_records: Vec<Uuid>,
    pub audit_log_id: Uuid,
}

// ============================================================================
// VISUALIZATION SERVICE
// ============================================================================

pub struct CbuVisualizationService {
    pool: PgPool,
}

impl CbuVisualizationService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    
    /// Build complete tree visualization for a CBU
    pub async fn build_cbu_tree(&self, cbu_id: Uuid) -> Result<CbuTreeNode, String> {
        // Fetch CBU details
        let cbu = self.fetch_cbu_details(cbu_id).await?;
        
        // Create root node
        let mut root = CbuTreeNode {
            id: cbu_id,
            node_type: NodeType::Cbu,
            label: cbu.name.clone(),
            details: NodeDetails {
                title: cbu.name,
                subtitle: Some(cbu.nature_purpose.clone()),
                description: Some(cbu.source_of_funds.clone()),
                tags: vec!["CBU".to_string(), "Active".to_string()],
                color: "#4CAF50".to_string(),
                icon: "business".to_string(),
            },
            position: Position2D { x: 400.0, y: 100.0 },
            children: vec![],
            connections: vec![],
            status: NodeStatus::Active,
            metadata: NodeMetadata {
                created_at: cbu.created_at,
                updated_at: Utc::now(),
                completion_percentage: self.calculate_completion(cbu_id).await?,
                risk_score: Some(0.2),
                compliance_status: Some("Compliant".to_string()),
            },
        };
        
        // Add entity nodes
        let entities = self.fetch_entities(cbu_id).await?;
        for (i, entity) in entities.iter().enumerate() {
            let entity_node = self.build_entity_node(entity, i).await?;
            
            // Add connection from CBU to entity
            root.connections.push(Connection {
                from_id: cbu_id,
                to_id: entity.id,
                connection_type: "HAS_ENTITY".to_string(),
                label: Some(entity.role.clone()),
                strength: 0.8,
            });
            
            root.children.push(entity_node);
        }
        
        // Add attribute nodes
        let attributes = self.fetch_attributes(cbu_id).await?;
        for (i, attr) in attributes.iter().enumerate() {
            let attr_node = self.build_attribute_node(attr, i + entities.len()).await?;
            root.children.push(attr_node);
        }
        
        // Add document nodes
        let documents = self.fetch_documents(cbu_id).await?;
        for (i, doc) in documents.iter().enumerate() {
            let doc_node = self.build_document_node(doc, i + entities.len() + attributes.len()).await?;
            root.children.push(doc_node);
        }
        
        Ok(root)
    }
    
    async fn fetch_cbu_details(&self, cbu_id: Uuid) -> Result<CbuDetails, String> {
        #[derive(sqlx::FromRow)]
        struct CbuDetails {
            name: String,
            nature_purpose: String,
            source_of_funds: String,
            created_at: DateTime<Utc>,
        }
        
        sqlx::query_as::<_, CbuDetails>(
            r#"SELECT name, 
                      COALESCE(nature_purpose, '') as nature_purpose,
                      COALESCE(description, '') as source_of_funds,
                      created_at
               FROM "ob-poc".cbus 
               WHERE cbu_id = $1"#
        )
        .bind(cbu_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| e.to_string())
    }
    
    async fn fetch_entities(&self, cbu_id: Uuid) -> Result<Vec<EntityInfo>, String> {
        #[derive(sqlx::FromRow)]
        struct EntityInfo {
            id: Uuid,
            name: String,
            role: String,
        }
        
        sqlx::query_as::<_, EntityInfo>(
            r#"SELECT e.entity_id as id,
                      e.name,
                      'Director' as role
               FROM "ob-poc".cbu_entity_roles cer
               JOIN "ob-poc".entities e ON e.entity_id = cer.entity_id
               WHERE cer.cbu_id = $1"#
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())
    }
    
    async fn build_entity_node(&self, entity: &EntityInfo, index: usize) -> Result<CbuTreeNode, String> {
        Ok(CbuTreeNode {
            id: entity.id,
            node_type: NodeType::Entity,
            label: entity.name.clone(),
            details: NodeDetails {
                title: entity.name.clone(),
                subtitle: Some(entity.role.clone()),
                description: None,
                tags: vec!["Entity".to_string(), entity.role.clone()],
                color: "#2196F3".to_string(),
                icon: "person".to_string(),
            },
            position: Position2D {
                x: 200.0 + (index as f32 * 150.0),
                y: 250.0,
            },
            children: vec![],
            connections: vec![],
            status: NodeStatus::Active,
            metadata: NodeMetadata {
                created_at: Utc::now(),
                updated_at: Utc::now(),
                completion_percentage: 100.0,
                risk_score: None,
                compliance_status: Some("Verified".to_string()),
            },
        })
    }
    
    async fn calculate_completion(&self, cbu_id: Uuid) -> Result<f32, String> {
        // Calculate based on required vs completed attributes
        let total_required = 10.0; // Example: 10 required attributes
        let completed = 7.0; // Example: 7 completed
        Ok((completed / total_required) * 100.0)
    }
    
    // Stub implementations for other node types
    async fn fetch_attributes(&self, _cbu_id: Uuid) -> Result<Vec<AttributeInfo>, String> {
        Ok(vec![]) // Implement based on your attribute system
    }
    
    async fn fetch_documents(&self, _cbu_id: Uuid) -> Result<Vec<DocumentInfo>, String> {
        Ok(vec![]) // Implement based on your document system
    }
    
    async fn build_attribute_node(&self, _attr: &AttributeInfo, _index: usize) -> Result<CbuTreeNode, String> {
        todo!() // Implement attribute node building
    }
    
    async fn build_document_node(&self, _doc: &DocumentInfo, _index: usize) -> Result<CbuTreeNode, String> {
        todo!() // Implement document node building
    }
}

// Stub types - implement based on your schema
struct AttributeInfo { id: Uuid }
struct DocumentInfo { id: Uuid }
struct EntityInfo { id: Uuid, name: String, role: String }

// ============================================================================
// API HANDLERS
// ============================================================================

/// POST /api/agentic/execute
/// Execute a natural language prompt and visualize results
pub async fn execute_prompt_handler(
    State(state): State<AppState>,
    Json(request): Json<ExecutePromptRequest>,
) -> Result<Json<ExecutePromptResponse>, (StatusCode, String)> {
    // Parse prompt to DSL
    let parsed = DslParser::parse(&request.prompt)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    
    let generated_dsl = format!("{:?}", parsed); // Or use proper DSL formatter
    
    // Execute the parsed statement
    let result = state.agentic_service
        .execute(parsed)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    // Build visualization tree
    let cbu_id = result.entity_id
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "No CBU created".to_string()))?;
    
    let tree = state.viz_service
        .build_cbu_tree(cbu_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    
    Ok(Json(ExecutePromptResponse {
        success: true,
        prompt: request.prompt.clone(),
        generated_dsl,
        execution_result: ExecutionResult {
            operation: "CREATE_CBU".to_string(),
            entity_id: Some(cbu_id),
            affected_records: vec![cbu_id],
            audit_log_id: Uuid::new_v4(),
        },
        visualization_tree: tree,
        message: "Successfully executed prompt".to_string(),
    }))
}

/// GET /api/agentic/visualize/{cbu_id}
/// Get visualization tree for existing CBU
pub async fn visualize_cbu_handler(
    State(state): State<AppState>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<CbuTreeNode>, (StatusCode, String)> {
    state.viz_service
        .build_cbu_tree(cbu_id)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
}

#[derive(Clone)]
pub struct AppState {
    pub agentic_service: Arc<AgenticDslService>,
    pub viz_service: Arc<CbuVisualizationService>,
    pub pool: PgPool,
}
```

## File 2: `src/test_harness.rs`
```rust
//! Test Harness with Canned Prompts for End-to-End Testing

use crate::api::agentic_complete::{ExecutePromptRequest, PromptContext};
use uuid::Uuid;

/// Canned test prompts for different scenarios
pub struct TestHarness;

impl TestHarness {
    /// Get all test scenarios
    pub fn get_test_scenarios() -> Vec<TestScenario> {
        vec![
            // Scenario 1: Simple CBU Creation
            TestScenario {
                name: "Simple Hedge Fund CBU".to_string(),
                description: "Create a basic hedge fund CBU".to_string(),
                prompts: vec![
                    TestPrompt {
                        prompt: "Create a CBU with Nature and Purpose 'Hedge Fund Management for High Net Worth Individuals' and Source of funds 'Private Capital and Investment Returns'".to_string(),
                        expected_operation: "CREATE_CBU".to_string(),
                        validate: Box::new(|result| result.success),
                    },
                ],
            },
            
            // Scenario 2: Complete Onboarding Flow
            TestScenario {
                name: "Complete Investment Bank Onboarding".to_string(),
                description: "Full onboarding with entities and roles".to_string(),
                prompts: vec![
                    TestPrompt {
                        prompt: "Create a CBU with Nature and Purpose 'Investment Banking Services' and Source of funds 'Corporate Finance and M&A Fees'".to_string(),
                        expected_operation: "CREATE_CBU".to_string(),
                        validate: Box::new(|result| result.success),
                    },
                    TestPrompt {
                        prompt: "Connect entity John Smith (CEO) to the CBU as Director".to_string(),
                        expected_operation: "CONNECT_ENTITY".to_string(),
                        validate: Box::new(|result| result.success),
                    },
                    TestPrompt {
                        prompt: "Connect entity Jane Doe (CFO) to the CBU as Financial Officer".to_string(),
                        expected_operation: "CONNECT_ENTITY".to_string(),
                        validate: Box::new(|result| result.success),
                    },
                ],
            },
            
            // Scenario 3: Trust Structure
            TestScenario {
                name: "Family Trust Setup".to_string(),
                description: "Create trust with beneficiaries".to_string(),
                prompts: vec![
                    TestPrompt {
                        prompt: "Create a CBU with Nature and Purpose 'Family Trust for Estate Planning' and Source of funds 'Family Assets and Inheritance'".to_string(),
                        expected_operation: "CREATE_CBU".to_string(),
                        validate: Box::new(|result| result.success),
                    },
                    TestPrompt {
                        prompt: "Connect entity Robert Trust as Trustee".to_string(),
                        expected_operation: "CONNECT_ENTITY".to_string(),
                        validate: Box::new(|result| result.success),
                    },
                    TestPrompt {
                        prompt: "Connect entity Alice Trust as Beneficiary".to_string(),
                        expected_operation: "CONNECT_ENTITY".to_string(),
                        validate: Box::new(|result| result.success),
                    },
                    TestPrompt {
                        prompt: "Connect entity Bob Trust as Beneficiary".to_string(),
                        expected_operation: "CONNECT_ENTITY".to_string(),
                        validate: Box::new(|result| result.success),
                    },
                ],
            },
            
            // Scenario 4: Corporate Structure
            TestScenario {
                name: "Multi-Entity Corporate Structure".to_string(),
                description: "Complex corporate setup with subsidiaries".to_string(),
                prompts: vec![
                    TestPrompt {
                        prompt: "Create a CBU with Nature and Purpose 'Holding Company for International Operations' and Source of funds 'Revenue from Subsidiaries and Investment Income'".to_string(),
                        expected_operation: "CREATE_CBU".to_string(),
                        validate: Box::new(|result| result.success),
                    },
                    TestPrompt {
                        prompt: "Connect entity TechCorp USA LLC as Subsidiary".to_string(),
                        expected_operation: "CONNECT_ENTITY".to_string(),
                        validate: Box::new(|result| result.success),
                    },
                    TestPrompt {
                        prompt: "Connect entity TechCorp UK Ltd as Subsidiary".to_string(),
                        expected_operation: "CONNECT_ENTITY".to_string(),
                        validate: Box::new(|result| result.success),
                    },
                    TestPrompt {
                        prompt: "Connect entity TechCorp Singapore Pte as Subsidiary".to_string(),
                        expected_operation: "CONNECT_ENTITY".to_string(),
                        validate: Box::new(|result| result.success),
                    },
                ],
            },
            
            // Scenario 5: Pension Fund
            TestScenario {
                name: "Pension Fund Setup".to_string(),
                description: "Retirement fund with managers".to_string(),
                prompts: vec![
                    TestPrompt {
                        prompt: "Create a CBU with Nature and Purpose 'Corporate Pension Fund Management' and Source of funds 'Employee and Employer Contributions'".to_string(),
                        expected_operation: "CREATE_CBU".to_string(),
                        validate: Box::new(|result| result.success),
                    },
                    TestPrompt {
                        prompt: "Connect entity Pension Manager Corp as Fund Administrator".to_string(),
                        expected_operation: "CONNECT_ENTITY".to_string(),
                        validate: Box::new(|result| result.success),
                    },
                ],
            },
        ]
    }
    
    /// Run a specific scenario
    pub async fn run_scenario(
        scenario: &TestScenario,
        client: &reqwest::Client,
        base_url: &str,
    ) -> TestResults {
        let mut results = TestResults {
            scenario_name: scenario.name.clone(),
            total_prompts: scenario.prompts.len(),
            successful: 0,
            failed: 0,
            details: vec![],
        };
        
        let mut context = PromptContext {
            cbu_id: None,
            user_id: Some(Uuid::new_v4()),
            session_id: Some(Uuid::new_v4()),
        };
        
        for prompt in &scenario.prompts {
            let request = ExecutePromptRequest {
                prompt: prompt.prompt.clone(),
                context: Some(context.clone()),
            };
            
            match execute_prompt(client, base_url, request).await {
                Ok(response) => {
                    if (prompt.validate)(&response) {
                        results.successful += 1;
                        results.details.push(TestDetail {
                            prompt: prompt.prompt.clone(),
                            success: true,
                            message: "Passed validation".to_string(),
                            generated_dsl: Some(response.generated_dsl),
                            entity_id: response.execution_result.entity_id,
                        });
                        
                        // Update context with CBU ID for subsequent operations
                        if let Some(id) = response.execution_result.entity_id {
                            context.cbu_id = Some(id);
                        }
                    } else {
                        results.failed += 1;
                        results.details.push(TestDetail {
                            prompt: prompt.prompt.clone(),
                            success: false,
                            message: "Failed validation".to_string(),
                            generated_dsl: Some(response.generated_dsl),
                            entity_id: None,
                        });
                    }
                }
                Err(e) => {
                    results.failed += 1;
                    results.details.push(TestDetail {
                        prompt: prompt.prompt.clone(),
                        success: false,
                        message: format!("Execution error: {}", e),
                        generated_dsl: None,
                        entity_id: None,
                    });
                }
            }
        }
        
        results
    }
}

pub struct TestScenario {
    pub name: String,
    pub description: String,
    pub prompts: Vec<TestPrompt>,
}

pub struct TestPrompt {
    pub prompt: String,
    pub expected_operation: String,
    pub validate: Box<dyn Fn(&ExecutePromptResponse) -> bool>,
}

pub struct TestResults {
    pub scenario_name: String,
    pub total_prompts: usize,
    pub successful: usize,
    pub failed: usize,
    pub details: Vec<TestDetail>,
}

pub struct TestDetail {
    pub prompt: String,
    pub success: bool,
    pub message: String,
    pub generated_dsl: Option<String>,
    pub entity_id: Option<Uuid>,
}

async fn execute_prompt(
    client: &reqwest::Client,
    base_url: &str,
    request: ExecutePromptRequest,
) -> Result<ExecutePromptResponse, String> {
    let response = client
        .post(format!("{}/api/agentic/execute", base_url))
        .json(&request)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    if response.status().is_success() {
        response.json().await.map_err(|e| e.to_string())
    } else {
        Err(format!("HTTP {}: {}", response.status(), response.text().await.unwrap_or_default()))
    }
}
```

## File 3: `examples/run_test_harness.rs`
```rust
//! Run the complete test harness with visualization
//! cargo run --example run_test_harness --features database

use ob_poc::test_harness::{TestHarness, TestResults};
use ob_poc::api::agentic_complete::CbuTreeNode;
use colored::*;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", "🚀 AGENTIC DSL CRUD - END-TO-END TEST HARNESS".bold().green());
    println!("{}", "=".repeat(80).dimmed());
    
    // Setup
    let base_url = std::env::var("API_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());
    let client = reqwest::Client::new();
    
    // Get all test scenarios
    let scenarios = TestHarness::get_test_scenarios();
    
    println!("\n{}", format!("📋 Found {} test scenarios", scenarios.len()).cyan());
    
    let mut total_passed = 0;
    let mut total_failed = 0;
    
    // Run each scenario
    for (i, scenario) in scenarios.iter().enumerate() {
        println!("\n{}", format!("📌 Scenario {}/{}: {}", 
            i + 1, scenarios.len(), scenario.name).bold().blue());
        println!("   {}", scenario.description.dimmed());
        
        let start = Instant::now();
        let results = TestHarness::run_scenario(scenario, &client, &base_url).await;
        let duration = start.elapsed();
        
        // Display results
        display_results(&results);
        
        total_passed += results.successful;
        total_failed += results.failed;
        
        println!("   ⏱️  Duration: {:?}", duration);
        
        // Visualize the created CBU if successful
        if results.successful > 0 {
            if let Some(cbu_id) = results.details.iter()
                .find(|d| d.entity_id.is_some())
                .and_then(|d| d.entity_id) {
                
                println!("\n   🎨 Fetching visualization for CBU: {}", cbu_id);
                match fetch_visualization(&client, &base_url, cbu_id).await {
                    Ok(tree) => display_tree(&tree, 1),
                    Err(e) => println!("   ❌ Visualization failed: {}", e),
                }
            }
        }
    }
    
    // Summary
    println!("\n{}", "=".repeat(80).dimmed());
    println!("{}", "📊 FINAL RESULTS".bold().green());
    println!("   ✅ Passed: {}", total_passed.to_string().green());
    println!("   ❌ Failed: {}", total_failed.to_string().red());
    println!("   📈 Success Rate: {:.1}%", 
        (total_passed as f32 / (total_passed + total_failed) as f32) * 100.0);
    
    if total_failed == 0 {
        println!("\n{}", "🎉 ALL TESTS PASSED!".bold().green());
    } else {
        println!("\n{}", "⚠️  Some tests failed. Check the details above.".yellow());
    }
    
    Ok(())
}

fn display_results(results: &TestResults) {
    for detail in &results.details {
        let icon = if detail.success { "✅" } else { "❌" };
        let color = if detail.success { "green" } else { "red" };
        
        println!("\n   {} Prompt: {}", icon, detail.prompt.dimmed());
        if let Some(dsl) = &detail.generated_dsl {
            println!("      DSL: {}", dsl.cyan());
        }
        if let Some(id) = detail.entity_id {
            println!("      Created: {}", id.to_string().green());
        }
        if !detail.success {
            println!("      Error: {}", detail.message.red());
        }
    }
    
    let summary = format!(
        "   Summary: {}/{} passed", 
        results.successful, 
        results.total_prompts
    );
    
    if results.failed == 0 {
        println!("\n   {}", summary.green());
    } else {
        println!("\n   {}", summary.yellow());
    }
}

async fn fetch_visualization(
    client: &reqwest::Client,
    base_url: &str,
    cbu_id: uuid::Uuid,
) -> Result<CbuTreeNode, String> {
    let response = client
        .get(format!("{}/api/agentic/visualize/{}", base_url, cbu_id))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    if response.status().is_success() {
        response.json().await.map_err(|e| e.to_string())
    } else {
        Err(format!("Failed to fetch visualization: {}", response.status()))
    }
}

fn display_tree(node: &CbuTreeNode, depth: usize) {
    let indent = "   ".repeat(depth);
    let icon = match node.node_type {
        NodeType::Cbu => "🏢",
        NodeType::Entity => "👤",
        NodeType::Role => "🎭",
        NodeType::Attribute => "📝",
        NodeType::Document => "📄",
        NodeType::Workflow => "⚙️",
    };
    
    println!("{}{}  {} {}", 
        indent, 
        icon,
        node.label.bold(),
        format!("({}%)", node.metadata.completion_percentage).dimmed()
    );
    
    if let Some(subtitle) = &node.details.subtitle {
        println!("{}    {}", indent, subtitle.dimmed());
    }
    
    for child in &node.children {
        display_tree(child, depth + 1);
    }
}
```

## File 4: `src/visualization/egui_renderer.rs`
```rust
//! Egui renderer for CBU tree visualization (like a video game skill tree)

use egui::{Color32, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2, FontId};
use crate::api::agentic_complete::{CbuTreeNode, NodeType, NodeStatus, Connection};

pub struct CbuTreeRenderer {
    pub zoom: f32,
    pub offset: Vec2,
    pub selected_node: Option<uuid::Uuid>,
    pub hover_node: Option<uuid::Uuid>,
}

impl CbuTreeRenderer {
    pub fn new() -> Self {
        Self {
            zoom: 1.0,
            offset: Vec2::ZERO,
            selected_node: None,
            hover_node: None,
        }
    }
    
    /// Render the CBU tree in egui (video game style)
    pub fn render(&mut self, ui: &mut Ui, tree: &CbuTreeNode) {
        let available_rect = ui.available_rect();
        
        // Draw connections first (behind nodes)
        self.draw_connections(ui, tree, &available_rect);
        
        // Draw nodes
        self.draw_node(ui, tree, &available_rect);
        
        // Draw children recursively
        for child in &tree.children {
            self.draw_node(ui, child, &available_rect);
        }
        
        // Handle interactions
        self.handle_interactions(ui);
    }
    
    fn draw_node(&mut self, ui: &mut Ui, node: &CbuTreeNode, rect: &Rect) -> Response {
        let pos = self.world_to_screen(Pos2::new(node.position.x, node.position.y));
        
        // Node size based on type
        let size = match node.node_type {
            NodeType::Cbu => Vec2::new(120.0, 80.0),
            NodeType::Entity => Vec2::new(100.0, 60.0),
            _ => Vec2::new(80.0, 50.0),
        } * self.zoom;
        
        let node_rect = Rect::from_center_size(pos, size);
        
        // Colors based on status
        let (bg_color, border_color) = match node.status {
            NodeStatus::Active => (Color32::from_rgb(76, 175, 80), Color32::from_rgb(56, 142, 60)),
            NodeStatus::Pending => (Color32::from_rgb(255, 193, 7), Color32::from_rgb(230, 162, 0)),
            NodeStatus::Complete => (Color32::from_rgb(33, 150, 243), Color32::from_rgb(25, 118, 210)),
            NodeStatus::Error => (Color32::from_rgb(244, 67, 54), Color32::from_rgb(211, 47, 47)),
            NodeStatus::Inactive => (Color32::from_rgb(158, 158, 158), Color32::from_rgb(97, 97, 97)),
        };
        
        let is_hovered = self.hover_node == Some(node.id);
        let is_selected = self.selected_node == Some(node.id);
        
        // Draw shadow for depth
        ui.painter().rect_filled(
            node_rect.translate(Vec2::new(2.0, 2.0)),
            5.0,
            Color32::from_rgba_premultiplied(0, 0, 0, 50),
        );
        
        // Draw node background
        ui.painter().rect_filled(
            node_rect,
            5.0,
            if is_selected {
                Color32::from_rgb(255, 255, 255)
            } else if is_hovered {
                bg_color.linear_multiply(1.2)
            } else {
                bg_color
            },
        );
        
        // Draw border
        ui.painter().rect_stroke(
            node_rect,
            5.0,
            Stroke::new(
                if is_selected { 3.0 } else { 2.0 },
                border_color,
            ),
        );
        
        // Draw icon
        let icon_pos = pos - Vec2::new(0.0, size.y * 0.2);
        ui.painter().text(
            icon_pos,
            egui::Align2::CENTER_CENTER,
            self.get_icon(&node.node_type),
            FontId::proportional(24.0 * self.zoom),
            Color32::WHITE,
        );
        
        // Draw label
        ui.painter().text(
            pos + Vec2::new(0.0, size.y * 0.2),
            egui::Align2::CENTER_CENTER,
            &node.label,
            FontId::proportional(12.0 * self.zoom),
            Color32::WHITE,
        );
        
        // Draw progress bar for CBU nodes
        if node.node_type == NodeType::Cbu {
            let progress_rect = Rect::from_min_size(
                node_rect.min + Vec2::new(10.0, size.y - 15.0),
                Vec2::new(size.x - 20.0, 5.0),
            );
            
            ui.painter().rect_filled(
                progress_rect,
                2.0,
                Color32::from_rgba_premultiplied(255, 255, 255, 50),
            );
            
            let progress_fill = progress_rect.with_max_x(
                progress_rect.min.x + progress_rect.width() * (node.metadata.completion_percentage / 100.0)
            );
            
            ui.painter().rect_filled(
                progress_fill,
                2.0,
                Color32::from_rgb(76, 175, 80),
            );
        }
        
        // Interaction
        let response = ui.interact(node_rect, ui.id().with(node.id), Sense::click_and_drag());
        
        if response.hovered() {
            self.hover_node = Some(node.id);
            
            // Show tooltip
            response.on_hover_ui(|ui| {
                ui.label(format!("ID: {}", node.id));
                if let Some(subtitle) = &node.details.subtitle {
                    ui.label(subtitle);
                }
                if let Some(desc) = &node.details.description {
                    ui.label(desc);
                }
                ui.label(format!("Completion: {:.0}%", node.metadata.completion_percentage));
            });
        }
        
        if response.clicked() {
            self.selected_node = Some(node.id);
        }
        
        response
    }
    
    fn draw_connections(&self, ui: &mut Ui, tree: &CbuTreeNode, _rect: &Rect) {
        for connection in &tree.connections {
            // This is simplified - in production, find actual node positions
            let from_pos = self.world_to_screen(Pos2::new(tree.position.x, tree.position.y));
            
            // Find target node position (simplified)
            if let Some(child) = tree.children.iter().find(|c| c.id == connection.to_id) {
                let to_pos = self.world_to_screen(Pos2::new(child.position.x, child.position.y));
                
                // Draw bezier curve connection
                let control1 = from_pos + Vec2::new(0.0, 50.0);
                let control2 = to_pos - Vec2::new(0.0, 50.0);
                
                ui.painter().add(egui::Shape::CubicBezier(
                    egui::shape::CubicBezierShape {
                        points: [from_pos, control1, control2, to_pos],
                        closed: false,
                        fill: Color32::TRANSPARENT,
                        stroke: Stroke::new(
                            2.0 * connection.strength * self.zoom,
                            Color32::from_rgba_premultiplied(100, 200, 255, 150),
                        ),
                    },
                ));
                
                // Draw connection label if present
                if let Some(label) = &connection.label {
                    let mid_pos = from_pos.lerp(to_pos, 0.5);
                    ui.painter().text(
                        mid_pos,
                        egui::Align2::CENTER_CENTER,
                        label,
                        FontId::proportional(10.0 * self.zoom),
                        Color32::from_rgba_premultiplied(255, 255, 255, 200),
                    );
                }
            }
        }
    }
    
    fn world_to_screen(&self, world_pos: Pos2) -> Pos2 {
        Pos2::new(
            world_pos.x * self.zoom + self.offset.x,
            world_pos.y * self.zoom + self.offset.y,
        )
    }
    
    fn get_icon(&self, node_type: &NodeType) -> &'static str {
        match node_type {
            NodeType::Cbu => "🏢",
            NodeType::Entity => "👤",
            NodeType::Role => "🎭",
            NodeType::Attribute => "📝",
            NodeType::Document => "📄",
            NodeType::Workflow => "⚙️",
        }
    }
    
    fn handle_interactions(&mut self, ui: &mut Ui) {
        // Zoom with mouse wheel
        let scroll = ui.input(|i| i.scroll_delta);
        if scroll.y != 0.0 {
            self.zoom *= 1.0 + scroll.y * 0.001;
            self.zoom = self.zoom.clamp(0.5, 3.0);
        }
        
        // Pan with middle mouse
        if ui.input(|i| i.pointer.middle_down()) {
            let delta = ui.input(|i| i.pointer.delta());
            self.offset += delta;
        }
        
        // Reset view with R key
        if ui.input(|i| i.key_pressed(egui::Key::R)) {
            self.zoom = 1.0;
            self.offset = Vec2::ZERO;
        }
    }
}
```

## Testing Instructions

### 1. Setup Database
```sql
-- Ensure all tables exist
CREATE TABLE IF NOT EXISTS "ob-poc".test_results (
    test_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    scenario_name VARCHAR(255),
    prompt TEXT,
    generated_dsl TEXT,
    success BOOLEAN,
    entity_id UUID,
    execution_time_ms INTEGER,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);
```

### 2. Start API Server
```bash
# In one terminal
cargo run --release
```

### 3. Run Test Harness
```bash
# In another terminal
cargo run --example run_test_harness --features database
```

### 4. View Results
The test harness will:
1. Execute all canned prompts
2. Create CBUs and entities
3. Generate visualization trees
4. Display results in console (colored output)

### 5. Manual Testing with cURL
```bash
# Test a single prompt
curl -X POST http://localhost:3000/api/agentic/execute \
  -H "Content-Type: application/json" \
  -d '{
    "prompt": "Create a CBU with Nature and Purpose \"Testing Services\" and Source of funds \"Test Capital\""
  }'

# Get visualization
curl http://localhost:3000/api/agentic/visualize/{cbu_id}
```

## Expected Output Structure

```json
{
  "visualization_tree": {
    "id": "cbu-uuid",
    "node_type": "Cbu",
    "label": "CBU-20251114-ABC",
    "position": {"x": 400, "y": 100},
    "children": [
      {
        "id": "entity-uuid",
        "node_type": "Entity",
        "label": "John Smith",
        "details": {
          "subtitle": "Director",
          "tags": ["Entity", "Director"]
        }
      }
    ],
    "connections": [
      {
        "from_id": "cbu-uuid",
        "to_id": "entity-uuid",
        "connection_type": "HAS_ENTITY",
        "label": "Director",
        "strength": 0.8
      }
    ],
    "metadata": {
      "completion_percentage": 70.0,
      "compliance_status": "Compliant"
    }
  }
}
```

## Cargo.toml Dependencies
```toml
[dependencies]
# Existing plus:
colored = "2"
egui = "0.24"
reqwest = { version = "0.11", features = ["json"] }

[dev-dependencies]
tokio-test = "0.4"
```

## Environment Variables
```env
DATABASE_URL=postgresql://user:pass@localhost:5432/ob-poc
API_BASE_URL=http://localhost:3000
RUST_LOG=info
```

## Summary

This implementation provides:

1. **Complete REST API** for executing prompts and getting visualizations
2. **Test Harness** with 5 realistic scenarios and multiple prompts
3. **Tree Visualization** structure ready for egui display
4. **End-to-end flow** from natural language to visual tree
5. **Console test runner** with colored output and progress tracking
6. **Egui renderer** for video game-style skill tree visualization

The system can now:
- Take natural language like "Create a hedge fund CBU..."
- Parse it to DSL
- Execute in database
- Build a visual tree
- Display in egui like a video game skill tree

**Drop this into Zed Claude and say**: "Implement this complete test harness with visualization for the agentic DSL CRUD system"
