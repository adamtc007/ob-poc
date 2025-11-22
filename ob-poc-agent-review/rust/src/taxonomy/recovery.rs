//! Error recovery and retry logic for taxonomy operations
//!
//! Provides automatic retry with exponential backoff and compensation handlers
//! for rolling back failed operations.

use anyhow::Result;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use tokio::time::sleep;

/// Strategy for retrying failed operations
#[derive(Debug, Clone)]
pub struct RecoveryStrategy {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for RecoveryStrategy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
        }
    }
}

impl RecoveryStrategy {
    /// Execute a function with automatic retry on failure
    pub async fn execute_with_retry<F, Fut, T>(&self, mut f: F) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let mut last_error = None;

        for attempt in 0..=self.max_retries {
            match f().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    if attempt < self.max_retries {
                        let delay = self.calculate_delay(attempt);
                        tracing::warn!(
                            "Attempt {} failed: {}. Retrying in {:?}...",
                            attempt + 1,
                            e,
                            delay
                        );
                        sleep(delay).await;
                        last_error = Some(e);
                    } else {
                        last_error = Some(e);
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            anyhow::anyhow!("Operation failed after {} attempts", self.max_retries)
        }))
    }

    fn calculate_delay(&self, attempt: u32) -> Duration {
        let delay = self.base_delay * 2_u32.pow(attempt);
        delay.min(self.max_delay)
    }
}

/// Handler for compensating failed operations (rollback)
pub struct CompensationHandler {
    compensations: Vec<Pin<Box<dyn Future<Output = Result<()>> + Send>>>,
}

impl CompensationHandler {
    pub fn new() -> Self {
        Self {
            compensations: Vec::new(),
        }
    }

    /// Add a compensation action to be executed on rollback
    pub fn add_compensation<F>(&mut self, compensation: F)
    where
        F: Future<Output = Result<()>> + Send + 'static,
    {
        self.compensations.push(Box::pin(compensation));
    }

    /// Execute all compensation actions in reverse order
    pub async fn rollback(mut self) -> Result<()> {
        let mut errors = Vec::new();

        // Execute compensations in reverse order (LIFO)
        while let Some(compensation) = self.compensations.pop() {
            if let Err(e) = compensation.await {
                errors.push(e);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Rollback failed with {} errors: {:?}",
                errors.len(),
                errors
            ))
        }
    }
}

impl Default for CompensationHandler {
    fn default() -> Self {
        Self::new()
    }
}

