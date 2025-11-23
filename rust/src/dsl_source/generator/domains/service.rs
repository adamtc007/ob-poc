//! Service DSL generation
#![allow(dead_code)]

pub struct ServiceDslGenerator;

impl ServiceDslGenerator {
    pub fn create(code: &str, name: &str, category: Option<&str>) -> String {
        let mut dsl = format!("(service.create :service-code \"{}\" :name \"{}\"", code, name);
        if let Some(cat) = category {
            dsl.push_str(&format!(" :category \"{}\"", cat));
        }
        dsl.push(')');
        dsl
    }
    
    pub fn read(service_id: &str) -> String {
        format!("(service.read :service-id \"{}\")", service_id)
    }
    
    pub fn delete(service_id: &str) -> String {
        format!("(service.delete :service-id \"{}\")", service_id)
    }
    
    pub fn link_product(service_id: &str, product_id: &str, is_mandatory: bool) -> String {
        format!(
            "(service.link-product :service-id \"{}\" :product-id \"{}\" :is-mandatory {})",
            service_id, product_id, is_mandatory
        )
    }
}
