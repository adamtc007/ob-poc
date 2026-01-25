//! Structured Intent Pipeline
//!
//! ALL user input flows through this pipeline. No exceptions.
//!
//! ## Pipeline Flow
//!
//! ```text
//! User Input
//!     │
//!     ▼
//! IntentPipeline.process()
//!     │
//!     ├─► Direct DSL? (starts with "(")
//!     │       └─► Parse → Validate → Return (no LLM)
//!     │
//!     └─► Natural Language
//!             │
//!             ▼
//!         HybridVerbSearcher.search() [semantic + learned + phrase]
//!             │
//!             ├─► Match found → LLM extracts args (JSON only) → Assemble DSL
//!             │
//!             └─► No match → Error with suggestions
//! ```
//!
//! The LLM NEVER writes DSL syntax — it only extracts argument values.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::Arc;

use ob_agentic::{create_llm_client, LlmClient};

use crate::dsl_v2::ast::find_unresolved_ref_locations;
use crate::dsl_v2::runtime_registry::{RuntimeArg, RuntimeVerb};
use crate::dsl_v2::{compile, enrich_program, parse_program, registry, runtime_registry_arc};
use crate::mcp::scope_resolution::{ScopeContext, ScopeResolutionOutcome, ScopeResolver};
use crate::mcp::verb_search::{
    check_ambiguity, HybridVerbSearcher, VerbSearchOutcome, VerbSearchResult,
};

#[cfg(feature = "database")]
use sqlx::PgPool;

// =============================================================================
// PIPELINE-LOCAL TYPES (avoid cascading changes to shared DSL types)
// =============================================================================

/// Argument value types for intent extraction (pipeline-local)
///
/// This is separate from DSL's ArgumentValue to avoid ripple effects across
/// serde/DB/UI boundaries. Converted to DSL syntax during assembly.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum IntentArgValue {
    /// Plain string literal (no lookup config)
    String(String),
    /// Numeric value
    Number(f64),
    /// Boolean value
    Boolean(bool),
    /// @symbol reference
    Reference(String),
    /// Resolved UUID
    Uuid(String),
    /// Needs entity resolution (has lookup config in YAML)
    Unresolved {
        value: String,
        entity_type: Option<String>,
    },
    /// Required arg not extracted by LLM
    Missing { arg_name: String },
    /// List of values
    List(Vec<IntentArgValue>),
    /// Map of key-value pairs (BTreeMap for stable ordering)
    Map(BTreeMap<String, IntentArgValue>),
}

/// Extracted structured intent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredIntent {
    /// The verb to execute
    pub verb: String,
    /// Extracted argument values
    pub arguments: Vec<IntentArgument>,
    /// Confidence in extraction
    pub confidence: f32,
    /// Any extraction notes/warnings
    pub notes: Vec<String>,
}

impl StructuredIntent {
    /// Create empty intent (for early exit cases)
    pub fn empty() -> Self {
        Self {
            verb: String::new(),
            arguments: vec![],
            confidence: 0.0,
            notes: vec![],
        }
    }
}

/// A single argument extracted from user intent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentArgument {
    pub name: String,
    pub value: IntentArgValue,
    pub resolved: bool,
}

/// Pipeline outcome enum for clear status reporting
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PipelineOutcome {
    /// DSL ready for execution (may have unresolved refs)
    Ready,
    /// Missing required arguments - need user input
    NeedsUserInput,
    /// Ambiguous verb selection - need clarification
    NeedsClarification,
    /// No matching verb found
    NoMatch,
    /// Scope was resolved - session context set, no DSL generated
    /// This is Stage 0: scope phrase consumed the input
    ScopeResolved {
        group_id: String,
        group_name: String,
        entity_count: i64,
    },
    /// Scope candidates need user selection
    ScopeCandidates,
}

/// Pipeline result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineResult {
    pub intent: StructuredIntent,
    pub verb_candidates: Vec<VerbSearchResult>,
    pub dsl: String,
    /// Hash of DSL for version tracking (enables safe commit)
    pub dsl_hash: Option<String>,
    pub valid: bool,
    pub validation_error: Option<String>,
    pub unresolved_refs: Vec<UnresolvedRef>,
    /// Missing required arguments (Problem B)
    pub missing_required: Vec<String>,
    /// Pipeline outcome for clear status
    pub outcome: PipelineOutcome,
    /// Scope resolution outcome (Stage 0) - if present, scope was attempted
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope_resolution: Option<ScopeResolutionOutcome>,
    /// Scope context for downstream entity resolution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope_context: Option<ScopeContext>,
}

/// An unresolved entity reference that needs lookup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnresolvedRef {
    pub param_name: String,
    pub search_value: String,
    pub entity_type: Option<String>,
    /// Search column from lookup config (Problem C)
    pub search_column: Option<String>,
    /// Unique ref_id for commit targeting (Problem K)
    pub ref_id: Option<String>,
}

/// Structured intent extraction pipeline
pub struct IntentPipeline {
    verb_searcher: HybridVerbSearcher,
    llm_client: Option<Arc<dyn LlmClient>>,
    scope_resolver: ScopeResolver,
    #[cfg(feature = "database")]
    pool: Option<PgPool>,
}

impl IntentPipeline {
    /// Create pipeline with verb searcher (lazy LLM init)
    pub fn new(verb_searcher: HybridVerbSearcher) -> Self {
        Self {
            verb_searcher,
            llm_client: None,
            scope_resolver: ScopeResolver::new(),
            #[cfg(feature = "database")]
            pool: None,
        }
    }

    /// Create pipeline with pre-initialized LLM client
    pub fn with_llm(verb_searcher: HybridVerbSearcher, llm_client: Arc<dyn LlmClient>) -> Self {
        Self {
            verb_searcher,
            llm_client: Some(llm_client),
            scope_resolver: ScopeResolver::new(),
            #[cfg(feature = "database")]
            pool: None,
        }
    }

    /// Create pipeline with database pool for scope resolution
    #[cfg(feature = "database")]
    pub fn with_pool(verb_searcher: HybridVerbSearcher, pool: PgPool) -> Self {
        Self {
            verb_searcher,
            llm_client: None,
            scope_resolver: ScopeResolver::new(),
            pool: Some(pool),
        }
    }

    /// Create pipeline with LLM client and database pool
    #[cfg(feature = "database")]
    pub fn with_llm_and_pool(
        verb_searcher: HybridVerbSearcher,
        llm_client: Arc<dyn LlmClient>,
        pool: PgPool,
    ) -> Self {
        Self {
            verb_searcher,
            llm_client: Some(llm_client),
            scope_resolver: ScopeResolver::new(),
            pool: Some(pool),
        }
    }

    /// Get or create LLM client
    fn get_llm(&self) -> Result<Arc<dyn LlmClient>> {
        if let Some(client) = &self.llm_client {
            Ok(Arc::clone(client))
        } else {
            create_llm_client()
        }
    }

    /// Full pipeline: instruction → structured intent → DSL
    ///
    /// Handles both:
    /// - Direct DSL input: `(view.book :client <Allianz>)` → parse, validate, return
    /// - Natural language: "show all allianz lux cbu" → semantic search → LLM → DSL
    ///
    /// ## Stage 0: Scope Resolution (HARD GATE)
    ///
    /// Before ANY verb discovery, we check if the input is a scope-setting phrase.
    /// If scope resolution resolves or returns candidates, we return early and
    /// do NOT proceed to Candle/LLM. This ensures:
    /// 1. Scope is always established before entity resolution
    /// 2. No spurious entity-search modals for client names
    /// 3. Deterministic UX: Resolved → chip, Candidates → picker, else → continue
    pub async fn process(
        &self,
        instruction: &str,
        domain_hint: Option<&str>,
    ) -> Result<PipelineResult> {
        self.process_with_scope(instruction, domain_hint, None)
            .await
    }

    /// Process with existing scope context (for subsequent commands after scope is set)
    pub async fn process_with_scope(
        &self,
        instruction: &str,
        domain_hint: Option<&str>,
        existing_scope: Option<ScopeContext>,
    ) -> Result<PipelineResult> {
        let trimmed = instruction.trim();

        // Fast path: Direct DSL input (starts with "(")
        // Skip semantic search and LLM entirely
        if trimmed.starts_with('(') {
            return self.process_direct_dsl(trimmed, existing_scope).await;
        }

        // =========================================================================
        // STAGE 0: Scope Resolution (HARD GATE - runs BEFORE Candle)
        // =========================================================================
        #[cfg(feature = "database")]
        if let Some(pool) = &self.pool {
            let scope_outcome = self.scope_resolver.resolve(trimmed, pool).await?;

            match &scope_outcome {
                ScopeResolutionOutcome::Resolved {
                    group_id,
                    group_name,
                    entity_count,
                } => {
                    // Scope phrase consumed the input - return early, do NOT call Candle
                    tracing::info!(
                        group_id = %group_id,
                        group_name = %group_name,
                        entity_count = %entity_count,
                        "Stage 0: Scope resolved (hard gate - skipping Candle)"
                    );

                    let scope_ctx =
                        ScopeContext::new().with_client_group(*group_id, group_name.clone());

                    return Ok(PipelineResult {
                        intent: StructuredIntent::empty(),
                        verb_candidates: vec![],
                        dsl: String::new(),
                        dsl_hash: None,
                        valid: true, // Scope resolution is a valid outcome
                        validation_error: None,
                        unresolved_refs: vec![],
                        missing_required: vec![],
                        outcome: PipelineOutcome::ScopeResolved {
                            group_id: group_id.to_string(),
                            group_name: group_name.clone(),
                            entity_count: *entity_count,
                        },
                        scope_resolution: Some(scope_outcome),
                        scope_context: Some(scope_ctx),
                    });
                }
                ScopeResolutionOutcome::Candidates(candidates) => {
                    // Multiple matches - return for user to pick (compact picker, not modal)
                    tracing::info!(
                        candidate_count = candidates.len(),
                        "Stage 0: Scope candidates (hard gate - skipping Candle)"
                    );

                    return Ok(PipelineResult {
                        intent: StructuredIntent::empty(),
                        verb_candidates: vec![],
                        dsl: String::new(),
                        dsl_hash: None,
                        valid: false,
                        validation_error: Some(format!(
                            "Multiple clients match. Did you mean {}?",
                            candidates
                                .iter()
                                .map(|c| format!("'{}'", c.group_name))
                                .collect::<Vec<_>>()
                                .join(" or ")
                        )),
                        unresolved_refs: vec![],
                        missing_required: vec![],
                        outcome: PipelineOutcome::ScopeCandidates,
                        scope_resolution: Some(scope_outcome),
                        scope_context: None,
                    });
                }
                ScopeResolutionOutcome::Unresolved | ScopeResolutionOutcome::NotScopePhrase => {
                    // Not a scope phrase or no match - continue to verb discovery
                    tracing::debug!("Stage 0: Not a scope phrase, continuing to Candle");
                }
            }
        }

        // Use existing scope or empty
        let scope_ctx = existing_scope.unwrap_or_default();

        // Natural language path (with scope context for entity resolution)
        self.process_as_natural_language(instruction, domain_hint, scope_ctx)
            .await
    }

    /// Process input as natural language (semantic search → LLM extraction → DSL)
    ///
    /// This is the main NL processing path, also called when direct DSL parsing fails.
    async fn process_as_natural_language(
        &self,
        instruction: &str,
        domain_hint: Option<&str>,
        scope_ctx: ScopeContext,
    ) -> Result<PipelineResult> {
        // Step 1: Find verb candidates via semantic search
        // TODO: Pass scope_ctx to verb_searcher.search() for scoped entity resolution
        let candidates = self
            .verb_searcher
            .search(instruction, None, domain_hint, 5)
            .await?;

        if candidates.is_empty() {
            // Provide helpful message indicating if semantic search is available
            let semantic_status = if self.verb_searcher.has_semantic_search() {
                "" // Semantic is available, just no match
            } else {
                " (semantic search still initializing - try again in a moment)"
            };
            return Ok(PipelineResult {
                intent: StructuredIntent::empty(),
                verb_candidates: vec![],
                dsl: String::new(),
                dsl_hash: None,
                valid: false,
                validation_error: Some(format!(
                    "No matching verbs found for: {}{}",
                    instruction, semantic_status
                )),
                unresolved_refs: vec![],
                missing_required: vec![],
                outcome: PipelineOutcome::NoMatch,
                scope_resolution: None,
                scope_context: if scope_ctx.has_scope() {
                    Some(scope_ctx)
                } else {
                    None
                },
            });
        }

        // Step 1b: Check for ambiguity (Issue D/J)
        // Use searcher's semantic_threshold for consistent behavior
        let threshold = self.verb_searcher.semantic_threshold();
        let ambiguity_outcome = check_ambiguity(&candidates, threshold);

        match ambiguity_outcome {
            VerbSearchOutcome::NoMatch => {
                // All candidates below threshold
                return Ok(PipelineResult {
                    intent: StructuredIntent::empty(),
                    verb_candidates: candidates,
                    dsl: String::new(),
                    dsl_hash: None,
                    valid: false,
                    validation_error: Some(format!(
                        "No verbs matched with confidence >= {:.0}%",
                        threshold * 100.0
                    )),
                    unresolved_refs: vec![],
                    missing_required: vec![],
                    outcome: PipelineOutcome::NoMatch,
                    scope_resolution: None,
                    scope_context: if scope_ctx.has_scope() {
                        Some(scope_ctx)
                    } else {
                        None
                    },
                });
            }
            VerbSearchOutcome::Ambiguous {
                top,
                runner_up,
                margin,
            } => {
                // DO NOT call LLM - return for user clarification
                return Ok(PipelineResult {
                    intent: StructuredIntent::empty(),
                    verb_candidates: vec![top.clone(), runner_up.clone()],
                    dsl: String::new(),
                    dsl_hash: None,
                    valid: false,
                    validation_error: Some(format!(
                        "Ambiguous verb match (margin={:.3}). Did you mean '{}' or '{}'?",
                        margin, top.verb, runner_up.verb
                    )),
                    unresolved_refs: vec![],
                    missing_required: vec![],
                    outcome: PipelineOutcome::NeedsClarification,
                    scope_resolution: None,
                    scope_context: if scope_ctx.has_scope() {
                        Some(scope_ctx.clone())
                    } else {
                        None
                    },
                });
            }
            VerbSearchOutcome::Matched(matched_verb) => {
                // Clear winner - continue with LLM extraction
                // Use matched_verb below
                let _ = matched_verb; // We'll use candidates[0] for consistency
            }
        }

        let top_verb = &candidates[0].verb;

        // Step 2: Get verb signature from registry
        let reg = registry();
        let parts: Vec<&str> = top_verb.splitn(2, '.').collect();
        if parts.len() != 2 {
            return Err(anyhow!("Invalid verb format: {}", top_verb));
        }

        let verb_def = reg
            .get_runtime_verb(parts[0], parts[1])
            .ok_or_else(|| anyhow!("Verb not in registry: {}", top_verb))?;

        // Step 3: Extract arguments via LLM (structured output only)
        let intent = self
            .extract_arguments(instruction, top_verb, verb_def, candidates[0].score)
            .await?;

        // Step 4: Check for missing required args BEFORE assembly (Problem B - fail early)
        let missing_required: Vec<String> = intent
            .arguments
            .iter()
            .filter_map(|arg| match &arg.value {
                IntentArgValue::Missing { arg_name } => Some(arg_name.clone()),
                _ => None,
            })
            .collect();

        if !missing_required.is_empty() {
            // FAIL EARLY - don't waste work on DSL compile
            return Ok(PipelineResult {
                intent,
                verb_candidates: candidates,
                dsl: String::new(),
                dsl_hash: None,
                valid: false,
                validation_error: Some(format!(
                    "Missing required arguments: {}",
                    missing_required.join(", ")
                )),
                unresolved_refs: vec![],
                missing_required,
                outcome: PipelineOutcome::NeedsUserInput,
                scope_resolution: None,
                scope_context: if scope_ctx.has_scope() {
                    Some(scope_ctx)
                } else {
                    None
                },
            });
        }

        // Step 5: Assemble DSL string deterministically (no synthetic ref tracking)
        let dsl = self.assemble_dsl_string(&intent)?;

        // Step 6: Parse and enrich to extract real refs (FIX C)
        // Parse → Enrich → Walk = proper span-based ref_ids + search_column
        // NOTE: This runs even if compile/validate will fail - we still want unresolved refs
        let (unresolved, parse_error) = match parse_program(&dsl) {
            Ok(ast) => {
                let registry = runtime_registry_arc();
                let enriched = enrich_program(ast, &registry);

                // Use canonical walker - handles nested maps/lists correctly
                let locations = find_unresolved_ref_locations(&enriched.program);

                // Map to UnresolvedRef - DON'T double-wrap Option fields
                let refs: Vec<UnresolvedRef> = locations
                    .into_iter()
                    .map(|loc| UnresolvedRef {
                        param_name: loc.arg_key,
                        search_value: loc.search_text,
                        entity_type: Some(loc.entity_type), // UnresolvedRefLocation.entity_type is String, not Option
                        search_column: loc.search_column,   // Already Option<String>
                        ref_id: loc.ref_id,                 // Already Option<String>
                    })
                    .collect();

                (refs, None)
            }
            Err(e) => {
                // Don't swallow - surface parse error
                (vec![], Some(format!("Parse error after assembly: {:?}", e)))
            }
        };

        // Step 7: Validate (compile check) - runs independently of parse/enrich
        let (valid, validation_error) = match &parse_error {
            Some(err) => (false, Some(err.clone())),
            None => self.validate_dsl(&dsl),
        };

        // Compute dsl_hash for version tracking (enables safe commit)
        let dsl_hash = if dsl.is_empty() {
            None
        } else {
            Some(compute_dsl_hash(&dsl))
        };

        Ok(PipelineResult {
            intent,
            verb_candidates: candidates,
            dsl,
            dsl_hash,
            valid,
            validation_error,
            unresolved_refs: unresolved, // Now has real refs even if valid=false
            missing_required: vec![],
            outcome: if valid {
                PipelineOutcome::Ready
            } else {
                PipelineOutcome::NeedsUserInput
            },
            scope_resolution: None,
            scope_context: if scope_ctx.has_scope() {
                Some(scope_ctx)
            } else {
                None
            },
        })
    }

    /// Extract arguments from instruction using LLM (structured output only)
    ///
    /// Problem A fix: Uses verb schema to determine if string needs resolution.
    /// Only fields with explicit `lookup` config in YAML are marked as Unresolved.
    async fn extract_arguments(
        &self,
        instruction: &str,
        verb: &str,
        verb_def: &RuntimeVerb,
        verb_confidence: f32,
    ) -> Result<StructuredIntent> {
        let llm = self.get_llm()?;

        // Build parameter schema for LLM
        // For uuid/entity parameters with lookup config, show as "entity name" not "Uuid"
        // This helps the LLM understand it should extract names, not UUIDs
        let params_desc: Vec<String> = verb_def
            .args
            .iter()
            .map(|p| {
                let req = if p.required { "REQUIRED" } else { "optional" };
                let desc = p.description.as_deref().unwrap_or("");
                // If this has lookup config, it's an entity reference - extract name not UUID
                let type_hint = if p.lookup.is_some() {
                    "entity name (will be resolved to UUID)".to_string()
                } else {
                    format!("{:?}", p.arg_type)
                };
                format!("- {}: {} ({}) - {}", p.name, type_hint, req, desc)
            })
            .collect();

        let system_prompt = format!(
            r#"You are an argument extractor for a DSL system.

Given a natural language instruction, extract argument values for the verb: {verb}

VERB PARAMETERS:
{params}

RULES:
1. Extract values mentioned in the instruction - look for names, identifiers, and references
2. For "entity name" parameters:
   - Extract ONLY the proper noun/entity name (e.g., "Allianz", "BlackRock", "Goldman Sachs")
   - Do NOT include descriptive words like "cbu", "universe", "fund", "book", "system" in the entity name
   - Example: "show allianz cbu universe" → entity name is "Allianz" (not "allianz cbu")
   - Example: "load blackrock fund book" → entity name is "BlackRock" (not "blackrock fund")
3. For dates, use ISO format (YYYY-MM-DD)
4. For enums, match to closest valid value
5. If a required parameter cannot be found in the instruction, set value to null
6. Do NOT write DSL syntax - only extract values

Respond with ONLY valid JSON:
{{
  "arguments": [
    {{"name": "param_name", "value": "extracted_value"}},
    ...
  ],
  "notes": ["any extraction notes"]
}}"#,
            verb = verb,
            params = params_desc.join("\n"),
        );

        let response = llm.chat(&system_prompt, instruction).await?;

        tracing::debug!(verb = verb, "LLM extraction complete");

        // Parse LLM response - handle potential markdown code blocks
        let json_str = extract_json_from_response(&response);

        let parsed: Value = serde_json::from_str(json_str)
            .map_err(|e| anyhow!("LLM returned invalid JSON: {} - response: {}", e, response))?;

        // Problem A: Use verb schema to classify strings
        let mut arguments = Vec::new();
        if let Some(args) = parsed["arguments"].as_array() {
            for arg in args {
                let name = arg["name"].as_str().unwrap_or_default().to_string();
                if name.is_empty() {
                    continue;
                }

                // Find the arg definition from verb schema
                let arg_def = verb_def.args.iter().find(|a| a.name == name);

                // Problem A: Only mark as Unresolved if lookup config exists
                let needs_lookup = arg_def.map(|a| a.lookup.is_some()).unwrap_or(false);

                let value = convert_json_to_intent_value(&arg["value"], arg_def, needs_lookup);

                // Problem B: Track missing required args
                if let Value::Null = &arg["value"] {
                    let is_required = arg_def.map(|a| a.required).unwrap_or(false);
                    if is_required {
                        arguments.push(IntentArgument {
                            name: name.clone(),
                            value: IntentArgValue::Missing { arg_name: name },
                            resolved: false,
                        });
                    }
                    continue;
                }

                if let Some(val) = value {
                    arguments.push(IntentArgument {
                        name,
                        value: val,
                        resolved: false,
                    });
                }
            }
        }

        // Problem B: Check for required args that weren't even mentioned by LLM
        for arg_def in &verb_def.args {
            if arg_def.required {
                let was_extracted = arguments.iter().any(|a| a.name == arg_def.name);
                if !was_extracted {
                    arguments.push(IntentArgument {
                        name: arg_def.name.clone(),
                        value: IntentArgValue::Missing {
                            arg_name: arg_def.name.clone(),
                        },
                        resolved: false,
                    });
                }
            }
        }

        let notes: Vec<String> = parsed["notes"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        Ok(StructuredIntent {
            verb: verb.to_string(),
            arguments,
            confidence: verb_confidence,
            notes,
        })
    }

    /// Process direct DSL input (bypass semantic search and LLM)
    ///
    /// When user types DSL directly like `(view.book :client <Allianz>)`,
    /// we parse and validate it without involving the LLM.
    async fn process_direct_dsl(
        &self,
        dsl: &str,
        scope: Option<ScopeContext>,
    ) -> Result<PipelineResult> {
        use crate::mcp::verb_search::VerbSearchSource;
        use dsl_core::Statement;

        let scope_ctx = scope.unwrap_or_default();

        tracing::info!("Processing direct DSL input: {}", dsl);

        // Parse the DSL - on failure, re-route through natural language pipeline
        // This lets the LLM interpret malformed DSL as user intent
        let ast = match parse_program(dsl) {
            Ok(ast) => ast,
            Err(parse_error) => {
                tracing::info!(
                    "DSL parse failed, re-routing to NL pipeline: {}",
                    parse_error
                );
                // Recursively call process() but skip the DSL detection
                // by treating the malformed DSL as natural language
                return self.process_as_natural_language(dsl, None, scope_ctx).await;
            }
        };

        // Extract verb from first statement
        let verb = if let Some(stmt) = ast.statements.first() {
            match stmt {
                Statement::VerbCall(vc) => format!("{}.{}", vc.domain, vc.verb),
                Statement::Comment(_) => return Err(anyhow!("First statement is a comment")),
            }
        } else {
            return Err(anyhow!("Empty DSL program"));
        };

        // Validate via compile
        let (valid, validation_error) = self.validate_dsl(dsl);

        // Enrich AST and extract entity refs using canonical walker (FIX C)
        let registry = runtime_registry_arc();
        let enriched = enrich_program(ast, &registry);
        let locations = find_unresolved_ref_locations(&enriched.program);

        let unresolved: Vec<UnresolvedRef> = locations
            .into_iter()
            .map(|loc| UnresolvedRef {
                param_name: loc.arg_key,
                search_value: loc.search_text,
                entity_type: Some(loc.entity_type),
                search_column: loc.search_column,
                ref_id: loc.ref_id,
            })
            .collect();

        // Compute dsl_hash for version tracking
        let dsl_hash = Some(compute_dsl_hash(dsl));

        // Build a minimal StructuredIntent for consistency
        let intent = StructuredIntent {
            verb: verb.clone(),
            arguments: vec![], // Args are in the DSL itself
            confidence: 1.0,   // Direct DSL = full confidence
            notes: vec!["Direct DSL input".to_string()],
        };

        Ok(PipelineResult {
            intent,
            verb_candidates: vec![VerbSearchResult {
                verb,
                score: 1.0,
                source: VerbSearchSource::DirectDsl,
                matched_phrase: dsl.to_string(),
                description: Some("Direct DSL input".to_string()),
            }],
            dsl: dsl.to_string(),
            dsl_hash,
            valid,
            validation_error,
            unresolved_refs: unresolved,
            missing_required: vec![],
            outcome: if valid {
                PipelineOutcome::Ready
            } else {
                PipelineOutcome::NeedsUserInput
            },
            scope_resolution: None,
            scope_context: if scope_ctx.has_scope() {
                Some(scope_ctx)
            } else {
                None
            },
        })
    }

    /// Assemble DSL string from structured intent (deterministic)
    ///
    /// Returns string only - unresolved refs are extracted from enriched AST later (Fix C)
    fn assemble_dsl_string(&self, intent: &StructuredIntent) -> Result<String> {
        let mut dsl = format!("({}", intent.verb);

        for arg in &intent.arguments {
            // Skip Missing args - they shouldn't appear in DSL
            if matches!(arg.value, IntentArgValue::Missing { .. }) {
                continue;
            }

            let value_str = format_intent_value_string_only(&arg.value);
            dsl.push_str(&format!(" :{} {}", arg.name, value_str));
        }

        dsl.push(')');
        Ok(dsl)
    }

    /// Validate generated DSL
    fn validate_dsl(&self, dsl: &str) -> (bool, Option<String>) {
        match parse_program(dsl) {
            Ok(ast) => match compile(&ast) {
                Ok(_) => (true, None),
                Err(e) => (false, Some(format!("Compile error: {:?}", e))),
            },
            Err(e) => (false, Some(format!("Parse error: {:?}", e))),
        }
    }
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Convert JSON value to IntentArgValue using verb arg definition (Problem A)
///
/// Only marks strings as Unresolved if the arg has lookup config.
fn convert_json_to_intent_value(
    value: &Value,
    arg_def: Option<&RuntimeArg>,
    needs_lookup: bool,
) -> Option<IntentArgValue> {
    let entity_type = arg_def
        .and_then(|a| a.lookup.as_ref())
        .and_then(|l| l.entity_type.clone());

    match value {
        Value::Null => None,

        Value::Bool(b) => Some(IntentArgValue::Boolean(*b)),

        Value::Number(n) => Some(IntentArgValue::Number(n.as_f64().unwrap_or(0.0))),

        Value::String(s) => {
            // Check if it looks like a UUID
            if uuid::Uuid::parse_str(s).is_ok() {
                Some(IntentArgValue::Uuid(s.clone()))
            } else if let Some(stripped) = s.strip_prefix('@') {
                // @symbol reference
                Some(IntentArgValue::Reference(stripped.to_string()))
            } else if needs_lookup {
                // Problem A: Only Unresolved if lookup config exists
                Some(IntentArgValue::Unresolved {
                    value: s.clone(),
                    entity_type,
                })
            } else {
                // Plain string literal
                Some(IntentArgValue::String(s.clone()))
            }
        }

        Value::Array(arr) => {
            let items: Vec<IntentArgValue> = arr
                .iter()
                .filter_map(|v| convert_json_to_intent_value(v, arg_def, needs_lookup))
                .collect();
            Some(IntentArgValue::List(items))
        }

        Value::Object(obj) => {
            let entries: BTreeMap<String, IntentArgValue> = obj
                .iter()
                .filter_map(|(k, v)| {
                    convert_json_to_intent_value(v, None, false).map(|av| (k.clone(), av))
                })
                .collect();
            Some(IntentArgValue::Map(entries))
        }
    }
}

/// Format IntentArgValue to DSL string only (Fix C - no synthetic refs)
///
/// Unresolved refs are extracted from the enriched AST after parsing,
/// which gives us real span-based ref_ids and search_column metadata.
fn format_intent_value_string_only(value: &IntentArgValue) -> String {
    match value {
        IntentArgValue::String(s) => format!("\"{}\"", s.replace('"', "\\\"")),
        IntentArgValue::Number(n) => n.to_string(),
        IntentArgValue::Boolean(b) => b.to_string(),
        IntentArgValue::Reference(r) => format!("@{}", r),
        IntentArgValue::Uuid(u) => format!("\"{}\"", u),
        IntentArgValue::Unresolved { value, .. } => {
            // Emit as quoted string - enrichment pass will convert to EntityRef
            // based on verb arg's lookup config
            format!("\"{}\"", value.replace('"', "\\\""))
        }
        IntentArgValue::Missing { .. } => "nil".to_string(),
        IntentArgValue::List(items) => {
            let formatted: Vec<String> =
                items.iter().map(format_intent_value_string_only).collect();
            format!("[{}]", formatted.join(" "))
        }
        IntentArgValue::Map(entries) => {
            let formatted: Vec<String> = entries
                .iter()
                .map(|(k, v)| format!(":{} {}", k, format_intent_value_string_only(v)))
                .collect();
            format!("{{{}}}", formatted.join(" "))
        }
    }
}

/// Compute SHA-256 hash of DSL string for version tracking (Issue K)
///
/// Used to verify that commit requests apply to the correct DSL version,
/// preventing race conditions where DSL is modified between disambiguation
/// and resolution commit.
pub fn compute_dsl_hash(dsl: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(dsl.as_bytes());
    let result = hasher.finalize();
    // Use first 16 hex chars for brevity while maintaining collision resistance
    format!("{:x}", result)[..16].to_string()
}

/// Extract JSON from LLM response, handling markdown code blocks
fn extract_json_from_response(response: &str) -> &str {
    let trimmed = response.trim();

    // Handle ```json ... ``` blocks
    if trimmed.starts_with("```json") {
        if let Some(end) = trimmed.rfind("```") {
            let start = "```json".len();
            if end > start {
                return trimmed[start..end].trim();
            }
        }
    }

    // Handle ``` ... ``` blocks without language
    if let Some(stripped) = trimmed.strip_prefix("```") {
        if let Some(end) = stripped.find("```") {
            return stripped[..end].trim();
        }
    }

    trimmed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_from_response() {
        // Plain JSON
        let plain = r#"{"arguments": []}"#;
        assert_eq!(extract_json_from_response(plain), plain);

        // Markdown code block
        let markdown = "```json\n{\"arguments\": []}\n```";
        assert_eq!(extract_json_from_response(markdown), "{\"arguments\": []}");

        // With whitespace
        let whitespace = "  \n```json\n{\"arguments\": []}\n```\n  ";
        assert_eq!(
            extract_json_from_response(whitespace),
            "{\"arguments\": []}"
        );
    }

    #[test]
    fn test_assemble_dsl_string() {
        let intent = StructuredIntent {
            verb: "cbu.create".to_string(),
            arguments: vec![
                IntentArgument {
                    name: "name".to_string(),
                    value: IntentArgValue::String("Apex Fund".to_string()),
                    resolved: false,
                },
                IntentArgument {
                    name: "jurisdiction".to_string(),
                    value: IntentArgValue::String("LU".to_string()),
                    resolved: true,
                },
                IntentArgument {
                    name: "client".to_string(),
                    value: IntentArgValue::Unresolved {
                        value: "Allianz".to_string(),
                        entity_type: Some("entity".to_string()),
                    },
                    resolved: false,
                },
            ],
            confidence: 0.95,
            notes: vec![],
        };

        // Use minimal searcher for testing (no DB required)
        let searcher = HybridVerbSearcher::minimal();
        let pipeline = IntentPipeline::new(searcher);

        // assemble_dsl_string returns only the DSL string (Fix C)
        let dsl = pipeline.assemble_dsl_string(&intent).unwrap();
        assert!(dsl.contains("cbu.create"));
        assert!(dsl.contains(":name \"Apex Fund\""));
        assert!(dsl.contains(":jurisdiction \"LU\""));
        assert!(dsl.contains(":client \"Allianz\""));

        // Unresolved refs are now extracted from enriched AST, not tracked during assembly
        // This is tested separately in test_unresolved_refs_from_enriched_ast
    }

    #[test]
    fn test_missing_required_tracked() {
        let intent = StructuredIntent {
            verb: "entity.create".to_string(),
            arguments: vec![
                IntentArgument {
                    name: "name".to_string(),
                    value: IntentArgValue::String("John Doe".to_string()),
                    resolved: false,
                },
                IntentArgument {
                    name: "lei".to_string(),
                    value: IntentArgValue::Missing {
                        arg_name: "lei".to_string(),
                    },
                    resolved: false,
                },
            ],
            confidence: 0.9,
            notes: vec![],
        };

        // Check missing args are correctly identified
        let missing: Vec<String> = intent
            .arguments
            .iter()
            .filter_map(|arg| match &arg.value {
                IntentArgValue::Missing { arg_name } => Some(arg_name.clone()),
                _ => None,
            })
            .collect();

        assert_eq!(missing, vec!["lei".to_string()]);
    }

    #[test]
    fn test_format_list_and_map() {
        let list_value = IntentArgValue::List(vec![
            IntentArgValue::String("a".to_string()),
            IntentArgValue::String("b".to_string()),
        ]);

        // format_intent_value_string_only is now used (Fix C)
        let formatted = format_intent_value_string_only(&list_value);
        assert_eq!(formatted, "[\"a\" \"b\"]");

        let mut map = BTreeMap::new();
        map.insert(
            "key1".to_string(),
            IntentArgValue::String("val1".to_string()),
        );
        map.insert("key2".to_string(), IntentArgValue::Number(42.0));
        let map_value = IntentArgValue::Map(map);

        let formatted = format_intent_value_string_only(&map_value);
        assert!(formatted.contains(":key1 \"val1\""));
        assert!(formatted.contains(":key2 42"));
    }

    #[test]
    fn test_compute_dsl_hash() {
        let dsl1 = "(cbu.create :name \"Test\")";
        let dsl2 = "(cbu.create :name \"Test\")";
        let dsl3 = "(cbu.create :name \"Different\")";

        // Same input should produce same hash
        assert_eq!(compute_dsl_hash(dsl1), compute_dsl_hash(dsl2));

        // Different input should produce different hash
        assert_ne!(compute_dsl_hash(dsl1), compute_dsl_hash(dsl3));

        // Hash should be 16 hex chars
        assert_eq!(compute_dsl_hash(dsl1).len(), 16);
    }

    // =========================================================================
    // Issue K Acceptance Test - List Commit Correctness
    // =========================================================================

    #[test]
    fn test_list_commit_resolves_single_ref() {
        use crate::dsl_v2::ast::find_unresolved_ref_locations;
        use crate::dsl_v2::{enrich_program, runtime_registry_arc};
        use dsl_core::ast::{Argument, AstNode, Literal, Program, Span, Statement, VerbCall};
        // HashSet used in commented-out TODO assertion for unique ref_ids
        #[allow(unused_imports)]
        use std::collections::HashSet;

        // Construct raw AST with list of strings that will become EntityRefs
        // Using cbu.assign-role which has entity-id with lookup config
        let raw = Program {
            statements: vec![Statement::VerbCall(VerbCall {
                domain: "cbu".to_string(),
                verb: "assign-role".to_string(),
                arguments: vec![
                    Argument {
                        key: "cbu-id".to_string(),
                        value: AstNode::Literal(Literal::String("test-cbu-uuid".to_string())),
                        span: Span::new(10, 30),
                    },
                    // entity-id as a list - each will become EntityRef after enrichment
                    Argument {
                        key: "entity-id".to_string(),
                        value: AstNode::List {
                            items: vec![
                                AstNode::Literal(Literal::String("Allianz".to_string())),
                                AstNode::Literal(Literal::String("BlackRock".to_string())),
                                AstNode::Literal(Literal::String("Vanguard".to_string())),
                            ],
                            span: Span::new(40, 80),
                        },
                        span: Span::new(35, 85),
                    },
                    Argument {
                        key: "role".to_string(),
                        value: AstNode::Literal(Literal::String("DIRECTOR".to_string())),
                        span: Span::new(90, 110),
                    },
                ],
                binding: None,
                span: Span::new(0, 120),
            })],
        };

        // Enrich to convert strings to EntityRefs
        let registry = runtime_registry_arc();
        let enriched = enrich_program(raw, &registry);

        // Get unresolved refs
        let refs = find_unresolved_ref_locations(&enriched.program);

        // Should have refs (includes cbu-id, entity-id list items, role)
        // The list items should each have distinct ref_ids
        assert!(
            refs.len() >= 3,
            "Expected at least 3 unresolved refs, got {}",
            refs.len()
        );

        // All should have ref_ids
        for r in &refs {
            assert!(
                r.ref_id.is_some(),
                "ref_id should be present for '{}'",
                r.search_text
            );
        }

        // Filter to just the entity-id list items (Allianz, BlackRock, Vanguard)
        let list_refs: Vec<_> = refs
            .iter()
            .filter(|r| {
                r.search_text == "Allianz"
                    || r.search_text == "BlackRock"
                    || r.search_text == "Vanguard"
            })
            .collect();

        assert_eq!(
            list_refs.len(),
            3,
            "Expected 3 entity-id list refs, got {}",
            list_refs.len()
        );

        // Verify ref_ids exist for list items
        // NOTE: Currently list items share the parent list's span, so ref_ids are NOT unique.
        // This is a known limitation - for full Issue K correctness, the enrichment should
        // assign unique spans to each list item. For now, we verify refs exist and have ref_ids.
        let ref_ids: Vec<String> = list_refs.iter().filter_map(|r| r.ref_id.clone()).collect();
        assert_eq!(
            ref_ids.len(),
            3,
            "All list items should have ref_ids: {:?}",
            ref_ids
        );
        // TODO: When list item spans are fixed, uncomment this assertion:
        // let unique: HashSet<&String> = ref_ids.iter().collect();
        // assert_eq!(unique.len(), 3, "List item ref_ids should be unique");

        // Verify all expected search values are present
        let search_values: Vec<&str> = refs.iter().map(|r| r.search_text.as_str()).collect();
        assert!(search_values.contains(&"Allianz"), "Should contain Allianz");
        assert!(
            search_values.contains(&"BlackRock"),
            "Should contain BlackRock"
        );
        assert!(
            search_values.contains(&"Vanguard"),
            "Should contain Vanguard"
        );
    }

    /// Issue K acceptance test: Construct AST → enrich → commit one ref → verify
    ///
    /// This proves the full end-to-end flow for list resolution:
    /// 1. Construct raw AST with list of strings (simulates parsed DSL)
    /// 2. Enrich to get EntityRef nodes with unique ref_ids
    /// 3. Commit resolution for ONE ref_id
    /// 4. Verify only that one is resolved, others remain unresolved
    #[test]
    fn test_issue_k_commit_resolves_single_list_item() {
        use crate::dsl_v2::ast::{find_unresolved_ref_locations, Statement};
        use crate::dsl_v2::{enrich_program, runtime_registry_arc};
        use dsl_core::ast::{Argument, AstNode, Literal, Program, Span, VerbCall};
        use std::collections::HashSet;

        // Step 1: Construct raw AST with list of strings (enrichment converts to EntityRefs)
        let raw = Program {
            statements: vec![Statement::VerbCall(VerbCall {
                domain: "cbu".to_string(),
                verb: "assign-role".to_string(),
                arguments: vec![
                    Argument {
                        key: "cbu-id".to_string(),
                        value: AstNode::Literal(Literal::String("test-cbu".to_string())),
                        span: Span::new(10, 30),
                    },
                    // entity-id as a list - each becomes EntityRef after enrichment
                    Argument {
                        key: "entity-id".to_string(),
                        value: AstNode::List {
                            items: vec![
                                AstNode::Literal(Literal::String("Allianz".to_string())),
                                AstNode::Literal(Literal::String("BlackRock".to_string())),
                                AstNode::Literal(Literal::String("Vanguard".to_string())),
                            ],
                            span: Span::new(40, 80),
                        },
                        span: Span::new(35, 85),
                    },
                    Argument {
                        key: "role".to_string(),
                        value: AstNode::Literal(Literal::String("DIRECTOR".to_string())),
                        span: Span::new(90, 110),
                    },
                ],
                binding: None,
                span: Span::new(0, 120),
            })],
        };

        // Step 2: Enrich to convert strings to EntityRef nodes
        let registry = runtime_registry_arc();
        let enriched = enrich_program(raw, &registry);

        // Step 3: Get unresolved refs - should have 3 list items
        let refs_before = find_unresolved_ref_locations(&enriched.program);
        let list_refs: Vec<_> = refs_before
            .iter()
            .filter(|r| {
                r.search_text == "Allianz"
                    || r.search_text == "BlackRock"
                    || r.search_text == "Vanguard"
            })
            .collect();

        assert_eq!(
            list_refs.len(),
            3,
            "Should have 3 unresolved entity refs in list"
        );

        // Verify ref_ids are unique (required for Issue K)
        let ref_ids: HashSet<_> = list_refs.iter().filter_map(|r| r.ref_id.as_ref()).collect();
        assert_eq!(
            ref_ids.len(),
            3,
            "Each list item must have a unique ref_id for commit to work"
        );

        // Step 4: Commit resolution for just "Allianz" using its ref_id
        let allianz_ref = list_refs
            .iter()
            .find(|r| r.search_text == "Allianz")
            .expect("Should find Allianz ref");
        let allianz_ref_id = allianz_ref
            .ref_id
            .as_ref()
            .expect("Allianz should have ref_id");

        // Mutate the AST to commit the resolution by ref_id
        // The ref_id is unique even for list items (includes :list_index suffix)
        let mut program = enriched.program;
        let resolved =
            commit_entity_ref_by_ref_id(&mut program, allianz_ref_id, "uuid-allianz-resolved");
        assert!(
            resolved,
            "Should find and resolve Allianz EntityRef by ref_id: {}",
            allianz_ref_id
        );

        // Step 5: Verify only Allianz is resolved, BlackRock and Vanguard remain unresolved
        let refs_after = find_unresolved_ref_locations(&program);
        let remaining_list_refs: Vec<_> = refs_after
            .iter()
            .filter(|r| {
                r.search_text == "Allianz"
                    || r.search_text == "BlackRock"
                    || r.search_text == "Vanguard"
            })
            .collect();

        assert_eq!(
            remaining_list_refs.len(),
            2,
            "Should have 2 remaining unresolved refs after committing Allianz"
        );

        let remaining_names: HashSet<_> = remaining_list_refs
            .iter()
            .map(|r| r.search_text.as_str())
            .collect();
        assert!(
            !remaining_names.contains("Allianz"),
            "Allianz should be resolved (not in remaining)"
        );
        assert!(
            remaining_names.contains("BlackRock"),
            "BlackRock should still be unresolved"
        );
        assert!(
            remaining_names.contains("Vanguard"),
            "Vanguard should still be unresolved"
        );
    }

    /// Helper to commit resolution by ref_id (Issue K - handles lists/maps correctly)
    fn commit_entity_ref_by_ref_id(
        program: &mut dsl_core::ast::Program,
        target_ref_id: &str,
        resolved_key: &str,
    ) -> bool {
        use dsl_core::ast::Statement;
        for stmt in &mut program.statements {
            if let Statement::VerbCall(vc) = stmt {
                for arg in &mut vc.arguments {
                    if commit_node_by_ref_id(&mut arg.value, target_ref_id, resolved_key) {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn commit_node_by_ref_id(
        node: &mut dsl_core::ast::AstNode,
        target_ref_id: &str,
        resolved_key: &str,
    ) -> bool {
        use dsl_core::ast::AstNode;

        match node {
            AstNode::EntityRef {
                ref_id,
                resolved_key: ref mut existing,
                ..
            } => {
                if ref_id.as_deref() == Some(target_ref_id) && existing.is_none() {
                    *existing = Some(resolved_key.to_string());
                    return true;
                }
                false
            }
            AstNode::List { items, .. } => {
                for item in items.iter_mut() {
                    if commit_node_by_ref_id(item, target_ref_id, resolved_key) {
                        return true;
                    }
                }
                false
            }
            AstNode::Map { entries, .. } => {
                for (_, value) in entries.iter_mut() {
                    if commit_node_by_ref_id(value, target_ref_id, resolved_key) {
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }
}
