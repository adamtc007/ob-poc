use std::collections::HashMap;

use crate::authoring::agent_mode::AgentMode;
use crate::error::SemOsError;

#[derive(Debug, Clone)]
pub struct Principal {
    pub actor_id: String,
    pub roles: Vec<String>,
    pub claims: HashMap<String, String>,
    pub tenancy: Option<String>,
}

impl Principal {
    /// Construct from validated JWT claims at the server boundary (remote mode).
    /// The server middleware calls this; core logic never reads raw JWT tokens.
    pub fn from_jwt_claims(claims: &JwtClaims) -> Result<Self, SemOsError> {
        let actor_id = claims
            .sub
            .clone()
            .ok_or_else(|| SemOsError::Unauthorized("missing sub claim".into()))?;
        Ok(Self {
            actor_id,
            roles: claims.roles.clone().unwrap_or_default(),
            claims: claims.extra.clone().unwrap_or_default(),
            tenancy: claims.tenancy.clone(),
        })
    }

    /// Construct explicitly for in-process mode.
    /// Caller is responsible for populating roles correctly.
    /// There is no implicit or thread-local identity anywhere in the codebase.
    pub fn in_process(actor_id: impl Into<String>, roles: Vec<String>) -> Self {
        Self {
            actor_id: actor_id.into(),
            roles,
            claims: HashMap::new(),
            tenancy: None,
        }
    }

    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }

    pub fn is_admin(&self) -> bool {
        self.has_role("admin")
    }

    pub fn require_admin(&self) -> Result<(), SemOsError> {
        if self.is_admin() {
            Ok(())
        } else {
            Err(SemOsError::Unauthorized(format!(
                "{} is not an admin",
                self.actor_id
            )))
        }
    }

    /// Read agent mode from JWT claims.
    /// Accepts "research" or "governed" (case-insensitive).
    /// Defaults to `AgentMode::Governed` if claim is missing or unrecognised.
    pub fn agent_mode(&self) -> AgentMode {
        self.claims
            .get("agent_mode")
            .and_then(|v| AgentMode::parse(v))
            .unwrap_or(AgentMode::Governed)
    }
}

/// JWT claims shape expected from the identity provider.
/// Deserialised by the server JWT middleware.
#[derive(Debug, serde::Deserialize)]
pub struct JwtClaims {
    pub sub: Option<String>,
    pub roles: Option<Vec<String>>,
    pub tenancy: Option<String>,
    #[serde(flatten)]
    pub extra: Option<HashMap<String, String>>,
}
