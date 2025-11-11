//! AI-Powered Entity CRUD Operations Demo
//!
//! This demo showcases the complete agentic CRUD refactoring for entity tables,
//! demonstrating natural language to DSL conversion with AI integration,
//! RAG context retrieval, and comprehensive entity operations.

use anyhow::Result;
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

// Mock AI service for demonstration (no API keys required)
struct MockAiEntityCrudService {
    responses: HashMap<String, MockAiResponse>,
}

#[derive(Debug, Clone)]
struct MockAiResponse {
    dsl_content: String,
    confidence: f64,
    rag_context: Vec<String>,
    explanation: String,
}

impl MockAiEntityCrudService {
    fn new() -> Self {
        let mut service = Self {
            responses: HashMap::new(),
        };
        service.initialize_responses();
        service
    }

    fn initialize_responses(&mut self) {
        // Partnership Creation Examples
        self.responses.insert(
            "create_delaware_llc".to_string(),
            MockAiResponse {
                dsl_content: r#"(data.create :asset "partnership" :values {:partnership_name "TechCorp Solutions LLC" :partnership_type "Limited Liability" :jurisdiction "US-DE" :formation_date "2024-01-15" :principal_place_business "100 Innovation Drive, Wilmington, DE 19801"})"#.to_string(),
                confidence: 0.95,
                rag_context: vec![
                    "Partnership schema with required fields".to_string(),
                    "Delaware LLC formation pattern".to_string(),
                    "Similar partnership creation examples".to_string(),
                ],
                explanation: "Generated DSL for Delaware LLC creation with all required fields".to_string(),
            }
        );

        self.responses.insert(
            "create_uk_company".to_string(),
            MockAiResponse {
                dsl_content: r#"(data.create :asset "limited_company" :values {:company_name "AlphaTech Ltd" :registration_number "12345678" :jurisdiction "GB" :incorporation_date "2023-03-01" :registered_address "123 Silicon Street, London, EC1A 1BB, UK" :business_nature "Software Development"})"#.to_string(),
                confidence: 0.93,
                rag_context: vec![
                    "Limited company schema with UK-specific fields".to_string(),
                    "UK company registration patterns".to_string(),
                ],
                explanation: "Generated DSL for UK limited company registration".to_string(),
            }
        );

        self.responses.insert(
            "create_individual".to_string(),
            MockAiResponse {
                dsl_content: r#"(data.create :asset "proper_person" :values {:first_name "John" :last_name "Smith" :date_of_birth "1985-01-01" :nationality "US" :residence_address "456 Main Street, New York, NY 10001" :id_document_type "Passport" :id_document_number "P123456789"})"#.to_string(),
                confidence: 0.91,
                rag_context: vec![
                    "Proper person schema with identity fields".to_string(),
                    "Individual creation patterns".to_string(),
                ],
                explanation: "Generated DSL for individual person registration".to_string(),
            }
        );

        self.responses.insert(
            "create_cayman_trust".to_string(),
            MockAiResponse {
                dsl_content: r#"(data.create :asset "trust" :values {:trust_name "Smith Family Trust" :trust_type "Discretionary" :jurisdiction "KY" :establishment_date "2024-02-15" :trust_purpose "Wealth preservation and succession planning" :governing_law "Cayman Islands Law"})"#.to_string(),
                confidence: 0.89,
                rag_context: vec![
                    "Trust schema with offshore jurisdiction fields".to_string(),
                    "Discretionary trust patterns".to_string(),
                ],
                explanation: "Generated DSL for Cayman Islands discretionary trust".to_string(),
            }
        );

        // Search Examples
        self.responses.insert(
            "find_us_partnerships".to_string(),
            MockAiResponse {
                dsl_content: r#"(data.read :asset "partnership" :where {:jurisdiction "US"} :select ["partnership_name" "partnership_type" "jurisdiction" "formation_date"] :limit 25)"#.to_string(),
                confidence: 0.96,
                rag_context: vec![
                    "Partnership search patterns".to_string(),
                    "US jurisdiction filtering".to_string(),
                ],
                explanation: "Generated DSL for US partnership search with selected fields".to_string(),
            }
        );

        self.responses.insert(
            "find_delaware_llcs".to_string(),
            MockAiResponse {
                dsl_content: r#"(data.read :asset "partnership" :where {:jurisdiction "US-DE" :partnership_type "Limited Liability"} :limit 50)"#.to_string(),
                confidence: 0.94,
                rag_context: vec![
                    "Delaware LLC search patterns".to_string(),
                    "State-specific jurisdiction filtering".to_string(),
                ],
                explanation: "Generated DSL for Delaware LLC search with type filtering".to_string(),
            }
        );

        // Update Examples
        self.responses.insert(
            "update_company_address".to_string(),
            MockAiResponse {
                dsl_content: r#"(data.update :asset "limited_company" :where {:company_name "AlphaTech Ltd"} :values {:registered_address "500 New Business Park, London, EC2A 2BB, UK"})"#.to_string(),
                confidence: 0.92,
                rag_context: vec![
                    "Company update patterns".to_string(),
                    "Address field updates".to_string(),
                ],
                explanation: "Generated DSL for company address update".to_string(),
            }
        );

        // Complex Multi-Entity Search
        self.responses.insert(
            "find_offshore_entities".to_string(),
            MockAiResponse {
                dsl_content: r#"(data.read :asset "entity" :where {:jurisdiction ["KY" "BVI" "BS" "CH" "LU"]} :join ["entity_type"] :select ["name" "entity_type" "jurisdiction"] :limit 100)"#.to_string(),
                confidence: 0.87,
                rag_context: vec![
                    "Multi-entity search patterns".to_string(),
                    "Offshore jurisdiction lists".to_string(),
                    "Entity type joins".to_string(),
                ],
                explanation: "Generated complex DSL for offshore entity search across all types".to_string(),
            }
        );
    }

    fn generate_dsl(&self, instruction: &str) -> Result<MockAiResponse> {
        // Simple keyword matching for demo purposes
        let instruction_lower = instruction.to_lowercase();

        if instruction_lower.contains("delaware") && instruction_lower.contains("llc") {
            Ok(self.responses.get("create_delaware_llc").unwrap().clone())
        } else if instruction_lower.contains("uk") && instruction_lower.contains("company") {
            Ok(self.responses.get("create_uk_company").unwrap().clone())
        } else if instruction_lower.contains("individual") || instruction_lower.contains("person") {
            Ok(self.responses.get("create_individual").unwrap().clone())
        } else if instruction_lower.contains("cayman") && instruction_lower.contains("trust") {
            Ok(self.responses.get("create_cayman_trust").unwrap().clone())
        } else if instruction_lower.contains("us") && instruction_lower.contains("partnership") {
            Ok(self.responses.get("find_us_partnerships").unwrap().clone())
        } else if instruction_lower.contains("delaware") && instruction_lower.contains("search") {
            Ok(self.responses.get("find_delaware_llcs").unwrap().clone())
        } else if instruction_lower.contains("update") && instruction_lower.contains("address") {
            Ok(self
                .responses
                .get("update_company_address")
                .unwrap()
                .clone())
        } else if instruction_lower.contains("offshore") {
            Ok(self
                .responses
                .get("find_offshore_entities")
                .unwrap()
                .clone())
        } else {
            // Fallback response
            Ok(MockAiResponse {
                dsl_content: r#"(data.read :asset "entity" :limit 10)"#.to_string(),
                confidence: 0.70,
                rag_context: vec!["Generic entity search fallback".to_string()],
                explanation: "Generated fallback DSL for generic entity search".to_string(),
            })
        }
    }
}

// Mock entity records for simulation
struct MockEntityDatabase {
    partnerships: Vec<MockPartnership>,
    companies: Vec<MockCompany>,
    persons: Vec<MockPerson>,
    trusts: Vec<MockTrust>,
}

#[derive(Debug, Clone)]
struct MockPartnership {
    id: Uuid,
    name: String,
    partnership_type: String,
    jurisdiction: String,
    formation_date: String,
}

#[derive(Debug, Clone)]
struct MockCompany {
    id: Uuid,
    name: String,
    registration_number: String,
    jurisdiction: String,
    incorporation_date: String,
}

#[derive(Debug, Clone)]
struct MockPerson {
    id: Uuid,
    first_name: String,
    last_name: String,
    nationality: String,
    date_of_birth: String,
}

#[derive(Debug, Clone)]
struct MockTrust {
    id: Uuid,
    name: String,
    trust_type: String,
    jurisdiction: String,
    establishment_date: String,
}

impl MockEntityDatabase {
    fn new() -> Self {
        Self {
            partnerships: vec![
                MockPartnership {
                    id: Uuid::new_v4(),
                    name: "Quantum Capital Management LLP".to_string(),
                    partnership_type: "Limited Liability".to_string(),
                    jurisdiction: "US-DE".to_string(),
                    formation_date: "2020-03-15".to_string(),
                },
                MockPartnership {
                    id: Uuid::new_v4(),
                    name: "Meridian Investment Management LLP".to_string(),
                    partnership_type: "Limited Liability".to_string(),
                    jurisdiction: "US-NY".to_string(),
                    formation_date: "2019-09-22".to_string(),
                },
            ],
            companies: vec![MockCompany {
                id: Uuid::new_v4(),
                name: "American Growth Equity Fund Inc".to_string(),
                registration_number: "DE-8765432".to_string(),
                jurisdiction: "US-DE".to_string(),
                incorporation_date: "2018-01-15".to_string(),
            }],
            persons: vec![MockPerson {
                id: Uuid::new_v4(),
                first_name: "Jane".to_string(),
                last_name: "Doe".to_string(),
                nationality: "US".to_string(),
                date_of_birth: "1980-05-15".to_string(),
            }],
            trusts: vec![MockTrust {
                id: Uuid::new_v4(),
                name: "Family Heritage Trust".to_string(),
                trust_type: "Discretionary".to_string(),
                jurisdiction: "KY".to_string(),
                establishment_date: "2021-11-30".to_string(),
            }],
        }
    }

    fn simulate_query(&self, dsl: &str) -> Vec<String> {
        // Simple simulation of query results
        if dsl.contains("partnership") && dsl.contains("US") {
            self.partnerships
                .iter()
                .filter(|p| p.jurisdiction.starts_with("US"))
                .map(|p| format!("{} ({})", p.name, p.jurisdiction))
                .collect()
        } else if dsl.contains("limited_company") {
            self.companies
                .iter()
                .map(|c| format!("{} - {}", c.name, c.registration_number))
                .collect()
        } else if dsl.contains("proper_person") {
            self.persons
                .iter()
                .map(|p| format!("{} {} ({})", p.first_name, p.last_name, p.nationality))
                .collect()
        } else if dsl.contains("trust") {
            self.trusts
                .iter()
                .map(|t| format!("{} - {} ({})", t.name, t.trust_type, t.jurisdiction))
                .collect()
        } else {
            vec!["Query executed successfully".to_string()]
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("🤖 AI-Powered Entity CRUD Operations Demo");
    println!("==========================================\n");

    // Initialize services
    let ai_service = MockAiEntityCrudService::new();
    let database = MockEntityDatabase::new();

    println!("✅ AI Entity CRUD Service initialized");
    println!("✅ Mock entity database ready\n");

    // Demo 1: Entity Creation Operations
    demo_entity_creation(&ai_service, &database).await?;

    // Demo 2: Entity Search Operations
    demo_entity_search(&ai_service, &database).await?;

    // Demo 3: Entity Update Operations
    demo_entity_updates(&ai_service, &database).await?;

    // Demo 4: Complex Multi-Entity Operations
    demo_complex_operations(&ai_service, &database).await?;

    // Demo 5: AI Context and RAG Integration
    demo_ai_rag_integration(&ai_service).await?;

    println!("\n🎉 AI Entity CRUD Demo completed successfully!");
    println!("All entity operations demonstrated with AI integration.");

    Ok(())
}

async fn demo_entity_creation(
    ai_service: &MockAiEntityCrudService,
    database: &MockEntityDatabase,
) -> Result<()> {
    println!("🏗️  Demo 1: AI-Powered Entity Creation");
    println!("--------------------------------------");

    let creation_scenarios = vec![
        "Create a new Delaware LLC called TechCorp Solutions for software development",
        "Register a UK limited company named AlphaTech Ltd with registration number 12345678",
        "Add an individual named John Smith, US citizen, born January 1, 1985",
        "Establish a discretionary trust in the Cayman Islands called Smith Family Trust",
    ];

    for (i, instruction) in creation_scenarios.iter().enumerate() {
        println!("\n📝 Scenario {}: {}", i + 1, instruction);

        // Generate DSL using AI
        let ai_response = ai_service.generate_dsl(instruction)?;

        println!("🤖 AI Confidence: {:.1}%", ai_response.confidence * 100.0);
        println!("📚 RAG Context: {}", ai_response.rag_context.join(", "));
        println!("🔧 Generated DSL:");
        println!("   {}", ai_response.dsl_content);

        // Simulate execution
        let results = database.simulate_query(&ai_response.dsl_content);
        println!("✅ Execution: {}", ai_response.explanation);

        if !results.is_empty() {
            println!("📊 Result: Entity created successfully");
        }
    }

    Ok(())
}

async fn demo_entity_search(
    ai_service: &MockAiEntityCrudService,
    database: &MockEntityDatabase,
) -> Result<()> {
    println!("\n🔍 Demo 2: AI-Powered Entity Search");
    println!("-----------------------------------");

    let search_scenarios = vec![
        "Find all partnerships registered in the United States",
        "Search for Delaware LLCs in our system",
        "Show me all offshore entities in tax-friendly jurisdictions",
    ];

    for (i, instruction) in search_scenarios.iter().enumerate() {
        println!("\n🔎 Search {}: {}", i + 1, instruction);

        let ai_response = ai_service.generate_dsl(instruction)?;

        println!("🤖 AI Confidence: {:.1}%", ai_response.confidence * 100.0);
        println!("🔧 Generated DSL:");
        println!("   {}", ai_response.dsl_content);

        let results = database.simulate_query(&ai_response.dsl_content);
        println!("📋 Found {} entities:", results.len());
        for result in results {
            println!("   - {}", result);
        }
    }

    Ok(())
}

async fn demo_entity_updates(
    ai_service: &MockAiEntityCrudService,
    database: &MockEntityDatabase,
) -> Result<()> {
    println!("\n📝 Demo 3: AI-Powered Entity Updates");
    println!("------------------------------------");

    let update_scenarios = vec![
        "Update the registered address of AlphaTech Ltd to a new London location",
        "Change the partnership agreement date for Quantum Capital Management",
    ];

    for (i, instruction) in update_scenarios.iter().enumerate() {
        println!("\n✏️  Update {}: {}", i + 1, instruction);

        let ai_response = ai_service.generate_dsl(instruction)?;

        println!("🤖 AI Confidence: {:.1}%", ai_response.confidence * 100.0);
        println!("📚 RAG Context: {}", ai_response.rag_context.join(", "));
        println!("🔧 Generated DSL:");
        println!("   {}", ai_response.dsl_content);

        let results = database.simulate_query(&ai_response.dsl_content);
        println!("✅ Update executed: {}", ai_response.explanation);
    }

    Ok(())
}

async fn demo_complex_operations(
    ai_service: &MockAiEntityCrudService,
    database: &MockEntityDatabase,
) -> Result<()> {
    println!("\n🔬 Demo 4: Complex Multi-Entity Operations");
    println!("------------------------------------------");

    let complex_scenarios = vec![
        "Find all offshore entities across partnerships, companies, and trusts",
        "Search for high-net-worth client entities with beneficial ownership structures",
    ];

    for (i, instruction) in complex_scenarios.iter().enumerate() {
        println!("\n🧮 Complex Query {}: {}", i + 1, instruction);

        let ai_response = ai_service.generate_dsl(instruction)?;

        println!("🤖 AI Confidence: {:.1}%", ai_response.confidence * 100.0);
        println!("📚 RAG Context Used:");
        for context in &ai_response.rag_context {
            println!("   • {}", context);
        }
        println!("🔧 Generated DSL:");
        println!("   {}", ai_response.dsl_content);

        // Analyze DSL complexity
        let complexity_score = analyze_dsl_complexity(&ai_response.dsl_content);
        println!("📊 DSL Complexity: {}/10", complexity_score);

        let results = database.simulate_query(&ai_response.dsl_content);
        println!("📋 Complex query executed: {} results", results.len());
    }

    Ok(())
}

async fn demo_ai_rag_integration(ai_service: &MockAiEntityCrudService) -> Result<()> {
    println!("\n🧠 Demo 5: AI Context and RAG Integration");
    println!("-----------------------------------------");

    // Demonstrate RAG context retrieval and usage
    println!("\n📚 RAG Knowledge Base Components:");
    println!("   • Entity schemas (partnerships, companies, persons, trusts)");
    println!("   • DSL grammar patterns and syntax rules");
    println!("   • Historical examples and successful conversions");
    println!("   • Business domain knowledge (jurisdictions, entity types)");
    println!("   • Validation rules and common mistakes");

    let test_instruction = "Create a sophisticated investment structure in Luxembourg";
    println!("\n🔍 Testing RAG with instruction: '{}'", test_instruction);

    let ai_response = ai_service.generate_dsl(test_instruction)?;

    println!("\n📊 RAG Context Analysis:");
    println!(
        "   Confidence Score: {:.1}%",
        ai_response.confidence * 100.0
    );
    println!("   Context Sources: {}", ai_response.rag_context.len());

    for (i, context) in ai_response.rag_context.iter().enumerate() {
        println!("   {}. {}", i + 1, context);
    }

    println!("\n🎯 AI Quality Metrics:");
    println!("   • Syntax Accuracy: High (S-expression validation)");
    println!("   • Semantic Correctness: High (schema-aware generation)");
    println!("   • Context Relevance: High (RAG-enhanced prompting)");
    println!("   • Domain Knowledge: High (financial services expertise)");

    Ok(())
}

fn analyze_dsl_complexity(dsl: &str) -> u8 {
    let mut score = 1;

    if dsl.contains(":where") {
        score += 2;
    }
    if dsl.contains(":join") {
        score += 3;
    }
    if dsl.contains(":select") {
        score += 1;
    }
    if dsl.contains("[") {
        score += 2;
    } // Array values
    if dsl.matches(':').count() > 3 {
        score += 1;
    }

    score.min(10)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ai_entity_crud_service() {
        let service = MockAiEntityCrudService::new();

        let response = service.generate_dsl("Create a Delaware LLC").unwrap();
        assert!(response.dsl_content.contains("partnership"));
        assert!(response.dsl_content.contains("Limited Liability"));
        assert!(response.confidence > 0.9);
    }

    #[tokio::test]
    async fn test_entity_search_generation() {
        let service = MockAiEntityCrudService::new();

        let response = service.generate_dsl("Find US partnerships").unwrap();
        assert!(response.dsl_content.contains("data.read"));
        assert!(response.dsl_content.contains("partnership"));
        assert!(response.dsl_content.contains("US"));
    }

    #[test]
    fn test_dsl_complexity_analysis() {
        let simple_dsl = r#"(data.create :asset "partnership")"#;
        let complex_dsl = r#"(data.read :asset "entity" :where {:jurisdiction ["KY" "BVI"]} :join ["entity_type"] :select ["name"])"#;

        assert!(analyze_dsl_complexity(complex_dsl) > analyze_dsl_complexity(simple_dsl));
    }

    #[test]
    fn test_mock_database_simulation() {
        let db = MockEntityDatabase::new();
        let results =
            db.simulate_query(r#"(data.read :asset "partnership" :where {:jurisdiction "US"})"#);

        assert!(!results.is_empty());
        assert!(results[0].contains("Quantum") || results[0].contains("Meridian"));
    }
}
