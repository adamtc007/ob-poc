// @generated
/// Generated client implementations.
pub mod grammar_service_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct GrammarServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl GrammarServiceClient<tonic::transport::Channel> {
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
    impl<T> GrammarServiceClient<T>
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
        ) -> GrammarServiceClient<InterceptedService<T, F>>
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
            GrammarServiceClient::new(InterceptedService::new(inner, interceptor))
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
        pub async fn parse_grammar(
            &mut self,
            request: impl tonic::IntoRequest<super::ParseGrammarRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ParseGrammarResponse>,
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
                "/ob_poc.grammar.GrammarService/ParseGrammar",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("ob_poc.grammar.GrammarService", "ParseGrammar"),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn validate_grammar(
            &mut self,
            request: impl tonic::IntoRequest<super::ValidateGrammarRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ValidateGrammarResponse>,
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
                "/ob_poc.grammar.GrammarService/ValidateGrammar",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("ob_poc.grammar.GrammarService", "ValidateGrammar"),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn load_grammar(
            &mut self,
            request: impl tonic::IntoRequest<super::LoadGrammarRequest>,
        ) -> std::result::Result<
            tonic::Response<super::LoadGrammarResponse>,
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
                "/ob_poc.grammar.GrammarService/LoadGrammar",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("ob_poc.grammar.GrammarService", "LoadGrammar"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_grammar_info(
            &mut self,
            request: impl tonic::IntoRequest<super::GetGrammarInfoRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetGrammarInfoResponse>,
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
                "/ob_poc.grammar.GrammarService/GetGrammarInfo",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("ob_poc.grammar.GrammarService", "GetGrammarInfo"),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn list_grammars(
            &mut self,
            request: impl tonic::IntoRequest<super::ListGrammarsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ListGrammarsResponse>,
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
                "/ob_poc.grammar.GrammarService/ListGrammars",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("ob_poc.grammar.GrammarService", "ListGrammars"),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn set_active_grammar(
            &mut self,
            request: impl tonic::IntoRequest<super::SetActiveGrammarRequest>,
        ) -> std::result::Result<
            tonic::Response<super::SetActiveGrammarResponse>,
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
                "/ob_poc.grammar.GrammarService/SetActiveGrammar",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("ob_poc.grammar.GrammarService", "SetActiveGrammar"),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_grammar_summary(
            &mut self,
            request: impl tonic::IntoRequest<super::GetGrammarSummaryRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetGrammarSummaryResponse>,
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
                "/ob_poc.grammar.GrammarService/GetGrammarSummary",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("ob_poc.grammar.GrammarService", "GetGrammarSummary"),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn export_grammar(
            &mut self,
            request: impl tonic::IntoRequest<super::ExportGrammarRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ExportGrammarResponse>,
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
                "/ob_poc.grammar.GrammarService/ExportGrammar",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("ob_poc.grammar.GrammarService", "ExportGrammar"),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn analyze_grammar(
            &mut self,
            request: impl tonic::IntoRequest<super::AnalyzeGrammarRequest>,
        ) -> std::result::Result<
            tonic::Response<super::AnalyzeGrammarResponse>,
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
                "/ob_poc.grammar.GrammarService/AnalyzeGrammar",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("ob_poc.grammar.GrammarService", "AnalyzeGrammar"),
                );
            self.inner.unary(req, path, codec).await
        }
    }
}
/// Generated server implementations.
pub mod grammar_service_server {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    /// Generated trait containing gRPC methods that should be implemented for use with GrammarServiceServer.
    #[async_trait]
    pub trait GrammarService: Send + Sync + 'static {
        async fn parse_grammar(
            &self,
            request: tonic::Request<super::ParseGrammarRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ParseGrammarResponse>,
            tonic::Status,
        >;
        async fn validate_grammar(
            &self,
            request: tonic::Request<super::ValidateGrammarRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ValidateGrammarResponse>,
            tonic::Status,
        >;
        async fn load_grammar(
            &self,
            request: tonic::Request<super::LoadGrammarRequest>,
        ) -> std::result::Result<
            tonic::Response<super::LoadGrammarResponse>,
            tonic::Status,
        >;
        async fn get_grammar_info(
            &self,
            request: tonic::Request<super::GetGrammarInfoRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetGrammarInfoResponse>,
            tonic::Status,
        >;
        async fn list_grammars(
            &self,
            request: tonic::Request<super::ListGrammarsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ListGrammarsResponse>,
            tonic::Status,
        >;
        async fn set_active_grammar(
            &self,
            request: tonic::Request<super::SetActiveGrammarRequest>,
        ) -> std::result::Result<
            tonic::Response<super::SetActiveGrammarResponse>,
            tonic::Status,
        >;
        async fn get_grammar_summary(
            &self,
            request: tonic::Request<super::GetGrammarSummaryRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetGrammarSummaryResponse>,
            tonic::Status,
        >;
        async fn export_grammar(
            &self,
            request: tonic::Request<super::ExportGrammarRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ExportGrammarResponse>,
            tonic::Status,
        >;
        async fn analyze_grammar(
            &self,
            request: tonic::Request<super::AnalyzeGrammarRequest>,
        ) -> std::result::Result<
            tonic::Response<super::AnalyzeGrammarResponse>,
            tonic::Status,
        >;
    }
    #[derive(Debug)]
    pub struct GrammarServiceServer<T: GrammarService> {
        inner: Arc<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
        max_decoding_message_size: Option<usize>,
        max_encoding_message_size: Option<usize>,
    }
    impl<T: GrammarService> GrammarServiceServer<T> {
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
    impl<T, B> tonic::codegen::Service<http::Request<B>> for GrammarServiceServer<T>
    where
        T: GrammarService,
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
                "/ob_poc.grammar.GrammarService/ParseGrammar" => {
                    #[allow(non_camel_case_types)]
                    struct ParseGrammarSvc<T: GrammarService>(pub Arc<T>);
                    impl<
                        T: GrammarService,
                    > tonic::server::UnaryService<super::ParseGrammarRequest>
                    for ParseGrammarSvc<T> {
                        type Response = super::ParseGrammarResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ParseGrammarRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as GrammarService>::parse_grammar(&inner, request).await
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
                        let method = ParseGrammarSvc(inner);
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
                "/ob_poc.grammar.GrammarService/ValidateGrammar" => {
                    #[allow(non_camel_case_types)]
                    struct ValidateGrammarSvc<T: GrammarService>(pub Arc<T>);
                    impl<
                        T: GrammarService,
                    > tonic::server::UnaryService<super::ValidateGrammarRequest>
                    for ValidateGrammarSvc<T> {
                        type Response = super::ValidateGrammarResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ValidateGrammarRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as GrammarService>::validate_grammar(&inner, request)
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
                        let method = ValidateGrammarSvc(inner);
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
                "/ob_poc.grammar.GrammarService/LoadGrammar" => {
                    #[allow(non_camel_case_types)]
                    struct LoadGrammarSvc<T: GrammarService>(pub Arc<T>);
                    impl<
                        T: GrammarService,
                    > tonic::server::UnaryService<super::LoadGrammarRequest>
                    for LoadGrammarSvc<T> {
                        type Response = super::LoadGrammarResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::LoadGrammarRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as GrammarService>::load_grammar(&inner, request).await
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
                        let method = LoadGrammarSvc(inner);
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
                "/ob_poc.grammar.GrammarService/GetGrammarInfo" => {
                    #[allow(non_camel_case_types)]
                    struct GetGrammarInfoSvc<T: GrammarService>(pub Arc<T>);
                    impl<
                        T: GrammarService,
                    > tonic::server::UnaryService<super::GetGrammarInfoRequest>
                    for GetGrammarInfoSvc<T> {
                        type Response = super::GetGrammarInfoResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetGrammarInfoRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as GrammarService>::get_grammar_info(&inner, request)
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
                        let method = GetGrammarInfoSvc(inner);
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
                "/ob_poc.grammar.GrammarService/ListGrammars" => {
                    #[allow(non_camel_case_types)]
                    struct ListGrammarsSvc<T: GrammarService>(pub Arc<T>);
                    impl<
                        T: GrammarService,
                    > tonic::server::UnaryService<super::ListGrammarsRequest>
                    for ListGrammarsSvc<T> {
                        type Response = super::ListGrammarsResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ListGrammarsRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as GrammarService>::list_grammars(&inner, request).await
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
                        let method = ListGrammarsSvc(inner);
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
                "/ob_poc.grammar.GrammarService/SetActiveGrammar" => {
                    #[allow(non_camel_case_types)]
                    struct SetActiveGrammarSvc<T: GrammarService>(pub Arc<T>);
                    impl<
                        T: GrammarService,
                    > tonic::server::UnaryService<super::SetActiveGrammarRequest>
                    for SetActiveGrammarSvc<T> {
                        type Response = super::SetActiveGrammarResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::SetActiveGrammarRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as GrammarService>::set_active_grammar(&inner, request)
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
                        let method = SetActiveGrammarSvc(inner);
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
                "/ob_poc.grammar.GrammarService/GetGrammarSummary" => {
                    #[allow(non_camel_case_types)]
                    struct GetGrammarSummarySvc<T: GrammarService>(pub Arc<T>);
                    impl<
                        T: GrammarService,
                    > tonic::server::UnaryService<super::GetGrammarSummaryRequest>
                    for GetGrammarSummarySvc<T> {
                        type Response = super::GetGrammarSummaryResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetGrammarSummaryRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as GrammarService>::get_grammar_summary(&inner, request)
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
                        let method = GetGrammarSummarySvc(inner);
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
                "/ob_poc.grammar.GrammarService/ExportGrammar" => {
                    #[allow(non_camel_case_types)]
                    struct ExportGrammarSvc<T: GrammarService>(pub Arc<T>);
                    impl<
                        T: GrammarService,
                    > tonic::server::UnaryService<super::ExportGrammarRequest>
                    for ExportGrammarSvc<T> {
                        type Response = super::ExportGrammarResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ExportGrammarRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as GrammarService>::export_grammar(&inner, request).await
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
                        let method = ExportGrammarSvc(inner);
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
                "/ob_poc.grammar.GrammarService/AnalyzeGrammar" => {
                    #[allow(non_camel_case_types)]
                    struct AnalyzeGrammarSvc<T: GrammarService>(pub Arc<T>);
                    impl<
                        T: GrammarService,
                    > tonic::server::UnaryService<super::AnalyzeGrammarRequest>
                    for AnalyzeGrammarSvc<T> {
                        type Response = super::AnalyzeGrammarResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::AnalyzeGrammarRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as GrammarService>::analyze_grammar(&inner, request)
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
                        let method = AnalyzeGrammarSvc(inner);
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
    impl<T: GrammarService> Clone for GrammarServiceServer<T> {
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
    impl<T: GrammarService> tonic::server::NamedService for GrammarServiceServer<T> {
        const NAME: &'static str = "ob_poc.grammar.GrammarService";
    }
}
