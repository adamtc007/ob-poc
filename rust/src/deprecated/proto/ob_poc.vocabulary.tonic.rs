// @generated
/// Generated client implementations.
pub mod vocabulary_service_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct VocabularyServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl VocabularyServiceClient<tonic::transport::Channel> {
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
    impl<T> VocabularyServiceClient<T>
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
        ) -> VocabularyServiceClient<InterceptedService<T, F>>
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
            VocabularyServiceClient::new(InterceptedService::new(inner, interceptor))
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
        pub async fn register_verb(
            &mut self,
            request: impl tonic::IntoRequest<super::RegisterVerbRequest>,
        ) -> std::result::Result<
            tonic::Response<super::RegisterVerbResponse>,
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
                "/ob_poc.vocabulary.VocabularyService/RegisterVerb",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "ob_poc.vocabulary.VocabularyService",
                        "RegisterVerb",
                    ),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn update_verb(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateVerbRequest>,
        ) -> std::result::Result<
            tonic::Response<super::UpdateVerbResponse>,
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
                "/ob_poc.vocabulary.VocabularyService/UpdateVerb",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("ob_poc.vocabulary.VocabularyService", "UpdateVerb"),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn remove_verb(
            &mut self,
            request: impl tonic::IntoRequest<super::RemoveVerbRequest>,
        ) -> std::result::Result<
            tonic::Response<super::RemoveVerbResponse>,
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
                "/ob_poc.vocabulary.VocabularyService/RemoveVerb",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("ob_poc.vocabulary.VocabularyService", "RemoveVerb"),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_verb(
            &mut self,
            request: impl tonic::IntoRequest<super::GetVerbRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetVerbResponse>,
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
                "/ob_poc.vocabulary.VocabularyService/GetVerb",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("ob_poc.vocabulary.VocabularyService", "GetVerb"),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn list_verbs(
            &mut self,
            request: impl tonic::IntoRequest<super::ListVerbsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ListVerbsResponse>,
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
                "/ob_poc.vocabulary.VocabularyService/ListVerbs",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("ob_poc.vocabulary.VocabularyService", "ListVerbs"),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn search_verbs(
            &mut self,
            request: impl tonic::IntoRequest<super::SearchVerbsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::SearchVerbsResponse>,
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
                "/ob_poc.vocabulary.VocabularyService/SearchVerbs",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("ob_poc.vocabulary.VocabularyService", "SearchVerbs"),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn validate_verb_usage(
            &mut self,
            request: impl tonic::IntoRequest<super::ValidateVerbUsageRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ValidateVerbUsageResponse>,
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
                "/ob_poc.vocabulary.VocabularyService/ValidateVerbUsage",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "ob_poc.vocabulary.VocabularyService",
                        "ValidateVerbUsage",
                    ),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn register_domain(
            &mut self,
            request: impl tonic::IntoRequest<super::RegisterDomainRequest>,
        ) -> std::result::Result<
            tonic::Response<super::RegisterDomainResponse>,
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
                "/ob_poc.vocabulary.VocabularyService/RegisterDomain",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "ob_poc.vocabulary.VocabularyService",
                        "RegisterDomain",
                    ),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn list_domains(
            &mut self,
            request: impl tonic::IntoRequest<super::ListDomainsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ListDomainsResponse>,
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
                "/ob_poc.vocabulary.VocabularyService/ListDomains",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("ob_poc.vocabulary.VocabularyService", "ListDomains"),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_domain_info(
            &mut self,
            request: impl tonic::IntoRequest<super::GetDomainInfoRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetDomainInfoResponse>,
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
                "/ob_poc.vocabulary.VocabularyService/GetDomainInfo",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "ob_poc.vocabulary.VocabularyService",
                        "GetDomainInfo",
                    ),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn migrate_verbs(
            &mut self,
            request: impl tonic::IntoRequest<super::MigrateVerbsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::MigrateVerbsResponse>,
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
                "/ob_poc.vocabulary.VocabularyService/MigrateVerbs",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "ob_poc.vocabulary.VocabularyService",
                        "MigrateVerbs",
                    ),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn export_vocabulary(
            &mut self,
            request: impl tonic::IntoRequest<super::ExportVocabularyRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ExportVocabularyResponse>,
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
                "/ob_poc.vocabulary.VocabularyService/ExportVocabulary",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "ob_poc.vocabulary.VocabularyService",
                        "ExportVocabulary",
                    ),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn import_vocabulary(
            &mut self,
            request: impl tonic::IntoRequest<super::ImportVocabularyRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ImportVocabularyResponse>,
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
                "/ob_poc.vocabulary.VocabularyService/ImportVocabulary",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "ob_poc.vocabulary.VocabularyService",
                        "ImportVocabulary",
                    ),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_vocabulary_stats(
            &mut self,
            request: impl tonic::IntoRequest<super::GetVocabularyStatsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetVocabularyStatsResponse>,
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
                "/ob_poc.vocabulary.VocabularyService/GetVocabularyStats",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "ob_poc.vocabulary.VocabularyService",
                        "GetVocabularyStats",
                    ),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn validate_vocabulary_consistency(
            &mut self,
            request: impl tonic::IntoRequest<super::ValidateVocabularyConsistencyRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ValidateVocabularyConsistencyResponse>,
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
                "/ob_poc.vocabulary.VocabularyService/ValidateVocabularyConsistency",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "ob_poc.vocabulary.VocabularyService",
                        "ValidateVocabularyConsistency",
                    ),
                );
            self.inner.unary(req, path, codec).await
        }
    }
}
/// Generated server implementations.
pub mod vocabulary_service_server {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    /// Generated trait containing gRPC methods that should be implemented for use with VocabularyServiceServer.
    #[async_trait]
    pub trait VocabularyService: Send + Sync + 'static {
        async fn register_verb(
            &self,
            request: tonic::Request<super::RegisterVerbRequest>,
        ) -> std::result::Result<
            tonic::Response<super::RegisterVerbResponse>,
            tonic::Status,
        >;
        async fn update_verb(
            &self,
            request: tonic::Request<super::UpdateVerbRequest>,
        ) -> std::result::Result<
            tonic::Response<super::UpdateVerbResponse>,
            tonic::Status,
        >;
        async fn remove_verb(
            &self,
            request: tonic::Request<super::RemoveVerbRequest>,
        ) -> std::result::Result<
            tonic::Response<super::RemoveVerbResponse>,
            tonic::Status,
        >;
        async fn get_verb(
            &self,
            request: tonic::Request<super::GetVerbRequest>,
        ) -> std::result::Result<tonic::Response<super::GetVerbResponse>, tonic::Status>;
        async fn list_verbs(
            &self,
            request: tonic::Request<super::ListVerbsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ListVerbsResponse>,
            tonic::Status,
        >;
        async fn search_verbs(
            &self,
            request: tonic::Request<super::SearchVerbsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::SearchVerbsResponse>,
            tonic::Status,
        >;
        async fn validate_verb_usage(
            &self,
            request: tonic::Request<super::ValidateVerbUsageRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ValidateVerbUsageResponse>,
            tonic::Status,
        >;
        async fn register_domain(
            &self,
            request: tonic::Request<super::RegisterDomainRequest>,
        ) -> std::result::Result<
            tonic::Response<super::RegisterDomainResponse>,
            tonic::Status,
        >;
        async fn list_domains(
            &self,
            request: tonic::Request<super::ListDomainsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ListDomainsResponse>,
            tonic::Status,
        >;
        async fn get_domain_info(
            &self,
            request: tonic::Request<super::GetDomainInfoRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetDomainInfoResponse>,
            tonic::Status,
        >;
        async fn migrate_verbs(
            &self,
            request: tonic::Request<super::MigrateVerbsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::MigrateVerbsResponse>,
            tonic::Status,
        >;
        async fn export_vocabulary(
            &self,
            request: tonic::Request<super::ExportVocabularyRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ExportVocabularyResponse>,
            tonic::Status,
        >;
        async fn import_vocabulary(
            &self,
            request: tonic::Request<super::ImportVocabularyRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ImportVocabularyResponse>,
            tonic::Status,
        >;
        async fn get_vocabulary_stats(
            &self,
            request: tonic::Request<super::GetVocabularyStatsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetVocabularyStatsResponse>,
            tonic::Status,
        >;
        async fn validate_vocabulary_consistency(
            &self,
            request: tonic::Request<super::ValidateVocabularyConsistencyRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ValidateVocabularyConsistencyResponse>,
            tonic::Status,
        >;
    }
    #[derive(Debug)]
    pub struct VocabularyServiceServer<T: VocabularyService> {
        inner: Arc<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
        max_decoding_message_size: Option<usize>,
        max_encoding_message_size: Option<usize>,
    }
    impl<T: VocabularyService> VocabularyServiceServer<T> {
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
    impl<T, B> tonic::codegen::Service<http::Request<B>> for VocabularyServiceServer<T>
    where
        T: VocabularyService,
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
                "/ob_poc.vocabulary.VocabularyService/RegisterVerb" => {
                    #[allow(non_camel_case_types)]
                    struct RegisterVerbSvc<T: VocabularyService>(pub Arc<T>);
                    impl<
                        T: VocabularyService,
                    > tonic::server::UnaryService<super::RegisterVerbRequest>
                    for RegisterVerbSvc<T> {
                        type Response = super::RegisterVerbResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::RegisterVerbRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as VocabularyService>::register_verb(&inner, request)
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
                        let method = RegisterVerbSvc(inner);
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
                "/ob_poc.vocabulary.VocabularyService/UpdateVerb" => {
                    #[allow(non_camel_case_types)]
                    struct UpdateVerbSvc<T: VocabularyService>(pub Arc<T>);
                    impl<
                        T: VocabularyService,
                    > tonic::server::UnaryService<super::UpdateVerbRequest>
                    for UpdateVerbSvc<T> {
                        type Response = super::UpdateVerbResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::UpdateVerbRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as VocabularyService>::update_verb(&inner, request).await
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
                        let method = UpdateVerbSvc(inner);
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
                "/ob_poc.vocabulary.VocabularyService/RemoveVerb" => {
                    #[allow(non_camel_case_types)]
                    struct RemoveVerbSvc<T: VocabularyService>(pub Arc<T>);
                    impl<
                        T: VocabularyService,
                    > tonic::server::UnaryService<super::RemoveVerbRequest>
                    for RemoveVerbSvc<T> {
                        type Response = super::RemoveVerbResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::RemoveVerbRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as VocabularyService>::remove_verb(&inner, request).await
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
                        let method = RemoveVerbSvc(inner);
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
                "/ob_poc.vocabulary.VocabularyService/GetVerb" => {
                    #[allow(non_camel_case_types)]
                    struct GetVerbSvc<T: VocabularyService>(pub Arc<T>);
                    impl<
                        T: VocabularyService,
                    > tonic::server::UnaryService<super::GetVerbRequest>
                    for GetVerbSvc<T> {
                        type Response = super::GetVerbResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetVerbRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as VocabularyService>::get_verb(&inner, request).await
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
                        let method = GetVerbSvc(inner);
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
                "/ob_poc.vocabulary.VocabularyService/ListVerbs" => {
                    #[allow(non_camel_case_types)]
                    struct ListVerbsSvc<T: VocabularyService>(pub Arc<T>);
                    impl<
                        T: VocabularyService,
                    > tonic::server::UnaryService<super::ListVerbsRequest>
                    for ListVerbsSvc<T> {
                        type Response = super::ListVerbsResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ListVerbsRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as VocabularyService>::list_verbs(&inner, request).await
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
                        let method = ListVerbsSvc(inner);
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
                "/ob_poc.vocabulary.VocabularyService/SearchVerbs" => {
                    #[allow(non_camel_case_types)]
                    struct SearchVerbsSvc<T: VocabularyService>(pub Arc<T>);
                    impl<
                        T: VocabularyService,
                    > tonic::server::UnaryService<super::SearchVerbsRequest>
                    for SearchVerbsSvc<T> {
                        type Response = super::SearchVerbsResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::SearchVerbsRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as VocabularyService>::search_verbs(&inner, request)
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
                        let method = SearchVerbsSvc(inner);
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
                "/ob_poc.vocabulary.VocabularyService/ValidateVerbUsage" => {
                    #[allow(non_camel_case_types)]
                    struct ValidateVerbUsageSvc<T: VocabularyService>(pub Arc<T>);
                    impl<
                        T: VocabularyService,
                    > tonic::server::UnaryService<super::ValidateVerbUsageRequest>
                    for ValidateVerbUsageSvc<T> {
                        type Response = super::ValidateVerbUsageResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ValidateVerbUsageRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as VocabularyService>::validate_verb_usage(
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
                        let method = ValidateVerbUsageSvc(inner);
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
                "/ob_poc.vocabulary.VocabularyService/RegisterDomain" => {
                    #[allow(non_camel_case_types)]
                    struct RegisterDomainSvc<T: VocabularyService>(pub Arc<T>);
                    impl<
                        T: VocabularyService,
                    > tonic::server::UnaryService<super::RegisterDomainRequest>
                    for RegisterDomainSvc<T> {
                        type Response = super::RegisterDomainResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::RegisterDomainRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as VocabularyService>::register_domain(&inner, request)
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
                        let method = RegisterDomainSvc(inner);
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
                "/ob_poc.vocabulary.VocabularyService/ListDomains" => {
                    #[allow(non_camel_case_types)]
                    struct ListDomainsSvc<T: VocabularyService>(pub Arc<T>);
                    impl<
                        T: VocabularyService,
                    > tonic::server::UnaryService<super::ListDomainsRequest>
                    for ListDomainsSvc<T> {
                        type Response = super::ListDomainsResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ListDomainsRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as VocabularyService>::list_domains(&inner, request)
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
                        let method = ListDomainsSvc(inner);
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
                "/ob_poc.vocabulary.VocabularyService/GetDomainInfo" => {
                    #[allow(non_camel_case_types)]
                    struct GetDomainInfoSvc<T: VocabularyService>(pub Arc<T>);
                    impl<
                        T: VocabularyService,
                    > tonic::server::UnaryService<super::GetDomainInfoRequest>
                    for GetDomainInfoSvc<T> {
                        type Response = super::GetDomainInfoResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetDomainInfoRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as VocabularyService>::get_domain_info(&inner, request)
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
                        let method = GetDomainInfoSvc(inner);
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
                "/ob_poc.vocabulary.VocabularyService/MigrateVerbs" => {
                    #[allow(non_camel_case_types)]
                    struct MigrateVerbsSvc<T: VocabularyService>(pub Arc<T>);
                    impl<
                        T: VocabularyService,
                    > tonic::server::UnaryService<super::MigrateVerbsRequest>
                    for MigrateVerbsSvc<T> {
                        type Response = super::MigrateVerbsResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::MigrateVerbsRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as VocabularyService>::migrate_verbs(&inner, request)
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
                        let method = MigrateVerbsSvc(inner);
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
                "/ob_poc.vocabulary.VocabularyService/ExportVocabulary" => {
                    #[allow(non_camel_case_types)]
                    struct ExportVocabularySvc<T: VocabularyService>(pub Arc<T>);
                    impl<
                        T: VocabularyService,
                    > tonic::server::UnaryService<super::ExportVocabularyRequest>
                    for ExportVocabularySvc<T> {
                        type Response = super::ExportVocabularyResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ExportVocabularyRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as VocabularyService>::export_vocabulary(&inner, request)
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
                        let method = ExportVocabularySvc(inner);
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
                "/ob_poc.vocabulary.VocabularyService/ImportVocabulary" => {
                    #[allow(non_camel_case_types)]
                    struct ImportVocabularySvc<T: VocabularyService>(pub Arc<T>);
                    impl<
                        T: VocabularyService,
                    > tonic::server::UnaryService<super::ImportVocabularyRequest>
                    for ImportVocabularySvc<T> {
                        type Response = super::ImportVocabularyResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ImportVocabularyRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as VocabularyService>::import_vocabulary(&inner, request)
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
                        let method = ImportVocabularySvc(inner);
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
                "/ob_poc.vocabulary.VocabularyService/GetVocabularyStats" => {
                    #[allow(non_camel_case_types)]
                    struct GetVocabularyStatsSvc<T: VocabularyService>(pub Arc<T>);
                    impl<
                        T: VocabularyService,
                    > tonic::server::UnaryService<super::GetVocabularyStatsRequest>
                    for GetVocabularyStatsSvc<T> {
                        type Response = super::GetVocabularyStatsResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetVocabularyStatsRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as VocabularyService>::get_vocabulary_stats(
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
                        let method = GetVocabularyStatsSvc(inner);
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
                "/ob_poc.vocabulary.VocabularyService/ValidateVocabularyConsistency" => {
                    #[allow(non_camel_case_types)]
                    struct ValidateVocabularyConsistencySvc<T: VocabularyService>(
                        pub Arc<T>,
                    );
                    impl<
                        T: VocabularyService,
                    > tonic::server::UnaryService<
                        super::ValidateVocabularyConsistencyRequest,
                    > for ValidateVocabularyConsistencySvc<T> {
                        type Response = super::ValidateVocabularyConsistencyResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<
                                super::ValidateVocabularyConsistencyRequest,
                            >,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as VocabularyService>::validate_vocabulary_consistency(
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
                        let method = ValidateVocabularyConsistencySvc(inner);
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
    impl<T: VocabularyService> Clone for VocabularyServiceServer<T> {
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
    impl<T: VocabularyService> tonic::server::NamedService
    for VocabularyServiceServer<T> {
        const NAME: &'static str = "ob_poc.vocabulary.VocabularyService";
    }
}
