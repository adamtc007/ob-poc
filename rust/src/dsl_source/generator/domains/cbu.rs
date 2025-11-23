//! CBU DSL generation
#![allow(dead_code)]

pub struct CbuDslGenerator;

impl CbuDslGenerator {
    pub fn create(name: &str, client_type: &str, jurisdiction: &str, nature_purpose: &str, description: &str) -> String {
        format!(
            "(cbu.create :cbu-name \"{}\" :client-type \"{}\" :jurisdiction \"{}\" :nature-purpose \"{}\" :description \"{}\")",
            name, client_type, jurisdiction, nature_purpose, description
        )
    }
    
    pub fn read(cbu_id: &str) -> String {
        format!("(cbu.read :cbu-id \"{}\")", cbu_id)
    }
    
    pub fn delete(cbu_id: &str) -> String {
        format!("(cbu.delete :cbu-id \"{}\")", cbu_id)
    }
}
