//! Database verification for the test harness

use crate::database::DslRepository;
use crate::dsl_test_harness::types::{
    ErrorVerification, SymbolVerification, ValidationErrorInfo, VerificationResult,
};
use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

/// Database verifier for checking all writes succeeded
pub struct DatabaseVerifier {
    pool: PgPool,
    dsl_repo: DslRepository,
}

impl DatabaseVerifier {
    /// Create a new database verifier
    pub fn new(pool: PgPool) -> Self {
        Self {
            dsl_repo: DslRepository::new(pool.clone()),
            pool,
        }
    }

    /// Verify all database writes by querying back
    pub async fn verify(
        &self,
        request_id: Uuid,
        expected_product_codes: &[String],
        expected_dsl: &str,
        expect_ast: bool,
    ) -> Result<VerificationResult> {
        let business_reference = format!("onboarding:{}", request_id);

        // ═══════════════════════════════════════════════════════════════
        // VERIFY 1: Onboarding request exists
        // ═══════════════════════════════════════════════════════════════
        let request: Option<(Uuid, Option<String>, Option<serde_json::Value>)> = sqlx::query_as(
            r#"
            SELECT request_id, request_state, validation_errors
            FROM "ob-poc".onboarding_requests
            WHERE request_id = $1
            "#,
        )
        .bind(request_id)
        .fetch_optional(&self.pool)
        .await?;

        let (request_exists, request_state, errors_stored, error_count) = match &request {
            Some((_id, state, errors)) => {
                let err_count = errors
                    .as_ref()
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                (
                    true,
                    state.clone().unwrap_or_default(),
                    err_count > 0,
                    err_count,
                )
            }
            None => (false, String::new(), false, 0),
        };

        // ═══════════════════════════════════════════════════════════════
        // VERIFY 2: Products are linked
        // ═══════════════════════════════════════════════════════════════
        let products_linked: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM "ob-poc".onboarding_products
            WHERE request_id = $1
            "#,
        )
        .bind(request_id)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0);

        // ═══════════════════════════════════════════════════════════════
        // VERIFY 3: DSL instance exists and content matches
        // ═══════════════════════════════════════════════════════════════
        let dsl_instance = self
            .dsl_repo
            .get_instance_by_reference(&business_reference)
            .await?;

        let (dsl_instance_exists, dsl_version) = match &dsl_instance {
            Some(inst) => (true, inst.current_version),
            None => (false, 0),
        };

        let loaded_dsl = self.dsl_repo.load_dsl(&business_reference).await?;
        let dsl_content_matches = loaded_dsl
            .as_ref()
            .map(|(content, _)| content == expected_dsl)
            .unwrap_or(false);

        // ═══════════════════════════════════════════════════════════════
        // VERIFY 4: AST exists and is valid
        // ═══════════════════════════════════════════════════════════════
        let loaded_ast = self.dsl_repo.load_ast(&business_reference).await?;

        let (ast_exists, ast_has_expressions, ast_has_symbol_table, symbol_count) =
            match &loaded_ast {
                Some(ast) => {
                    let has_expr = ast.get("expressions").is_some();
                    let has_st = ast.get("symbol_table").is_some();
                    let sym_count = ast
                        .get("symbol_table")
                        .and_then(|st| st.as_object())
                        .map(|o| o.len())
                        .unwrap_or(0);
                    (true, has_expr, has_st, sym_count)
                }
                None => (false, false, false, 0),
            };

        // ═══════════════════════════════════════════════════════════════
        // VERIFY 5: All checks passed?
        // ═══════════════════════════════════════════════════════════════
        let all_checks_passed = request_exists
            && products_linked as usize == expected_product_codes.len()
            && (!expect_ast
                || (dsl_instance_exists
                    && dsl_content_matches
                    && ast_exists
                    && ast_has_expressions
                    && ast_has_symbol_table))
            && (expect_ast || errors_stored);

        Ok(VerificationResult {
            request_exists,
            request_state,
            products_linked: products_linked as usize,
            expected_products: expected_product_codes.len(),
            dsl_instance_exists,
            dsl_content_matches,
            dsl_version,
            ast_exists,
            ast_has_expressions,
            ast_has_symbol_table,
            symbol_count,
            errors_stored,
            error_count,
            all_checks_passed,
        })
    }

    /// Verify specific symbols exist in stored AST
    pub async fn verify_symbols(
        &self,
        request_id: Uuid,
        expected_symbols: &[&str],
    ) -> Result<SymbolVerification> {
        let business_reference = format!("onboarding:{}", request_id);
        let ast = self.dsl_repo.load_ast(&business_reference).await?;

        let symbol_table = ast
            .as_ref()
            .and_then(|a| a.get("symbol_table"))
            .and_then(|st| st.as_object())
            .cloned()
            .unwrap_or_default();

        let found: Vec<String> = symbol_table.keys().cloned().collect();
        let missing: Vec<String> = expected_symbols
            .iter()
            .filter(|s| !found.contains(&s.to_string()))
            .map(|s| s.to_string())
            .collect();

        Ok(SymbolVerification {
            expected: expected_symbols.iter().map(|s| s.to_string()).collect(),
            found,
            missing: missing.clone(),
            all_present: missing.is_empty(),
        })
    }

    /// Verify validation errors match expected codes
    pub async fn verify_errors(
        &self,
        request_id: Uuid,
        expected_codes: &[&str],
    ) -> Result<ErrorVerification> {
        let result: Option<(Option<serde_json::Value>,)> = sqlx::query_as(
            r#"
            SELECT validation_errors
            FROM "ob-poc".onboarding_requests
            WHERE request_id = $1
            "#,
        )
        .bind(request_id)
        .fetch_optional(&self.pool)
        .await?;

        let stored: Vec<ValidationErrorInfo> = result
            .and_then(|(errors,)| errors)
            .map(|v| serde_json::from_value(v).unwrap_or_default())
            .unwrap_or_default();

        let stored_codes: Vec<String> = stored.iter().map(|e| e.code.clone()).collect();
        let missing: Vec<String> = expected_codes
            .iter()
            .filter(|c| !stored_codes.contains(&c.to_string()))
            .map(|c| c.to_string())
            .collect();

        Ok(ErrorVerification {
            expected: expected_codes.iter().map(|c| c.to_string()).collect(),
            found: stored_codes,
            missing: missing.clone(),
            all_present: missing.is_empty(),
        })
    }
}
