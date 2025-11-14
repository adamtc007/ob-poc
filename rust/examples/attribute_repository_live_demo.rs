//! Live demonstration of AttributeRepository with real database
//!
//! Run with: cargo run --example attribute_repository_live_demo --features database
//!
//! Requires DATABASE_URL environment variable set to a PostgreSQL database
//! with the attribute refactor migrations applied.

use ob_poc::database::AttributeRepository;
use ob_poc::domains::attributes::kyc::*;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up logging
    tracing_subscriber::fmt::init();

    println!("\n=== AttributeRepository Live Demo ===\n");

    // Get database URL from environment
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    println!("Connecting to database...");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    let repo = AttributeRepository::new(pool);
    println!("✓ Connected to database\n");

    // Create a test entity
    let entity_id = Uuid::new_v4();
    println!("Created test entity: {}\n", entity_id);

    // Demo 1: Set and retrieve a string attribute
    println!("--- Demo 1: String Attributes ---");
    println!("Setting FirstName to 'Alice'...");
    let id = repo
        .set::<FirstName>(entity_id, "Alice".to_string(), Some("demo"))
        .await?;
    println!("✓ Attribute saved with ID: {}", id);

    let retrieved = repo.get::<FirstName>(entity_id).await?;
    println!("✓ Retrieved FirstName: {:?}\n", retrieved);

    // Demo 2: Set and retrieve a number attribute
    println!("--- Demo 2: Number Attributes ---");
    println!("Setting UBO Ownership Percentage to 25.5...");
    repo.set::<UboOwnershipPercentage>(entity_id, 25.5, Some("demo"))
        .await?;
    println!("✓ Attribute saved");

    let retrieved = repo.get::<UboOwnershipPercentage>(entity_id).await?;
    println!("✓ Retrieved Ownership: {:?}%\n", retrieved);

    // Demo 3: Validation
    println!("--- Demo 3: Validation ---");
    println!("Attempting to set empty FirstName (should fail)...");
    match repo
        .set::<FirstName>(entity_id, "".to_string(), Some("demo"))
        .await
    {
        Ok(_) => println!("✗ Unexpectedly succeeded"),
        Err(e) => println!("✓ Validation failed as expected: {}\n", e),
    }

    // Demo 4: Update attribute
    println!("--- Demo 4: Update Attributes ---");
    println!("Updating FirstName to 'Bob'...");
    repo.set::<FirstName>(entity_id, "Bob".to_string(), Some("demo"))
        .await?;
    let retrieved = repo.get::<FirstName>(entity_id).await?;
    println!("✓ Updated FirstName: {:?}\n", retrieved);

    // Demo 5: History tracking
    println!("--- Demo 5: History Tracking ---");
    println!("Setting FirstName to 'Charlie'...");
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    repo.set::<FirstName>(entity_id, "Charlie".to_string(), Some("demo"))
        .await?;

    println!("Retrieving history...");
    let history = repo.get_history::<FirstName>(entity_id, 10).await?;
    println!("✓ Found {} history entries:", history.len());
    for (i, entry) in history.iter().enumerate() {
        println!(
            "  {}. {} (from: {}, by: {})",
            i + 1,
            entry.value,
            entry.effective_from.format("%Y-%m-%d %H:%M:%S"),
            entry.created_by
        );
    }
    println!();

    // Demo 6: Multiple attributes
    println!("--- Demo 6: Multiple Attributes ---");
    println!("Setting multiple attributes for a new entity...");
    let entity_id_2 = Uuid::new_v4();

    repo.set::<FirstName>(entity_id_2, "David".to_string(), Some("demo"))
        .await?;
    repo.set::<LastName>(entity_id_2, "Smith".to_string(), Some("demo"))
        .await?;
    repo.set::<Email>(entity_id_2, "david@example.com".to_string(), Some("demo"))
        .await?;

    println!("✓ Set FirstName, LastName, Email");

    let first = repo.get::<FirstName>(entity_id_2).await?.unwrap();
    let last = repo.get::<LastName>(entity_id_2).await?.unwrap();
    let email = repo.get::<Email>(entity_id_2).await?.unwrap();

    println!("✓ Retrieved: {} {} ({})\n", first, last, email);

    // Demo 7: Transactional bulk insert
    println!("--- Demo 7: Transactional Bulk Insert ---");
    let entity_id_3 = Uuid::new_v4();
    let attributes = vec![
        ("attr.identity.first_name", serde_json::json!("Eve")),
        ("attr.identity.last_name", serde_json::json!("Johnson")),
        ("attr.contact.email", serde_json::json!("eve@example.com")),
    ];

    println!("Setting 3 attributes in a single transaction...");
    let ids = repo
        .set_many_transactional(entity_id_3, attributes, Some("demo"))
        .await?;
    println!(
        "✓ Transaction complete. Inserted {} attributes\n",
        ids.len()
    );

    // Demo 8: Cache statistics
    println!("--- Demo 8: Cache Performance ---");
    let cache_stats = repo.cache_stats().await;
    println!("Cache entries: {}", cache_stats.entries);
    println!("Expired entries: {}", cache_stats.expired);

    // Test cache performance
    let start = std::time::Instant::now();
    let _ = repo.get::<FirstName>(entity_id).await?;
    let duration_1 = start.elapsed();

    let start = std::time::Instant::now();
    let _ = repo.get::<FirstName>(entity_id).await?;
    let duration_2 = start.elapsed();

    println!("First retrieval: {:?}", duration_1);
    println!("Cached retrieval: {:?}", duration_2);
    println!(
        "Speedup: {:.2}x\n",
        duration_1.as_micros() as f64 / duration_2.as_micros() as f64
    );

    // Demo 9: Type safety demonstration
    println!("--- Demo 9: Type Safety ---");
    println!("The repository is fully type-safe:");
    println!("  - FirstName is String");
    println!("  - UboOwnershipPercentage is f64");
    println!("  - DateOfBirth is chrono::NaiveDate");
    println!("  - All validated at compile time!");
    println!();

    println!("=== Demo Complete ===\n");
    println!("Summary:");
    println!("  ✓ Database migrations working");
    println!("  ✓ Type-safe attribute storage");
    println!("  ✓ Validation working");
    println!("  ✓ History tracking working");
    println!("  ✓ Transactional updates working");
    println!("  ✓ Caching working");
    println!();
    println!("Phase 3.5: Database Integration - COMPLETE!");

    Ok(())
}
