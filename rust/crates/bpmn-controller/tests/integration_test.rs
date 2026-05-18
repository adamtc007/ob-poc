// bpmn-controller integration tests.
//
// DB tests require a Postgres instance with bpmn-lite migrations 001–032
// applied. Set BPMN_LITE_TEST_DATABASE_URL and run with:
//
//   BPMN_LITE_TEST_DATABASE_URL="postgresql:///data_designer" \
//     cargo test -p bpmn-controller -- --ignored --nocapture
//
// K8s tests require a real cluster (kind/minikube) and BPMN_LITE_K8S_NAMESPACE
// to be set (or the default namespace is used). Run with:
//
//   cargo test -p bpmn-controller k8s -- --ignored --nocapture

#[cfg(test)]
mod unit {
    #[test]
    fn scaffold_compiles() {}
}

// ── L5: instance kick-off ─────────────────────────────────────────────────────

#[cfg(test)]
mod l5_db {
    use bpmn_controller::{instance_status, list_tenant_instances, start_instance};
    use ob_poc_types::InstanceState;
    use sqlx::PgPool;
    use uuid::Uuid;

    async fn test_pool() -> Option<PgPool> {
        let url = std::env::var("BPMN_LITE_TEST_DATABASE_URL").ok()?;
        PgPool::connect(&url).await.ok()
    }

    /// Insert a minimal published workflow_template for test use.
    /// Returns the fake bytecode_version hex.
    async fn seed_template(pg: &PgPool, process_key: &str) -> String {
        // 64-char hex string — fake but structurally valid.
        let bytecode_hex = format!("{:0>64}", process_key.len());
        sqlx::query(
            "INSERT INTO workflow_templates \
             (template_key, template_version, process_key, bytecode_version, \
              state, dto_snapshot, task_manifest) \
             VALUES ($1, 1, $2, $3, 'published', '{}'::jsonb, '[]'::jsonb) \
             ON CONFLICT DO NOTHING",
        )
        .bind(format!("test-{}", process_key))
        .bind(process_key)
        .bind(&bytecode_hex)
        .execute(pg)
        .await
        .unwrap();
        bytecode_hex
    }

    async fn seed_tenant(pg: &PgPool, tenant_id: &str) {
        sqlx::query("INSERT INTO tenants (tenant_id) VALUES ($1) ON CONFLICT DO NOTHING")
            .bind(tenant_id)
            .execute(pg)
            .await
            .unwrap();
    }

    /// T-L5-1: start_instance writes a Running row and returns a UUID.
    #[tokio::test]
    #[ignore]
    async fn t_l5_1_start_instance_creates_running_row() {
        let pg = test_pool()
            .await
            .expect("BPMN_LITE_TEST_DATABASE_URL not set");
        let tenant_id = "l5-test-tenant-start";
        let process_key = "l5-test-proc-start";

        seed_tenant(&pg, tenant_id).await;
        seed_template(&pg, process_key).await;

        let instance_id = start_instance(
            &pg,
            tenant_id,
            process_key,
            serde_json::json!({"amount": 100}),
            None,
        )
        .await
        .expect("start_instance must succeed");

        // Row must be present with state=Running.
        let status = instance_status(&pg, instance_id)
            .await
            .expect("instance_status must succeed");

        assert_eq!(status.instance_id, instance_id);
        assert_eq!(status.tenant_id, tenant_id);
        assert_eq!(status.process_key, process_key);
        assert_eq!(status.state, InstanceState::Running);
        assert!(status.completed_at.is_none());
    }

    /// T-L5-2: start_instance with an unknown tenant errors.
    #[tokio::test]
    #[ignore]
    async fn t_l5_2_start_instance_unknown_tenant_errors() {
        let pg = test_pool()
            .await
            .expect("BPMN_LITE_TEST_DATABASE_URL not set");
        let process_key = "l5-test-proc-no-tenant";

        seed_template(&pg, process_key).await;

        let result = start_instance(
            &pg,
            "no-such-tenant-l5",
            process_key,
            serde_json::json!({}),
            None,
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    /// T-L5-3: start_instance with an unknown process_key errors.
    #[tokio::test]
    #[ignore]
    async fn t_l5_3_start_instance_unknown_process_errors() {
        let pg = test_pool()
            .await
            .expect("BPMN_LITE_TEST_DATABASE_URL not set");
        let tenant_id = "l5-test-tenant-no-proc";

        seed_tenant(&pg, tenant_id).await;

        let result = start_instance(
            &pg,
            tenant_id,
            "no-such-process-key-l5-xyz",
            serde_json::json!({}),
            None,
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    /// T-L5-4: idempotency_key returns the same instance_id on retry.
    #[tokio::test]
    #[ignore]
    async fn t_l5_4_start_instance_idempotency() {
        let pg = test_pool()
            .await
            .expect("BPMN_LITE_TEST_DATABASE_URL not set");
        let tenant_id = "l5-test-tenant-idem";
        let process_key = "l5-test-proc-idem";
        let idem_key = "l5-idem-key-abc123";

        seed_tenant(&pg, tenant_id).await;
        seed_template(&pg, process_key).await;

        let id1 = start_instance(
            &pg,
            tenant_id,
            process_key,
            serde_json::json!({"x": 1}),
            Some(idem_key),
        )
        .await
        .expect("first call must succeed");

        let id2 = start_instance(
            &pg,
            tenant_id,
            process_key,
            serde_json::json!({"x": 999}), // different payload — ignored on retry
            Some(idem_key),
        )
        .await
        .expect("second call must succeed");

        assert_eq!(id1, id2, "idempotency key must return the same instance_id");
    }

    /// T-L5-5: list_tenant_instances returns the started instance.
    #[tokio::test]
    #[ignore]
    async fn t_l5_5_list_tenant_instances_after_start() {
        let pg = test_pool()
            .await
            .expect("BPMN_LITE_TEST_DATABASE_URL not set");
        let tenant_id = "l5-test-tenant-list";
        let process_key = "l5-test-proc-list";

        seed_tenant(&pg, tenant_id).await;
        seed_template(&pg, process_key).await;

        let started_id = start_instance(&pg, tenant_id, process_key, serde_json::json!({}), None)
            .await
            .unwrap();

        let instances = list_tenant_instances(&pg, tenant_id)
            .await
            .expect("list must succeed");

        assert!(
            instances.iter().any(|i| i.instance_id == started_id),
            "started instance must appear in list"
        );

        // T-L5-6: instance_status errors for unknown UUID.
        let result = instance_status(&pg, Uuid::new_v4()).await;
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod l4_db {
    use bpmn_controller::{deprovision_pool, provision_pool, K8sClient};
    use ob_poc_types::{PoolConfig, PoolType};
    use sqlx::PgPool;

    async fn test_pool() -> Option<PgPool> {
        let url = std::env::var("BPMN_LITE_TEST_DATABASE_URL").ok()?;
        PgPool::connect(&url).await.ok()
    }

    fn small_config(k8s: &K8sClient) -> PoolConfig {
        PoolConfig {
            image: "alpine:latest".to_string(),
            replicas: 1,
            min_replicas: 1,
            max_replicas: 2,
            cpu_request: "50m".to_string(),
            memory_request: "64Mi".to_string(),
            cpu_limit: "200m".to_string(),
            memory_limit: "128Mi".to_string(),
            namespace: k8s.namespace().to_string(),
        }
    }

    /// T-L4-1: provision then deprovision a pool round-trip (DB side; placeholder K8s).
    #[tokio::test]
    #[ignore]
    async fn t_l4_1_provision_deprovision_roundtrip() {
        let pg = test_pool()
            .await
            .expect("BPMN_LITE_TEST_DATABASE_URL not set");
        let k8s = K8sClient::placeholder();
        let pool_id = "l4-test-pool-roundtrip";

        // Clean up any leftover from a previous run.
        sqlx::query("DELETE FROM tenant_pools WHERE pool_id = $1")
            .bind(pool_id)
            .execute(&pg)
            .await
            .unwrap();

        provision_pool(
            &pg,
            &k8s,
            pool_id,
            PoolType::Dedicated,
            &[],
            small_config(&k8s),
        )
        .await
        .expect("provision_pool must succeed");

        // Row must be present.
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tenant_pools WHERE pool_id = $1)")
                .bind(pool_id)
                .fetch_one(&pg)
                .await
                .unwrap();
        assert!(exists, "pool row must exist after provision");

        deprovision_pool(&pg, &k8s, pool_id)
            .await
            .expect("deprovision_pool must succeed");

        // Row must be gone.
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tenant_pools WHERE pool_id = $1)")
                .bind(pool_id)
                .fetch_one(&pg)
                .await
                .unwrap();
        assert!(!exists, "pool row must be absent after deprovision");
    }

    /// T-L4-2: provision_pool is idempotent — second call with same args succeeds.
    #[tokio::test]
    #[ignore]
    async fn t_l4_2_provision_idempotent() {
        let pg = test_pool()
            .await
            .expect("BPMN_LITE_TEST_DATABASE_URL not set");
        let k8s = K8sClient::placeholder();
        let pool_id = "l4-test-pool-idempotent";

        sqlx::query("DELETE FROM tenant_pools WHERE pool_id = $1")
            .bind(pool_id)
            .execute(&pg)
            .await
            .unwrap();

        provision_pool(
            &pg,
            &k8s,
            pool_id,
            PoolType::Dedicated,
            &[],
            small_config(&k8s),
        )
        .await
        .expect("first provision must succeed");

        provision_pool(
            &pg,
            &k8s,
            pool_id,
            PoolType::Dedicated,
            &[],
            small_config(&k8s),
        )
        .await
        .expect("second provision must succeed (idempotent)");

        // Cleanup.
        deprovision_pool(&pg, &k8s, pool_id).await.unwrap();
    }

    /// T-L4-3: provision_pool with an unknown tenant errors.
    #[tokio::test]
    #[ignore]
    async fn t_l4_3_provision_unknown_tenant_errors() {
        let pg = test_pool()
            .await
            .expect("BPMN_LITE_TEST_DATABASE_URL not set");
        let k8s = K8sClient::placeholder();

        let result = provision_pool(
            &pg,
            &k8s,
            "l4-test-pool-bad-tenant",
            PoolType::Dedicated,
            &["no_such_tenant_xyz".to_string()],
            small_config(&k8s),
        )
        .await;

        assert!(result.is_err(), "unknown tenant must error");
        assert!(
            result.unwrap_err().to_string().contains("not found"),
            "error must mention 'not found'"
        );
    }

    /// T-L4-4: deprovision_pool("default") is rejected.
    #[tokio::test]
    #[ignore]
    async fn t_l4_4_deprovision_default_pool_rejected() {
        let pg = test_pool()
            .await
            .expect("BPMN_LITE_TEST_DATABASE_URL not set");
        let k8s = K8sClient::placeholder();

        let result = deprovision_pool(&pg, &k8s, "default").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("default pool"));
    }

    /// T-L4-5: deprovision non-existent pool errors.
    #[tokio::test]
    #[ignore]
    async fn t_l4_5_deprovision_nonexistent_pool_errors() {
        let pg = test_pool()
            .await
            .expect("BPMN_LITE_TEST_DATABASE_URL not set");
        let k8s = K8sClient::placeholder();

        let result = deprovision_pool(&pg, &k8s, "no_such_pool_xyz_l4").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    /// T-L4-6: deprovision pool that still has tenants errors.
    #[tokio::test]
    #[ignore]
    async fn t_l4_6_deprovision_pool_with_tenants_errors() {
        let pg = test_pool()
            .await
            .expect("BPMN_LITE_TEST_DATABASE_URL not set");
        let k8s = K8sClient::placeholder();
        let pool_id = "l4-test-pool-has-tenants";
        let tenant_id = "l4-test-tenant-for-pool";

        // Setup: pool + tenant assigned to it.
        sqlx::query("DELETE FROM tenant_pools WHERE pool_id = $1")
            .bind(pool_id)
            .execute(&pg)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO tenants (tenant_id, pool_id) VALUES ($1, 'default') \
             ON CONFLICT (tenant_id) DO NOTHING",
        )
        .bind(tenant_id)
        .execute(&pg)
        .await
        .unwrap();

        provision_pool(
            &pg,
            &k8s,
            pool_id,
            PoolType::Dedicated,
            &[],
            small_config(&k8s),
        )
        .await
        .unwrap();

        // Manually assign the tenant to this pool.
        sqlx::query("UPDATE tenants SET pool_id = $1 WHERE tenant_id = $2")
            .bind(pool_id)
            .bind(tenant_id)
            .execute(&pg)
            .await
            .unwrap();

        // Deprovision must fail — pool still has a tenant.
        let result = deprovision_pool(&pg, &k8s, pool_id).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("tenants"));

        // Cleanup: move tenant back to default, then deprovision.
        sqlx::query("UPDATE tenants SET pool_id = 'default' WHERE tenant_id = $1")
            .bind(tenant_id)
            .execute(&pg)
            .await
            .unwrap();
        deprovision_pool(&pg, &k8s, pool_id).await.unwrap();
    }
}

#[cfg(test)]
mod k8s_tests {
    use bpmn_controller::K8sClient;

    /// T-L3-K8s-1: create then delete a Deployment round-trip.
    ///
    /// Requires a real K8s cluster. Run manually against kind/minikube.
    #[tokio::test]
    #[ignore]
    async fn t_l3_k8s_1_create_delete_deployment_roundtrip() {
        use bpmn_controller::loader::{deprovision_pool, provision_pool};
        use ob_poc_types::{PoolConfig, PoolType};

        let k8s = K8sClient::from_infer()
            .await
            .expect("K8s client must be available for this test");

        let pool_id = "bpmn-ctrl-test-pool";
        let config = PoolConfig {
            image: "alpine:latest".to_string(), // lightweight — no need to pull real image
            replicas: 1,
            min_replicas: 1,
            max_replicas: 2,
            cpu_request: "50m".to_string(),
            memory_request: "64Mi".to_string(),
            cpu_limit: "200m".to_string(),
            memory_limit: "128Mi".to_string(),
            namespace: k8s.namespace().to_string(),
        };

        // provision (create Deployment)
        let pg = {
            let url = std::env::var("BPMN_LITE_TEST_DATABASE_URL")
                .expect("BPMN_LITE_TEST_DATABASE_URL required");
            sqlx::PgPool::connect(&url).await.expect("DB connect")
        };
        provision_pool(&pg, &k8s, pool_id, PoolType::Dedicated, &[], config)
            .await
            .expect("provision_pool must succeed");

        // deprovision (delete Deployment)
        deprovision_pool(&pg, &k8s, pool_id)
            .await
            .expect("deprovision_pool must succeed");

        // second deprovision must be idempotent
        deprovision_pool(&pg, &k8s, pool_id)
            .await
            .expect("deprovision_pool must be idempotent");
    }

    /// T-L3-K8s-2: pool_status returns pod counts when Deployment exists.
    #[tokio::test]
    #[ignore]
    async fn t_l3_k8s_2_pool_status_with_real_k8s() {
        use bpmn_controller::loader::pool_status;
        use sqlx::PgPool;

        let k8s = K8sClient::from_infer()
            .await
            .expect("K8s client must be available");
        let pg = {
            let url = std::env::var("BPMN_LITE_TEST_DATABASE_URL")
                .expect("BPMN_LITE_TEST_DATABASE_URL required");
            PgPool::connect(&url).await.expect("DB connect")
        };

        let status = pool_status(&pg, &k8s, "default")
            .await
            .expect("pool_status must succeed");
        // With a real K8s cluster and a provisioned default pool, pod counts
        // may be Some. Without a Deployment, they're None — both are valid.
        println!(
            "pool_status: desired={:?} ready={:?}",
            status.desired_replicas, status.ready_replicas
        );
    }
}

#[cfg(test)]
mod db {
    use bpmn_controller::{
        instance_status, list_pool_tenants, list_pools, list_tenant_instances, pool_status,
        K8sClient,
    };
    use sqlx::PgPool;

    async fn test_pool() -> Option<PgPool> {
        let url = std::env::var("BPMN_LITE_TEST_DATABASE_URL").ok()?;
        PgPool::connect(&url).await.ok()
    }

    /// T-L2-1: list_pools returns at least the default pool after migrations.
    #[tokio::test]
    #[ignore]
    async fn t_l2_1_list_pools_contains_default() {
        let pg = test_pool()
            .await
            .expect("BPMN_LITE_TEST_DATABASE_URL not set");
        let pools = list_pools(&pg).await.expect("list_pools failed");
        assert!(
            pools.iter().any(|p| p.pool_id == "default"),
            "default pool must be present; got: {:?}",
            pools.iter().map(|p| &p.pool_id).collect::<Vec<_>>()
        );
    }

    /// T-L2-2: list_pool_tenants returns an empty vec for a nonexistent pool.
    #[tokio::test]
    #[ignore]
    async fn t_l2_2_list_pool_tenants_empty_for_unknown_pool() {
        let pg = test_pool()
            .await
            .expect("BPMN_LITE_TEST_DATABASE_URL not set");
        let tenants = list_pool_tenants(&pg, "no_such_pool_xyz")
            .await
            .expect("list_pool_tenants failed");
        assert!(tenants.is_empty());
    }

    /// T-L2-3: pool_status returns correct shape for the default pool.
    #[tokio::test]
    #[ignore]
    async fn t_l2_3_pool_status_default() {
        let pg = test_pool()
            .await
            .expect("BPMN_LITE_TEST_DATABASE_URL not set");
        let k8s = K8sClient::placeholder();
        let status = pool_status(&pg, &k8s, "default")
            .await
            .expect("pool_status failed");
        assert_eq!(status.pool_id, "default");
        assert!(!status.paused);
        assert!(
            status.desired_replicas.is_none(),
            "K8s counts must be None in L2"
        );
        assert!(
            status.ready_replicas.is_none(),
            "K8s counts must be None in L2"
        );
    }

    /// T-L2-4: pool_status errors for a nonexistent pool.
    #[tokio::test]
    #[ignore]
    async fn t_l2_4_pool_status_unknown_pool_errors() {
        let pg = test_pool()
            .await
            .expect("BPMN_LITE_TEST_DATABASE_URL not set");
        let k8s = K8sClient::placeholder();
        let result = pool_status(&pg, &k8s, "no_such_pool_xyz").await;
        assert!(result.is_err(), "must error for unknown pool");
    }

    /// T-L2-5: list_tenant_instances returns empty vec for an unknown tenant.
    #[tokio::test]
    #[ignore]
    async fn t_l2_5_list_tenant_instances_empty_for_unknown_tenant() {
        let pg = test_pool()
            .await
            .expect("BPMN_LITE_TEST_DATABASE_URL not set");
        let instances = list_tenant_instances(&pg, "no_such_tenant_xyz")
            .await
            .expect("list_tenant_instances failed");
        assert!(instances.is_empty());
    }

    /// T-L2-6: instance_status errors for an unknown instance_id.
    #[tokio::test]
    #[ignore]
    async fn t_l2_6_instance_status_unknown_errors() {
        let pg = test_pool()
            .await
            .expect("BPMN_LITE_TEST_DATABASE_URL not set");
        let unknown_id = uuid::Uuid::new_v4();
        let result = instance_status(&pg, unknown_id).await;
        assert!(result.is_err(), "must error for unknown instance");
    }
}
