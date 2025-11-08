//! Generated protobuf modules for gRPC services
//!
//! This module contains all the generated protobuf code for the gRPC interface
//! between Go and Rust components.

// Include generated protobuf code
pub mod ob_poc {
    pub mod dsl {
        include!("ob_poc.dsl.rs");
    }

    pub mod grammar {
        include!("ob_poc.grammar.rs");

        // Include tonic-generated service code
        pub mod tonic {
            include!("ob_poc.grammar.tonic.rs");
        }
    }

    pub mod parser {
        include!("ob_poc.parser.rs");

        pub mod tonic {
            include!("ob_poc.parser.tonic.rs");
        }
    }

    pub mod ubo {
        include!("ob_poc.ubo.rs");

        pub mod tonic {
            include!("ob_poc.ubo.tonic.rs");
        }
    }

    pub mod vocabulary {
        include!("ob_poc.vocabulary.rs");

        pub mod tonic {
            include!("ob_poc.vocabulary.tonic.rs");
        }
    }

    pub mod engine {
        include!("ob_poc.engine.rs");

        pub mod tonic {
            include!("ob_poc.engine.tonic.rs");
        }
    }
}

// Re-export commonly used types for convenience
pub use ob_poc::dsl::*;
pub use ob_poc::engine::tonic::dsl_engine_service_server::DslEngineServiceServer;
pub use ob_poc::grammar::tonic::grammar_service_server::GrammarServiceServer;
pub use ob_poc::parser::tonic::parser_service_server::ParserServiceServer;
pub use ob_poc::ubo::tonic::ubo_service_server::UboServiceServer;
pub use ob_poc::vocabulary::tonic::vocabulary_service_server::VocabularyServiceServer;
