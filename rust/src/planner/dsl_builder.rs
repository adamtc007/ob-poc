//! DSL Builder - Helper for generating DSL source strings
//!
//! This is a simple string builder that knows DSL syntax.
//! It produces clean, formatted DSL source code.

/// Builder for generating DSL source code
#[derive(Debug, Clone)]
pub struct DslBuilder {
    lines: Vec<String>,
}

impl DslBuilder {
    pub fn new() -> Self {
        Self { lines: Vec::new() }
    }

    /// Build the final DSL source string
    pub fn build(self) -> String {
        self.lines.join("\n")
    }

    /// Add a comment
    pub fn comment(&mut self, text: &str) {
        self.lines.push(format!(";; {}", text));
    }

    /// Add a blank line
    pub fn blank(&mut self) {
        self.lines.push(String::new());
    }

    // =========================================================================
    // CBU Operations
    // =========================================================================

    /// Create a CBU
    pub fn cbu_create(
        &mut self, 
        name: &str, 
        client_type: &str, 
        jurisdiction: Option<&str>,
        binding: &str
    ) {
        let mut args = format!(r#"(cbu.create :name "{}" :client-type "{}""#, name, client_type);
        if let Some(j) = jurisdiction {
            args.push_str(&format!(r#" :jurisdiction "{}""#, j));
        }
        args.push_str(&format!(" :as {})", binding));
        self.lines.push(args);
    }

    /// Assign a role to an entity on a CBU
    pub fn cbu_assign_role(&mut self, cbu_ref: &str, entity_ref: &str, role: &str) {
        self.lines.push(format!(
            r#"(cbu.assign-role :cbu-id {} :entity-id {} :role "{}")"#,
            cbu_ref, entity_ref, role
        ));
    }

    /// Assign a role with ownership percentage (for UBOs)
    pub fn cbu_assign_role_with_ownership(
        &mut self,
        cbu_ref: &str,
        entity_ref: &str,
        role: &str,
        ownership_percentage: f64,
    ) {
        self.lines.push(format!(
            r#"(cbu.assign-role :cbu-id {} :entity-id {} :role "{}" :ownership-percentage {:.1})"#,
            cbu_ref, entity_ref, role, ownership_percentage
        ));
    }

    /// Set risk rating on a CBU
    pub fn cbu_set_risk_rating(&mut self, cbu_ref: &str, rating: &str, rationale: Option<&str>) {
        let mut line = format!(
            r#"(cbu.set-risk-rating :cbu-id {} :rating "{}""#,
            cbu_ref, rating
        );
        if let Some(r) = rationale {
            line.push_str(&format!(r#" :rationale "{}""#, r));
        }
        line.push(')');
        self.lines.push(line);
    }

    /// Get CBU status
    pub fn cbu_get_status(&mut self, cbu_ref: &str) {
        self.lines.push(format!("(cbu.get-status :cbu-id {})", cbu_ref));
    }

    /// List UBOs for a CBU
    pub fn cbu_list_ubos(&mut self, cbu_ref: &str) {
        self.lines.push(format!("(cbu.list-ubos :cbu-id {})", cbu_ref));
    }

    // =========================================================================
    // Entity Operations
    // =========================================================================

    /// Create a natural person entity
    pub fn entity_create_natural_person(&mut self, name: &str, binding: &str) {
        self.lines.push(format!(
            r#"(entity.create-natural-person :name "{}" :as {})"#,
            name, binding
        ));
    }

    /// Create a corporate entity
    pub fn entity_create_corporate(&mut self, name: &str, entity_type: &str, binding: &str) {
        self.lines.push(format!(
            r#"(entity.create-corporate :name "{}" :entity-type "{}" :as {})"#,
            name, entity_type, binding
        ));
    }

    /// Create a fund entity
    pub fn entity_create_fund(&mut self, name: &str, fund_type: &str, binding: &str) {
        self.lines.push(format!(
            r#"(entity.create-fund :name "{}" :fund-type "{}" :as {})"#,
            name, fund_type, binding
        ));
    }

    /// Set an attribute on an entity
    pub fn entity_set_attribute(&mut self, entity_ref: &str, attribute: &str, value: &str) {
        self.lines.push(format!(
            r#"(entity.set-attribute :entity-id {} :attribute "{}" :value "{}")"#,
            entity_ref, attribute, value
        ));
    }

    /// Link corporate structure (parent-subsidiary)
    pub fn entity_link_corporate_structure(
        &mut self,
        parent_ref: &str,
        subsidiary_ref: &str,
        relationship_type: &str,
        ownership_percentage: Option<f64>,
    ) {
        let mut line = format!(
            r#"(entity.link-structure :parent-id {} :subsidiary-id {} :relationship "{}""#,
            parent_ref, subsidiary_ref, relationship_type
        );
        if let Some(pct) = ownership_percentage {
            line.push_str(&format!(" :ownership-percentage {}", pct));
        }
        line.push(')');
        self.lines.push(line);
    }

    // =========================================================================
    // Document Operations
    // =========================================================================

    /// Catalog a document
    pub fn document_catalog(
        &mut self,
        document_type: &str,
        cbu_ref: &str,
        title: Option<&str>,
        binding: &str,
    ) {
        let mut line = format!(
            r#"(document.catalog :document-type "{}" :cbu-id {}"#,
            document_type, cbu_ref
        );
        if let Some(t) = title {
            line.push_str(&format!(r#" :title "{}""#, t));
        }
        line.push_str(&format!(" :as {})", binding));
        self.lines.push(line);
    }

    /// Extract attributes from a document
    pub fn document_extract(&mut self, document_ref: &str) {
        self.lines.push(format!("(document.extract :document-id {})", document_ref));
    }

    /// Link a document to an entity
    pub fn document_link_entity(&mut self, document_ref: &str, entity_ref: &str) {
        self.lines.push(format!(
            "(document.link-entity :document-id {} :entity-id {})",
            document_ref, entity_ref
        ));
    }

    /// List documents for a CBU
    pub fn document_list(&mut self, cbu_ref: &str, document_type_filter: Option<&str>) {
        let mut line = format!("(document.list :cbu-id {}", cbu_ref);
        if let Some(filter) = document_type_filter {
            line.push_str(&format!(r#" :document-type "{}""#, filter));
        }
        line.push(')');
        self.lines.push(line);
    }

    // =========================================================================
    // KYC Operations
    // =========================================================================

    /// Run a KYC check
    pub fn kyc_run_check(&mut self, cbu_ref: &str, check_type: &str) {
        self.lines.push(format!(
            r#"(kyc.run-check :cbu-id {} :check-type "{}")"#,
            cbu_ref, check_type
        ));
    }

    /// Validate all attributes
    pub fn kyc_validate_all_attributes(&mut self, cbu_ref: &str) {
        self.lines.push(format!(
            "(kyc.validate-attributes :cbu-id {} :all true)",
            cbu_ref
        ));
    }

    /// Validate a specific attribute
    pub fn kyc_validate_attribute(&mut self, cbu_ref: &str, attribute_code: &str) {
        self.lines.push(format!(
            r#"(kyc.validate-attribute :cbu-id {} :attribute "{}")"#,
            cbu_ref, attribute_code
        ));
    }
}

impl Default for DslBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_cbu_creation() {
        let mut b = DslBuilder::new();
        b.cbu_create("John Smith", "individual", Some("UK"), "@cbu");
        
        let dsl = b.build();
        assert_eq!(
            dsl,
            r#"(cbu.create :name "John Smith" :client-type "individual" :jurisdiction "UK" :as @cbu)"#
        );
    }

    #[test]
    fn test_full_onboarding_flow() {
        let mut b = DslBuilder::new();
        
        b.comment("Create CBU and entity");
        b.cbu_create("Acme Corp", "corporate", Some("UK"), "@cbu");
        b.entity_create_corporate("Acme Corp", "limited-company", "@company");
        b.cbu_assign_role("@cbu", "@company", "account_holder");
        
        b.blank();
        b.comment("Add document");
        b.document_catalog("CERT_OF_INCORPORATION", "@cbu", None, "@doc0");
        b.document_extract("@doc0");
        
        b.blank();
        b.comment("Add beneficial owner");
        b.entity_create_natural_person("Jane Owner", "@ubo0");
        b.cbu_assign_role_with_ownership("@cbu", "@ubo0", "beneficial_owner", 75.0);
        
        let dsl = b.build();
        println!("Full DSL:\n{}", dsl);
        
        assert!(dsl.contains("Acme Corp"));
        assert!(dsl.contains("beneficial_owner"));
        assert!(dsl.contains("75.0"));
    }
}
