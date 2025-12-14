//! LSP Server implementation for the Onboarding DSL.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use crate::analysis::{DocumentState, SymbolTable};
use crate::entity_client::{gateway_addr, EntityLookupClient};
use crate::handlers;
use ob_poc::dsl_v2::planning_facade::PlanningOutput;

/// DSL Language Server state.
pub struct DslLanguageServer {
    /// LSP client for sending notifications
    client: Client,
    /// Open documents and their state
    documents: Arc<RwLock<HashMap<Url, DocumentState>>>,
    /// Planning output for each document (for code actions)
    planning_outputs: Arc<RwLock<HashMap<Url, PlanningOutput>>>,
    /// Session symbol table (shared across documents)
    symbols: Arc<RwLock<SymbolTable>>,
    /// Entity Gateway client for lookups (replaces direct DB access)
    entity_client: Arc<RwLock<Option<EntityLookupClient>>>,
}

impl DslLanguageServer {
    /// Create a new language server instance.
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(RwLock::new(HashMap::new())),
            planning_outputs: Arc::new(RwLock::new(HashMap::new())),
            symbols: Arc::new(RwLock::new(SymbolTable::new())),
            entity_client: Arc::new(RwLock::new(None)),
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
    pub async fn get_entity_client(&self) -> Option<EntityLookupClient> {
        // Clone the client for use - we need to reconnect each time since gRPC clients are !Clone
        let addr = gateway_addr();
        (EntityLookupClient::connect(&addr).await).ok()
    }

    /// Get a document by URL.
    pub async fn get_document(&self, uri: &Url) -> Option<DocumentState> {
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
    pub async fn get_planning_output(&self, uri: &Url) -> Option<PlanningOutput> {
        self.planning_outputs.read().await.get(uri).cloned()
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

        // Get full text from incremental changes
        if let Some(doc) = self.get_document(&params.text_document.uri).await {
            let mut text = doc.text.clone();
            for change in params.content_changes {
                if let Some(range) = change.range {
                    // Apply incremental change
                    let start_offset = offset_from_position(&text, range.start);
                    let end_offset = offset_from_position(&text, range.end);
                    text.replace_range(start_offset..end_offset, &change.text);
                } else {
                    // Full document replacement
                    text = change.text;
                }
            }
            self.analyze_document(&params.text_document.uri, &text)
                .await;
        }
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
            return Ok(handlers::goto_definition::get_definition(
                &doc, position, &symbols,
            ));
        }

        Ok(None)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = &params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        if let Some(doc) = self.get_document(uri).await {
            let symbols = self.symbols.read().await;
            return Ok(handlers::goto_definition::get_references(
                &doc, position, &symbols,
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
            let symbols = handlers::symbols::get_document_symbols(&doc);
            return Ok(Some(DocumentSymbolResponse::Flat(symbols)));
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

        // Generate code actions from planning output
        let actions =
            handlers::code_actions::get_code_actions(&planning_output, range, uri, &doc.text);

        if actions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(actions))
        }
    }
}

/// Convert LSP position to byte offset in text.
fn offset_from_position(text: &str, position: Position) -> usize {
    let mut offset = 0;
    for (line_num, line) in text.lines().enumerate() {
        if line_num == position.line as usize {
            // Add character offset within line
            offset += line
                .chars()
                .take(position.character as usize)
                .map(|c| c.len_utf8())
                .sum::<usize>();
            break;
        }
        // Add line length plus newline
        offset += line.len() + 1;
    }
    offset
}
