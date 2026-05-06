use crate::{
    CaseId, ConfigVersion, PackId, PackVersion, PhrasingBundleId, SeedBundleId, StateSnapshotId,
};
use chrono::{DateTime, Utc};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{Error as DeError, MapAccess, Visitor},
    ser::SerializeMap,
};
use std::fmt;

/// A typed, immutable test specification for the Sage Eval Harness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvalCase {
    pub case_id: CaseId,
    pub schema_version: u32,
    pub authored_at: DateTime<Utc>,
    pub authored_by: String,
    pub last_revalidated_at: Option<DateTime<Utc>>,
    pub status: CaseStatus,
    pub configuration_version: ConfigVersion,
    pub state_snapshot_id: StateSnapshotId,
    pub domain_pack_id: PackId,
    pub domain_pack_version: PackVersion,
    pub seed_bundle_id: SeedBundleId,
    pub fixture_strategy: FixtureStrategy,
    pub probe_policy: ProbePolicy,
    pub utterance: String,
    #[serde(default)]
    pub mentions: Vec<MentionRef>,
    #[serde(default)]
    pub embedded_resources: Vec<EmbeddedResourceRef>,
    pub editor_context: EditorContext,
    pub session_persona: AcpPersona,
    pub session_workflow_phase: SageWorkflowPhase,
    #[serde(default)]
    pub preceding_turns: Vec<PrecedingTurn>,
    pub axis_primary: EvalAxis,
    pub axis_secondary: Option<EvalAxis>,
    pub difficulty: Difficulty,
    pub substrate_complexity: SubstrateComplexity,
    pub expected_outcome_class: ExpectedOutcomeClass,
    pub expected_workbook_shape: Option<ExpectedWorkbookShape>,
    pub expected_refusal_reason: Option<RefusalReasonCode>,
    pub expected_disambiguation: Option<DisambiguationExpectation>,
    pub expected_evidence_refs: Option<EvidenceExpectation>,
    #[serde(default)]
    pub forbidden_outputs: Vec<ForbiddenOutput>,
    pub deterministic: bool,
    #[serde(default)]
    pub acceptable_variants: Vec<AcceptableVariant>,
    pub phrasing_bundle_id: Option<PhrasingBundleId>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub notes: String,
}

/// Parse an eval case from the YAML authoring format.
///
/// # Examples
///
/// ```rust
/// use ob_poc_eval_schema::{parse_eval_case_yaml, CaseStatus};
///
/// let yaml = r#"
/// case_id: 01HZK8P9X4YBVT2NRQF5J7M3WC
/// schema_version: 1
/// authored_at: 2026-05-06T14:23:00Z
/// authored_by: adam.cearns
/// last_revalidated_at: null
/// status: active
/// configuration_version: semos-v0.4.2
/// state_snapshot_id: snap-2026-05-06-baseline
/// domain_pack_id: ob-poc-kyc
/// domain_pack_version: 0.1.0
/// seed_bundle_id: seed-kyc-baseline-cbu-portfolio
/// fixture_strategy: ephemeral_fixture
/// probe_policy: stubbed
/// utterance: Promote CBU-12345 to active
/// mentions: []
/// embedded_resources: []
/// editor_context:
///   workspace_root: /repo/ob-poc
///   current_file: null
///   selection: null
/// session_persona: sage_planning
/// session_workflow_phase: planning
/// preceding_turns: []
/// axis_primary: happy_path
/// axis_secondary: null
/// difficulty: standard
/// substrate_complexity: small
/// expected_outcome_class: workbook_emitted
/// expected_workbook_shape: null
/// expected_refusal_reason: null
/// expected_disambiguation: null
/// expected_evidence_refs: null
/// forbidden_outputs: []
/// deterministic: true
/// acceptable_variants: []
/// phrasing_bundle_id: null
/// tags: []
/// notes: ""
/// "#;
///
/// let case = parse_eval_case_yaml(yaml).expect("case should parse");
/// assert_eq!(case.status, CaseStatus::Active);
/// ```
pub fn parse_eval_case_yaml(input: &str) -> Result<EvalCase, serde_yaml::Error> {
    serde_yaml::from_str(input)
}

/// Lifecycle status for an authored eval case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseStatus {
    Active,
    Deprecated,
    NeedsRevalidation,
}

/// Isolation strategy used to execute a case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureStrategy {
    TransactionalRollback,
    EphemeralFixture,
    EphemeralFixtureRequired,
}

/// Probe behavior allowed during a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbePolicy {
    Live,
    Stubbed,
    Recorded,
}

/// Primary and secondary classification axes for eval coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalAxis {
    HappyPath,
    Ambiguity,
    Frontier,
    Evidence,
    Adversarial,
    StaleState,
    Phrasing,
    Boundary,
    MultiTurn,
    WorkflowPhase,
    EntityResolution,
}

/// Human-authored difficulty estimate for a case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Difficulty {
    Trivial,
    Standard,
    Hard,
    Adversarial,
}

/// Starting substrate size for a case fixture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubstrateComplexity {
    Small,
    Medium,
    Large,
}

/// ACP persona used for the Sage session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcpPersona {
    SagePlanning,
    SageExecution,
}

/// Workflow phase exposed to the Sage session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SageWorkflowPhase {
    Planning,
    Review,
    Execution,
}

/// Structured mention supplied with the utterance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MentionRef {
    pub kind: MentionKind,
    pub namespace: String,
    #[serde(rename = "ref")]
    pub reference: String,
}

/// Mention target kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MentionKind {
    Entity,
    Document,
    Evidence,
}

/// Embedded resource reference supplied as context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddedResourceRef {
    pub resource_id: String,
    pub uri: String,
    pub media_type: String,
    pub classification: ClassificationLevel,
}

/// Editor state supplied to the candidate stack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorContext {
    pub workspace_root: String,
    pub current_file: Option<String>,
    pub selection: Option<EditorSelection>,
}

/// Optional editor selection span.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorSelection {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

/// Preceding turn replayed before the evaluated turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrecedingTurn {
    pub turn_index: u32,
    pub utterance: String,
    pub expected_outcome_class: ExpectedOutcomeClass,
}

/// Expected top-level response class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedOutcomeClass {
    WorkbookEmitted,
    Refused,
    ClarificationRequested,
    StaleDetected,
    EscalatedToHuman,
}

/// Expected workbook structure for workbook-emitting cases.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedWorkbookShape {
    pub transition_refs: Vec<TransitionRef>,
    pub evidence_ref_count: CountRange,
    pub evidence_classes: Vec<EvidenceClass>,
    pub governance_requirements: Vec<GovernanceRequirement>,
    pub intended_state_movement: StateMovementSpec,
    pub execution_mode: WorkbookExecutionMode,
}

/// Inclusive count range used by YAML authoring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CountRange {
    pub min: usize,
    pub max: usize,
}

/// Typed transition expectation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionRef {
    pub verb: String,
    pub target: String,
}

/// Evidence class expected in a workbook.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceClass {
    KycAttestation,
    SanctionsClearance,
    CorporateRegistry,
    LeiRecord,
    BeneficialOwnership,
    HumanApproval,
}

/// Governance requirement expected in a workbook.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernanceRequirement {
    PrimaryApprover,
    DualControl,
    SegregationOfDuties,
    EvidenceAttachment,
}

/// Expected state movement described by a workbook.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateMovementSpec {
    pub entity: String,
    pub from: String,
    pub to: String,
}

/// Workbook execution mode expected from Sage planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkbookExecutionMode {
    DryRun,
    ProposedWrite,
    Commit,
}

/// Typed refusal reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalReasonCode {
    OutsideAuthoritySurface,
    MissingEvidence,
    ClassifiedDataRestricted,
    FrontierViolation,
    UnsafeMutation,
}

/// Expected disambiguation behavior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisambiguationExpectation {
    pub alternatives: Vec<DisambiguationAlternative>,
    pub require_user_choice: bool,
}

/// A single expected disambiguation alternative.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisambiguationAlternative {
    pub label: String,
    pub entity_ref: Option<String>,
    pub transition_ref: Option<TransitionRef>,
}

/// Expected evidence references and classes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceExpectation {
    pub required_refs: Vec<String>,
    pub required_classes: Vec<EvidenceClass>,
}

/// Output that must not appear in a candidate response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForbiddenOutput {
    EntityBindingNotInSubstrate(String),
    InventedEvidenceRef(String),
    TransitionOutsideFrontier(TransitionRef),
    DslEmissionOutsideWorkbook,
    DirectMutationAttempt,
    ClassifiedFieldInPrompt(ClassificationLevel),
}

impl Serialize for ForbiddenOutput {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::EntityBindingNotInSubstrate(entity_ref) => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("entity_binding_not_in_substrate", entity_ref)?;
                map.end()
            }
            Self::InventedEvidenceRef(evidence_ref) => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("invented_evidence_ref", evidence_ref)?;
                map.end()
            }
            Self::TransitionOutsideFrontier(transition_ref) => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("transition_outside_frontier", transition_ref)?;
                map.end()
            }
            Self::DslEmissionOutsideWorkbook => {
                serializer.serialize_str("dsl_emission_outside_workbook")
            }
            Self::DirectMutationAttempt => serializer.serialize_str("direct_mutation_attempt"),
            Self::ClassifiedFieldInPrompt(classification) => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("classified_field_in_prompt", classification)?;
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for ForbiddenOutput {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(ForbiddenOutputVisitor)
    }
}

struct ForbiddenOutputVisitor;

impl<'de> Visitor<'de> for ForbiddenOutputVisitor {
    type Value = ForbiddenOutput;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a forbidden output string or single-key map")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        match value {
            "dsl_emission_outside_workbook" => Ok(ForbiddenOutput::DslEmissionOutsideWorkbook),
            "direct_mutation_attempt" => Ok(ForbiddenOutput::DirectMutationAttempt),
            "entity_binding_not_in_substrate" => Err(E::custom(
                "entity_binding_not_in_substrate requires a value",
            )),
            "invented_evidence_ref" => Err(E::custom("invented_evidence_ref requires a value")),
            "transition_outside_frontier" => {
                Err(E::custom("transition_outside_frontier requires a value"))
            }
            "classified_field_in_prompt" => {
                Err(E::custom("classified_field_in_prompt requires a value"))
            }
            unknown => Err(E::unknown_variant(
                unknown,
                &[
                    "entity_binding_not_in_substrate",
                    "invented_evidence_ref",
                    "transition_outside_frontier",
                    "dsl_emission_outside_workbook",
                    "direct_mutation_attempt",
                    "classified_field_in_prompt",
                ],
            )),
        }
    }

    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let Some(key) = access.next_key::<String>()? else {
            return Err(M::Error::custom("forbidden output map cannot be empty"));
        };

        let output = match key.as_str() {
            "entity_binding_not_in_substrate" => {
                ForbiddenOutput::EntityBindingNotInSubstrate(access.next_value()?)
            }
            "invented_evidence_ref" => ForbiddenOutput::InventedEvidenceRef(access.next_value()?),
            "transition_outside_frontier" => {
                ForbiddenOutput::TransitionOutsideFrontier(access.next_value()?)
            }
            "classified_field_in_prompt" => {
                ForbiddenOutput::ClassifiedFieldInPrompt(access.next_value()?)
            }
            "dsl_emission_outside_workbook" => {
                let _: serde::de::IgnoredAny = access.next_value()?;
                ForbiddenOutput::DslEmissionOutsideWorkbook
            }
            "direct_mutation_attempt" => {
                let _: serde::de::IgnoredAny = access.next_value()?;
                ForbiddenOutput::DirectMutationAttempt
            }
            unknown => {
                return Err(M::Error::unknown_variant(
                    unknown,
                    &[
                        "entity_binding_not_in_substrate",
                        "invented_evidence_ref",
                        "transition_outside_frontier",
                        "dsl_emission_outside_workbook",
                        "direct_mutation_attempt",
                        "classified_field_in_prompt",
                    ],
                ));
            }
        };

        if access.next_key::<String>()?.is_some() {
            return Err(M::Error::custom(
                "forbidden output map must contain exactly one entry",
            ));
        }

        Ok(output)
    }
}

/// Data classification level used in case and output expectations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassificationLevel {
    Public,
    Internal,
    Confidential,
    Restricted,
}

/// An acceptable alternative expected outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptableVariant {
    pub variant_id: String,
    pub description: String,
    pub workbook_shape: ExpectedWorkbookShape,
}

#[cfg(test)]
mod tests {
    use super::*;

    const CASE_YAML: &str =
        include_str!("../../ob-poc-eval-fixtures/test_cases/cbu_promote_active.yaml");

    #[test]
    fn fixture_case_deserializes() {
        let case = parse_eval_case_yaml(CASE_YAML).expect("fixture case should deserialize");

        assert_eq!(case.schema_version, 1);
        assert_eq!(case.status, CaseStatus::Active);
        assert_eq!(case.axis_primary, EvalAxis::HappyPath);
        assert_eq!(case.mentions.len(), 1);
    }

    #[test]
    fn fixture_case_round_trips_through_json() {
        let case = parse_eval_case_yaml(CASE_YAML).expect("fixture case should deserialize");
        let json = serde_json::to_string(&case).expect("case should serialize to JSON");
        let reparsed: EvalCase = serde_json::from_str(&json).expect("case should parse from JSON");

        assert_eq!(case, reparsed);
    }
}
