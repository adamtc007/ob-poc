//! Integration test for Document-Attribute CRUD operations
//!
//! Tests the full lifecycle:
//! 1. Create document type
//! 2. Link existing attributes to document type
//! 3. Verify mappings
//! 4. Unlink attributes
//! 5. Verify cleanup

use crate::services::document_attribute_crud_service::DocumentAttributeCrudService;
use crate::services::document_extraction_service::DocumentExtractionService;
use sqlx::PgPool;
use uuid::Uuid;

/// Test harness for document-attribute CRUD operations
pub struct DocumentAttributeTestHarness {
    crud_service: DocumentAttributeCrudService,
    #[allow(dead_code)]
    extraction_service: DocumentExtractionService,
    // Track created resources for cleanup
    created_doc_types: Vec<String>,
    created_mappings: Vec<(Uuid, Uuid)>, // (document_type_id, attribute_uuid)
}

impl DocumentAttributeTestHarness {
    pub fn new(pool: PgPool) -> Self {
        Self {
            crud_service: DocumentAttributeCrudService::new(pool.clone()),
            extraction_service: DocumentExtractionService::new(pool),
            created_doc_types: Vec::new(),
            created_mappings: Vec::new(),
        }
    }

    /// Run full CRUD test lifecycle
    pub async fn run_full_test(&mut self) -> Result<TestReport, String> {
        let mut report = TestReport::new();

        // =====================================================================
        // STEP 1: Create document type
        // =====================================================================
        println!("\n=== STEP 1: Create Document Type ===");

        let doc_type_result = self
            .crud_service
            .create_document_type(
                "TEST_NATIONAL_ID",
                "National ID Card (Test)",
                "IDENTITY",
                None, // domain
                Some("Test document type for national ID cards"),
            )
            .await?;

        report.add_step("Create doc type TEST_NATIONAL_ID", doc_type_result.success);
        println!("  Created TEST_NATIONAL_ID: {:?}", doc_type_result);
        self.created_doc_types.push("TEST_NATIONAL_ID".to_string());

        let doc_type_id = doc_type_result.id.ok_or("No document type ID returned")?;

        // =====================================================================
        // STEP 2: Find existing attributes to link
        // =====================================================================
        println!("\n=== STEP 2: Find Existing Attributes ===");

        // Try to find an existing attribute in the registry
        let attr1 = self
            .crud_service
            .get_attribute_by_code("full_legal_name")
            .await?;
        let attr2 = self
            .crud_service
            .get_attribute_by_code("date_of_birth")
            .await?;

        let attr1_uuid = match attr1 {
            Some(a) => {
                println!("  Found attribute: {} ({})", a.id, a.display_name);
                report.add_step("Find attribute full_legal_name", true);
                a.uuid
            }
            None => {
                println!("  Attribute full_legal_name not found in registry");
                report.add_step("Find attribute full_legal_name", false);
                return Ok(report);
            }
        };

        let attr2_uuid = match attr2 {
            Some(a) => {
                println!("  Found attribute: {} ({})", a.id, a.display_name);
                report.add_step("Find attribute date_of_birth", true);
                a.uuid
            }
            None => {
                println!("  Attribute date_of_birth not found in registry");
                report.add_step("Find attribute date_of_birth", false);
                return Ok(report);
            }
        };

        // =====================================================================
        // STEP 3: Link attributes to document type
        // =====================================================================
        println!("\n=== STEP 3: Link Attributes to Document Type ===");

        let link1_result = self
            .crud_service
            .link_attribute_to_document_type(
                doc_type_id,
                attr1_uuid,
                "OCR",              // extraction_method
                Some(true),         // is_required
                Some("name_field"), // field_name
            )
            .await?;

        report.add_step(
            "Link full_legal_name to TEST_NATIONAL_ID",
            link1_result.success,
        );
        println!("  Linked full_legal_name: {:?}", link1_result);
        if link1_result.success {
            self.created_mappings.push((doc_type_id, attr1_uuid));
        }

        let link2_result = self
            .crud_service
            .link_attribute_to_document_type(
                doc_type_id,
                attr2_uuid,
                "OCR",             // extraction_method
                Some(false),       // is_required (optional)
                Some("dob_field"), // field_name
            )
            .await?;

        report.add_step(
            "Link date_of_birth to TEST_NATIONAL_ID",
            link2_result.success,
        );
        println!("  Linked date_of_birth: {:?}", link2_result);
        if link2_result.success {
            self.created_mappings.push((doc_type_id, attr2_uuid));
        }

        // =====================================================================
        // STEP 4: Verify mappings exist
        // =====================================================================
        println!("\n=== STEP 4: Verify Mappings ===");

        let mappings = self
            .crud_service
            .get_mappings_for_document_type(doc_type_id)
            .await?;
        report.add_step("Get mappings for TEST_NATIONAL_ID", mappings.len() == 2);
        println!("  Found {} mappings for TEST_NATIONAL_ID:", mappings.len());
        for m in &mappings {
            println!(
                "    - {} (method: {}, required: {})",
                m.attribute_uuid, m.extraction_method, m.is_required
            );
        }

        // =====================================================================
        // STEP 5: Verify mapping_exists check
        // =====================================================================
        println!("\n=== STEP 5: Verify mapping_exists ===");

        let exists = self
            .crud_service
            .mapping_exists(doc_type_id, attr1_uuid)
            .await?;
        report.add_step("mapping_exists returns true", exists);
        println!("  mapping_exists(doc_type, attr1): {}", exists);

        // =====================================================================
        // STEP 6: Unlink an attribute
        // =====================================================================
        println!("\n=== STEP 6: Unlink Attribute ===");

        let unlink_result = self
            .crud_service
            .unlink_attribute_from_document_type(doc_type_id, attr2_uuid)
            .await?;

        report.add_step("Unlink date_of_birth", unlink_result.success);
        println!("  Unlinked date_of_birth: {:?}", unlink_result);

        // Remove from tracking
        self.created_mappings
            .retain(|(d, a)| !(*d == doc_type_id && *a == attr2_uuid));

        // =====================================================================
        // STEP 7: Verify unlink worked
        // =====================================================================
        println!("\n=== STEP 7: Verify Unlink ===");

        let remaining_mappings = self
            .crud_service
            .get_mappings_for_document_type(doc_type_id)
            .await?;

        report.add_step("Mapping removed", remaining_mappings.len() == 1);
        println!(
            "  Remaining mappings: {} (expected 1)",
            remaining_mappings.len()
        );

        // =====================================================================
        // STEP 8: Verify bidirectional lookup
        // =====================================================================
        println!("\n=== STEP 8: Verify Bidirectional Lookup ===");

        let doc_types = self
            .crud_service
            .get_document_types_for_attribute(attr1_uuid)
            .await?;
        report.add_step("Reverse lookup works", doc_types.contains(&doc_type_id));
        println!("  Document types for full_legal_name: {:?}", doc_types);

        Ok(report)
    }

    /// Cleanup test data
    pub async fn cleanup(&self) -> Result<(), String> {
        println!("\n=== CLEANUP ===");

        // Remove mappings first
        for (doc_type_id, attr_uuid) in &self.created_mappings {
            let result = self
                .crud_service
                .unlink_attribute_from_document_type(*doc_type_id, *attr_uuid)
                .await;
            println!("  Unlink mapping: {:?}", result);
        }

        // Delete document types
        for doc_type in self.created_doc_types.iter().rev() {
            let result = self.crud_service.delete_document_type(doc_type).await;
            println!("  Delete {}: {:?}", doc_type, result);
        }

        println!("  Cleanup complete");
        Ok(())
    }
}

/// Test report
pub struct TestReport {
    steps: Vec<(String, bool)>,
}

impl TestReport {
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    pub fn add_step(&mut self, name: &str, passed: bool) {
        self.steps.push((name.to_string(), passed));
    }

    pub fn print_summary(&self) {
        println!("\n========================================");
        println!("TEST SUMMARY");
        println!("========================================");

        let mut passed = 0;
        let mut failed = 0;

        for (name, success) in &self.steps {
            let status = if *success { "PASS" } else { "FAIL" };
            println!("  [{}] {}", status, name);
            if *success {
                passed += 1;
            } else {
                failed += 1;
            }
        }

        println!("----------------------------------------");
        println!("  Total: {} passed, {} failed", passed, failed);
        println!("========================================");
    }

    pub fn all_passed(&self) -> bool {
        self.steps.iter().all(|(_, passed)| *passed)
    }
}

impl Default for TestReport {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_creation() {
        let mut report = TestReport::new();
        report.add_step("Test step", true);
        assert!(report.all_passed());
    }
}
