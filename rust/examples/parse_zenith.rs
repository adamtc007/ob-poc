//! Simple AST Demonstration Example
//!
//! This example demonstrates working with DSL AST structures directly,
//! showcasing the capabilities without relying on complex parsing.

use anyhow::Result;
use std::collections::HashMap;

use ob_poc::{
    ast::{Program, Statement, Value, Workflow},
    system_info,
};

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Simple AST Demonstration");
    println!("============================\n");

    // Demo 1: Create AST manually
    demo_manual_ast_creation()?;

    // Demo 2: System information
    demo_system_info();

    // Demo 3: AST manipulation
    demo_ast_manipulation()?;

    println!("✅ All demonstrations completed successfully!");
    Ok(())
}

fn demo_manual_ast_creation() -> Result<()> {
    println!("🔧 Demo 1: Manual AST Creation");
    println!("------------------------------");

    // Create properties map
    let mut workflow_properties = HashMap::new();
    workflow_properties.insert(
        "nature-purpose".to_string(),
        Value::String("Simple customer onboarding demonstration".to_string()),
    );
    workflow_properties.insert("jurisdiction".to_string(), Value::String("US".to_string()));

    // Create entity properties
    let mut entity_properties = HashMap::new();
    entity_properties.insert(
        "legal-name".to_string(),
        Value::String("Example Corp".to_string()),
    );
    entity_properties.insert(
        "registration-number".to_string(),
        Value::String("12345".to_string()),
    );
    entity_properties.insert("status".to_string(), Value::String("active".to_string()));

    // Create document properties
    let mut document_properties = HashMap::new();
    document_properties.insert(
        "document-type".to_string(),
        Value::String("Certificate of Incorporation".to_string()),
    );
    document_properties.insert(
        "issuer".to_string(),
        Value::String("Delaware Secretary of State".to_string()),
    );
    document_properties.insert("confidence".to_string(), Value::Number(0.95));

    // Create edge properties
    let mut edge_properties = HashMap::new();
    edge_properties.insert("relationship-strength".to_string(), Value::Number(1.0));

    // Create statements
    let statements = vec![
        Statement::DeclareEntity {
            id: "customer1".to_string(),
            entity_type: "COMPANY".to_string(),
            properties: entity_properties,
        },
        Statement::ObtainDocument {
            document_type: "incorporation-cert".to_string(),
            source: "state-registry".to_string(),
            properties: document_properties,
        },
        Statement::CreateEdge {
            from: "customer1".to_string(),
            to: "incorporation-cert".to_string(),
            edge_type: "EVIDENCED_BY".to_string(),
            properties: edge_properties,
        },
    ];

    // Create workflow
    let workflow = Workflow {
        id: "simple-onboarding".to_string(),
        properties: workflow_properties,
        statements,
    };

    // Create program
    let program = Program {
        workflows: vec![workflow],
    };

    println!(
        "✅ Created AST program with {} workflow(s)",
        program.workflows.len()
    );

    for workflow in &program.workflows {
        println!("   📋 Workflow: {}", workflow.id);
        println!("      Properties: {}", workflow.properties.len());
        println!("      Statements: {}", workflow.statements.len());

        // Show workflow properties
        for (key, value) in &workflow.properties {
            println!("         {}: {}", key, format_value(value));
        }

        // Analyze statements
        let mut entity_count = 0;
        let mut document_count = 0;
        let mut edge_count = 0;

        for statement in &workflow.statements {
            match statement {
                Statement::DeclareEntity {
                    id, entity_type, ..
                } => {
                    entity_count += 1;
                    println!("      📍 Entity: {} ({})", id, entity_type);
                }
                Statement::ObtainDocument {
                    document_type,
                    source,
                    ..
                } => {
                    document_count += 1;
                    println!("      📄 Document: {} from {}", document_type, source);
                }
                Statement::CreateEdge {
                    from,
                    to,
                    edge_type,
                    ..
                } => {
                    edge_count += 1;
                    println!("      🔗 Edge: {} → {} ({})", from, to, edge_type);
                }
                _ => {}
            }
        }

        println!("      Summary:");
        println!("         - Entities: {}", entity_count);
        println!("         - Documents: {}", document_count);
        println!("         - Edges: {}", edge_count);
    }

    println!();
    Ok(())
}

fn demo_system_info() {
    println!("ℹ️  Demo 2: System Information");
    println!("------------------------------");

    let info = system_info();
    println!("✅ DSL System Information:");
    println!("   Package: {}", info.package_name);
    println!("   Version: {}", info.version);
    println!("   Rust version: {}", info.rust_version);
    println!("   Build date: {}", info.build_date);

    println!();
}

fn demo_ast_manipulation() -> Result<()> {
    println!("🔄 Demo 3: AST Manipulation");
    println!("----------------------------");

    // Start with empty program
    let mut program = Program {
        workflows: Vec::new(),
    };

    println!("✅ Started with empty program");

    // Add a workflow
    let mut properties = HashMap::new();
    properties.insert(
        "created-by".to_string(),
        Value::String("demo-system".to_string()),
    );

    let workflow = Workflow {
        id: "dynamic-workflow".to_string(),
        properties,
        statements: Vec::new(),
    };

    program.workflows.push(workflow);
    println!("   Added workflow: dynamic-workflow");

    // Add statements to the workflow
    if let Some(workflow) = program.workflows.get_mut(0) {
        // Add entity
        let mut entity_props = HashMap::new();
        entity_props.insert(
            "name".to_string(),
            Value::String("Dynamic Entity".to_string()),
        );
        entity_props.insert(
            "created-at".to_string(),
            Value::String("2024-01-01".to_string()),
        );

        workflow.statements.push(Statement::DeclareEntity {
            id: "dynamic-entity".to_string(),
            entity_type: "DYNAMIC".to_string(),
            properties: entity_props,
        });

        println!("   Added entity to workflow");

        // Add document
        let mut doc_props = HashMap::new();
        doc_props.insert("format".to_string(), Value::String("PDF".to_string()));

        workflow.statements.push(Statement::ObtainDocument {
            document_type: "dynamic-document".to_string(),
            source: "internal-system".to_string(),
            properties: doc_props,
        });

        println!("   Added document to workflow");
    }

    // Show final structure
    println!("   Final program structure:");
    println!("      Workflows: {}", program.workflows.len());
    for workflow in &program.workflows {
        println!(
            "         {}: {} statements",
            workflow.id,
            workflow.statements.len()
        );
    }

    println!();
    Ok(())
}

fn format_value(value: &Value) -> String {
    match value {
        Value::String(s) => format!("\"{}\"", s),
        Value::Number(n) => n.to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::Date(d) => d.to_string(),
        Value::List(items) => format!("[{} items]", items.len()),
        Value::Map(map) => format!("{{map with {} keys}}", map.len()),
        Value::MultiValue(values) => format!("multi-value with {} entries", values.len()),
        Value::Null => "null".to_string(),
    }
}
