//! DSL Retrieval Service Implementation
//!
//! This service implements the gRPC interface for all DSL and AST retrieval operations.
//! It serves as a read-only interface for the Web UI and other clients that need to
//! visualize or query DSL instances and their Abstract Syntax Trees.

use crate::database::dsl_instance_repository::{
    AstNode, AstNodeType, CompilationStatus, DslInstance, DslInstanceRepository,
    DslInstanceVersion, DslTemplate, InstanceStatus, OperationType, PgDslInstanceRepository,
};
use crate::dsl_manager::{DslManager, VisualizationOptions};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;
use tonic::{Request, Response, Status};
use uuid::Uuid;

// Generated proto types - when you run tonic-build, these will be available
use crate::proto::dsl_retrieval::{
    dsl_retrieval_service_server::{DslRetrievalService, DslRetrievalServiceServer},
    AstNode as ProtoAstNode, AstNodeDetailsResponse, AstTreeResponse, AstVisualizationResponse,
    BusinessContextData, CompilationStatus as ProtoCompilationStatus, CriticalPath,
    DictionaryReference, DomainVisualizationOptions, DomainVisualizationResponse,
    DslByDomainKeyResponse, DslInstanceResponse, DslTemplateResponse, DslVersionResponse,
    FindAstNodesByPathRequest, FindAstNodesByTypeRequest, FindAstNodesResponse,
    GenerateAstVisualizationRequest, GenerateDomainVisualizationRequest, GetAstNodeDetailsRequest,
    GetAstTreeRequest, GetDslByDomainKeyRequest, GetDslInstanceRequest, GetDslTemplateRequest,
    GetDslVersionRequest, GetInstanceByReferenceRequest, GetInstancesByBusinessReferenceRequest,
    GetLatestVersionRequest, InstanceStatus as ProtoInstanceStatus, ListDslInstancesRequest,
    ListDslInstancesResponse, ListDslTemplatesRequest, ListDslTemplatesResponse,
    ListVersionsRequest, ListVersionsResponse, NodeType as ProtoNodeType,
    OperationType as ProtoOperationType, VisualEdge, VisualNode, VisualizationMetadata,
    VisualizationOptions as ProtoVisualizationOptions, VisualizationStatistics,
    WorkflowProgression,
};

pub struct DslRetrievalServiceImpl {
    dsl_manager: Arc<DslManager>,
    instance_repository: Arc<PgDslInstanceRepository>,
}

impl DslRetrievalServiceImpl {
    pub fn new(
        dsl_manager: Arc<DslManager>,
        instance_repository: Arc<PgDslInstanceRepository>,
    ) -> Self {
        Self {
            dsl_manager,
            instance_repository,
        }
    }

    // Helper function to convert between internal and proto status enums
    fn to_proto_instance_status(&self, status: InstanceStatus) -> i32 {
        match status {
            InstanceStatus::Created => ProtoInstanceStatus::InstanceStatusCreated as i32,
            InstanceStatus::Editing => ProtoInstanceStatus::InstanceStatusEditing as i32,
            InstanceStatus::Compiled => ProtoInstanceStatus::InstanceStatusCompiled as i32,
            InstanceStatus::Finalized => ProtoInstanceStatus::InstanceStatusFinalized as i32,
            InstanceStatus::Archived => ProtoInstanceStatus::InstanceStatusArchived as i32,
            InstanceStatus::Failed => ProtoInstanceStatus::InstanceStatusFailed as i32,
        }
    }

    fn to_proto_operation_type(&self, op_type: OperationType) -> i32 {
        match op_type {
            OperationType::CreateFromTemplate => {
                ProtoOperationType::OperationTypeCreateFromTemplate as i32
            }
            OperationType::IncrementalEdit => {
                ProtoOperationType::OperationTypeIncrementalEdit as i32
            }
            OperationType::TemplateAddition => {
                ProtoOperationType::OperationTypeTemplateAddition as i32
            }
            OperationType::ManualEdit => ProtoOperationType::OperationTypeManualEdit as i32,
            OperationType::Recompilation => ProtoOperationType::OperationTypeRecompilation as i32,
        }
    }

    fn to_proto_compilation_status(&self, status: CompilationStatus) -> i32 {
        match status {
            CompilationStatus::Pending => ProtoCompilationStatus::CompilationStatusPending as i32,
            CompilationStatus::Success => ProtoCompilationStatus::CompilationStatusSuccess as i32,
            CompilationStatus::Error => ProtoCompilationStatus::CompilationStatusError as i32,
        }
    }

    fn to_proto_node_type(&self, node_type: AstNodeType) -> i32 {
        match node_type {
            AstNodeType::Verb => ProtoNodeType::NodeTypeVerb as i32,
            AstNodeType::Attribute => ProtoNodeType::NodeTypeAttribute as i32,
            AstNodeType::List => ProtoNodeType::NodeTypeList as i32,
            AstNodeType::Map => ProtoNodeType::NodeTypeMap as i32,
            AstNodeType::Value => ProtoNodeType::NodeTypeValue as i32,
            AstNodeType::Root => ProtoNodeType::NodeTypeRoot as i32,
            AstNodeType::Comment => ProtoNodeType::NodeTypeComment as i32,
            AstNodeType::Placeholder => ProtoNodeType::NodeTypePlaceholder as i32,
            AstNodeType::Reference => ProtoNodeType::NodeTypeReference as i32,
            AstNodeType::Special => ProtoNodeType::NodeTypeSpecial as i32,
        }
    }

    fn from_proto_node_type(&self, node_type: i32) -> Result<AstNodeType, Status> {
        match node_type {
            1 => Ok(AstNodeType::Verb),
            2 => Ok(AstNodeType::Attribute),
            3 => Ok(AstNodeType::List),
            4 => Ok(AstNodeType::Map),
            5 => Ok(AstNodeType::Value),
            6 => Ok(AstNodeType::Root),
            7 => Ok(AstNodeType::Comment),
            8 => Ok(AstNodeType::Placeholder),
            9 => Ok(AstNodeType::Reference),
            10 => Ok(AstNodeType::Special),
            _ => Err(Status::invalid_argument(format!(
                "Invalid node type: {}",
                node_type
            ))),
        }
    }

    // Helper to convert internal DslInstance to proto response
    fn to_proto_instance(&self, instance: DslInstance) -> DslInstanceResponse {
        let mut response = DslInstanceResponse {
            instance_id: instance.instance_id.to_string(),
            domain_name: instance.domain_name,
            business_reference: instance.business_reference,
            current_version: instance.current_version,
            status: self.to_proto_instance_status(instance.status),
            created_at: Some(prost_types::Timestamp {
                seconds: instance.created_at.timestamp(),
                nanos: instance.created_at.timestamp_subsec_nanos() as i32,
            }),
            updated_at: Some(prost_types::Timestamp {
                seconds: instance.updated_at.timestamp(),
                nanos: instance.updated_at.timestamp_subsec_nanos() as i32,
            }),
            metadata: None,
        };

        if let Some(metadata) = instance.metadata {
            if let Ok(struct_value) = serde_json::to_value(metadata) {
                if let Ok(proto_struct) = struct_value.try_into() {
                    response.metadata = Some(proto_struct);
                }
            }
        }

        response
    }

    // Helper to convert internal DslInstanceVersion to proto response
    fn to_proto_version(&self, version: DslInstanceVersion) -> DslVersionResponse {
        let mut response = DslVersionResponse {
            version_id: version.version_id.to_string(),
            instance_id: version.instance_id.to_string(),
            version_number: version.version_number,
            dsl_content: version.dsl_content,
            operation_type: self.to_proto_operation_type(version.operation_type),
            compilation_status: self.to_proto_compilation_status(version.compilation_status),
            created_at: Some(prost_types::Timestamp {
                seconds: version.created_at.timestamp(),
                nanos: version.created_at.timestamp_subsec_nanos() as i32,
            }),
            created_by: version.created_by.unwrap_or_default(),
            change_description: version.change_description.unwrap_or_default(),
            ast_json: None,
        };

        if let Some(ast) = version.ast_json {
            if let Ok(struct_value) = serde_json::to_value(ast) {
                if let Ok(proto_struct) = struct_value.try_into() {
                    response.ast_json = Some(proto_struct);
                }
            }
        }

        response
    }

    // Helper to convert internal DslTemplate to proto response
    fn to_proto_template(&self, template: DslTemplate) -> DslTemplateResponse {
        let mut response = DslTemplateResponse {
            template_id: template.template_id.to_string(),
            template_name: template.template_name,
            domain_name: template.domain_name,
            template_type: template.template_type,
            content: template.content,
            created_at: Some(prost_types::Timestamp {
                seconds: template.created_at.timestamp(),
                nanos: template.created_at.timestamp_subsec_nanos() as i32,
            }),
            updated_at: Some(prost_types::Timestamp {
                seconds: template.updated_at.timestamp(),
                nanos: template.updated_at.timestamp_subsec_nanos() as i32,
            }),
            variables: None,
            requirements: None,
            metadata: None,
        };

        if let Some(variables) = template.variables {
            if let Ok(struct_value) = serde_json::to_value(variables) {
                if let Ok(proto_struct) = struct_value.try_into() {
                    response.variables = Some(proto_struct);
                }
            }
        }

        if let Some(requirements) = template.requirements {
            if let Ok(struct_value) = serde_json::to_value(requirements) {
                if let Ok(proto_struct) = struct_value.try_into() {
                    response.requirements = Some(proto_struct);
                }
            }
        }

        if let Some(metadata) = template.metadata {
            if let Ok(struct_value) = serde_json::to_value(metadata) {
                if let Ok(proto_struct) = struct_value.try_into() {
                    response.metadata = Some(proto_struct);
                }
            }
        }

        response
    }

    // Helper to convert internal AstNode to proto response
    fn to_proto_ast_node(&self, node: AstNode, children: Vec<AstNode>) -> ProtoAstNode {
        let mut proto_node = ProtoAstNode {
            node_id: node.node_id.to_string(),
            parent_node_id: node
                .parent_node_id
                .map(|id| id.to_string())
                .unwrap_or_default(),
            node_type: self.to_proto_node_type(node.node_type),
            node_key: node.node_key.unwrap_or_default(),
            node_value: node.node_value.map(|v| prost_types::Value::from(v)),
            position_index: node.position_index.unwrap_or(0),
            depth: node.depth,
            path: node.path,
            children: Vec::new(),
        };

        // Convert children
        for child in children {
            // For this conversion, we'll assume children don't have nested children
            // In a full implementation, you'd need recursive conversion
            let child_proto = ProtoAstNode {
                node_id: child.node_id.to_string(),
                parent_node_id: child
                    .parent_node_id
                    .map(|id| id.to_string())
                    .unwrap_or_default(),
                node_type: self.to_proto_node_type(child.node_type),
                node_key: child.node_key.unwrap_or_default(),
                node_value: child.node_value.map(|v| prost_types::Value::from(v)),
                position_index: child.position_index.unwrap_or(0),
                depth: child.depth,
                path: child.path,
                children: Vec::new(),
            };
            proto_node.children.push(child_proto);
        }

        proto_node
    }
}

#[tonic::async_trait]
impl DslRetrievalService for DslRetrievalServiceImpl {
    // Instance retrieval
    async fn get_dsl_instance(
        &self,
        request: Request<GetDslInstanceRequest>,
    ) -> Result<Response<DslInstanceResponse>, Status> {
        let req = request.into_inner();

        let instance_id = Uuid::parse_str(&req.instance_id)
            .map_err(|_| Status::invalid_argument("Invalid instance ID format"))?;

        let instance = self
            .instance_repository
            .get_instance(instance_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to get DSL instance: {}", e)))?
            .ok_or_else(|| Status::not_found("DSL instance not found"))?;

        Ok(Response::new(self.to_proto_instance(instance)))
    }

    async fn get_instance_by_reference(
        &self,
        request: Request<GetInstanceByReferenceRequest>,
    ) -> Result<Response<DslInstanceResponse>, Status> {
        let req = request.into_inner();

        let instance = self
            .instance_repository
            .get_instance_by_reference(&req.domain_name, &req.business_reference)
            .await
            .map_err(|e| {
                Status::internal(format!("Failed to get DSL instance by reference: {}", e))
            })?
            .ok_or_else(|| Status::not_found("DSL instance not found"))?;

        Ok(Response::new(self.to_proto_instance(instance)))
    }

    async fn list_dsl_instances(
        &self,
        request: Request<ListDslInstancesRequest>,
    ) -> Result<Response<ListDslInstancesResponse>, Status> {
        let req = request.into_inner();

        let domain_name = if req.domain_name.is_empty() {
            None
        } else {
            Some(req.domain_name.as_str())
        };

        let limit = if req.limit > 0 {
            Some(req.limit as i64)
        } else {
            None
        };
        let offset = if req.offset > 0 {
            Some(req.offset as i64)
        } else {
            None
        };

        let instances = self
            .instance_repository
            .list_instances(domain_name, limit, offset)
            .await
            .map_err(|e| Status::internal(format!("Failed to list DSL instances: {}", e)))?;

        let proto_instances: Vec<DslInstanceResponse> = instances
            .into_iter()
            .map(|instance| self.to_proto_instance(instance))
            .collect();

        // For this simple implementation, we'll return the count as the size of the current page
        // In a full implementation, you'd do a separate count query
        let total_count = proto_instances.len() as i32;

        Ok(Response::new(ListDslInstancesResponse {
            instances: proto_instances,
            total_count,
        }))
    }

    // Version retrieval
    async fn get_dsl_version(
        &self,
        request: Request<GetDslVersionRequest>,
    ) -> Result<Response<DslVersionResponse>, Status> {
        let req = request.into_inner();

        let version_id = Uuid::parse_str(&req.version_id)
            .map_err(|_| Status::invalid_argument("Invalid version ID format"))?;

        let version = self
            .instance_repository
            .get_version(version_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to get DSL version: {}", e)))?
            .ok_or_else(|| Status::not_found("DSL version not found"))?;

        Ok(Response::new(self.to_proto_version(version)))
    }

    async fn get_latest_version(
        &self,
        request: Request<GetLatestVersionRequest>,
    ) -> Result<Response<DslVersionResponse>, Status> {
        let req = request.into_inner();

        let instance_id = Uuid::parse_str(&req.instance_id)
            .map_err(|_| Status::invalid_argument("Invalid instance ID format"))?;

        let version = self
            .instance_repository
            .get_latest_version(instance_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to get latest version: {}", e)))?
            .ok_or_else(|| Status::not_found("No versions found for this instance"))?;

        Ok(Response::new(self.to_proto_version(version)))
    }

    async fn list_versions(
        &self,
        request: Request<ListVersionsRequest>,
    ) -> Result<Response<ListVersionsResponse>, Status> {
        let req = request.into_inner();

        let instance_id = Uuid::parse_str(&req.instance_id)
            .map_err(|_| Status::invalid_argument("Invalid instance ID format"))?;

        let limit = if req.limit > 0 {
            Some(req.limit as i64)
        } else {
            None
        };
        let offset = if req.offset > 0 {
            Some(req.offset as i64)
        } else {
            None
        };

        let versions = self
            .instance_repository
            .list_versions(instance_id, limit, offset)
            .await
            .map_err(|e| Status::internal(format!("Failed to list versions: {}", e)))?;

        let proto_versions: Vec<DslVersionResponse> = versions
            .into_iter()
            .map(|version| self.to_proto_version(version))
            .collect();

        let total_count = proto_versions.len() as i32;

        Ok(Response::new(ListVersionsResponse {
            versions: proto_versions,
            total_count,
        }))
    }

    // AST retrieval
    async fn get_ast_tree(
        &self,
        request: Request<GetAstTreeRequest>,
    ) -> Result<Response<AstTreeResponse>, Status> {
        let req = request.into_inner();

        let version_id = Uuid::parse_str(&req.version_id)
            .map_err(|_| Status::invalid_argument("Invalid version ID format"))?;

        let nodes = self
            .instance_repository
            .get_ast_nodes_by_version(version_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to get AST nodes: {}", e)))?;

        if nodes.is_empty() {
            return Err(Status::not_found("No AST nodes found for this version"));
        }

        // Build a hierarchical tree structure
        let mut node_map = HashMap::new();
        let mut max_depth = 0;

        for node in &nodes {
            node_map.insert(node.node_id, node.clone());
            if node.depth > max_depth {
                max_depth = node.depth;
            }
        }

        // Find the root node (should be the first node or have no parent)
        let root_node = nodes
            .iter()
            .find(|node| node.parent_node_id.is_none())
            .cloned()
            .unwrap_or_else(|| nodes[0].clone());

        // Build the children for the root (simplified - in practice you'd build the full tree)
        let children = nodes
            .iter()
            .filter(|node| node.parent_node_id == Some(root_node.node_id))
            .cloned()
            .collect();

        let proto_root = self.to_proto_ast_node(root_node, children);

        Ok(Response::new(AstTreeResponse {
            version_id: version_id.to_string(),
            root_node: Some(proto_root),
            node_count: nodes.len() as i32,
            max_depth,
            created_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
        }))
    }

    async fn get_ast_node_details(
        &self,
        request: Request<GetAstNodeDetailsRequest>,
    ) -> Result<Response<AstNodeDetailsResponse>, Status> {
        let req = request.into_inner();

        let node_id = Uuid::parse_str(&req.node_id)
            .map_err(|_| Status::invalid_argument("Invalid node ID format"))?;

        let node = self
            .instance_repository
            .get_ast_node(node_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to get AST node: {}", e)))?
            .ok_or_else(|| Status::not_found("AST node not found"))?;

        // Get all nodes from the same version to build relationships
        let all_nodes = self
            .instance_repository
            .get_ast_nodes_by_version(node.version_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to get related nodes: {}", e)))?;

        // Find children
        let children: Vec<AstNode> = all_nodes
            .iter()
            .filter(|n| n.parent_node_id == Some(node.node_id))
            .cloned()
            .collect();

        // Find parent
        let parent = if let Some(parent_id) = node.parent_node_id {
            all_nodes.iter().find(|n| n.node_id == parent_id).cloned()
        } else {
            None
        };

        // Find siblings (nodes with the same parent)
        let siblings: Vec<AstNode> = if let Some(parent_id) = node.parent_node_id {
            all_nodes
                .iter()
                .filter(|n| n.parent_node_id == Some(parent_id) && n.node_id != node.node_id)
                .cloned()
                .collect()
        } else {
            Vec::new()
        };

        let proto_node = self.to_proto_ast_node(node, Vec::new());
        let proto_children: Vec<ProtoAstNode> = children
            .into_iter()
            .map(|child| self.to_proto_ast_node(child, Vec::new()))
            .collect();
        let proto_parent = parent.map(|p| self.to_proto_ast_node(p, Vec::new()));
        let proto_siblings: Vec<ProtoAstNode> = siblings
            .into_iter()
            .map(|sibling| self.to_proto_ast_node(sibling, Vec::new()))
            .collect();

        Ok(Response::new(AstNodeDetailsResponse {
            node: Some(proto_node),
            children: proto_children,
            parent: proto_parent,
            siblings: proto_siblings,
            related_business_concepts: Vec::new(), // Would be populated based on dictionary lookup
            dictionary_ref: None,                  // Would be populated based on attribute lookup
        }))
    }

    async fn find_ast_nodes_by_type(
        &self,
        request: Request<FindAstNodesByTypeRequest>,
    ) -> Result<Response<FindAstNodesResponse>, Status> {
        let req = request.into_inner();

        let version_id = Uuid::parse_str(&req.version_id)
            .map_err(|_| Status::invalid_argument("Invalid version ID format"))?;

        let node_type = self.from_proto_node_type(req.node_type)?;

        let nodes = self
            .instance_repository
            .get_ast_nodes_by_type(version_id, node_type)
            .await
            .map_err(|e| Status::internal(format!("Failed to find AST nodes by type: {}", e)))?;

        let proto_nodes: Vec<ProtoAstNode> = nodes
            .into_iter()
            .map(|node| self.to_proto_ast_node(node, Vec::new()))
            .collect();

        let total_count = proto_nodes.len() as i32;

        Ok(Response::new(FindAstNodesResponse {
            nodes: proto_nodes,
            total_count,
        }))
    }

    async fn find_ast_nodes_by_path(
        &self,
        request: Request<FindAstNodesByPathRequest>,
    ) -> Result<Response<FindAstNodesResponse>, Status> {
        let req = request.into_inner();

        let version_id = Uuid::parse_str(&req.version_id)
            .map_err(|_| Status::invalid_argument("Invalid version ID format"))?;

        let nodes = self
            .instance_repository
            .get_ast_nodes_by_path(version_id, &req.path_pattern)
            .await
            .map_err(|e| Status::internal(format!("Failed to find AST nodes by path: {}", e)))?;

        let proto_nodes: Vec<ProtoAstNode> = nodes
            .into_iter()
            .map(|node| self.to_proto_ast_node(node, Vec::new()))
            .collect();

        let total_count = proto_nodes.len() as i32;

        Ok(Response::new(FindAstNodesResponse {
            nodes: proto_nodes,
            total_count,
        }))
    }

    // Template retrieval
    async fn get_dsl_template(
        &self,
        request: Request<GetDslTemplateRequest>,
    ) -> Result<Response<DslTemplateResponse>, Status> {
        let req = request.into_inner();

        let template = match req.identifier {
            Some(identifier) => match identifier {
                crate::proto::dsl_retrieval::get_dsl_template_request::Identifier::TemplateId(
                    id,
                ) => {
                    let template_id = Uuid::parse_str(&id)
                        .map_err(|_| Status::invalid_argument("Invalid template ID format"))?;
                    self.instance_repository
                        .get_template(template_id)
                        .await
                        .map_err(|e| {
                            Status::internal(format!("Failed to get template by ID: {}", e))
                        })?
                }
                crate::proto::dsl_retrieval::get_dsl_template_request::Identifier::TemplateName(
                    name,
                ) => self
                    .instance_repository
                    .get_template_by_name(&name)
                    .await
                    .map_err(|e| {
                        Status::internal(format!("Failed to get template by name: {}", e))
                    })?,
            },
            None => return Err(Status::invalid_argument("Template identifier is required")),
        };

        let template = template.ok_or_else(|| Status::not_found("Template not found"))?;

        Ok(Response::new(self.to_proto_template(template)))
    }

    async fn list_dsl_templates(
        &self,
        request: Request<ListDslTemplatesRequest>,
    ) -> Result<Response<ListDslTemplatesResponse>, Status> {
        let req = request.into_inner();

        let domain_name = if req.domain_name.is_empty() {
            None
        } else {
            Some(req.domain_name.as_str())
        };

        let template_type = if req.template_type.is_empty() {
            None
        } else {
            Some(req.template_type.as_str())
        };

        let templates = self
            .instance_repository
            .list_templates(domain_name, template_type)
            .await
            .map_err(|e| Status::internal(format!("Failed to list templates: {}", e)))?;

        let proto_templates: Vec<DslTemplateResponse> = templates
            .into_iter()
            .map(|template| self.to_proto_template(template))
            .collect();

        Ok(Response::new(ListDslTemplatesResponse {
            templates: proto_templates,
        }))
    }

    // Business reference retrieval
    async fn get_instances_by_business_reference(
        &self,
        request: Request<GetInstancesByBusinessReferenceRequest>,
    ) -> Result<Response<ListDslInstancesResponse>, Status> {
        let req = request.into_inner();

        let instances = self
            .instance_repository
            .get_instances_by_reference(&req.reference_type, &req.reference_id_value)
            .await
            .map_err(|e| {
                Status::internal(format!("Failed to get instances by reference: {}", e))
            })?;

        let proto_instances: Vec<DslInstanceResponse> = instances
            .into_iter()
            .map(|instance| self.to_proto_instance(instance))
            .collect();

        let total_count = proto_instances.len() as i32;

        Ok(Response::new(ListDslInstancesResponse {
            instances: proto_instances,
            total_count,
        }))
    }

    // DSL domain retrieval
    async fn get_dsl_by_domain_key(
        &self,
        request: Request<GetDslByDomainKeyRequest>,
    ) -> Result<Response<DslByDomainKeyResponse>, Status> {
        let req = request.into_inner();

        // This is a high-level operation that would typically involve:
        // 1. Finding instances by domain and key
        // 2. Getting the latest version
        // 3. Optionally getting version history
        // 4. Enriching with business context

        // For now, we'll implement a simplified version
        let instances = match req.key_type.as_str() {
            "cbu_id" => self
                .instance_repository
                .get_instances_by_reference("CBU", &req.key_value)
                .await
                .map_err(|e| Status::internal(format!("Failed to get instances by CBU: {}", e)))?,
            _ => {
                // For other key types, search by business reference
                let domain_instances = self
                    .instance_repository
                    .list_instances(Some(&req.domain), None, None)
                    .await
                    .map_err(|e| {
                        Status::internal(format!("Failed to list domain instances: {}", e))
                    })?;

                domain_instances
                    .into_iter()
                    .filter(|i| i.business_reference.contains(&req.key_value))
                    .collect()
            }
        };

        let proto_instances: Vec<DslInstanceResponse> = instances
            .iter()
            .map(|instance| self.to_proto_instance(instance.clone()))
            .collect();

        // Get the latest version from the first instance (if any)
        let latest_version = if let Some(first_instance) = instances.first() {
            self.instance_repository
                .get_latest_version(first_instance.instance_id)
                .await
                .map_err(|e| Status::internal(format!("Failed to get latest version: {}", e)))?
                .map(|v| self.to_proto_version(v))
        } else {
            None
        };

        // Get version history if requested
        let version_history = if req.include_history && instances.first().is_some() {
            let versions = self
                .instance_repository
                .list_versions(instances[0].instance_id, None, None)
                .await
                .map_err(|e| Status::internal(format!("Failed to get version history: {}", e)))?;

            versions
                .into_iter()
                .map(|v| self.to_proto_version(v))
                .collect()
        } else {
            Vec::new()
        };

        // Build business context (simplified)
        let business_context = BusinessContextData {
            entity_type: "Unknown".to_string(),
            entity_name: req.key_value.clone(),
            jurisdiction: "Unknown".to_string(),
            products: Vec::new(),
            services: Vec::new(),
            additional_data: None,
        };

        Ok(Response::new(DslByDomainKeyResponse {
            instances: proto_instances,
            latest_version,
            version_history,
            business_context: Some(business_context),
        }))
    }

    // Visualization
    async fn generate_ast_visualization(
        &self,
        request: Request<GenerateAstVisualizationRequest>,
    ) -> Result<Response<AstVisualizationResponse>, Status> {
        let req = request.into_inner();

        let version_id = Uuid::parse_str(&req.version_id)
            .map_err(|_| Status::invalid_argument("Invalid version ID format"))?;

        // Get the version to ensure it exists and has AST data
        let version = self
            .instance_repository
            .get_version(version_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to get version: {}", e)))?
            .ok_or_else(|| Status::not_found("Version not found"))?;

        // Use the dsl_manager to generate visualization
        let viz_options = VisualizationOptions::default(); // Convert proto options if needed

        let visualization = self
            .dsl_manager
            .generate_ast_visualization(version.instance_id, Some(viz_options))
            .await
            .map_err(|e| Status::internal(format!("Failed to generate visualization: {}", e)))?;

        // Convert internal visualization to proto response
        let metadata = VisualizationMetadata {
            generated_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
            parser_version: "1.0.0".to_string(),
            grammar_version: "1.0.0".to_string(),
            node_count: visualization.nodes.len() as i32,
            edge_count: visualization.edges.len() as i32,
            instance_id: version.instance_id.to_string(),
            version_id: version.version_id.to_string(),
        };

        let statistics = VisualizationStatistics {
            total_nodes: visualization.nodes.len() as i32,
            total_edges: visualization.edges.len() as i32,
            max_depth: 10, // Would be calculated from actual data
            complexity_score: 1.0,
            compilation_time_ms: 0,
            visualization_time_ms: 0,
        };

        // Convert nodes and edges (simplified conversion)
        let proto_nodes: Vec<VisualNode> = visualization
            .nodes
            .into_iter()
            .map(|node| VisualNode {
                id: node.id,
                label: node.label,
                node_type: node.node_type,
                properties: node.properties.map(|p| p.try_into().ok()).flatten(),
                position: node
                    .position
                    .map(|p| crate::proto::dsl_retrieval::NodePosition {
                        x: p.x,
                        y: p.y,
                        z: p.z.unwrap_or(0.0),
                    }),
                styling: node
                    .styling
                    .map(|s| crate::proto::dsl_retrieval::NodeStyling {
                        color: s.color,
                        border_color: s.border_color,
                        border_width: s.border_width,
                        shape: s.shape,
                        size: s.size,
                    }),
                domain_annotations: node.domain_annotations.map(|a| a.try_into().ok()).flatten(),
                priority_level: node.priority_level,
                functional_relevance: node.functional_relevance,
            })
            .collect();

        let proto_edges: Vec<VisualEdge> = visualization
            .edges
            .into_iter()
            .map(|edge| VisualEdge {
                id: edge.id,
                from: edge.from,
                to: edge.to,
                edge_type: edge.edge_type,
                label: edge.label,
                styling: edge
                    .styling
                    .map(|s| crate::proto::dsl_retrieval::EdgeStyling {
                        color: s.color,
                        width: s.width,
                        style: s.style,
                        arrow_type: s.arrow_type,
                    }),
                weight: edge.weight,
            })
            .collect();

        Ok(Response::new(AstVisualizationResponse {
            metadata: Some(metadata),
            root_node: proto_nodes.first().cloned(),
            nodes: proto_nodes,
            edges: proto_edges,
            statistics: Some(statistics),
        }))
    }

    async fn generate_domain_visualization(
        &self,
        request: Request<GenerateDomainVisualizationRequest>,
    ) -> Result<Response<DomainVisualizationResponse>, Status> {
        let req = request.into_inner();

        let instance_id = Uuid::parse_str(&req.instance_id)
            .map_err(|_| Status::invalid_argument("Invalid instance ID format"))?;

        // Get the instance to ensure it exists
        let instance = self
            .instance_repository
            .get_instance(instance_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to get instance: {}", e)))?
            .ok_or_else(|| Status::not_found("Instance not found"))?;

        // Use the dsl_manager to generate domain-enhanced visualization
        let viz_options = VisualizationOptions::default(); // Convert proto options if needed

        let domain_visualization = self
            .dsl_manager
            .generate_domain_enhanced_visualization(instance_id, Some(viz_options))
            .await
            .map_err(|e| {
                Status::internal(format!("Failed to generate domain visualization: {}", e))
            })?;

        // Convert to base visualization response first
        let base_viz = domain_visualization.base_visualization;

        let metadata = VisualizationMetadata {
            generated_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
            parser_version: "1.0.0".to_string(),
            grammar_version: "1.0.0".to_string(),
            node_count: base_viz.nodes.len() as i32,
            edge_count: base_viz.edges.len() as i32,
            instance_id: instance.instance_id.to_string(),
            version_id: "".to_string(),
        };

        let statistics = VisualizationStatistics {
            total_nodes: base_viz.nodes.len() as i32,
            total_edges: base_viz.edges.len() as i32,
            max_depth: 10,
            complexity_score: 1.0,
            compilation_time_ms: 0,
            visualization_time_ms: 0,
        };

        // Convert nodes and edges (simplified)
        let proto_nodes: Vec<VisualNode> = base_viz
            .nodes
            .into_iter()
            .map(|node| VisualNode {
                id: node.id,
                label: node.label,
                node_type: node.node_type,
                properties: None,
                position: None,
                styling: None,
                domain_annotations: None,
                priority_level: node.priority_level,
                functional_relevance: node.functional_relevance,
            })
            .collect();

        let proto_edges: Vec<VisualEdge> = base_viz
            .edges
            .into_iter()
            .map(|edge| VisualEdge {
                id: edge.id,
                from: edge.from,
                to: edge.to,
                edge_type: edge.edge_type,
                label: edge.label,
                styling: None,
                weight: edge.weight,
            })
            .collect();

        let base_ast_viz = AstVisualizationResponse {
            metadata: Some(metadata),
            root_node: proto_nodes.first().cloned(),
            nodes: proto_nodes,
            edges: proto_edges,
            statistics: Some(statistics),
        };

        // Build workflow progression
        let workflow_progression = WorkflowProgression {
            current_stage: domain_visualization.workflow_progression.current_stage,
            completed_stages: domain_visualization.workflow_progression.completed_stages,
            remaining_stages: domain_visualization.workflow_progression.remaining_stages,
            progression_percentage: domain_visualization
                .workflow_progression
                .progression_percentage as f64,
        };

        // Build critical paths
        let critical_paths: Vec<CriticalPath> = domain_visualization
            .critical_paths
            .into_iter()
            .map(|path| CriticalPath {
                path_id: path.path_id,
                nodes: path.nodes,
                estimated_duration: path.estimated_duration,
                risk_level: path.risk_level,
            })
            .collect();

        Ok(Response::new(DomainVisualizationResponse {
            base_visualization: Some(base_ast_viz),
            domain_context: domain_visualization
                .domain_context
                .map(|c| c.try_into().ok())
                .flatten(),
            highlighted_nodes: domain_visualization.highlighted_nodes,
            workflow_progression: Some(workflow_progression),
            critical_paths,
        }))
    }
}
