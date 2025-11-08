//! Minimal gRPC implementation for the Parser service
//!
//! This is a simplified implementation to test the gRPC setup

use tonic::{Request, Response, Status};

use crate::proto::ob_poc::parser::tonic::parser_service_server::ParserService;
use crate::proto::ob_poc::parser::*;

/// Minimal Parser service implementation
#[derive(Debug, Default)]
pub struct ParserServiceImpl;

impl ParserServiceImpl {
    pub fn new() -> Self {
        Self
    }
}

#[tonic::async_trait]
impl ParserService for ParserServiceImpl {
    /// Parse DSL source code into AST
    async fn parse_dsl(
        &self,
        request: Request<ParseDslRequest>,
    ) -> Result<Response<ParseDslResponse>, Status> {
        let req = request.into_inner();

        // For now, just return success with empty program
        let program = crate::proto::ob_poc::dsl::Program {
            workflows: vec![],
            global_properties: None,
            global_errors: vec![],
            global_warnings: vec![],
            semantic_info: None,
            parsed_at: Some(prost_types::Timestamp {
                seconds: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64,
                nanos: 0,
            }),
        };

        let response = ParseDslResponse {
            result: Some(parse_dsl_response::Result::Program(program)),
            metrics: None,
            warnings: vec![],
        };

        Ok(Response::new(response))
    }

    /// Parse DSL with streaming for large files - unimplemented
    async fn parse_dsl_stream(
        &self,
        _request: Request<tonic::Streaming<ParseDslStreamRequest>>,
    ) -> Result<
        Response<
            std::pin::Pin<
                Box<
                    dyn tokio_stream::Stream<Item = Result<ParseDslStreamResponse, Status>>
                        + Send
                        + 'static,
                >,
            >,
        >,
        Status,
    > {
        Err(Status::unimplemented(
            "parse_dsl_stream not yet implemented",
        ))
    }

    /// Parse and validate DSL in one operation - unimplemented
    async fn parse_and_validate(
        &self,
        _request: Request<ParseAndValidateRequest>,
    ) -> Result<Response<ParseAndValidateResponse>, Status> {
        Err(Status::unimplemented(
            "parse_and_validate not yet implemented",
        ))
    }

    /// Parse a single workflow - unimplemented
    async fn parse_workflow(
        &self,
        _request: Request<ParseWorkflowRequest>,
    ) -> Result<Response<ParseWorkflowResponse>, Status> {
        Err(Status::unimplemented("parse_workflow not yet implemented"))
    }

    /// Parse a single value/expression - unimplemented
    async fn parse_value(
        &self,
        _request: Request<ParseValueRequest>,
    ) -> Result<Response<ParseValueResponse>, Status> {
        Err(Status::unimplemented("parse_value not yet implemented"))
    }

    /// Validate a parsed AST - unimplemented
    async fn validate_ast(
        &self,
        _request: Request<ValidateAstRequest>,
    ) -> Result<Response<ValidateAstResponse>, Status> {
        Err(Status::unimplemented("validate_ast not yet implemented"))
    }

    /// Get parser configuration - unimplemented
    async fn get_parser_config(
        &self,
        _request: Request<GetParserConfigRequest>,
    ) -> Result<Response<GetParserConfigResponse>, Status> {
        Err(Status::unimplemented(
            "get_parser_config not yet implemented",
        ))
    }

    /// Update parser configuration - unimplemented
    async fn update_parser_config(
        &self,
        _request: Request<UpdateParserConfigRequest>,
    ) -> Result<Response<UpdateParserConfigResponse>, Status> {
        Err(Status::unimplemented(
            "update_parser_config not yet implemented",
        ))
    }

    /// Format/prettify DSL source code - unimplemented
    async fn format_dsl(
        &self,
        _request: Request<FormatDslRequest>,
    ) -> Result<Response<FormatDslResponse>, Status> {
        Err(Status::unimplemented("format_dsl not yet implemented"))
    }

    /// Get parse tree for debugging - unimplemented
    async fn get_parse_tree(
        &self,
        _request: Request<GetParseTreeRequest>,
    ) -> Result<Response<GetParseTreeResponse>, Status> {
        Err(Status::unimplemented("get_parse_tree not yet implemented"))
    }
}
