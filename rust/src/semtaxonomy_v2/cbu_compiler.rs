//! Minimal compiler-backed CBU path for the first NLCI cutover slice.

use anyhow::{anyhow, Result};
use std::sync::Arc;

use super::compiler::{
    CandidateSelector, CompositionBinder, Discriminator, IntentCompiler, OperationResolver,
    SurfaceObjectResolver,
};
use super::phases::{
    BindingResolutionInput, BindingResolutionOutput, CandidateSelectionInput,
    CandidateSelectionOutput, CompositionInput, CompositionOutput, DiscriminationInput,
    DiscriminationOutput, OperationResolutionInput, OperationResolutionOutput,
    SurfaceObjectResolutionInput, SurfaceObjectResolutionOutput,
};
use super::{CompilerCandidate, CompilerInputEnvelope, CompilerSelection, SemanticStep};

/// Build the minimal CBU compiler used for the first real cutover path.
///
/// # Examples
///
/// ```rust
/// let compiler = ob_poc::semtaxonomy_v2::build_minimal_cbu_compiler();
/// let _ = compiler;
/// ```
pub fn build_minimal_cbu_compiler() -> Arc<dyn IntentCompiler> {
    Arc::new(super::compiler::CompilerPipeline {
        surface_object_resolver: Arc::new(CbuSurfaceResolver),
        operation_resolver: Arc::new(CbuOperationResolver),
        binding_resolver: Arc::new(CbuBindingResolver),
        candidate_selector: Arc::new(CbuCandidateSelector),
        discriminator: Arc::new(CbuDiscriminator),
        composition_binder: Arc::new(CbuCompositionBinder),
    })
}

/// True when the compiler-backed CBU slice supports this legacy intent.
///
/// # Examples
///
/// ```rust
/// use ob_poc::semtaxonomy_v2::{
///     supports_cbu_compiler_slice, BindingMode, CompilerInputEnvelope, IntentStep, SemanticIr,
///     SemanticStep, StructuredIntentPlan,
/// };
///
/// let input = CompilerInputEnvelope {
///     structured_intent: StructuredIntentPlan {
///         steps: vec![IntentStep {
///             action: "read".to_string(),
///             entity: "cbu".to_string(),
///             target: None,
///             qualifiers: vec![],
///             parameters: vec![],
///             confidence: "high".to_string(),
///         }],
///         composition: Some("single_step".to_string()),
///         data_flow: vec![],
///     },
///     semantic_ir: SemanticIr {
///         steps: vec![SemanticStep {
///             action: "read".to_string(),
///             entity: "cbu".to_string(),
///             binding_mode: BindingMode::SessionReference,
///             target: None,
///             parameters: vec![],
///             qualifiers: vec![],
///         }],
///         composition: Some("single_step".to_string()),
///     },
///     session_id: None,
///     session_entity_id: None,
///     session_entity_kind: Some("cbu".to_string()),
///     session_entity_name: Some("Current CBU".to_string()),
/// };
///
/// assert!(supports_cbu_compiler_slice(&input));
/// ```
pub fn supports_cbu_compiler_slice(input: &CompilerInputEnvelope) -> bool {
    matches!(
        input.semantic_ir.steps.first(),
        Some(step)
            if step.entity == "cbu"
                && matches!(step.action.as_str(), "create" | "read" | "update")
    )
}

struct CbuSurfaceResolver;
struct CbuOperationResolver;
struct CbuBindingResolver;
struct CbuCandidateSelector;
struct CbuDiscriminator;
struct CbuCompositionBinder;

impl SurfaceObjectResolver for CbuSurfaceResolver {
    fn resolve_surface(
        &self,
        input: SurfaceObjectResolutionInput,
    ) -> Result<SurfaceObjectResolutionOutput> {
        let step = first_step(&input.envelope)?;
        if step.entity != "cbu" {
            return Err(anyhow!("CBU compiler only supports entity 'cbu'"));
        }

        Ok(SurfaceObjectResolutionOutput {
            semantic_ir: input.envelope.semantic_ir,
            resolved_surface_entity: "cbu".to_string(),
        })
    }
}

impl OperationResolver for CbuOperationResolver {
    fn resolve_operation(
        &self,
        input: OperationResolutionInput,
    ) -> Result<OperationResolutionOutput> {
        let step = first_step_from_surface(&input.surface)?;
        let verb = resolve_cbu_verb(step)?;
        Ok(OperationResolutionOutput {
            surface: input.surface,
            resolved_operations: vec![verb.to_string()],
        })
    }
}

impl super::compiler::BindingResolver for CbuBindingResolver {
    fn resolve_binding(
        &self,
        input: BindingResolutionInput,
    ) -> Result<BindingResolutionOutput> {
        let step = first_step_from_operation(&input.operation)?;
        let verb = input
            .operation
            .resolved_operations
            .first()
            .cloned()
            .ok_or_else(|| anyhow!("operation resolution returned no CBU verb"))?;
        let mut bindings = vec![];

        match verb.as_str() {
            "cbu.create" => {
                let name = required_parameter(step, "name").map_err(|message| anyhow!(message))?;
                bindings.push(("name".to_string(), name));

                if let Ok(jurisdiction) = optional_parameter_any(step, &["jurisdiction"]) {
                    bindings.push(("jurisdiction".to_string(), jurisdiction));
                }
                if let Ok(fund_entity_id) = optional_parameter_any(step, &["fund-entity-id"]) {
                    bindings.push(("fund-entity-id".to_string(), fund_entity_id));
                }
                if let Ok(client_type) =
                    optional_parameter_any(step, &["client-type", "client_type"])
                {
                    bindings.push(("client-type".to_string(), client_type));
                }
                if let Ok(commercial_client_entity_id) =
                    optional_parameter_any(step, &["commercial-client-entity-id"])
                {
                    bindings.push((
                        "commercial-client-entity-id".to_string(),
                        commercial_client_entity_id,
                    ));
                }
                if let Ok(manco_entity_id) = optional_parameter_any(step, &["manco-entity-id"]) {
                    bindings.push(("manco-entity-id".to_string(), manco_entity_id));
                }
            }
            "cbu.rename" => {
                let cbu_id = resolve_cbu_id(step).map_err(|message| anyhow!(message))?;
                bindings.push(("cbu-id".to_string(), cbu_id));
                let name = required_parameter(step, "name")
                    .map_err(|message| anyhow!(message))?;
                bindings.push(("name".to_string(), name));
            }
            "cbu.set-jurisdiction" => {
                let cbu_id = resolve_cbu_id(step).map_err(|message| anyhow!(message))?;
                bindings.push(("cbu-id".to_string(), cbu_id));
                let jurisdiction = required_parameter(step, "jurisdiction")
                    .map_err(|message| anyhow!(message))?;
                bindings.push(("jurisdiction".to_string(), jurisdiction));
            }
            "cbu.set-client-type" => {
                let cbu_id = resolve_cbu_id(step).map_err(|message| anyhow!(message))?;
                bindings.push(("cbu-id".to_string(), cbu_id));
                let client_type = required_parameter_any(step, &["client-type", "client_type"])
                    .map_err(|message| anyhow!(message))?;
                bindings.push(("client-type".to_string(), client_type));
            }
            "cbu.set-commercial-client" => {
                let cbu_id = resolve_cbu_id(step).map_err(|message| anyhow!(message))?;
                bindings.push(("cbu-id".to_string(), cbu_id));
                let commercial_client_entity_id =
                    required_parameter_any(step, &["commercial-client-entity-id"])
                        .map_err(|message| anyhow!(message))?;
                bindings.push((
                    "commercial-client-entity-id".to_string(),
                    commercial_client_entity_id,
                ));
            }
            "cbu.set-category" => {
                let cbu_id = resolve_cbu_id(step).map_err(|message| anyhow!(message))?;
                bindings.push(("cbu-id".to_string(), cbu_id));
                let category =
                    required_parameter_any(step, &["category", "cbu-category"])
                        .map_err(|message| anyhow!(message))?;
                bindings.push(("category".to_string(), category));
            }
            "cbu.submit-for-validation"
            | "cbu.request-proof-update"
            | "cbu.reopen-validation" => {
                let cbu_id = resolve_cbu_id(step).map_err(|message| anyhow!(message))?;
                bindings.push(("cbu-id".to_string(), cbu_id));
            }
            "cbu.list" => {
                if let Ok(status) = optional_parameter_any(step, &["status"]) {
                    bindings.push(("status".to_string(), status));
                }
                if let Ok(client_type) =
                    optional_parameter_any(step, &["client-type", "client_type"])
                {
                    bindings.push(("client-type".to_string(), client_type));
                }
                if let Ok(jurisdiction) = optional_parameter_any(step, &["jurisdiction"]) {
                    bindings.push(("jurisdiction".to_string(), jurisdiction));
                }
            }
            "cbu.read" => {
                let cbu_id = resolve_cbu_id(step).map_err(|message| anyhow!(message))?;
                bindings.push(("cbu-id".to_string(), cbu_id));
            }
            _ => return Err(anyhow!("unsupported CBU verb {verb}")),
        }

        Ok(BindingResolutionOutput {
            operation: input.operation,
            resolved_bindings: bindings,
        })
    }
}

impl CandidateSelector for CbuCandidateSelector {
    fn select_candidates(
        &self,
        input: CandidateSelectionInput,
    ) -> Result<CandidateSelectionOutput> {
        let verb = input
            .binding
            .operation
            .resolved_operations
            .first()
            .cloned()
            .ok_or_else(|| anyhow!("binding resolution returned no operation"))?;
        Ok(CandidateSelectionOutput {
            binding: input.binding,
            candidates: vec![CompilerCandidate {
                verb_id: verb,
                score: 1.0,
                rationale: "supported CBU compiler slice".to_string(),
            }],
        })
    }
}

impl Discriminator for CbuDiscriminator {
    fn discriminate(&self, input: DiscriminationInput) -> Result<DiscriminationOutput> {
        let candidate = input.candidates.candidates.first().cloned();
        Ok(DiscriminationOutput {
            candidates: input.candidates,
            selected_candidate: candidate,
            failure: None,
        })
    }
}

impl CompositionBinder for CbuCompositionBinder {
    fn compose(&self, input: CompositionInput) -> Result<CompositionOutput> {
        if let Some(failure) = input.discrimination.failure.clone() {
            return Ok(CompositionOutput {
                candidates: input.discrimination.candidates.candidates.clone(),
                selection: None,
                failure: Some(failure),
            });
        }

        let candidate = input
            .discrimination
            .selected_candidate
            .clone()
            .ok_or_else(|| anyhow!("discrimination returned no selected candidate"))?;
        let requires_confirmation = candidate.verb_id != "cbu.read";

        Ok(CompositionOutput {
            candidates: input.discrimination.candidates.candidates.clone(),
            selection: Some(CompilerSelection {
                verb_id: candidate.verb_id,
                arguments: input
                    .discrimination
                    .candidates
                    .binding
                    .resolved_bindings
                    .clone(),
                requires_confirmation,
                explanation: "compiler-backed CBU selection".to_string(),
            }),
            failure: None,
        })
    }
}

fn first_step(input: &CompilerInputEnvelope) -> Result<&SemanticStep> {
    input
        .semantic_ir
        .steps
        .first()
        .ok_or_else(|| anyhow!("semantic IR contains no steps"))
}

fn first_step_from_surface(surface: &SurfaceObjectResolutionOutput) -> Result<&SemanticStep> {
    surface
        .semantic_ir
        .steps
        .first()
        .ok_or_else(|| anyhow!("surface resolution output contains no semantic step"))
}

fn first_step_from_operation(operation: &OperationResolutionOutput) -> Result<&SemanticStep> {
    operation
        .surface
        .semantic_ir
        .steps
        .first()
        .ok_or_else(|| anyhow!("operation resolution output contains no semantic step"))
}

fn resolve_cbu_verb(step: &SemanticStep) -> Result<&'static str> {
    match step.action.as_str() {
        "create" => Ok("cbu.create"),
        "read" => {
            if has_parameter(step, "status")
                || has_parameter(step, "client-type")
                || has_parameter(step, "client_type")
                || has_parameter(step, "jurisdiction")
                || (!has_bound_identifier(step)
                    && qualifier_mentions_any(
                        step,
                        &[
                            "list",
                            "show all",
                            "show me",
                            "what cbus exist",
                            "what cbus do we have",
                            "cbus",
                        ],
                    ))
            {
                Ok("cbu.list")
            } else {
                Ok("cbu.read")
            }
        }
        "update" => {
            if has_parameter(step, "name") {
                Ok("cbu.rename")
            } else if has_parameter(step, "jurisdiction") {
                Ok("cbu.set-jurisdiction")
            } else if has_parameter(step, "client-type") || has_parameter(step, "client_type") {
                Ok("cbu.set-client-type")
            } else if has_parameter(step, "commercial-client-entity-id") {
                Ok("cbu.set-commercial-client")
            } else if has_parameter(step, "category") || has_parameter(step, "cbu-category") {
                Ok("cbu.set-category")
            } else if qualifier_mentions_any(
                step,
                &[
                    "submit",
                    "start validation",
                    "begin validation review",
                    "send cbu to review",
                    "validation pending",
                    "move to validation pending",
                ],
            ) {
                Ok("cbu.submit-for-validation")
            } else if qualifier_mentions_any(
                step,
                &[
                    "proof update",
                    "update pending proof",
                    "additional proof",
                    "revalidation proof",
                ],
            ) {
                Ok("cbu.request-proof-update")
            } else if qualifier_mentions_any(
                step,
                &[
                    "reopen validation",
                    "retry validation",
                    "restart validation review",
                    "resubmit after failure",
                    "failed cbu back to validation",
                ],
            ) {
                Ok("cbu.reopen-validation")
            } else {
                Err(anyhow!(
                    "CBU compiler only supports update intents for name, jurisdiction, client type, commercial client, category, submit-for-validation, request-proof-update, or reopen-validation"
                ))
            }
        }
        other => Err(anyhow!(
            "CBU compiler only supports create/read/update actions, got {other}"
        )),
    }
}

fn resolve_cbu_id(step: &SemanticStep) -> std::result::Result<String, String> {
    if let Some(target) = &step.target {
        if let Some(identifier) = &target.identifier {
            return Ok(identifier.clone());
        }
    }
    Err("CBU compiler requires a cbu-id or session-grounded current CBU".to_string())
}

fn has_parameter(step: &SemanticStep, name: &str) -> bool {
    step.parameters.iter().any(|parameter| parameter.name == name)
}

fn has_bound_identifier(step: &SemanticStep) -> bool {
    step.target
        .as_ref()
        .and_then(|target| target.identifier.as_ref())
        .is_some()
}

fn required_parameter(step: &SemanticStep, name: &str) -> std::result::Result<String, String> {
    step.parameters
        .iter()
        .find(|parameter| parameter.name == name)
        .map(|parameter| parameter.value.clone())
        .ok_or_else(|| format!("CBU compiler requires parameter '{name}'"))
}

fn required_parameter_any(
    step: &SemanticStep,
    names: &[&str],
) -> std::result::Result<String, String> {
    step.parameters
        .iter()
        .find(|parameter| names.iter().any(|name| parameter.name == *name))
        .map(|parameter| parameter.value.clone())
        .ok_or_else(|| format!("CBU compiler requires one of parameters {:?}", names))
}

fn optional_parameter_any(
    step: &SemanticStep,
    names: &[&str],
) -> std::result::Result<String, String> {
    step.parameters
        .iter()
        .find(|parameter| names.iter().any(|name| parameter.name == *name))
        .map(|parameter| parameter.value.clone())
        .ok_or_else(|| format!("CBU compiler optional parameter missing {:?}", names))
}

fn qualifier_mentions_any(step: &SemanticStep, needles: &[&str]) -> bool {
    step.qualifiers.iter().any(|(_, value)| {
        let haystack = value.to_ascii_lowercase();
        needles
            .iter()
            .any(|needle| haystack.contains(&needle.to_ascii_lowercase()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semtaxonomy_v2::{
        BindingMode, CompilerInputEnvelope, IntentParameter, IntentStep, SemanticIr, SemanticStep,
        SemanticTarget, StructuredIntentPlan,
    };

    fn make_input(
        action: &str,
        params: Vec<(&str, &str)>,
        session_entity_id: Option<&str>,
    ) -> CompilerInputEnvelope {
        CompilerInputEnvelope {
            structured_intent: StructuredIntentPlan {
                steps: vec![IntentStep {
                    action: action.to_string(),
                    entity: "cbu".to_string(),
                    target: Some(super::super::IntentTarget {
                        identifier: None,
                        reference: Some("current".to_string()),
                        filter: None,
                    }),
                    qualifiers: vec![],
                    parameters: params
                        .iter()
                        .map(|(name, value)| IntentParameter {
                            name: (*name).to_string(),
                            value: (*value).to_string(),
                        })
                        .collect(),
                    confidence: "high".to_string(),
                }],
                composition: Some("single_step".to_string()),
                data_flow: vec![],
            },
            semantic_ir: SemanticIr {
                steps: vec![SemanticStep {
                    action: action.to_string(),
                    entity: "cbu".to_string(),
                    binding_mode: BindingMode::SessionReference,
                    target: Some(SemanticTarget {
                        subject_kind: "cbu".to_string(),
                        identifier: session_entity_id.map(str::to_string),
                        identifier_type: session_entity_id.map(|_| "uuid".to_string()),
                        reference: Some("current".to_string()),
                        filter: None,
                    }),
                    parameters: params
                        .iter()
                        .map(|(name, value)| IntentParameter {
                            name: (*name).to_string(),
                            value: (*value).to_string(),
                        })
                        .collect(),
                    qualifiers: vec![],
                }],
                composition: Some("single_step".to_string()),
            },
            session_id: None,
            session_entity_id: session_entity_id.map(str::to_string),
            session_entity_kind: Some("cbu".to_string()),
            session_entity_name: Some("Current CBU".to_string()),
        }
    }

    #[test]
    fn compiler_supports_cbu_read_slice() {
        let compiler = build_minimal_cbu_compiler();
        let output = compiler
            .compile(make_input(
                "read",
                vec![],
                Some("123e4567-e89b-12d3-a456-426614174000"),
            ))
            .expect("compile should succeed");
        assert_eq!(
            output.selection.expect("selection should exist").verb_id,
            "cbu.read"
        );
    }

    #[test]
    fn compiler_supports_cbu_list_slice() {
        let compiler = build_minimal_cbu_compiler();
        let mut input = make_input("read", vec![], None);
        input.structured_intent.steps[0].qualifiers = vec![
            super::super::IntentQualifier {
                name: "legacy-summary".to_string(),
                value: "Show me the CBUs".to_string(),
            },
            super::super::IntentQualifier {
                name: "legacy-notes".to_string(),
                value: "show all cbus".to_string(),
            },
        ];
        input.semantic_ir.steps[0].qualifiers = vec![
            ("legacy-summary".to_string(), "Show me the CBUs".to_string()),
            ("legacy-notes".to_string(), "show all cbus".to_string()),
        ];
        let output = compiler.compile(input).expect("compile should succeed");
        let selection = output.selection.expect("selection should exist");
        assert_eq!(selection.verb_id, "cbu.list");
        assert!(selection.arguments.is_empty());
    }

    #[test]
    fn compiler_supports_cbu_list_with_filter_slice() {
        let compiler = build_minimal_cbu_compiler();
        let output = compiler
            .compile(make_input(
                "read",
                vec![("jurisdiction", "LU"), ("client-type", "FUND")],
                None,
            ))
            .expect("compile should succeed");
        let selection = output.selection.expect("selection should exist");
        assert_eq!(selection.verb_id, "cbu.list");
        assert_eq!(
            selection.arguments,
            vec![
                ("jurisdiction".to_string(), "LU".to_string()),
                ("client-type".to_string(), "FUND".to_string()),
            ]
        );
    }

    #[test]
    fn compiler_supports_cbu_create_slice() {
        let compiler = build_minimal_cbu_compiler();
        let output = compiler
            .compile(make_input(
                "create",
                vec![
                    ("name", "Apex Growth Fund"),
                    ("jurisdiction", "LU"),
                    ("client-type", "FUND"),
                ],
                None,
            ))
            .expect("compile should succeed");
        let selection = output.selection.expect("selection should exist");
        assert_eq!(selection.verb_id, "cbu.create");
        assert_eq!(
            selection.arguments,
            vec![
                ("name".to_string(), "Apex Growth Fund".to_string()),
                ("jurisdiction".to_string(), "LU".to_string()),
                ("client-type".to_string(), "FUND".to_string()),
            ]
        );
    }

    #[test]
    fn compiler_supports_cbu_create_with_commercial_client_slice() {
        let compiler = build_minimal_cbu_compiler();
        let output = compiler
            .compile(make_input(
                "create",
                vec![
                    ("name", "Apex Growth Fund"),
                    (
                        "commercial-client-entity-id",
                        "123e4567-e89b-12d3-a456-426614174111",
                    ),
                ],
                None,
            ))
            .expect("compile should succeed");
        let selection = output.selection.expect("selection should exist");
        assert_eq!(selection.verb_id, "cbu.create");
        assert_eq!(
            selection.arguments,
            vec![
                ("name".to_string(), "Apex Growth Fund".to_string()),
                (
                    "commercial-client-entity-id".to_string(),
                    "123e4567-e89b-12d3-a456-426614174111".to_string(),
                ),
            ]
        );
    }

    #[test]
    fn compiler_supports_cbu_create_with_fund_entity_slice() {
        let compiler = build_minimal_cbu_compiler();
        let output = compiler
            .compile(make_input(
                "create",
                vec![
                    ("name", "Apex Growth Fund"),
                    ("fund-entity-id", "123e4567-e89b-12d3-a456-426614174222"),
                ],
                None,
            ))
            .expect("compile should succeed");
        let selection = output.selection.expect("selection should exist");
        assert_eq!(selection.verb_id, "cbu.create");
        assert_eq!(
            selection.arguments,
            vec![
                ("name".to_string(), "Apex Growth Fund".to_string()),
                (
                    "fund-entity-id".to_string(),
                    "123e4567-e89b-12d3-a456-426614174222".to_string(),
                ),
            ]
        );
    }

    #[test]
    fn compiler_supports_cbu_create_with_manco_entity_slice() {
        let compiler = build_minimal_cbu_compiler();
        let output = compiler
            .compile(make_input(
                "create",
                vec![
                    ("name", "Apex Growth Fund"),
                    ("manco-entity-id", "123e4567-e89b-12d3-a456-426614174333"),
                ],
                None,
            ))
            .expect("compile should succeed");
        let selection = output.selection.expect("selection should exist");
        assert_eq!(selection.verb_id, "cbu.create");
        assert_eq!(
            selection.arguments,
            vec![
                ("name".to_string(), "Apex Growth Fund".to_string()),
                (
                    "manco-entity-id".to_string(),
                    "123e4567-e89b-12d3-a456-426614174333".to_string(),
                ),
            ]
        );
    }

    #[test]
    fn compiler_supports_cbu_rename_slice() {
        let compiler = build_minimal_cbu_compiler();
        let output = compiler
            .compile(make_input(
                "update",
                vec![("name", "Apex Growth Fund")],
                Some("123e4567-e89b-12d3-a456-426614174000"),
            ))
            .expect("compile should succeed");
        assert_eq!(
            output.selection.expect("selection should exist").verb_id,
            "cbu.rename"
        );
    }

    #[test]
    fn compiler_supports_cbu_set_jurisdiction_slice() {
        let compiler = build_minimal_cbu_compiler();
        let output = compiler
            .compile(make_input(
                "update",
                vec![("jurisdiction", "LU")],
                Some("123e4567-e89b-12d3-a456-426614174000"),
            ))
            .expect("compile should succeed");
        assert_eq!(
            output.selection.expect("selection should exist").verb_id,
            "cbu.set-jurisdiction"
        );
    }

    #[test]
    fn compiler_supports_cbu_set_client_type_slice() {
        let compiler = build_minimal_cbu_compiler();
        let output = compiler
            .compile(make_input(
                "update",
                vec![("client-type", "FUND")],
                Some("123e4567-e89b-12d3-a456-426614174000"),
            ))
            .expect("compile should succeed");
        assert_eq!(
            output.selection.expect("selection should exist").verb_id,
            "cbu.set-client-type"
        );
    }

    #[test]
    fn compiler_supports_cbu_set_commercial_client_slice() {
        let compiler = build_minimal_cbu_compiler();
        let output = compiler
            .compile(make_input(
                "update",
                vec![(
                    "commercial-client-entity-id",
                    "123e4567-e89b-12d3-a456-426614174111",
                )],
                Some("123e4567-e89b-12d3-a456-426614174000"),
            ))
            .expect("compile should succeed");
        assert_eq!(
            output.selection.expect("selection should exist").verb_id,
            "cbu.set-commercial-client"
        );
    }

    #[test]
    fn compiler_supports_cbu_set_category_slice() {
        let compiler = build_minimal_cbu_compiler();
        let output = compiler
            .compile(make_input(
                "update",
                vec![("category", "FUND_MANDATE")],
                Some("123e4567-e89b-12d3-a456-426614174000"),
            ))
            .expect("compile should succeed");
        assert_eq!(
            output.selection.expect("selection should exist").verb_id,
            "cbu.set-category"
        );
    }

    #[test]
    fn compiler_supports_cbu_submit_for_validation_slice() {
        let compiler = build_minimal_cbu_compiler();
        let mut input = make_input(
            "update",
            vec![],
            Some("123e4567-e89b-12d3-a456-426614174000"),
        );
        input.structured_intent.steps[0].qualifiers = vec![
            super::super::IntentQualifier {
                name: "legacy-summary".to_string(),
                value: "Submit the current CBU for validation".to_string(),
            },
            super::super::IntentQualifier {
                name: "legacy-notes".to_string(),
                value: "move lifecycle into validation review".to_string(),
            },
        ];
        input.semantic_ir.steps[0].qualifiers = vec![
            (
                "legacy-summary".to_string(),
                "Submit the current CBU for validation".to_string(),
            ),
            (
                "legacy-notes".to_string(),
                "move lifecycle into validation review".to_string(),
            ),
        ];
        let output = compiler.compile(input).expect("compile should succeed");
        assert_eq!(
            output.selection.expect("selection should exist").verb_id,
            "cbu.submit-for-validation"
        );
    }

    #[test]
    fn compiler_supports_cbu_request_proof_update_slice() {
        let compiler = build_minimal_cbu_compiler();
        let mut input = make_input(
            "update",
            vec![],
            Some("123e4567-e89b-12d3-a456-426614174000"),
        );
        input.structured_intent.steps[0].qualifiers = vec![
            super::super::IntentQualifier {
                name: "legacy-summary".to_string(),
                value: "Request proof update for the current CBU".to_string(),
            },
            super::super::IntentQualifier {
                name: "legacy-notes".to_string(),
                value: "move to update pending proof".to_string(),
            },
        ];
        input.semantic_ir.steps[0].qualifiers = vec![
            (
                "legacy-summary".to_string(),
                "Request proof update for the current CBU".to_string(),
            ),
            (
                "legacy-notes".to_string(),
                "move to update pending proof".to_string(),
            ),
        ];
        let output = compiler.compile(input).expect("compile should succeed");
        assert_eq!(
            output.selection.expect("selection should exist").verb_id,
            "cbu.request-proof-update"
        );
    }

    #[test]
    fn compiler_supports_cbu_reopen_validation_slice() {
        let compiler = build_minimal_cbu_compiler();
        let mut input = make_input(
            "update",
            vec![],
            Some("123e4567-e89b-12d3-a456-426614174000"),
        );
        input.structured_intent.steps[0].qualifiers = vec![
            super::super::IntentQualifier {
                name: "legacy-summary".to_string(),
                value: "Reopen validation for the current CBU".to_string(),
            },
            super::super::IntentQualifier {
                name: "legacy-notes".to_string(),
                value: "move failed cbu back to validation".to_string(),
            },
        ];
        input.semantic_ir.steps[0].qualifiers = vec![
            (
                "legacy-summary".to_string(),
                "Reopen validation for the current CBU".to_string(),
            ),
            (
                "legacy-notes".to_string(),
                "move failed cbu back to validation".to_string(),
            ),
        ];
        let output = compiler.compile(input).expect("compile should succeed");
        assert_eq!(
            output.selection.expect("selection should exist").verb_id,
            "cbu.reopen-validation"
        );
    }
}
