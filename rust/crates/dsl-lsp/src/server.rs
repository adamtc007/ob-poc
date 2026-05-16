//! LSP Server implementation for the Onboarding DSL.
//!
//! Consumed only by the `dsl-lsp` binary (`main.rs`). The library target
//! (`lib.rs`) exposes `pub mod server` for source discoverability but
//! has no reachable consumer of `DslLanguageServer`, hence the
//! `#[allow(dead_code)]` blanket on this file.

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tokio::time::Duration;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use crate::analysis::{DocumentState, SymbolTable};
use crate::encoding::{position_to_offset, PositionEncoding};
use crate::entity_client::{gateway_addr, EntityLookupClient};
use crate::handlers;
use dsl_analysis::planning_facade::PlanningOutput;
use dsl_analysis::validation::Diagnostic as SemanticDiagnostic;

/// File type detection for dispatch
enum FileType {
    Dsl,
    Playbook,
    Unknown,
}

fn file_type(uri: &Url) -> FileType {
    let path = uri.path();
    if path.ends_with(".playbook.yaml") || path.ends_with(".playbook.yml") {
        FileType::Playbook
    } else if path.ends_with(".dsl") {
        FileType::Dsl
    } else {
        FileType::Unknown
    }
}

/// DSL Language Server state.
pub(crate) struct DslLanguageServer {
    /// LSP client for sending notifications
    client: Client,
    /// Open documents and their state
    documents: Arc<RwLock<HashMap<Url, DocumentState>>>,
    /// Planning output for each document (for code actions)
    planning_outputs: Arc<RwLock<HashMap<Url, PlanningOutput>>>,
    /// Semantic diagnostics for each document (for entity suggestion code actions)
    semantic_diagnostics: Arc<RwLock<HashMap<Url, Vec<SemanticDiagnostic>>>>,
    /// Session symbol table (shared across documents)
    symbols: Arc<RwLock<SymbolTable>>,
    /// Entity Gateway client for lookups (replaces direct DB access)
    entity_client: Arc<RwLock<Option<EntityLookupClient>>>,
    /// Pending changes for debouncing (uri -> timestamp)
    pending_changes: Arc<RwLock<HashMap<Url, Instant>>>,
}

impl DslLanguageServer {
    /// Create a new language server instance.
    pub(crate) fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(RwLock::new(HashMap::new())),
            planning_outputs: Arc::new(RwLock::new(HashMap::new())),
            semantic_diagnostics: Arc::new(RwLock::new(HashMap::new())),
            symbols: Arc::new(RwLock::new(SymbolTable::new())),
            entity_client: Arc::new(RwLock::new(None)),
            pending_changes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Initialize EntityGateway connection (called from initialized())
    async fn init_entity_gateway(&self) {
        let addr = gateway_addr();
        tracing::info!("Connecting to EntityGateway at {}", addr);

        match EntityLookupClient::connect(&addr).await {
            Ok(client) => {
                tracing::info!("Connected to EntityGateway");
                *self.entity_client.write().await = Some(client);
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to connect to EntityGateway: {}. Lookups will be unavailable.",
                    e
                );
            }
        }
    }

    /// Get the entity client (if connected)
    pub(crate) async fn get_entity_client(&self) -> Option<EntityLookupClient> {
        if let Some(client) = self.entity_client.read().await.clone() {
            return Some(client);
        }

        let addr = gateway_addr();
        let client = EntityLookupClient::connect(&addr).await.ok()?;
        *self.entity_client.write().await = Some(client.clone());
        Some(client)
    }

    /// Get a document by URL.
    pub(crate) async fn get_document(&self, uri: &Url) -> Option<DocumentState> {
        self.documents.read().await.get(uri).cloned()
    }

    /// Update diagnostics for a document.
    async fn publish_diagnostics(&self, uri: Url, diagnostics: Vec<Diagnostic>) {
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }

    /// Analyze a document and publish diagnostics.
    async fn analyze_document(&self, uri: &Url, text: &str) {
        match file_type(uri) {
            FileType::Playbook => {
                let diagnostics = handlers::playbook::analyze_playbook(text).await;
                self.publish_diagnostics(uri.clone(), diagnostics).await;
                return;
            }
            FileType::Unknown => {
                // No analysis for unknown file types
                return;
            }
            FileType::Dsl => {
                // Continue with DSL analysis below
            }
        }

        // Use full analysis to get planning output for code actions
        let result = handlers::diagnostics::analyze_document_full(text).await;

        // Store document state
        {
            let mut docs = self.documents.write().await;
            docs.insert(uri.clone(), result.state.clone());
        }

        // Store planning output for code actions
        {
            let mut planning = self.planning_outputs.write().await;
            planning.insert(uri.clone(), result.planning_output);
        }

        // Store semantic diagnostics for entity suggestion code actions
        {
            let mut sem_diags = self.semantic_diagnostics.write().await;
            sem_diags.insert(uri.clone(), result.semantic_diagnostics);
        }

        // Update symbol table from this document
        {
            let mut symbols = self.symbols.write().await;
            symbols.merge_from_document(uri, &result.state);
        }

        // Publish diagnostics
        self.publish_diagnostics(uri.clone(), result.diagnostics)
            .await;
    }

    /// Get planning output for a document (for code actions)
    pub(crate) async fn get_planning_output(&self, uri: &Url) -> Option<PlanningOutput> {
        self.planning_outputs.read().await.get(uri).cloned()
    }

    /// Get semantic diagnostics for a document (for entity suggestion code actions)
    pub(crate) async fn get_semantic_diagnostics(&self, uri: &Url) -> Vec<SemanticDiagnostic> {
        self.semantic_diagnostics
            .read()
            .await
            .get(uri)
            .cloned()
            .unwrap_or_default()
    }
    /// Static version of analyze_document for use in spawned tasks
    async fn analyze_document_static(
        uri: &Url,
        text: &str,
        documents: &Arc<RwLock<HashMap<Url, DocumentState>>>,
        planning_outputs: &Arc<RwLock<HashMap<Url, PlanningOutput>>>,
        semantic_diagnostics: &Arc<RwLock<HashMap<Url, Vec<SemanticDiagnostic>>>>,
        symbols: &Arc<RwLock<SymbolTable>>,
        client: &Client,
    ) {
        match file_type(uri) {
            FileType::Playbook => {
                let diagnostics = handlers::playbook::analyze_playbook(text).await;
                client
                    .publish_diagnostics(uri.clone(), diagnostics, None)
                    .await;
                return;
            }
            FileType::Unknown => {
                return;
            }
            FileType::Dsl => {}
        }

        let result = handlers::diagnostics::analyze_document_full(text).await;

        // Store document state
        {
            let mut docs = documents.write().await;
            docs.insert(uri.clone(), result.state.clone());
        }

        // Store planning output for code actions
        {
            let mut planning = planning_outputs.write().await;
            planning.insert(uri.clone(), result.planning_output);
        }

        // Store semantic diagnostics for entity suggestion code actions
        {
            let mut sem_diag = semantic_diagnostics.write().await;
            sem_diag.insert(uri.clone(), result.semantic_diagnostics);
        }

        // Update symbol table from this document
        {
            let mut symbols = symbols.write().await;
            symbols.merge_from_document(uri, &result.state);
        }

        // Publish diagnostics
        client
            .publish_diagnostics(uri.clone(), result.diagnostics, None)
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for DslLanguageServer {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        tracing::info!("Initializing DSL Language Server");

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                // Incremental sync for efficiency
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::INCREMENTAL,
                )),

                // Completion support
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![
                        ":".to_string(), // Keywords
                        "@".to_string(), // Symbols/entity refs
                        "$".to_string(), // Runbook placeholder aliases
                        "(".to_string(), // S-expressions (verbs)
                        " ".to_string(), // After keyword (for values)
                    ]),
                    ..Default::default()
                }),

                // Hover support
                hover_provider: Some(HoverProviderCapability::Simple(true)),

                // Go to definition
                definition_provider: Some(OneOf::Left(true)),

                // Find references
                references_provider: Some(OneOf::Left(true)),

                // Signature help
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".to_string(), " ".to_string()]),
                    retrigger_characters: Some(vec![" ".to_string()]),
                    ..Default::default()
                }),

                // Document symbols (outline)
                document_symbol_provider: Some(OneOf::Left(true)),

                // Code actions (quick fixes, refactoring)
                code_action_provider: Some(CodeActionProviderCapability::Options(
                    CodeActionOptions {
                        code_action_kinds: Some(vec![
                            CodeActionKind::QUICKFIX,
                            CodeActionKind::REFACTOR_REWRITE,
                        ]),
                        ..Default::default()
                    },
                )),

                // Rename support (prepare + execute)
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: Default::default(),
                })),

                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "dsl-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        tracing::info!("DSL Language Server initialized");

        // Initialize EntityGateway connection
        self.init_entity_gateway().await;

        self.client
            .log_message(MessageType::INFO, "DSL Language Server ready")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        tracing::info!("Shutting down DSL Language Server");
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        tracing::debug!("Document opened: {}", params.text_document.uri);
        self.analyze_document(&params.text_document.uri, &params.text_document.text)
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        tracing::debug!("Document changed: {}", params.text_document.uri);

        let uri = params.text_document.uri.clone();

        // Get full text from incremental changes
        let text = if let Some(doc) = self.get_document(&uri).await {
            let mut text = doc.text.clone();
            for change in params.content_changes {
                if let Some(range) = change.range {
                    let start_offset = offset_from_position(&text, range.start);
                    let end_offset = offset_from_position(&text, range.end);
                    text.replace_range(start_offset..end_offset, &change.text);
                } else {
                    text = change.text;
                }
            }
            text
        } else {
            params
                .content_changes
                .into_iter()
                .last()
                .map(|c| c.text)
                .unwrap_or_default()
        };

        // Debounce: record timestamp and spawn delayed analysis
        let now = Instant::now();
        self.pending_changes.write().await.insert(uri.clone(), now);

        let pending = self.pending_changes.clone();
        let docs = self.documents.clone();
        let client = self.client.clone();
        let planning_outputs = self.planning_outputs.clone();
        let semantic_diagnostics = self.semantic_diagnostics.clone();
        let symbols = self.symbols.clone();
        let uri2 = uri.clone();
        let text2 = text.clone();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;

            // Check if this is still the most recent change
            let pending_time = pending.read().await.get(&uri2).cloned();
            if pending_time == Some(now) {
                // Still current - run analysis
                Self::analyze_document_static(
                    &uri2,
                    &text2,
                    &docs,
                    &planning_outputs,
                    &semantic_diagnostics,
                    &symbols,
                    &client,
                )
                .await;
            }
        });
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        tracing::debug!("Document closed: {}", params.text_document.uri);

        // Remove document state
        {
            let mut docs = self.documents.write().await;
            docs.remove(&params.text_document.uri);
        }

        // Remove planning output
        {
            let mut planning = self.planning_outputs.write().await;
            planning.remove(&params.text_document.uri);
        }

        {
            let mut sem_diags = self.semantic_diagnostics.write().await;
            sem_diags.remove(&params.text_document.uri);
        }

        {
            let mut pending = self.pending_changes.write().await;
            pending.remove(&params.text_document.uri);
        }

        {
            let mut symbols = self.symbols.write().await;
            symbols.remove_document(&params.text_document.uri);
        }

        // Clear diagnostics
        self.publish_diagnostics(params.text_document.uri, vec![])
            .await;
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        tracing::info!(
            "Completion request: uri={}, line={}, char={}",
            uri,
            position.line,
            position.character
        );

        if let Some(doc) = self.get_document(uri).await {
            let symbols = self.symbols.read().await;
            let entity_client = self.get_entity_client().await;
            tracing::info!("Entity client connected: {}", entity_client.is_some());
            let completions =
                handlers::completion::get_completions(&doc, position, &symbols, entity_client)
                    .await;
            tracing::info!("Returning {} completions", completions.len());
            return Ok(Some(CompletionResponse::Array(completions)));
        }

        tracing::warn!("No document found for completion request");
        Ok(None)
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        if let Some(doc) = self.get_document(uri).await {
            return Ok(handlers::hover::get_hover(&doc, position));
        }

        Ok(None)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        if let Some(doc) = self.get_document(uri).await {
            let symbols = self.symbols.read().await;
            return Ok(handlers::goto_definition::get_definition_with_uri(
                &doc, position, &symbols, uri,
            ));
        }

        Ok(None)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = &params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        if let Some(doc) = self.get_document(uri).await {
            let symbols = self.symbols.read().await;
            return Ok(handlers::goto_definition::get_references_with_uri(
                &doc, position, &symbols, uri,
            ));
        }

        Ok(None)
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        if let Some(doc) = self.get_document(uri).await {
            return Ok(handlers::signature::get_signature_help(&doc, position));
        }

        Ok(None)
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = &params.text_document.uri;

        if let Some(doc) = self.get_document(uri).await {
            let symbols = handlers::symbols::get_document_symbols(&doc, uri);
            return Ok(Some(DocumentSymbolResponse::Flat(symbols)));
        }

        Ok(None)
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let uri = &params.text_document.uri;
        let position = params.position;

        if let Some(doc) = self.get_document(uri).await {
            return Ok(handlers::rename::prepare_rename(&doc, position));
        }

        Ok(None)
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = &params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let new_name = &params.new_name;

        if let Some(doc) = self.get_document(uri).await {
            return Ok(handlers::rename::rename_symbol(
                &doc, position, new_name, uri,
            ));
        }

        Ok(None)
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = &params.text_document.uri;
        let range = params.range;

        tracing::debug!("Code action request: uri={}, range={:?}", uri, range);

        // Get document and planning output
        let doc = match self.get_document(uri).await {
            Some(d) => d,
            None => return Ok(None),
        };

        let planning_output = match self.get_planning_output(uri).await {
            Some(p) => p,
            None => return Ok(None),
        };

        // Get semantic diagnostics for entity suggestion actions
        let semantic_diagnostics = self.get_semantic_diagnostics(uri).await;

        // Generate code actions from planning output and semantic diagnostics
        let actions = handlers::code_actions::get_code_actions(
            &planning_output,
            &semantic_diagnostics,
            range,
            uri,
            &doc.text,
        );

        if actions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(actions))
        }
    }
}

/// Convert LSP position to byte offset in text.
fn offset_from_position(text: &str, position: Position) -> usize {
    position_to_offset(text, position, PositionEncoding::Utf16).unwrap_or(text.len())
}
