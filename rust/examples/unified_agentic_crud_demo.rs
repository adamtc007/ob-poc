//! Unified Agentic CRUD Demo - Complete Integration Testing
//!
//! This demo showcases the unified agentic CRUD system that brings together
//! CBU operations, Entity management, and Document processing into a single
//! AI-powered interface. It demonstrates the complete workflow from natural
//! language instructions to executed operations across all three domains.

use anyhow::Result;
use ob_poc::ai::unified_agentic_service::{
    OperationType, UnifiedAgenticRequest, UnifiedAgenticService, UnifiedContext,
    UnifiedOperationResult,
};
use std::time::Instant;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 === Unified Agentic CRUD Demo ===");
    println!("Testing AI-powered operations across CBUs, Entities, and Documents\n");

    let demo = UnifiedAgenticCrudDemo::new().await;
    demo.run_comprehensive_demo().await?;

    Ok(())
}

/// Comprehensive demo for the unified agentic CRUD system
pub struct UnifiedAgenticCrudDemo {
    service: UnifiedAgenticService,
    test_scenarios: Vec<TestScenario>,
}

/// Test scenario for unified operations
#[derive(Debug, Clone)]
pub struct TestScenario {
    pub name: String,
    pub description: String,
    pub instruction: String,
    pub expected_operation_type: OperationType,
    pub context: UnifiedContext,
    pub execute: bool,
}

impl UnifiedAgenticCrudDemo {
    /// Create a new demo instance
    pub async fn new() -> Self {
        let service = UnifiedAgenticService::with_mock();
        let test_scenarios = create_test_scenarios();

        Self {
            service,
            test_scenarios,
        }
    }

    /// Run the comprehensive demo
    pub async fn run_comprehensive_demo(&self) -> Result<()> {
        println!(
            "📋 Running {} test scenarios...\n",
            self.test_scenarios.len()
        );

        // Step 1: Service Health Check
        self.demonstrate_service_health().await?;

        // Step 2: Individual Operation Type Tests
        self.demonstrate_operation_detection().await?;

        // Step 3: Comprehensive Workflow Tests
        self.demonstrate_complete_workflows().await?;

        // Step 4: Cross-Domain Integration Tests
        self.demonstrate_cross_domain_operations().await?;

        // Step 5: Performance and Statistics
        self.demonstrate_performance_metrics().await?;

        // Step 6: Error Handling and Edge Cases
        self.demonstrate_error_handling().await?;

        println!("\n🎉 === Demo Complete ===");
        println!("✅ All unified agentic CRUD operations demonstrated successfully");

        Ok(())
    }

    /// Demonstrate service health and capabilities
    async fn demonstrate_service_health(&self) -> Result<()> {
        println!("🏥 === Service Health Check ===\n");

        // Health check
        let health = self.service.health_check().await?;
        println!(
            "   ✅ Service health: {}",
            if health { "HEALTHY" } else { "UNHEALTHY" }
        );

        // Available operations
        let operations = self.service.get_available_operations();
        println!("   📋 Available operations:");
        for operation in operations {
            println!("      • {}", operation);
        }

        // Service statistics
        let stats = self.service.get_statistics();
        println!("   📊 Service statistics:");
        println!("      • Total requests: {}", stats.total_requests);
        println!("      • Uptime: {} seconds", stats.uptime_seconds);

        println!("   ✅ Service health check complete\n");
        Ok(())
    }

    /// Demonstrate operation type detection
    async fn demonstrate_operation_detection(&self) -> Result<()> {
        println!("🔍 === Operation Type Detection ===\n");

        let detection_tests = vec![
            ("Create a new hedge fund client", OperationType::Cbu),
            (
                "Add a partnership entity called Tech Partners LLC",
                OperationType::Entity,
            ),
            (
                "Extract data from this passport document",
                OperationType::Document,
            ),
            (
                "Upload and catalog a corporate certificate",
                OperationType::Document,
            ),
            ("Find all companies in Delaware", OperationType::Entity),
            (
                "Show me the onboarding status for client ABC",
                OperationType::Cbu,
            ),
            ("Perform comprehensive analysis", OperationType::General),
        ];

        for (instruction, expected) in detection_tests {
            let request = UnifiedAgenticRequest {
                instruction: instruction.to_string(),
                operation_type_hint: None, // Let it auto-detect
                context: UnifiedContext::default(),
                execute: false,
                request_id: Some(format!("detect_{}", Uuid::new_v4())),
            };

            let response = self.service.process_request(request).await?;

            let status = if response.operation_type == expected {
                "✅"
            } else {
                "❌"
            };
            println!("   {} \"{}\"", status, instruction);
            println!(
                "      Expected: {:?}, Got: {:?}",
                expected, response.operation_type
            );
        }

        println!("   ✅ Operation detection tests complete\n");
        Ok(())
    }

    /// Demonstrate complete workflows for each operation type
    async fn demonstrate_complete_workflows(&self) -> Result<()> {
        println!("🔄 === Complete Workflow Demonstrations ===\n");

        for (i, scenario) in self.test_scenarios.iter().enumerate() {
            println!("   🎯 Scenario {}: {}", i + 1, scenario.name);
            println!("      Description: {}", scenario.description);
            println!("      Instruction: \"{}\"", scenario.instruction);

            let start_time = Instant::now();

            let request = UnifiedAgenticRequest {
                instruction: scenario.instruction.clone(),
                operation_type_hint: Some(scenario.expected_operation_type.clone()),
                context: scenario.context.clone(),
                execute: scenario.execute,
                request_id: Some(format!("workflow_{}", i)),
            };

            match self.service.process_request(request).await {
                Ok(response) => {
                    let duration = start_time.elapsed().as_millis();

                    println!("      ✅ Success in {}ms", duration);
                    println!("      Operation Type: {:?}", response.operation_type);
                    println!(
                        "      Generated DSL: {}",
                        if response.generated_dsl.len() > 60 {
                            format!("{}...", &response.generated_dsl[..60])
                        } else {
                            response.generated_dsl
                        }
                    );

                    // Display operation result details
                    match response.operation_result {
                        UnifiedOperationResult::CbuResult {
                            operation,
                            affected_records,
                            ..
                        } => {
                            println!(
                                "      CBU Operation: {} (affected: {})",
                                operation, affected_records
                            );
                        }
                        UnifiedOperationResult::EntityResult {
                            operation,
                            entity_type,
                            affected_records,
                            ..
                        } => {
                            println!(
                                "      Entity Operation: {} on {} (affected: {})",
                                operation, entity_type, affected_records
                            );
                        }
                        UnifiedOperationResult::DocumentResult {
                            operation,
                            doc_id,
                            metadata_updated,
                            ..
                        } => {
                            println!(
                                "      Document Operation: {} (doc: {:?}, metadata updated: {})",
                                operation, doc_id, metadata_updated
                            );
                        }
                        UnifiedOperationResult::GeneralResult {
                            operations_performed,
                            total_affected_records,
                            ..
                        } => {
                            println!(
                                "      General Operations: {:?} (total affected: {})",
                                operations_performed, total_affected_records
                            );
                        }
                    }

                    // Display performance metrics
                    println!("      Performance:");
                    println!(
                        "         • Detection: {}ms",
                        response.metadata.detection_time_ms
                    );
                    println!("         • RAG: {}ms", response.metadata.rag_time_ms);
                    println!(
                        "         • AI Generation: {}ms",
                        response.metadata.ai_generation_time_ms
                    );
                    println!(
                        "         • Execution: {}ms",
                        response.metadata.execution_time_ms
                    );
                    println!("         • Total: {}ms", response.metadata.total_time_ms);
                }
                Err(e) => {
                    println!("      ❌ Failed: {}", e);
                }
            }

            println!();
        }

        println!("   ✅ Workflow demonstrations complete\n");
        Ok(())
    }

    /// Demonstrate cross-domain operations
    async fn demonstrate_cross_domain_operations(&self) -> Result<()> {
        println!("🌐 === Cross-Domain Integration Tests ===\n");

        let cross_domain_scenarios = vec![
            CrossDomainScenario {
                name: "Complete Client Onboarding".to_string(),
                description: "Create CBU, add entities, upload documents".to_string(),
                steps: vec![
                    "Create a new client called Global Investment Partners".to_string(),
                    "Add the main entity as a Cayman Islands company".to_string(),
                    "Upload and catalog their certificate of incorporation".to_string(),
                    "Link the document to the entity for KYC verification".to_string(),
                ],
            },
            CrossDomainScenario {
                name: "Document-Driven Entity Discovery".to_string(),
                description: "Extract entity data from documents and create entities".to_string(),
                steps: vec![
                    "Extract entity information from uploaded corporate documents".to_string(),
                    "Create partnership entities based on extracted data".to_string(),
                    "Link all entities to the source documents".to_string(),
                ],
            },
            CrossDomainScenario {
                name: "Compliance Audit Trail".to_string(),
                description: "Track document usage across CBUs and entities".to_string(),
                steps: vec![
                    "Find all documents used for client XYZ".to_string(),
                    "List all entities associated with those documents".to_string(),
                    "Generate compliance report for the CBU".to_string(),
                ],
            },
        ];

        for (i, scenario) in cross_domain_scenarios.iter().enumerate() {
            println!("   🎯 Cross-Domain Scenario {}: {}", i + 1, scenario.name);
            println!("      Description: {}", scenario.description);

            for (j, step) in scenario.steps.iter().enumerate() {
                println!("      Step {}: {}", j + 1, step);

                let request = UnifiedAgenticRequest {
                    instruction: step.clone(),
                    operation_type_hint: None,
                    context: UnifiedContext {
                        cbu_id: Some(Uuid::new_v4()),
                        entity_id: if j > 0 { Some(Uuid::new_v4()) } else { None },
                        doc_id: if step.contains("document") {
                            Some(Uuid::new_v4())
                        } else {
                            None
                        },
                        hints: vec!["cross_domain".to_string()],
                        domain: Some("compliance".to_string()),
                    },
                    execute: false,
                    request_id: Some(format!("cross_{}_{}", i, j)),
                };

                match self.service.process_request(request).await {
                    Ok(response) => {
                        println!(
                            "         ✅ Success - Operation: {:?}",
                            response.operation_type
                        );
                    }
                    Err(e) => {
                        println!("         ❌ Failed: {}", e);
                    }
                }
            }

            println!();
        }

        println!("   ✅ Cross-domain integration tests complete\n");
        Ok(())
    }

    /// Demonstrate performance metrics and optimization
    async fn demonstrate_performance_metrics(&self) -> Result<()> {
        println!("📈 === Performance Metrics & Optimization ===\n");

        // Batch performance test
        println!("   🔄 Running batch performance test...");

        let batch_instructions = vec![
            "Create client Alpha Fund",
            "Add entity Beta Corp",
            "Upload document gamma.pdf",
            "Find all clients",
            "Extract document data",
            "Update entity information",
            "Generate compliance report",
            "Search for documents",
            "Create partnership entity",
            "Link documents to client",
        ];

        let mut total_time = 0u64;
        let mut success_count = 0;

        for (i, instruction) in batch_instructions.iter().enumerate() {
            let request = UnifiedAgenticRequest {
                instruction: instruction.to_string(),
                operation_type_hint: None,
                context: UnifiedContext::default(),
                execute: false,
                request_id: Some(format!("perf_{}", i)),
            };

            let _start = Instant::now();
            match self.service.process_request(request).await {
                Ok(response) => {
                    success_count += 1;
                    total_time += response.metadata.total_time_ms;
                }
                Err(_) => {}
            }
        }

        let avg_time = if success_count > 0 {
            total_time / success_count as u64
        } else {
            0
        };
        let success_rate = (success_count as f64 / batch_instructions.len() as f64) * 100.0;

        println!("   📊 Batch Performance Results:");
        println!("      • Total requests: {}", batch_instructions.len());
        println!("      • Successful: {}", success_count);
        println!("      • Success rate: {:.1}%", success_rate);
        println!("      • Average processing time: {}ms", avg_time);
        println!("      • Total processing time: {}ms", total_time);

        // Concurrent operations test
        println!("\n   ⚡ Testing concurrent operations...");

        let concurrent_start = Instant::now();
        let concurrent_requests: Vec<_> = (0..5)
            .map(|i| UnifiedAgenticRequest {
                instruction: format!("Concurrent operation {}", i),
                operation_type_hint: None,
                context: UnifiedContext::default(),
                execute: false,
                request_id: Some(format!("concurrent_{}", i)),
            })
            .collect();

        // Simulate concurrent processing (in a real scenario, you'd use join_all)
        let mut concurrent_success = 0;
        for request in concurrent_requests {
            if self.service.process_request(request).await.is_ok() {
                concurrent_success += 1;
            }
        }

        let concurrent_duration = concurrent_start.elapsed().as_millis();
        println!("      • Concurrent operations: 5");
        println!("      • Successful: {}", concurrent_success);
        println!("      • Total time: {}ms", concurrent_duration);

        println!("   ✅ Performance metrics demonstration complete\n");
        Ok(())
    }

    /// Demonstrate error handling and edge cases
    async fn demonstrate_error_handling(&self) -> Result<()> {
        println!("🛡️ === Error Handling & Edge Cases ===\n");

        let error_scenarios = vec![
            ("", "Empty instruction"),
            ("ksdjfksjdf ksjdfkjsd fjksdf", "Nonsensical instruction"),
            (
                "Do something with UUID 00000000-0000-0000-0000-000000000000",
                "Invalid UUID reference",
            ),
            (
                "Create 1000000 entities simultaneously",
                "Resource-intensive operation",
            ),
            ("Delete everything in the database", "Dangerous operation"),
            ("Show me the secret passwords", "Security-sensitive request"),
        ];

        for (instruction, description) in error_scenarios {
            println!("   🧪 Testing: {}", description);
            println!("      Instruction: \"{}\"", instruction);

            let request = UnifiedAgenticRequest {
                instruction: instruction.to_string(),
                operation_type_hint: None,
                context: UnifiedContext::default(),
                execute: false,
                request_id: Some(format!("error_{}", Uuid::new_v4())),
            };

            match self.service.process_request(request).await {
                Ok(response) => {
                    if response.success {
                        println!(
                            "      ✅ Handled gracefully - Type: {:?}",
                            response.operation_type
                        );
                        if !response.warnings.is_empty() {
                            println!("         Warnings: {:?}", response.warnings);
                        }
                    } else {
                        println!(
                            "      ⚠️  Failed gracefully with {} errors",
                            response.errors.len()
                        );
                    }
                }
                Err(e) => {
                    println!("      ❌ Error caught: {}", e);
                }
            }
        }

        println!("   ✅ Error handling demonstration complete\n");
        Ok(())
    }
}

/// Cross-domain scenario for testing
#[derive(Debug, Clone)]
struct CrossDomainScenario {
    name: String,
    description: String,
    steps: Vec<String>,
}

/// Create comprehensive test scenarios
fn create_test_scenarios() -> Vec<TestScenario> {
    vec![
        // CBU Operations
        TestScenario {
            name: "CBU Creation".to_string(),
            description: "Create a new client business unit for a hedge fund".to_string(),
            instruction: "Create a new client called Zenith Capital Management, a hedge fund from Cayman Islands".to_string(),
            expected_operation_type: OperationType::Cbu,
            context: UnifiedContext::default(),
            execute: false,
        },
        TestScenario {
            name: "CBU Query".to_string(),
            description: "Search for existing client business units".to_string(),
            instruction: "Find all clients that are hedge funds".to_string(),
            expected_operation_type: OperationType::Cbu,
            context: UnifiedContext::default(),
            execute: false,
        },

        // Entity Operations
        TestScenario {
            name: "Company Entity Creation".to_string(),
            description: "Create a new company entity".to_string(),
            instruction: "Add a new company entity called Tech Innovations LLC, incorporated in Delaware".to_string(),
            expected_operation_type: OperationType::Entity,
            context: UnifiedContext {
                cbu_id: Some(Uuid::new_v4()),
                hints: vec!["company".to_string(), "delaware".to_string()],
                ..Default::default()
            },
            execute: false,
        },
        TestScenario {
            name: "Partnership Entity Creation".to_string(),
            description: "Create a partnership entity with multiple partners".to_string(),
            instruction: "Create a partnership entity called Global Investment Partners with three managing partners".to_string(),
            expected_operation_type: OperationType::Entity,
            context: UnifiedContext {
                hints: vec!["partnership".to_string(), "multiple_partners".to_string()],
                ..Default::default()
            },
            execute: false,
        },
        TestScenario {
            name: "Trust Entity Creation".to_string(),
            description: "Create a complex trust entity".to_string(),
            instruction: "Add a discretionary trust entity called Family Wealth Trust with offshore trustees".to_string(),
            expected_operation_type: OperationType::Entity,
            context: UnifiedContext {
                hints: vec!["trust".to_string(), "offshore".to_string()],
                ..Default::default()
            },
            execute: false,
        },

        // Document Operations
        TestScenario {
            name: "Document Cataloging".to_string(),
            description: "Catalog a new document in the system".to_string(),
            instruction: "Catalog this passport document for John Smith, issued by UK Home Office".to_string(),
            expected_operation_type: OperationType::Document,
            context: UnifiedContext {
                hints: vec!["passport".to_string(), "uk".to_string()],
                ..Default::default()
            },
            execute: false,
        },
        TestScenario {
            name: "Document Extraction".to_string(),
            description: "Extract data from a document using AI".to_string(),
            instruction: "Extract all relevant data from this corporate certificate of incorporation".to_string(),
            expected_operation_type: OperationType::Document,
            context: UnifiedContext {
                doc_id: Some(Uuid::new_v4()),
                hints: vec!["corporate".to_string(), "certificate".to_string()],
                ..Default::default()
            },
            execute: false,
        },
        TestScenario {
            name: "Document Search".to_string(),
            description: "Search for documents matching criteria".to_string(),
            instruction: "Find all passport documents from European countries uploaded in the last month".to_string(),
            expected_operation_type: OperationType::Document,
            context: UnifiedContext {
                hints: vec!["passport".to_string(), "european".to_string(), "recent".to_string()],
                ..Default::default()
            },
            execute: false,
        },
        TestScenario {
            name: "Document Linking".to_string(),
            description: "Link documents together in relationships".to_string(),
            instruction: "Link this amended certificate to the original incorporation document".to_string(),
            expected_operation_type: OperationType::Document,
            context: UnifiedContext {
                doc_id: Some(Uuid::new_v4()),
                hints: vec!["amendment".to_string(), "link".to_string()],
                ..Default::default()
            },
            execute: false,
        },

        // Mixed/Complex Operations
        TestScenario {
            name: "Complete Onboarding Workflow".to_string(),
            description: "Perform a complete client onboarding with entities and documents".to_string(),
            instruction: "Complete onboarding for new client Alpha Investment Group including their management company and required documentation".to_string(),
            expected_operation_type: OperationType::General,
            context: UnifiedContext {
                hints: vec!["onboarding".to_string(), "complete".to_string(), "management_company".to_string()],
                domain: Some("onboarding".to_string()),
                ..Default::default()
            },
            execute: false,
        },
    ]
}
