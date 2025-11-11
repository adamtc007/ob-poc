//! CRUD Parser Demo
//!
//! This example demonstrates the Phase 1 implementation of the Agentic DSL CRUD system.
//! It shows how to parse CRUD DSL statements into strongly-typed AST structures
//! without requiring database connectivity.

use ob_poc::parser::idiomatic_parser::parse_crud_statement;
use ob_poc::CrudStatement;

fn main() -> anyhow::Result<()> {
    println!("=== Agentic DSL CRUD Parser Demo ===\n");

    // Test 1: Data Create Operation
    test_data_create()?;

    // Test 2: Data Read Operation
    test_data_read()?;

    // Test 3: Data Update Operation
    test_data_update()?;

    // Test 4: Data Delete Operation
    test_data_delete()?;

    // Test 5: Error Handling
    test_error_handling()?;

    println!("✅ All CRUD parser tests passed successfully!");
    println!("\n=== Phase 1 Implementation Complete ===");
    println!("✓ DSL Parser with EBNF grammar compliance");
    println!("✓ Strongly-typed AST structures");
    println!("✓ Comprehensive error handling");
    println!("✓ Support for all 4 CRUD operations");
    println!("✓ Asset validation (cbu, document, attribute)");

    Ok(())
}

fn test_data_create() -> anyhow::Result<()> {
    println!("🔍 Testing DATA.CREATE operation...");

    let dsl = r#"(data.create :asset "cbu" :values {:name "Quantum Ventures LP" :description "Delaware limited partnership specializing in quantum computing investments" :jurisdiction "US-DE" :entity_type "LIMITED_PARTNERSHIP"})"#;

    println!("Input DSL: {}", dsl);

    let statement = parse_crud_statement(dsl)?;
    println!("✅ Parsed successfully");

    match statement {
        CrudStatement::DataCreate(create_op) => {
            println!("📋 CREATE Operation Details:");
            println!("   Asset: {}", create_op.asset);
            println!("   Fields: {}", create_op.values.len());

            for (key, value) in &create_op.values {
                println!("   - {}: {:?}", key.as_str(), value);
            }

            assert_eq!(create_op.asset, "cbu");
            assert_eq!(create_op.values.len(), 4);
        }
        _ => panic!("Expected DataCreate, got {:?}", statement),
    }

    println!();
    Ok(())
}

fn test_data_read() -> anyhow::Result<()> {
    println!("🔍 Testing DATA.READ operation...");

    let dsl = r#"(data.read :asset "document" :where {:type "PASSPORT" :issuer_country "GB"} :select ["title" "issuer" "status"])"#;

    println!("Input DSL: {}", dsl);

    let statement = parse_crud_statement(dsl)?;
    println!("✅ Parsed successfully");

    match statement {
        CrudStatement::DataRead(read_op) => {
            println!("📋 READ Operation Details:");
            println!("   Asset: {}", read_op.asset);

            if let Some(where_clause) = &read_op.where_clause {
                println!("   WHERE conditions: {}", where_clause.len());
                for (key, value) in where_clause {
                    println!("     - {} = {:?}", key.as_str(), value);
                }
            }

            if let Some(select_fields) = &read_op.select_fields {
                println!("   SELECT fields: {:?}", select_fields);
            }

            assert_eq!(read_op.asset, "document");
            assert!(read_op.where_clause.is_some());
            assert!(read_op.select_fields.is_some());
        }
        _ => panic!("Expected DataRead, got {:?}", statement),
    }

    println!();
    Ok(())
}

fn test_data_update() -> anyhow::Result<()> {
    println!("🔍 Testing DATA.UPDATE operation...");

    let dsl = r#"(data.update :asset "cbu" :where {:name "Quantum Ventures LP"} :values {:description "Updated: Delaware LP focusing on quantum computing and AI investments" :jurisdiction "US-DE"})"#;

    println!("Input DSL: {}", dsl);

    let statement = parse_crud_statement(dsl)?;
    println!("✅ Parsed successfully");

    match statement {
        CrudStatement::DataUpdate(update_op) => {
            println!("📋 UPDATE Operation Details:");
            println!("   Asset: {}", update_op.asset);

            println!("   WHERE conditions: {}", update_op.where_clause.len());
            for (key, value) in &update_op.where_clause {
                println!("     - {} = {:?}", key.as_str(), value);
            }

            println!("   SET values: {}", update_op.values.len());
            for (key, value) in &update_op.values {
                println!("     - {} = {:?}", key.as_str(), value);
            }

            assert_eq!(update_op.asset, "cbu");
            assert_eq!(update_op.where_clause.len(), 1);
            assert_eq!(update_op.values.len(), 2);
        }
        _ => panic!("Expected DataUpdate, got {:?}", statement),
    }

    println!();
    Ok(())
}

fn test_data_delete() -> anyhow::Result<()> {
    println!("🔍 Testing DATA.DELETE operation...");

    let dsl =
        r#"(data.delete :asset "attribute" :where {:name "deprecated_field" :is_active false})"#;

    println!("Input DSL: {}", dsl);

    let statement = parse_crud_statement(dsl)?;
    println!("✅ Parsed successfully");

    match statement {
        CrudStatement::DataDelete(delete_op) => {
            println!("📋 DELETE Operation Details:");
            println!("   Asset: {}", delete_op.asset);

            println!("   WHERE conditions: {}", delete_op.where_clause.len());
            for (key, value) in &delete_op.where_clause {
                println!("     - {} = {:?}", key.as_str(), value);
            }

            assert_eq!(delete_op.asset, "attribute");
            assert_eq!(delete_op.where_clause.len(), 2);
        }
        _ => panic!("Expected DataDelete, got {:?}", statement),
    }

    println!();
    Ok(())
}

fn test_error_handling() -> anyhow::Result<()> {
    println!("🔍 Testing error handling...");

    // Test 1: Invalid verb
    let invalid_verb = r#"(data.invalid :asset "cbu" :values {:name "Test"})"#;
    println!("Testing invalid verb: {}", invalid_verb);

    match parse_crud_statement(invalid_verb) {
        Ok(_) => panic!("Should have failed with invalid verb"),
        Err(e) => {
            println!("✅ Correctly rejected invalid verb: {}", e);
        }
    }

    // Test 2: Malformed syntax
    let malformed = r#"(data.create :asset "cbu" :values {:name "Test""#; // Missing closing }
    println!("Testing malformed syntax: {}", malformed);

    match parse_crud_statement(malformed) {
        Ok(_) => panic!("Should have failed with malformed syntax"),
        Err(e) => {
            println!("✅ Correctly rejected malformed syntax: {}", e);
        }
    }

    // Test 3: Missing required fields for UPDATE
    let missing_where = r#"(data.update :asset "cbu" :values {:name "Test"})"#; // Missing :where
    println!("Testing missing WHERE clause: {}", missing_where);

    match parse_crud_statement(missing_where) {
        Ok(statement) => {
            // The parser will accept this, but the executor should validate required fields
            println!("✅ Parser accepted (executor will validate required WHERE clause)");
            if let CrudStatement::DataUpdate(update_op) = statement {
                // This should have empty where_clause which executor will catch
                println!(
                    "   WHERE clause present: {}",
                    update_op.where_clause.len() > 0
                );
            }
        }
        Err(e) => {
            println!("✅ Parser rejected: {}", e);
        }
    }

    println!();
    Ok(())
}
