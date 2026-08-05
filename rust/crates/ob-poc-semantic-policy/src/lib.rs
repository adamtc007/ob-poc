#![forbid(unsafe_code)]

//! Application-owned semantic policy loaded through the standalone pack API.

use std::sync::OnceLock;

use sem_os_policy::pack_policy::{
    context_has_attribute, evaluate_capability, identity_namespace_uuid, CapabilityDecision,
    PackPolicyError, PrincipalContext,
};
use sem_os_types::AgentMode;
use semantic_pack::{
    admit_pack, CapabilityId, InMemoryPackRegistry, PackBytes, PackRegistry, PolicyAttributeId,
    PolicyContextId, SemanticSnapshot,
};
use thiserror::Error;
use uuid::Uuid;

const POLICY_SOURCE_NAME: &str = "rust/config/semantic-packs/platform-policy.yaml";
const POLICY_SOURCE: &str = include_str!("../../../config/semantic-packs/platform-policy.yaml");

static SNAPSHOT: OnceLock<Result<SemanticSnapshot, String>> = OnceLock::new();

/// Failure to load or evaluate the application semantic policy.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ObPocSemanticPolicyError {
    /// The embedded application pack did not pass the public admission pipeline.
    #[error("invalid embedded ob-poc semantic policy: {0}")]
    InvalidEmbeddedPack(String),
    /// A typed policy operation failed against the admitted snapshot.
    #[error(transparent)]
    Policy(#[from] PackPolicyError),
    /// A stable identifier in application composition code was malformed.
    #[error("invalid application policy identifier: {0}")]
    InvalidIdentifier(String),
}

fn load_snapshot() -> Result<SemanticSnapshot, String> {
    let pack = admit_pack(PackBytes::new(POLICY_SOURCE_NAME, POLICY_SOURCE))
        .map_err(|error| error.to_string())?;
    InMemoryPackRegistry::new()
        .install(pack)
        .map_err(|error| error.to_string())
}

/// Borrow the immutable application policy snapshot.
///
/// # Examples
///
/// ```
/// let snapshot = ob_poc_semantic_policy::snapshot().expect("embedded policy is valid");
/// assert_eq!(snapshot.pack().identity().pack_id.as_str(), "ob-poc.platform-policy");
/// ```
pub fn snapshot() -> Result<&'static SemanticSnapshot, ObPocSemanticPolicyError> {
    match SNAPSHOT.get_or_init(load_snapshot) {
        Ok(snapshot) => Ok(snapshot),
        Err(error) => Err(ObPocSemanticPolicyError::InvalidEmbeddedPack(error.clone())),
    }
}

/// Clone the immutable snapshot for service composition.
///
/// # Examples
///
/// ```
/// let first = ob_poc_semantic_policy::snapshot_owned().unwrap();
/// let second = ob_poc_semantic_policy::snapshot_owned().unwrap();
/// assert_eq!(first.pack().receipt(), second.pack().receipt());
/// ```
pub fn snapshot_owned() -> Result<SemanticSnapshot, ObPocSemanticPolicyError> {
    snapshot().cloned()
}

fn context_id(value: String) -> Result<PolicyContextId, ObPocSemanticPolicyError> {
    PolicyContextId::new(value)
        .map_err(|error| ObPocSemanticPolicyError::InvalidIdentifier(error.to_string()))
}

fn capability_id(value: &str) -> Result<CapabilityId, ObPocSemanticPolicyError> {
    CapabilityId::new(value)
        .map_err(|error| ObPocSemanticPolicyError::InvalidIdentifier(error.to_string()))
}

/// Evaluate a capability in a required application policy context.
///
/// # Examples
///
/// ```
/// use sem_os_policy::pack_policy::PrincipalContext;
/// let decision = ob_poc_semantic_policy::evaluate_context(
///     "mode.governed",
///     &PrincipalContext::default(),
///     "authoring.publish",
/// ).unwrap();
/// assert!(decision.allowed);
/// ```
pub fn evaluate_context(
    context: &str,
    principal: &PrincipalContext,
    capability: &str,
) -> Result<CapabilityDecision, ObPocSemanticPolicyError> {
    Ok(evaluate_capability(
        snapshot()?,
        principal,
        &context_id(context.to_owned())?,
        &capability_id(capability)?,
    )?)
}

/// Test whether the active application pack declares a policy context.
pub fn policy_context_exists(context: &str) -> Result<bool, ObPocSemanticPolicyError> {
    let context = context_id(context.to_owned())?;
    Ok(snapshot()?
        .pack()
        .policy()
        .eligibility
        .iter()
        .any(|policy| policy.context == context))
}

/// Evaluate a capability under the policy context for an agent mode.
///
/// # Examples
///
/// ```
/// use sem_os_types::AgentMode;
/// assert!(!ob_poc_semantic_policy::evaluate_mode(
///     AgentMode::Research,
///     "authoring.publish",
/// ).unwrap().allowed);
/// ```
pub fn evaluate_mode(
    mode: AgentMode,
    capability: &str,
) -> Result<CapabilityDecision, ObPocSemanticPolicyError> {
    evaluate_context(
        &format!("mode.{mode}"),
        &PrincipalContext::default(),
        capability,
    )
}

/// Evaluate an optional workflow context derived from its stable focus ID.
///
/// An undeclared workflow preserves the previous behavior of applying no
/// workflow-specific narrowing.
pub fn evaluate_workflow(
    stage_focus: &str,
    capability: &str,
) -> Result<Option<CapabilityDecision>, ObPocSemanticPolicyError> {
    match evaluate_context(
        &format!("workflow.{stage_focus}"),
        &PrincipalContext::default(),
        capability,
    ) {
        Ok(decision) => Ok(Some(decision)),
        Err(ObPocSemanticPolicyError::Policy(PackPolicyError::MissingContext(_))) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Test a typed feature attribute declared on an agent-mode context.
pub fn mode_has_attribute(
    mode: AgentMode,
    attribute: &str,
) -> Result<bool, ObPocSemanticPolicyError> {
    policy_context_has_attribute(&format!("mode.{mode}"), attribute)
}

/// Test a typed feature attribute declared on any required policy context.
pub fn policy_context_has_attribute(
    context: &str,
    attribute: &str,
) -> Result<bool, ObPocSemanticPolicyError> {
    let attribute = PolicyAttributeId::new(attribute)
        .map_err(|error| ObPocSemanticPolicyError::InvalidIdentifier(error.to_string()))?;
    Ok(context_has_attribute(
        snapshot()?,
        &context_id(context.to_owned())?,
        &attribute,
    )?)
}

/// Read the preserved v1 deterministic identity namespace.
pub fn identity_namespace() -> Result<Uuid, ObPocSemanticPolicyError> {
    Ok(identity_namespace_uuid(snapshot()?)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_identity_namespace_is_preserved_byte_for_byte() {
        assert_eq!(
            identity_namespace().unwrap(),
            Uuid::from_bytes([
                0x7a, 0x3b, 0x9f, 0x42, 0xe1, 0xd4, 0x5a, 0x8b, 0x91, 0x0c, 0x4f, 0x2d, 0x6e, 0x8a,
                0x1b, 0x3c,
            ])
        );
    }

    #[test]
    fn mode_policy_preserves_authoring_and_business_boundaries() {
        assert!(
            evaluate_mode(AgentMode::Research, "authoring.propose")
                .unwrap()
                .allowed
        );
        assert!(
            !evaluate_mode(AgentMode::Research, "authoring.publish")
                .unwrap()
                .allowed
        );
        assert!(
            !evaluate_mode(AgentMode::Governed, "authoring.propose")
                .unwrap()
                .allowed
        );
        assert!(
            evaluate_mode(AgentMode::Governed, "authoring.publish")
                .unwrap()
                .allowed
        );
        assert!(
            !evaluate_mode(AgentMode::Maintenance, "cbu.create")
                .unwrap()
                .allowed
        );
        assert!(
            evaluate_mode(AgentMode::Maintenance, "maintenance.reload")
                .unwrap()
                .allowed
        );
    }

    #[test]
    fn scope_and_workflow_policy_preserve_narrowing() {
        assert!(
            evaluate_context(
                "scope.fail-closed",
                &PrincipalContext::default(),
                "session.help",
            )
            .unwrap()
            .allowed
        );
        assert!(
            !evaluate_context(
                "scope.fail-closed",
                &PrincipalContext::default(),
                "deal.create",
            )
            .unwrap()
            .allowed
        );
        assert!(
            evaluate_workflow("semos-kyc", "screening.run")
                .unwrap()
                .unwrap()
                .allowed
        );
        assert!(
            !evaluate_workflow("semos-kyc", "billing.create")
                .unwrap()
                .unwrap()
                .allowed
        );
        assert!(evaluate_workflow("unconfigured", "billing.create")
            .unwrap()
            .is_none());
    }

    #[test]
    fn feature_and_role_policy_are_pack_owned() {
        assert!(mode_has_attribute(AgentMode::Research, "feature.full-introspection").unwrap());
        assert!(!mode_has_attribute(AgentMode::Governed, "feature.full-introspection").unwrap());
        assert!(
            evaluate_context(
                "mode.governed",
                &PrincipalContext::new(["admin"]),
                "semantic.change.remove",
            )
            .unwrap()
            .allowed
        );
        assert!(
            !evaluate_context(
                "mode.governed",
                &PrincipalContext::new(["steward"]),
                "semantic.change.remove",
            )
            .unwrap()
            .allowed
        );
    }
}
