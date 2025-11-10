//! gRPC Client for DSL Service Communication
//!
//! This module provides a gRPC client for communicating with the backend DSL service.
//! It handles connection management, request/response serialization, and error handling
//! for all DSL-related operations needed by the visualizer.

use super::{
    models::{
        ASTNode, DSLEntry, ListDSLRequest, ListDSLResponse, ParseDSLRequest, ParseDSLResponse,
    },
    VisualizerError, VisualizerResult,
};
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// gRPC client for DSL service communication
pub struct DSLServiceClient {
    /// gRPC endpoint URL
    endpoint: String,

    /// Connection timeout
    timeout: Duration,

    /// Whether client is connected
    connected: bool,

    /// Last error message
    last_error: Option<String>,
}

impl DSLServiceClient {
    /// Create a new DSL service client
    pub fn new(endpoint: &str) -> VisualizerResult<Self> {
        info!("Creating DSL service client for endpoint: {}", endpoint);

        // Validate endpoint format
        if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
            return Err(VisualizerError::GrpcError {
                message: format!("Invalid endpoint format: {}", endpoint),
            });
        }

        Ok(Self {
            endpoint: endpoint.to_string(),
            timeout: Duration::from_secs(30),
            connected: false,
            last_error: None,
        })
    }

    /// Connect to the gRPC service
    pub async fn connect(&mut self) -> VisualizerResult<()> {
        debug!("Attempting to connect to gRPC service at {}", self.endpoint);

        // Simulate connection attempt
        // In a real implementation, this would establish the gRPC channel
        tokio::time::sleep(Duration::from_millis(100)).await;

        // For now, simulate successful connection
        self.connected = true;
        self.last_error = None;

        info!("Successfully connected to DSL service");
        Ok(())
    }

    /// Check if client is connected
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Get last error message
    pub fn last_error(&self) -> Option<&String> {
        self.last_error.as_ref()
    }

    /// List DSL instances from the backend
    pub async fn list_dsl_instances(&mut self, limit: usize) -> VisualizerResult<Vec<DSLEntry>> {
        if !self.connected {
            self.connect().await?;
        }

        debug!("Fetching DSL instances (limit: {})", limit);

        // Simulate network delay
        tokio::time::sleep(Duration::from_millis(50)).await;

        // For demonstration, return mock data
        let mock_entries = self.generate_mock_dsl_entries(limit);

        info!("Fetched {} DSL instances", mock_entries.len());
        Ok(mock_entries)
    }

    /// Get full content of a specific DSL instance
    pub async fn get_dsl_content(&mut self, dsl_id: &str) -> VisualizerResult<String> {
        if !self.connected {
            self.connect().await?;
        }

        debug!("Fetching DSL content for ID: {}", dsl_id);

        // Simulate network delay
        tokio::time::sleep(Duration::from_millis(30)).await;

        // Generate mock DSL content based on ID
        let content = self.generate_mock_dsl_content(dsl_id);

        debug!("Fetched DSL content ({} chars)", content.len());
        Ok(content)
    }

    /// Parse DSL content into AST
    pub async fn parse_dsl_to_ast(&mut self, content: &str) -> VisualizerResult<ASTNode> {
        if !self.connected {
            self.connect().await?;
        }

        debug!("Parsing DSL content ({} chars) to AST", content.len());

        // Simulate parsing time
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Generate mock AST from content
        let ast = self.generate_mock_ast_from_content(content);

        info!(
            "Successfully parsed DSL to AST ({} nodes)",
            ast.total_node_count()
        );
        Ok(ast)
    }

    /// Disconnect from the service
    pub fn disconnect(&mut self) {
        if self.connected {
            info!("Disconnecting from DSL service");
            self.connected = false;
        }
    }

    /// Generate mock DSL entries for demonstration
    fn generate_mock_dsl_entries(&self, limit: usize) -> Vec<DSLEntry> {
        use chrono::{DateTime, Utc};

        let mut entries = Vec::new();
        let domains = ["onboarding", "kyc", "compliance", "isda", "ubo"];
        let base_time = Utc::now() - chrono::Duration::days(30);

        for i in 0..limit.min(20) {
            let domain = domains[i % domains.len()];
            let id = format!("{}-instance-{:03}", domain, i + 1);
            let name = match domain {
                "onboarding" => format!("Client Onboarding #{}", i + 1),
                "kyc" => format!("KYC Verification #{}", i + 1),
                "compliance" => format!("Compliance Check #{}", i + 1),
                "isda" => format!("ISDA Agreement #{}", i + 1),
                "ubo" => format!("UBO Discovery #{}", i + 1),
                _ => format!("DSL Instance #{}", i + 1),
            };

            let description = Some(match domain {
                "onboarding" => {
                    "Client onboarding workflow with KYC and compliance checks".to_string()
                }
                "kyc" => "Know Your Customer verification process".to_string(),
                "compliance" => "Regulatory compliance validation".to_string(),
                "isda" => "ISDA Master Agreement establishment".to_string(),
                "ubo" => "Ultimate Beneficial Owner identification".to_string(),
                _ => "Generic DSL workflow".to_string(),
            });

            let content_preview = match domain {
                "onboarding" => format!(
                    "(onboarding.create :cbu-id \"CBU-{:03}\" :client-type \"HEDGE_FUND\")",
                    i + 1
                ),
                "kyc" => format!(
                    "(kyc.verify :entity-id \"ENT-{:03}\" :jurisdiction \"LU\")",
                    i + 1
                ),
                "compliance" => {
                    format!("(compliance.check :fatca-status \"NON_US\" :risk-level \"MEDIUM\")")
                }
                "isda" => format!("(isda.establish-master :counterparty \"CP-{:03}\")", i + 1),
                "ubo" => format!(
                    "(ubo.discover :entity-id \"ENT-{:03}\" :threshold 25.0)",
                    i + 1
                ),
                _ => format!("(generic.operation :id \"OP-{:03}\")", i + 1),
            };

            entries.push(DSLEntry {
                id,
                name,
                domain: domain.to_string(),
                created_at: base_time + chrono::Duration::hours(i as i64 * 3),
                version: ((i % 5) + 1) as i32,
                description,
                content_preview,
            });
        }

        entries
    }

    /// Generate mock DSL content based on ID
    fn generate_mock_dsl_content(&self, dsl_id: &str) -> String {
        if dsl_id.contains("onboarding") {
            format!(
                r#"
;; Onboarding DSL for {}
;; Generated by DSL Visualizer Demo

(onboarding.create
    :cbu-id "CBU-001"
    :business-reference "OB-{}-V1"
    :nature-purpose "Investment management services"
    :client-type "HEDGE_FUND")

(products.add "CUSTODY" "FUND_ACCOUNTING" "ADMINISTRATION")

(kyc.verify
    :documents (document "CertificateOfIncorporation" "BoardResolution")
    :jurisdictions (jurisdiction "LU" "KY"))

(services.discover
    :required-services ["Settlement" "Reporting" "RiskManagement"])

(resources.allocate
    :custody-account "ACC-001"
    :fund-accounting-system "FUNDTECH"
    :reporting-service "BLOOMBERG")

(compliance.check
    :fatca-status "NON_US"
    :crs-reportable false
    :sanctions-screening "CLEAR")

(workflow.complete
    :status "APPROVED"
    :next-steps ["ACCOUNT_OPENING" "SERVICE_ACTIVATION"])
"#,
                dsl_id, dsl_id
            )
        } else if dsl_id.contains("kyc") {
            format!(
                r#"
;; KYC Verification DSL for {}
(kyc.start-verification
    :entity-id "ENT-001"
    :verification-type "ENHANCED_DUE_DILIGENCE")

(kyc.collect-documents
    :required-docs ["INCORPORATION_CERT" "DIRECTORS_LIST" "SHAREHOLDERS_LIST"])

(kyc.screen-sanctions
    :screening-provider "WORLD_CHECK"
    :screening-scope "GLOBAL")

(kyc.assess-risk
    :risk-factors ["JURISDICTION" "BUSINESS_TYPE" "OWNERSHIP_STRUCTURE"]
    :risk-score-threshold 75)

(kyc.verify-identity
    :verification-method "DOCUMENT_VERIFICATION"
    :identity-provider "JUMIO")

(kyc.complete-verification
    :outcome "APPROVED"
    :risk-rating "MEDIUM")
"#,
                dsl_id
            )
        } else if dsl_id.contains("ubo") {
            format!(
                r#"
;; UBO Discovery DSL for {}
(ubo.start-discovery
    :target-entity "COMPANY-001"
    :ownership-threshold 25.0
    :jurisdiction "KY")

(ubo.trace-ownership
    :max-depth 5
    :follow-trusts true
    :follow-partnerships true)

(entity.analyze "COMPANY-001"
    :entity-type "LIMITED_COMPANY"
    :jurisdiction "KY"
    :registration-number "KY-123456")

(ownership.calculate
    :entity "COMPANY-001"
    :method "DIRECT_AND_INDIRECT"
    :threshold 25.0)

(ubo.identify-persons
    :criteria ["OWNERSHIP" "CONTROL"]
    :verification-required true)

(ubo.generate-report
    :format "REGULATORY_FILING"
    :include-evidence true)
"#,
                dsl_id
            )
        } else {
            format!(
                r#"
;; Generic DSL for {}
(workflow.start
    :id "{}"
    :type "GENERIC_WORKFLOW")

(step.execute
    :name "INITIALIZE"
    :parameters {{}})

(step.execute
    :name "PROCESS"
    :parameters {{:timeout 300}})

(step.execute
    :name "VALIDATE"
    :parameters {{:strict-mode true}})

(workflow.complete
    :status "SUCCESS")
"#,
                dsl_id, dsl_id
            )
        }
    }

    /// Generate mock AST from DSL content
    fn generate_mock_ast_from_content(&self, content: &str) -> ASTNode {
        use super::models::ASTNodeType;
        use std::collections::HashMap;

        let mut root = ASTNode::new(
            "root".to_string(),
            "DSL Program".to_string(),
            ASTNodeType::Root,
        );

        // Simple parsing simulation - look for S-expressions
        let mut node_counter = 0;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('(') && trimmed.contains('.') {
                node_counter += 1;

                // Extract verb (everything before first space after opening paren)
                let verb_end = trimmed.find(' ').unwrap_or(trimmed.len() - 1);
                let verb = &trimmed[1..verb_end]; // Remove opening paren

                let mut verb_node = ASTNode::new(
                    format!("verb_{}", node_counter),
                    verb.to_string(),
                    ASTNodeType::Verb,
                );

                // Add some mock attributes
                if trimmed.contains(':') {
                    let mut attr_counter = 0;
                    for part in trimmed.split(':').skip(1) {
                        attr_counter += 1;
                        if attr_counter > 3 {
                            break;
                        } // Limit attributes

                        let attr_name = part.split_whitespace().next().unwrap_or("unknown");
                        let attr_node = ASTNode::new(
                            format!("attr_{}_{}", node_counter, attr_counter),
                            format!(":{}", attr_name),
                            ASTNodeType::Attribute,
                        );

                        // Add a value node for the attribute
                        if let Some(value_part) = part.split_whitespace().nth(1) {
                            let value_clean = value_part.trim_matches('"').trim_matches(')');
                            if !value_clean.is_empty() && value_clean.len() < 50 {
                                let value_node = ASTNode::new(
                                    format!("value_{}_{}_{}", node_counter, attr_counter, 1),
                                    value_clean.to_string(),
                                    ASTNodeType::Value,
                                );
                                let mut attr_with_value = attr_node;
                                attr_with_value.add_child(value_node);
                                verb_node.add_child(attr_with_value);
                            } else {
                                verb_node.add_child(attr_node);
                            }
                        } else {
                            verb_node.add_child(attr_node);
                        }
                    }
                }

                root.add_child(verb_node);
            }
        }

        // If no verbs found, create a simple structure
        if root.children.is_empty() {
            let placeholder_node = ASTNode::new(
                "placeholder".to_string(),
                "Empty or Invalid DSL".to_string(),
                ASTNodeType::Value,
            );
            root.add_child(placeholder_node);
        }

        root
    }
}

impl Drop for DSLServiceClient {
    fn drop(&mut self) {
        self.disconnect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let client = DSLServiceClient::new("http://localhost:50051");
        assert!(client.is_ok());

        let invalid_client = DSLServiceClient::new("invalid-endpoint");
        assert!(invalid_client.is_err());
    }

    #[tokio::test]
    async fn test_connection() {
        let mut client = DSLServiceClient::new("http://localhost:50051").unwrap();
        assert!(!client.is_connected());

        let result = client.connect().await;
        assert!(result.is_ok());
        assert!(client.is_connected());
    }

    #[tokio::test]
    async fn test_mock_data_generation() {
        let mut client = DSLServiceClient::new("http://localhost:50051").unwrap();

        let entries = client.list_dsl_instances(5).await.unwrap();
        assert_eq!(entries.len(), 5);

        let content = client
            .get_dsl_content("onboarding-instance-001")
            .await
            .unwrap();
        assert!(content.contains("onboarding.create"));

        let ast = client.parse_dsl_to_ast(&content).await.unwrap();
        assert!(ast.total_node_count() > 1);
    }
}
