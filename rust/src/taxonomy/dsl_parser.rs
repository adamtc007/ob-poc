//! Natural language to DSL parser for taxonomy operations

use super::crud_ast::*;
use crate::forth_engine::ast::{DslParser, Expr};
use crate::forth_engine::parser_nom::NomDslParser;
use anyhow::{anyhow, Result};
use regex::Regex;
use std::collections::HashMap;
use uuid::Uuid;

pub struct TaxonomyDslParser;

impl TaxonomyDslParser {
    /// Parse natural language or structured DSL into taxonomy CRUD AST
    pub fn parse(input: &str) -> Result<TaxonomyCrudStatement> {
        let normalized = input.trim().to_lowercase();

        // Try structured DSL format first (S-expressions)
        if normalized.starts_with('(') {
            return Self::parse_structured_dsl(input);
        }

        // Parse natural language
        match Self::identify_operation(&normalized) {
            Operation::CreateProduct => Self::parse_create_product(input),
            Operation::CreateService => Self::parse_create_service(input),
            Operation::CreateOnboarding => Self::parse_create_onboarding(input),
            Operation::AddProducts => Self::parse_add_products(input),
            Operation::ConfigureService => Self::parse_configure_service(input),
            Operation::DiscoverServices => Self::parse_discover_services(input),
            Operation::QueryWorkflow => Self::parse_query_workflow(input),
            Operation::Unknown => Err(anyhow!("Could not identify operation from: {}", input)),
        }
    }

    fn parse_structured_dsl(input: &str) -> Result<TaxonomyCrudStatement> {
        let parser = NomDslParser::new();
        let exprs = parser.parse(input)
            .map_err(|e| anyhow!("Parse error: {}", e))?;
        
        if exprs.is_empty() {
            return Err(anyhow!("Empty DSL statement"));
        }
        
        Self::ast_to_crud_statement(&exprs[0])
    }

    fn ast_to_crud_statement(expr: &Expr) -> Result<TaxonomyCrudStatement> {
        match expr {
            Expr::WordCall { name, args } => {
                let params = Self::extract_keyword_pairs(args)?;
                
                match name.as_str() {
                    "product.create" => Self::build_create_product(params),
                    "service.create" => Self::build_create_service(params),
                    "onboarding.create" => Self::build_create_onboarding(params),
                    "products.add" => Self::build_add_products(params),
                    "service.configure" => Self::build_configure_service(params),
                    "services.discover" => Self::build_discover_services(params),
                    "workflow.query" => Self::build_query_workflow(params),
                    _ => Err(anyhow!("Unknown DSL operation: {}", name)),
                }
            }
            _ => Err(anyhow!("Expected WordCall expression")),
        }
    }

    fn extract_keyword_pairs(args: &[Expr]) -> Result<HashMap<String, serde_json::Value>> {
        let mut params = HashMap::new();
        let mut i = 0;
        
        while i < args.len() {
            if let Expr::Keyword(key) = &args[i] {
                let key_name = key.trim_start_matches(':').to_string();
                if i + 1 < args.len() {
                    let value = Self::expr_to_json_value(&args[i + 1])?;
                    params.insert(key_name, value);
                    i += 2;
                } else {
                    return Err(anyhow!("Keyword {} missing value", key));
                }
            } else {
                i += 1;
            }
        }
        
        Ok(params)
    }

    fn expr_to_json_value(expr: &Expr) -> Result<serde_json::Value> {
        match expr {
            Expr::StringLiteral(s) => Ok(serde_json::json!(s)),
            Expr::IntegerLiteral(n) => Ok(serde_json::json!(n)),
            Expr::BoolLiteral(b) => Ok(serde_json::json!(b)),
            Expr::Keyword(k) => Ok(serde_json::json!(k)),
            _ => Err(anyhow!("Cannot convert {:?} to JSON value", expr)),
        }
    }

    fn build_create_product(mut params: HashMap<String, serde_json::Value>) -> Result<TaxonomyCrudStatement> {
        Ok(TaxonomyCrudStatement::CreateProduct(CreateProduct {
            product_code: params.remove("code")
                .ok_or_else(|| anyhow!("Missing :code parameter"))?
                .as_str().unwrap().to_string(),
            name: params.remove("name")
                .ok_or_else(|| anyhow!("Missing :name parameter"))?
                .as_str().unwrap().to_string(),
            category: params.remove("category").and_then(|v| v.as_str().map(String::from)),
            regulatory_framework: params.remove("regulatory").and_then(|v| v.as_str().map(String::from)),
            min_asset_requirement: params.remove("min-assets").and_then(|v| v.as_f64()),
            metadata: if params.is_empty() { None } else { Some(params) },
        }))
    }

    fn build_create_service(mut params: HashMap<String, serde_json::Value>) -> Result<TaxonomyCrudStatement> {
        Ok(TaxonomyCrudStatement::CreateService(CreateService {
            service_code: params.remove("code")
                .ok_or_else(|| anyhow!("Missing :code parameter"))?
                .as_str().unwrap().to_string(),
            name: params.remove("name")
                .ok_or_else(|| anyhow!("Missing :name parameter"))?
                .as_str().unwrap().to_string(),
            category: params.remove("category").and_then(|v| v.as_str().map(String::from)),
            sla_definition: params.remove("sla"),
            options: Vec::new(),
        }))
    }

    fn build_create_onboarding(mut params: HashMap<String, serde_json::Value>) -> Result<TaxonomyCrudStatement> {
        let cbu_id_str = params.remove("cbu-id")
            .ok_or_else(|| anyhow!("Missing :cbu-id parameter"))?;
        let cbu_id = Uuid::parse_str(cbu_id_str.as_str().unwrap())
            .map_err(|e| anyhow!("Invalid UUID: {}", e))?;
        
        Ok(TaxonomyCrudStatement::CreateOnboarding(CreateOnboarding {
            cbu_id,
            initiated_by: params.remove("initiated-by")
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_else(|| "system".to_string()),
            metadata: if params.is_empty() { None } else { Some(params) },
        }))
    }

    fn build_add_products(mut params: HashMap<String, serde_json::Value>) -> Result<TaxonomyCrudStatement> {
        let onboarding_id_str = params.remove("onboarding-id")
            .ok_or_else(|| anyhow!("Missing :onboarding-id parameter"))?;
        let onboarding_id = Uuid::parse_str(onboarding_id_str.as_str().unwrap())
            .map_err(|e| anyhow!("Invalid UUID: {}", e))?;
        
        let products = params.remove("products")
            .ok_or_else(|| anyhow!("Missing :products parameter"))?;
        let product_codes: Vec<String> = serde_json::from_value(products)
            .map_err(|e| anyhow!("Invalid products list: {}", e))?;
        
        Ok(TaxonomyCrudStatement::AddProductsToOnboarding(AddProductsToOnboarding {
            onboarding_id,
            product_codes,
            auto_discover_services: params.remove("auto-discover")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        }))
    }

    fn build_configure_service(mut params: HashMap<String, serde_json::Value>) -> Result<TaxonomyCrudStatement> {
        let onboarding_id_str = params.remove("onboarding-id")
            .ok_or_else(|| anyhow!("Missing :onboarding-id parameter"))?;
        let onboarding_id = Uuid::parse_str(onboarding_id_str.as_str().unwrap())
            .map_err(|e| anyhow!("Invalid UUID: {}", e))?;
        
        let service_code = params.remove("service-code")
            .ok_or_else(|| anyhow!("Missing :service-code parameter"))?
            .as_str().unwrap().to_string();
        
        Ok(TaxonomyCrudStatement::ConfigureService(ConfigureService {
            onboarding_id,
            service_code,
            options: params,
        }))
    }

    fn build_discover_services(mut params: HashMap<String, serde_json::Value>) -> Result<TaxonomyCrudStatement> {
        let product_id_str = params.remove("product-id")
            .ok_or_else(|| anyhow!("Missing :product-id parameter"))?;
        let product_id = Uuid::parse_str(product_id_str.as_str().unwrap())
            .map_err(|e| anyhow!("Invalid UUID: {}", e))?;
        
        Ok(TaxonomyCrudStatement::DiscoverServices(DiscoverServices {
            product_id,
            include_optional: params.remove("include-optional")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        }))
    }

    fn build_query_workflow(mut params: HashMap<String, serde_json::Value>) -> Result<TaxonomyCrudStatement> {
        let onboarding_id_str = params.remove("onboarding-id")
            .ok_or_else(|| anyhow!("Missing :onboarding-id parameter"))?;
        let onboarding_id = Uuid::parse_str(onboarding_id_str.as_str().unwrap())
            .map_err(|e| anyhow!("Invalid UUID: {}", e))?;
        
        Ok(TaxonomyCrudStatement::QueryWorkflow(QueryWorkflow {
            onboarding_id,
            include_history: params.remove("include-history")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        }))
    }
    fn parse_create_product(input: &str) -> Result<TaxonomyCrudStatement> {
        let product_code = Self::extract_code(input)
            .or_else(|| Self::extract_after_keyword(input, "code"))
            .ok_or_else(|| anyhow!("Product code not found"))?;

        let name = Self::extract_quoted(input)
            .or_else(|| Self::extract_between(input, "called", "with"))
            .or_else(|| Self::extract_after_keyword(input, "product"))
            .unwrap_or_else(|| product_code.replace('_', " "));

        let category = Self::extract_after_keyword(input, "category").or_else(|| {
            if input.contains("custody") {
                Some("Custody".to_string())
            } else if input.contains("prime") {
                Some("Prime Services".to_string())
            } else if input.contains("fund") {
                Some("Fund Services".to_string())
            } else {
                None
            }
        });

        let min_asset =
            Self::extract_number(input, "minimum").or_else(|| Self::extract_number(input, "min"));

        Ok(TaxonomyCrudStatement::CreateProduct(CreateProduct {
            product_code,
            name,
            category,
            regulatory_framework: Self::extract_regulatory_framework(input),
            min_asset_requirement: min_asset,
            metadata: None,
        }))
    }

    fn parse_create_service(input: &str) -> Result<TaxonomyCrudStatement> {
        let service_code = Self::extract_code(input)
            .or_else(|| Self::extract_after_keyword(input, "service"))
            .ok_or_else(|| anyhow!("Service code not found"))?;

        let name = Self::extract_quoted(input).unwrap_or_else(|| service_code.replace('_', " "));

        let mut options = Vec::new();

        // Parse options if mentioned
        if input.contains("options") || input.contains("choices") {
            options = Self::extract_service_options(input);
        }

        Ok(TaxonomyCrudStatement::CreateService(CreateService {
            service_code,
            name,
            category: None,
            sla_definition: None,
            options,
        }))
    }

    fn parse_create_onboarding(input: &str) -> Result<TaxonomyCrudStatement> {
        let cbu_id = Self::extract_uuid(input).ok_or_else(|| anyhow!("CBU ID not found"))?;

        let initiated_by = Self::extract_after_keyword(input, "by")
            .or_else(|| Self::extract_after_keyword(input, "initiated"))
            .unwrap_or_else(|| "system".to_string());

        Ok(TaxonomyCrudStatement::CreateOnboarding(CreateOnboarding {
            cbu_id,
            initiated_by,
            metadata: None,
        }))
    }

    fn parse_add_products(input: &str) -> Result<TaxonomyCrudStatement> {
        let onboarding_id =
            Self::extract_uuid(input).ok_or_else(|| anyhow!("Onboarding ID not found"))?;

        let product_codes = Self::extract_all_codes(input);

        if product_codes.is_empty() {
            return Err(anyhow!("No product codes found"));
        }

        let auto_discover = input.contains("auto") || input.contains("discover");

        Ok(TaxonomyCrudStatement::AddProductsToOnboarding(
            AddProductsToOnboarding {
                onboarding_id,
                product_codes,
                auto_discover_services: auto_discover,
            },
        ))
    }

    fn parse_configure_service(input: &str) -> Result<TaxonomyCrudStatement> {
        let onboarding_id =
            Self::extract_uuid(input).ok_or_else(|| anyhow!("Onboarding ID not found"))?;

        let service_code = Self::extract_code(input)
            .or_else(|| {
                if input.contains("settlement") {
                    Some("SETTLEMENT".to_string())
                } else if input.contains("reporting") {
                    Some("REPORTING".to_string())
                } else if input.contains("safekeeping") {
                    Some("SAFEKEEPING".to_string())
                } else {
                    None
                }
            })
            .ok_or_else(|| anyhow!("Service code not found"))?;

        let options = Self::extract_service_configuration(input);

        Ok(TaxonomyCrudStatement::ConfigureService(ConfigureService {
            onboarding_id,
            service_code,
            options,
        }))
    }

    fn parse_discover_services(input: &str) -> Result<TaxonomyCrudStatement> {
        let product_id =
            Self::extract_uuid(input).ok_or_else(|| anyhow!("Product ID not found"))?;

        let include_optional = input.contains("all") || input.contains("optional");

        Ok(TaxonomyCrudStatement::DiscoverServices(DiscoverServices {
            product_id,
            include_optional,
        }))
    }

    fn parse_query_workflow(input: &str) -> Result<TaxonomyCrudStatement> {
        let onboarding_id =
            Self::extract_uuid(input).ok_or_else(|| anyhow!("Onboarding ID not found"))?;

        let include_history = input.contains("history") || input.contains("full");

        Ok(TaxonomyCrudStatement::QueryWorkflow(QueryWorkflow {
            onboarding_id,
            include_history,
        }))
    }

    // Helper methods

    fn identify_operation(input: &str) -> Operation {
        if input.contains("create") || input.contains("add") || input.contains("new") {
            if input.contains("product") {
                Operation::CreateProduct
            } else if input.contains("service") {
                Operation::CreateService
            } else if input.contains("onboarding") || input.contains("workflow") {
                Operation::CreateOnboarding
            } else {
                Operation::Unknown
            }
        } else if input.contains("configure") || input.contains("set") {
            Operation::ConfigureService
        } else if input.contains("discover") || input.contains("find") {
            Operation::DiscoverServices
        } else if input.contains("query") || input.contains("status") {
            Operation::QueryWorkflow
        } else if input.contains("add") && input.contains("product") {
            Operation::AddProducts
        } else {
            Operation::Unknown
        }
    }

    fn extract_uuid(input: &str) -> Option<Uuid> {
        let uuid_regex =
            Regex::new(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}").ok()?;
        uuid_regex
            .find(input)
            .and_then(|m| Uuid::parse_str(m.as_str()).ok())
    }

    fn extract_code(input: &str) -> Option<String> {
        let code_regex = Regex::new(r"\b[A-Z][A-Z_]+[A-Z]\b").ok()?;
        code_regex.find(input).map(|m| m.as_str().to_string())
    }

    fn extract_all_codes(input: &str) -> Vec<String> {
        let code_regex = Regex::new(r"\b[A-Z][A-Z_]+[A-Z]\b").unwrap();
        code_regex
            .find_iter(input)
            .map(|m| m.as_str().to_string())
            .collect()
    }

    fn extract_quoted(input: &str) -> Option<String> {
        let quote_regex = Regex::new(r#"["']([^"']+)["']"#).ok()?;
        quote_regex
            .captures(input)
            .map(|c| c.get(1).unwrap().as_str().to_string())
    }

    fn extract_after_keyword(input: &str, keyword: &str) -> Option<String> {
        let pattern = format!(r"{}\s+(\S+)", keyword);
        let regex = Regex::new(&pattern).ok()?;
        regex
            .captures(input)
            .map(|c| c.get(1).unwrap().as_str().to_string())
    }

    fn extract_between(input: &str, start: &str, end: &str) -> Option<String> {
        let start_idx = input.find(start)?;
        let after_start = &input[start_idx + start.len()..];
        let end_idx = after_start.find(end)?;
        Some(after_start[..end_idx].trim().to_string())
    }

    fn extract_number(input: &str, keyword: &str) -> Option<f64> {
        let pattern = format!(
            r"{}\s+(?:assets?\s+)?(\d+(?:\.\d+)?)\s*(?:million|billion|k|m|b)?",
            keyword
        );
        let regex = Regex::new(&pattern).ok()?;
        regex
            .captures(input)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse::<f64>().ok())
            .map(|n| {
                if input.contains("million") {
                    n * 1_000_000.0
                } else if input.contains("billion") {
                    n * 1_000_000_000.0
                } else if input.contains("k") {
                    n * 1_000.0
                } else {
                    n
                }
            })
    }

    fn extract_regulatory_framework(input: &str) -> Option<String> {
        if input.contains("mifid") {
            Some("MiFID II".to_string())
        } else if input.contains("dodd") {
            Some("Dodd-Frank".to_string())
        } else if input.contains("basel") {
            Some("Basel III".to_string())
        } else if input.contains("ucits") {
            Some("UCITS".to_string())
        } else if input.contains("aifmd") {
            Some("AIFMD".to_string())
        } else {
            None
        }
    }

    fn extract_service_options(input: &str) -> Vec<ServiceOptionDef> {
        let mut options = Vec::new();

        // Extract market options
        if input.contains("market") {
            options.push(ServiceOptionDef {
                option_key: "markets".to_string(),
                option_type: "multi_select".to_string(),
                is_required: true,
                choices: vec![
                    "US_EQUITY".to_string(),
                    "EU_EQUITY".to_string(),
                    "APAC_EQUITY".to_string(),
                ],
                validation_rules: None,
            });
        }

        // Extract speed/frequency options
        if input.contains("speed") || input.contains("frequency") {
            let option_key = if input.contains("speed") {
                "speed"
            } else {
                "frequency"
            };
            let choices = if input.contains("speed") {
                vec!["T0".to_string(), "T1".to_string(), "T2".to_string()]
            } else {
                vec![
                    "daily".to_string(),
                    "weekly".to_string(),
                    "monthly".to_string(),
                ]
            };

            options.push(ServiceOptionDef {
                option_key: option_key.to_string(),
                option_type: "single_select".to_string(),
                is_required: false,
                choices,
                validation_rules: None,
            });
        }

        options
    }

    fn extract_service_configuration(input: &str) -> HashMap<String, serde_json::Value> {
        let mut config = HashMap::new();

        // Extract markets
        if input.contains("US") || input.contains("EU") || input.contains("APAC") {
            let mut markets = Vec::new();
            if input.contains("US") {
                markets.push("US_EQUITY");
            }
            if input.contains("EU") {
                markets.push("EU_EQUITY");
            }
            if input.contains("APAC") {
                markets.push("APAC_EQUITY");
            }
            config.insert("markets".to_string(), serde_json::json!(markets));
        }

        // Extract speed
        if let Some(speed) = Regex::new(r"T[0-2+]")
            .ok()
            .and_then(|r| r.find(input))
            .map(|m| m.as_str())
        {
            config.insert("speed".to_string(), serde_json::json!(speed));
        }

        config
    }

    // DSL parsing helper methods (for S-expression style)

    fn parse_product_create_dsl(parts: &[&str]) -> Result<TaxonomyCrudStatement> {
        let mut params = Self::parse_dsl_params(parts);

        Ok(TaxonomyCrudStatement::CreateProduct(CreateProduct {
            product_code: params
                .remove("code")
                .ok_or_else(|| anyhow!("Missing :code parameter"))?
                .as_str()
                .unwrap()
                .to_string(),
            name: params
                .remove("name")
                .ok_or_else(|| anyhow!("Missing :name parameter"))?
                .as_str()
                .unwrap()
                .to_string(),
            category: params
                .remove("category")
                .and_then(|v| v.as_str().map(String::from)),
            regulatory_framework: params
                .remove("regulatory")
                .and_then(|v| v.as_str().map(String::from)),
            min_asset_requirement: params.remove("min_assets").and_then(|v| v.as_f64()),
            metadata: Some(params),
        }))
    }

    fn parse_dsl_params(parts: &[&str]) -> HashMap<String, serde_json::Value> {
        let mut params = HashMap::new();
        let mut i = 0;

        while i < parts.len() {
            if parts[i].starts_with(':') {
                let key = parts[i].trim_start_matches(':');
                if i + 1 < parts.len() {
                    let value = parts[i + 1];
                    params.insert(key.to_string(), Self::parse_dsl_value(value));
                    i += 2;
                } else {
                    i += 1;
                }
            } else {
                i += 1;
            }
        }

        params
    }

    fn parse_dsl_value(value: &str) -> serde_json::Value {
        let cleaned = value.trim_matches('"').trim_matches('\'');

        // Try to parse as number
        if let Ok(n) = cleaned.parse::<f64>() {
            return serde_json::json!(n);
        }

        // Try to parse as boolean
        if cleaned == "true" || cleaned == "false" {
            return serde_json::json!(cleaned == "true");
        }

        // Try to parse as UUID
        if let Ok(uuid) = Uuid::parse_str(cleaned) {
            return serde_json::json!(uuid.to_string());
        }

        // Default to string
        serde_json::json!(cleaned)
    }

    // Simplified implementations for other DSL formats
    fn parse_service_create_dsl(_parts: &[&str]) -> Result<TaxonomyCrudStatement> {
        Err(anyhow!("Service create DSL parsing not yet implemented"))
    }

    fn parse_onboarding_create_dsl(_parts: &[&str]) -> Result<TaxonomyCrudStatement> {
        Err(anyhow!("Onboarding create DSL parsing not yet implemented"))
    }

    fn parse_products_add_dsl(_parts: &[&str]) -> Result<TaxonomyCrudStatement> {
        Err(anyhow!("Products add DSL parsing not yet implemented"))
    }

    fn parse_service_configure_dsl(_parts: &[&str]) -> Result<TaxonomyCrudStatement> {
        Err(anyhow!("Service configure DSL parsing not yet implemented"))
    }

    fn parse_services_discover_dsl(_parts: &[&str]) -> Result<TaxonomyCrudStatement> {
        Err(anyhow!("Services discover DSL parsing not yet implemented"))
    }

    fn parse_workflow_query_dsl(_parts: &[&str]) -> Result<TaxonomyCrudStatement> {
        Err(anyhow!("Workflow query DSL parsing not yet implemented"))
    }
}

#[derive(Debug, Clone, Copy)]
enum Operation {
    CreateProduct,
    CreateService,
    CreateOnboarding,
    AddProducts,
    ConfigureService,
    DiscoverServices,
    QueryWorkflow,
    Unknown,
}
