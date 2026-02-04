//! Confirm Token Generation and Validation
//!
//! Provides two-phase commit tokens for DecisionPacket::Proposal.
//! Tokens are cryptographically random and short-lived.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::Rng;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// Default token validity duration (30 seconds)
pub const DEFAULT_TOKEN_TTL_SECS: u64 = 30;

/// Token format: base64(random_bytes || timestamp_secs)
const RANDOM_BYTES_LEN: usize = 16;

/// Errors that can occur during token operations
#[derive(Debug, Error)]
pub enum ConfirmTokenError {
    #[error("Token has expired")]
    Expired,

    #[error("Token format is invalid")]
    InvalidFormat,

    #[error("Token does not match expected value")]
    Mismatch,

    #[error("Token generation failed: {0}")]
    GenerationFailed(String),
}

/// Generate a new confirm token with embedded timestamp.
///
/// The token contains:
/// - 16 random bytes for uniqueness
/// - 8 bytes for Unix timestamp (seconds)
///
/// Returns a URL-safe base64 encoded string.
pub fn generate_confirm_token() -> Result<String, ConfirmTokenError> {
    let mut rng = rand::thread_rng();
    let mut token_bytes = [0u8; RANDOM_BYTES_LEN + 8];

    // Fill random portion
    rng.fill(&mut token_bytes[..RANDOM_BYTES_LEN]);

    // Add timestamp
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| ConfirmTokenError::GenerationFailed(e.to_string()))?
        .as_secs();

    token_bytes[RANDOM_BYTES_LEN..].copy_from_slice(&timestamp.to_le_bytes());

    Ok(URL_SAFE_NO_PAD.encode(token_bytes))
}

/// Validate a confirm token.
///
/// Checks:
/// 1. Token format is valid
/// 2. Token matches expected value
/// 3. Token has not expired (within TTL)
///
/// # Arguments
/// * `token` - The token to validate
/// * `expected` - The expected token value
/// * `ttl` - Optional TTL override (defaults to 30 seconds)
pub fn validate_confirm_token(
    token: &str,
    expected: &str,
    ttl: Option<Duration>,
) -> Result<(), ConfirmTokenError> {
    // Check token matches
    if token != expected {
        return Err(ConfirmTokenError::Mismatch);
    }

    // Decode and check expiry
    let token_bytes = URL_SAFE_NO_PAD
        .decode(token)
        .map_err(|_| ConfirmTokenError::InvalidFormat)?;

    if token_bytes.len() != RANDOM_BYTES_LEN + 8 {
        return Err(ConfirmTokenError::InvalidFormat);
    }

    // Extract timestamp
    let mut timestamp_bytes = [0u8; 8];
    timestamp_bytes.copy_from_slice(&token_bytes[RANDOM_BYTES_LEN..]);
    let token_timestamp = u64::from_le_bytes(timestamp_bytes);

    // Check expiry
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ConfirmTokenError::InvalidFormat)?
        .as_secs();

    let ttl_secs = ttl.map(|d| d.as_secs()).unwrap_or(DEFAULT_TOKEN_TTL_SECS);

    if now.saturating_sub(token_timestamp) > ttl_secs {
        return Err(ConfirmTokenError::Expired);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_generate_token() {
        let token = generate_confirm_token().unwrap();
        assert!(!token.is_empty());
        // Should be URL-safe base64
        assert!(token
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn test_token_uniqueness() {
        let token1 = generate_confirm_token().unwrap();
        let token2 = generate_confirm_token().unwrap();
        assert_ne!(token1, token2);
    }

    #[test]
    fn test_validate_matching_token() {
        let token = generate_confirm_token().unwrap();
        assert!(validate_confirm_token(&token, &token, None).is_ok());
    }

    #[test]
    fn test_validate_mismatched_token() {
        let token1 = generate_confirm_token().unwrap();
        let token2 = generate_confirm_token().unwrap();
        let result = validate_confirm_token(&token1, &token2, None);
        assert!(matches!(result, Err(ConfirmTokenError::Mismatch)));
    }

    #[test]
    fn test_token_expiry() {
        let token = generate_confirm_token().unwrap();
        // Token timestamps have 1-second granularity, so sleep > 1 second
        // and use a TTL of 0 seconds to guarantee expiry
        sleep(Duration::from_secs(2));
        let result = validate_confirm_token(&token, &token, Some(Duration::from_secs(1)));
        assert!(matches!(result, Err(ConfirmTokenError::Expired)));
    }

    #[test]
    fn test_invalid_token_format() {
        let result = validate_confirm_token("invalid", "invalid", None);
        assert!(matches!(result, Err(ConfirmTokenError::InvalidFormat)));
    }
}
