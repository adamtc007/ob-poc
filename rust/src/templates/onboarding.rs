//! Onboarding Templates - Multi-statement DSL generation for product onboarding
//!
//! These templates generate complete onboarding workflows:
//! CBU → Resource Instance → Attributes → Activate → Delivery

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Onboarding template definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub product_code: String,
    pub parameters: Vec<OnboardingParam>,
    pub template_body: String,
}

/// Parameter for onboarding template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingParam {
    pub name: String,
    pub display: String,
    pub param_type: ParamType,
    pub required: bool,
    pub default: Option<String>,
    pub options: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamType {
    String,
    Enum,
    Boolean,
}

/// Registry of onboarding templates
pub struct OnboardingTemplateRegistry {
    templates: HashMap<String, OnboardingTemplate>,
}

impl OnboardingTemplateRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            templates: HashMap::new(),
        };
        registry.register_builtins();
        registry
    }

    pub fn get(&self, id: &str) -> Option<&OnboardingTemplate> {
        self.templates.get(id)
    }

    pub fn list(&self) -> Vec<&OnboardingTemplate> {
        self.templates.values().collect()
    }

    pub fn list_by_product(&self, product_code: &str) -> Vec<&OnboardingTemplate> {
        self.templates
            .values()
            .filter(|t| t.product_code == product_code)
            .collect()
    }

    fn register(&mut self, template: OnboardingTemplate) {
        self.templates.insert(template.id.clone(), template);
    }

    fn register_builtins(&mut self) {
        self.register(Self::global_custody_template());
        self.register(Self::fund_accounting_template());
        self.register(Self::ibor_template());
    }

    /// Global Custody onboarding template
    fn global_custody_template() -> OnboardingTemplate {
        OnboardingTemplate {
            id: "onboarding_global_custody".into(),
            name: "Global Custody Onboarding".into(),
            description: "Complete global custody setup with safekeeping and settlement".into(),
            product_code: "GLOB_CUSTODY".into(),
            parameters: vec![
                OnboardingParam {
                    name: "client_name".into(),
                    display: "Client Name".into(),
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                    options: None,
                },
                OnboardingParam {
                    name: "jurisdiction".into(),
                    display: "Jurisdiction".into(),
                    param_type: ParamType::String,
                    required: true,
                    default: Some("US".into()),
                    options: None,
                },
                OnboardingParam {
                    name: "client_type".into(),
                    display: "Client Type".into(),
                    param_type: ParamType::Enum,
                    required: true,
                    default: Some("fund".into()),
                    options: Some(vec![
                        "fund".into(),
                        "corporate".into(),
                        "individual".into(),
                    ]),
                },
                OnboardingParam {
                    name: "base_currency".into(),
                    display: "Base Currency".into(),
                    param_type: ParamType::String,
                    required: true,
                    default: Some("USD".into()),
                    options: None,
                },
                OnboardingParam {
                    name: "account_type".into(),
                    display: "Account Type".into(),
                    param_type: ParamType::Enum,
                    required: true,
                    default: Some("SEGREGATED".into()),
                    options: Some(vec!["SEGREGATED".into(), "OMNIBUS".into()]),
                },
                OnboardingParam {
                    name: "include_settlement".into(),
                    display: "Include Settlement".into(),
                    param_type: ParamType::Boolean,
                    required: false,
                    default: Some("true".into()),
                    options: None,
                },
            ],
            template_body: r#";; Global Custody Onboarding for {{client_name}}

;; 1. Create Client Business Unit
(cbu.ensure
    :name "{{client_name}}"
    :jurisdiction "{{jurisdiction}}"
    :client-type "{{client_type}}"
    :as @client)

;; 2. Create Custody Account
(resource.create
    :cbu-id @client
    :resource-type "CUSTODY_ACCT"
    :instance-url "https://custody.bank.com/accounts/{{client_slug}}-001"
    :instance-id "{{client_slug_upper}}-CUSTODY-001"
    :instance-name "{{client_name}} Custody Account"
    :as @custody)

(resource.set-attr :instance-id @custody :attr "resource.account.account_number" :value "CUST-{{client_slug_upper}}-001")
(resource.set-attr :instance-id @custody :attr "resource.account.account_name" :value "{{client_name}} - Main Custody")
(resource.set-attr :instance-id @custody :attr "resource.account.base_currency" :value "{{base_currency}}")
(resource.set-attr :instance-id @custody :attr "resource.account.account_type" :value "{{account_type}}")

(resource.activate :instance-id @custody)

(delivery.record :cbu-id @client :product "GLOB_CUSTODY" :service "SAFEKEEPING" :instance-id @custody)
(delivery.complete :cbu-id @client :product "GLOB_CUSTODY" :service "SAFEKEEPING")
"#
            .into(),
        }
    }

    /// Fund Accounting onboarding template
    fn fund_accounting_template() -> OnboardingTemplate {
        OnboardingTemplate {
            id: "onboarding_fund_accounting".into(),
            name: "Fund Accounting Onboarding".into(),
            description: "NAV calculation and investor accounting setup".into(),
            product_code: "FUND_ACCT".into(),
            parameters: vec![
                OnboardingParam {
                    name: "fund_name".into(),
                    display: "Fund Name".into(),
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                    options: None,
                },
                OnboardingParam {
                    name: "jurisdiction".into(),
                    display: "Jurisdiction".into(),
                    param_type: ParamType::String,
                    required: true,
                    default: Some("LU".into()),
                    options: None,
                },
                OnboardingParam {
                    name: "fund_type".into(),
                    display: "Fund Type".into(),
                    param_type: ParamType::Enum,
                    required: true,
                    default: Some("fund".into()),
                    options: Some(vec!["fund".into(), "corporate".into()]),
                },
                OnboardingParam {
                    name: "valuation_frequency".into(),
                    display: "Valuation Frequency".into(),
                    param_type: ParamType::Enum,
                    required: true,
                    default: Some("DAILY".into()),
                    options: Some(vec!["DAILY".into(), "WEEKLY".into(), "MONTHLY".into()]),
                },
                OnboardingParam {
                    name: "pricing_source".into(),
                    display: "Pricing Source".into(),
                    param_type: ParamType::Enum,
                    required: true,
                    default: Some("Bloomberg".into()),
                    options: Some(vec!["Bloomberg".into(), "Reuters".into(), "ICE".into()]),
                },
                OnboardingParam {
                    name: "nav_cutoff".into(),
                    display: "NAV Cutoff Time".into(),
                    param_type: ParamType::String,
                    required: true,
                    default: Some("16:00 CET".into()),
                    options: None,
                },
                OnboardingParam {
                    name: "include_investor_accounting".into(),
                    display: "Include Investor Accounting".into(),
                    param_type: ParamType::Boolean,
                    required: false,
                    default: Some("true".into()),
                    options: None,
                },
            ],
            template_body: r#";; Fund Accounting Onboarding for {{fund_name}}

;; 1. Create Fund as CBU
(cbu.ensure
    :name "{{fund_name}}"
    :jurisdiction "{{jurisdiction}}"
    :client-type "{{fund_type}}"
    :as @fund)

;; 2. Create NAV Calculation Engine
(resource.create
    :cbu-id @fund
    :resource-type "NAV_ENGINE"
    :instance-url "https://nav.fundservices.com/funds/{{fund_slug}}"
    :instance-id "{{fund_slug_upper}}-NAV-001"
    :instance-name "{{fund_name}} NAV Engine"
    :as @nav)

(resource.set-attr :instance-id @nav :attr "resource.fund.fund_code" :value "{{fund_slug_upper}}-001")
(resource.set-attr :instance-id @nav :attr "resource.fund.valuation_frequency" :value "{{valuation_frequency}}")
(resource.set-attr :instance-id @nav :attr "resource.fund.pricing_source" :value "{{pricing_source}}")
(resource.set-attr :instance-id @nav :attr "resource.fund.nav_cutoff_time" :value "{{nav_cutoff}}")

(resource.activate :instance-id @nav)

(delivery.record :cbu-id @fund :product "FUND_ACCT" :service "NAV_CALC" :instance-id @nav)
(delivery.complete :cbu-id @fund :product "FUND_ACCT" :service "NAV_CALC")
"#
            .into(),
        }
    }

    /// Middle Office IBOR onboarding template
    fn ibor_template() -> OnboardingTemplate {
        OnboardingTemplate {
            id: "onboarding_ibor".into(),
            name: "Middle Office IBOR Onboarding".into(),
            description: "IBOR system and P&L attribution setup".into(),
            product_code: "MO_IBOR".into(),
            parameters: vec![
                OnboardingParam {
                    name: "client_name".into(),
                    display: "Client Name".into(),
                    param_type: ParamType::String,
                    required: true,
                    default: None,
                    options: None,
                },
                OnboardingParam {
                    name: "jurisdiction".into(),
                    display: "Jurisdiction".into(),
                    param_type: ParamType::String,
                    required: true,
                    default: Some("UK".into()),
                    options: None,
                },
                OnboardingParam {
                    name: "client_type".into(),
                    display: "Client Type".into(),
                    param_type: ParamType::Enum,
                    required: true,
                    default: Some("corporate".into()),
                    options: Some(vec![
                        "fund".into(),
                        "corporate".into(),
                        "individual".into(),
                    ]),
                },
                OnboardingParam {
                    name: "base_currency".into(),
                    display: "Base Currency".into(),
                    param_type: ParamType::String,
                    required: true,
                    default: Some("GBP".into()),
                    options: None,
                },
                OnboardingParam {
                    name: "accounting_basis".into(),
                    display: "Accounting Basis".into(),
                    param_type: ParamType::Enum,
                    required: true,
                    default: Some("TRADE_DATE".into()),
                    options: Some(vec!["TRADE_DATE".into(), "SETTLEMENT_DATE".into()]),
                },
                OnboardingParam {
                    name: "position_source".into(),
                    display: "Position Source".into(),
                    param_type: ParamType::String,
                    required: true,
                    default: Some("OMS".into()),
                    options: None,
                },
                OnboardingParam {
                    name: "include_pnl".into(),
                    display: "Include P&L Attribution".into(),
                    param_type: ParamType::Boolean,
                    required: false,
                    default: Some("true".into()),
                    options: None,
                },
            ],
            template_body: r#";; Middle Office IBOR Onboarding for {{client_name}}

;; 1. Create Client Business Unit
(cbu.ensure
    :name "{{client_name}}"
    :jurisdiction "{{jurisdiction}}"
    :client-type "{{client_type}}"
    :as @client)

;; 2. Create IBOR System
(resource.create
    :cbu-id @client
    :resource-type "IBOR_SYSTEM"
    :instance-url "https://ibor.platform.com/portfolios/{{client_slug}}"
    :instance-id "{{client_slug_upper}}-IBOR-001"
    :instance-name "{{client_name}} IBOR"
    :as @ibor)

(resource.set-attr :instance-id @ibor :attr "resource.ibor.portfolio_code" :value "{{client_slug_upper}}-MASTER")
(resource.set-attr :instance-id @ibor :attr "resource.ibor.accounting_basis" :value "{{accounting_basis}}")
(resource.set-attr :instance-id @ibor :attr "resource.account.base_currency" :value "{{base_currency}}")
(resource.set-attr :instance-id @ibor :attr "resource.ibor.position_source" :value "{{position_source}}")

(resource.activate :instance-id @ibor)

(delivery.record :cbu-id @client :product "MO_IBOR" :service "POSITION_MGMT" :instance-id @ibor)
(delivery.record :cbu-id @client :product "MO_IBOR" :service "TRADE_CAPTURE" :instance-id @ibor)
(delivery.complete :cbu-id @client :product "MO_IBOR" :service "POSITION_MGMT")
(delivery.complete :cbu-id @client :product "MO_IBOR" :service "TRADE_CAPTURE")
"#
            .into(),
        }
    }
}

impl Default for OnboardingTemplateRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple template renderer for onboarding templates
pub struct OnboardingRenderer;

impl OnboardingRenderer {
    /// Render an onboarding template with parameters
    pub fn render(
        template: &OnboardingTemplate,
        params: &HashMap<String, String>,
    ) -> Result<String, String> {
        let mut result = template.template_body.clone();

        // Validate required parameters
        for param in &template.parameters {
            if param.required && !params.contains_key(&param.name)
                && param.default.is_none() {
                    return Err(format!("Missing required parameter: {}", param.name));
                }
        }

        // Apply parameter substitutions
        for param in &template.parameters {
            let value = params
                .get(&param.name)
                .or(param.default.as_ref())
                .cloned()
                .unwrap_or_default();

            let placeholder = format!("{{{{{}}}}}", param.name);
            result = result.replace(&placeholder, &value);

            // Generate derived values for common patterns
            if param.name == "client_name" || param.name == "fund_name" {
                let slug = slugify(&value);
                let slug_upper = slug.to_uppercase();

                // Replace derived placeholders
                if param.name == "client_name" {
                    result = result.replace("{{client_slug}}", &slug);
                    result = result.replace("{{client_slug_upper}}", &slug_upper);
                } else {
                    result = result.replace("{{fund_slug}}", &slug);
                    result = result.replace("{{fund_slug_upper}}", &slug_upper);
                }
            }
        }

        Ok(result)
    }
}

/// Convert a name to a URL-safe slug
fn slugify(name: &str) -> String {
    name.to_lowercase()
        .replace(' ', "-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_has_templates() {
        let registry = OnboardingTemplateRegistry::new();
        assert_eq!(registry.list().len(), 3);
    }

    #[test]
    fn test_get_template_by_id() {
        let registry = OnboardingTemplateRegistry::new();
        let template = registry.get("onboarding_global_custody");
        assert!(template.is_some());
        assert_eq!(template.unwrap().product_code, "GLOB_CUSTODY");
    }

    #[test]
    fn test_list_by_product() {
        let registry = OnboardingTemplateRegistry::new();
        let templates = registry.list_by_product("GLOB_CUSTODY");
        assert_eq!(templates.len(), 1);
    }

    #[test]
    fn test_render_global_custody() {
        let registry = OnboardingTemplateRegistry::new();
        let template = registry.get("onboarding_global_custody").unwrap();

        let mut params = HashMap::new();
        params.insert("client_name".into(), "Apex Capital".into());
        params.insert("jurisdiction".into(), "US".into());
        params.insert("client_type".into(), "fund".into());
        params.insert("base_currency".into(), "USD".into());
        params.insert("account_type".into(), "SEGREGATED".into());

        let result = OnboardingRenderer::render(template, &params).unwrap();

        assert!(result.contains("Apex Capital"));
        assert!(result.contains("cbu.ensure"));
        assert!(result.contains("resource.create"));
        assert!(result.contains("CUSTODY_ACCT"));
        assert!(result.contains("apex-capital"));
        assert!(result.contains("APEX-CAPITAL"));
        assert!(result.contains("delivery.record"));
    }

    #[test]
    fn test_render_fund_accounting() {
        let registry = OnboardingTemplateRegistry::new();
        let template = registry.get("onboarding_fund_accounting").unwrap();

        let mut params = HashMap::new();
        params.insert("fund_name".into(), "Pacific Growth Fund".into());
        params.insert("jurisdiction".into(), "LU".into());
        params.insert("fund_type".into(), "fund".into());
        params.insert("valuation_frequency".into(), "DAILY".into());
        params.insert("pricing_source".into(), "Bloomberg".into());
        params.insert("nav_cutoff".into(), "16:00 CET".into());

        let result = OnboardingRenderer::render(template, &params).unwrap();

        assert!(result.contains("Pacific Growth Fund"));
        assert!(result.contains("NAV_ENGINE"));
        assert!(result.contains("DAILY"));
        assert!(result.contains("Bloomberg"));
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Apex Capital"), "apex-capital");
        assert_eq!(slugify("Pacific Growth Fund"), "pacific-growth-fund");
        assert_eq!(slugify("Test & Co."), "test--co");
    }
}
