//! Transaction management for taxonomy operations
//!
//! Provides transaction support with rollback capabilities for complex
//! multi-step taxonomy operations.

use anyhow::Result;
use sqlx::{PgPool, Postgres, Transaction};

/// Transaction wrapper for taxonomy operations
pub struct TaxonomyTransaction<'a> {
    tx: Transaction<'a, Postgres>,
}

impl<'a> TaxonomyTransaction<'a> {
    /// Begin a new transaction
    pub async fn begin(pool: &'a PgPool) -> Result<Self> {
        let tx = pool.begin().await?;
        Ok(Self { tx })
    }

    /// Get a reference to the underlying transaction
    pub fn as_mut(&mut self) -> &mut Transaction<'a, Postgres> {
        &mut self.tx
    }

    /// Commit the transaction
    pub async fn commit(self) -> Result<()> {
        self.tx.commit().await?;
        Ok(())
    }

    /// Rollback the transaction
    pub async fn rollback(self) -> Result<()> {
        self.tx.rollback().await?;
        Ok(())
    }
}

/// Helper function to execute operations with automatic rollback on error
pub async fn with_transaction<F, T>(pool: &PgPool, f: F) -> Result<T>
where
    F: for<'a> FnOnce(
        &'a mut Transaction<'_, Postgres>,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<T>> + Send + 'a>,
    >,
{
    let mut tx = pool.begin().await?;

    match f(&mut tx).await {
        Ok(result) => {
            tx.commit().await?;
            Ok(result)
        }
        Err(e) => {
            tx.rollback().await?;
            Err(e)
        }
    }
}
