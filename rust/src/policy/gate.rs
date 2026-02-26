//! PolicyGate — server-side enforcement of single-pipeline invariants.
//!
//! Loaded once at startup from environment variables or config.
//! Every bypass/privilege decision flows through here.

use crate::sem_reg::abac::ActorContext;
use serde::Serialize;

/// Server-side policy configuration. Loaded once at startup.
#[derive(Debug, Clone, Serialize)]
pub struct PolicyGate {
    pub strict_single_pipeline: bool,
    pub allow_raw_execute: bool,
    pub strict_semreg: bool,
    pub allow_legacy_generate: bool,
}

impl PolicyGate {
    pub fn from_env() -> Self {
        Self {
            strict_single_pipeline: env_bool("OBPOC_STRICT_SINGLE_PIPELINE", true),
            allow_raw_execute: env_bool("OBPOC_ALLOW_RAW_EXECUTE", false),
            strict_semreg: env_bool("OBPOC_STRICT_SEMREG", true),
            allow_legacy_generate: env_bool("OBPOC_ALLOW_LEGACY_GENERATE", false),
        }
    }

    pub fn permissive() -> Self {
        Self {
            strict_single_pipeline: false,
            allow_raw_execute: true,
            strict_semreg: false,
            allow_legacy_generate: true,
        }
    }

    pub fn strict() -> Self {
        Self {
            strict_single_pipeline: true,
            allow_raw_execute: false,
            strict_semreg: true,
            allow_legacy_generate: false,
        }
    }

    pub fn can_execute_raw_dsl(&self, actor: &ActorContext) -> bool {
        self.allow_raw_execute && actor.roles.iter().any(|r| r == "operator" || r == "admin")
    }

    pub fn can_use_legacy_generate(&self, actor: &ActorContext) -> bool {
        if self.strict_single_pipeline {
            self.allow_legacy_generate
                && actor.roles.iter().any(|r| r == "operator" || r == "admin")
        } else {
            true
        }
    }

    pub fn semreg_fail_closed(&self) -> bool {
        self.strict_semreg
    }

    pub fn snapshot(&self) -> PolicySnapshot {
        PolicySnapshot {
            strict_single_pipeline: self.strict_single_pipeline,
            allow_raw_execute: self.allow_raw_execute,
            strict_semreg: self.strict_semreg,
            allow_legacy_generate: self.allow_legacy_generate,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PolicySnapshot {
    pub strict_single_pipeline: bool,
    pub allow_raw_execute: bool,
    pub strict_semreg: bool,
    pub allow_legacy_generate: bool,
}

pub struct ActorResolver;

impl ActorResolver {
    #[cfg(feature = "server")]
    pub fn from_headers(headers: &axum::http::HeaderMap) -> ActorContext {
        use crate::sem_reg::types::Classification;
        let actor_id = headers
            .get("x-obpoc-actor-id")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("anonymous")
            .to_string();
        let roles: Vec<String> = headers
            .get("x-obpoc-roles")
            .and_then(|v| v.to_str().ok())
            .map(|r| r.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_else(|| vec!["viewer".into()]);
        let department = headers
            .get("x-obpoc-department")
            .and_then(|v| v.to_str().ok())
            .map(String::from);
        let clearance = headers
            .get("x-obpoc-clearance")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| match s.to_lowercase().as_str() {
                "public" => Some(Classification::Public),
                "internal" => Some(Classification::Internal),
                "confidential" => Some(Classification::Confidential),
                "restricted" => Some(Classification::Restricted),
                _ => None,
            });
        let jurisdictions: Vec<String> = headers
            .get("x-obpoc-jurisdictions")
            .and_then(|v| v.to_str().ok())
            .map(|j| j.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default();
        ActorContext {
            actor_id,
            roles,
            department,
            clearance,
            jurisdictions,
        }
    }

    pub fn from_env() -> ActorContext {
        use crate::sem_reg::types::Classification;
        let actor_id = std::env::var("MCP_ACTOR_ID").unwrap_or_else(|_| "mcp_anonymous".into());
        let roles: Vec<String> = std::env::var("MCP_ROLES")
            .map(|r| r.split(',').map(String::from).collect())
            .unwrap_or_else(|_| vec!["viewer".into()]);
        let department = std::env::var("MCP_DEPARTMENT").ok();
        let clearance =
            std::env::var("MCP_CLEARANCE")
                .ok()
                .and_then(|s| match s.to_lowercase().as_str() {
                    "public" => Some(Classification::Public),
                    "internal" => Some(Classification::Internal),
                    "confidential" => Some(Classification::Confidential),
                    "restricted" => Some(Classification::Restricted),
                    _ => None,
                });
        let jurisdictions: Vec<String> = std::env::var("MCP_JURISDICTIONS")
            .map(|j| j.split(',').map(String::from).collect())
            .unwrap_or_default();
        ActorContext {
            actor_id,
            roles,
            department,
            clearance,
            jurisdictions,
        }
    }

    pub fn from_session_id(session_id: uuid::Uuid) -> ActorContext {
        let roles: Vec<String> = std::env::var("REPL_ROLES")
            .map(|r| r.split(',').map(String::from).collect())
            .unwrap_or_else(|_| vec!["viewer".into()]);
        ActorContext {
            actor_id: session_id.to_string(),
            roles,
            department: None,
            clearance: None,
            jurisdictions: vec![],
        }
    }
}

fn env_bool(key: &str, default: bool) -> bool {
    match std::env::var(key) {
        Ok(v) => matches!(v.to_lowercase().as_str(), "true" | "1" | "yes"),
        Err(_) => default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn viewer() -> ActorContext {
        ActorContext {
            actor_id: "v".into(),
            roles: vec!["viewer".into()],
            department: None,
            clearance: None,
            jurisdictions: vec![],
        }
    }
    fn operator() -> ActorContext {
        ActorContext {
            actor_id: "o".into(),
            roles: vec!["operator".into()],
            department: None,
            clearance: None,
            jurisdictions: vec![],
        }
    }
    fn admin() -> ActorContext {
        ActorContext {
            actor_id: "a".into(),
            roles: vec!["admin".into()],
            department: None,
            clearance: None,
            jurisdictions: vec![],
        }
    }

    #[test]
    fn test_strict_denies_all() {
        let g = PolicyGate::strict();
        assert!(!g.can_execute_raw_dsl(&operator()));
        assert!(!g.can_use_legacy_generate(&operator()));
        assert!(g.semreg_fail_closed());
    }
    #[test]
    fn test_permissive_allows_operator() {
        let g = PolicyGate::permissive();
        assert!(!g.can_execute_raw_dsl(&viewer()));
        assert!(g.can_execute_raw_dsl(&operator()));
    }
    #[test]
    fn test_admin_same_as_operator() {
        let g = PolicyGate::permissive();
        assert!(g.can_execute_raw_dsl(&admin()));
    }
    #[test]
    fn test_strict_with_legacy_flag() {
        let mut g = PolicyGate::strict();
        g.allow_legacy_generate = true;
        assert!(!g.can_use_legacy_generate(&viewer()));
        assert!(g.can_use_legacy_generate(&operator()));
    }
    #[test]
    fn test_non_strict_legacy_open() {
        let mut g = PolicyGate::strict();
        g.strict_single_pipeline = false;
        assert!(g.can_use_legacy_generate(&viewer()));
    }
    #[test]
    fn test_snapshot_serializes() {
        let s = PolicyGate::strict().snapshot();
        let j = serde_json::to_string(&s).unwrap();
        assert!(j.contains("strict_single_pipeline"));
    }
    #[test]
    fn test_session_defaults_viewer() {
        let a = ActorResolver::from_session_id(uuid::Uuid::nil());
        assert_eq!(a.roles, vec!["viewer"]);
    }
}
