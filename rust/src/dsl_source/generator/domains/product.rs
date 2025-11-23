//! Product DSL generation
#![allow(dead_code)]

pub struct ProductDslGenerator;

impl ProductDslGenerator {
    pub fn create(code: &str, name: &str, category: Option<&str>, min_asset: Option<f64>) -> String {
        let mut dsl = format!("(product.create :product-code \"{}\" :name \"{}\"", code, name);
        if let Some(cat) = category {
            dsl.push_str(&format!(" :category \"{}\"", cat));
        }
        if let Some(amt) = min_asset {
            dsl.push_str(&format!(" :min-asset-requirement {}", amt));
        }
        dsl.push(')');
        dsl
    }
    
    pub fn read_by_id(product_id: &str) -> String {
        format!("(product.read :product-id \"{}\")", product_id)
    }
    
    pub fn read_by_code(product_code: &str) -> String {
        format!("(product.read :product-code \"{}\")", product_code)
    }
    
    pub fn delete(product_id: &str) -> String {
        format!("(product.delete :product-id \"{}\")", product_id)
    }
}
