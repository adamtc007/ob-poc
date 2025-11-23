//! Lifecycle Resource DSL generation
#![allow(dead_code)]

pub struct LifecycleResourceDslGenerator;

impl LifecycleResourceDslGenerator {
    pub fn create(code: &str, name: &str, resource_type: &str, vendor: Option<&str>) -> String {
        let mut dsl = format!(
            "(lifecycle-resource.create :resource-code \"{}\" :name \"{}\" :resource-type \"{}\"",
            code, name, resource_type
        );
        if let Some(v) = vendor {
            dsl.push_str(&format!(" :vendor \"{}\"", v));
        }
        dsl.push(')');
        dsl
    }
    
    pub fn read(resource_id: &str) -> String {
        format!("(lifecycle-resource.read :resource-id \"{}\")", resource_id)
    }
    
    pub fn delete(resource_id: &str) -> String {
        format!("(lifecycle-resource.delete :resource-id \"{}\")", resource_id)
    }
    
    pub fn link_service(resource_id: &str, service_id: &str) -> String {
        format!(
            "(lifecycle-resource.link-service :resource-id \"{}\" :service-id \"{}\")",
            resource_id, service_id
        )
    }
}
