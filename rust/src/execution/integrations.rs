//! External Integrations Framework
//!
//! This module provides a framework for connecting DSL operations to external systems
//! such as risk engines, document stores, compliance databases, and other enterprise systems.
//! It follows a plugin architecture where integrations can be registered dynamically.

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;

use serde_json::Value;
use std::collections::HashMap;

use std::time::Duration;

use super::{ExecutionContext, ExternalIntegration};
use crate::data_dictionary::AttributeId;
use crate::dsl::operations::ExecutableDslOperation as DslOperation;

/// HTTP-based integration for REST APIs
pub(crate) struct HttpIntegration {
    base_url: String,
    client: Client,
    headers: HashMap<String, String>,
}

impl HttpIntegration {
    pub fn new(base_url: impl Into<String>, timeout_seconds: u64) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_seconds))
            .build()?;

        Ok(Self {
            base_url: base_url.into(),
            client,
            headers: HashMap::new(),
        })
    }

    pub(crate) fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    pub(crate) fn with_auth_token(self, token: impl Into<String>) -> Self {
        self.with_header("Authorization", format!("Bearer {}", token.into()))
    }

    pub(crate) fn with_api_key(self, key: impl Into<String>) -> Self {
        self.with_header("X-API-Key", key.into())
    }

    async fn post_json(&self, endpoint: &str, payload: &Value) -> Result<Value> {
        let url = format!("{}/{}", self.base_url.trim_end_matches('/'), endpoint);

        let mut request = self.client.post(&url).json(payload);

        for (key, value) in &self.headers {
            request = request.header(key, value);
        }

        let response = request
            .send()
            .await
            .context("Failed to send HTTP request")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "HTTP request failed with status: {} for URL: {}",
                response.status(),
                url
            ));
        }

        let json: Value = response
            .json()
            .await
            .context("Failed to parse HTTP response as JSON")?;

        Ok(json)
    }

    async fn get_json(
        &self,
        endpoint: &str,
        query_params: Option<&[(&str, &str)]>,
    ) -> Result<Value> {
        let url = format!("{}/{}", self.base_url.trim_end_matches('/'), endpoint);

        let mut request = self.client.get(&url);

        if let Some(params) = query_params {
            request = request.query(params);
        }

        for (key, value) in &self.headers {
            request = request.header(key, value);
        }

        let response = request
            .send()
            .await
            .context("Failed to send HTTP request")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "HTTP request failed with status: {} for URL: {}",
                response.status(),
                url
            ));
        }

        let json: Value = response
            .json()
            .await
            .context("Failed to parse HTTP response as JSON")?;

        Ok(json)
    }
}

#[async_trait]
impl ExternalIntegration for HttpIntegration {
    fn name(&self) -> &str {
        "http_integration"
    }

    async fn execute(&self, operation: &DslOperation, context: &ExecutionContext) -> Result<Value> {
        // Generic HTTP execution - override in specific integrations
        let payload = serde_json::json!({
            "operation": operation,
            "context": {
                "business_unit_id": context.business_unit_id,
                "session_id": context.session_id,
                "domain": context.domain,
                "executor": context.executor,
            }
        });

        self.post_json("execute", &payload).await
    }

    async fn validate(&self, operation: &DslOperation) -> Result<bool> {
        // Basic validation - check if the operation type is supported
        // Specific integrations should override this
        Ok(!operation.operation_type.is_empty())
    }
}

/// Risk Engine Integration
pub(crate) struct RiskEngineIntegration {
    http: HttpIntegration,
}

impl RiskEngineIntegration {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Result<Self> {
        let http = HttpIntegration::new(base_url, 30)?.with_api_key(api_key);

        Ok(Self { http })
    }
}

#[async_trait]
impl ExternalIntegration for RiskEngineIntegration {
    fn name(&self) -> &str {
        "risk_engine"
    }

    async fn execute(&self, operation: &DslOperation, context: &ExecutionContext) -> Result<Value> {
        match operation.operation_type.as_str() {
            "collect" => {
                if let Some(from) = operation.parameters.get("from").and_then(|v| v.as_str()) {
                    if from == "risk-engine" {
                        return self.collect_risk_data(operation, context).await;
                    }
                }
            }
            "validate" => {
                if let Some(attr_id) = operation.parameters.get("attribute_id") {
                    if let Ok(attribute_id) = serde_json::from_value::<AttributeId>(attr_id.clone())
                    {
                        if attribute_id.to_string().contains("risk") {
                            return self.validate_risk_data(operation, context).await;
                        }
                    }
                }
            }
            _ => {}
        }

        // Default fallback
        self.http.execute(operation, context).await
    }

    async fn validate(&self, operation: &DslOperation) -> Result<bool> {
        // Validate that we can handle this operation
        match operation.operation_type.as_str() {
            "collect" => Ok(
                operation.parameters.get("from").and_then(|v| v.as_str()) == Some("risk-engine")
            ),
            "validate" => Ok(operation
                .parameters
                .get("attribute_id")
                .and_then(|v| serde_json::from_value::<AttributeId>(v.clone()).ok())
                .is_some_and(|attr_id| attr_id.to_string().contains("risk"))),
            _ => Ok(false),
        }
    }
}

impl RiskEngineIntegration {
    async fn collect_risk_data(
        &self,
        operation: &DslOperation,
        context: &ExecutionContext,
    ) -> Result<Value> {
        let entity_id = context.business_unit_id.clone();

        let query_params = vec![
            ("entity_id", entity_id.as_str()),
            ("domain", context.domain.as_str()),
        ];

        let risk_data = self
            .http
            .get_json("risk/assess", Some(&query_params))
            .await?;

        Ok(serde_json::json!({
            "source": "risk_engine",
            "data": risk_data,
            "collected_at": chrono::Utc::now().to_rfc3339(),
            "operation_id": operation.metadata.get("id").cloned().unwrap_or_default()
        }))
    }

    async fn validate_risk_data(
        &self,
        operation: &DslOperation,
        _context: &ExecutionContext,
    ) -> Result<Value> {
        let value = operation
            .parameters
            .get("value")
            .cloned()
            .unwrap_or_default();

        let validation_request = serde_json::json!({
            "attribute_id": operation.parameters.get("attribute_id"),
            "value": value,
            "validation_type": "risk_assessment"
        });

        let validation_result = self
            .http
            .post_json("risk/validate", &validation_request)
            .await?;

        Ok(serde_json::json!({
            "source": "risk_engine",
            "validation": validation_result,
            "validated_at": chrono::Utc::now().to_rfc3339()
        }))
    }
}

/// Document Store Integration
pub(crate) struct DocumentStoreIntegration {
    http: HttpIntegration,
}

impl DocumentStoreIntegration {
    pub fn new(base_url: impl Into<String>, auth_token: impl Into<String>) -> Result<Self> {
        let http = HttpIntegration::new(base_url, 60)?.with_auth_token(auth_token);

        Ok(Self { http })
    }
}

#[async_trait]
impl ExternalIntegration for DocumentStoreIntegration {
    fn name(&self) -> &str {
        "document_store"
    }

    async fn execute(&self, operation: &DslOperation, context: &ExecutionContext) -> Result<Value> {
        match operation.operation_type.as_str() {
            "collect" => {
                if let Some(from) = operation.parameters.get("from").and_then(|v| v.as_str()) {
                    if from == "document-store" {
                        return self.collect_document(operation, context).await;
                    }
                }
            }
            "validate" => {
                if let Some(doc_id) = operation.parameters.get("document_id") {
                    return self.validate_document(doc_id, context).await;
                }
            }
            _ => {}
        }

        self.http.execute(operation, context).await
    }

    async fn validate(&self, operation: &DslOperation) -> Result<bool> {
        match operation.operation_type.as_str() {
            "collect" => {
                Ok(operation.parameters.get("from").and_then(|v| v.as_str())
                    == Some("document-store"))
            }
            "validate" => Ok(operation.parameters.contains_key("document_id")),
            _ => Ok(false),
        }
    }
}

impl DocumentStoreIntegration {
    async fn collect_document(
        &self,
        operation: &DslOperation,
        context: &ExecutionContext,
    ) -> Result<Value> {
        let document_type = operation
            .parameters
            .get("document_type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let query_params = vec![
            ("entity_id", context.business_unit_id.as_str()),
            ("document_type", document_type),
        ];

        let document_data = self
            .http
            .get_json("documents/search", Some(&query_params))
            .await?;

        Ok(serde_json::json!({
            "source": "document_store",
            "documents": document_data,
            "collected_at": chrono::Utc::now().to_rfc3339()
        }))
    }

    async fn validate_document(
        &self,
        document_id: &Value,
        _context: &ExecutionContext,
    ) -> Result<Value> {
        let doc_id_str = document_id.as_str().unwrap_or("");

        let validation_result = self
            .http
            .get_json(&format!("documents/{}/validate", doc_id_str), None)
            .await?;

        Ok(serde_json::json!({
            "source": "document_store",
            "document_id": doc_id_str,
            "validation": validation_result,
            "validated_at": chrono::Utc::now().to_rfc3339()
        }))
    }
}

/// CRS (Common Reporting Standard) Compliance Integration
pub(crate) struct CrsComplianceIntegration {
    http: HttpIntegration,
}

impl CrsComplianceIntegration {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Result<Self> {
        let http = HttpIntegration::new(base_url, 45)?.with_api_key(api_key);

        Ok(Self { http })
    }
}

#[async_trait]
impl ExternalIntegration for CrsComplianceIntegration {
    fn name(&self) -> &str {
        "crs_compliance"
    }

    async fn execute(&self, operation: &DslOperation, context: &ExecutionContext) -> Result<Value> {
        match operation.operation_type.as_str() {
            "check" => {
                if let Some(attr_id) = operation.parameters.get("attribute_id") {
                    if let Ok(attribute_id) = serde_json::from_value::<AttributeId>(attr_id.clone())
                    {
                        let attr_str = attribute_id.to_string();
                        if attr_str.contains("fatca") || attr_str.contains("crs") {
                            return self.check_compliance_status(operation, context).await;
                        }
                    }
                }
            }
            "collect" => {
                if let Some(from) = operation.parameters.get("from").and_then(|v| v.as_str()) {
                    if from == "crs-check" {
                        return self.collect_compliance_data(operation, context).await;
                    }
                }
            }
            _ => {}
        }

        self.http.execute(operation, context).await
    }

    async fn validate(&self, operation: &DslOperation) -> Result<bool> {
        match operation.operation_type.as_str() {
            "check" => Ok(operation
                .parameters
                .get("attribute_id")
                .and_then(|v| serde_json::from_value::<AttributeId>(v.clone()).ok())
                .is_some_and(|attr_id| {
                    let attr_str = attr_id.to_string();
                    attr_str.contains("fatca") || attr_str.contains("crs")
                })),
            "collect" => {
                Ok(operation.parameters.get("from").and_then(|v| v.as_str()) == Some("crs-check"))
            }
            _ => Ok(false),
        }
    }
}

impl CrsComplianceIntegration {
    async fn check_compliance_status(
        &self,
        operation: &DslOperation,
        context: &ExecutionContext,
    ) -> Result<Value> {
        let check_payload = serde_json::json!({
            "entity_id": context.business_unit_id,
            "attribute_id": operation.parameters.get("attribute_id"),
            "expected_value": operation.parameters.get("equals"),
            "check_type": "compliance_status"
        });

        let compliance_result = self
            .http
            .post_json("compliance/check", &check_payload)
            .await?;

        Ok(serde_json::json!({
            "source": "crs_compliance",
            "check_result": compliance_result,
            "checked_at": chrono::Utc::now().to_rfc3339()
        }))
    }

    async fn collect_compliance_data(
        &self,
        _operation: &DslOperation,
        context: &ExecutionContext,
    ) -> Result<Value> {
        let query_params = vec![
            ("entity_id", context.business_unit_id.as_str()),
            ("include_history", "true"),
        ];

        let compliance_data = self
            .http
            .get_json("compliance/status", Some(&query_params))
            .await?;

        Ok(serde_json::json!({
            "source": "crs_compliance",
            "compliance_data": compliance_data,
            "collected_at": chrono::Utc::now().to_rfc3339()
        }))
    }
}

/// Mock Integration for Testing and Development
pub(crate) struct MockIntegration {
    name: String,
    responses: HashMap<String, Value>,
}

impl MockIntegration {
    pub fn new(name: impl Into<String>) -> Self {
        let mut responses = HashMap::new();

        // Default mock responses
        responses.insert(
            "risk-engine".to_string(),
            serde_json::json!({
                "risk_score": 2.5,
                "risk_rating": "LOW",
                "factors": ["clean_sanctions", "good_credit_history"],
                "last_updated": chrono::Utc::now().to_rfc3339()
            }),
        );

        responses.insert(
            "document-store".to_string(),
            serde_json::json!({
                "documents": [{
                    "document_id": "doc-cert-001",
                    "document_type": "certificate_of_incorporation",
                    "status": "verified",
                    "upload_date": chrono::Utc::now().to_rfc3339()
                }]
            }),
        );

        responses.insert(
            "crs-check".to_string(),
            serde_json::json!({
                "crs_status": "REPORTABLE",
                "jurisdiction": "US",
                "tax_residence": ["US"],
                "fatca_status": "US_PERSON"
            }),
        );

        Self {
            name: name.into(),
            responses,
        }
    }

    pub(crate) fn with_response(mut self, key: impl Into<String>, response: Value) -> Self {
        self.responses.insert(key.into(), response);
        self
    }
}

#[async_trait]
impl ExternalIntegration for MockIntegration {
    fn name(&self) -> &str {
        &self.name
    }

    async fn execute(
        &self,
        operation: &DslOperation,
        _context: &ExecutionContext,
    ) -> Result<Value> {
        // For collect operations, use the 'from' parameter
        if operation.operation_type == "collect" {
            if let Some(from) = operation.parameters.get("from").and_then(|v| v.as_str()) {
                if let Some(response) = self.responses.get(from) {
                    return Ok(response.clone());
                }
            }
        }

        // For other operations, try to find a matching response
        if let Some(response) = self.responses.get(&operation.operation_type) {
            return Ok(response.clone());
        }

        // Default mock response
        Ok(serde_json::json!({
            "mock": true,
            "operation": operation.operation_type,
            "status": "success",
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    async fn validate(&self, _operation: &DslOperation) -> Result<bool> {
        // Mock integration accepts all operations
        Ok(true)
    }
}

/// Integration registry for managing multiple external integrations
pub(crate) struct IntegrationRegistry {
    integrations: HashMap<String, std::sync::Arc<dyn ExternalIntegration>>,
}

impl IntegrationRegistry {
    pub fn new() -> Self {
        Self {
            integrations: HashMap::new(),
        }
    }

    pub fn register(&mut self, integration: std::sync::Arc<dyn ExternalIntegration>) {
        self.integrations
            .insert(integration.name().to_string(), integration);
    }

    pub fn get(&self, name: &str) -> Option<&dyn ExternalIntegration> {
        self.integrations.get(name).map(|i| i.as_ref())
    }

    pub async fn execute_with_integration(
        &self,
        integration_name: &str,
        operation: &DslOperation,
        context: &ExecutionContext,
    ) -> Result<Value> {
        let integration = self
            .get(integration_name)
            .ok_or_else(|| anyhow::anyhow!("Integration not found: {}", integration_name))?;

        integration.execute(operation, context).await
    }

    pub(crate) fn list_integrations(&self) -> Vec<&str> {
        self.integrations.keys().map(|k| k.as_str()).collect()
    }
}

impl Default for IntegrationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Factory function to create standard integrations for development
pub(crate) fn create_standard_integrations() -> IntegrationRegistry {
    let mut registry = IntegrationRegistry::new();

    // Add mock integrations for development
    registry.register(std::sync::Arc::new(MockIntegration::new(
        "risk_engine_mock",
    )));
    registry.register(std::sync::Arc::new(MockIntegration::new(
        "document_store_mock",
    )));
    registry.register(std::sync::Arc::new(MockIntegration::new(
        "crs_compliance_mock",
    )));

    registry
}

/// Factory function to create production integrations (requires environment variables)
pub(crate) fn create_production_integrations() -> Result<IntegrationRegistry> {
    let mut registry = IntegrationRegistry::new();

    // Risk Engine Integration
    if let (Ok(risk_url), Ok(risk_key)) = (
        std::env::var("RISK_ENGINE_URL"),
        std::env::var("RISK_ENGINE_API_KEY"),
    ) {
        registry.register(std::sync::Arc::new(RiskEngineIntegration::new(
            risk_url, risk_key,
        )?));
    }

    // Document Store Integration
    if let (Ok(doc_url), Ok(doc_token)) = (
        std::env::var("DOCUMENT_STORE_URL"),
        std::env::var("DOCUMENT_STORE_AUTH_TOKEN"),
    ) {
        registry.register(std::sync::Arc::new(DocumentStoreIntegration::new(
            doc_url, doc_token,
        )?));
    }

    // CRS Compliance Integration
    if let (Ok(crs_url), Ok(crs_key)) = (
        std::env::var("CRS_COMPLIANCE_URL"),
        std::env::var("CRS_COMPLIANCE_API_KEY"),
    ) {
        registry.register(std::sync::Arc::new(CrsComplianceIntegration::new(
            crs_url, crs_key,
        )?));
    }

    Ok(registry)
}

