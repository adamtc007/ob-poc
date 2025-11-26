//! Fluent DSL Builder API
//!
//! Provides a type-safe, fluent interface for building DSL source text.

use std::fmt::Write;

/// Main DSL builder for constructing DSL source text
pub struct DslBuilder {
    statements: Vec<String>,
}

impl DslBuilder {
    pub fn new() -> Self {
        Self {
            statements: Vec::new(),
        }
    }

    /// Add a raw DSL statement
    pub fn raw(&mut self, dsl: &str) -> &mut Self {
        self.statements.push(dsl.to_string());
        self
    }

    // ========== CBU Operations ==========

    /// Start building a cbu.create statement
    pub fn cbu_create(&mut self) -> CbuCreateBuilder<'_> {
        CbuCreateBuilder::new(self)
    }

    /// Add a cbu.read statement
    pub fn cbu_read(&mut self, cbu_id: &str) -> &mut Self {
        self.statements
            .push(format!("(cbu.read :cbu-id \"{}\")", cbu_id));
        self
    }

    /// Start building a cbu.update statement
    pub fn cbu_update(&mut self, cbu_id: &str) -> CbuUpdateBuilder<'_> {
        CbuUpdateBuilder::new(self, cbu_id.to_string())
    }

    /// Add a cbu.delete statement
    pub fn cbu_delete(&mut self, cbu_id: &str) -> &mut Self {
        self.statements
            .push(format!("(cbu.delete :cbu-id \"{}\")", cbu_id));
        self
    }

    // ========== Product Operations ==========

    /// Start building a product.create statement
    pub fn product_create(&mut self) -> ProductCreateBuilder<'_> {
        ProductCreateBuilder::new(self)
    }

    /// Add a product.read statement
    pub fn product_read_by_id(&mut self, product_id: &str) -> &mut Self {
        self.statements
            .push(format!("(product.read :product-id \"{}\")", product_id));
        self
    }

    /// Add a product.read by code statement
    pub fn product_read_by_code(&mut self, product_code: &str) -> &mut Self {
        self.statements
            .push(format!("(product.read :product-code \"{}\")", product_code));
        self
    }

    /// Add a product.delete statement
    pub fn product_delete(&mut self, product_id: &str) -> &mut Self {
        self.statements
            .push(format!("(product.delete :product-id \"{}\")", product_id));
        self
    }

    // ========== Service Operations ==========

    /// Start building a service.create statement
    pub fn service_create(&mut self) -> ServiceCreateBuilder<'_> {
        ServiceCreateBuilder::new(self)
    }

    /// Add a service.read statement
    pub fn service_read(&mut self, service_id: &str) -> &mut Self {
        self.statements
            .push(format!("(service.read :service-id \"{}\")", service_id));
        self
    }

    /// Add a service.delete statement
    pub fn service_delete(&mut self, service_id: &str) -> &mut Self {
        self.statements
            .push(format!("(service.delete :service-id \"{}\")", service_id));
        self
    }

    /// Link a service to a product
    pub fn service_link_product(
        &mut self,
        service_id: &str,
        product_id: &str,
        is_mandatory: bool,
    ) -> &mut Self {
        self.statements.push(format!(
            "(service.link-product :service-id \"{}\" :product-id \"{}\" :is-mandatory {})",
            service_id, product_id, is_mandatory
        ));
        self
    }

    // ========== Lifecycle Resource Operations ==========

    /// Start building a lifecycle-resource.create statement
    pub fn lifecycle_resource_create(&mut self) -> ResourceCreateBuilder<'_> {
        ResourceCreateBuilder::new(self)
    }

    /// Add a lifecycle-resource.read statement
    pub fn lifecycle_resource_read(&mut self, resource_id: &str) -> &mut Self {
        self.statements.push(format!(
            "(lifecycle-resource.read :resource-id \"{}\")",
            resource_id
        ));
        self
    }

    /// Add a lifecycle-resource.delete statement
    pub fn lifecycle_resource_delete(&mut self, resource_id: &str) -> &mut Self {
        self.statements.push(format!(
            "(lifecycle-resource.delete :resource-id \"{}\")",
            resource_id
        ));
        self
    }

    /// Link a resource to a service
    pub fn lifecycle_resource_link_service(
        &mut self,
        resource_id: &str,
        service_id: &str,
    ) -> &mut Self {
        self.statements.push(format!(
            "(lifecycle-resource.link-service :resource-id \"{}\" :service-id \"{}\")",
            resource_id, service_id
        ));
        self
    }

    // ========== Build ==========

    /// Build the final DSL source text
    pub fn build(&self) -> String {
        self.statements.join("\n")
    }

    /// Get number of statements
    pub fn len(&self) -> usize {
        self.statements.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.statements.is_empty()
    }

    /// Add a statement (used by sub-builders)
    pub(crate) fn add_statement(&mut self, stmt: String) {
        self.statements.push(stmt);
    }
}

impl Default for DslBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ========== Sub-Builders ==========

/// Builder for cbu.create statements
pub struct CbuCreateBuilder<'a> {
    parent: &'a mut DslBuilder,
    name: Option<String>,
    client_type: Option<String>,
    jurisdiction: Option<String>,
    nature_purpose: Option<String>,
    description: Option<String>,
}

impl<'a> CbuCreateBuilder<'a> {
    fn new(parent: &'a mut DslBuilder) -> Self {
        Self {
            parent,
            name: None,
            client_type: None,
            jurisdiction: None,
            nature_purpose: None,
            description: None,
        }
    }

    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    pub fn client_type(mut self, client_type: &str) -> Self {
        self.client_type = Some(client_type.to_string());
        self
    }

    pub fn jurisdiction(mut self, jurisdiction: &str) -> Self {
        self.jurisdiction = Some(jurisdiction.to_string());
        self
    }

    pub fn nature_purpose(mut self, nature_purpose: &str) -> Self {
        self.nature_purpose = Some(nature_purpose.to_string());
        self
    }

    pub fn description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }

    pub fn done(self) -> &'a mut DslBuilder {
        let mut stmt = String::from("(cbu.create");

        if let Some(name) = &self.name {
            write!(stmt, " :cbu-name \"{}\"", name).unwrap();
        }
        if let Some(ct) = &self.client_type {
            write!(stmt, " :client-type \"{}\"", ct).unwrap();
        }
        if let Some(j) = &self.jurisdiction {
            write!(stmt, " :jurisdiction \"{}\"", j).unwrap();
        }
        if let Some(np) = &self.nature_purpose {
            write!(stmt, " :nature-purpose \"{}\"", np).unwrap();
        }
        if let Some(d) = &self.description {
            write!(stmt, " :description \"{}\"", d).unwrap();
        }

        stmt.push(')');
        self.parent.add_statement(stmt);
        self.parent
    }
}

/// Builder for cbu.update statements
pub struct CbuUpdateBuilder<'a> {
    parent: &'a mut DslBuilder,
    cbu_id: String,
    updates: Vec<(String, String)>,
}

impl<'a> CbuUpdateBuilder<'a> {
    fn new(parent: &'a mut DslBuilder, cbu_id: String) -> Self {
        Self {
            parent,
            cbu_id,
            updates: Vec::new(),
        }
    }

    pub fn set(mut self, field: &str, value: &str) -> Self {
        self.updates.push((field.to_string(), value.to_string()));
        self
    }

    pub fn done(self) -> &'a mut DslBuilder {
        let mut stmt = format!("(cbu.update :cbu-id \"{}\"", self.cbu_id);

        for (field, value) in &self.updates {
            write!(stmt, " :{} \"{}\"", field, value).unwrap();
        }

        stmt.push(')');
        self.parent.add_statement(stmt);
        self.parent
    }
}

/// Builder for product.create statements
pub struct ProductCreateBuilder<'a> {
    parent: &'a mut DslBuilder,
    code: Option<String>,
    name: Option<String>,
    category: Option<String>,
    regulatory_framework: Option<String>,
    min_asset_requirement: Option<f64>,
}

impl<'a> ProductCreateBuilder<'a> {
    fn new(parent: &'a mut DslBuilder) -> Self {
        Self {
            parent,
            code: None,
            name: None,
            category: None,
            regulatory_framework: None,
            min_asset_requirement: None,
        }
    }

    pub fn code(mut self, code: &str) -> Self {
        self.code = Some(code.to_string());
        self
    }

    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    pub fn category(mut self, category: &str) -> Self {
        self.category = Some(category.to_string());
        self
    }

    pub fn regulatory_framework(mut self, framework: &str) -> Self {
        self.regulatory_framework = Some(framework.to_string());
        self
    }

    pub fn min_asset_requirement(mut self, amount: f64) -> Self {
        self.min_asset_requirement = Some(amount);
        self
    }

    pub fn done(self) -> &'a mut DslBuilder {
        let mut stmt = String::from("(product.create");

        if let Some(code) = &self.code {
            write!(stmt, " :product-code \"{}\"", code).unwrap();
        }
        if let Some(name) = &self.name {
            write!(stmt, " :name \"{}\"", name).unwrap();
        }
        if let Some(cat) = &self.category {
            write!(stmt, " :category \"{}\"", cat).unwrap();
        }
        if let Some(rf) = &self.regulatory_framework {
            write!(stmt, " :regulatory-framework \"{}\"", rf).unwrap();
        }
        if let Some(mar) = self.min_asset_requirement {
            write!(stmt, " :min-asset-requirement {}", mar).unwrap();
        }

        stmt.push(')');
        self.parent.add_statement(stmt);
        self.parent
    }
}

/// Builder for service.create statements
pub struct ServiceCreateBuilder<'a> {
    parent: &'a mut DslBuilder,
    code: Option<String>,
    name: Option<String>,
    category: Option<String>,
}

impl<'a> ServiceCreateBuilder<'a> {
    fn new(parent: &'a mut DslBuilder) -> Self {
        Self {
            parent,
            code: None,
            name: None,
            category: None,
        }
    }

    pub fn code(mut self, code: &str) -> Self {
        self.code = Some(code.to_string());
        self
    }

    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    pub fn category(mut self, category: &str) -> Self {
        self.category = Some(category.to_string());
        self
    }

    pub fn done(self) -> &'a mut DslBuilder {
        let mut stmt = String::from("(service.create");

        if let Some(code) = &self.code {
            write!(stmt, " :service-code \"{}\"", code).unwrap();
        }
        if let Some(name) = &self.name {
            write!(stmt, " :name \"{}\"", name).unwrap();
        }
        if let Some(cat) = &self.category {
            write!(stmt, " :category \"{}\"", cat).unwrap();
        }

        stmt.push(')');
        self.parent.add_statement(stmt);
        self.parent
    }
}

/// Builder for lifecycle-resource.create statements
pub struct ResourceCreateBuilder<'a> {
    parent: &'a mut DslBuilder,
    code: Option<String>,
    name: Option<String>,
    resource_type: Option<String>,
    vendor: Option<String>,
}

impl<'a> ResourceCreateBuilder<'a> {
    fn new(parent: &'a mut DslBuilder) -> Self {
        Self {
            parent,
            code: None,
            name: None,
            resource_type: None,
            vendor: None,
        }
    }

    pub fn code(mut self, code: &str) -> Self {
        self.code = Some(code.to_string());
        self
    }

    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    pub fn resource_type(mut self, rtype: &str) -> Self {
        self.resource_type = Some(rtype.to_string());
        self
    }

    pub fn vendor(mut self, vendor: &str) -> Self {
        self.vendor = Some(vendor.to_string());
        self
    }

    pub fn done(self) -> &'a mut DslBuilder {
        let mut stmt = String::from("(lifecycle-resource.create");

        if let Some(code) = &self.code {
            write!(stmt, " :resource-code \"{}\"", code).unwrap();
        }
        if let Some(name) = &self.name {
            write!(stmt, " :name \"{}\"", name).unwrap();
        }
        if let Some(rt) = &self.resource_type {
            write!(stmt, " :resource-type \"{}\"", rt).unwrap();
        }
        if let Some(v) = &self.vendor {
            write!(stmt, " :vendor \"{}\"", v).unwrap();
        }

        stmt.push(')');
        self.parent.add_statement(stmt);
        self.parent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cbu_create() {
        let dsl = DslBuilder::new()
            .cbu_create()
            .name("TechCorp")
            .client_type("HEDGE_FUND")
            .jurisdiction("GB")
            .nature_purpose("Investment")
            .description("Test CBU")
            .done()
            .build();

        assert!(dsl.contains("cbu.create"));
        assert!(dsl.contains(":cbu-name \"TechCorp\""));
        assert!(dsl.contains(":client-type \"HEDGE_FUND\""));
    }

    #[test]
    fn test_product_create() {
        let dsl = DslBuilder::new()
            .product_create()
            .code("ASSET_MGMT")
            .name("Asset Management")
            .category("Investment")
            .min_asset_requirement(1_000_000.0)
            .done()
            .build();

        assert!(dsl.contains("product.create"));
        assert!(dsl.contains(":product-code \"ASSET_MGMT\""));
        assert!(dsl.contains(":min-asset-requirement 1000000"));
    }

    #[test]
    fn test_multiple_statements() {
        let dsl = DslBuilder::new()
            .product_create()
            .code("HEDGE_FUND")
            .name("Hedge Fund")
            .done()
            .service_create()
            .code("CUSTODY")
            .name("Custody Services")
            .done()
            .service_link_product("@last_service", "@last_product", true)
            .build();

        let lines: Vec<&str> = dsl.lines().collect();
        assert_eq!(lines.len(), 3);
    }
}
