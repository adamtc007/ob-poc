// @generated
/// Generated client implementations.
pub mod ubo_service_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct UboServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl UboServiceClient<tonic::transport::Channel> {
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
    impl<T> UboServiceClient<T>
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
        ) -> UboServiceClient<InterceptedService<T, F>>
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
            UboServiceClient::new(InterceptedService::new(inner, interceptor))
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
        pub async fn calculate_ubo(
            &mut self,
            request: impl tonic::IntoRequest<super::CalculateUboRequest>,
        ) -> std::result::Result<
            tonic::Response<super::CalculateUboResponse>,
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
                "/ob_poc.ubo.UboService/CalculateUbo",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("ob_poc.ubo.UboService", "CalculateUbo"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn calculate_ubo_batch(
            &mut self,
            request: impl tonic::IntoRequest<super::CalculateUboBatchRequest>,
        ) -> std::result::Result<
            tonic::Response<super::CalculateUboBatchResponse>,
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
                "/ob_poc.ubo.UboService/CalculateUboBatch",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("ob_poc.ubo.UboService", "CalculateUboBatch"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn calculate_ubo_stream(
            &mut self,
            request: impl tonic::IntoStreamingRequest<
                Message = super::CalculateUboStreamRequest,
            >,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::CalculateUboStreamResponse>>,
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
                "/ob_poc.ubo.UboService/CalculateUboStream",
            );
            let mut req = request.into_streaming_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("ob_poc.ubo.UboService", "CalculateUboStream"));
            self.inner.streaming(req, path, codec).await
        }
        pub async fn get_ubo_history(
            &mut self,
            request: impl tonic::IntoRequest<super::GetUboHistoryRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetUboHistoryResponse>,
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
                "/ob_poc.ubo.UboService/GetUboHistory",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("ob_poc.ubo.UboService", "GetUboHistory"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn analyze_ownership_structure(
            &mut self,
            request: impl tonic::IntoRequest<super::AnalyzeOwnershipRequest>,
        ) -> std::result::Result<
            tonic::Response<super::AnalyzeOwnershipResponse>,
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
                "/ob_poc.ubo.UboService/AnalyzeOwnershipStructure",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("ob_poc.ubo.UboService", "AnalyzeOwnershipStructure"),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn validate_ubo_rules(
            &mut self,
            request: impl tonic::IntoRequest<super::ValidateUboRulesRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ValidateUboRulesResponse>,
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
                "/ob_poc.ubo.UboService/ValidateUboRules",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("ob_poc.ubo.UboService", "ValidateUboRules"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_ubo_metrics(
            &mut self,
            request: impl tonic::IntoRequest<super::GetUboMetricsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetUboMetricsResponse>,
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
                "/ob_poc.ubo.UboService/GetUboMetrics",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("ob_poc.ubo.UboService", "GetUboMetrics"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn export_ubo_results(
            &mut self,
            request: impl tonic::IntoRequest<super::ExportUboResultsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ExportUboResultsResponse>,
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
                "/ob_poc.ubo.UboService/ExportUboResults",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("ob_poc.ubo.UboService", "ExportUboResults"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn trace_ubo_calculation(
            &mut self,
            request: impl tonic::IntoRequest<super::TraceUboCalculationRequest>,
        ) -> std::result::Result<
            tonic::Response<super::TraceUboCalculationResponse>,
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
                "/ob_poc.ubo.UboService/TraceUboCalculation",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("ob_poc.ubo.UboService", "TraceUboCalculation"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn update_ubo_algorithm(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateUboAlgorithmRequest>,
        ) -> std::result::Result<
            tonic::Response<super::UpdateUboAlgorithmResponse>,
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
                "/ob_poc.ubo.UboService/UpdateUboAlgorithm",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("ob_poc.ubo.UboService", "UpdateUboAlgorithm"));
            self.inner.unary(req, path, codec).await
        }
    }
}
/// Generated server implementations.
pub mod ubo_service_server {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    /// Generated trait containing gRPC methods that should be implemented for use with UboServiceServer.
    #[async_trait]
    pub trait UboService: Send + Sync + 'static {
        async fn calculate_ubo(
            &self,
            request: tonic::Request<super::CalculateUboRequest>,
        ) -> std::result::Result<
            tonic::Response<super::CalculateUboResponse>,
            tonic::Status,
        >;
        async fn calculate_ubo_batch(
            &self,
            request: tonic::Request<super::CalculateUboBatchRequest>,
        ) -> std::result::Result<
            tonic::Response<super::CalculateUboBatchResponse>,
            tonic::Status,
        >;
        /// Server streaming response type for the CalculateUboStream method.
        type CalculateUboStreamStream: tonic::codegen::tokio_stream::Stream<
                Item = std::result::Result<
                    super::CalculateUboStreamResponse,
                    tonic::Status,
                >,
            >
            + Send
            + 'static;
        async fn calculate_ubo_stream(
            &self,
            request: tonic::Request<tonic::Streaming<super::CalculateUboStreamRequest>>,
        ) -> std::result::Result<
            tonic::Response<Self::CalculateUboStreamStream>,
            tonic::Status,
        >;
        async fn get_ubo_history(
            &self,
            request: tonic::Request<super::GetUboHistoryRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetUboHistoryResponse>,
            tonic::Status,
        >;
        async fn analyze_ownership_structure(
            &self,
            request: tonic::Request<super::AnalyzeOwnershipRequest>,
        ) -> std::result::Result<
            tonic::Response<super::AnalyzeOwnershipResponse>,
            tonic::Status,
        >;
        async fn validate_ubo_rules(
            &self,
            request: tonic::Request<super::ValidateUboRulesRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ValidateUboRulesResponse>,
            tonic::Status,
        >;
        async fn get_ubo_metrics(
            &self,
            request: tonic::Request<super::GetUboMetricsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetUboMetricsResponse>,
            tonic::Status,
        >;
        async fn export_ubo_results(
            &self,
            request: tonic::Request<super::ExportUboResultsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ExportUboResultsResponse>,
            tonic::Status,
        >;
        async fn trace_ubo_calculation(
            &self,
            request: tonic::Request<super::TraceUboCalculationRequest>,
        ) -> std::result::Result<
            tonic::Response<super::TraceUboCalculationResponse>,
            tonic::Status,
        >;
        async fn update_ubo_algorithm(
            &self,
            request: tonic::Request<super::UpdateUboAlgorithmRequest>,
        ) -> std::result::Result<
            tonic::Response<super::UpdateUboAlgorithmResponse>,
            tonic::Status,
        >;
    }
    #[derive(Debug)]
    pub struct UboServiceServer<T: UboService> {
        inner: Arc<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
        max_decoding_message_size: Option<usize>,
        max_encoding_message_size: Option<usize>,
    }
    impl<T: UboService> UboServiceServer<T> {
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
    impl<T, B> tonic::codegen::Service<http::Request<B>> for UboServiceServer<T>
    where
        T: UboService,
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
                "/ob_poc.ubo.UboService/CalculateUbo" => {
                    #[allow(non_camel_case_types)]
                    struct CalculateUboSvc<T: UboService>(pub Arc<T>);
                    impl<
                        T: UboService,
                    > tonic::server::UnaryService<super::CalculateUboRequest>
                    for CalculateUboSvc<T> {
                        type Response = super::CalculateUboResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::CalculateUboRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as UboService>::calculate_ubo(&inner, request).await
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
                        let method = CalculateUboSvc(inner);
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
                "/ob_poc.ubo.UboService/CalculateUboBatch" => {
                    #[allow(non_camel_case_types)]
                    struct CalculateUboBatchSvc<T: UboService>(pub Arc<T>);
                    impl<
                        T: UboService,
                    > tonic::server::UnaryService<super::CalculateUboBatchRequest>
                    for CalculateUboBatchSvc<T> {
                        type Response = super::CalculateUboBatchResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::CalculateUboBatchRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as UboService>::calculate_ubo_batch(&inner, request)
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
                        let method = CalculateUboBatchSvc(inner);
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
                "/ob_poc.ubo.UboService/CalculateUboStream" => {
                    #[allow(non_camel_case_types)]
                    struct CalculateUboStreamSvc<T: UboService>(pub Arc<T>);
                    impl<
                        T: UboService,
                    > tonic::server::StreamingService<super::CalculateUboStreamRequest>
                    for CalculateUboStreamSvc<T> {
                        type Response = super::CalculateUboStreamResponse;
                        type ResponseStream = T::CalculateUboStreamStream;
                        type Future = BoxFuture<
                            tonic::Response<Self::ResponseStream>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<
                                tonic::Streaming<super::CalculateUboStreamRequest>,
                            >,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as UboService>::calculate_ubo_stream(&inner, request)
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
                        let method = CalculateUboStreamSvc(inner);
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
                "/ob_poc.ubo.UboService/GetUboHistory" => {
                    #[allow(non_camel_case_types)]
                    struct GetUboHistorySvc<T: UboService>(pub Arc<T>);
                    impl<
                        T: UboService,
                    > tonic::server::UnaryService<super::GetUboHistoryRequest>
                    for GetUboHistorySvc<T> {
                        type Response = super::GetUboHistoryResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetUboHistoryRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as UboService>::get_ubo_history(&inner, request).await
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
                        let method = GetUboHistorySvc(inner);
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
                "/ob_poc.ubo.UboService/AnalyzeOwnershipStructure" => {
                    #[allow(non_camel_case_types)]
                    struct AnalyzeOwnershipStructureSvc<T: UboService>(pub Arc<T>);
                    impl<
                        T: UboService,
                    > tonic::server::UnaryService<super::AnalyzeOwnershipRequest>
                    for AnalyzeOwnershipStructureSvc<T> {
                        type Response = super::AnalyzeOwnershipResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::AnalyzeOwnershipRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as UboService>::analyze_ownership_structure(
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
                        let method = AnalyzeOwnershipStructureSvc(inner);
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
                "/ob_poc.ubo.UboService/ValidateUboRules" => {
                    #[allow(non_camel_case_types)]
                    struct ValidateUboRulesSvc<T: UboService>(pub Arc<T>);
                    impl<
                        T: UboService,
                    > tonic::server::UnaryService<super::ValidateUboRulesRequest>
                    for ValidateUboRulesSvc<T> {
                        type Response = super::ValidateUboRulesResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ValidateUboRulesRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as UboService>::validate_ubo_rules(&inner, request).await
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
                        let method = ValidateUboRulesSvc(inner);
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
                "/ob_poc.ubo.UboService/GetUboMetrics" => {
                    #[allow(non_camel_case_types)]
                    struct GetUboMetricsSvc<T: UboService>(pub Arc<T>);
                    impl<
                        T: UboService,
                    > tonic::server::UnaryService<super::GetUboMetricsRequest>
                    for GetUboMetricsSvc<T> {
                        type Response = super::GetUboMetricsResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetUboMetricsRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as UboService>::get_ubo_metrics(&inner, request).await
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
                        let method = GetUboMetricsSvc(inner);
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
                "/ob_poc.ubo.UboService/ExportUboResults" => {
                    #[allow(non_camel_case_types)]
                    struct ExportUboResultsSvc<T: UboService>(pub Arc<T>);
                    impl<
                        T: UboService,
                    > tonic::server::UnaryService<super::ExportUboResultsRequest>
                    for ExportUboResultsSvc<T> {
                        type Response = super::ExportUboResultsResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ExportUboResultsRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as UboService>::export_ubo_results(&inner, request).await
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
                        let method = ExportUboResultsSvc(inner);
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
                "/ob_poc.ubo.UboService/TraceUboCalculation" => {
                    #[allow(non_camel_case_types)]
                    struct TraceUboCalculationSvc<T: UboService>(pub Arc<T>);
                    impl<
                        T: UboService,
                    > tonic::server::UnaryService<super::TraceUboCalculationRequest>
                    for TraceUboCalculationSvc<T> {
                        type Response = super::TraceUboCalculationResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::TraceUboCalculationRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as UboService>::trace_ubo_calculation(&inner, request)
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
                        let method = TraceUboCalculationSvc(inner);
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
                "/ob_poc.ubo.UboService/UpdateUboAlgorithm" => {
                    #[allow(non_camel_case_types)]
                    struct UpdateUboAlgorithmSvc<T: UboService>(pub Arc<T>);
                    impl<
                        T: UboService,
                    > tonic::server::UnaryService<super::UpdateUboAlgorithmRequest>
                    for UpdateUboAlgorithmSvc<T> {
                        type Response = super::UpdateUboAlgorithmResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::UpdateUboAlgorithmRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as UboService>::update_ubo_algorithm(&inner, request)
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
                        let method = UpdateUboAlgorithmSvc(inner);
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
    impl<T: UboService> Clone for UboServiceServer<T> {
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
    impl<T: UboService> tonic::server::NamedService for UboServiceServer<T> {
        const NAME: &'static str = "ob_poc.ubo.UboService";
    }
}
