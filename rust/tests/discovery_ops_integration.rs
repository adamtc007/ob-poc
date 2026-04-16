//! Discovery ops integration smoke tests.
//!
//! Verifies runtime wiring (DSL executor -> plugin op -> core discovery/diagram logic)
//! for:
//! - `registry.discover-dsl`
//! - `schema.generate-discovery-map`
//!
//! Run with:
//! DATABASE_URL="postgresql:///data_designer" \
//!   RUSTC_WRAPPER= cargo test --features database --test discovery_ops_integration -- --ignored --nocapture

#[cfg(feature = "database")]
mod integration {
    use anyhow::Result;
    use ob_poc::dsl_v2::execution::{DslExecutor, ExecutionContext, ExecutionResult};
    use ob_poc::dsl_v2::{Argument, AstNode, Literal, Span, VerbCall};
    use sqlx::PgPool;

    fn s(value: &str) -> AstNode {
        AstNode::Literal(Literal::String(value.to_owned()), Span::default())
    }

    fn discover_call(utterance: &str) -> VerbCall {
        VerbCall {
            domain: "registry".to_owned(),
            verb: "discover-dsl".to_owned(),
            arguments: vec![
                Argument {
                    key: "utterance".to_owned(),
                    value: s(utterance),
                    span: Span::default(),
                },
                Argument {
                    key: "max-chain-length".to_owned(),
                    value: AstNode::Literal(Literal::Integer(5), Span::default()),
                    span: Span::default(),
                },
            ],
            binding: None,
            span: Span::default(),
        }
    }

    fn discovery_map_call() -> VerbCall {
        VerbCall {
            domain: "schema".to_owned(),
            verb: "generate-discovery-map".to_owned(),
            arguments: vec![Argument {
                key: "format".to_owned(),
                value: s("mermaid"),
                span: Span::default(),
            }],
            binding: None,
            span: Span::default(),
        }
    }

    async fn pool() -> Result<PgPool> {
        let url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql:///data_designer".to_owned());
        Ok(PgPool::connect(&url).await?)
    }

    #[tokio::test]
    #[ignore]
    async fn smoke_registry_discover_dsl() -> Result<()> {
        let pool = pool().await?;
        let exec = DslExecutor::new(pool);
        let mut ctx = ExecutionContext::new();

        let result = exec
            .execute_verb(&discover_call("set up depositary"), &mut ctx)
            .await?;

        let record = match result {
            ExecutionResult::Record(v) => v,
            other => anyhow::bail!("expected Record result, got: {other:?}"),
        };

        assert!(record.get("intent_matches").is_some());
        assert!(record.get("suggested_sequence").is_some());
        assert!(record.get("disambiguation_needed").is_some());
        assert!(record.get("governance_context").is_some());
        assert!(
            record.get("status").is_none(),
            "stub payload still returned"
        );

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn smoke_schema_generate_discovery_map() -> Result<()> {
        let pool = pool().await?;
        let exec = DslExecutor::new(pool);
        let mut ctx = ExecutionContext::new();

        let result = exec.execute_verb(&discovery_map_call(), &mut ctx).await?;

        let record = match result {
            ExecutionResult::Record(v) => v,
            other => anyhow::bail!("expected Record result, got: {other:?}"),
        };

        assert_eq!(
            record.get("format").and_then(|v| v.as_str()),
            Some("mermaid")
        );
        let diagram = record
            .get("diagram")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert!(diagram.starts_with("graph TD"));
        assert!(
            record.get("status").is_none(),
            "stub payload still returned"
        );

        Ok(())
    }
}
