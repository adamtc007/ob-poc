//! DSL Visualizer - Mock version without database dependency
//!
//! ⚠️  URGENT TODO: ALL MOCK DATA IN THIS FILE MUST BE REPLACED WITH REAL DATA ASAP ⚠️
//! ⚠️  TODO: Replace mock DSL entries with real database queries ⚠️
//! ⚠️  TODO: Replace mock AST generation with real DSL parsing ⚠️
//! ⚠️  TODO: Replace mock CBU data with real CBU repository calls ⚠️
//! ⚠️  TODO: Connect to actual DSL instance repository for persistence ⚠️
//!
//! This version creates a working demo of the DSL creation functionality
//! without requiring a database connection. Perfect for testing the UI.
//!
//! Usage:
//!   cargo run --features visualizer --bin egui_dsl_visualizer_mock

use eframe::egui;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{error, info, warn};
use uuid::Uuid;

/// DSL entry for display
#[derive(Debug, Clone)]
struct DslEntry {
    id: String,
    name: String,
    domain: String,
    version: u32,
    description: String,
    created_at: String,
    status: String,
}

/// AST node structure for display
#[derive(Debug, Clone)]
struct AstNode {
    id: String,
    node_type: String,
    label: String,
    properties: HashMap<String, String>,
    children: Vec<AstNode>,
}

/// Application operation state
#[derive(Debug, Clone, PartialEq)]
enum AppState {
    Idle,
    LoadingEntries,
    LoadingContent(String),
    CreatingDsl { cbu_name: String },
    Error(String),
}

/// Application mode
#[derive(Debug, Clone, PartialEq)]
enum AppMode {
    ViewMode,     // Browse and view existing DSLs
    CreateMode,   // Create new DSL.Onboarding
    CreateKycMode, // Create new DSL.KYC linked to existing Onboarding
}

/// CBU data for creation picker
#[derive(Debug, Clone)]
struct CbuData {
    cbu_id: Uuid,
    name: String,
    description: Option<String>,
    nature_purpose: Option<String>,
}

/// KYC creation form data
#[derive(Debug, Clone)]
struct KycFormData {
    selected_onboarding_id: Option<String>,
    kyc_type: String,
    risk_level: String,
    verification_method: String,
    documentation_required: Vec<String>,
    special_instructions: String,
}

/// Mock creation result
#[derive(Debug, Clone)]
struct MockCreationResult {
    instance_id: Uuid,
    version_id: Uuid,
    business_reference: String,
    created_at: String,
    parent_instance_id: Option<Uuid>, // For KYC linking to parent Onboarding
}

/// Application state
struct DslVisualizerMockApp {
    /// Current operation state
    state: AppState,

    /// Current application mode
    app_mode: AppMode,

    /// Last refresh attempt timestamp
    last_refresh: Instant,

    /// Minimum time between refresh attempts (milliseconds)
    refresh_cooldown: Duration,

    /// Available DSL entries (mock data)
    dsl_entries: Vec<DslEntry>,

    /// Currently selected entry index
    selected_index: Option<usize>,

    /// Current DSL content
    current_dsl_content: String,

    /// Current AST
    current_ast: Option<AstNode>,

    /// UI state
    show_raw_ast: bool,

    /// CREATE MODE STATE
    /// Available CBUs for creation
    available_cbus: Vec<CbuData>,
    /// Selected CBU index
    selected_cbu_index: Option<usize>,
    /// Creation form fields
    onboarding_name: String,
    onboarding_description: String,
    nature_purpose: String,
    source_of_funds: String,

    /// Mock creation results
    creation_results: Vec<MockCreationResult>,
    pending_creation_result: Option<Result<MockCreationResult, String>>,

    /// All CBUs (before filtering)
    all_cbus: Vec<CbuData>,

    /// KYC creation form data
    kyc_form: KycFormData,
}

impl DslVisualizerMockApp {
    fn new() -> Self {
        info!("🚀 Initializing DSL Visualizer Mock App");

        let mut app = Self {
            state: AppState::Idle,
            app_mode: AppMode::ViewMode,
            last_refresh: Instant::now() - Duration::from_secs(10),
            refresh_cooldown: Duration::from_millis(1000),
            dsl_entries: Vec::new(),
            selected_index: None,
            current_dsl_content: String::new(),
            current_ast: None,
            show_raw_ast: false,
            // Create mode state
            available_cbus: Vec::new(),
            selected_cbu_index: None,
            onboarding_name: String::new(),
            onboarding_description: String::new(),
            nature_purpose: String::new(),
            source_of_funds: String::new(),
            creation_results: Vec::new(),
            pending_creation_result: None,
            all_cbus: Vec::new(),
            kyc_form: KycFormData {
                selected_onboarding_id: None,
                kyc_type: "Enhanced Due Diligence".to_string(),
                risk_level: "Medium".to_string(),
                verification_method: "Document Review".to_string(),
                documentation_required: vec!["Certificate of Incorporation".to_string()],
                special_instructions: String::new(),
            },
        };

        // Initialize with mock data
        app.initialize_mock_data();

        // Filter CBUs to exclude those already in active onboarding
        app.update_available_cbus();

        app
    }

    /// Initialize mock CBU and DSL data
    /// ⚠️ URGENT TODO: REPLACE ALL MOCK DATA BELOW WITH REAL DATABASE QUERIES ⚠️
    fn initialize_mock_data(&mut self) {
        // ⚠️ TODO: Replace with real CBU repository query: cbu_repo.get_all_active_cbus() ⚠️
        self.all_cbus = vec![
            CbuData {
                cbu_id: Uuid::new_v4(),
                name: "Alpha Holdings Singapore".to_string(),
                description: Some("Institutional investment entity".to_string()),
                nature_purpose: Some("Investment management and advisory services".to_string()),
            },
            CbuData {
                cbu_id: Uuid::new_v4(),
                name: "Beta Capital Partners".to_string(),
                description: Some("Private equity fund".to_string()),
                nature_purpose: Some("Private equity investments in technology sector".to_string()),
            },
            CbuData {
                cbu_id: Uuid::new_v4(),
                name: "Gamma Asset Management".to_string(),
                description: Some("Hedge fund manager".to_string()),
                nature_purpose: Some("Alternative investment strategies".to_string()),
            },
        ];

        // ⚠️ TODO: Replace with real DSL instance repository query: dsl_repo.get_all_instances() ⚠️
        self.dsl_entries = vec![
            DslEntry {
                id: "550e8400-e29b-41d4-a716-446655440001".to_string(),
                name: "Alpha Holdings Onboarding".to_string(),
                domain: "onboarding".to_string(),
                version: 1,
                description: "Initial onboarding for Alpha Holdings Singapore".to_string(),
                created_at: "2024-12-16T10:30:00Z".to_string(),
                status: "active".to_string(),
            },
            DslEntry {
                id: "550e8400-e29b-41d4-a716-446655440002".to_string(),
                name: "Beta Capital KYC Case".to_string(),
                domain: "kyc".to_string(),
                version: 2,
                description: "KYC verification for Beta Capital Partners".to_string(),
                created_at: "2024-12-15T14:20:00Z".to_string(),
                status: "completed".to_string(),
            },
        ];
    }

    /// Filter available CBUs to exclude those already in active onboarding
    /// ⚠️ URGENT TODO: Replace with proper database implementation ⚠️
    /// Real implementation should:
    /// 1. Query: SELECT DISTINCT business_reference FROM dsl_instances
    ///          WHERE domain = 'onboarding' AND status NOT IN ('completed', 'archived', 'cancelled', 'failed')
    /// 2. Map business_reference -> CBU ID -> CBU name via proper joins
    /// 3. Filter self.all_cbus against the excluded CBU IDs
    /// Example: let excluded_cbu_ids = dsl_repo.get_cbus_with_active_onboarding().await?;
    fn update_available_cbus(&mut self) {
        // Get CBU names that are already in ANY onboarding DSL instances
        // (not just active - also created, editing, pending, etc.)
        let onboarding_cbu_names: std::collections::HashSet<String> = self
            .dsl_entries
            .iter()
            .filter(|entry| {
                // Filter for onboarding domain and exclude only completed/archived statuses
                entry.domain == "onboarding"
                    && entry.status != "completed"
                    && entry.status != "archived"
                    && entry.status != "cancelled"
                    && entry.status != "failed"
            })
            .filter_map(|entry| {
                // ⚠️ TODO: In real implementation, use proper CBU ID -> Name mapping from database
                // Extract CBU name from DSL entry name patterns
                self.extract_cbu_name_from_dsl_entry(&entry.name)
            })
            .collect();

        // Also check business_reference field patterns (common in real DSL instances)
        let business_ref_cbu_names: std::collections::HashSet<String> = self
            .dsl_entries
            .iter()
            .filter(|entry| entry.domain == "onboarding")
            .filter_map(|entry| {
                // Extract from business reference patterns like "OB-BETA-CAPITAL-PARTNERS-20251110"
                if entry.name.starts_with("OB-") {
                    self.extract_cbu_from_business_reference(&entry.name)
                } else {
                    None
                }
            })
            .collect();

        // Combine both sets
        let all_excluded_cbu_names: std::collections::HashSet<String> = onboarding_cbu_names
            .union(&business_ref_cbu_names)
            .cloned()
            .collect();

        // Filter out CBUs that are already in onboarding processes
        self.available_cbus = self
            .all_cbus
            .iter()
            .filter(|cbu| !all_excluded_cbu_names.contains(&cbu.name))
            .cloned()
            .collect();

        // Reset selected index if the selected CBU is no longer available
        if let Some(selected_idx) = self.selected_cbu_index {
            if selected_idx >= self.available_cbus.len() {
                self.selected_cbu_index = None;
            }
        }

        info!(
            "CBU Filtering Results: {} available out of {} total",
            self.available_cbus.len(),
            self.all_cbus.len()
        );
        info!(
            "Excluded CBUs (already in onboarding): {:?}",
            all_excluded_cbu_names
        );
    }

    /// Mock KYC creation process linked to parent Onboarding DSL
    /// ⚠️ TODO: Replace with real KYC DSL generation and database persistence ⚠️
    fn mock_create_kyc_case(&mut self) {
        if let Some(parent_id) = &self.kyc_form.selected_onboarding_id {
            if let Some(parent_entry) = self.dsl_entries.iter().find(|e| e.id == *parent_id) {
                info!("🚀 Mock creating DSL.KYC for parent Onboarding: {}", parent_entry.name);

                self.state = AppState::CreatingDsl {
                    cbu_name: format!("KYC-{}", parent_entry.name),
                };

                // Generate KYC business reference linked to parent
                let kyc_business_reference = format!(
                    "KYC-{}-{}",
                    parent_id.split('-').next().unwrap_or("UNKNOWN"),
                    chrono::Utc::now().format("%Y%m%d")
                );

                let mock_result = MockCreationResult {
                    instance_id: Uuid::new_v4(),
                    version_id: Uuid::new_v4(),
                    business_reference: kyc_business_reference.clone(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                    parent_instance_id: Some(Uuid::parse_str(parent_id).unwrap_or_else(|_| Uuid::new_v4())),
                };

                self.pending_creation_result = Some(Ok(mock_result));
                info!("✅ Mock KYC creation completed: {}", kyc_business_reference);
            }
        }
    }

    /// Generate mock KYC DSL content
    /// ⚠️ TODO: Replace with real DSL generation using parent onboarding data ⚠️
    fn generate_kyc_dsl_content(&self, parent_entry: &DslEntry) -> String {
        format!(r#"
;; KYC Case DSL - Linked to Parent Onboarding: {}
;; Generated: {}
;; Parent Instance ID: {}

(define-kyc-case "{}"
  :parent-onboarding-id "{}"
  :kyc-type "{}"
  :risk-level "{}"
  :verification-method "{}"

  (kyc.verify
    :entity-name "{}"
    :required-documents {}
    :special-instructions "{}")

  (compliance.check
    :fatca-status "PENDING"
    :aml-screening "REQUIRED"
    :sanctions-check "REQUIRED")

  (risk.assess
    :methodology "STANDARD"
    :factors ["JURISDICTION" "BUSINESS_NATURE" "OWNERSHIP_STRUCTURE"])

  (document.collect
    :categories {}
    :retention-period "7-YEARS"))"#,
            parent_entry.name,
            chrono::Utc::now().to_rfc3339(),
            parent_entry.id,
            format!("KYC-{}", parent_entry.name.replace(" ", "-").to_uppercase()),
            parent_entry.id,
            self.kyc_form.kyc_type,
            self.kyc_form.risk_level,
            self.kyc_form.verification_method,
            parent_entry.name,
            format!("[{}]", self.kyc_form.documentation_required.iter()
                .map(|d| format!("\"{}\"", d))
                .collect::<Vec<_>>()
                .join(" ")),
            self.kyc_form.special_instructions,
            format!("[{}]", self.kyc_form.documentation_required.iter()
                .map(|d| format!("\"{}\"", d))
                .collect::<Vec<_>>()
                .join(" "))
        )
    }

    /// Get available onboarding DSL entries for KYC creation
    /// ⚠️ TODO: Replace with database query for active onboarding instances ⚠️
    fn get_available_onboarding_entries(&self) -> Vec<&DslEntry> {
        self.dsl_entries
            .iter()
            .filter(|entry| {
                entry.domain == "onboarding"
                && (entry.status == "active" || entry.status == "finalized")
            })
            .collect()
    }

    /// Extract CBU name from DSL entry name
    /// ⚠️ TODO: Replace with proper CBU ID lookup from dsl_instance.business_reference ⚠️
    fn extract_cbu_name_from_dsl_entry(&self, entry_name: &str) -> Option<String> {
        // Mock pattern matching - real implementation would query database
        if entry_name.contains("Alpha Holdings") {
            Some("Alpha Holdings Singapore".to_string())
        } else if entry_name.contains("Beta Capital") {
            Some("Beta Capital Partners".to_string())
        } else if entry_name.contains("Gamma Asset") {
            Some("Gamma Asset Management".to_string())
        } else {
            None
        }
    }

    /// Extract CBU name from business reference pattern
    /// ⚠️ TODO: Replace with proper business_reference -> CBU mapping ⚠️
    fn extract_cbu_from_business_reference(&self, business_ref: &str) -> Option<String> {
        // Pattern: "OB-BETA-CAPITAL-PARTNERS-20251110" -> "Beta Capital Partners"
        if let Some(cbu_part) = business_ref.strip_prefix("OB-") {
            if let Some(dash_pos) = cbu_part.rfind('-') {
                if cbu_part[dash_pos + 1..].chars().all(|c| c.is_ascii_digit()) {
                    // Remove the date suffix
                    let cbu_slug = &cbu_part[..dash_pos];
                    return Some(
                        cbu_slug
                            .replace('-', " ")
                            .split_whitespace()
                            .map(|word| {
                                let mut chars = word.chars();
                                match chars.next() {
                                    None => String::new(),
                                    Some(first) => {
                                        first.to_uppercase().collect::<String>()
                                            + &chars.as_str().to_lowercase()
                                    }
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(" "),
                    );
                }
            }
        }
        None
    }

    /// Mock DSL creation process
    fn mock_create_onboarding(&mut self) {
        if let Some(selected_idx) = self.selected_cbu_index {
            if let Some(cbu) = self.available_cbus.get(selected_idx) {
                info!("🚀 Mock creating DSL.Onboarding for CBU: {}", cbu.name);

                self.state = AppState::CreatingDsl {
                    cbu_name: cbu.name.clone(),
                };

                // Simulate async creation with a result after 2 seconds
                let business_reference = format!(
                    "OB-{}-{}",
                    cbu.name.replace(" ", "-").to_uppercase(),
                    chrono::Utc::now().format("%Y%m%d")
                );

                let mock_result = MockCreationResult {
                    instance_id: Uuid::new_v4(),
                    version_id: Uuid::new_v4(),
                    business_reference: business_reference.clone(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                    parent_instance_id: None,
                };

                // Simulate success (you can change this to test error cases)
                self.pending_creation_result = Some(Ok(mock_result));

                info!("✅ Mock DSL creation completed: {}", business_reference);
            }
        }
    }

    /// Handle DSL entry selection
    fn handle_dsl_selection(&mut self, entry_index: usize) {
        if let Some(entry) = self.dsl_entries.get(entry_index) {
            info!("📋 Selected DSL entry: {} ({})", entry.name, entry.id);
            self.selected_index = Some(entry_index);

            // Generate mock DSL content
            self.current_dsl_content = self.generate_mock_dsl_content(entry);

            // Generate mock AST
            self.current_ast = Some(self.generate_mock_ast(entry));
        }
    }

    /// Generate mock DSL content based on entry
    fn generate_mock_dsl_content(&self, entry: &DslEntry) -> String {
        match entry.domain.as_str() {
            "onboarding" => format!(
                r#";; DSL.Onboarding - {}
;; Generated: {}

(case.create
  :cbu-id "{}"
  :business-reference "{}"
  :request-type "INSTITUTIONAL_ONBOARDING"
  :jurisdiction "SG"
  :priority "STANDARD")

(document.catalog
  :document-id "doc-{}-incorporation-001"
  :document-type "CERTIFICATE_OF_INCORPORATION"
  :issuer "acra_singapore"
  :title "Certificate of Incorporation"
  :jurisdiction "SG"
  :extracted-data {{
    :company.legal_name "{}"
    :company.registration_number "202412345A"
    :company.jurisdiction "SG"
  }})

(kyc.verify
  :entity-id "entity-{}"
  :verification-type "ENHANCED_DUE_DILIGENCE"
  :risk-factors ["SINGAPORE_INCORPORATED" "INSTITUTIONAL_CLIENT"]
  :status "VERIFIED")

(case.update
  :business-reference "{}"
  :status "READY_FOR_APPROVAL"
  :completion-percentage 85.0)
"#,
                entry.name,
                entry.created_at,
                entry.id,
                entry.name.replace(" ", "-").to_uppercase(),
                entry.id.split('-').next().unwrap_or("unknown"),
                entry.name,
                entry.id.split('-').next().unwrap_or("unknown"),
                entry.name.replace(" ", "-").to_uppercase()
            ),
            "kyc" => format!(
                r#";; KYC Case - {}
;; Generated: {}

(kyc.start
  :case-id "{}"
  :customer-type "INSTITUTIONAL"
  :risk-profile "MEDIUM")

(kyc.collect
  :document-types ["PASSPORT" "PROOF_OF_ADDRESS" "BANK_STATEMENT"]
  :verification-method "ELECTRONIC")

(kyc.screen
  :screening-lists ["OFAC" "EU_SANCTIONS" "UN_SANCTIONS"]
  :results "NO_MATCHES")

(kyc.assess
  :risk-rating "MEDIUM"
  :approval-status "APPROVED")
"#,
                entry.name, entry.created_at, entry.id
            ),
            _ => format!(
                r#";; Generic DSL - {}
;; Generated: {}

(generic.operation
  :id "{}"
  :type "{}"
  :status "{}")
"#,
                entry.name, entry.created_at, entry.id, entry.domain, entry.status
            ),
        }
    }

    /// Generate mock AST based on entry
    fn generate_mock_ast(&self, entry: &DslEntry) -> AstNode {
        AstNode {
            id: "root".to_string(),
            node_type: "Program".to_string(),
            label: format!("DSL Program: {}", entry.name),
            properties: HashMap::from([
                ("domain".to_string(), entry.domain.clone()),
                ("version".to_string(), entry.version.to_string()),
            ]),
            children: match entry.domain.as_str() {
                "onboarding" => vec![
                    AstNode {
                        id: "case_create".to_string(),
                        node_type: "Form".to_string(),
                        label: "case.create".to_string(),
                        properties: HashMap::from([
                            ("verb".to_string(), "case.create".to_string()),
                            ("cbu-id".to_string(), entry.id.clone()),
                        ]),
                        children: vec![],
                    },
                    AstNode {
                        id: "document_catalog".to_string(),
                        node_type: "Form".to_string(),
                        label: "document.catalog".to_string(),
                        properties: HashMap::from([
                            ("verb".to_string(), "document.catalog".to_string()),
                            (
                                "document-type".to_string(),
                                "CERTIFICATE_OF_INCORPORATION".to_string(),
                            ),
                        ]),
                        children: vec![],
                    },
                    AstNode {
                        id: "kyc_verify".to_string(),
                        node_type: "Form".to_string(),
                        label: "kyc.verify".to_string(),
                        properties: HashMap::from([
                            ("verb".to_string(), "kyc.verify".to_string()),
                            (
                                "verification-type".to_string(),
                                "ENHANCED_DUE_DILIGENCE".to_string(),
                            ),
                        ]),
                        children: vec![],
                    },
                ],
                "kyc" => vec![AstNode {
                    id: "kyc_start".to_string(),
                    node_type: "Form".to_string(),
                    label: "kyc.start".to_string(),
                    properties: HashMap::from([
                        ("verb".to_string(), "kyc.start".to_string()),
                        ("customer-type".to_string(), "INSTITUTIONAL".to_string()),
                    ]),
                    children: vec![],
                }],
                _ => vec![AstNode {
                    id: "generic_op".to_string(),
                    node_type: "Form".to_string(),
                    label: "generic.operation".to_string(),
                    properties: HashMap::new(),
                    children: vec![],
                }],
            },
        }
    }

    /// Render DSL creation form
    fn render_creation_form(&mut self, ui: &mut egui::Ui) {
        ui.heading("🆕 Create New DSL.Onboarding");
        ui.separator();

        // CBU Picker
        ui.group(|ui| {
            ui.heading("1. Select CBU");
            ui.label("ℹ️ Only CBUs without active onboarding DSL instances are shown");

            egui::ComboBox::from_label("CBU")
                .selected_text(
                    self.selected_cbu_index
                        .and_then(|i| self.available_cbus.get(i))
                        .map(|cbu| cbu.name.as_str())
                        .unwrap_or("Select CBU..."),
                )
                .show_ui(ui, |ui| {
                    for (idx, cbu) in self.available_cbus.iter().enumerate() {
                        let selected = self.selected_cbu_index == Some(idx);
                        if ui.selectable_label(selected, &cbu.name).clicked() {
                            self.selected_cbu_index = Some(idx);
                        }
                    }
                });

            if self.available_cbus.is_empty() {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    "⚠️ No CBUs available - all are currently in active onboarding processes",
                );
            }

            // Show selected CBU details
            if let Some(selected_idx) = self.selected_cbu_index {
                if let Some(cbu) = self.available_cbus.get(selected_idx) {
                    ui.horizontal(|ui| {
                        ui.label("Selected:");
                        ui.strong(&cbu.name);
                    });
                    if let Some(desc) = &cbu.description {
                        ui.label(format!("Description: {}", desc));
                    }
                }
            }
        });

        ui.add_space(10.0);

        // Onboarding Details
        ui.group(|ui| {
            ui.heading("2. Onboarding Details");

            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut self.onboarding_name);
            });

            ui.horizontal(|ui| {
                ui.label("Description:");
                ui.add(
                    egui::TextEdit::multiline(&mut self.onboarding_description)
                        .desired_rows(2)
                        .desired_width(300.0),
                );
            });

            ui.horizontal(|ui| {
                ui.label("Nature & Purpose:");
                ui.add(
                    egui::TextEdit::multiline(&mut self.nature_purpose)
                        .desired_rows(2)
                        .desired_width(300.0),
                );
            });

            ui.horizontal(|ui| {
                ui.label("Source of Funds:");
                ui.add(
                    egui::TextEdit::multiline(&mut self.source_of_funds)
                        .desired_rows(2)
                        .desired_width(300.0),
                );
            });
        });

        ui.add_space(10.0);

        // Create Button
        ui.horizontal(|ui| {
            let can_create = self.selected_cbu_index.is_some()
                && !self.onboarding_name.is_empty()
                && !self.nature_purpose.is_empty()
                && !self.source_of_funds.is_empty()
                && matches!(self.state, AppState::Idle);

            ui.add_enabled_ui(can_create, |ui| {
                if ui.button("🚀 Create DSL.Onboarding").clicked() {
                    self.mock_create_onboarding();
                }
            });

            if ui.button("🔄 Reset Form").clicked() {
                self.reset_creation_form();
            }
        });

        // Status/Results
        match &self.state {
            AppState::CreatingDsl { cbu_name } => {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(format!("Creating DSL for {}...", cbu_name));
                });
            }

            /// Render KYC creation form
            /// ⚠️ TODO: Connect to real onboarding DSL instances and KYC repository ⚠️
            fn render_kyc_creation_form(&mut self, ui: &mut egui::Ui) {
                ui.heading("🔍 Create New DSL.KYC Case");
                ui.separator();

                // Parent Onboarding Picker
                ui.group(|ui| {
                    ui.heading("1. Select Parent Onboarding DSL");
                    ui.label("ℹ️ KYC cases are linked to existing onboarding DSL instances");

                    let available_onboarding = self.get_available_onboarding_entries();

                    egui::ComboBox::from_label("Parent Onboarding DSL")
                        .selected_text(
                            self.kyc_form.selected_onboarding_id
                                .as_ref()
                                .and_then(|id| available_onboarding.iter().find(|e| e.id == *id))
                                .map(|entry| entry.name.as_str())
                                .unwrap_or("Select Onboarding DSL..."),
                        )
                        .show_ui(ui, |ui| {
                            for entry in available_onboarding.iter() {
                                let selected = self.kyc_form.selected_onboarding_id.as_ref() == Some(&entry.id);
                                if ui.selectable_label(selected, format!("{} ({})", entry.name, entry.status)).clicked() {
                                    self.kyc_form.selected_onboarding_id = Some(entry.id.clone());
                                }
                            }
                        });

                    if available_onboarding.is_empty() {
                        ui.colored_label(
                            egui::Color32::YELLOW,
                            "⚠️ No active onboarding DSL instances available for KYC case creation",
                        );
                    }

                    // Show selected onboarding details
                    if let Some(selected_id) = &self.kyc_form.selected_onboarding_id {
                        if let Some(parent_entry) = available_onboarding.iter().find(|e| e.id == *selected_id) {
                            ui.separator();
                            ui.group(|ui| {
                                ui.label("Selected Onboarding DSL:");
                                ui.strong(&parent_entry.name);
                                ui.label(format!("Status: {}", parent_entry.status));
                                ui.label(format!("Created: {}", parent_entry.created_at));
                                ui.label(format!("Description: {}", parent_entry.description));
                            });
                        }
                    }
                });

                ui.add_space(10.0);

                // KYC Configuration
                ui.group(|ui| {
                    ui.heading("2. KYC Configuration");

                    ui.horizontal(|ui| {
                        ui.label("KYC Type:");
                        egui::ComboBox::from_label("")
                            .selected_text(&self.kyc_form.kyc_type)
                            .show_ui(ui, |ui| {
                                let kyc_types = ["Enhanced Due Diligence", "Standard Due Diligence", "Simplified Due Diligence", "Ongoing Monitoring"];
                                for kyc_type in kyc_types {
                                    let selected = self.kyc_form.kyc_type == kyc_type;
                                    if ui.selectable_label(selected, kyc_type).clicked() {
                                        self.kyc_form.kyc_type = kyc_type.to_string();
                                    }
                                }
                            });
                    });

                    ui.horizontal(|ui| {
                        ui.label("Risk Level:");
                        egui::ComboBox::from_label("")
                            .selected_text(&self.kyc_form.risk_level)
                            .show_ui(ui, |ui| {
                                let risk_levels = ["Low", "Medium", "High", "Very High"];
                                for level in risk_levels {
                                    let selected = self.kyc_form.risk_level == level;
                                    if ui.selectable_label(selected, level).clicked() {
                                        self.kyc_form.risk_level = level.to_string();
                                    }
                                }
                            });
                    });

                    ui.horizontal(|ui| {
                        ui.label("Verification Method:");
                        egui::ComboBox::from_label("")
                            .selected_text(&self.kyc_form.verification_method)
                            .show_ui(ui, |ui| {
                                let methods = ["Document Review", "Video Call", "In-Person Meeting", "Third-Party Verification"];
                                for method in methods {
                                    let selected = self.kyc_form.verification_method == method;
                                    if ui.selectable_label(selected, method).clicked() {
                                        self.kyc_form.verification_method = method.to_string();
                                    }
                                }
                            });
                    });

                    ui.separator();
                    ui.label("Required Documentation:");
                    ui.horizontal_wrapped(|ui| {
                        let mut docs_changed = false;
                        let available_docs = [
                            "Certificate of Incorporation",
                            "Articles of Association",
                            "Register of Directors",
                            "Register of Shareholders",
                            "Beneficial Ownership Declaration",
                            "Financial Statements",
                            "Bank References",
                            "Regulatory Licenses",
                        ];

                        for doc in available_docs {
                            let mut selected = self.kyc_form.documentation_required.contains(&doc.to_string());
                            if ui.checkbox(&mut selected, doc).changed() {
                                if selected {
                                    if !self.kyc_form.documentation_required.contains(&doc.to_string()) {
                                        self.kyc_form.documentation_required.push(doc.to_string());
                                    }
                                } else {
                                    self.kyc_form.documentation_required.retain(|d| d != doc);
                                }
                                docs_changed = true;
                            }
                        }
                    });

                    ui.separator();
                    ui.label("Special Instructions:");
                    ui.text_edit_multiline(&mut self.kyc_form.special_instructions);
                });

                ui.add_space(10.0);

                // Create Button
                ui.group(|ui| {
                    let can_create = self.kyc_form.selected_onboarding_id.is_some()
                        && !self.kyc_form.documentation_required.is_empty();

                    ui.add_enabled_ui(can_create, |ui| {
                        if ui.button("🔍 Create DSL.KYC Case").clicked() {
                            self.mock_create_kyc_case();
                        }
                    });

                    if !can_create {
                        ui.colored_label(
                            egui::Color32::GRAY,
                            "Select parent onboarding DSL and at least one required document"
                        );
                    }
                });

                // Show creation results for KYC
                if let Some(result) = self.pending_creation_result.take() {
                    match result {
                        Ok(mock_result) => {
                            self.state = AppState::Idle;
                            info!(
                                "✅ KYC DSL created successfully: instance_id={}, parent_id={:?}",
                                mock_result.instance_id, mock_result.parent_instance_id
                            );

                            // Add to creation results
                            self.creation_results.push(mock_result.clone());

                            // Add new KYC entry to the list
                            let parent_name = self.kyc_form.selected_onboarding_id
                                .as_ref()
                                .and_then(|id| self.dsl_entries.iter().find(|e| e.id == *id))
                                .map(|e| e.name.clone())
                                .unwrap_or_else(|| "Unknown".to_string());

                            let new_entry = DslEntry {
                                id: mock_result.instance_id.to_string(),
                                name: format!("KYC-{}", parent_name),
                                domain: "kyc".to_string(),
                                version: 1,
                                description: format!(
                                    "{} KYC case for {} (Risk: {})",
                                    self.kyc_form.kyc_type,
                                    parent_name,
                                    self.kyc_form.risk_level
                                ),
                                created_at: mock_result.created_at.clone(),
                                status: "active".to_string(),
                            };
                            self.dsl_entries.push(new_entry);

                            ui.separator();
                            ui.colored_label(
                                egui::Color32::GREEN,
                                format!(
                                    "✅ KYC DSL created successfully!\nBusiness Reference: {}\nInstance ID: {}\nParent Instance ID: {:?}",
                                    mock_result.business_reference,
                                    mock_result.instance_id,
                                    mock_result.parent_instance_id
                                ),
                            );

                            if ui.button("👁️ View Created KYC DSL").clicked() {
                                self.app_mode = AppMode::ViewMode;
                                // Auto-select the newly created KYC DSL
                                if let Some(index) = self.dsl_entries.iter().position(|e| e.id == mock_result.instance_id.to_string()) {
                                    self.selected_index = Some(index);
                                    self.handle_dsl_selection(index);
                                }
                            }

                            if ui.button("🔄 Reset Form").clicked() {
                                self.kyc_form = KycFormData {
                                    selected_onboarding_id: None,
                                    kyc_type: "Enhanced Due Diligence".to_string(),
                                    risk_level: "Medium".to_string(),
                                    verification_method: "Document Review".to_string(),
                                    documentation_required: vec!["Certificate of Incorporation".to_string()],
                                    special_instructions: String::new(),
                                };
                                info!("🔄 KYC form reset");
                            }
                        }
                        Err(error_msg) => {
                            self.state = AppState::Error(error_msg.clone());
                            ui.colored_label(egui::Color32::RED, format!("❌ Error: {}", error_msg));
                        }
                    }
                    _ => {}
                }
            }
            AppState::Error(error_msg) => {
                ui.separator();
                ui.colored_label(
                    egui::Color32::RED,
                    format!("❌ Creation failed: {}", error_msg),
                );
                if ui.button("Clear Error").clicked() {
                    self.state = AppState::Idle;
                }
            }
            _ => {}
        }

        // Check for creation results
        if let Some(result) = self.pending_creation_result.take() {
            match result {
                Ok(mock_result) => {
                    self.state = AppState::Idle;
                    info!(
                        "✅ DSL created successfully: instance_id={}, version_id={}",
                        mock_result.instance_id, mock_result.version_id
                    );

                    // Add to creation results
                    self.creation_results.push(mock_result.clone());

                    // Add new DSL entry to the list
                    let new_entry = DslEntry {
                        id: mock_result.instance_id.to_string(),
                        name: mock_result.business_reference.clone(),
                        domain: "onboarding".to_string(),
                        version: 1,
                        description: format!(
                            "{} - {}",
                            self.onboarding_description, self.nature_purpose
                        ),
                        created_at: mock_result.created_at.clone(),
                        status: "active".to_string(),
                    };
                    self.dsl_entries.push(new_entry);

                    // ⚠️ TODO: In real implementation, this should trigger a database refresh
                    // Update available CBUs to exclude the newly onboarded CBU
                    self.update_available_cbus();

                    ui.separator();
                    ui.colored_label(
                        egui::Color32::GREEN,
                        format!(
                            "✅ DSL created successfully!\nBusiness Reference: {}\nInstance ID: {}\nVersion ID: {}",
                            mock_result.business_reference,
                            mock_result.instance_id,
                            mock_result.version_id
                        ),
                    );

                    if ui.button("👁️ View Created DSL").clicked() {
                        self.app_mode = AppMode::ViewMode;
                        self.reset_creation_form();
                        // Auto-select the newly created DSL
                        self.selected_index = Some(self.dsl_entries.len() - 1);
                        self.handle_dsl_selection(self.dsl_entries.len() - 1);
                    }
                }
                Err(error_msg) => {
                    self.state = AppState::Error(error_msg);
                }
            }
        }
    }

    /// Reset the creation form
    fn reset_creation_form(&mut self) {
        self.selected_cbu_index = None;
        self.onboarding_name.clear();
        self.onboarding_description.clear();
        self.nature_purpose.clear();
        self.source_of_funds.clear();
        self.state = AppState::Idle;
        self.pending_creation_result = None;
        info!("🔄 Creation form reset");
    }

    /// Render entry picker
    fn render_entry_picker(&mut self, ui: &mut egui::Ui) {
        ui.heading("Available DSL Entries");

        if self.dsl_entries.is_empty() {
            ui.label("No DSL entries available");
            return;
        }

        let mut clicked_index = None;

        egui::ScrollArea::vertical()
            .max_height(200.0)
            .show(ui, |ui| {
                for (index, entry) in self.dsl_entries.iter().enumerate() {
                    let is_selected = self.selected_index == Some(index);

                    let response = ui.selectable_label(
                        is_selected,
                        format!(
                            "{} (v{}) - {}\nDomain: {} | Status: {}",
                            entry.name,
                            entry.version,
                            entry.description,
                            entry.domain,
                            entry.status
                        ),
                    );

                    if response.clicked() {
                        clicked_index = Some(index);
                    }
                }
            });

        if let Some(index) = clicked_index {
            self.handle_dsl_selection(index);
        }
    }

    /// Render DSL content panel
    fn render_dsl_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("📝 DSL Content");

        if self.current_dsl_content.is_empty() {
            ui.label("Select a DSL entry to view its content");
            return;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut self.current_dsl_content.as_str())
                    .code_editor()
                    .desired_width(f32::INFINITY)
                    .desired_rows(20),
            );
        });
    }

    /// Render AST panel
    fn render_ast_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("🌳 AST Visualization");

        ui.horizontal(|ui| {
            ui.checkbox(&mut self.show_raw_ast, "Show Raw AST");
        });

        ui.separator();

        match &self.current_ast {
            Some(ast) => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if self.show_raw_ast {
                        ui.label(format!("AST Root: {}", ast.label));
                        ui.label(format!("Node Type: {}", ast.node_type));
                        ui.label(format!("Children: {}", ast.children.len()));
                        ui.separator();
                        ui.label(format!("{:#?}", ast));
                    } else {
                        self.render_ast_tree(ui, ast, 0);
                    }
                });
            }
            None => {
                ui.label("Select a DSL entry to view its AST");
            }
        }
    }

    /// Render AST tree recursively
    fn render_ast_tree(&self, ui: &mut egui::Ui, node: &AstNode, depth: usize) {
        let indent = "  ".repeat(depth);

        ui.horizontal(|ui| {
            ui.label(format!("{}🌿 {}", indent, node.label));
            ui.weak(format!("({})", node.node_type));
        });

        // Show properties if any
        if !node.properties.is_empty() {
            for (key, value) in &node.properties {
                ui.horizontal(|ui| {
                    ui.label(format!("{}  📎 {}: {}", indent, key, value));
                });
            }
        }

        // Recursively render children
        for child in &node.children {
            self.render_ast_tree(ui, child, depth + 1);
        }
    }
}

impl eframe::App for DslVisualizerMockApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Top menu bar
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.heading("🔍 DSL/AST Visualizer (Mock Mode)");

                ui.separator();

                // Mode switcher
                ui.horizontal(|ui| {
                    ui.label("Mode:");
                    if ui
                        .selectable_label(self.app_mode == AppMode::ViewMode, "👁️ View DSLs")
                        .clicked()
                    {
                        self.app_mode = AppMode::ViewMode;
                    }
                    if ui
                        .selectable_label(self.app_mode == AppMode::CreateMode, "🆕 Create DSL")
                        .clicked()
                    {
                        self.app_mode = AppMode::CreateMode;
                        // Refresh available CBUs to ensure latest filtering
                        self.update_available_cbus();
                    }
                    if ui
                        .selectable_label(self.app_mode == AppMode::CreateKycMode, "🔍 Create KYC")
                        .clicked()
                    {
                        self.app_mode = AppMode::CreateKycMode;
                    }
                });

                ui.separator();

                // Status indicator
                let (status, color) = match &self.state {
                    AppState::Idle => ("✅ Ready".to_string(), egui::Color32::GREEN),
                    AppState::LoadingEntries => {
                        ("📥 Loading entries...".to_string(), egui::Color32::BLUE)
                    }
                    AppState::LoadingContent(_) => {
                        ("📄 Loading content...".to_string(), egui::Color32::BLUE)
                    }
                    AppState::CreatingDsl { cbu_name } => (
                        format!("🚀 Creating DSL for {}...", cbu_name),
                        egui::Color32::BLUE,
                    ),
                    AppState::Error(_) => ("❌ Error".to_string(), egui::Color32::RED),
                };
                ui.colored_label(color, format!("Status: {}", status));

                if self.app_mode == AppMode::ViewMode {
                    ui.label(format!("Entries: {}", self.dsl_entries.len()));
                }

                if !self.creation_results.is_empty() {
                    ui.label(format!("Created: {}", self.creation_results.len()));
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.app_mode {
                AppMode::ViewMode => {
                    // VIEW MODE - Original functionality

                    // Entry picker at top
                    ui.group(|ui| {
                        self.render_entry_picker(ui);
                    });

                    ui.separator();

                    // Main content: DSL on left, AST on right with resizable splitter
                    egui::SidePanel::left("dsl_panel")
                        .min_width(400.0)
                        .default_width(600.0)
                        .max_width(1000.0)
                        .resizable(true)
                        .show_inside(ui, |ui| {
                            self.render_dsl_panel(ui);
                        });

                    egui::CentralPanel::default().show_inside(ui, |ui| {
                        self.render_ast_panel(ui);
                    });
                }

                AppMode::CreateMode => {
                    // CREATE MODE - DSL creation form
                    self.render_creation_form(ui);
                }
                AppMode::CreateKycMode => {
                    // CREATE KYC MODE - KYC creation form
                    self.render_kyc_creation_form(ui);
                }
            }
        });
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    info!("🚀 Starting DSL/AST Visualizer (Mock Mode)");

    #[cfg(not(feature = "visualizer"))]
    {
        eprintln!("❌ Visualizer feature not enabled!");
        eprintln!("   Run with: cargo run --features visualizer --bin egui_dsl_visualizer_mock");
        std::process::exit(1);
    }

    #[cfg(feature = "visualizer")]
    {
        info!("💡 Running in mock mode - no database required!");

        // Set up eframe with minimal options
        let native_options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("DSL/AST Visualizer - Mock Mode")
                .with_inner_size([1400.0, 900.0])
                .with_min_inner_size([1000.0, 700.0])
                .with_resizable(true)
                .with_maximize_button(true),
            ..Default::default()
        };

        info!("Initializing egui application...");

        // Create and run the application
        let app_creator = |cc: &eframe::CreationContext<'_>| {
            // Set dark theme
            cc.egui_ctx.set_theme(egui::Theme::Dark);
            Ok(Box::new(DslVisualizerMockApp::new()) as Box<dyn eframe::App>)
        };

        // Run the application
        match eframe::run_native(
            "DSL/AST Visualizer - Mock",
            native_options,
            Box::new(app_creator),
        ) {
            Ok(_) => {
                info!("✅ Application shut down cleanly");
                Ok(())
            }
            Err(e) => {
                error!("❌ Application error: {}", e);
                Err(Box::new(e))
            }
        }
    }

    #[cfg(not(feature = "visualizer"))]
    {
        unreachable!("This should not be reached due to early exit above")
    }
}
