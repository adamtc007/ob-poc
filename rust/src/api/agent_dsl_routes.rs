//! DSL-related route handlers for the agent REST API.
//!
//! Includes parsing, resolution, generation, validation, completion,
//! onboarding, and batch operations.
//!
//! Extracted from `agent_routes.rs` for maintainability. All handler functions
//! referenced by the router are `pub(crate)`.

use crate::api::agent_state::AgentState;
use crate::api::agent_types::{
    BatchAddProductsRequest, BatchAddProductsResponse, BatchProductResult, CompleteRequest,
    CompleteResponse, CompletionItem, DomainInfo, DomainsResponse, EntityCandidateResponse,
    EntityMentionResponse, EvidenceResponse, ExtractEntitiesRequest, ExtractEntitiesResponse,
    GenerateDslRequest, GenerateDslResponse, HealthResponse, MissingArg, OnboardingExecutionResult,
    OnboardingRequest, OnboardingResponse, ParseDiscriminatorsRequest, ParseDiscriminatorsResponse,
    ParseDslRequest, ParseDslResponse, ParsedDiscriminators, PipelineStage, RemainingUnresolvedRef,
    ResolutionStats, ResolveByRefIdRequest, ResolveByRefIdResponse, ResolveRefRequest,
    ResolveRefResponse, UnresolvedRef, ValidateDslRequest, ValidationError, ValidationResult,
    VerbInfo, VocabQuery, VocabResponse,
};
use crate::dsl_v2::{
    compile, parse_program, verb_registry::registry, ExecutionContext,
    ExecutionResult as DslV2Result, SemanticValidator,
};
use crate::session::SessionState;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use sqlx::PgPool;

// ============================================================================
// DSL Parsing & Resolution
// ============================================================================

/// POST /api/dsl/parse - Parse DSL source into AST
///
/// Parses DSL source text and returns the AST with any unresolved EntityRefs.
/// DSL + AST are persisted as a tuple pair. If there are unresolved refs,
/// the UI should prompt the user to resolve them before execution.
///
/// ## Request
/// ```json
/// {
///   "dsl": "(cbu.assign-role :cbu-id \"Apex\" :entity-id \"John Smith\" :role \"DIRECTOR\")",
///   "session_id": "optional-uuid"
/// }
/// ```
///
/// ## Response
/// Returns AST with unresolved_refs array for UI to show resolution popups.
pub(crate) async fn parse_dsl(
    State(state): State<AgentState>,
    Json(req): Json<ParseDslRequest>,
) -> Json<ParseDslResponse> {
    use crate::dsl_v2::ast::find_unresolved_ref_locations;
    use crate::dsl_v2::validation::{Severity, ValidationContext, ValidationRequest};
    use crate::dsl_v2::{enrich_program, runtime_registry};

    // Parse the DSL (raw AST with literals only)
    let raw_program = match parse_program(&req.dsl) {
        Ok(p) => p,
        Err(e) => {
            return Json(ParseDslResponse {
                success: false,
                stage: PipelineStage::Draft,
                dsl_source: req.dsl,
                ast: None,
                unresolved_refs: vec![],
                missing_args: vec![],
                validation_errors: vec![],
                error: Some(format!("Parse error: {}", e)),
            });
        }
    };

    // Enrich: convert string literals to EntityRefs based on YAML verb config
    let registry = runtime_registry();
    let enrichment_result = enrich_program(raw_program, registry);
    let mut program = enrichment_result.program;

    // Auto-resolve EntityRefs with exact matches via EntityGateway
    // This handles cases like :product "CUSTODY" where the value matches exactly
    program = auto_resolve_entity_refs(program).await;

    // Run semantic validation with CSG linter to find missing required args
    let mut missing_args: Vec<MissingArg> = vec![];
    let mut validation_errors: Vec<String> = vec![];

    // Try to initialize validator and run CSG validation
    let validator_result = async {
        let v = SemanticValidator::new(state.pool.clone()).await?;
        v.with_csg_linter().await
    }
    .await;

    if let Ok(mut validator) = validator_result {
        let request = ValidationRequest {
            source: req.dsl.clone(),
            context: ValidationContext::default(),
        };
        if let crate::dsl_v2::validation::ValidationResult::Err(diagnostics) =
            validator.validate(&request).await
        {
            for (idx, diag) in diagnostics.iter().enumerate() {
                if diag.severity != Severity::Error {
                    continue;
                }
                let msg = format!("[{}] {}", diag.code.as_str(), diag.message);
                // Pattern: "[E020] missing required argument 'name' for verb 'cbu.create'"
                if diag.code.as_str() == "E020" {
                    // Parse: "missing required argument 'name' for verb 'cbu.create'"
                    // Find first quoted string (arg name) and last quoted string (verb)
                    let parts: Vec<&str> = diag.message.split('\'').collect();
                    if parts.len() >= 4 {
                        // parts: ["missing required argument ", "name", " for verb ", "cbu.create", ""]
                        let arg_name = parts[1].to_string();
                        let verb = parts[3].to_string();
                        missing_args.push(MissingArg {
                            statement_index: idx,
                            arg_name,
                            verb,
                        });
                        continue;
                    }
                }
                // Other validation errors
                validation_errors.push(msg);
            }
        }
    }

    // Find unresolved EntityRefs (those with resolved_key: None)
    let unresolved_locations = find_unresolved_ref_locations(&program);
    let unresolved_refs: Vec<UnresolvedRef> = unresolved_locations
        .into_iter()
        .map(|loc| UnresolvedRef {
            statement_index: loc.statement_index,
            arg_key: loc.arg_key,
            entity_type: loc.entity_type,
            search_text: loc.search_text,
        })
        .collect();

    // Determine stage based on what's needed
    let stage = if !missing_args.is_empty() {
        PipelineStage::Draft // Still need required args
    } else if !unresolved_refs.is_empty() {
        PipelineStage::Resolving // Have all args, need to resolve refs
    } else if !validation_errors.is_empty() {
        PipelineStage::Draft // Other validation errors
    } else {
        PipelineStage::Resolved // Ready to execute
    };

    // If session_id provided, store AST in in-memory session only.
    // Database persistence happens at execution time when we have a CBU context.
    if let Some(session_id) = req.session_id {
        let mut sessions = state.sessions.write().await;
        if let Some(session) = sessions.get_mut(&session_id) {
            session.context.ast = program.statements.clone();
            if stage == PipelineStage::Resolved {
                session.state = SessionState::ReadyToExecute;
            } else {
                session.state = SessionState::PendingValidation;
            }
        }
    }

    Json(ParseDslResponse {
        success: true,
        stage,
        dsl_source: req.dsl,
        ast: Some(program.statements),
        unresolved_refs,
        missing_args,
        validation_errors,
        error: None,
    })
}

/// Auto-resolve EntityRefs with exact matches (100% confidence) via EntityGateway.
///
/// For each unresolved EntityRef, searches the gateway. If an exact match is found
/// (value matches token exactly, case-insensitive for lookup tables), the resolved_key
/// is set automatically without requiring user interaction.
async fn auto_resolve_entity_refs(
    program: crate::dsl_v2::ast::Program,
) -> crate::dsl_v2::ast::Program {
    use crate::dsl_v2::ast::{
        find_unresolved_ref_locations, Argument, AstNode, Program, Statement, VerbCall,
    };
    use entity_gateway::proto::ob::gateway::v1::{SearchMode, SearchRequest};

    // Find all unresolved refs
    let unresolved = find_unresolved_ref_locations(&program);
    if unresolved.is_empty() {
        return program;
    }

    // Connect to EntityGateway
    let addr = crate::dsl_v2::gateway_resolver::gateway_addr();
    let mut client = match entity_gateway::proto::ob::gateway::v1::entity_gateway_client::EntityGatewayClient::connect(addr.clone()).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Auto-resolve: Failed to connect to EntityGateway: {}", e);
            return program;
        }
    };

    // Build resolution map: (stmt_idx, arg_key) -> resolved_key
    let mut resolutions: std::collections::HashMap<(usize, String), String> =
        std::collections::HashMap::new();

    for loc in &unresolved {
        // Map entity_type to gateway nickname (uppercase)
        let nickname = loc.entity_type.to_uppercase();

        let search_request = SearchRequest {
            nickname: nickname.clone(),
            values: vec![loc.search_text.clone()],
            search_key: None,
            mode: SearchMode::Exact as i32, // Exact mode for precise matching
            limit: Some(1),
            discriminators: std::collections::HashMap::new(),
            tenant_id: None,
            cbu_id: None,
        };

        if let Ok(response) = client.search(search_request).await {
            let inner = response.into_inner();
            if let Some(m) = inner.matches.first() {
                // Check for exact match: search_text matches token (case-insensitive for lookup tables)
                let is_exact = m.token.eq_ignore_ascii_case(&loc.search_text)
                    || m.display
                        .to_uppercase()
                        .contains(&loc.search_text.to_uppercase());

                if is_exact {
                    tracing::debug!(
                        "Auto-resolved: {} '{}' -> '{}'",
                        loc.entity_type,
                        loc.search_text,
                        m.token
                    );
                    resolutions.insert((loc.statement_index, loc.arg_key.clone()), m.token.clone());
                }
            }
        }
    }

    if resolutions.is_empty() {
        return program;
    }

    // Apply resolutions to AST
    let resolved_statements: Vec<Statement> = program
        .statements
        .into_iter()
        .enumerate()
        .map(|(stmt_idx, stmt)| {
            if let Statement::VerbCall(vc) = stmt {
                let resolved_args: Vec<Argument> = vc
                    .arguments
                    .into_iter()
                    .map(|arg| {
                        if let Some(resolved_key) = resolutions.get(&(stmt_idx, arg.key.clone())) {
                            // Update the EntityRef with resolved_key
                            if let AstNode::EntityRef {
                                entity_type,
                                search_column,
                                value,
                                span,
                                ref_id,
                                ..
                            } = arg.value
                            {
                                return Argument {
                                    key: arg.key,
                                    value: AstNode::EntityRef {
                                        entity_type,
                                        search_column,
                                        value,
                                        resolved_key: Some(resolved_key.clone()),
                                        span,
                                        ref_id,
                                        explain: None, // Resolution explain not captured in batch commit
                                    },
                                    span: arg.span,
                                };
                            }
                        }
                        arg
                    })
                    .collect();

                Statement::VerbCall(VerbCall {
                    domain: vc.domain,
                    verb: vc.verb,
                    arguments: resolved_args,
                    binding: vc.binding,
                    span: vc.span,
                })
            } else {
                stmt
            }
        })
        .collect();

    Program {
        statements: resolved_statements,
    }
}

// ============================================================================
// Entity Reference Resolution
// ============================================================================

/// POST /api/dsl/resolve-ref - Update AST triplet with resolved primary key
///
/// Updates an EntityRef's resolved_key in the session AST without changing
/// the DSL source text. The AST is the source of truth; source is rendered from it.
///
/// ## Request
///
/// ```json
/// {
///   "session_id": "uuid",
///   "ref_id": { "statement_index": 2, "arg_key": "entity-id" },
///   "resolved_key": "550e8400-..."
/// }
/// ```
///
/// ## Response
///
/// Returns the refreshed AST with resolution stats and can_execute flag.
pub(crate) async fn resolve_entity_ref(
    State(state): State<AgentState>,
    Json(req): Json<ResolveRefRequest>,
) -> Result<Json<ResolveRefResponse>, StatusCode> {
    use crate::dsl_v2::ast::{count_entity_refs, AstNode, Statement};

    let mut sessions = state.sessions.write().await;
    let session = sessions
        .get_mut(&req.session_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    // Validate statement index
    if req.ref_id.statement_index >= session.context.ast.len() {
        return Ok(Json(ResolveRefResponse {
            success: false,
            dsl_source: None,
            ast: None,
            resolution_stats: ResolutionStats {
                total_refs: 0,
                unresolved_count: 0,
            },
            can_execute: false,
            error: Some(format!(
                "Statement index {} out of range (AST has {} statements)",
                req.ref_id.statement_index,
                session.context.ast.len()
            )),
            code: Some("INVALID_REF_ID".to_string()),
        }));
    }

    // Get the statement and find the argument
    let stmt = &mut session.context.ast[req.ref_id.statement_index];

    let update_result = match stmt {
        Statement::VerbCall(vc) => {
            // Find the argument by key
            let arg = vc
                .arguments
                .iter_mut()
                .find(|a| a.key == req.ref_id.arg_key);

            match arg {
                Some(arg) => {
                    // Check if it's an EntityRef
                    match &arg.value {
                        AstNode::EntityRef {
                            entity_type,
                            search_column,
                            value,
                            resolved_key,
                            span,
                            ref_id,
                            explain,
                        } => {
                            if resolved_key.is_some() {
                                Err((
                                    "EntityRef already has a resolved_key".to_string(),
                                    "ALREADY_RESOLVED".to_string(),
                                ))
                            } else {
                                // Update with resolved key
                                arg.value = AstNode::EntityRef {
                                    entity_type: entity_type.clone(),
                                    search_column: search_column.clone(),
                                    value: value.clone(),
                                    resolved_key: Some(req.resolved_key.clone()),
                                    span: *span,
                                    ref_id: ref_id.clone(),
                                    explain: explain.clone(),
                                };
                                Ok(())
                            }
                        }
                        _ => Err((
                            format!("Argument '{}' is not an EntityRef", req.ref_id.arg_key),
                            "NOT_ENTITY_REF".to_string(),
                        )),
                    }
                }
                None => Err((
                    format!("Argument '{}' not found in statement", req.ref_id.arg_key),
                    "INVALID_REF_ID".to_string(),
                )),
            }
        }
        Statement::Comment(_) => Err((
            "Cannot resolve ref in a comment statement".to_string(),
            "INVALID_REF_ID".to_string(),
        )),
    };

    // Handle update result
    match update_result {
        Ok(()) => {
            // Calculate resolution stats
            let program = session.context.as_program();
            let stats = count_entity_refs(&program);

            let can_execute = stats.is_fully_resolved();

            // Update session state if ready to execute
            if can_execute && !session.context.ast.is_empty() {
                session.state = SessionState::ReadyToExecute;
            }

            // Re-render DSL from updated AST (DSL + AST are a tuple pair)
            let dsl_source = session.context.to_dsl_source();

            Ok(Json(ResolveRefResponse {
                success: true,
                dsl_source: Some(dsl_source),
                ast: Some(session.context.ast.clone()),
                resolution_stats: ResolutionStats {
                    total_refs: stats.total_refs,
                    unresolved_count: stats.unresolved_count,
                },
                can_execute,
                error: None,
                code: None,
            }))
        }
        Err((message, code)) => Ok(Json(ResolveRefResponse {
            success: false,
            dsl_source: None,
            ast: None,
            resolution_stats: ResolutionStats {
                total_refs: 0,
                unresolved_count: 0,
            },
            can_execute: false,
            error: Some(message),
            code: Some(code),
        })),
    }
}

/// POST /api/dsl/resolve-by-ref-id
///
/// Resolve an EntityRef by span-based ref_id with dsl_hash verification.
/// This enables precise targeting of refs in lists and maps.
///
/// ## Request
/// ```json
/// {
///   "session_id": "...",
///   "ref_id": "0:15-30",
///   "resolved_key": "550e8400-e29b-41d4-a716-446655440000",
///   "dsl_hash": "a1b2c3d4e5f67890"
/// }
/// ```
pub(crate) async fn resolve_by_ref_id(
    State(state): State<AgentState>,
    Json(req): Json<ResolveByRefIdRequest>,
) -> Result<Json<ResolveByRefIdResponse>, StatusCode> {
    use crate::dsl_v2::ast::{find_unresolved_ref_locations, Statement};

    let mut sessions = state.sessions.write().await;
    let session = sessions
        .get_mut(&req.session_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    // Step 1: Verify dsl_hash matches current session DSL
    let current_dsl = session.context.to_dsl_source();
    let current_hash = compute_dsl_hash_internal(&current_dsl);

    if current_hash != req.dsl_hash {
        return Ok(Json(ResolveByRefIdResponse {
            success: false,
            dsl: current_dsl,
            dsl_hash: current_hash,
            remaining_unresolved: vec![],
            fully_resolved: false,
            error: Some(
                "DSL has changed since disambiguation was generated. Please refresh.".to_string(),
            ),
        }));
    }

    // Step 2: Parse ref_id format "stmt_idx:start-end"
    let parts: Vec<&str> = req.ref_id.split(':').collect();
    if parts.len() != 2 {
        return Ok(Json(ResolveByRefIdResponse {
            success: false,
            dsl: current_dsl,
            dsl_hash: current_hash,
            remaining_unresolved: vec![],
            fully_resolved: false,
            error: Some(format!(
                "Invalid ref_id format: '{}'. Expected 'stmt_idx:start-end'",
                req.ref_id
            )),
        }));
    }

    let stmt_idx: usize = parts[0].parse().map_err(|_| StatusCode::BAD_REQUEST)?;
    let span_parts: Vec<&str> = parts[1].split('-').collect();
    if span_parts.len() != 2 {
        return Ok(Json(ResolveByRefIdResponse {
            success: false,
            dsl: current_dsl,
            dsl_hash: current_hash,
            remaining_unresolved: vec![],
            fully_resolved: false,
            error: Some(format!("Invalid span format in ref_id: '{}'", req.ref_id)),
        }));
    }

    let span_start: usize = span_parts[0].parse().map_err(|_| StatusCode::BAD_REQUEST)?;
    let span_end: usize = span_parts[1].parse().map_err(|_| StatusCode::BAD_REQUEST)?;

    // Step 3: Find and update the EntityRef by span
    if stmt_idx >= session.context.ast.len() {
        return Ok(Json(ResolveByRefIdResponse {
            success: false,
            dsl: current_dsl,
            dsl_hash: current_hash,
            remaining_unresolved: vec![],
            fully_resolved: false,
            error: Some(format!("Statement index {} out of range", stmt_idx)),
        }));
    }

    let stmt = &mut session.context.ast[stmt_idx];
    let update_result = match stmt {
        Statement::VerbCall(vc) => {
            update_entity_ref_by_span(&mut vc.arguments, span_start, span_end, &req.resolved_key)
        }
        Statement::Comment(_) => Err("Cannot resolve ref in a comment statement".to_string()),
    };

    match update_result {
        Ok(()) => {
            // Step 4: Re-render DSL and compute new hash
            let updated_dsl = session.context.to_dsl_source();
            let new_hash = compute_dsl_hash_internal(&updated_dsl);

            // Step 5: Get remaining unresolved refs
            let program = session.context.as_program();
            let locations = find_unresolved_ref_locations(&program);

            let remaining: Vec<RemainingUnresolvedRef> = locations
                .into_iter()
                .map(|loc| RemainingUnresolvedRef {
                    param_name: loc.arg_key,
                    search_value: loc.search_text,
                    entity_type: loc.entity_type,
                    search_column: loc.search_column,
                    ref_id: loc.ref_id.unwrap_or_default(),
                })
                .collect();

            let fully_resolved = remaining.is_empty();

            // Update session state and run_sheet if ready to execute
            if fully_resolved && !session.context.ast.is_empty() {
                session.state = SessionState::ReadyToExecute;
                // Add resolved DSL to run_sheet so /execute can find it
                session.set_pending_dsl(
                    updated_dsl.clone(),
                    session.context.ast.clone(),
                    None,
                    false,
                );
            }

            Ok(Json(ResolveByRefIdResponse {
                success: true,
                dsl: updated_dsl,
                dsl_hash: new_hash,
                remaining_unresolved: remaining,
                fully_resolved,
                error: None,
            }))
        }
        Err(message) => Ok(Json(ResolveByRefIdResponse {
            success: false,
            dsl: current_dsl,
            dsl_hash: current_hash,
            remaining_unresolved: vec![],
            fully_resolved: false,
            error: Some(message),
        })),
    }
}

/// Recursively update an EntityRef by matching span coordinates
fn update_entity_ref_by_span(
    args: &mut [crate::dsl_v2::ast::Argument],
    span_start: usize,
    span_end: usize,
    resolved_key: &str,
) -> Result<(), String> {
    for arg in args.iter_mut() {
        if update_node_by_span(&mut arg.value, span_start, span_end, resolved_key)? {
            return Ok(());
        }
    }
    Err(format!(
        "No EntityRef found with span {}-{}",
        span_start, span_end
    ))
}

/// Recursively search and update a node by span (handles lists/maps)
fn update_node_by_span(
    node: &mut crate::dsl_v2::ast::AstNode,
    span_start: usize,
    span_end: usize,
    resolved_key: &str,
) -> Result<bool, String> {
    use crate::dsl_v2::ast::AstNode;

    match node {
        AstNode::EntityRef {
            span,
            resolved_key: ref mut existing_key,
            ..
        } => {
            if span.start == span_start && span.end == span_end {
                if existing_key.is_some() {
                    return Err("EntityRef already resolved".to_string());
                }
                *existing_key = Some(resolved_key.to_string());
                return Ok(true);
            }
            Ok(false)
        }
        AstNode::List { items, .. } => {
            for item in items.iter_mut() {
                if update_node_by_span(item, span_start, span_end, resolved_key)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        AstNode::Map { entries, .. } => {
            for (_, value) in entries.iter_mut() {
                if update_node_by_span(value, span_start, span_end, resolved_key)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        _ => Ok(false),
    }
}

/// Compute SHA-256 hash of DSL string (first 16 hex chars)
fn compute_dsl_hash_internal(dsl: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(dsl.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)[..16].to_string()
}

// ============================================================================
// Discriminator Parsing
// ============================================================================

/// POST /api/resolution/parse-discriminators - Parse natural language into discriminators
///
/// Takes user input like "the British one" or "born 1965" and extracts
/// structured discriminators for entity resolution filtering.
///
/// ## Examples
///
/// - "the British one" -> nationality: "GB"
/// - "born 1965" -> dob_year: 1965
/// - "the director" -> role: "DIRECTOR"
/// - "first" or "1" -> selection_index: 0
/// - "UK citizen, director at Acme" -> nationality: "GB", role: "DIRECTOR", associated_entity: "Acme"
pub(crate) async fn parse_discriminators(
    Json(req): Json<ParseDiscriminatorsRequest>,
) -> Json<ParseDiscriminatorsResponse> {
    let input = req.input.trim().to_lowercase();
    let mut discriminators = ParsedDiscriminators::default();
    let mut unrecognized = Vec::new();
    let mut is_selection = false;

    // Check for selection patterns first
    if let Some(idx) = parse_selection_pattern(&input) {
        discriminators.selection_index = Some(idx);
        is_selection = true;
    }

    // Parse nationality patterns
    if let Some(nat) = parse_nationality(&input) {
        discriminators.nationality = Some(nat);
    }

    // Parse date of birth patterns
    if let Some(year) = parse_dob_year(&input) {
        discriminators.dob_year = Some(year);
    }

    // Parse role patterns
    if let Some(role) = parse_role(&input) {
        discriminators.role = Some(role);
    }

    // Parse association patterns ("at X", "works for X", "from X company")
    if let Some(entity) = parse_association(&input) {
        discriminators.associated_entity = Some(entity);
    }

    // Parse jurisdiction patterns
    if let Some(juris) = parse_jurisdiction(&input) {
        discriminators.jurisdiction = Some(juris);
    }

    // Track what wasn't recognized
    // (simplified - in production would do proper tokenization)
    if discriminators.nationality.is_none()
        && discriminators.dob_year.is_none()
        && discriminators.role.is_none()
        && discriminators.associated_entity.is_none()
        && discriminators.jurisdiction.is_none()
        && discriminators.selection_index.is_none()
    {
        unrecognized.push(req.input.clone());
    }

    Json(ParseDiscriminatorsResponse {
        success: true,
        discriminators,
        is_selection,
        input: req.input,
        unrecognized,
    })
}

/// Parse selection patterns: "1", "first", "the first one", "select 2"
fn parse_selection_pattern(input: &str) -> Option<usize> {
    // Direct number
    if let Ok(n) = input.parse::<usize>() {
        if n >= 1 {
            return Some(n - 1);
        }
    }

    // "select N"
    if let Some(rest) = input.strip_prefix("select ") {
        if let Ok(n) = rest.trim().parse::<usize>() {
            if n >= 1 {
                return Some(n - 1);
            }
        }
    }

    // Ordinals
    let ordinals = [
        ("first", 0),
        ("1st", 0),
        ("second", 1),
        ("2nd", 1),
        ("third", 2),
        ("3rd", 2),
        ("fourth", 3),
        ("4th", 3),
        ("fifth", 4),
        ("5th", 4),
    ];

    for (word, idx) in ordinals {
        if input.contains(word) {
            return Some(idx);
        }
    }

    None
}

/// Parse nationality from natural language
fn parse_nationality(input: &str) -> Option<String> {
    // Map of patterns to ISO codes
    let patterns = [
        // Demonyms
        ("british", "GB"),
        ("uk citizen", "GB"),
        ("english", "GB"),
        ("scottish", "GB"),
        ("welsh", "GB"),
        ("american", "US"),
        ("us citizen", "US"),
        ("german", "DE"),
        ("french", "FR"),
        ("italian", "IT"),
        ("spanish", "ES"),
        ("dutch", "NL"),
        ("belgian", "BE"),
        ("swiss", "CH"),
        ("austrian", "AT"),
        ("irish", "IE"),
        ("luxembourgish", "LU"),
        ("luxembourg", "LU"),
        ("canadian", "CA"),
        ("australian", "AU"),
        ("japanese", "JP"),
        ("chinese", "CN"),
        ("indian", "IN"),
        ("brazilian", "BR"),
        ("mexican", "MX"),
        ("swedish", "SE"),
        ("norwegian", "NO"),
        ("danish", "DK"),
        ("finnish", "FI"),
        ("polish", "PL"),
        ("portuguese", "PT"),
        ("greek", "GR"),
        ("russian", "RU"),
        // Direct codes
        ("from uk", "GB"),
        ("from us", "US"),
        ("from usa", "US"),
        ("from gb", "GB"),
    ];

    for (pattern, code) in patterns {
        if input.contains(pattern) {
            return Some(code.to_string());
        }
    }

    None
}

/// Parse year of birth
fn parse_dob_year(input: &str) -> Option<i32> {
    // Pattern: "born YYYY" or "dob YYYY" or "birth year YYYY"
    let year_patterns = ["born ", "dob ", "birth year ", "year of birth ", "born in "];

    for prefix in year_patterns {
        if let Some(rest) = input.find(prefix).map(|i| &input[i + prefix.len()..]) {
            // Extract first 4 digits
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if digits.len() == 4 {
                if let Ok(year) = digits.parse::<i32>() {
                    if (1900..=2010).contains(&year) {
                        return Some(year);
                    }
                }
            }
        }
    }

    None
}

/// Parse role from natural language
fn parse_role(input: &str) -> Option<String> {
    let role_patterns = [
        ("director", "DIRECTOR"),
        ("ubo", "UBO"),
        ("beneficial owner", "UBO"),
        ("shareholder", "SHAREHOLDER"),
        ("officer", "OFFICER"),
        ("secretary", "SECRETARY"),
        ("chairman", "CHAIRMAN"),
        ("ceo", "CEO"),
        ("cfo", "CFO"),
        ("manager", "MANAGER"),
        ("partner", "PARTNER"),
        ("trustee", "TRUSTEE"),
        ("signatory", "SIGNATORY"),
        ("authorized", "AUTHORISED_SIGNATORY"),
    ];

    for (pattern, role) in role_patterns {
        if input.contains(pattern) {
            return Some(role.to_string());
        }
    }

    None
}

/// Parse entity association
fn parse_association(input: &str) -> Option<String> {
    // Patterns: "at X", "works for X", "from X company", "employed by X"
    let patterns = [
        " at ",
        " works for ",
        " employed by ",
        " works at ",
        " from ",
    ];

    for pattern in patterns {
        if let Some(idx) = input.find(pattern) {
            let rest = &input[idx + pattern.len()..];
            // Take words until punctuation or end
            let entity: String = rest
                .split([',', '.', ';'])
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if !entity.is_empty() && entity.len() > 2 {
                return Some(entity);
            }
        }
    }

    None
}

/// Parse jurisdiction
fn parse_jurisdiction(input: &str) -> Option<String> {
    let patterns = [
        ("luxembourg", "LU"),
        ("lux ", "LU"),
        ("cayman", "KY"),
        ("jersey", "JE"),
        ("guernsey", "GG"),
        ("ireland", "IE"),
        ("delaware", "US-DE"),
        ("singapore", "SG"),
        ("hong kong", "HK"),
        ("switzerland", "CH"),
        ("liechtenstein", "LI"),
    ];

    for (pattern, code) in patterns {
        if input.contains(pattern) {
            return Some(code.to_string());
        }
    }

    None
}

// ============================================================================
// DSL Generation
// ============================================================================

/// POST /api/agent/generate - Generate DSL from natural language
///
/// LEGACY endpoint — gated by PolicyGate. Use /api/session/:id/chat instead.
pub(crate) async fn generate_dsl(
    State(state): State<AgentState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<GenerateDslRequest>,
) -> Json<GenerateDslResponse> {
    // PolicyGate: check if legacy generate is allowed
    let actor = crate::policy::ActorResolver::from_headers(&headers);
    if !state.policy_gate.can_use_legacy_generate(&actor) {
        return Json(GenerateDslResponse {
            dsl: None,
            explanation: None,
            error: Some("Legacy /generate endpoint is disabled. Use /api/session/:id/chat instead.".into()),
        });
    }

    // Get vocabulary for the prompt
    let vocab = build_vocab_prompt(req.domain.as_deref());

    // Build the system prompt with onboarding context
    let system_prompt = format!(
        r#"You are a DSL generator for a KYC/AML onboarding system.
Generate valid DSL S-expressions from natural language instructions.

AVAILABLE VERBS:
{}

DSL SYNTAX:
- Format: (domain.verb :key "value" :key2 value2)
- Strings must be quoted: "text"
- Numbers are unquoted: 42, 25.5
- References start with @: @symbol_name (use underscores, not hyphens)
- Use :as @name to capture results

## CRITICAL: EXISTING vs NEW CBUs

**EXISTING CBU** - When user references an existing CBU by name (e.g., "onboard Aviva", "add custody to Apex"):
- Use `cbu.add-product` to add a product to the existing CBU
- The CBU name is matched case-insensitively in the database
- DO NOT use `cbu.ensure` - that would create a duplicate!

**NEW CBU** - Only when explicitly creating a new client:
- Use `cbu.ensure` to create the CBU first
- Then use `cbu.add-product` to add products

### EXAMPLE: Adding product to EXISTING CBU
User: "Onboard Aviva to Custody product"
```
(cbu.add-product :cbu-id "Aviva" :product "CUSTODY")
```

User: "Add Fund Accounting to Apex Capital"
```
(cbu.add-product :cbu-id "Apex Capital" :product "FUND_ACCOUNTING")
```

### EXAMPLE: Creating NEW CBU and adding product
User: "Create a new fund called Pacific Growth in Luxembourg and add Custody"
```
(cbu.ensure :name "Pacific Growth" :jurisdiction "LU" :client-type "fund" :as @fund)
(cbu.add-product :cbu-id @fund :product "CUSTODY")
```

## Available Products (use product CODE, not display name)

| Product Code | Description |
|--------------|-------------|
| `CUSTODY` | Asset safekeeping, settlement, corporate actions |
| `FUND_ACCOUNTING` | NAV calculation, investor accounting, reporting |
| `TRANSFER_AGENCY` | Investor registry, subscriptions, redemptions |
| `MIDDLE_OFFICE` | Position management, trade capture, P&L |
| `COLLATERAL_MGMT` | Collateral optimization and margin |
| `MARKETS_FX` | Foreign exchange services |
| `ALTS` | Alternative investment administration |

## Client Types
- `fund` - Investment fund (hedge fund, mutual fund, etc.)
- `corporate` - Corporate client
- `individual` - Individual client
- `trust` - Trust structure

## Common Jurisdictions
- `US` - United States
- `GB` - United Kingdom
- `LU` - Luxembourg
- `IE` - Ireland
- `KY` - Cayman Islands
- `JE` - Jersey

## Other DSL Examples

Create entities:
(entity.create-proper-person :first-name "John" :last-name "Smith" :date-of-birth "1980-01-15" :as @john)
(entity.create-limited-company :name "Holdings Ltd" :jurisdiction "GB" :as @company)

Assign roles (note: @fund must be defined first with cbu.ensure):
(cbu.ensure :name "Acme Fund" :jurisdiction "LU" :client-type "fund" :as @fund)
(cbu.assign-role :cbu-id @fund :entity-id @john :role "DIRECTOR")
(cbu.assign-role :cbu-id @fund :entity-id @company :role "PRINCIPAL")

List CBUs:
(cbu.list)

Respond with ONLY the DSL, no explanation. If you cannot generate valid DSL, respond with: ERROR: <reason>"#,
        vocab
    );

    // Create LLM client (uses AGENT_BACKEND env var to select provider)
    let llm_client = match crate::agentic::create_llm_client() {
        Ok(client) => client,
        Err(e) => {
            return Json(GenerateDslResponse {
                dsl: None,
                explanation: None,
                error: Some(format!("LLM client error: {}", e)),
            });
        }
    };

    // Call LLM API with JSON output format
    let json_system_prompt = format!(
        "{}\n\nIMPORTANT: Always respond with valid JSON in this exact format:\n{{\n  \"dsl\": \"(verb.name :arg value ...)\",\n  \"explanation\": \"Brief explanation of what the DSL does\"\n}}\n\nIf you cannot generate DSL, respond with:\n{{\n  \"dsl\": null,\n  \"explanation\": null,\n  \"error\": \"Error message explaining why\"\n}}",
        system_prompt
    );

    let response = llm_client
        .chat_json(&json_system_prompt, &req.instruction)
        .await;

    match response {
        Ok(content) => {
            match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(structured) => {
                    let dsl = structured["dsl"].as_str().map(|s| s.to_string());
                    let explanation = structured["explanation"].as_str().map(|s| s.to_string());
                    let error = structured["error"].as_str().map(|s| s.to_string());

                    if let Some(err) = error {
                        Json(GenerateDslResponse {
                            dsl: None,
                            explanation,
                            error: Some(err),
                        })
                    } else if let Some(ref dsl_str) = dsl {
                        // Validate the generated DSL
                        match parse_program(dsl_str) {
                            Ok(_) => Json(GenerateDslResponse {
                                dsl,
                                explanation,
                                error: None,
                            }),
                            Err(e) => Json(GenerateDslResponse {
                                dsl,
                                explanation,
                                error: Some(format!("Syntax error: {}", e)),
                            }),
                        }
                    } else {
                        Json(GenerateDslResponse {
                            dsl: None,
                            explanation,
                            error: Some("No DSL in response".to_string()),
                        })
                    }
                }
                Err(e) => Json(GenerateDslResponse {
                    dsl: None,
                    explanation: None,
                    error: Some(format!("Failed to parse structured response: {}", e)),
                }),
            }
        }
        Err(e) => Json(GenerateDslResponse {
            dsl: None,
            explanation: None,
            error: Some(format!("LLM API error: {}", e)),
        }),
    }
}

/// POST /api/agent/generate-with-tools - Generate DSL using Claude tool_use
///
/// This endpoint uses Claude's tool calling feature to look up real database IDs
/// before generating DSL, preventing UUID hallucination.
pub(crate) async fn generate_dsl_with_tools(
    State(state): State<AgentState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<GenerateDslRequest>,
) -> Json<GenerateDslResponse> {
    // PolicyGate: check if legacy generate is allowed
    let actor = crate::policy::ActorResolver::from_headers(&headers);
    if !state.policy_gate.can_use_legacy_generate(&actor) {
        return Json(GenerateDslResponse {
            dsl: None,
            explanation: None,
            error: Some("Legacy /generate-with-tools endpoint is disabled. Use /api/session/:id/chat instead.".into()),
        });
    }

    let api_key = match std::env::var("ANTHROPIC_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            return Json(GenerateDslResponse {
                dsl: None,
                explanation: None,
                error: Some("ANTHROPIC_API_KEY not configured".to_string()),
            });
        }
    };

    let vocab = build_vocab_prompt(req.domain.as_deref());
    let system_prompt = build_tool_use_system_prompt(&vocab);

    // Define tools for Claude
    let tools = serde_json::json!([
        {
            "name": "lookup_cbu",
            "description": "Look up an existing CBU (Client Business Unit) by name. ALWAYS use this before referencing a CBU to get the real ID.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "CBU name to search for (case-insensitive)"
                    }
                },
                "required": ["name"]
            }
        },
        {
            "name": "lookup_entity",
            "description": "Look up an existing entity by name. Use this to find persons or companies.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Entity name to search for"
                    },
                    "entity_type": {
                        "type": "string",
                        "description": "Optional: filter by type (proper_person, limited_company, etc.)"
                    }
                },
                "required": ["name"]
            }
        },
        {
            "name": "lookup_product",
            "description": "Look up available products by name.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Product name to search for"
                    }
                },
                "required": ["name"]
            }
        },
        {
            "name": "list_cbus",
            "description": "List all CBUs in the system. Use this to see what clients exist.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Max results to return (default 10)"
                    }
                }
            }
        }
    ]);

    let client = reqwest::Client::new();

    // First call - may include tool use
    let mut messages = vec![serde_json::json!({"role": "user", "content": req.instruction})];

    let mut tool_results: Vec<String> = Vec::new();
    let max_iterations = 5;

    for iteration in 0..max_iterations {
        let response = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&serde_json::json!({
                "model": "claude-sonnet-4-20250514",
                "max_tokens": 2048,
                "system": system_prompt,
                "tools": tools,
                "messages": messages
            }))
            .send()
            .await;

        let resp = match response {
            Ok(r) => r,
            Err(e) => {
                return Json(GenerateDslResponse {
                    dsl: None,
                    explanation: None,
                    error: Some(format!("Request failed: {}", e)),
                });
            }
        };

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Json(GenerateDslResponse {
                dsl: None,
                explanation: None,
                error: Some(format!("API error {}: {}", status, body)),
            });
        }

        let json: serde_json::Value = match resp.json().await {
            Ok(j) => j,
            Err(e) => {
                return Json(GenerateDslResponse {
                    dsl: None,
                    explanation: None,
                    error: Some(format!("Failed to parse response: {}", e)),
                });
            }
        };

        let stop_reason = json["stop_reason"].as_str().unwrap_or("");

        // Check if Claude wants to use tools
        if stop_reason == "tool_use" {
            let empty_vec = vec![];
            let content = json["content"].as_array().unwrap_or(&empty_vec);
            let mut tool_use_results = Vec::new();

            for block in content {
                if block["type"] == "tool_use" {
                    let tool_name = block["name"].as_str().unwrap_or("");
                    let tool_id = block["id"].as_str().unwrap_or("");
                    let input = &block["input"];

                    // Execute the tool
                    let result = execute_tool(&state.pool, tool_name, input).await;
                    tool_results.push(format!("{}: {}", tool_name, result));

                    tool_use_results.push(serde_json::json!({
                        "type": "tool_result",
                        "tool_use_id": tool_id,
                        "content": result
                    }));
                }
            }

            // Add assistant message with tool use
            messages.push(serde_json::json!({
                "role": "assistant",
                "content": content
            }));

            // Add tool results
            messages.push(serde_json::json!({
                "role": "user",
                "content": tool_use_results
            }));

            tracing::debug!("Tool use iteration {}: {:?}", iteration, tool_results);
            continue;
        }

        // Claude finished - extract the DSL
        // Note: tool_use mode doesn't support structured outputs, so we extract DSL from text
        let empty_vec2 = vec![];
        let content = json["content"].as_array().unwrap_or(&empty_vec2);
        for block in content {
            if block["type"] == "text" {
                let text = block["text"].as_str().unwrap_or("").trim();

                if text.starts_with("ERROR:") {
                    return Json(GenerateDslResponse {
                        dsl: None,
                        explanation: None,
                        error: Some(text.to_string()),
                    });
                }

                // Try to extract DSL from the response (handle markdown fencing, etc.)
                let dsl_text = extract_dsl_from_text(text);

                // Validate the generated DSL
                match parse_program(&dsl_text) {
                    Ok(_) => {
                        let explanation = if tool_results.is_empty() {
                            "DSL generated successfully".to_string()
                        } else {
                            format!("DSL generated with lookups: {}", tool_results.join(", "))
                        };
                        return Json(GenerateDslResponse {
                            dsl: Some(dsl_text),
                            explanation: Some(explanation),
                            error: None,
                        });
                    }
                    Err(e) => {
                        return Json(GenerateDslResponse {
                            dsl: Some(dsl_text),
                            explanation: None,
                            error: Some(format!("Generated DSL has syntax error: {}", e)),
                        });
                    }
                }
            }
        }

        break;
    }

    Json(GenerateDslResponse {
        dsl: None,
        explanation: None,
        error: Some("Failed to generate DSL after max iterations".to_string()),
    })
}

/// Execute a tool call via EntityGateway and return the result as a string
///
/// All lookups go through the central EntityGateway service for consistent
/// fuzzy matching behavior across LSP, validation, MCP tools, and Claude tool_use.
async fn execute_tool(_pool: &PgPool, tool_name: &str, input: &serde_json::Value) -> String {
    // Connect to EntityGateway
    let addr = crate::dsl_v2::gateway_resolver::gateway_addr();
    let mut client = match entity_gateway::proto::ob::gateway::v1::entity_gateway_client::EntityGatewayClient::connect(addr.clone()).await {
        Ok(c) => c,
        Err(e) => return format!("Failed to connect to EntityGateway at {}: {}", addr, e),
    };

    // Helper to search via gateway
    async fn gateway_search(
        client: &mut entity_gateway::proto::ob::gateway::v1::entity_gateway_client::EntityGatewayClient<tonic::transport::Channel>,
        nickname: &str,
        search: &str,
        limit: i32,
    ) -> Result<Vec<(String, String, f32)>, String> {
        use entity_gateway::proto::ob::gateway::v1::{SearchMode, SearchRequest};

        let request = SearchRequest {
            nickname: nickname.to_string(),
            values: vec![search.to_string()],
            search_key: None,
            mode: SearchMode::Fuzzy as i32,
            limit: Some(limit),
            discriminators: std::collections::HashMap::new(),
            tenant_id: None,
            cbu_id: None,
        };

        let response = client
            .search(request)
            .await
            .map_err(|e| format!("EntityGateway search failed: {}", e))?;

        Ok(response
            .into_inner()
            .matches
            .into_iter()
            .map(|m| (m.token, m.display, m.score))
            .collect())
    }

    match tool_name {
        "lookup_cbu" => {
            let name = input["name"].as_str().unwrap_or("");
            match gateway_search(&mut client, "CBU", name, 5).await {
                Ok(matches) if !matches.is_empty() => {
                    let results: Vec<String> = matches
                        .iter()
                        .map(|(id, display, _)| format!("- {} (id: {})", display, id))
                        .collect();
                    format!("Found {} CBU(s):\n{}", matches.len(), results.join("\n"))
                }
                Ok(_) => format!("No CBU found matching '{}'", name),
                Err(e) => e,
            }
        }
        "lookup_entity" => {
            let name = input["name"].as_str().unwrap_or("");
            // Use ENTITY nickname which searches across all entity types
            match gateway_search(&mut client, "ENTITY", name, 5).await {
                Ok(matches) if !matches.is_empty() => {
                    let results: Vec<String> = matches
                        .iter()
                        .map(|(id, display, _)| format!("- {} (id: {})", display, id))
                        .collect();
                    format!("Found {} entity(s):\n{}", matches.len(), results.join("\n"))
                }
                Ok(_) => format!("No entity found matching '{}'", name),
                Err(e) => e,
            }
        }
        "lookup_product" => {
            let name = input["name"].as_str().unwrap_or("");
            match gateway_search(&mut client, "PRODUCT", name, 5).await {
                Ok(matches) if !matches.is_empty() => {
                    let results: Vec<String> = matches
                        .iter()
                        .map(|(id, display, _)| format!("- {} (code: {})", display, id))
                        .collect();
                    format!(
                        "Found {} product(s):\n{}",
                        matches.len(),
                        results.join("\n")
                    )
                }
                Ok(_) => format!("No product found matching '{}'", name),
                Err(e) => e,
            }
        }
        "list_cbus" => {
            let limit = input["limit"].as_i64().unwrap_or(10) as i32;
            // Empty search with high limit to list all
            match gateway_search(&mut client, "CBU", "", limit).await {
                Ok(matches) => {
                    let results: Vec<String> = matches
                        .iter()
                        .map(|(_, display, _)| format!("- {}", display))
                        .collect();
                    format!("CBUs in system:\n{}", results.join("\n"))
                }
                Err(e) => e,
            }
        }
        _ => format!("Unknown tool: {}", tool_name),
    }
}

/// Build system prompt for tool-use generation
fn build_tool_use_system_prompt(vocab: &str) -> String {
    format!(
        r#"You are a DSL generator for a KYC/AML onboarding system.
Generate valid DSL S-expressions from natural language instructions.

## CRITICAL WORKFLOW

1. **ALWAYS look up existing data first** before generating DSL that references existing entities:
   - Use `lookup_cbu` when user mentions a client/CBU name
   - Use `lookup_entity` when user mentions a person or company
   - Use `lookup_product` when adding products
   - Use `list_cbus` if unsure what clients exist

2. **Use names, not UUIDs** - The DSL system accepts names for CBUs:
   - `(cbu.add-product :cbu-id "Apex Capital" :product "CUSTODY")` ✓
   - The name is matched case-insensitively in the database

3. **Create new entities with @references**:
   - `(cbu.ensure :name "New Fund" :jurisdiction "LU" :as @fund)`
   - Then reference: `(cbu.add-product :cbu-id @fund :product "CUSTODY")`

## AVAILABLE VERBS
{}

## DSL SYNTAX
- Format: (domain.verb :key "value" :key2 value2)
- Strings must be quoted: "text"
- Numbers are unquoted: 42, 25.5
- References start with @: @symbol_name
- Use :as @name to capture results

## EXAMPLES

Adding product to EXISTING CBU (after lookup confirms it exists):
```
(cbu.add-product :cbu-id "Apex Capital" :product "CUSTODY")
```

Creating NEW CBU and adding product:
```
(cbu.ensure :name "Pacific Growth" :jurisdiction "LU" :client-type "fund" :as @fund)
(cbu.add-product :cbu-id @fund :product "CUSTODY")
```

## PRODUCTS (use exact CODES)
- CUSTODY
- FUND_ACCOUNTING
- TRANSFER_AGENCY
- MIDDLE_OFFICE
- COLLATERAL_MGMT
- MARKETS_FX
- ALTS

Respond with ONLY the DSL, no explanation. If you cannot generate valid DSL, respond with: ERROR: <reason>"#,
        vocab
    )
}

/// Extract DSL from agent response text
///
/// Handles common agent output formats:
/// 1. Raw DSL text (ideal)
/// 2. Markdown code fences (```dsl ... ``` or ``` ... ```)
/// 3. Text with DSL embedded (extracts S-expressions)
fn extract_dsl_from_text(text: &str) -> String {
    let trimmed = text.trim();

    // If it already parses as DSL, return as-is
    if trimmed.starts_with('(') && parse_program(trimmed).is_ok() {
        return trimmed.to_string();
    }

    // Try to extract from markdown code fence
    // Matches ```dsl, ```clojure, ```lisp, or just ```
    let fence_patterns = ["```dsl", "```clojure", "```lisp", "```"];
    for pattern in fence_patterns {
        if let Some(start_idx) = trimmed.find(pattern) {
            let after_fence = &trimmed[start_idx + pattern.len()..];
            // Skip to newline after opening fence
            let content_start = after_fence.find('\n').map(|i| i + 1).unwrap_or(0);
            let content = &after_fence[content_start..];
            // Find closing fence
            if let Some(end_idx) = content.find("```") {
                let extracted = content[..end_idx].trim();
                if parse_program(extracted).is_ok() {
                    return extracted.to_string();
                }
            }
        }
    }

    // Try to find S-expression block in text
    // Look for opening paren and find matching close
    if let Some(start) = trimmed.find('(') {
        let mut depth = 0;
        let mut end = start;
        for (i, c) in trimmed[start..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = start + i + 1;
                        // Check if there are more statements
                        let remaining = trimmed[end..].trim();
                        if remaining.starts_with('(') {
                            // Multiple statements - find the last closing paren
                            continue;
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
        // Find last balanced paren for multi-statement DSL
        let mut last_end = end;
        let mut search_start = end;
        while let Some(next_start) = trimmed[search_start..].find('(') {
            let abs_start = search_start + next_start;
            let mut d = 0;
            for (i, c) in trimmed[abs_start..].char_indices() {
                match c {
                    '(' => d += 1,
                    ')' => {
                        d -= 1;
                        if d == 0 {
                            last_end = abs_start + i + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            search_start = last_end;
        }
        let extracted = trimmed[start..last_end].trim();
        if parse_program(extracted).is_ok() {
            return extracted.to_string();
        }
    }

    // Fall back to original text
    trimmed.to_string()
}

// ============================================================================
// DSL Validation & Vocabulary
// ============================================================================

/// Build vocabulary prompt for a domain
fn build_vocab_prompt(domain: Option<&str>) -> String {
    let mut lines = Vec::new();
    let reg = registry();

    let domain_list: Vec<String> = if let Some(d) = domain {
        vec![d.to_string()]
    } else {
        reg.domains().to_vec()
    };

    for domain_name in domain_list {
        for verb in reg.verbs_for_domain(&domain_name) {
            let required = verb.required_arg_names().join(", ");
            let optional = verb.optional_arg_names().join(", ");
            lines.push(format!(
                "{}.{}: {} [required: {}] [optional: {}]",
                verb.domain, verb.verb, verb.description, required, optional
            ));
        }
    }

    lines.join("\n")
}

/// POST /api/agent/validate - Validate DSL syntax and semantics (including dataflow)
pub(crate) async fn validate_dsl(
    State(state): State<AgentState>,
    Json(req): Json<ValidateDslRequest>,
) -> Result<Json<ValidationResult>, StatusCode> {
    use crate::dsl_v2::validation::{Severity, ValidationContext, ValidationRequest};

    // First parse
    if let Err(e) = parse_program(&req.dsl) {
        return Ok(Json(ValidationResult {
            valid: false,
            errors: vec![ValidationError {
                line: None,
                column: None,
                message: e,
                suggestion: None,
            }],
            warnings: vec![],
        }));
    }

    // Then run full semantic validation with CSG linter (includes dataflow)
    let validator_result = async {
        let v = SemanticValidator::new(state.pool.clone()).await?;
        v.with_csg_linter().await
    }
    .await;

    match validator_result {
        Ok(mut validator) => {
            let request = ValidationRequest {
                source: req.dsl.clone(),
                context: ValidationContext::default(),
            };
            match validator.validate(&request).await {
                crate::dsl_v2::validation::ValidationResult::Ok(_) => Ok(Json(ValidationResult {
                    valid: true,
                    errors: vec![],
                    warnings: vec![],
                })),
                crate::dsl_v2::validation::ValidationResult::Err(diagnostics) => {
                    let errors: Vec<ValidationError> = diagnostics
                        .iter()
                        .filter(|d| d.severity == Severity::Error)
                        .map(|d| ValidationError {
                            line: Some(d.span.line as usize),
                            column: Some(d.span.column as usize),
                            message: format!("[{}] {}", d.code.as_str(), d.message),
                            suggestion: d.suggestions.first().map(|s| s.message.clone()),
                        })
                        .collect();
                    let warnings: Vec<String> = diagnostics
                        .iter()
                        .filter(|d| d.severity == Severity::Warning)
                        .map(|d| format!("[{}] {}", d.code.as_str(), d.message))
                        .collect();
                    Ok(Json(ValidationResult {
                        valid: errors.is_empty(),
                        errors,
                        warnings,
                    }))
                }
            }
        }
        Err(_) => {
            // If validator fails to initialize, fall back to parse-only validation
            Ok(Json(ValidationResult {
                valid: true,
                errors: vec![],
                warnings: vec![],
            }))
        }
    }
}

/// GET /api/agent/domains - List available domains
pub(crate) async fn list_domains() -> Json<DomainsResponse> {
    let reg = registry();
    let domain_list = reg.domains();
    let domains: Vec<DomainInfo> = domain_list
        .iter()
        .map(|name| {
            let verbs = reg.verbs_for_domain(name);
            DomainInfo {
                name: name.to_string(),
                description: get_domain_description(name),
                verb_count: verbs.len(),
            }
        })
        .collect();

    Json(DomainsResponse {
        total_verbs: reg.len(),
        domains,
    })
}

/// GET /api/agent/vocabulary - Get vocabulary
pub(crate) async fn get_vocabulary(Query(query): Query<VocabQuery>) -> Json<VocabResponse> {
    let reg = registry();
    let verbs: Vec<VerbInfo> = if let Some(domain) = &query.domain {
        reg.verbs_for_domain(domain)
            .iter()
            .map(|v| VerbInfo {
                domain: v.domain.to_string(),
                name: v.verb.to_string(),
                full_name: format!("{}.{}", v.domain, v.verb),
                description: v.description.to_string(),
                required_args: v
                    .required_arg_names()
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                optional_args: v
                    .optional_arg_names()
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            })
            .collect()
    } else {
        // Return all verbs
        reg.domains()
            .iter()
            .flat_map(|d| {
                reg.verbs_for_domain(d)
                    .iter()
                    .map(|v| VerbInfo {
                        domain: v.domain.to_string(),
                        name: v.verb.to_string(),
                        full_name: format!("{}.{}", v.domain, v.verb),
                        description: v.description.to_string(),
                        required_args: v
                            .required_arg_names()
                            .iter()
                            .map(|s| s.to_string())
                            .collect(),
                        optional_args: v
                            .optional_arg_names()
                            .iter()
                            .map(|s| s.to_string())
                            .collect(),
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    };

    Json(VocabResponse { verbs })
}

/// GET /api/agent/health - Health check
pub(crate) async fn health_check() -> Json<HealthResponse> {
    let reg = registry();
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        verb_count: reg.len(),
        domain_count: reg.domains().len(),
    })
}

// ============================================================================
// Completions & Entity Extraction
// ============================================================================

/// POST /api/agent/complete - Get completions for entities
///
/// Provides LSP-style autocomplete for CBUs, entities, products, roles,
/// jurisdictions, and other reference data via EntityGateway.
pub(crate) async fn complete_entity(
    Json(req): Json<CompleteRequest>,
) -> Result<Json<CompleteResponse>, StatusCode> {
    // Map entity_type to EntityGateway nickname
    let nickname = match req.entity_type.to_lowercase().as_str() {
        "cbu" => "CBU",
        "entity" | "person" | "company" => "ENTITY",
        "product" => "PRODUCT",
        "role" => "ROLE",
        "jurisdiction" => "JURISDICTION",
        "currency" => "CURRENCY",
        "client_type" | "clienttype" => "CLIENT_TYPE",
        "instrument_class" | "instrumentclass" => "INSTRUMENT_CLASS",
        "market" => "MARKET",
        _ => {
            // Unknown type - return empty
            return Ok(Json(CompleteResponse {
                items: vec![],
                total: 0,
            }));
        }
    };

    // Connect to EntityGateway
    let addr = crate::dsl_v2::gateway_resolver::gateway_addr();
    let mut client = match entity_gateway::proto::ob::gateway::v1::entity_gateway_client::EntityGatewayClient::connect(addr.clone()).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to connect to EntityGateway at {}: {}", addr, e);
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
    };

    // Search via EntityGateway
    use entity_gateway::proto::ob::gateway::v1::{SearchMode, SearchRequest};

    let search_request = SearchRequest {
        nickname: nickname.to_string(),
        values: vec![req.query.clone()],
        search_key: None,
        mode: SearchMode::Fuzzy as i32,
        limit: Some(req.limit),
        discriminators: std::collections::HashMap::new(),
        tenant_id: None,
        cbu_id: None,
    };

    let response = match client.search(search_request).await {
        Ok(r) => r.into_inner(),
        Err(e) => {
            tracing::error!("EntityGateway search failed: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Convert to completion items
    let items: Vec<CompletionItem> = response
        .matches
        .into_iter()
        .map(|m| CompletionItem {
            value: m.token,
            label: m.display.clone(),
            detail: if m.display.contains('(') {
                // Extract detail from display if present, e.g., "Apex Fund (LU)"
                None
            } else {
                Some(nickname.to_string())
            },
            score: m.score,
        })
        .collect();

    let total = items.len();
    Ok(Json(CompleteResponse { items, total }))
}

/// POST /api/agent/extract-entities - Extract entity mentions from utterance
///
/// Uses the in-memory EntityLinkingService to extract entity mentions
/// without database queries. Useful for:
/// - Pre-filtering entity resolution before DSL generation
/// - Debugging entity recognition
/// - Verb boosting based on entity kinds
pub(crate) async fn extract_entity_mentions(
    State(state): State<AgentState>,
    Json(req): Json<ExtractEntitiesRequest>,
) -> Json<ExtractEntitiesResponse> {
    use crate::entity_linking::Evidence;

    // Convert evidence to wire format
    fn evidence_to_response(ev: &Evidence) -> EvidenceResponse {
        match ev {
            Evidence::AliasExact { alias } => EvidenceResponse {
                kind: "alias_exact".to_string(),
                details: serde_json::json!({ "alias": alias }),
            },
            Evidence::AliasTokenOverlap { tokens, overlap } => EvidenceResponse {
                kind: "token_overlap".to_string(),
                details: serde_json::json!({ "tokens": tokens, "overlap": overlap }),
            },
            Evidence::KindMatchBoost {
                expected,
                actual,
                boost,
            } => EvidenceResponse {
                kind: "kind_match_boost".to_string(),
                details: serde_json::json!({ "expected": expected, "actual": actual, "boost": boost }),
            },
            Evidence::KindMismatchPenalty {
                expected,
                actual,
                penalty,
            } => EvidenceResponse {
                kind: "kind_mismatch_penalty".to_string(),
                details: serde_json::json!({ "expected": expected, "actual": actual, "penalty": penalty }),
            },
            Evidence::ConceptOverlapBoost { concepts, boost } => EvidenceResponse {
                kind: "concept_overlap_boost".to_string(),
                details: serde_json::json!({ "concepts": concepts, "boost": boost }),
            },
        }
    }

    let expected_kinds: Option<Vec<String>> = req.expected_kinds;
    let context_concepts: Option<Vec<String>> = req.context_concepts;

    // Call entity linking service
    let resolutions = state.entity_linker.resolve_mentions(
        &req.utterance,
        expected_kinds.as_deref(),
        context_concepts.as_deref(),
        req.limit,
    );

    // Convert to response format
    let mentions: Vec<EntityMentionResponse> = resolutions
        .iter()
        .map(|r| EntityMentionResponse {
            span: r.mention_span,
            text: r.mention_text.clone(),
            candidates: r
                .candidates
                .iter()
                .map(|c| EntityCandidateResponse {
                    entity_id: c.entity_id.to_string(),
                    entity_kind: c.entity_kind.clone(),
                    canonical_name: c.canonical_name.clone(),
                    score: c.score,
                    evidence: c.evidence.iter().map(evidence_to_response).collect(),
                })
                .collect(),
            selected_id: r.selected.map(|id| id.to_string()),
            confidence: r.confidence,
        })
        .collect();

    // Find dominant entity (highest confidence selected across all mentions)
    let dominant = resolutions
        .iter()
        .filter(|r| r.selected.is_some() && r.confidence >= 0.5)
        .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap())
        .and_then(|r| {
            r.candidates
                .iter()
                .find(|c| Some(c.entity_id) == r.selected)
        });

    let dominant_entity = dominant.map(|c| EntityCandidateResponse {
        entity_id: c.entity_id.to_string(),
        entity_kind: c.entity_kind.clone(),
        canonical_name: c.canonical_name.clone(),
        score: c.score,
        evidence: c.evidence.iter().map(evidence_to_response).collect(),
    });

    let dominant_kind = dominant.map(|c| c.entity_kind.clone());

    Json(ExtractEntitiesResponse {
        snapshot_hash: state.entity_linker.snapshot_hash().to_string(),
        snapshot_version: state.entity_linker.snapshot_version(),
        entity_count: state.entity_linker.entity_count(),
        mentions,
        dominant_entity,
        dominant_kind,
    })
}

// ============================================================================
// Onboarding
// ============================================================================

/// POST /api/agent/onboard - Generate onboarding DSL from natural language
///
/// Uses the enhanced system prompt with onboarding context to generate
/// complete onboarding workflows from natural language descriptions.
pub(crate) async fn generate_onboarding_dsl(
    State(state): State<AgentState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<OnboardingRequest>,
) -> Json<OnboardingResponse> {
    // PolicyGate: check if legacy generate is allowed
    let actor = crate::policy::ActorResolver::from_headers(&headers);
    if !state.policy_gate.can_use_legacy_generate(&actor) {
        return Json(OnboardingResponse {
            dsl: None,
            explanation: None,
            validation: None,
            execution: None,
            error: Some("Legacy /onboard endpoint is disabled. Use /api/session/:id/chat instead.".into()),
        });
    }

    // Use the existing generate_dsl logic with onboarding-focused instruction
    let generate_req = GenerateDslRequest {
        instruction: req.description.clone(),
        domain: None, // Let it use all domains including resource and delivery
    };

    let gen_response = generate_dsl(State(state.clone()), headers.clone(), Json(generate_req)).await;

    match (gen_response.dsl.clone(), gen_response.error.clone()) {
        (Some(dsl), None) => {
            // Validate the generated DSL
            let validation = match parse_program(&dsl) {
                Ok(_) => ValidationResult {
                    valid: true,
                    errors: vec![],
                    warnings: vec![],
                },
                Err(e) => ValidationResult {
                    valid: false,
                    errors: vec![ValidationError {
                        line: None,
                        column: None,
                        message: e,
                        suggestion: None,
                    }],
                    warnings: vec![],
                },
            };

            // Execute if requested and valid
            let execution = if req.execute && validation.valid {
                match execute_onboarding_dsl(&state, &dsl).await {
                    Ok(result) => Some(result),
                    Err(e) => Some(OnboardingExecutionResult {
                        success: false,
                        cbu_id: None,
                        resource_count: 0,
                        delivery_count: 0,
                        errors: vec![e],
                    }),
                }
            } else {
                None
            };

            Json(OnboardingResponse {
                dsl: Some(dsl),
                explanation: Some("Onboarding DSL generated successfully".to_string()),
                validation: Some(validation),
                execution,
                error: None,
            })
        }
        (_, Some(error)) => Json(OnboardingResponse {
            dsl: None,
            explanation: None,
            validation: None,
            execution: None,
            error: Some(error),
        }),
        _ => Json(OnboardingResponse {
            dsl: None,
            explanation: None,
            validation: None,
            execution: None,
            error: Some("Unknown error generating DSL".to_string()),
        }),
    }
}

/// Helper: Execute onboarding DSL and count results
async fn execute_onboarding_dsl(
    state: &AgentState,
    dsl: &str,
) -> Result<OnboardingExecutionResult, String> {
    use crate::dsl_v2::validation::{Severity, ValidationContext, ValidationRequest};

    let program = parse_program(dsl).map_err(|e| format!("Parse error: {}", e))?;

    // CSG validation (includes dataflow)
    let validator_result = async {
        let v = SemanticValidator::new(state.pool.clone()).await?;
        v.with_csg_linter().await
    }
    .await;

    if let Ok(mut validator) = validator_result {
        let request = ValidationRequest {
            source: dsl.to_string(),
            context: ValidationContext::default(),
        };
        if let crate::dsl_v2::validation::ValidationResult::Err(diagnostics) =
            validator.validate(&request).await
        {
            let errors: Vec<String> = diagnostics
                .iter()
                .filter(|d| d.severity == Severity::Error)
                .map(|d| format!("[{}] {}", d.code.as_str(), d.message))
                .collect();
            if !errors.is_empty() {
                return Err(format!("Validation errors: {}", errors.join("; ")));
            }
        }
    }

    let plan = compile(&program).map_err(|e| format!("Compile error: {}", e))?;

    let mut ctx = ExecutionContext::new();
    state
        .dsl_v2_executor
        .execute_plan(&plan, &mut ctx)
        .await
        .map_err(|e| format!("Execution error: {}", e))?;

    // Count resources and deliveries from bindings
    let cbu_id = ctx
        .symbols
        .get("cbu_id")
        .or_else(|| ctx.symbols.get("client"))
        .copied();

    let resource_count = ctx
        .symbols
        .keys()
        .filter(|k| {
            k.contains("custody")
                || k.contains("settle")
                || k.contains("swift")
                || k.contains("nav")
                || k.contains("ibor")
                || k.contains("pnl")
                || k.contains("ledger")
        })
        .count();

    let delivery_count = ctx
        .symbols
        .keys()
        .filter(|k| k.contains("delivery"))
        .count();

    Ok(OnboardingExecutionResult {
        success: true,
        cbu_id,
        resource_count,
        delivery_count,
        errors: vec![],
    })
}

fn get_domain_description(domain: &str) -> String {
    match domain {
        "cbu" => "Client Business Unit lifecycle management".to_string(),
        "entity" => "Legal entity creation and management".to_string(),
        "document" => "Document management and verification".to_string(),
        "kyc" => "KYC investigation and risk assessment".to_string(),
        "screening" => "PEP, sanctions, and adverse media screening".to_string(),
        "decision" => "Approval workflow and decision management".to_string(),
        "monitoring" => "Ongoing monitoring and periodic reviews".to_string(),
        "attribute" => "Attribute value management".to_string(),
        "resource" => "Resource instance management for onboarding".to_string(),
        "delivery" => "Service delivery tracking for onboarding".to_string(),
        _ => format!("{} domain operations", domain),
    }
}

// ============================================================================
// Batch Operations
// ============================================================================

/// POST /api/batch/add-products - Add products to multiple CBUs
/// Server-side DSL generation and execution (no LLM needed)
pub(crate) async fn batch_add_products(
    State(state): State<AgentState>,
    Json(req): Json<BatchAddProductsRequest>,
) -> Json<BatchAddProductsResponse> {
    use std::time::Instant;

    let start = Instant::now();
    let mut results = Vec::new();
    let mut success_count = 0;
    let mut failure_count = 0;

    // Process each CBU x product combination
    for cbu_id in &req.cbu_ids {
        for product in &req.products {
            // Generate DSL server-side (deterministic, no LLM)
            let dsl = format!(
                r#"(cbu.add-product :cbu-id "{}" :product "{}")"#,
                cbu_id, product
            );

            // Parse and execute using shared executor (singleton batch)
            match parse_program(&dsl) {
                Ok(program) => {
                    let plan = match compile(&program) {
                        Ok(p) => p,
                        Err(e) => {
                            failure_count += 1;
                            results.push(BatchProductResult {
                                cbu_id: *cbu_id,
                                product: product.clone(),
                                success: false,
                                error: Some(format!("Compile error: {}", e)),
                                services_added: None,
                            });
                            continue;
                        }
                    };

                    let mut ctx = ExecutionContext::new().with_audit_user("batch_add_products");
                    match state.dsl_v2_executor.execute_plan(&plan, &mut ctx).await {
                        Ok(exec_results) => {
                            // Count services added from the result
                            let services_added = exec_results
                                .iter()
                                .filter_map(|r| {
                                    if let DslV2Result::Affected(n) = r {
                                        Some(*n as i32)
                                    } else {
                                        None
                                    }
                                })
                                .sum();

                            success_count += 1;
                            results.push(BatchProductResult {
                                cbu_id: *cbu_id,
                                product: product.clone(),
                                success: true,
                                error: None,
                                services_added: Some(services_added),
                            });
                        }
                        Err(e) => {
                            failure_count += 1;
                            results.push(BatchProductResult {
                                cbu_id: *cbu_id,
                                product: product.clone(),
                                success: false,
                                error: Some(format!("Execution error: {}", e)),
                                services_added: None,
                            });
                        }
                    }
                }
                Err(e) => {
                    failure_count += 1;
                    results.push(BatchProductResult {
                        cbu_id: *cbu_id,
                        product: product.clone(),
                        success: false,
                        error: Some(format!("Parse error: {:?}", e)),
                        services_added: None,
                    });
                }
            }
        }
    }

    let duration_ms = start.elapsed().as_millis() as u64;

    Json(BatchAddProductsResponse {
        total_operations: results.len(),
        success_count,
        failure_count,
        duration_ms,
        results,
    })
}
