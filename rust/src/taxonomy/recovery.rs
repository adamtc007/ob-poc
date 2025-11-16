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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_retry_success_on_second_attempt() {
        let strategy = RecoveryStrategy::default();
        let mut attempt_count = 0;

        let result = strategy
            .execute_with_retry(|| async {
                attempt_count += 1;
                if attempt_count < 2 {
                    Err(anyhow::anyhow!("Temporary failure"))
                } else {
                    Ok(42)
                }
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempt_count, 2);
    }

    #[tokio::test]
    async fn test_retry_exhaustion() {
        let strategy = RecoveryStrategy {
            max_retries: 2,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
        };

        let result = strategy
            .execute_with_retry(|| async { Err(anyhow::anyhow!("Permanent failure")) })
            .await;

        assert!(result.is_err());
    }

    #[test]
    fn test_delay_calculation() {
        let strategy = RecoveryStrategy {
            max_retries: 5,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(2),
        };

        assert_eq!(strategy.calculate_delay(0), Duration::from_millis(100));
        assert_eq!(strategy.calculate_delay(1), Duration::from_millis(200));
        assert_eq!(strategy.calculate_delay(2), Duration::from_millis(400));
        assert_eq!(strategy.calculate_delay(3), Duration::from_millis(800));
        // Should cap at max_delay
        assert_eq!(strategy.calculate_delay(10), Duration::from_secs(2));
    }
}
