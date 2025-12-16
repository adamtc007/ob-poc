//! Resolution Service for Entity Reference Resolution
//!
//! Manages the lifecycle of entity resolution sessions where users/agents
//! disambiguate and confirm entity references in DSL.
//!
//! ## Architecture
//!
//! ```text
//! Session DSL (with EntityRefs) → ResolutionSession → Resolved AST
//!                                       ↑
//!                        EntityGateway (search/lookup)
//! ```
//!
//! ## Flow
//!
//! 1. `start_resolution()` - Extract unresolved refs from session's AST
//! 2. `search()` - Search for matches with discriminators
//! 3. `select()` - User/agent selects a resolution
//! 4. `commit()` - Apply resolutions to AST, enable execution

use crate::dsl_v2::ast::{find_unresolved_ref_locations, AstNode, Program, Statement};
use crate::dsl_v2::gateway_resolver::GatewayRefResolver;
use crate::dsl_v2::validation::RefType;
use anyhow::{bail, Context, Result};
use ob_poc_types::resolution::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

// ============================================================================
// CORE TYPES (SERVER-SIDE)
// ============================================================================

/// Server-side resolution session state
#[derive(Debug, Clone)]
pub struct ResolutionSession {
    /// Resolution session ID
    pub id: Uuid,
    /// Parent session ID
    pub session_id: Uuid,
    /// Current state
    pub state: ResolutionState,
    /// Refs needing resolution
    pub unresolved: Vec<UnresolvedRef>,
    /// Auto-resolved refs (exact match, reference data)
    pub auto_resolved: Vec<ResolvedRef>,
    /// User resolutions (confirmed)
    pub resolved: HashMap<String, ResolvedRef>,
    /// The original AST (for applying resolutions)
    pub original_ast: Vec<Statement>,
}

/// Server-side resolution state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionState {
    /// User picking entities
    Resolving,
    /// All resolved, user reviewing
    Reviewing,
    /// Applied to AST
    Committed,
    /// Cancelled
    Cancelled,
}

/// Server-side unresolved reference
#[derive(Debug, Clone)]
pub struct UnresolvedRef {
    /// Unique ref ID within this resolution session
    pub ref_id: String,
    /// Entity type (e.g., "cbu", "entity", "proper_person")
    pub entity_type: String,
    /// Entity subtype if applicable
    pub entity_subtype: Option<String>,
    /// Original search value from DSL
    pub search_value: String,
    /// Context about where this ref appears
    pub context: RefContextInternal,
    /// Pre-fetched initial matches
    pub initial_matches: Vec<EntityMatchInternal>,
    /// Agent's suggested resolution (if confident)
    pub agent_suggestion: Option<EntityMatchInternal>,
    /// Reason for agent's suggestion
    pub suggestion_reason: Option<String>,
    /// Review requirement level
    pub review_requirement: ReviewRequirement,
    /// Discriminator fields from search schema
    pub discriminator_fields: Vec<DiscriminatorFieldInternal>,
}

/// Context about where a reference appears in DSL
#[derive(Debug, Clone)]
pub struct RefContextInternal {
    /// Statement index in DSL
    pub statement_index: usize,
    /// Verb (e.g., "cbu.assign-role")
    pub verb: String,
    /// Argument name (e.g., "entity-id")
    pub arg_name: String,
    /// DSL snippet for context
    pub dsl_snippet: Option<String>,
}

/// Server-side resolved reference
#[derive(Debug, Clone)]
pub struct ResolvedRef {
    /// Unique ref ID
    pub ref_id: String,
    /// Entity type
    pub entity_type: String,
    /// Original search value
    pub original_search: String,
    /// Resolved primary key
    pub resolved_key: Uuid,
    /// Display name of resolved entity
    pub display: String,
    /// Key discriminators for display
    pub discriminators: HashMap<String, String>,
    /// Entity status
    pub entity_status: EntityStatus,
    /// Warnings about this resolution
    pub warnings: Vec<ResolutionWarning>,
    /// Number of alternative matches
    pub alternative_count: usize,
    /// Confidence score
    pub confidence: f32,
    /// Has user reviewed this resolution
    pub reviewed: bool,
    /// Was this changed from initial suggestion
    pub changed_from_original: bool,
    /// How this was resolved
    pub resolution_method: ResolutionMethod,
}

/// Server-side entity match
#[derive(Debug, Clone)]
pub struct EntityMatchInternal {
    /// Entity ID
    pub id: Uuid,
    /// Display name
    pub display: String,
    /// Entity type
    pub entity_type: String,
    /// Match score
    pub score: f32,
    /// Key discriminators
    pub discriminators: HashMap<String, String>,
    /// Entity status
    pub status: EntityStatus,
    /// Additional context
    pub context: Option<String>,
}

/// Server-side discriminator field
#[derive(Debug, Clone)]
pub struct DiscriminatorFieldInternal {
    /// Field name
    pub name: String,
    /// Display label
    pub label: String,
    /// Selectivity
    pub selectivity: f32,
    /// Current value if known
    pub value: Option<String>,
}

// ============================================================================
// RESOLUTION SESSION STORE
// ============================================================================

/// In-memory store for resolution sessions
pub type ResolutionStore = Arc<RwLock<HashMap<Uuid, ResolutionSession>>>;

/// Create a new resolution store
pub fn create_resolution_store() -> ResolutionStore {
    Arc::new(RwLock::new(HashMap::new()))
}

// ============================================================================
// RESOLUTION SERVICE
// ============================================================================

/// Service for managing entity resolution
pub struct ResolutionService {
    store: ResolutionStore,
    /// EntityGateway address for search
    gateway_addr: String,
}

impl ResolutionService {
    /// Create a new resolution service
    pub fn new(store: ResolutionStore) -> Self {
        Self::with_gateway(store, crate::dsl_v2::gateway_resolver::gateway_addr())
    }

    /// Create with specific gateway address
    pub fn with_gateway(store: ResolutionStore, gateway_addr: String) -> Self {
        Self {
            store,
            gateway_addr,
        }
    }

    /// Map entity_type from DSL/verb YAML to EntityGateway nickname
    ///
    /// The verb YAML defines `lookup.entity_type` which must map to
    /// EntityGateway's `nickname` field (uppercase in entity_index.yaml).
    #[allow(dead_code)] // Reserved for future EntityGateway integration
    fn entity_type_to_nickname(entity_type: &str) -> String {
        // Convert to uppercase and handle special mappings
        match entity_type.to_lowercase().as_str() {
            // Core entities
            "cbu" => "CBU".to_string(),
            "entity" => "ENTITY".to_string(),
            "proper_person" | "person" => "PERSON".to_string(),
            "limited_company" | "legal_entity" => "LEGAL_ENTITY".to_string(),
            "fund" => "FUND".to_string(),

            // Reference data
            "jurisdiction" => "JURISDICTION".to_string(),
            "role" => "ROLE".to_string(),
            "currency" => "CURRENCY".to_string(),
            "client_type" => "CLIENT_TYPE".to_string(),
            "case_type" => "CASE_TYPE".to_string(),
            "screening_type" => "SCREENING_TYPE".to_string(),
            "risk_rating" => "RISK_RATING".to_string(),
            "settlement_type" => "SETTLEMENT_TYPE".to_string(),
            "ssi_type" => "SSI_TYPE".to_string(),

            // Documents & Attributes
            "document_type" => "DOCUMENT_TYPE".to_string(),
            "document" => "DOCUMENT".to_string(),
            "entity_type" => "ENTITY_TYPE".to_string(),
            "attribute" => "ATTRIBUTE".to_string(),

            // Services & Products
            "product" => "PRODUCT".to_string(),
            "service" => "SERVICE".to_string(),
            "resource_type" => "RESOURCE_TYPE".to_string(),

            // Custody
            "instrument_class" => "INSTRUMENT_CLASS".to_string(),
            "market" => "MARKET".to_string(),
            "ssi" => "SSI".to_string(),
            "booking_rule" => "BOOKING_RULE".to_string(),

            // Investor Registry
            "share_class" => "SHARE_CLASS".to_string(),

            // Default: uppercase the entity_type
            other => other.to_uppercase(),
        }
    }

    /// Map entity_type to RefType for gateway resolution
    fn entity_type_to_ref_type(entity_type: &str) -> RefType {
        match entity_type.to_lowercase().as_str() {
            "cbu" => RefType::Cbu,
            "entity" | "proper_person" | "person" | "limited_company" | "legal_entity" | "fund" => {
                RefType::Entity
            }
            "jurisdiction" => RefType::Jurisdiction,
            "role" => RefType::Role,
            "currency" => RefType::Currency,
            "client_type" => RefType::ClientType,
            "document_type" => RefType::DocumentType,
            "document" => RefType::Document,
            "entity_type" => RefType::EntityType,
            "attribute" => RefType::AttributeId,
            "product" => RefType::Product,
            "service" => RefType::Service,
            "screening_type" => RefType::ScreeningType,
            // Default to Entity for unknown types
            _ => RefType::Entity,
        }
    }

    /// Connect to EntityGateway and perform search
    async fn search_via_gateway(
        &self,
        entity_type: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<EntityMatchInternal>> {
        let ref_type = Self::entity_type_to_ref_type(entity_type);

        // Connect to gateway
        let mut resolver = GatewayRefResolver::connect(&self.gateway_addr)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to connect to EntityGateway: {}", e))?;

        // Perform fuzzy search
        let matches = resolver
            .search_fuzzy(ref_type, query, limit)
            .await
            .map_err(|e| anyhow::anyhow!("EntityGateway search failed: {}", e))?;

        // Convert to internal match type
        let results = matches
            .into_iter()
            .map(|m| {
                // Try to parse token as UUID, fall back to generating a deterministic one for ref data
                let id = Uuid::parse_str(&m.value).unwrap_or_else(|_| {
                    // For reference data (roles, jurisdictions), use a deterministic UUID
                    // based on the value to enable re-selection
                    // Use v4 random UUID namespace-style hash
                    use std::hash::{Hash, Hasher};
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    m.value.hash(&mut hasher);
                    entity_type.hash(&mut hasher);
                    let hash = hasher.finish();
                    // Create a UUID from the hash bytes
                    Uuid::from_u128(hash as u128 | ((hash as u128) << 64))
                });

                EntityMatchInternal {
                    id,
                    display: m.display,
                    entity_type: entity_type.to_string(),
                    score: m.score,
                    discriminators: std::collections::HashMap::new(),
                    status: EntityStatus::Active,
                    context: Some(m.value), // Store original token for ref data
                }
            })
            .collect();

        Ok(results)
    }

    /// Start a resolution session for a session's AST
    ///
    /// Extracts all unresolved EntityRefs and pre-fetches initial matches.
    pub async fn start_resolution(
        &self,
        session_id: Uuid,
        ast: &[Statement],
    ) -> Result<ResolutionSession> {
        // Convert AST to Program for extraction
        let program = Program {
            statements: ast.to_vec(),
        };

        // Extract unresolved refs
        let unresolved_locations = find_unresolved_ref_locations(&program);

        if unresolved_locations.is_empty() {
            // No unresolved refs - create empty session in Committed state
            let session = ResolutionSession {
                id: Uuid::new_v4(),
                session_id,
                state: ResolutionState::Committed,
                unresolved: vec![],
                auto_resolved: vec![],
                resolved: HashMap::new(),
                original_ast: ast.to_vec(),
            };
            return Ok(session);
        }

        // Convert to UnresolvedRef with context, pre-fetching initial matches
        let mut unresolved_refs = Vec::new();
        for (idx, loc) in unresolved_locations.iter().enumerate() {
            let ref_id = format!("ref-{}", idx);

            // Get the verb from the statement
            let verb = if let Some(Statement::VerbCall(vc)) = ast.get(loc.statement_index) {
                format!("{}.{}", vc.domain, vc.verb)
            } else {
                "unknown".to_string()
            };

            // Pre-fetch initial matches via EntityGateway
            let initial_matches = match self
                .search_via_gateway(&loc.entity_type, &loc.search_text, 10)
                .await
            {
                Ok(matches) => matches,
                Err(e) => {
                    tracing::warn!(
                        "Failed to pre-fetch matches for {} '{}': {}",
                        loc.entity_type,
                        loc.search_text,
                        e
                    );
                    Vec::new()
                }
            };

            // Determine review requirement based on match quality
            let (review_requirement, agent_suggestion, suggestion_reason) =
                if initial_matches.is_empty() {
                    (ReviewRequirement::Required, None, None)
                } else if initial_matches.len() == 1 && initial_matches[0].score > 0.95 {
                    // High-confidence single match - can auto-resolve with optional review
                    let suggestion = initial_matches[0].clone();
                    (
                        ReviewRequirement::Optional,
                        Some(suggestion),
                        Some("Exact match found".to_string()),
                    )
                } else if initial_matches[0].score > 0.85 {
                    // Good match but should be reviewed
                    let suggestion = initial_matches[0].clone();
                    (
                        ReviewRequirement::Recommended,
                        Some(suggestion),
                        Some(format!(
                            "Best match (score: {:.0}%)",
                            initial_matches[0].score * 100.0
                        )),
                    )
                } else {
                    // Low confidence - require review
                    (ReviewRequirement::Required, None, None)
                };

            unresolved_refs.push(UnresolvedRef {
                ref_id,
                entity_type: loc.entity_type.clone(),
                entity_subtype: None,
                search_value: loc.search_text.clone(),
                context: RefContextInternal {
                    statement_index: loc.statement_index,
                    verb,
                    arg_name: loc.arg_key.clone(),
                    dsl_snippet: None,
                },
                initial_matches,
                agent_suggestion,
                suggestion_reason,
                review_requirement,
                discriminator_fields: vec![],
            });
        }

        // Create session
        let resolution_id = Uuid::new_v4();
        let session = ResolutionSession {
            id: resolution_id,
            session_id,
            state: ResolutionState::Resolving,
            unresolved: unresolved_refs,
            auto_resolved: vec![],
            resolved: HashMap::new(),
            original_ast: ast.to_vec(),
        };

        // Store session
        {
            let mut store = self.store.write().await;
            store.insert(resolution_id, session.clone());
        }

        Ok(session)
    }

    /// Get a resolution session by ID
    pub async fn get_session(&self, resolution_id: Uuid) -> Result<ResolutionSession> {
        let store = self.store.read().await;
        store
            .get(&resolution_id)
            .cloned()
            .context("Resolution session not found")
    }

    /// Search for entity matches
    pub async fn search(
        &self,
        resolution_id: Uuid,
        ref_id: &str,
        query: &str,
        discriminators: &HashMap<String, String>,
        limit: Option<usize>,
    ) -> Result<Vec<EntityMatchInternal>> {
        let session = self.get_session(resolution_id).await?;

        // Find the ref
        let unresolved = session
            .unresolved
            .iter()
            .find(|r| r.ref_id == ref_id)
            .context("Reference not found")?;

        let search_limit = limit.unwrap_or(20);

        // Search via EntityGateway
        // TODO: Apply discriminators to narrow search (future enhancement)
        // For now, discriminators are logged but not used in search
        if !discriminators.is_empty() {
            tracing::debug!(
                "Search discriminators provided but not yet implemented: {:?}",
                discriminators
            );
        }

        self.search_via_gateway(&unresolved.entity_type, query, search_limit)
            .await
    }

    /// Select a resolution for a reference
    pub async fn select(
        &self,
        resolution_id: Uuid,
        ref_id: &str,
        entity_match: EntityMatchInternal,
    ) -> Result<ResolutionSession> {
        let mut store = self.store.write().await;
        let session = store
            .get_mut(&resolution_id)
            .context("Resolution session not found")?;

        if session.state != ResolutionState::Resolving
            && session.state != ResolutionState::Reviewing
        {
            bail!("Cannot select resolution in state {:?}", session.state);
        }

        // Find the unresolved ref
        let unresolved = session
            .unresolved
            .iter()
            .find(|r| r.ref_id == ref_id)
            .context("Reference not found")?
            .clone();

        // Create resolved ref
        let resolved = ResolvedRef {
            ref_id: ref_id.to_string(),
            entity_type: unresolved.entity_type.clone(),
            original_search: unresolved.search_value.clone(),
            resolved_key: entity_match.id,
            display: entity_match.display.clone(),
            discriminators: entity_match.discriminators.clone(),
            entity_status: entity_match.status.clone(),
            warnings: vec![],
            alternative_count: unresolved.initial_matches.len(),
            confidence: entity_match.score,
            reviewed: false,
            changed_from_original: false,
            resolution_method: ResolutionMethod::UserSelected,
        };

        // Move from unresolved to resolved
        session.unresolved.retain(|r| r.ref_id != ref_id);
        session.resolved.insert(ref_id.to_string(), resolved);

        // Check if all resolved -> transition to Reviewing
        if session.unresolved.is_empty() {
            session.state = ResolutionState::Reviewing;
        }

        Ok(session.clone())
    }

    /// Confirm a resolution (mark as reviewed)
    pub async fn confirm(&self, resolution_id: Uuid, ref_id: &str) -> Result<ResolutionSession> {
        let mut store = self.store.write().await;
        let session = store
            .get_mut(&resolution_id)
            .context("Resolution session not found")?;

        if let Some(resolved) = session.resolved.get_mut(ref_id) {
            resolved.reviewed = true;
        }

        Ok(session.clone())
    }

    /// Confirm all high-confidence resolutions
    pub async fn confirm_all(
        &self,
        resolution_id: Uuid,
        min_confidence: Option<f32>,
    ) -> Result<ResolutionSession> {
        let threshold = min_confidence.unwrap_or(0.9);

        let mut store = self.store.write().await;
        let session = store
            .get_mut(&resolution_id)
            .context("Resolution session not found")?;

        for resolved in session.resolved.values_mut() {
            if resolved.confidence >= threshold {
                resolved.reviewed = true;
            }
        }

        Ok(session.clone())
    }

    /// Commit resolutions to the AST
    ///
    /// Returns the updated AST with resolved EntityRefs.
    pub async fn commit(&self, resolution_id: Uuid) -> Result<Vec<Statement>> {
        let mut store = self.store.write().await;
        let session = store
            .get_mut(&resolution_id)
            .context("Resolution session not found")?;

        // Check all refs are resolved
        if !session.unresolved.is_empty() {
            bail!(
                "Cannot commit: {} refs still unresolved",
                session.unresolved.len()
            );
        }

        // Build resolved AST
        let mut resolved_ast = session.original_ast.clone();

        // Apply resolutions
        for resolved in session.resolved.values() {
            // Find the statement and arg
            for stmt in &mut resolved_ast {
                if let Statement::VerbCall(vc) = stmt {
                    for arg in &mut vc.arguments {
                        // Check if this arg matches the resolved ref
                        if let AstNode::EntityRef {
                            entity_type,
                            value,
                            resolved_key,
                            search_column,
                            span,
                        } = &arg.value
                        {
                            if entity_type == &resolved.entity_type
                                && value == &resolved.original_search
                                && resolved_key.is_none()
                            {
                                // Apply resolution
                                arg.value = AstNode::EntityRef {
                                    entity_type: entity_type.clone(),
                                    search_column: search_column.clone(),
                                    value: value.clone(),
                                    resolved_key: Some(resolved.resolved_key.to_string()),
                                    span: *span,
                                };
                            }
                        }
                    }
                }
            }
        }

        // Update state
        session.state = ResolutionState::Committed;

        Ok(resolved_ast)
    }

    /// Cancel a resolution session
    pub async fn cancel(&self, resolution_id: Uuid) -> Result<()> {
        let mut store = self.store.write().await;
        let session = store
            .get_mut(&resolution_id)
            .context("Resolution session not found")?;

        session.state = ResolutionState::Cancelled;
        Ok(())
    }
}

// ============================================================================
// TYPE CONVERSIONS (Server → API Response)
// ============================================================================

impl From<&ResolutionState> for ResolutionStateResponse {
    fn from(state: &ResolutionState) -> Self {
        match state {
            ResolutionState::Resolving => ResolutionStateResponse::Resolving,
            ResolutionState::Reviewing => ResolutionStateResponse::Reviewing,
            ResolutionState::Committed => ResolutionStateResponse::Committed,
            ResolutionState::Cancelled => ResolutionStateResponse::Cancelled,
        }
    }
}

impl From<&UnresolvedRef> for UnresolvedRefResponse {
    fn from(r: &UnresolvedRef) -> Self {
        UnresolvedRefResponse {
            ref_id: r.ref_id.clone(),
            entity_type: r.entity_type.clone(),
            entity_subtype: r.entity_subtype.clone(),
            search_value: r.search_value.clone(),
            context: RefContext {
                statement_index: r.context.statement_index,
                verb: r.context.verb.clone(),
                arg_name: r.context.arg_name.clone(),
                dsl_snippet: r.context.dsl_snippet.clone(),
            },
            initial_matches: r.initial_matches.iter().map(|m| m.into()).collect(),
            agent_suggestion: r.agent_suggestion.as_ref().map(|m| m.into()),
            suggestion_reason: r.suggestion_reason.clone(),
            review_requirement: r.review_requirement.clone(),
            discriminator_fields: r
                .discriminator_fields
                .iter()
                .map(|f| DiscriminatorField {
                    name: f.name.clone(),
                    label: f.label.clone(),
                    selectivity: f.selectivity,
                    value: f.value.clone(),
                })
                .collect(),
        }
    }
}

impl From<&ResolvedRef> for ResolvedRefResponse {
    fn from(r: &ResolvedRef) -> Self {
        ResolvedRefResponse {
            ref_id: r.ref_id.clone(),
            entity_type: r.entity_type.clone(),
            original_search: r.original_search.clone(),
            resolved_key: r.resolved_key.to_string(),
            display: r.display.clone(),
            discriminators: r.discriminators.clone(),
            entity_status: r.entity_status.clone(),
            warnings: r.warnings.clone(),
            alternative_count: r.alternative_count,
            confidence: r.confidence,
            reviewed: r.reviewed,
            changed_from_original: r.changed_from_original,
            resolution_method: r.resolution_method.clone(),
        }
    }
}

impl From<&EntityMatchInternal> for EntityMatchResponse {
    fn from(m: &EntityMatchInternal) -> Self {
        EntityMatchResponse {
            id: m.id.to_string(),
            display: m.display.clone(),
            entity_type: m.entity_type.clone(),
            score: m.score,
            discriminators: m.discriminators.clone(),
            status: m.status.clone(),
            context: m.context.clone(),
        }
    }
}

impl ResolutionSession {
    /// Convert to API response
    pub fn to_response(&self) -> ResolutionSessionResponse {
        let resolved_count = self.resolved.len() + self.auto_resolved.len();
        let total_refs = resolved_count + self.unresolved.len();

        let warnings_count = self
            .resolved
            .values()
            .filter(|r| !r.warnings.is_empty())
            .count();

        let required_review_count = self
            .unresolved
            .iter()
            .filter(|r| r.review_requirement == ReviewRequirement::Required)
            .count()
            + self
                .resolved
                .values()
                .filter(|r| {
                    !r.reviewed
                        && r.warnings
                            .iter()
                            .any(|w| w.severity == WarningSeverity::Error)
                })
                .count();

        ResolutionSessionResponse {
            id: self.session_id.to_string(),
            resolution_id: self.id.to_string(),
            state: (&self.state).into(),
            unresolved: self.unresolved.iter().map(|r| r.into()).collect(),
            auto_resolved: self.auto_resolved.iter().map(|r| r.into()).collect(),
            resolved: self.resolved.values().map(|r| r.into()).collect(),
            summary: ResolutionSummary::new(
                total_refs,
                resolved_count,
                warnings_count,
                required_review_count,
            ),
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl_v2::ast::{Argument, Span, VerbCall};

    fn make_test_ast_with_unresolved_ref() -> Vec<Statement> {
        vec![Statement::VerbCall(VerbCall {
            domain: "cbu".to_string(),
            verb: "assign-role".to_string(),
            arguments: vec![
                Argument {
                    key: "cbu-id".to_string(),
                    value: AstNode::entity_ref("cbu", "name", "Test Fund", Span::new(0, 10)),
                    span: Span::new(0, 20),
                },
                Argument {
                    key: "entity-id".to_string(),
                    value: AstNode::entity_ref("entity", "name", "John Smith", Span::new(21, 35)),
                    span: Span::new(21, 45),
                },
            ],
            binding: None,
            span: Span::new(0, 50),
        })]
    }

    #[tokio::test]
    async fn test_start_resolution_extracts_refs() {
        let store = create_resolution_store();
        let service = ResolutionService::new(store);

        let ast = make_test_ast_with_unresolved_ref();
        let session = service
            .start_resolution(Uuid::new_v4(), &ast)
            .await
            .unwrap();

        assert_eq!(session.state, ResolutionState::Resolving);
        assert_eq!(session.unresolved.len(), 2);
        assert_eq!(session.unresolved[0].entity_type, "cbu");
        assert_eq!(session.unresolved[0].search_value, "Test Fund");
        assert_eq!(session.unresolved[1].entity_type, "entity");
        assert_eq!(session.unresolved[1].search_value, "John Smith");
    }

    #[tokio::test]
    async fn test_start_resolution_empty_ast() {
        let store = create_resolution_store();
        let service = ResolutionService::new(store);

        let ast: Vec<Statement> = vec![];
        let session = service
            .start_resolution(Uuid::new_v4(), &ast)
            .await
            .unwrap();

        assert_eq!(session.state, ResolutionState::Committed);
        assert!(session.unresolved.is_empty());
    }

    #[tokio::test]
    async fn test_select_resolution() {
        let store = create_resolution_store();
        let service = ResolutionService::new(store);

        let ast = make_test_ast_with_unresolved_ref();
        let session = service
            .start_resolution(Uuid::new_v4(), &ast)
            .await
            .unwrap();
        let resolution_id = session.id;

        // Select first ref
        let entity_match = EntityMatchInternal {
            id: Uuid::new_v4(),
            display: "Test Fund".to_string(),
            entity_type: "cbu".to_string(),
            score: 0.95,
            discriminators: HashMap::new(),
            status: EntityStatus::Active,
            context: None,
        };

        let session = service
            .select(resolution_id, "ref-0", entity_match)
            .await
            .unwrap();

        assert_eq!(session.unresolved.len(), 1);
        assert_eq!(session.resolved.len(), 1);
        assert!(session.resolved.contains_key("ref-0"));
        assert_eq!(session.state, ResolutionState::Resolving);
    }

    #[tokio::test]
    async fn test_select_all_transitions_to_reviewing() {
        let store = create_resolution_store();
        let service = ResolutionService::new(store);

        let ast = make_test_ast_with_unresolved_ref();
        let session = service
            .start_resolution(Uuid::new_v4(), &ast)
            .await
            .unwrap();
        let resolution_id = session.id;

        // Select first ref
        let match1 = EntityMatchInternal {
            id: Uuid::new_v4(),
            display: "Test Fund".to_string(),
            entity_type: "cbu".to_string(),
            score: 0.95,
            discriminators: HashMap::new(),
            status: EntityStatus::Active,
            context: None,
        };
        service
            .select(resolution_id, "ref-0", match1)
            .await
            .unwrap();

        // Select second ref
        let match2 = EntityMatchInternal {
            id: Uuid::new_v4(),
            display: "John Smith".to_string(),
            entity_type: "entity".to_string(),
            score: 0.90,
            discriminators: HashMap::new(),
            status: EntityStatus::Active,
            context: None,
        };
        let session = service
            .select(resolution_id, "ref-1", match2)
            .await
            .unwrap();

        assert!(session.unresolved.is_empty());
        assert_eq!(session.resolved.len(), 2);
        assert_eq!(session.state, ResolutionState::Reviewing);
    }

    #[tokio::test]
    async fn test_commit_applies_resolutions() {
        let store = create_resolution_store();
        let service = ResolutionService::new(store);

        let ast = make_test_ast_with_unresolved_ref();
        let session = service
            .start_resolution(Uuid::new_v4(), &ast)
            .await
            .unwrap();
        let resolution_id = session.id;

        // Resolve both refs
        let cbu_id = Uuid::new_v4();
        let entity_id = Uuid::new_v4();

        service
            .select(
                resolution_id,
                "ref-0",
                EntityMatchInternal {
                    id: cbu_id,
                    display: "Test Fund".to_string(),
                    entity_type: "cbu".to_string(),
                    score: 1.0,
                    discriminators: HashMap::new(),
                    status: EntityStatus::Active,
                    context: None,
                },
            )
            .await
            .unwrap();

        service
            .select(
                resolution_id,
                "ref-1",
                EntityMatchInternal {
                    id: entity_id,
                    display: "John Smith".to_string(),
                    entity_type: "entity".to_string(),
                    score: 1.0,
                    discriminators: HashMap::new(),
                    status: EntityStatus::Active,
                    context: None,
                },
            )
            .await
            .unwrap();

        // Commit
        let resolved_ast = service.commit(resolution_id).await.unwrap();

        // Check that EntityRefs are now resolved
        if let Statement::VerbCall(vc) = &resolved_ast[0] {
            if let AstNode::EntityRef { resolved_key, .. } = &vc.arguments[0].value {
                assert_eq!(resolved_key, &Some(cbu_id.to_string()));
            } else {
                panic!("Expected EntityRef");
            }
            if let AstNode::EntityRef { resolved_key, .. } = &vc.arguments[1].value {
                assert_eq!(resolved_key, &Some(entity_id.to_string()));
            } else {
                panic!("Expected EntityRef");
            }
        } else {
            panic!("Expected VerbCall");
        }
    }

    #[tokio::test]
    async fn test_commit_fails_with_unresolved() {
        let store = create_resolution_store();
        let service = ResolutionService::new(store);

        let ast = make_test_ast_with_unresolved_ref();
        let session = service
            .start_resolution(Uuid::new_v4(), &ast)
            .await
            .unwrap();

        // Try to commit without resolving
        let result = service.commit(session.id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_to_response() {
        let store = create_resolution_store();
        let service = ResolutionService::new(store);

        let ast = make_test_ast_with_unresolved_ref();
        let session = service
            .start_resolution(Uuid::new_v4(), &ast)
            .await
            .unwrap();

        let response = session.to_response();

        assert_eq!(response.state, ResolutionStateResponse::Resolving);
        assert_eq!(response.unresolved.len(), 2);
        assert_eq!(response.summary.total_refs, 2);
        assert_eq!(response.summary.resolved_count, 0);
        assert!(!response.summary.can_commit);
    }
}
