//! DSL Transform Service Implementation
//!
//! This service implements the gRPC interface for all DSL state transformations
//! and edit operations. It serves as the boundary between external systems and
//! the DSL engine core, handling all mutations to DSL instances.

use crate::database::dsl_instance_repository::PgDslInstanceRepository;
use crate::dsl_manager::{
    CompilationStatus, DslError, DslInstance, DslInstanceVersion, DslManager, DslResult,
    DslTemplate, InstanceStatus, OperationType,
};
use crate::parser::parse_program;
use chrono::Utc;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use tonic::{Request, Response, Status};
use uuid::Uuid;

// Generated proto types - when you run tonic-build, these will be available
use crate::proto::dsl_transform::{
    dsl_transform_service_server::{DslTransformService, DslTransformServiceServer},
    AddProductsRequest, ApplyTemplateRequest, AssociateCbuRequest, AssociateCbuResponse,
    BusinessReferenceResponse, CompilationLog, CompilationResponse,
    CompilationStatus as ProtoCompilationStatus, CompileDslVersionRequest,
    CreateDslInstanceRequest, CreateDslTemplateRequest, CreateKycCaseRequest,
    CreateOnboardingCaseRequest, CreateVersionRequest, DeleteDslInstanceRequest,
    DeleteDslInstanceResponse, DiscoverResourcesRequest, DiscoverServicesRequest,
    DslInstanceResponse, DslTemplateResponse, DslVersionResponse, EditDslContentRequest, EditType,
    InstanceStatus as ProtoInstanceStatus, KycCaseResponse, LinkBusinessReferenceRequest,
    OnboardingCaseResponse, OperationType as ProtoOperationType, UpdateDslTemplateRequest,
    UpdateInstanceMetadataRequest, UpdateInstanceStatusRequest,
};

pub struct DslTransformServiceImpl {
    dsl_manager: Arc<DslManager>,
    instance_repository: Arc<PgDslInstanceRepository>,
}

impl DslTransformServiceImpl {
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

    fn from_proto_instance_status(&self, status: i32) -> DslResult<InstanceStatus> {
        match status {
            1 => Ok(InstanceStatus::Created),
            2 => Ok(InstanceStatus::Editing),
            3 => Ok(InstanceStatus::Compiled),
            4 => Ok(InstanceStatus::Finalized),
            5 => Ok(InstanceStatus::Archived),
            6 => Ok(InstanceStatus::Failed),
            _ => Err(DslError::ValidationError {
                message: format!("Invalid instance status: {}", status),
            }),
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

    fn from_proto_operation_type(&self, op_type: i32) -> DslResult<OperationType> {
        match op_type {
            1 => Ok(OperationType::CreateFromTemplate),
            2 => Ok(OperationType::IncrementalEdit),
            3 => Ok(OperationType::TemplateAddition),
            4 => Ok(OperationType::ManualEdit),
            5 => Ok(OperationType::Recompilation),
            _ => Err(DslError::ValidationError {
                message: format!("Invalid operation type: {}", op_type),
            }),
        }
    }

    fn to_proto_compilation_status(&self, status: CompilationStatus) -> i32 {
        match status {
            CompilationStatus::Pending => ProtoCompilationStatus::CompilationStatusPending as i32,
            CompilationStatus::Success => ProtoCompilationStatus::CompilationStatusSuccess as i32,
            CompilationStatus::Error => ProtoCompilationStatus::CompilationStatusError as i32,
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
}

#[tonic::async_trait]
impl DslTransformService for DslTransformServiceImpl {
    // Instance management
    async fn create_dsl_instance(
        &self,
        request: Request<CreateDslInstanceRequest>,
    ) -> Result<Response<DslInstanceResponse>, Status> {
        let req = request.into_inner();

        let domain_name = req.domain_name;
        let business_reference = req.business_reference;

        let metadata = if let Some(metadata_struct) = req.metadata {
            let metadata_value: serde_json::Value = metadata_struct
                .try_into()
                .map_err(|e| Status::invalid_argument(format!("Invalid metadata format: {}", e)))?;
            Some(metadata_value)
        } else {
            None
        };

        let instance = self
            .instance_repository
            .create_instance(&domain_name, &business_reference, metadata)
            .await
            .map_err(|e| Status::internal(format!("Failed to create DSL instance: {}", e)))?;

        Ok(Response::new(self.to_proto_instance(instance)))
    }

    async fn update_instance_status(
        &self,
        request: Request<UpdateInstanceStatusRequest>,
    ) -> Result<Response<DslInstanceResponse>, Status> {
        let req = request.into_inner();

        let instance_id = Uuid::parse_str(&req.instance_id)
            .map_err(|_| Status::invalid_argument("Invalid instance ID format"))?;

        let status = self
            .from_proto_instance_status(req.status)
            .map_err(|e| Status::invalid_argument(format!("Invalid status: {}", e)))?;

        let instance = self
            .instance_repository
            .update_instance_status(instance_id, status)
            .await
            .map_err(|e| Status::internal(format!("Failed to update instance status: {}", e)))?;

        Ok(Response::new(self.to_proto_instance(instance)))
    }

    async fn update_instance_metadata(
        &self,
        request: Request<UpdateInstanceMetadataRequest>,
    ) -> Result<Response<DslInstanceResponse>, Status> {
        let req = request.into_inner();

        let instance_id = Uuid::parse_str(&req.instance_id)
            .map_err(|_| Status::invalid_argument("Invalid instance ID format"))?;

        let metadata_struct = req
            .metadata
            .ok_or_else(|| Status::invalid_argument("Metadata is required"))?;

        let metadata_value: serde_json::Value = metadata_struct
            .try_into()
            .map_err(|e| Status::invalid_argument(format!("Invalid metadata format: {}", e)))?;

        let instance = self
            .instance_repository
            .update_instance_metadata(instance_id, metadata_value)
            .await
            .map_err(|e| Status::internal(format!("Failed to update instance metadata: {}", e)))?;

        Ok(Response::new(self.to_proto_instance(instance)))
    }

    async fn delete_dsl_instance(
        &self,
        request: Request<DeleteDslInstanceRequest>,
    ) -> Result<Response<DeleteDslInstanceResponse>, Status> {
        let req = request.into_inner();

        let instance_id = Uuid::parse_str(&req.instance_id)
            .map_err(|_| Status::invalid_argument("Invalid instance ID format"))?;

        self.instance_repository
            .delete_instance(instance_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to delete DSL instance: {}", e)))?;

        Ok(Response::new(DeleteDslInstanceResponse { success: true }))
    }

    // Version and state changes
    async fn create_version(
        &self,
        request: Request<CreateVersionRequest>,
    ) -> Result<Response<DslVersionResponse>, Status> {
        let req = request.into_inner();

        let instance_id = Uuid::parse_str(&req.instance_id)
            .map_err(|_| Status::invalid_argument("Invalid instance ID format"))?;

        let operation_type = self
            .from_proto_operation_type(req.operation_type)
            .map_err(|e| Status::invalid_argument(format!("Invalid operation type: {}", e)))?;

        let created_by = if req.created_by.is_empty() {
            None
        } else {
            Some(&req.created_by)
        };
        let change_description = if req.change_description.is_empty() {
            None
        } else {
            Some(&req.change_description)
        };

        // Validate DSL syntax before creating version
        if let Err(parse_error) = parse_program(&req.dsl_content) {
            return Err(Status::invalid_argument(format!(
                "DSL parsing failed: {:?}",
                parse_error
            )));
        }

        let version = self
            .instance_repository
            .create_version(
                instance_id,
                &req.dsl_content,
                operation_type,
                created_by,
                change_description,
            )
            .await
            .map_err(|e| Status::internal(format!("Failed to create DSL version: {}", e)))?;

        Ok(Response::new(self.to_proto_version(version)))
    }

    async fn edit_dsl_content(
        &self,
        request: Request<EditDslContentRequest>,
    ) -> Result<Response<DslVersionResponse>, Status> {
        let req = request.into_inner();

        let instance_id = Uuid::parse_str(&req.instance_id)
            .map_err(|_| Status::invalid_argument("Invalid instance ID format"))?;

        // Get the latest version first
        let latest_version = self
            .instance_repository
            .get_latest_version(instance_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to get latest version: {}", e)))?
            .ok_or_else(|| Status::not_found("No versions found for this instance"))?;

        // Apply the edit based on the edit type
        let new_content = match req.edit_type {
            // Replace the entire content
            2 /* EDIT_TYPE_REPLACE */ => req.dsl_content,

            // Append to the end
            1 /* EDIT_TYPE_APPEND */ => format!("{}\n{}", latest_version.dsl_content, req.dsl_content),

            // Other edit types not yet implemented
            _ => return Err(Status::unimplemented("This edit type is not implemented yet")),
        };

        // Validate DSL syntax after edit
        if let Err(parse_error) = parse_program(&new_content) {
            return Err(Status::invalid_argument(format!(
                "DSL parsing failed after edit: {:?}",
                parse_error
            )));
        }

        let created_by = if req.created_by.is_empty() {
            None
        } else {
            Some(&req.created_by)
        };
        let change_description = if req.change_description.is_empty() {
            None
        } else {
            Some(&req.change_description)
        };

        let version = self
            .instance_repository
            .create_version(
                instance_id,
                &new_content,
                OperationType::IncrementalEdit,
                created_by,
                change_description,
            )
            .await
            .map_err(|e| Status::internal(format!("Failed to create edited DSL version: {}", e)))?;

        Ok(Response::new(self.to_proto_version(version)))
    }

    async fn apply_template_to_instance(
        &self,
        request: Request<ApplyTemplateRequest>,
    ) -> Result<Response<DslVersionResponse>, Status> {
        let req = request.into_inner();

        let instance_id = Uuid::parse_str(&req.instance_id)
            .map_err(|_| Status::invalid_argument("Invalid instance ID format"))?;

        // Get the template
        let template = self
            .instance_repository
            .get_template_by_name(&req.template_name)
            .await
            .map_err(|e| Status::internal(format!("Failed to fetch template: {}", e)))?
            .ok_or_else(|| {
                Status::not_found(format!("Template '{}' not found", req.template_name))
            })?;

        // Get the latest version
        let latest_version = self
            .instance_repository
            .get_latest_version(instance_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to get latest version: {}", e)))?
            .ok_or_else(|| Status::not_found("No versions found for this instance"))?;

        // Convert variables to a suitable format
        let variables = if let Some(vars_struct) = req.variables {
            let vars_value: serde_json::Value = vars_struct.try_into().map_err(|e| {
                Status::invalid_argument(format!("Invalid variables format: {}", e))
            })?;
            vars_value
        } else {
            serde_json::Value::Object(serde_json::Map::new())
        };

        // Apply template variables to generate content
        let template_content = template.content;
        let mut new_content = template_content.clone();

        // Simple placeholder replacement (a more sophisticated template engine would be used in production)
        if let serde_json::Value::Object(obj) = &variables {
            for (key, value) in obj {
                let placeholder = format!("{{{{ {} }}}}", key);
                if let serde_json::Value::String(str_val) = value {
                    new_content = new_content.replace(&placeholder, str_val);
                }
            }
        }

        // Combine with existing content
        let combined_content = format!("{}\n{}", latest_version.dsl_content, new_content);

        // Validate the combined DSL
        if let Err(parse_error) = parse_program(&combined_content) {
            return Err(Status::invalid_argument(format!(
                "DSL parsing failed after template application: {:?}",
                parse_error
            )));
        }

        let created_by = if req.created_by.is_empty() {
            None
        } else {
            Some(&req.created_by)
        };
        let change_description = if req.change_description.is_empty() {
            Some(&format!("Applied template: {}", req.template_name))
        } else {
            Some(&req.change_description)
        };

        let version = self
            .instance_repository
            .create_version(
                instance_id,
                &combined_content,
                OperationType::TemplateAddition,
                created_by,
                change_description,
            )
            .await
            .map_err(|e| {
                Status::internal(format!("Failed to create template-based version: {}", e))
            })?;

        Ok(Response::new(self.to_proto_version(version)))
    }

    // Template management
    async fn create_dsl_template(
        &self,
        request: Request<CreateDslTemplateRequest>,
    ) -> Result<Response<DslTemplateResponse>, Status> {
        let req = request.into_inner();

        let variables = if let Some(vars_struct) = req.variables {
            let vars_value: serde_json::Value = vars_struct.try_into().map_err(|e| {
                Status::invalid_argument(format!("Invalid variables format: {}", e))
            })?;
            Some(vars_value)
        } else {
            None
        };

        let requirements = if let Some(reqs_struct) = req.requirements {
            let reqs_value: serde_json::Value = reqs_struct.try_into().map_err(|e| {
                Status::invalid_argument(format!("Invalid requirements format: {}", e))
            })?;
            Some(reqs_value)
        } else {
            None
        };

        let metadata = if let Some(meta_struct) = req.metadata {
            let meta_value: serde_json::Value = meta_struct
                .try_into()
                .map_err(|e| Status::invalid_argument(format!("Invalid metadata format: {}", e)))?;
            Some(meta_value)
        } else {
            None
        };

        let template = self
            .instance_repository
            .create_template(
                &req.template_name,
                &req.domain_name,
                &req.template_type,
                &req.content,
                variables,
                requirements,
                metadata,
            )
            .await
            .map_err(|e| Status::internal(format!("Failed to create DSL template: {}", e)))?;

        Ok(Response::new(self.to_proto_template(template)))
    }

    async fn update_dsl_template(
        &self,
        request: Request<UpdateDslTemplateRequest>,
    ) -> Result<Response<DslTemplateResponse>, Status> {
        let req = request.into_inner();

        let template_id = Uuid::parse_str(&req.template_id)
            .map_err(|_| Status::invalid_argument("Invalid template ID format"))?;

        let variables = if let Some(vars_struct) = req.variables {
            let vars_value: serde_json::Value = vars_struct.try_into().map_err(|e| {
                Status::invalid_argument(format!("Invalid variables format: {}", e))
            })?;
            Some(vars_value)
        } else {
            None
        };

        let requirements = if let Some(reqs_struct) = req.requirements {
            let reqs_value: serde_json::Value = reqs_struct.try_into().map_err(|e| {
                Status::invalid_argument(format!("Invalid requirements format: {}", e))
            })?;
            Some(reqs_value)
        } else {
            None
        };

        let metadata = if let Some(meta_struct) = req.metadata {
            let meta_value: serde_json::Value = meta_struct
                .try_into()
                .map_err(|e| Status::invalid_argument(format!("Invalid metadata format: {}", e)))?;
            Some(meta_value)
        } else {
            None
        };

        let template = self
            .instance_repository
            .update_template(template_id, &req.content, variables, requirements, metadata)
            .await
            .map_err(|e| Status::internal(format!("Failed to update DSL template: {}", e)))?;

        Ok(Response::new(self.to_proto_template(template)))
    }

    // Compilation and AST generation
    async fn compile_dsl_version(
        &self,
        request: Request<CompileDslVersionRequest>,
    ) -> Result<Response<CompilationResponse>, Status> {
        let req = request.into_inner();

        let version_id = Uuid::parse_str(&req.version_id)
            .map_err(|_| Status::invalid_argument("Invalid version ID format"))?;

        let version = self
            .instance_repository
            .get_version(version_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to fetch version: {}", e)))?
            .ok_or_else(|| Status::not_found("Version not found"))?;

        // Start compilation log
        let compilation_log = self
            .instance_repository
            .create_compilation_log(version_id, Utc::now())
            .await
            .map_err(|e| Status::internal(format!("Failed to create compilation log: {}", e)))?;

        // Parse and compile the DSL
        let compilation_result = match parse_program(&version.dsl_content) {
            Ok(program) => {
                // Convert the AST to JSON
                let ast_json = match serde_json::to_value(&program) {
                    Ok(json) => {
                        // Update the version with the AST
                        let updated_version = self
                            .instance_repository
                            .update_version_ast(
                                version_id,
                                json.clone(),
                                CompilationStatus::Success,
                            )
                            .await
                            .map_err(|e| {
                                Status::internal(format!("Failed to update AST: {}", e))
                            })?;

                        // If requested, store the AST nodes for querying
                        if req.store_ast_nodes {
                            // Simplified - in a real implementation, you'd create AstNode objects
                            // from the program structure and store them
                            // This is just a placeholder
                        }

                        // Complete the compilation log
                        let end_time = Utc::now();
                        let _ = self
                            .instance_repository
                            .complete_compilation_log(
                                compilation_log.log_id,
                                end_time,
                                true,
                                None,
                                None,
                                Some(program.statements.len() as i32),
                                Some(1.0), // Simple complexity score
                                None,
                            )
                            .await;

                        // Build response
                        let json_struct = json.try_into().map_err(|e| {
                            Status::internal(format!(
                                "Failed to convert AST to proto struct: {}",
                                e
                            ))
                        })?;

                        (
                            ProtoCompilationStatus::CompilationStatusSuccess as i32,
                            Some(json_struct),
                            true,
                        )
                    }
                    Err(e) => {
                        // Complete the compilation log with error
                        let end_time = Utc::now();
                        let _ = self
                            .instance_repository
                            .complete_compilation_log(
                                compilation_log.log_id,
                                end_time,
                                false,
                                Some(&format!("AST serialization error: {}", e)),
                                None,
                                None,
                                None,
                                None,
                            )
                            .await;

                        (
                            ProtoCompilationStatus::CompilationStatusError as i32,
                            None,
                            false,
                        )
                    }
                };
            }
            Err(e) => {
                // Update version with error status
                let _ = self
                    .instance_repository
                    .update_version_ast(
                        version_id,
                        serde_json::Value::Null,
                        CompilationStatus::Error,
                    )
                    .await;

                // Complete the compilation log with error
                let end_time = Utc::now();
                let _ = self
                    .instance_repository
                    .complete_compilation_log(
                        compilation_log.log_id,
                        end_time,
                        false,
                        Some(&format!("Parse error: {:?}", e)),
                        None,
                        None,
                        None,
                        None,
                    )
                    .await;

                (
                    ProtoCompilationStatus::CompilationStatusError as i32,
                    None,
                    false,
                )
            }
        };

        // Build the compilation log response
        let proto_log = CompilationLog {
            log_id: compilation_log.log_id.to_string(),
            compilation_start: Some(prost_types::Timestamp {
                seconds: compilation_log.compilation_start.timestamp(),
                nanos: compilation_log.compilation_start.timestamp_subsec_nanos() as i32,
            }),
            compilation_end: compilation_log
                .compilation_end
                .map(|t| prost_types::Timestamp {
                    seconds: t.timestamp(),
                    nanos: t.timestamp_subsec_nanos() as i32,
                }),
            success: compilation_result.2,
            error_message: if compilation_result.2 {
                String::new()
            } else {
                "Compilation failed".to_string()
            },
            error_location: None,
            node_count: compilation_log.node_count.unwrap_or(0),
            complexity_score: compilation_log.complexity_score.unwrap_or(0.0),
            performance_metrics: None,
        };

        Ok(Response::new(CompilationResponse {
            version_id: version_id.to_string(),
            status: compilation_result.0,
            ast_json: compilation_result.1,
            log: Some(proto_log),
        }))
    }

    // Business reference operations
    async fn link_business_reference(
        &self,
        request: Request<LinkBusinessReferenceRequest>,
    ) -> Result<Response<BusinessReferenceResponse>, Status> {
        let req = request.into_inner();

        let instance_id = Uuid::parse_str(&req.instance_id)
            .map_err(|_| Status::invalid_argument("Invalid instance ID format"))?;

        let reference = self
            .instance_repository
            .create_business_reference(instance_id, &req.reference_type, &req.reference_id_value)
            .await
            .map_err(|e| Status::internal(format!("Failed to create business reference: {}", e)))?;

        Ok(Response::new(BusinessReferenceResponse {
            reference_id: reference.reference_id.to_string(),
            instance_id: reference.instance_id.to_string(),
            reference_type: reference.reference_type,
            reference_id_value: reference.reference_id_value,
            created_at: Some(prost_types::Timestamp {
                seconds: reference.created_at.timestamp(),
                nanos: reference.created_at.timestamp_subsec_nanos() as i32,
            }),
        }))
    }

    // Domain-specific operations - these delegate to the DSL manager's domain-specific functionality
    async fn create_onboarding_case(
        &self,
        request: Request<CreateOnboardingCaseRequest>,
    ) -> Result<Response<OnboardingCaseResponse>, Status> {
        // This operation would delegate to the existing dsl_manager.create_onboarding_request method
        // For now, we'll return unimplemented
        Err(Status::unimplemented(
            "Creating onboarding case through gRPC not yet implemented",
        ))
    }

    async fn create_kyc_case(
        &self,
        request: Request<CreateKycCaseRequest>,
    ) -> Result<Response<KycCaseResponse>, Status> {
        // This operation would delegate to the existing dsl_manager.create_kyc_case method
        // For now, we'll return unimplemented
        Err(Status::unimplemented(
            "Creating KYC case through gRPC not yet implemented",
        ))
    }

    async fn associate_cbu(
        &self,
        request: Request<AssociateCbuRequest>,
    ) -> Result<Response<AssociateCbuResponse>, Status> {
        // This operation would delegate to the existing dsl_manager.associate_cbu method
        // For now, we'll return unimplemented
        Err(Status::unimplemented(
            "Associating CBU through gRPC not yet implemented",
        ))
    }

    async fn add_products(
        &self,
        request: Request<AddProductsRequest>,
    ) -> Result<Response<DslVersionResponse>, Status> {
        // This operation would delegate to the existing dsl_manager.add_products method
        // For now, we'll return unimplemented
        Err(Status::unimplemented(
            "Adding products through gRPC not yet implemented",
        ))
    }

    async fn discover_services(
        &self,
        request: Request<DiscoverServicesRequest>,
    ) -> Result<Response<DslVersionResponse>, Status> {
        // This operation would delegate to the existing dsl_manager.discover_services method
        // For now, we'll return unimplemented
        Err(Status::unimplemented(
            "Discovering services through gRPC not yet implemented",
        ))
    }

    async fn discover_resources(
        &self,
        request: Request<DiscoverResourcesRequest>,
    ) -> Result<Response<DslVersionResponse>, Status> {
        // This operation would delegate to the existing dsl_manager.discover_resources method
        // For now, we'll return unimplemented
        Err(Status::unimplemented(
            "Discovering resources through gRPC not yet implemented",
        ))
    }
}
