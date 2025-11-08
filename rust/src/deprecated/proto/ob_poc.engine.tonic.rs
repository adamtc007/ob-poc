// @generated
/// Generated client implementations.
pub mod dsl_engine_service_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct DslEngineServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl DslEngineServiceClient<tonic::transport::Channel> {
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
    impl<T> DslEngineServiceClient<T>
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
        ) -> DslEngineServiceClient<InterceptedService<T, F>>
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
            DslEngineServiceClient::new(InterceptedService::new(inner, interceptor))
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
        pub async fn get_system_info(
            &mut self,
            request: impl tonic::IntoRequest<super::GetSystemInfoRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetSystemInfoResponse>,
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
                "/ob_poc.engine.DSLEngineService/GetSystemInfo",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("ob_poc.engine.DSLEngineService", "GetSystemInfo"),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_health(
            &mut self,
            request: impl tonic::IntoRequest<super::GetHealthRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetHealthResponse>,
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
                "/ob_poc.engine.DSLEngineService/GetHealth",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("ob_poc.engine.DSLEngineService", "GetHealth"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_metrics(
            &mut self,
            request: impl tonic::IntoRequest<super::GetMetricsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetMetricsResponse>,
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
                "/ob_poc.engine.DSLEngineService/GetMetrics",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("ob_poc.engine.DSLEngineService", "GetMetrics"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn update_configuration(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateConfigurationRequest>,
        ) -> std::result::Result<
            tonic::Response<super::UpdateConfigurationResponse>,
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
                "/ob_poc.engine.DSLEngineService/UpdateConfiguration",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "ob_poc.engine.DSLEngineService",
                        "UpdateConfiguration",
                    ),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn process_workflow(
            &mut self,
            request: impl tonic::IntoRequest<super::ProcessWorkflowRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ProcessWorkflowResponse>,
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
                "/ob_poc.engine.DSLEngineService/ProcessWorkflow",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("ob_poc.engine.DSLEngineService", "ProcessWorkflow"),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn process_workflow_stream(
            &mut self,
            request: impl tonic::IntoStreamingRequest<
                Message = super::ProcessWorkflowStreamRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<
                tonic::codec::Streaming<super::ProcessWorkflowStreamResponse>,
            >,
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
                "/ob_poc.engine.DSLEngineService/ProcessWorkflowStream",
            );
            let mut req = request.into_streaming_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "ob_poc.engine.DSLEngineService",
                        "ProcessWorkflowStream",
                    ),
                );
            self.inner.streaming(req, path, codec).await
        }
        pub async fn parse_and_execute(
            &mut self,
            request: impl tonic::IntoRequest<super::ParseAndExecuteRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ParseAndExecuteResponse>,
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
                "/ob_poc.engine.DSLEngineService/ParseAndExecute",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("ob_poc.engine.DSLEngineService", "ParseAndExecute"),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn validate_and_process(
            &mut self,
            request: impl tonic::IntoRequest<super::ValidateAndProcessRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ValidateAndProcessResponse>,
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
                "/ob_poc.engine.DSLEngineService/ValidateAndProcess",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "ob_poc.engine.DSLEngineService",
                        "ValidateAndProcess",
                    ),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn process_batch(
            &mut self,
            request: impl tonic::IntoRequest<super::ProcessBatchRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ProcessBatchResponse>,
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
                "/ob_poc.engine.DSLEngineService/ProcessBatch",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("ob_poc.engine.DSLEngineService", "ProcessBatch"),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn analyze_dsl(
            &mut self,
            request: impl tonic::IntoRequest<super::AnalyzeDslRequest>,
        ) -> std::result::Result<
            tonic::Response<super::AnalyzeDslResponse>,
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
                "/ob_poc.engine.DSLEngineService/AnalyzeDSL",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("ob_poc.engine.DSLEngineService", "AnalyzeDSL"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_processing_trace(
            &mut self,
            request: impl tonic::IntoRequest<super::GetProcessingTraceRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetProcessingTraceResponse>,
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
                "/ob_poc.engine.DSLEngineService/GetProcessingTrace",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "ob_poc.engine.DSLEngineService",
                        "GetProcessingTrace",
                    ),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn validate_configuration(
            &mut self,
            request: impl tonic::IntoRequest<super::ValidateConfigurationRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ValidateConfigurationResponse>,
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
                "/ob_poc.engine.DSLEngineService/ValidateConfiguration",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "ob_poc.engine.DSLEngineService",
                        "ValidateConfiguration",
                    ),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn clear_cache(
            &mut self,
            request: impl tonic::IntoRequest<super::ClearCacheRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ClearCacheResponse>,
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
                "/ob_poc.engine.DSLEngineService/ClearCache",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("ob_poc.engine.DSLEngineService", "ClearCache"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_cache_stats(
            &mut self,
            request: impl tonic::IntoRequest<super::GetCacheStatsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetCacheStatsResponse>,
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
                "/ob_poc.engine.DSLEngineService/GetCacheStats",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("ob_poc.engine.DSLEngineService", "GetCacheStats"),
                );
            self.inner.unary(req, path, codec).await
        }
    }
}
/// Generated server implementations.
pub mod dsl_engine_service_server {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    /// Generated trait containing gRPC methods that should be implemented for use with DslEngineServiceServer.
    #[async_trait]
    pub trait DslEngineService: Send + Sync + 'static {
        async fn get_system_info(
            &self,
            request: tonic::Request<super::GetSystemInfoRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetSystemInfoResponse>,
            tonic::Status,
        >;
        async fn get_health(
            &self,
            request: tonic::Request<super::GetHealthRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetHealthResponse>,
            tonic::Status,
        >;
        async fn get_metrics(
            &self,
            request: tonic::Request<super::GetMetricsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetMetricsResponse>,
            tonic::Status,
        >;
        async fn update_configuration(
            &self,
            request: tonic::Request<super::UpdateConfigurationRequest>,
        ) -> std::result::Result<
            tonic::Response<super::UpdateConfigurationResponse>,
            tonic::Status,
        >;
        async fn process_workflow(
            &self,
            request: tonic::Request<super::ProcessWorkflowRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ProcessWorkflowResponse>,
            tonic::Status,
        >;
        /// Server streaming response type for the ProcessWorkflowStream method.
        type ProcessWorkflowStreamStream: tonic::codegen::tokio_stream::Stream<
                Item = std::result::Result<
                    super::ProcessWorkflowStreamResponse,
                    tonic::Status,
                >,
            >
            + Send
            + 'static;
        async fn process_workflow_stream(
            &self,
            request: tonic::Request<
                tonic::Streaming<super::ProcessWorkflowStreamRequest>,
            >,
        ) -> std::result::Result<
            tonic::Response<Self::ProcessWorkflowStreamStream>,
            tonic::Status,
        >;
        async fn parse_and_execute(
            &self,
            request: tonic::Request<super::ParseAndExecuteRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ParseAndExecuteResponse>,
            tonic::Status,
        >;
        async fn validate_and_process(
            &self,
            request: tonic::Request<super::ValidateAndProcessRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ValidateAndProcessResponse>,
            tonic::Status,
        >;
        async fn process_batch(
            &self,
            request: tonic::Request<super::ProcessBatchRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ProcessBatchResponse>,
            tonic::Status,
        >;
        async fn analyze_dsl(
            &self,
            request: tonic::Request<super::AnalyzeDslRequest>,
        ) -> std::result::Result<
            tonic::Response<super::AnalyzeDslResponse>,
            tonic::Status,
        >;
        async fn get_processing_trace(
            &self,
            request: tonic::Request<super::GetProcessingTraceRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetProcessingTraceResponse>,
            tonic::Status,
        >;
        async fn validate_configuration(
            &self,
            request: tonic::Request<super::ValidateConfigurationRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ValidateConfigurationResponse>,
            tonic::Status,
        >;
        async fn clear_cache(
            &self,
            request: tonic::Request<super::ClearCacheRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ClearCacheResponse>,
            tonic::Status,
        >;
        async fn get_cache_stats(
            &self,
            request: tonic::Request<super::GetCacheStatsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetCacheStatsResponse>,
            tonic::Status,
        >;
    }
    #[derive(Debug)]
    pub struct DslEngineServiceServer<T: DslEngineService> {
        inner: Arc<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
        max_decoding_message_size: Option<usize>,
        max_encoding_message_size: Option<usize>,
    }
    impl<T: DslEngineService> DslEngineServiceServer<T> {
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
    impl<T, B> tonic::codegen::Service<http::Request<B>> for DslEngineServiceServer<T>
    where
        T: DslEngineService,
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
                "/ob_poc.engine.DSLEngineService/GetSystemInfo" => {
                    #[allow(non_camel_case_types)]
                    struct GetSystemInfoSvc<T: DslEngineService>(pub Arc<T>);
                    impl<
                        T: DslEngineService,
                    > tonic::server::UnaryService<super::GetSystemInfoRequest>
                    for GetSystemInfoSvc<T> {
                        type Response = super::GetSystemInfoResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetSystemInfoRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as DslEngineService>::get_system_info(&inner, request)
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
                        let method = GetSystemInfoSvc(inner);
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
                "/ob_poc.engine.DSLEngineService/GetHealth" => {
                    #[allow(non_camel_case_types)]
                    struct GetHealthSvc<T: DslEngineService>(pub Arc<T>);
                    impl<
                        T: DslEngineService,
                    > tonic::server::UnaryService<super::GetHealthRequest>
                    for GetHealthSvc<T> {
                        type Response = super::GetHealthResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetHealthRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as DslEngineService>::get_health(&inner, request).await
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
                        let method = GetHealthSvc(inner);
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
                "/ob_poc.engine.DSLEngineService/GetMetrics" => {
                    #[allow(non_camel_case_types)]
                    struct GetMetricsSvc<T: DslEngineService>(pub Arc<T>);
                    impl<
                        T: DslEngineService,
                    > tonic::server::UnaryService<super::GetMetricsRequest>
                    for GetMetricsSvc<T> {
                        type Response = super::GetMetricsResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetMetricsRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as DslEngineService>::get_metrics(&inner, request).await
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
                        let method = GetMetricsSvc(inner);
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
                "/ob_poc.engine.DSLEngineService/UpdateConfiguration" => {
                    #[allow(non_camel_case_types)]
                    struct UpdateConfigurationSvc<T: DslEngineService>(pub Arc<T>);
                    impl<
                        T: DslEngineService,
                    > tonic::server::UnaryService<super::UpdateConfigurationRequest>
                    for UpdateConfigurationSvc<T> {
                        type Response = super::UpdateConfigurationResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::UpdateConfigurationRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as DslEngineService>::update_configuration(
                                        &inner,
                                        request,
                                    )
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
                        let method = UpdateConfigurationSvc(inner);
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
                "/ob_poc.engine.DSLEngineService/ProcessWorkflow" => {
                    #[allow(non_camel_case_types)]
                    struct ProcessWorkflowSvc<T: DslEngineService>(pub Arc<T>);
                    impl<
                        T: DslEngineService,
                    > tonic::server::UnaryService<super::ProcessWorkflowRequest>
                    for ProcessWorkflowSvc<T> {
                        type Response = super::ProcessWorkflowResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ProcessWorkflowRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as DslEngineService>::process_workflow(&inner, request)
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
                        let method = ProcessWorkflowSvc(inner);
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
                "/ob_poc.engine.DSLEngineService/ProcessWorkflowStream" => {
                    #[allow(non_camel_case_types)]
                    struct ProcessWorkflowStreamSvc<T: DslEngineService>(pub Arc<T>);
                    impl<
                        T: DslEngineService,
                    > tonic::server::StreamingService<
                        super::ProcessWorkflowStreamRequest,
                    > for ProcessWorkflowStreamSvc<T> {
                        type Response = super::ProcessWorkflowStreamResponse;
                        type ResponseStream = T::ProcessWorkflowStreamStream;
                        type Future = BoxFuture<
                            tonic::Response<Self::ResponseStream>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<
                                tonic::Streaming<super::ProcessWorkflowStreamRequest>,
                            >,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as DslEngineService>::process_workflow_stream(
                                        &inner,
                                        request,
                                    )
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
                        let method = ProcessWorkflowStreamSvc(inner);
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
                "/ob_poc.engine.DSLEngineService/ParseAndExecute" => {
                    #[allow(non_camel_case_types)]
                    struct ParseAndExecuteSvc<T: DslEngineService>(pub Arc<T>);
                    impl<
                        T: DslEngineService,
                    > tonic::server::UnaryService<super::ParseAndExecuteRequest>
                    for ParseAndExecuteSvc<T> {
                        type Response = super::ParseAndExecuteResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ParseAndExecuteRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as DslEngineService>::parse_and_execute(&inner, request)
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
                        let method = ParseAndExecuteSvc(inner);
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
                "/ob_poc.engine.DSLEngineService/ValidateAndProcess" => {
                    #[allow(non_camel_case_types)]
                    struct ValidateAndProcessSvc<T: DslEngineService>(pub Arc<T>);
                    impl<
                        T: DslEngineService,
                    > tonic::server::UnaryService<super::ValidateAndProcessRequest>
                    for ValidateAndProcessSvc<T> {
                        type Response = super::ValidateAndProcessResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ValidateAndProcessRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as DslEngineService>::validate_and_process(
                                        &inner,
                                        request,
                                    )
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
                        let method = ValidateAndProcessSvc(inner);
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
                "/ob_poc.engine.DSLEngineService/ProcessBatch" => {
                    #[allow(non_camel_case_types)]
                    struct ProcessBatchSvc<T: DslEngineService>(pub Arc<T>);
                    impl<
                        T: DslEngineService,
                    > tonic::server::UnaryService<super::ProcessBatchRequest>
                    for ProcessBatchSvc<T> {
                        type Response = super::ProcessBatchResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ProcessBatchRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as DslEngineService>::process_batch(&inner, request)
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
                        let method = ProcessBatchSvc(inner);
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
                "/ob_poc.engine.DSLEngineService/AnalyzeDSL" => {
                    #[allow(non_camel_case_types)]
                    struct AnalyzeDSLSvc<T: DslEngineService>(pub Arc<T>);
                    impl<
                        T: DslEngineService,
                    > tonic::server::UnaryService<super::AnalyzeDslRequest>
                    for AnalyzeDSLSvc<T> {
                        type Response = super::AnalyzeDslResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::AnalyzeDslRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as DslEngineService>::analyze_dsl(&inner, request).await
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
                        let method = AnalyzeDSLSvc(inner);
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
                "/ob_poc.engine.DSLEngineService/GetProcessingTrace" => {
                    #[allow(non_camel_case_types)]
                    struct GetProcessingTraceSvc<T: DslEngineService>(pub Arc<T>);
                    impl<
                        T: DslEngineService,
                    > tonic::server::UnaryService<super::GetProcessingTraceRequest>
                    for GetProcessingTraceSvc<T> {
                        type Response = super::GetProcessingTraceResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetProcessingTraceRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as DslEngineService>::get_processing_trace(
                                        &inner,
                                        request,
                                    )
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
                        let method = GetProcessingTraceSvc(inner);
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
                "/ob_poc.engine.DSLEngineService/ValidateConfiguration" => {
                    #[allow(non_camel_case_types)]
                    struct ValidateConfigurationSvc<T: DslEngineService>(pub Arc<T>);
                    impl<
                        T: DslEngineService,
                    > tonic::server::UnaryService<super::ValidateConfigurationRequest>
                    for ValidateConfigurationSvc<T> {
                        type Response = super::ValidateConfigurationResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ValidateConfigurationRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as DslEngineService>::validate_configuration(
                                        &inner,
                                        request,
                                    )
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
                        let method = ValidateConfigurationSvc(inner);
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
                "/ob_poc.engine.DSLEngineService/ClearCache" => {
                    #[allow(non_camel_case_types)]
                    struct ClearCacheSvc<T: DslEngineService>(pub Arc<T>);
                    impl<
                        T: DslEngineService,
                    > tonic::server::UnaryService<super::ClearCacheRequest>
                    for ClearCacheSvc<T> {
                        type Response = super::ClearCacheResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ClearCacheRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as DslEngineService>::clear_cache(&inner, request).await
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
                        let method = ClearCacheSvc(inner);
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
                "/ob_poc.engine.DSLEngineService/GetCacheStats" => {
                    #[allow(non_camel_case_types)]
                    struct GetCacheStatsSvc<T: DslEngineService>(pub Arc<T>);
                    impl<
                        T: DslEngineService,
                    > tonic::server::UnaryService<super::GetCacheStatsRequest>
                    for GetCacheStatsSvc<T> {
                        type Response = super::GetCacheStatsResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetCacheStatsRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as DslEngineService>::get_cache_stats(&inner, request)
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
                        let method = GetCacheStatsSvc(inner);
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
    impl<T: DslEngineService> Clone for DslEngineServiceServer<T> {
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
    impl<T: DslEngineService> tonic::server::NamedService for DslEngineServiceServer<T> {
        const NAME: &'static str = "ob_poc.engine.DSLEngineService";
    }
}
