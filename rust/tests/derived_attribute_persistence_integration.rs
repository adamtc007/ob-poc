//! Integration coverage for canonical derived attribute persistence.
//!
//! Run with:
//! DATABASE_URL="postgresql:///data_designer" \
//!   RUSTC_WRAPPER= cargo test --features database --test derived_attribute_persistence_integration -- --ignored --nocapture

#[cfg(feature = "database")]
mod integration {
    use anyhow::{Context, Result};
    use ob_poc::sem_reg::derivation_spec::{
        DerivationExpression, DerivationInput, DerivationSpecBody, NullSemantics,
        SecurityInheritanceMode,
    };
    use ob_poc::sem_reg::ids::object_id_for;
    use ob_poc::sem_reg::types::ObjectType;
    use ob_poc::service_resources::{PopulationEngine, ServiceResourcePipelineService};
    use sem_os_core::types::EvidenceGrade;
    use serde_json::json;
    use sqlx::PgPool;
    use uuid::Uuid;

    struct TestDb {
        pool: PgPool,
        prefix: String,
    }

    impl TestDb {
        async fn new() -> Result<Self> {
            let url = std::env::var("TEST_DATABASE_URL")
                .or_else(|_| std::env::var("DATABASE_URL"))
                .unwrap_or_else(|_| "postgresql:///data_designer".into());
            Ok(Self {
                pool: PgPool::connect(&url).await?,
                prefix: format!("derived_test_{}", &Uuid::new_v4().to_string()[..8]),
            })
        }

        fn name(&self, base: &str) -> String {
            format!("{}_{}", self.prefix, base)
        }

        async fn cleanup(
            &self,
            attr_ids: &[Uuid],
            entity_ids: &[Uuid],
            cbu_ids: &[Uuid],
            snapshot_object_ids: &[Uuid],
        ) -> Result<()> {
            sqlx::query(
                r#"DELETE FROM "ob-poc".derived_attribute_dependencies
                   WHERE derived_value_id IN (
                       SELECT id FROM "ob-poc".derived_attribute_values
                       WHERE attr_id = ANY($1)
                          OR entity_id = ANY($2)
                   )"#,
            )
            .bind(attr_ids)
            .bind(entity_ids)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM "ob-poc".derived_attribute_values
                   WHERE attr_id = ANY($1) OR entity_id = ANY($2)"#,
            )
            .bind(attr_ids)
            .bind(entity_ids)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM "ob-poc".cbu_attr_values
                   WHERE attr_id = ANY($1) OR cbu_id = ANY($2)"#,
            )
            .bind(attr_ids)
            .bind(cbu_ids)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM "ob-poc".attribute_values_typed
                   WHERE attribute_uuid = ANY($1) OR entity_id = ANY($2)"#,
            )
            .bind(attr_ids)
            .bind(entity_ids)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(r#"DELETE FROM sem_reg.snapshots WHERE object_id = ANY($1)"#)
                .bind(snapshot_object_ids)
                .execute(&self.pool)
                .await
                .ok();

            sqlx::query(r#"DELETE FROM "ob-poc".attribute_registry WHERE uuid = ANY($1)"#)
                .bind(attr_ids)
                .execute(&self.pool)
                .await
                .ok();

            sqlx::query(r#"DELETE FROM "ob-poc".entities WHERE entity_id = ANY($1)"#)
                .bind(entity_ids)
                .execute(&self.pool)
                .await
                .ok();

            sqlx::query(r#"DELETE FROM "ob-poc".cbus WHERE cbu_id = ANY($1)"#)
                .bind(cbu_ids)
                .execute(&self.pool)
                .await
                .ok();

            Ok(())
        }
    }

    async fn existing_entity_type_id(pool: &PgPool, codes: &[&str]) -> Result<(Uuid, String)> {
        for code in codes {
            let row: Option<(Uuid, String)> = sqlx::query_as(
                r#"
                SELECT entity_type_id, lower(COALESCE(type_code, name))
                FROM "ob-poc".entity_types
                WHERE lower(COALESCE(type_code, '')) = $1
                   OR lower(name) = $1
                ORDER BY entity_type_id
                LIMIT 1
                "#,
            )
            .bind(code.to_ascii_lowercase())
            .fetch_optional(pool)
            .await?;

            if let Some(row) = row {
                return Ok(row);
            }
        }

        anyhow::bail!("none of the entity types {:?} were found", codes)
    }

    async fn insert_attr_registry(
        pool: &PgPool,
        uuid: Uuid,
        id: &str,
        category: &str,
        value_type: &str,
        is_derived: bool,
        derivation_spec_fqn: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".attribute_registry (
                id, display_name, category, value_type, uuid, domain, is_derived, derivation_spec_fqn
            ) VALUES ($1, $2, $3, $4, $5, 'test', $6, $7)
            "#,
        )
        .bind(id)
        .bind(id)
        .bind(category)
        .bind(value_type)
        .bind(uuid)
        .bind(is_derived)
        .bind(derivation_spec_fqn)
        .execute(pool)
        .await?;
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn cbu_reads_prefer_canonical_projection_for_derived_values() -> Result<()> {
        let db = TestDb::new().await?;
        let cbu_id = Uuid::new_v4();
        let legacy_attr_id = Uuid::new_v4();
        let derived_attr_id = Uuid::new_v4();
        let derived_row_id = Uuid::new_v4();
        let spec_snapshot_id = Uuid::new_v4();
        let legacy_attr_code = db.name("legacy_passthrough");
        let derived_attr_code = format!("{}_value", db.name("canonical_projection_flag"));
        let derived_spec_fqn = db.name("canonical_projection_flag");

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbus (cbu_id, name, jurisdiction, client_type)
            VALUES ($1, $2, 'LU', 'INTERNAL_TEST')
            "#,
        )
        .bind(cbu_id)
        .bind(db.name("cbu"))
        .execute(&db.pool)
        .await?;

        insert_attr_registry(
            &db.pool,
            legacy_attr_id,
            &legacy_attr_code,
            "entity",
            "string",
            false,
            None,
        )
        .await?;
        insert_attr_registry(
            &db.pool,
            derived_attr_id,
            &derived_attr_code,
            "risk",
            "boolean",
            true,
            Some(&derived_spec_fqn),
        )
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbu_unified_attr_requirements (
                cbu_id, attr_id, requirement_strength, merged_constraints, required_by_srdefs
            )
            VALUES
                ($1, $2, 'required', '{}'::jsonb, '[]'::jsonb),
                ($1, $3, 'required', '{}'::jsonb, '[]'::jsonb)
            "#,
        )
        .bind(cbu_id)
        .bind(legacy_attr_id)
        .bind(derived_attr_id)
        .execute(&db.pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbu_attr_values
                (cbu_id, attr_id, value, source, evidence_refs, explain_refs, as_of)
            VALUES
                ($1, $2, '"legacy"'::jsonb, 'manual', '[]'::jsonb, '[]'::jsonb, NOW()),
                ($1, $3, 'false'::jsonb, 'derived', '[]'::jsonb, '[]'::jsonb, NOW())
            "#,
        )
        .bind(cbu_id)
        .bind(legacy_attr_id)
        .bind(derived_attr_id)
        .execute(&db.pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".derived_attribute_values (
                id, attr_id, entity_id, entity_type, value, derivation_spec_fqn,
                spec_snapshot_id, content_hash, input_values, inherited_security_label,
                dependency_depth, evaluated_at, stale
            ) VALUES (
                $1, $2, $3, 'cbu', 'true'::jsonb, $4,
                $5, 'hash', '{}'::jsonb, '{}'::jsonb, 0, NOW(), false
            )
            "#,
        )
        .bind(derived_row_id)
        .bind(derived_attr_id)
        .bind(cbu_id)
        .bind(&derived_spec_fqn)
        .bind(spec_snapshot_id)
        .execute(&db.pool)
        .await?;

        let service = ServiceResourcePipelineService::new(db.pool.clone());
        let values = service.get_cbu_attr_values(cbu_id).await?;
        let derived = values
            .iter()
            .find(|row| row.attr_id == derived_attr_id)
            .context("missing derived row from effective CBU read")?;
        let legacy = values
            .iter()
            .find(|row| row.attr_id == legacy_attr_id)
            .context("missing legacy non-derived row from effective CBU read")?;

        assert_eq!(derived.source, "derived");
        assert_eq!(derived.value, json!(true));
        assert_eq!(legacy.value, json!("legacy"));

        let gap_row: (bool, Option<String>) = sqlx::query_as(
            r#"
            SELECT has_value, value_source
            FROM "ob-poc".v_cbu_attr_gaps
            WHERE cbu_id = $1 AND attr_id = $2
            "#,
        )
        .bind(cbu_id)
        .bind(derived_attr_id)
        .fetch_one(&db.pool)
        .await?;
        assert!(gap_row.0);
        assert_eq!(gap_row.1.as_deref(), Some("derived"));

        let summary_row: (i64, i64, i64) = sqlx::query_as(
            r#"
            SELECT total_required, populated, missing
            FROM "ob-poc".v_cbu_attr_summary
            WHERE cbu_id = $1
            "#,
        )
        .bind(cbu_id)
        .fetch_one(&db.pool)
        .await?;
        assert_eq!(summary_row, (2, 2, 0));

        db.cleanup(&[legacy_attr_id, derived_attr_id], &[], &[cbu_id], &[])
            .await?;
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn non_cbu_recompute_uses_attribute_values_typed_inputs() -> Result<()> {
        let db = TestDb::new().await?;
        let entity_id = Uuid::new_v4();
        let input_attr_id = Uuid::new_v4();
        let derived_attr_id = Uuid::new_v4();
        let (company_type_id, company_type_code) =
            existing_entity_type_id(&db.pool, &["limited_company", "limited_company_private"])
                .await?;
        let spec_fqn = "test.non_cbu_threshold_flag";
        let output_fqn = "test.non_cbu_threshold_flag_value";
        let snapshot_object_id = object_id_for(ObjectType::DerivationSpec, spec_fqn);

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name, name_norm)
            VALUES ($1, $2, $3, lower($3))
            "#,
        )
        .bind(entity_id)
        .bind(company_type_id)
        .bind(db.name("company"))
        .execute(&db.pool)
        .await?;

        insert_attr_registry(
            &db.pool,
            input_attr_id,
            "test.non_cbu_total_ownership_pct_value",
            "risk",
            "number",
            false,
            None,
        )
        .await?;
        insert_attr_registry(
            &db.pool,
            derived_attr_id,
            output_fqn,
            "risk",
            "boolean",
            true,
            Some(spec_fqn),
        )
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".attribute_values_typed (
                entity_id, attribute_id, value_number, attribute_uuid, source
            )
            VALUES ($1, $2, 30, $3, '{"origin":"integration-test"}'::jsonb)
            "#,
        )
        .bind(entity_id)
        .bind("test.non_cbu_total_ownership_pct_value")
        .bind(input_attr_id)
        .execute(&db.pool)
        .await?;

        let definition = serde_json::to_value(DerivationSpecBody {
            fqn: spec_fqn.to_string(),
            name: "non-cbu threshold".to_string(),
            description: "integration test".to_string(),
            output_attribute_fqn: output_fqn.to_string(),
            inputs: vec![DerivationInput {
                attribute_fqn: "test.non_cbu_total_ownership_pct_value".to_string(),
                role: "primary".to_string(),
                required: true,
            }],
            expression: DerivationExpression::FunctionRef {
                ref_name: "threshold_flag".to_string(),
            },
            null_semantics: NullSemantics::Propagate,
            freshness_rule: None,
            security_inheritance: SecurityInheritanceMode::Strict,
            evidence_grade: EvidenceGrade::Prohibited,
            tests: Vec::new(),
        })?;

        sqlx::query(
            r#"
            INSERT INTO sem_reg.snapshots (
                object_type, object_id, status, governance_tier, trust_class,
                security_label, change_type, created_by, definition
            )
            VALUES (
                'derivation_spec', $1, 'active', 'operational', 'convenience',
                '{}'::jsonb, 'created', 'integration-test', $2
            )
            "#,
        )
        .bind(snapshot_object_id)
        .bind(&definition)
        .execute(&db.pool)
        .await?;

        let engine = PopulationEngine::new(&db.pool);
        let outcome = engine
            .recompute_derived(&company_type_code, entity_id, derived_attr_id)
            .await?;
        assert!(matches!(
            outcome,
            ob_poc::service_resources::discovery::RecomputeOutcome::Recomputed
        ));

        let row: Option<(String, serde_json::Value)> = sqlx::query_as(
            r#"
            SELECT entity_type, value
            FROM "ob-poc".v_derived_current
            WHERE entity_id = $1 AND attr_id = $2
            "#,
        )
        .bind(entity_id)
        .bind(derived_attr_id)
        .fetch_optional(&db.pool)
        .await?;
        let row = row.context("missing canonical non-CBU derived row")?;
        assert_eq!(row.0, company_type_code);
        assert_eq!(row.1, json!(true));

        let cbu_projection_count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".v_cbu_derived_values WHERE attr_id = $1"#,
        )
        .bind(derived_attr_id)
        .fetch_one(&db.pool)
        .await?;
        assert_eq!(cbu_projection_count, 0);

        db.cleanup(
            &[input_attr_id, derived_attr_id],
            &[entity_id],
            &[],
            &[snapshot_object_id],
        )
        .await?;
        Ok(())
    }
}
