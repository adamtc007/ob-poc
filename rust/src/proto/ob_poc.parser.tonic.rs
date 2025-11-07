// @generated
/// Generated client implementations.
pub mod parser_service_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct ParserServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl ParserServiceClient<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> ParserServiceClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_origin(inner: T, origin: Uri) -> Self {
            let inner = tonic::client::Grpc::with_origin(inner, origin);
            Self { inner }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> ParserServiceClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
            >>::Error: Into<StdError> + Send + Sync,
        {
            ParserServiceClient::new(InterceptedService::new(inner, interceptor))
        }
        /// Compress requests with the given encoding.
        ///
        /// This requires the server to support it otherwise it might respond with an
        /// error.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.send_compressed(encoding);
            self
        }
        /// Enable decompressing responses.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.accept_compressed(encoding);
            self
        }
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_decoding_message_size(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_encoding_message_size(limit);
            self
        }
        pub async fn parse_dsl(
            &mut self,
            request: impl tonic::IntoRequest<super::ParseDslRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ParseDslResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/ob_poc.parser.ParserService/ParseDSL",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("ob_poc.parser.ParserService", "ParseDSL"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn parse_dsl_stream(
            &mut self,
            request: impl tonic::IntoStreamingRequest<
                Message = super::ParseDslStreamRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::ParseDslStreamResponse>>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/ob_poc.parser.ParserService/ParseDSLStream",
            );
            let mut req = request.into_streaming_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("ob_poc.parser.ParserService", "ParseDSLStream"),
                );
            self.inner.streaming(req, path, codec).await
        }
        pub async fn parse_and_validate(
            &mut self,
            request: impl tonic::IntoRequest<super::ParseAndValidateRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ParseAndValidateResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/ob_poc.parser.ParserService/ParseAndValidate",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("ob_poc.parser.ParserService", "ParseAndValidate"),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn parse_workflow(
            &mut self,
            request: impl tonic::IntoRequest<super::ParseWorkflowRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ParseWorkflowResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/ob_poc.parser.ParserService/ParseWorkflow",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("ob_poc.parser.ParserService", "ParseWorkflow"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn parse_value(
            &mut self,
            request: impl tonic::IntoRequest<super::ParseValueRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ParseValueResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/ob_poc.parser.ParserService/ParseValue",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("ob_poc.parser.ParserService", "ParseValue"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn validate_ast(
            &mut self,
            request: impl tonic::IntoRequest<super::ValidateAstRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ValidateAstResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/ob_poc.parser.ParserService/ValidateAST",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("ob_poc.parser.ParserService", "ValidateAST"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_parser_config(
            &mut self,
            request: impl tonic::IntoRequest<super::GetParserConfigRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetParserConfigResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/ob_poc.parser.ParserService/GetParserConfig",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("ob_poc.parser.ParserService", "GetParserConfig"),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn update_parser_config(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateParserConfigRequest>,
        ) -> std::result::Result<
            tonic::Response<super::UpdateParserConfigResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/ob_poc.parser.ParserService/UpdateParserConfig",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("ob_poc.parser.ParserService", "UpdateParserConfig"),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn format_dsl(
            &mut self,
            request: impl tonic::IntoRequest<super::FormatDslRequest>,
        ) -> std::result::Result<
            tonic::Response<super::FormatDslResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/ob_poc.parser.ParserService/FormatDSL",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("ob_poc.parser.ParserService", "FormatDSL"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_parse_tree(
            &mut self,
            request: impl tonic::IntoRequest<super::GetParseTreeRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetParseTreeResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/ob_poc.parser.ParserService/GetParseTree",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("ob_poc.parser.ParserService", "GetParseTree"));
            self.inner.unary(req, path, codec).await
        }
    }
}
/// Generated server implementations.
pub mod parser_service_server {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    /// Generated trait containing gRPC methods that should be implemented for use with ParserServiceServer.
    #[async_trait]
    pub trait ParserService: Send + Sync + 'static {
        async fn parse_dsl(
            &self,
            request: tonic::Request<super::ParseDslRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ParseDslResponse>,
            tonic::Status,
        >;
        /// Server streaming response type for the ParseDSLStream method.
        type ParseDSLStreamStream: tonic::codegen::tokio_stream::Stream<
                Item = std::result::Result<super::ParseDslStreamResponse, tonic::Status>,
            >
            + Send
            + 'static;
        async fn parse_dsl_stream(
            &self,
            request: tonic::Request<tonic::Streaming<super::ParseDslStreamRequest>>,
        ) -> std::result::Result<
            tonic::Response<Self::ParseDSLStreamStream>,
            tonic::Status,
        >;
        async fn parse_and_validate(
            &self,
            request: tonic::Request<super::ParseAndValidateRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ParseAndValidateResponse>,
            tonic::Status,
        >;
        async fn parse_workflow(
            &self,
            request: tonic::Request<super::ParseWorkflowRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ParseWorkflowResponse>,
            tonic::Status,
        >;
        async fn parse_value(
            &self,
            request: tonic::Request<super::ParseValueRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ParseValueResponse>,
            tonic::Status,
        >;
        async fn validate_ast(
            &self,
            request: tonic::Request<super::ValidateAstRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ValidateAstResponse>,
            tonic::Status,
        >;
        async fn get_parser_config(
            &self,
            request: tonic::Request<super::GetParserConfigRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetParserConfigResponse>,
            tonic::Status,
        >;
        async fn update_parser_config(
            &self,
            request: tonic::Request<super::UpdateParserConfigRequest>,
        ) -> std::result::Result<
            tonic::Response<super::UpdateParserConfigResponse>,
            tonic::Status,
        >;
        async fn format_dsl(
            &self,
            request: tonic::Request<super::FormatDslRequest>,
        ) -> std::result::Result<
            tonic::Response<super::FormatDslResponse>,
            tonic::Status,
        >;
        async fn get_parse_tree(
            &self,
            request: tonic::Request<super::GetParseTreeRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetParseTreeResponse>,
            tonic::Status,
        >;
    }
    #[derive(Debug)]
    pub struct ParserServiceServer<T: ParserService> {
        inner: Arc<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
        max_decoding_message_size: Option<usize>,
        max_encoding_message_size: Option<usize>,
    }
    impl<T: ParserService> ParserServiceServer<T> {
        pub fn new(inner: T) -> Self {
            Self::from_arc(Arc::new(inner))
        }
        pub fn from_arc(inner: Arc<T>) -> Self {
            Self {
                inner,
                accept_compression_encodings: Default::default(),
                send_compression_encodings: Default::default(),
                max_decoding_message_size: None,
                max_encoding_message_size: None,
            }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> InterceptedService<Self, F>
        where
            F: tonic::service::Interceptor,
        {
            InterceptedService::new(Self::new(inner), interceptor)
        }
        /// Enable decompressing requests with the given encoding.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.accept_compression_encodings.enable(encoding);
            self
        }
        /// Compress responses with the given encoding, if the client supports it.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.send_compression_encodings.enable(encoding);
            self
        }
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.max_decoding_message_size = Some(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.max_encoding_message_size = Some(limit);
            self
        }
    }
    impl<T, B> tonic::codegen::Service<http::Request<B>> for ParserServiceServer<T>
    where
        T: ParserService,
        B: Body + Send + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = std::convert::Infallible;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(
            &mut self,
            _cx: &mut Context<'_>,
        ) -> Poll<std::result::Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            match req.uri().path() {
                "/ob_poc.parser.ParserService/ParseDSL" => {
                    #[allow(non_camel_case_types)]
                    struct ParseDSLSvc<T: ParserService>(pub Arc<T>);
                    impl<
                        T: ParserService,
                    > tonic::server::UnaryService<super::ParseDslRequest>
                    for ParseDSLSvc<T> {
                        type Response = super::ParseDslResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ParseDslRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as ParserService>::parse_dsl(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let method = ParseDSLSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/ob_poc.parser.ParserService/ParseDSLStream" => {
                    #[allow(non_camel_case_types)]
                    struct ParseDSLStreamSvc<T: ParserService>(pub Arc<T>);
                    impl<
                        T: ParserService,
                    > tonic::server::StreamingService<super::ParseDslStreamRequest>
                    for ParseDSLStreamSvc<T> {
                        type Response = super::ParseDslStreamResponse;
                        type ResponseStream = T::ParseDSLStreamStream;
                        type Future = BoxFuture<
                            tonic::Response<Self::ResponseStream>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<
                                tonic::Streaming<super::ParseDslStreamRequest>,
                            >,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as ParserService>::parse_dsl_stream(&inner, request)
                                    .await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let method = ParseDSLStreamSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.streaming(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/ob_poc.parser.ParserService/ParseAndValidate" => {
                    #[allow(non_camel_case_types)]
                    struct ParseAndValidateSvc<T: ParserService>(pub Arc<T>);
                    impl<
                        T: ParserService,
                    > tonic::server::UnaryService<super::ParseAndValidateRequest>
                    for ParseAndValidateSvc<T> {
                        type Response = super::ParseAndValidateResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ParseAndValidateRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as ParserService>::parse_and_validate(&inner, request)
                                    .await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let method = ParseAndValidateSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/ob_poc.parser.ParserService/ParseWorkflow" => {
                    #[allow(non_camel_case_types)]
                    struct ParseWorkflowSvc<T: ParserService>(pub Arc<T>);
                    impl<
                        T: ParserService,
                    > tonic::server::UnaryService<super::ParseWorkflowRequest>
                    for ParseWorkflowSvc<T> {
                        type Response = super::ParseWorkflowResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ParseWorkflowRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as ParserService>::parse_workflow(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let method = ParseWorkflowSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/ob_poc.parser.ParserService/ParseValue" => {
                    #[allow(non_camel_case_types)]
                    struct ParseValueSvc<T: ParserService>(pub Arc<T>);
                    impl<
                        T: ParserService,
                    > tonic::server::UnaryService<super::ParseValueRequest>
                    for ParseValueSvc<T> {
                        type Response = super::ParseValueResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ParseValueRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as ParserService>::parse_value(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let method = ParseValueSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/ob_poc.parser.ParserService/ValidateAST" => {
                    #[allow(non_camel_case_types)]
                    struct ValidateASTSvc<T: ParserService>(pub Arc<T>);
                    impl<
                        T: ParserService,
                    > tonic::server::UnaryService<super::ValidateAstRequest>
                    for ValidateASTSvc<T> {
                        type Response = super::ValidateAstResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ValidateAstRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as ParserService>::validate_ast(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let method = ValidateASTSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/ob_poc.parser.ParserService/GetParserConfig" => {
                    #[allow(non_camel_case_types)]
                    struct GetParserConfigSvc<T: ParserService>(pub Arc<T>);
                    impl<
                        T: ParserService,
                    > tonic::server::UnaryService<super::GetParserConfigRequest>
                    for GetParserConfigSvc<T> {
                        type Response = super::GetParserConfigResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetParserConfigRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as ParserService>::get_parser_config(&inner, request)
                                    .await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let method = GetParserConfigSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/ob_poc.parser.ParserService/UpdateParserConfig" => {
                    #[allow(non_camel_case_types)]
                    struct UpdateParserConfigSvc<T: ParserService>(pub Arc<T>);
                    impl<
                        T: ParserService,
                    > tonic::server::UnaryService<super::UpdateParserConfigRequest>
                    for UpdateParserConfigSvc<T> {
                        type Response = super::UpdateParserConfigResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::UpdateParserConfigRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as ParserService>::update_parser_config(&inner, request)
                                    .await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let method = UpdateParserConfigSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/ob_poc.parser.ParserService/FormatDSL" => {
                    #[allow(non_camel_case_types)]
                    struct FormatDSLSvc<T: ParserService>(pub Arc<T>);
                    impl<
                        T: ParserService,
                    > tonic::server::UnaryService<super::FormatDslRequest>
                    for FormatDSLSvc<T> {
                        type Response = super::FormatDslResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::FormatDslRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as ParserService>::format_dsl(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let method = FormatDSLSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/ob_poc.parser.ParserService/GetParseTree" => {
                    #[allow(non_camel_case_types)]
                    struct GetParseTreeSvc<T: ParserService>(pub Arc<T>);
                    impl<
                        T: ParserService,
                    > tonic::server::UnaryService<super::GetParseTreeRequest>
                    for GetParseTreeSvc<T> {
                        type Response = super::GetParseTreeResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetParseTreeRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as ParserService>::get_parse_tree(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let method = GetParseTreeSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => {
                    Box::pin(async move {
                        Ok(
                            http::Response::builder()
                                .status(200)
                                .header("grpc-status", tonic::Code::Unimplemented as i32)
                                .header(
                                    http::header::CONTENT_TYPE,
                                    tonic::metadata::GRPC_CONTENT_TYPE,
                                )
                                .body(empty_body())
                                .unwrap(),
                        )
                    })
                }
            }
        }
    }
    impl<T: ParserService> Clone for ParserServiceServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self {
                inner,
                accept_compression_encodings: self.accept_compression_encodings,
                send_compression_encodings: self.send_compression_encodings,
                max_decoding_message_size: self.max_decoding_message_size,
                max_encoding_message_size: self.max_encoding_message_size,
            }
        }
    }
    impl<T: ParserService> tonic::server::NamedService for ParserServiceServer<T> {
        const NAME: &'static str = "ob_poc.parser.ParserService";
    }
}
