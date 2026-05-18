//! HTTP-binding for the policy ActorContext.
//!
//! Sits in `src/api/` because envelope (where `PolicyGate` and
//! `ActorResolver` live) is transport-neutral and must not depend on
//! axum/HTTP types. The transport-specific lift from headers happens
//! here, then the canonical `ActorContext` is handed to the gate.

use sem_os_core::types::Classification;
use sem_os_policy::abac::ActorContext;

/// Build an `ActorContext` from request headers using the
/// `x-obpoc-actor-id` / `x-obpoc-roles` / `x-obpoc-department` /
/// `x-obpoc-clearance` / `x-obpoc-jurisdictions` convention.
pub fn actor_from_headers(headers: &axum::http::HeaderMap) -> ActorContext {
    let actor_id = headers
        .get("x-obpoc-actor-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("anonymous")
        .to_string();
    let roles: Vec<String> = headers
        .get("x-obpoc-roles")
        .and_then(|v| v.to_str().ok())
        .map(|r| r.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_else(|| vec!["analyst".into()]);
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
        })
        .or(Some(Classification::Restricted));
    let jurisdictions: Vec<String> = headers
        .get("x-obpoc-jurisdictions")
        .and_then(|v| v.to_str().ok())
        .map(|j| j.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_else(|| vec!["*".into()]);
    ActorContext {
        actor_id,
        roles,
        department,
        clearance,
        jurisdictions,
    }
}
