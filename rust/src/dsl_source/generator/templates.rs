//! DSL Template Engine
//!
//! Pre-defined templates for common DSL patterns

use std::collections::HashMap;

/// Template-based DSL generation
pub struct DslTemplate {
    template: String,
    variables: HashMap<String, String>,
}

impl DslTemplate {
    /// Create a new template
    pub fn new(template: &str) -> Self {
        Self {
            template: template.to_string(),
            variables: HashMap::new(),
        }
    }
    
    /// Set a template variable
    pub fn set(&mut self, key: &str, value: &str) -> &mut Self {
        self.variables.insert(key.to_string(), value.to_string());
        self
    }
    
    /// Render the template with variables substituted
    pub fn render(&self) -> String {
        let mut result = self.template.clone();
        for (key, value) in &self.variables {
            let placeholder = format!("{{{{{}}}}}", key);
            result = result.replace(&placeholder, value);
        }
        result
    }
    
    // ========== Pre-defined Templates ==========
    
    /// Template for creating a complete CBU with entity
    pub fn cbu_with_entity() -> Self {
        Self::new(r#"
(cbu.create :cbu-name "{{cbu_name}}" :client-type "{{client_type}}" :jurisdiction "{{jurisdiction}}" :nature-purpose "{{nature_purpose}}" :description "{{description}}")
(kyc.declare-entity :entity-type "{{entity_type}}" :name "{{entity_name}}" :data {})
(cbu.attach-entity :cbu-id @last_cbu :entity-id @last_entity :role "{{role}}")
"#.trim())
    }
    
    /// Template for product with services
    pub fn product_with_services() -> Self {
        Self::new(r#"
(product.create :product-code "{{product_code}}" :name "{{product_name}}" :category "{{category}}")
{{#services}}
(service.create :service-code "{{service_code}}" :name "{{service_name}}")
(service.link-product :service-code "{{service_code}}" :product-code "{{product_code}}" :is-mandatory {{is_mandatory}})
{{/services}}
"#.trim())
    }
    
    /// Template for onboarding workflow
    pub fn onboarding_workflow() -> Self {
        Self::new(r#"
(cbu.create :cbu-name "{{cbu_name}}" :client-type "{{client_type}}" :jurisdiction "{{jurisdiction}}" :nature-purpose "{{nature_purpose}}" :description "{{description}}")
(product.read :product-code "{{product_code}}")
(service.discover :product-id @last_product :include-optional true)
"#.trim())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_template_rendering() {
        let mut template = DslTemplate::new("(cbu.create :cbu-name \"{{name}}\")");
        template.set("name", "TechCorp");
        
        let result = template.render();
        assert_eq!(result, "(cbu.create :cbu-name \"TechCorp\")");
    }
    
    #[test]
    fn test_cbu_with_entity_template() {
        let mut template = DslTemplate::cbu_with_entity();
        template
            .set("cbu_name", "TechCorp")
            .set("client_type", "COMPANY")
            .set("jurisdiction", "GB")
            .set("nature_purpose", "Investment")
            .set("description", "Test")
            .set("entity_type", "COMPANY")
            .set("entity_name", "TechCorp Ltd")
            .set("role", "PRINCIPAL");
        
        let result = template.render();
        assert!(result.contains("cbu.create"));
        assert!(result.contains("kyc.declare-entity"));
        assert!(result.contains("cbu.attach-entity"));
    }
}
