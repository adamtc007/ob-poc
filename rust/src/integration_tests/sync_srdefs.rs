use crate::service_resources::load_and_sync_srdefs;
use anyhow::Result;
use sqlx::postgres::PgPoolOptions;

#[tokio::test]
#[ignore]
async fn sync_srdefs() -> Result<()> {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".into());
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    let (registry, result) = load_and_sync_srdefs(&pool).await?;
    println!(
        "synced_srdefs={} inserted={} updated={} errors={}",
        registry.srdefs.len(),
        result.inserted,
        result.updated,
        result.errors.len()
    );
    Ok(())
}
