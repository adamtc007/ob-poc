//! Attribute Repository Demo
//!
//! This example demonstrates the type-safe attribute repository pattern
//! from Phase 3 of the Attribute Dictionary Refactoring.
//!
//! Run with: cargo run --example attribute_repository_demo --features database

#[cfg(feature = "database")]
use ob_poc::database::AttributeRepository;
use ob_poc::domains::attributes::kyc::*;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(not(feature = "database"))]
    {
        println!("This example requires the 'database' feature.");
        println!("Run with: cargo run --example attribute_repository_demo --features database");
        return Ok(());
    }

    #[cfg(feature = "database")]
    {
        println!("=== Attribute Repository Demo ===\n");

        // Create repository
        let repo = AttributeRepository::new();
        let entity_id = Uuid::new_v4();

        println!("Entity ID: {}\n", entity_id);

        // Example 1: Type-safe attribute setting with validation
        println!("1. Setting FirstName attribute...");
        match repo
            .set::<FirstName>(entity_id, "John".to_string(), Some("demo"))
            .await
        {
            Ok(id) => println!("   ✓ Set FirstName='John' (validation passed, id={})", id),
            Err(e) => println!("   ✗ Error: {}", e),
        }

        // Example 2: Validation catches invalid data
        println!("\n2. Attempting to set invalid FirstName (empty string)...");
        match repo
            .set::<FirstName>(entity_id, "".to_string(), Some("demo"))
            .await
        {
            Ok(_) => println!("   ✗ Should have failed!"),
            Err(e) => println!("   ✓ Validation caught error: {}", e),
        }

        // Example 3: Different attribute types
        println!("\n3. Setting Email attribute...");
        match repo
            .set::<Email>(entity_id, "john@example.com".to_string(), Some("demo"))
            .await
        {
            Ok(id) => println!("   ✓ Set Email='john@example.com' (id={})", id),
            Err(e) => println!("   ✗ Error: {}", e),
        }

        println!("\n4. Attempting to set invalid Email...");
        match repo
            .set::<Email>(entity_id, "not-an-email".to_string(), Some("demo"))
            .await
        {
            Ok(_) => println!("   ✗ Should have failed!"),
            Err(e) => println!("   ✓ Validation caught error: {}", e),
        }

        // Example 4: Numeric attributes with range validation
        println!("\n5. Setting UBO ownership percentage...");
        match repo
            .set::<UboOwnershipPercentage>(entity_id, 25.5, Some("demo"))
            .await
        {
            Ok(id) => println!("   ✓ Set ownership=25.5% (id={})", id),
            Err(e) => println!("   ✗ Error: {}", e),
        }

        println!("\n6. Attempting to set invalid percentage (> 100)...");
        match repo
            .set::<UboOwnershipPercentage>(entity_id, 150.0, Some("demo"))
            .await
        {
            Ok(_) => println!("   ✗ Should have failed!"),
            Err(e) => println!("   ✓ Validation caught error: {}", e),
        }

        // Example 5: Enum-based attributes
        println!("\n7. Setting FATCA status...");
        match repo
            .set::<FatcaStatus>(entity_id, "COMPLIANT".to_string(), Some("demo"))
            .await
        {
            Ok(id) => println!("   ✓ Set FATCA status='COMPLIANT' (id={})", id),
            Err(e) => println!("   ✗ Error: {}", e),
        }

        println!("\n8. Attempting to set invalid FATCA status...");
        match repo
            .set::<FatcaStatus>(entity_id, "INVALID".to_string(), Some("demo"))
            .await
        {
            Ok(_) => println!("   ✗ Should have failed!"),
            Err(e) => println!("   ✓ Validation caught error: {}", e),
        }

        println!("\n=== Demo Complete ===");
        println!("\nKey Benefits Demonstrated:");
        println!("  ✓ Compile-time type safety");
        println!("  ✓ Automatic validation before database");
        println!("  ✓ Clear error messages");
        println!("  ✓ No SQL in application code");
        println!("  ✓ Easy to test without database");
    }

    Ok(())
}
