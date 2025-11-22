//! DSL Executor with UUID Support
//!
//! Executes DSL with UUID-based attribute references, extracting UUIDs from the AST,
//! binding them to values using the ValueBinder, and persisting to the database.

use uuid::Uuid;

use crate::domains::attributes::execution_context::ExecutionContext;
use crate::execution::value_binder::ValueBinder;
use crate::parser::parse_program;
use crate::parser_ast::{Form, Program, Value, VerbForm};
use crate::services::AttributeService;

pub struct DslExecutor {
    service: AttributeService,
    binder: ValueBinder,
}

impl DslExecutor {
    pub fn new(service: AttributeService) -> Self {
        Self {
            service,
            binder: ValueBinder::new(),
        }
    }

    pub fn with_binder(service: AttributeService, binder: ValueBinder) -> Self {
        Self { service, binder }
    }

    /// Extract all UUID references from a program
    pub fn extract_uuids(&self, program: &Program) -> Vec<Uuid> {
        let mut uuids = Vec::new();

        for form in program {
            self.extract_uuids_from_form(form, &mut uuids);
        }

        uuids
    }

    /// Recursively extract UUIDs from a form
    fn extract_uuids_from_form(&self, form: &Form, uuids: &mut Vec<Uuid>) {
        match form {
            Form::Verb(verb) => {
                self.extract_uuids_from_verb(verb, uuids);
            }
            Form::Comment(_) => {
                // Comments don't contain UUIDs
            }
        }
    }

    /// Extract UUIDs from a verb form
    fn extract_uuids_from_verb(&self, verb: &VerbForm, uuids: &mut Vec<Uuid>) {
        for (_, value) in &verb.pairs {
            self.extract_uuids_from_value(value, uuids);
        }
    }

    /// Extract UUIDs from a value
    fn extract_uuids_from_value(&self, value: &Value, uuids: &mut Vec<Uuid>) {
        match value {
            Value::AttrUuid(uuid) => {
                uuids.push(*uuid);
            }
            Value::List(list) => {
                for item in list {
                    self.extract_uuids_from_value(item, uuids);
                }
            }
            _ => {}
        }
    }

    /// Execute DSL with UUID resolution and value binding
    pub async fn execute(&self, dsl: &str, entity_id: Uuid) -> Result<ExecutionResult, String> {
        // 1. Parse DSL
        let program = parse_program(dsl).map_err(|e| format!("Parse error: {:?}", e))?;

        // 2. Extract UUIDs
        let uuids = self.extract_uuids(&program);
        log::info!("Extracted {} UUIDs from DSL", uuids.len());

        // 3. Create execution context
        let mut context = ExecutionContext::new();

        // 4. Bind values for all UUIDs
        let bind_results = self.binder.bind_all(uuids.clone(), &mut context).await;

        // 5. Count successful bindings
        let successful = bind_results.iter().filter(|r| r.is_ok()).count();
        let errors: Vec<String> = bind_results
            .into_iter()
            .filter_map(|r| r.err().map(|e| e.to_string()))
            .collect();

        log::info!(
            "Bound {}/{} attributes successfully",
            successful,
            uuids.len()
        );

        // 6. Store bound values in database
        let mut stored_count = 0;
        for uuid in &uuids {
            if let Some(value) = context.get_value(uuid) {
                match self
                    .service
                    .set_by_uuid(entity_id, *uuid, value.clone(), Some("dsl_executor"))
                    .await
                {
                    Ok(_) => stored_count += 1,
                    Err(e) => {
                        log::error!("Failed to store attribute {}: {}", uuid, e);
                    }
                }
            }
        }

        log::info!(
            "Stored {}/{} attributes to database",
            stored_count,
            successful
        );

        Ok(ExecutionResult {
            entity_id,
            attributes_resolved: successful,
            attributes_stored: stored_count,
            bound_attributes: context.bound_attributes(),
            errors,
        })
    }

    /// Execute DSL and return the execution context (for inspection)
    pub async fn execute_with_context(
        &self,
        dsl: &str,
        entity_id: Uuid,
    ) -> Result<(ExecutionResult, ExecutionContext), String> {
        let program = parse_program(dsl).map_err(|e| format!("Parse error: {:?}", e))?;

        let uuids = self.extract_uuids(&program);
        let mut context = ExecutionContext::new();

        let bind_results = self.binder.bind_all(uuids.clone(), &mut context).await;

        let successful = bind_results.iter().filter(|r| r.is_ok()).count();
        let errors: Vec<String> = bind_results
            .into_iter()
            .filter_map(|r| r.err().map(|e| e.to_string()))
            .collect();

        let mut stored_count = 0;
        for uuid in &uuids {
            if let Some(value) = context.get_value(uuid) {
                if self
                    .service
                    .set_by_uuid(entity_id, *uuid, value.clone(), Some("dsl_executor"))
                    .await
                    .is_ok()
                {
                    stored_count += 1;
                }
            }
        }

        let result = ExecutionResult {
            entity_id,
            attributes_resolved: successful,
            attributes_stored: stored_count,
            bound_attributes: context.bound_attributes(),
            errors,
        };

        Ok((result, context))
    }
}

#[derive(Debug)]
pub struct ExecutionResult {
    pub entity_id: Uuid,
    pub attributes_resolved: usize,
    pub attributes_stored: usize,
    pub bound_attributes: Vec<Uuid>,
    pub errors: Vec<String>,
}

impl ExecutionResult {
    pub fn is_success(&self) -> bool {
        self.errors.is_empty() && self.attributes_resolved > 0
    }

    pub fn success_rate(&self) -> f64 {
        if self.attributes_resolved == 0 {
            0.0
        } else {
            self.attributes_stored as f64 / self.attributes_resolved as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_extract_uuids_from_dsl() {
        use crate::domains::attributes::validator::AttributeValidator;

        // Create mock service (won't actually use DB in this test)
        let pool = sqlx::PgPool::connect_lazy("postgresql://test").unwrap();
        let validator = AttributeValidator::new();
        let service = AttributeService::from_pool(pool, validator);
        let executor = DslExecutor::new(service);

        let dsl = r#"
            (kyc.collect
                :first-name @attr{3020d46f-472c-5437-9647-1b0682c35935}
                :last-name @attr{0af112fd-ec04-5938-84e8-6e5949db0b52}
            )
        "#;

        let program = parse_program(dsl).unwrap();
        let uuids = executor.extract_uuids(&program);

        assert_eq!(uuids.len(), 2);

        // Check both UUIDs are present (order may vary)
        let first_name_uuid = Uuid::parse_str("3020d46f-472c-5437-9647-1b0682c35935").unwrap();
        let last_name_uuid = Uuid::parse_str("0af112fd-ec04-5938-84e8-6e5949db0b52").unwrap();

        assert!(uuids.contains(&first_name_uuid));
        assert!(uuids.contains(&last_name_uuid));
    }

    #[tokio::test]
    async fn test_extract_uuids_from_nested_dsl() {
        use crate::domains::attributes::validator::AttributeValidator;

        let pool = sqlx::PgPool::connect_lazy("postgresql://test").unwrap();
        let validator = AttributeValidator::new();
        let service = AttributeService::from_pool(pool, validator);
        let executor = DslExecutor::new(service);

        let dsl = r#"
            (kyc.collect
                :names [@attr{3020d46f-472c-5437-9647-1b0682c35935} @attr{0af112fd-ec04-5938-84e8-6e5949db0b52}]
            )
        "#;

        let program = parse_program(dsl).unwrap();
        let uuids = executor.extract_uuids(&program);

        assert_eq!(uuids.len(), 2);
    }

    #[test]
    fn test_execution_result() {
        let result = ExecutionResult {
            entity_id: Uuid::new_v4(),
            attributes_resolved: 10,
            attributes_stored: 8,
            bound_attributes: vec![],
            errors: vec![],
        };

        assert!(result.is_success());
        assert_eq!(result.success_rate(), 0.8);
    }
}
