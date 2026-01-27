//! Verb schema migration from V1 to V2 format
//!
//! Provides:
//! - `migrate_v2`: Convert V1 YAML verbs to V2 schema format
//! - `lint_schemas`: Validate V2 schemas against lint rules
//! - `build_registry`: Compile VerbRegistry artifact

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ============================================================================
// Domain Templates
// ============================================================================

/// Domain template for generating V2 schemas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainTemplate {
    pub name: String,
    pub default_tier: String,
    pub default_tags: Vec<String>,
    pub common_args: Vec<TemplateArg>,
    pub positional_sugar: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateArg {
    pub name: String,
    pub typ: String,
    pub required: bool,
    #[serde(default)]
    pub default: Option<serde_json::Value>,
}

/// Get domain templates
pub fn domain_templates() -> HashMap<String, DomainTemplate> {
    let mut templates = HashMap::new();

    // View navigation template
    templates.insert(
        "view".to_string(),
        DomainTemplate {
            name: "view".to_string(),
            default_tier: "intent".to_string(),
            default_tags: vec!["navigation".to_string(), "view".to_string()],
            common_args: vec![
                TemplateArg {
                    name: "entity".to_string(),
                    typ: "entity_name".to_string(),
                    required: false,
                    default: None,
                },
                TemplateArg {
                    name: "mode".to_string(),
                    typ: "enum".to_string(),
                    required: false,
                    default: None,
                },
            ],
            positional_sugar: vec!["entity".to_string()],
        },
    );

    // Session template
    templates.insert(
        "session".to_string(),
        DomainTemplate {
            name: "session".to_string(),
            default_tier: "intent".to_string(),
            default_tags: vec!["session".to_string()],
            common_args: vec![TemplateArg {
                name: "target".to_string(),
                typ: "entity_name".to_string(),
                required: false,
                default: None,
            }],
            positional_sugar: vec!["target".to_string()],
        },
    );

    // Ownership template
    templates.insert(
        "ownership".to_string(),
        DomainTemplate {
            name: "ownership".to_string(),
            default_tier: "intent".to_string(),
            default_tags: vec!["ownership".to_string()],
            common_args: vec![
                TemplateArg {
                    name: "entity".to_string(),
                    typ: "entity_ref".to_string(),
                    required: true,
                    default: None,
                },
                TemplateArg {
                    name: "depth".to_string(),
                    typ: "int".to_string(),
                    required: false,
                    default: Some(serde_json::json!(10)),
                },
            ],
            positional_sugar: vec!["entity".to_string()],
        },
    );

    // UBO template
    templates.insert(
        "ubo".to_string(),
        DomainTemplate {
            name: "ubo".to_string(),
            default_tier: "intent".to_string(),
            default_tags: vec!["ubo".to_string(), "ownership".to_string()],
            common_args: vec![
                TemplateArg {
                    name: "entity".to_string(),
                    typ: "entity_ref".to_string(),
                    required: true,
                    default: None,
                },
                TemplateArg {
                    name: "threshold".to_string(),
                    typ: "decimal".to_string(),
                    required: false,
                    default: Some(serde_json::json!("25.0")),
                },
            ],
            positional_sugar: vec!["entity".to_string()],
        },
    );

    // Entity CRUD template
    templates.insert(
        "entity".to_string(),
        DomainTemplate {
            name: "entity".to_string(),
            default_tier: "crud".to_string(),
            default_tags: vec!["entity".to_string()],
            common_args: vec![TemplateArg {
                name: "entity".to_string(),
                typ: "entity_ref".to_string(),
                required: false,
                default: None,
            }],
            positional_sugar: vec!["entity".to_string()],
        },
    );

    // CBU template
    templates.insert(
        "cbu".to_string(),
        DomainTemplate {
            name: "cbu".to_string(),
            default_tier: "crud".to_string(),
            default_tags: vec!["cbu".to_string()],
            common_args: vec![TemplateArg {
                name: "cbu".to_string(),
                typ: "entity_ref".to_string(),
                required: false,
                default: None,
            }],
            positional_sugar: vec!["cbu".to_string()],
        },
    );

    // Fund template
    templates.insert(
        "fund".to_string(),
        DomainTemplate {
            name: "fund".to_string(),
            default_tier: "crud".to_string(),
            default_tags: vec!["fund".to_string()],
            common_args: vec![
                TemplateArg {
                    name: "name".to_string(),
                    typ: "str".to_string(),
                    required: true,
                    default: None,
                },
                TemplateArg {
                    name: "jurisdiction".to_string(),
                    typ: "str".to_string(),
                    required: false,
                    default: None,
                },
            ],
            positional_sugar: vec!["name".to_string(), "jurisdiction".to_string()],
        },
    );

    // KYC template
    templates.insert(
        "kyc".to_string(),
        DomainTemplate {
            name: "kyc".to_string(),
            default_tier: "intent".to_string(),
            default_tags: vec!["kyc".to_string()],
            common_args: vec![TemplateArg {
                name: "entity".to_string(),
                typ: "entity_ref".to_string(),
                required: false,
                default: None,
            }],
            positional_sugar: vec!["entity".to_string()],
        },
    );

    // Document template
    templates.insert(
        "document".to_string(),
        DomainTemplate {
            name: "document".to_string(),
            default_tier: "crud".to_string(),
            default_tags: vec!["document".to_string()],
            common_args: vec![TemplateArg {
                name: "entity".to_string(),
                typ: "entity_ref".to_string(),
                required: false,
                default: None,
            }],
            positional_sugar: vec!["entity".to_string()],
        },
    );

    // Default fallback template
    templates.insert(
        "default".to_string(),
        DomainTemplate {
            name: "default".to_string(),
            default_tier: "crud".to_string(),
            default_tags: vec![],
            common_args: vec![],
            positional_sugar: vec![],
        },
    );

    templates
}

/// Select appropriate template for a verb
pub fn select_template(domain: &str, action: &str) -> String {
    // Domain-based selection
    match domain {
        "view" => return "view".to_string(),
        "session" => return "session".to_string(),
        "ownership" => return "ownership".to_string(),
        "ubo" => return "ubo".to_string(),
        "entity" => return "entity".to_string(),
        "cbu" => return "cbu".to_string(),
        "fund" => return "fund".to_string(),
        "kyc" => return "kyc".to_string(),
        "document" => return "document".to_string(),
        _ => {}
    }

    // Action-based patterns
    if action.starts_with("list-") || action.starts_with("get-") {
        return "default".to_string();
    }
    if action.starts_with("compute-") || action.starts_with("calculate-") {
        return "default".to_string();
    }

    "default".to_string()
}

// ============================================================================
// Synonym Dictionaries
// ============================================================================

/// Get verb action synonyms
pub fn verb_synonyms() -> HashMap<&'static str, Vec<&'static str>> {
    let mut synonyms = HashMap::new();
    // CRUD operations
    synonyms.insert("create", vec!["add", "new", "make", "register"]);
    synonyms.insert("list", vec!["show", "get all", "display", "enumerate"]);
    synonyms.insert("get", vec!["show", "fetch", "retrieve", "read"]);
    synonyms.insert("read", vec!["get", "fetch", "show", "view"]);
    synonyms.insert("update", vec!["edit", "modify", "change", "set"]);
    synonyms.insert("delete", vec!["remove", "drop", "terminate"]);
    synonyms.insert("remove", vec!["delete", "drop", "clear"]);

    // Computation
    synonyms.insert("compute", vec!["calculate", "derive", "run"]);
    synonyms.insert("calculate", vec!["compute", "derive", "determine"]);
    synonyms.insert("analyze", vec!["examine", "inspect", "review"]);
    synonyms.insert("validate", vec!["verify", "check", "confirm"]);

    // Navigation
    synonyms.insert("drill", vec!["dive", "expand", "zoom in", "enter"]);
    synonyms.insert("surface", vec!["back", "up", "zoom out", "parent"]);
    synonyms.insert("load", vec!["open", "switch", "select"]);
    synonyms.insert("unload", vec!["close", "remove", "clear"]);

    // Discovery
    synonyms.insert("trace", vec!["follow", "track", "path"]);
    synonyms.insert("discover", vec!["find", "identify", "detect"]);
    synonyms.insert("find", vec!["search", "locate", "lookup"]);
    synonyms.insert("search", vec!["find", "lookup", "query"]);

    // Workflow
    synonyms.insert("approve", vec!["accept", "confirm", "authorize"]);
    synonyms.insert("reject", vec!["decline", "deny", "refuse"]);
    synonyms.insert("submit", vec!["send", "complete", "finish"]);
    synonyms.insert("assign", vec!["allocate", "set", "give"]);

    // State changes
    synonyms.insert("activate", vec!["enable", "start", "turn on"]);
    synonyms.insert("deactivate", vec!["disable", "stop", "turn off"]);
    synonyms.insert("suspend", vec!["pause", "hold", "freeze"]);
    synonyms.insert("provision", vec!["setup", "configure", "initialize"]);

    // Linking
    synonyms.insert("link", vec!["connect", "attach", "associate"]);
    synonyms.insert("attach", vec!["link", "connect", "add"]);
    synonyms.insert("sync", vec!["synchronize", "refresh", "update"]);

    synonyms
}

/// Get domain noun mappings
pub fn domain_nouns() -> HashMap<&'static str, Vec<&'static str>> {
    let mut nouns = HashMap::new();
    // Core entities
    nouns.insert("entity", vec!["entity", "company", "person"]);
    nouns.insert("cbu", vec!["cbu", "client business unit", "trading unit"]);
    nouns.insert("fund", vec!["fund", "investment vehicle", "sicav"]);

    // Ownership/control
    nouns.insert("ownership", vec!["ownership", "stake", "holding"]);
    nouns.insert("ubo", vec!["ubo", "beneficial owner", "ultimate owner"]);
    nouns.insert("control", vec!["control", "ownership chain", "hierarchy"]);

    // KYC/Compliance
    nouns.insert("kyc", vec!["kyc", "case", "compliance check"]);
    nouns.insert("kyc-case", vec!["kyc case", "compliance case"]);
    nouns.insert("screening", vec!["screening", "check", "verification"]);
    nouns.insert("document", vec!["document", "file", "attachment"]);
    nouns.insert("requirement", vec!["requirement", "document requirement"]);

    // Session/Navigation
    nouns.insert("session", vec!["session", "scope", "workspace"]);
    nouns.insert("view", vec!["view", "display", "visualization"]);
    nouns.insert("graph", vec!["graph", "visualization", "diagram"]);

    // Trading/Settlement
    nouns.insert("trading-profile", vec!["trading profile", "profile"]);
    nouns.insert("custody", vec!["custody", "safekeeping", "account"]);
    nouns.insert("isda", vec!["isda", "agreement", "contract"]);
    nouns.insert("ssi", vec!["ssi", "settlement instruction"]);

    // Products/Services
    nouns.insert("product", vec!["product", "service", "offering"]);
    nouns.insert("contract", vec!["contract", "agreement", "legal document"]);
    nouns.insert("service-resource", vec!["service resource", "resource"]);
    nouns.insert("service-intent", vec!["service intent", "intent"]);

    // Identifiers
    nouns.insert("identifier", vec!["identifier", "id", "reference"]);
    nouns.insert("gleif", vec!["gleif", "lei", "legal entity identifier"]);
    nouns.insert(
        "bods",
        vec!["bods", "beneficial ownership", "ownership data"],
    );

    // Reference data
    nouns.insert("jurisdiction", vec!["jurisdiction", "country"]);
    nouns.insert("currency", vec!["currency", "money"]);
    nouns.insert("role", vec!["role", "position"]);

    // Workflow
    nouns.insert("runbook", vec!["runbook", "command", "staged command"]);
    nouns.insert("agent", vec!["agent", "assistant"]);
    nouns.insert("batch", vec!["batch", "bulk operation"]);

    // Investor
    nouns.insert("investor", vec!["investor", "shareholder"]);
    nouns.insert("holding", vec!["holding", "position"]);
    nouns.insert("share-class", vec!["share class", "class"]);

    nouns
}

/// Generate invocation phrases for a verb
pub fn generate_phrases(domain: &str, action: &str, existing: &[String]) -> Vec<String> {
    let mut phrases: Vec<String> = existing.to_vec();
    let synonyms = verb_synonyms();
    let nouns = domain_nouns();

    // Get domain noun (primary and secondary)
    let domain_nouns_list = nouns.get(domain).map(|ns| ns.as_slice()).unwrap_or(&[]);
    let domain_noun = domain_nouns_list.first().copied().unwrap_or(domain);
    let domain_noun_alt = domain_nouns_list.get(1).copied();

    // Normalize action for phrase generation (kebab to space)
    let action_words = action.replace('-', " ");

    // Pattern 1: "{action} {domain_noun}" - e.g., "create cbu"
    let phrase1 = format!("{} {}", action_words, domain_noun);
    if !phrases.contains(&phrase1) {
        phrases.push(phrase1);
    }

    // Pattern 2: "{action} {domain}" - e.g., "create cbu" (if different from noun)
    if domain != domain_noun {
        let phrase2 = format!("{} {}", action_words, domain);
        if !phrases.contains(&phrase2) {
            phrases.push(phrase2);
        }
    }

    // Pattern 3: "{action} the {domain_noun}" - e.g., "create the client business unit"
    let phrase3 = format!("{} the {}", action_words, domain_noun);
    if !phrases.contains(&phrase3) {
        phrases.push(phrase3);
    }

    // Pattern 4: Add synonyms with domain noun
    if let Some(syns) = synonyms.get(action) {
        for syn in syns.iter().take(2) {
            let phrase = format!("{} {}", syn, domain_noun);
            if !phrases.contains(&phrase) {
                phrases.push(phrase);
            }
        }
    }

    // Pattern 5: Use alternate domain noun if available
    if let Some(alt_noun) = domain_noun_alt {
        let phrase = format!("{} {}", action_words, alt_noun);
        if !phrases.contains(&phrase) {
            phrases.push(phrase);
        }
    }

    // Pattern 6: "{domain_noun} {action}" - inverted form, e.g., "cbu create"
    let phrase6 = format!("{} {}", domain_noun, action_words);
    if !phrases.contains(&phrase6) {
        phrases.push(phrase6);
    }

    // Pattern 7: FQN as fallback
    if phrases.len() < 3 {
        let fqn_phrase = format!("{}.{}", domain, action);
        if !phrases.contains(&fqn_phrase) {
            phrases.push(fqn_phrase);
        }
    }

    // Pattern 8: Imperative form - "please {action} {domain}"
    if phrases.len() < 3 {
        let phrase = format!("please {} {}", action_words, domain_noun);
        if !phrases.contains(&phrase) {
            phrases.push(phrase);
        }
    }

    phrases.truncate(8); // Max 8 phrases
    phrases
}

/// Generate aliases for a verb
pub fn generate_aliases(action: &str, existing: &[String]) -> Vec<String> {
    let mut aliases: Vec<String> = existing
        .iter()
        .filter(|a| !a.contains(' ')) // Only single-word aliases
        .cloned()
        .collect();

    // Always include the action
    if !aliases.contains(&action.to_string()) {
        aliases.push(action.to_string());
    }

    // Add common synonyms as aliases
    let synonyms = verb_synonyms();
    if let Some(syns) = synonyms.get(action) {
        for syn in syns.iter().take(2) {
            if !aliases.contains(&syn.to_string()) {
                aliases.push(syn.to_string());
            }
        }
    }

    aliases.sort();
    aliases.dedup();
    aliases
}

// ============================================================================
// Lint Rules
// ============================================================================

/// Lint error type
#[derive(Debug, Clone)]
pub struct LintError {
    pub verb: String,
    pub code: String,
    pub message: String,
    pub severity: LintSeverity,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LintSeverity {
    Error,
    Warning,
}

/// Lint a V2 schema
pub fn lint_v2_schema(spec: &V2VerbSpec) -> Vec<LintError> {
    let mut errors = Vec::new();

    // Rule 1: Must have invocation_phrases
    if spec.invocation_phrases.is_empty() {
        errors.push(LintError {
            verb: spec.verb.clone(),
            code: "MISSING_PHRASES".to_string(),
            message: "Verb must have invocation_phrases".to_string(),
            severity: LintSeverity::Error,
        });
    }

    // Rule 2: Phrase count >= 3
    if spec.invocation_phrases.len() < 3 {
        errors.push(LintError {
            verb: spec.verb.clone(),
            code: "TOO_FEW_PHRASES".to_string(),
            message: format!(
                "Verb has {} phrases, minimum 3 required",
                spec.invocation_phrases.len()
            ),
            severity: LintSeverity::Warning,
        });
    }

    // Rule 3: Must have at least one example
    if spec.examples.is_empty() {
        errors.push(LintError {
            verb: spec.verb.clone(),
            code: "MISSING_EXAMPLES".to_string(),
            message: "Verb should have at least one example".to_string(),
            severity: LintSeverity::Warning,
        });
    }

    // Rule 4: positional_sugar max 2
    if spec.positional_sugar.len() > 2 {
        errors.push(LintError {
            verb: spec.verb.clone(),
            code: "TOO_MANY_POSITIONAL".to_string(),
            message: format!(
                "Verb has {} positional args, max 2 allowed",
                spec.positional_sugar.len()
            ),
            severity: LintSeverity::Error,
        });
    }

    // Rule 5: Must have doc
    if spec.doc.is_empty() {
        errors.push(LintError {
            verb: spec.verb.clone(),
            code: "MISSING_DOC".to_string(),
            message: "Verb should have documentation".to_string(),
            severity: LintSeverity::Warning,
        });
    }

    errors
}

// ============================================================================
// V2 Schema Types (output format)
// ============================================================================

/// V2 Verb specification (output format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V2VerbSpec {
    pub verb: String,
    pub domain: String,
    pub action: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    pub args: V2ArgSchema,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub positional_sugar: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub invocation_phrases: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub doc: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tier: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V2ArgSchema {
    #[serde(default = "default_style")]
    pub style: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub required: HashMap<String, V2ArgType>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub optional: HashMap<String, V2ArgType>,
}

fn default_style() -> String {
    "keyworded".to_string()
}

impl Default for V2ArgSchema {
    fn default() -> Self {
        Self {
            style: "keyworded".to_string(),
            required: HashMap::new(),
            optional: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V2ArgType {
    #[serde(rename = "type")]
    pub typ: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kinds: Option<Vec<String>>,
}

/// V2 Schema file (one domain per file)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V2SchemaFile {
    pub version: String,
    pub domain: String,
    #[serde(default)]
    pub description: String,
    pub verbs: Vec<V2VerbSpec>,
}

// ============================================================================
// Migration Logic
// ============================================================================

/// Migrate V1 verb YAML to V2 schema format
pub fn migrate_v2(verbs_dir: &Path, schemas_dir: &Path, dry_run: bool) -> Result<MigrationReport> {
    let mut report = MigrationReport::default();
    let templates = domain_templates();

    // Ensure output directory exists
    if !dry_run {
        std::fs::create_dir_all(schemas_dir)?;
    }

    // Process each YAML file
    for entry in std::fs::read_dir(verbs_dir)? {
        let entry = entry?;
        let path = entry.path();

        if !path
            .extension()
            .map(|e| e == "yaml" || e == "yml")
            .unwrap_or(false)
        {
            continue;
        }

        // Skip index and meta files
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('_') {
                continue;
            }
        }

        match process_verb_file(&path, schemas_dir, &templates, dry_run) {
            Ok(file_report) => {
                report.files_processed += 1;
                report.verbs_migrated += file_report.verbs_migrated;
                report.lint_errors += file_report.lint_errors;
                report.lint_warnings += file_report.lint_warnings;
            }
            Err(e) => {
                report.files_failed += 1;
                report.errors.push(format!("{}: {}", path.display(), e));
            }
        }
    }

    // Also process subdirectories
    for entry in std::fs::read_dir(verbs_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let subdir_name = path.file_name().unwrap().to_str().unwrap();
            let sub_schemas_dir = schemas_dir.join(subdir_name);

            match migrate_v2(&path, &sub_schemas_dir, dry_run) {
                Ok(sub_report) => {
                    report.files_processed += sub_report.files_processed;
                    report.files_failed += sub_report.files_failed;
                    report.verbs_migrated += sub_report.verbs_migrated;
                    report.lint_errors += sub_report.lint_errors;
                    report.lint_warnings += sub_report.lint_warnings;
                    report.errors.extend(sub_report.errors);
                }
                Err(e) => {
                    report.errors.push(format!("{}: {}", path.display(), e));
                }
            }
        }
    }

    Ok(report)
}

/// Process a single verb YAML file
fn process_verb_file(
    path: &Path,
    schemas_dir: &Path,
    templates: &HashMap<String, DomainTemplate>,
    dry_run: bool,
) -> Result<FileReport> {
    let content = std::fs::read_to_string(path)?;
    let v1: V1SchemaFile =
        serde_yaml::from_str(&content).with_context(|| format!("Parsing {}", path.display()))?;

    let mut report = FileReport::default();
    let mut v2_files: HashMap<String, V2SchemaFile> = HashMap::new();

    for (domain_name, domain_content) in v1.domains {
        let template_name = select_template(&domain_name, "");
        let template = templates
            .get(&template_name)
            .unwrap_or_else(|| templates.get("default").unwrap());

        for (verb_name, verb_content) in domain_content.verbs {
            let v2_spec = convert_verb_to_v2(&domain_name, &verb_name, &verb_content, template);

            // Lint the V2 spec
            let lint_errors = lint_v2_schema(&v2_spec);
            report.lint_errors += lint_errors
                .iter()
                .filter(|e| e.severity == LintSeverity::Error)
                .count();
            report.lint_warnings += lint_errors
                .iter()
                .filter(|e| e.severity == LintSeverity::Warning)
                .count();

            // Add to the appropriate V2 file
            let v2_file = v2_files
                .entry(domain_name.clone())
                .or_insert_with(|| V2SchemaFile {
                    version: "2.0".to_string(),
                    domain: domain_name.clone(),
                    description: domain_content.description.clone(),
                    verbs: Vec::new(),
                });
            v2_file.verbs.push(v2_spec);
            report.verbs_migrated += 1;
        }
    }

    // Write V2 files
    if !dry_run {
        for (domain_name, v2_file) in v2_files {
            let output_path = schemas_dir.join(format!("{}.yaml", domain_name));
            let yaml = serde_yaml::to_string(&v2_file)?;
            std::fs::write(&output_path, yaml)?;
        }
    }

    Ok(report)
}

/// Convert a V1 verb to V2 format
fn convert_verb_to_v2(
    domain: &str,
    action: &str,
    v1: &V1VerbContent,
    template: &DomainTemplate,
) -> V2VerbSpec {
    let verb = format!("{}.{}", domain, action);

    // Generate aliases
    let aliases = generate_aliases(action, &v1.invocation_phrases);

    // Convert args to V2 format
    let mut required: HashMap<String, V2ArgType> = HashMap::new();
    let mut optional: HashMap<String, V2ArgType> = HashMap::new();

    for arg in &v1.args {
        let v2_type = convert_arg_type(&arg.arg_type, &arg.valid_values, arg.lookup.is_some());
        if arg.required {
            required.insert(arg.name.clone(), v2_type);
        } else {
            optional.insert(arg.name.clone(), v2_type);
        }
    }

    // Compute positional sugar (first 1-2 required args)
    let positional_sugar: Vec<String> = v1
        .args
        .iter()
        .filter(|a| a.required)
        .take(2)
        .map(|a| a.name.clone())
        .collect();

    // Generate invocation phrases
    let invocation_phrases = generate_phrases(domain, action, &v1.invocation_phrases);

    // Generate examples
    let examples = generate_examples(&verb, &v1.args);

    // Determine tier (from metadata or template default)
    let tier = v1
        .metadata
        .tier
        .clone()
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| template.default_tier.clone());

    // Merge tags
    let mut tags = v1.metadata.tags.clone();
    for tag in &template.default_tags {
        if !tags.contains(tag) {
            tags.push(tag.clone());
        }
    }

    V2VerbSpec {
        verb,
        domain: domain.to_string(),
        action: action.to_string(),
        aliases,
        args: V2ArgSchema {
            style: "keyworded".to_string(),
            required,
            optional,
        },
        positional_sugar,
        invocation_phrases,
        examples,
        doc: v1.description.clone(),
        tier,
        tags,
    }
}

/// Convert V1 arg type to V2
fn convert_arg_type(
    v1_type: &str,
    valid_values: &Option<Vec<String>>,
    has_lookup: bool,
) -> V2ArgType {
    // Handle enum
    if let Some(values) = valid_values {
        return V2ArgType {
            typ: "enum".to_string(),
            default: None,
            values: Some(values.clone()),
            kinds: None,
        };
    }

    // Handle entity reference with lookup
    if has_lookup {
        return V2ArgType {
            typ: "entity_name".to_string(),
            default: None,
            values: None,
            kinds: None,
        };
    }

    // Map type names
    let typ = match v1_type {
        "string" => "str",
        "integer" | "int" => "int",
        "boolean" | "bool" => "bool",
        "uuid" => "uuid",
        "decimal" | "numeric" => "decimal",
        "date" => "date",
        "datetime" => "datetime",
        "json" | "object" => "json",
        "entity" | "entity_ref" => "entity_ref",
        "string_list" => "list",
        "uuid_list" => "list",
        other => other,
    };

    V2ArgType {
        typ: typ.to_string(),
        default: None,
        values: None,
        kinds: None,
    }
}

/// Generate example s-expressions
fn generate_examples(verb: &str, args: &[V1ArgContent]) -> Vec<String> {
    let required: Vec<_> = args.iter().filter(|a| a.required).collect();

    if required.is_empty() {
        return vec![format!("({})", verb)];
    }

    let args_str: String = required
        .iter()
        .map(|a| format!(":{} \"...\"", a.name))
        .collect::<Vec<_>>()
        .join(" ");

    vec![format!("({} {})", verb, args_str)]
}

// ============================================================================
// V1 Schema Types (input format)
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct V1SchemaFile {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub domains: HashMap<String, V1DomainContent>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct V1DomainContent {
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub verbs: HashMap<String, V1VerbContent>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct V1VerbContent {
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub invocation_phrases: Vec<String>,
    #[serde(default)]
    pub behavior: String,
    #[serde(default)]
    pub metadata: V1Metadata,
    #[serde(default)]
    pub args: Vec<V1ArgContent>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct V1Metadata {
    #[serde(default)]
    pub tier: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct V1ArgContent {
    pub name: String,
    #[serde(rename = "type", default)]
    pub arg_type: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub valid_values: Option<Vec<String>>,
    #[serde(default)]
    pub lookup: Option<serde_json::Value>,
}

// ============================================================================
// Reports
// ============================================================================

#[derive(Debug, Default)]
pub struct MigrationReport {
    pub files_processed: usize,
    pub files_failed: usize,
    pub verbs_migrated: usize,
    pub lint_errors: usize,
    pub lint_warnings: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Default)]
pub struct FileReport {
    pub verbs_migrated: usize,
    pub lint_errors: usize,
    pub lint_warnings: usize,
}

// ============================================================================
// Public API
// ============================================================================

/// Run verb migration
pub fn run_migrate_v2(dry_run: bool, verbose: bool) -> Result<()> {
    println!("===========================================");
    println!("  Verb Schema Migration: V1 → V2");
    println!("===========================================\n");

    // Locate directories
    let verbs_dir = PathBuf::from("config/verbs");
    let schemas_dir = PathBuf::from("config/verb_schemas/generated");

    if !verbs_dir.exists() {
        anyhow::bail!("Verbs directory not found: {:?}", verbs_dir);
    }

    println!("Source:      {:?}", verbs_dir);
    println!("Destination: {:?}", schemas_dir);
    println!("Dry run:     {}\n", dry_run);

    let report = migrate_v2(&verbs_dir, &schemas_dir, dry_run)?;

    println!("===========================================");
    println!("  Migration Report");
    println!("===========================================");
    println!("Files processed:  {}", report.files_processed);
    println!("Files failed:     {}", report.files_failed);
    println!("Verbs migrated:   {}", report.verbs_migrated);
    println!("Lint errors:      {}", report.lint_errors);
    println!("Lint warnings:    {}", report.lint_warnings);

    if verbose && !report.errors.is_empty() {
        println!("\n--- Errors ---");
        for err in &report.errors {
            println!("  {}", err);
        }
    }

    if dry_run {
        println!("\n(Dry run - no files written)");
    }

    if report.files_failed > 0 || report.lint_errors > 0 {
        std::process::exit(1);
    }

    Ok(())
}

/// Run schema linting
pub fn run_lint(errors_only: bool) -> Result<()> {
    println!("===========================================");
    println!("  Verb Schema Lint (V2)");
    println!("===========================================\n");

    let schemas_dir = PathBuf::from("config/verb_schemas/generated");

    if !schemas_dir.exists() {
        anyhow::bail!(
            "Schema directory not found: {:?}\nRun 'cargo x verbs migrate-v2' first.",
            schemas_dir
        );
    }

    let mut total_errors = 0;
    let mut total_warnings = 0;
    let mut total_verbs = 0;

    // Process each schema file
    for entry in std::fs::read_dir(&schemas_dir)? {
        let entry = entry?;
        let path = entry.path();

        if !path.extension().map(|e| e == "yaml").unwrap_or(false) {
            continue;
        }

        let content = std::fs::read_to_string(&path)?;
        let schema: V2SchemaFile = serde_yaml::from_str(&content)?;

        for spec in &schema.verbs {
            total_verbs += 1;
            let lint_errors = lint_v2_schema(spec);

            for err in lint_errors {
                match err.severity {
                    LintSeverity::Error => {
                        total_errors += 1;
                        println!("ERROR [{}] {}: {}", err.code, err.verb, err.message);
                    }
                    LintSeverity::Warning if !errors_only => {
                        total_warnings += 1;
                        println!("WARN  [{}] {}: {}", err.code, err.verb, err.message);
                    }
                    _ => {}
                }
            }
        }
    }

    println!("\n===========================================");
    println!("  Lint Summary");
    println!("===========================================");
    println!("Total verbs:    {}", total_verbs);
    println!("Errors:         {}", total_errors);
    println!("Warnings:       {}", total_warnings);

    if total_errors > 0 {
        std::process::exit(1);
    }

    Ok(())
}

/// Build compiled registry
pub fn run_build_registry() -> Result<()> {
    println!("===========================================");
    println!("  Build VerbRegistry");
    println!("===========================================\n");

    let schemas_dir = PathBuf::from("config/verb_schemas/generated");
    let output_path = PathBuf::from("config/verb_schemas/registry.json");

    if !schemas_dir.exists() {
        anyhow::bail!(
            "Schema directory not found: {:?}\nRun 'cargo x verbs migrate-v2' first.",
            schemas_dir
        );
    }

    let mut all_verbs: Vec<V2VerbSpec> = Vec::new();
    let mut alias_map: HashMap<String, Vec<String>> = HashMap::new();

    // Load all schemas
    for entry in std::fs::read_dir(&schemas_dir)? {
        let entry = entry?;
        let path = entry.path();

        if !path.extension().map(|e| e == "yaml").unwrap_or(false) {
            continue;
        }

        let content = std::fs::read_to_string(&path)?;
        let schema: V2SchemaFile = serde_yaml::from_str(&content)?;

        for spec in schema.verbs {
            // Track aliases
            for alias in &spec.aliases {
                alias_map
                    .entry(alias.to_lowercase())
                    .or_default()
                    .push(spec.verb.clone());
            }
            all_verbs.push(spec);
        }
    }

    // Check for alias collisions
    let collisions: Vec<_> = alias_map
        .iter()
        .filter(|(_, verbs)| verbs.len() > 1)
        .collect();

    if !collisions.is_empty() {
        println!("--- Alias Collisions ---");
        for (alias, verbs) in &collisions {
            println!("  '{}' → {:?}", alias, verbs);
        }
        println!();
    }

    // Write registry
    let registry = serde_json::json!({
        "version": "2.0",
        "generated": chrono::Utc::now().to_rfc3339(),
        "verb_count": all_verbs.len(),
        "alias_count": alias_map.len(),
        "collisions": collisions.len(),
        "verbs": all_verbs,
    });

    let json = serde_json::to_string_pretty(&registry)?;
    std::fs::write(&output_path, &json)?;

    println!("Registry built: {:?}", output_path);
    println!("Verbs:          {}", all_verbs.len());
    println!("Aliases:        {}", alias_map.len());
    println!("Collisions:     {}", collisions.len());

    Ok(())
}
