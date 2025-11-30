//! Onboarding Scenario Test Harness
//!
//! Tests full onboarding flows for custody bank products:
//! 1. Global Custody onboarding
//! 2. Fund Accounting onboarding
//! 3. Middle Office IBOR onboarding
//! 4. Multi-product onboarding

#[cfg(feature = "database")]
mod onboarding_tests {
    use anyhow::Result;
    use sqlx::PgPool;
    use uuid::Uuid;

    use ob_poc::dsl_v2::{compile, parse_program, DslExecutor, ExecutionContext};

    // =========================================================================
    // TEST INFRASTRUCTURE
    // =========================================================================

    struct TestDb {
        pool: PgPool,
        prefix: String,
    }

    impl TestDb {
        async fn new() -> Result<Self> {
            let url = std::env::var("TEST_DATABASE_URL")
                .or_else(|_| std::env::var("DATABASE_URL"))
                .unwrap_or_else(|_| "postgresql:///data_designer".into());

            let pool = PgPool::connect(&url).await?;
            let prefix = format!("onb_{}", &Uuid::new_v4().to_string()[..8]);
            Ok(Self { pool, prefix })
        }

        fn name(&self, base: &str) -> String {
            format!("{}_{}", self.prefix, base)
        }

        async fn cleanup(&self) -> Result<()> {
            let pattern = format!("{}%", self.prefix);

            // Delete in reverse dependency order
            // Service delivery map
            sqlx::query(
                r#"DELETE FROM "ob-poc".service_delivery_map WHERE cbu_id IN
                   (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // Resource instance attributes
            sqlx::query(
                r#"DELETE FROM "ob-poc".resource_instance_attributes WHERE instance_id IN
                   (SELECT instance_id FROM "ob-poc".cbu_resource_instances WHERE cbu_id IN
                    (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1))"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // Resource instances
            sqlx::query(
                r#"DELETE FROM "ob-poc".cbu_resource_instances WHERE cbu_id IN
                   (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // CBU entity roles
            sqlx::query(
                r#"DELETE FROM "ob-poc".cbu_entity_roles WHERE cbu_id IN
                   (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // CBUs
            sqlx::query(r#"DELETE FROM "ob-poc".cbus WHERE name LIKE $1"#)
                .bind(&pattern)
                .execute(&self.pool)
                .await
                .ok();

            Ok(())
        }

        async fn execute_dsl(&self, dsl: &str) -> Result<ExecutionContext> {
            let ast = parse_program(dsl).map_err(|e| anyhow::anyhow!("{:?}", e))?;
            let plan = compile(&ast).map_err(|e| anyhow::anyhow!("{:?}", e))?;
            let executor = DslExecutor::new(self.pool.clone());
            let mut ctx = ExecutionContext::new();
            executor.execute_plan(&plan, &mut ctx).await?;
            Ok(ctx)
        }

        async fn get_cbu_id(&self, name: &str) -> Result<Uuid> {
            let id: Uuid =
                sqlx::query_scalar(r#"SELECT cbu_id FROM "ob-poc".cbus WHERE name = $1"#)
                    .bind(name)
                    .fetch_one(&self.pool)
                    .await?;
            Ok(id)
        }

        async fn count_resource_instances(&self, cbu_id: Uuid) -> Result<i64> {
            let count: i64 = sqlx::query_scalar(
                r#"SELECT COUNT(*) FROM "ob-poc".cbu_resource_instances WHERE cbu_id = $1"#,
            )
            .bind(cbu_id)
            .fetch_one(&self.pool)
            .await?;
            Ok(count)
        }

        async fn count_active_instances(&self, cbu_id: Uuid) -> Result<i64> {
            let count: i64 = sqlx::query_scalar(
                r#"SELECT COUNT(*) FROM "ob-poc".cbu_resource_instances
                   WHERE cbu_id = $1 AND status = 'ACTIVE'"#,
            )
            .bind(cbu_id)
            .fetch_one(&self.pool)
            .await?;
            Ok(count)
        }

        async fn count_deliveries(&self, cbu_id: Uuid) -> Result<i64> {
            let count: i64 = sqlx::query_scalar(
                r#"SELECT COUNT(*) FROM "ob-poc".service_delivery_map WHERE cbu_id = $1"#,
            )
            .bind(cbu_id)
            .fetch_one(&self.pool)
            .await?;
            Ok(count)
        }

        async fn count_completed_deliveries(&self, cbu_id: Uuid) -> Result<i64> {
            let count: i64 = sqlx::query_scalar(
                r#"SELECT COUNT(*) FROM "ob-poc".service_delivery_map
                   WHERE cbu_id = $1 AND delivery_status = 'DELIVERED'"#,
            )
            .bind(cbu_id)
            .fetch_one(&self.pool)
            .await?;
            Ok(count)
        }
    }

    // =========================================================================
    // TEST SCENARIO 1: GLOBAL CUSTODY ONBOARDING
    // =========================================================================

    /// Test Scenario 1: Global Custody Onboarding
    ///
    /// Client: Apex Capital Partners (Hedge Fund)
    /// Product: Global Custody
    /// Services: Safekeeping, Settlement
    /// Resources: Custody Account, Settlement Account, SWIFT Connection
    #[tokio::test]
    async fn test_global_custody_onboarding() -> Result<()> {
        let db = TestDb::new().await?;
        let client_name = db.name("Apex_Capital");

        let dsl = format!(
            r#"
            ;; Create the CBU
            (cbu.ensure
                :name "{client_name}"
                :jurisdiction "US"
                :client-type "fund"
                :as @apex)

            ;; Create Custody Account
            (resource.create
                :cbu-id @apex
                :resource-type "CUSTODY_ACCT"
                :instance-url "https://custody.bank.com/accounts/{client_name}-001"
                :instance-id "{client_name}-CUSTODY-001"
                :instance-name "{client_name} Custody Account"
                :as @custody_acct)

            ;; Set custody account attributes
            (resource.set-attr :instance-id @custody_acct :attr "resource.account.account_number" :value "CUST-2024-APEX-001")
            (resource.set-attr :instance-id @custody_acct :attr "resource.account.account_name" :value "{client_name} - Main")
            (resource.set-attr :instance-id @custody_acct :attr "resource.account.base_currency" :value "USD")
            (resource.set-attr :instance-id @custody_acct :attr "resource.account.account_type" :value "SEGREGATED")

            ;; Activate custody account
            (resource.activate :instance-id @custody_acct)

            ;; Create Settlement Account (DTCC)
            (resource.create
                :cbu-id @apex
                :resource-type "SETTLE_ACCT"
                :instance-url "https://dtcc.com/participants/{client_name}-SETTLE"
                :instance-id "{client_name}-SETTLE-001"
                :as @settle_acct)

            (resource.set-attr :instance-id @settle_acct :attr "resource.account.account_number" :value "DTC-APEX-789")
            (resource.set-attr :instance-id @settle_acct :attr "resource.settlement.bic_code" :value "DTCYUS33")
            (resource.set-attr :instance-id @settle_acct :attr "resource.settlement.settlement_currency" :value "USD")
            (resource.activate :instance-id @settle_acct)

            ;; Create SWIFT Connection
            (resource.create
                :cbu-id @apex
                :resource-type "SWIFT_CONN"
                :instance-url "https://swift.com/connections/{client_name}US33"
                :instance-id "{client_name}US33"
                :as @swift)

            (resource.set-attr :instance-id @swift :attr "resource.settlement.bic_code" :value "APEXUS33XXX")
            (resource.set-attr :instance-id @swift :attr "resource.swift.logical_terminal" :value "APEXUS33AXXX")
            (resource.set-attr :instance-id @swift :attr "resource.swift.message_types" :value "[\"MT540\", \"MT541\", \"MT950\"]")
            (resource.activate :instance-id @swift)

            ;; Record service deliveries
            (delivery.record :cbu-id @apex :product "GLOB_CUSTODY" :service "SAFEKEEPING" :instance-id @custody_acct)
            (delivery.record :cbu-id @apex :product "GLOB_CUSTODY" :service "SETTLEMENT" :instance-id @settle_acct)
            (delivery.complete :cbu-id @apex :product "GLOB_CUSTODY" :service "SAFEKEEPING")
            (delivery.complete :cbu-id @apex :product "GLOB_CUSTODY" :service "SETTLEMENT")
            "#,
            client_name = client_name
        );

        let result = db.execute_dsl(&dsl).await;
        assert!(
            result.is_ok(),
            "Global Custody onboarding failed: {:?}",
            result.err()
        );

        // Verify resources created
        let cbu_id = db.get_cbu_id(&client_name).await?;
        let instance_count = db.count_resource_instances(cbu_id).await?;
        assert_eq!(instance_count, 3, "Expected 3 resource instances");

        let active_count = db.count_active_instances(cbu_id).await?;
        assert_eq!(active_count, 3, "All instances should be ACTIVE");

        // Verify deliveries
        let delivery_count = db.count_deliveries(cbu_id).await?;
        assert_eq!(delivery_count, 2, "Expected 2 service deliveries");

        let completed_count = db.count_completed_deliveries(cbu_id).await?;
        assert_eq!(completed_count, 2, "All deliveries should be DELIVERED");

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // TEST SCENARIO 2: FUND ACCOUNTING ONBOARDING
    // =========================================================================

    /// Test Scenario 2: Fund Accounting Onboarding
    ///
    /// Client: Pacific Growth Fund
    /// Product: Fund Accounting
    /// Services: NAV Calculation, Investor Accounting
    /// Resources: NAV Engine, Investor Ledger
    #[tokio::test]
    async fn test_fund_accounting_onboarding() -> Result<()> {
        let db = TestDb::new().await?;
        let client_name = db.name("Pacific_Growth");

        let dsl = format!(
            r#"
            ;; Create the CBU
            (cbu.ensure
                :name "{client_name}"
                :jurisdiction "LU"
                :client-type "fund"
                :as @pgf)

            ;; Create NAV Engine instance
            (resource.create
                :cbu-id @pgf
                :resource-type "NAV_ENGINE"
                :instance-url "https://nav.fundservices.com/funds/{client_name}-001"
                :instance-id "{client_name}-NAV-001"
                :instance-name "{client_name} NAV"
                :as @nav)

            (resource.set-attr :instance-id @nav :attr "resource.fund.fund_code" :value "PGF-LU-001")
            (resource.set-attr :instance-id @nav :attr "resource.fund.valuation_frequency" :value "DAILY")
            (resource.set-attr :instance-id @nav :attr "resource.fund.pricing_source" :value "Bloomberg")
            (resource.set-attr :instance-id @nav :attr "resource.fund.nav_cutoff_time" :value "16:00 CET")
            (resource.activate :instance-id @nav)

            ;; Create Investor Ledger instance
            (resource.create
                :cbu-id @pgf
                :resource-type "INVESTOR_LEDGER"
                :instance-url "https://ta.fundservices.com/funds/{client_name}-001"
                :instance-id "{client_name}-TA-001"
                :as @ledger)

            ;; Investor Ledger has no required attributes defined, so activate directly
            (resource.activate :instance-id @ledger)

            ;; Record deliveries
            (delivery.record :cbu-id @pgf :product "FUND_ACCT" :service "NAV_CALC" :instance-id @nav)
            (delivery.record :cbu-id @pgf :product "FUND_ACCT" :service "INVESTOR_ACCT" :instance-id @ledger)
            (delivery.complete :cbu-id @pgf :product "FUND_ACCT" :service "NAV_CALC")
            (delivery.complete :cbu-id @pgf :product "FUND_ACCT" :service "INVESTOR_ACCT")
            "#,
            client_name = client_name
        );

        let result = db.execute_dsl(&dsl).await;
        assert!(
            result.is_ok(),
            "Fund Accounting onboarding failed: {:?}",
            result.err()
        );

        // Verify
        let cbu_id = db.get_cbu_id(&client_name).await?;
        let instance_count = db.count_resource_instances(cbu_id).await?;
        assert_eq!(instance_count, 2, "Expected 2 resource instances");

        let active_count = db.count_active_instances(cbu_id).await?;
        assert_eq!(active_count, 2, "All instances should be ACTIVE");

        let completed_count = db.count_completed_deliveries(cbu_id).await?;
        assert_eq!(completed_count, 2, "All deliveries should be DELIVERED");

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // TEST SCENARIO 3: MIDDLE OFFICE IBOR ONBOARDING
    // =========================================================================

    /// Test Scenario 3: Middle Office IBOR Onboarding
    ///
    /// Client: Quantum Asset Management
    /// Product: Middle Office IBOR
    /// Services: Position Management, Trade Capture, P&L Attribution
    /// Resources: IBOR System, P&L Engine
    #[tokio::test]
    async fn test_ibor_onboarding() -> Result<()> {
        let db = TestDb::new().await?;
        let client_name = db.name("Quantum_Asset");

        let dsl = format!(
            r#"
            ;; Create the CBU
            (cbu.ensure
                :name "{client_name}"
                :jurisdiction "UK"
                :client-type "corporate"
                :as @quantum)

            ;; Create IBOR System instance
            (resource.create
                :cbu-id @quantum
                :resource-type "IBOR_SYSTEM"
                :instance-url "https://ibor.platform.com/portfolios/{client_name}-001"
                :instance-id "{client_name}-IBOR-001"
                :instance-name "Quantum IBOR"
                :as @ibor)

            (resource.set-attr :instance-id @ibor :attr "resource.ibor.portfolio_code" :value "QAM-MASTER")
            (resource.set-attr :instance-id @ibor :attr "resource.ibor.accounting_basis" :value "TRADE_DATE")
            (resource.set-attr :instance-id @ibor :attr "resource.account.base_currency" :value "GBP")
            (resource.set-attr :instance-id @ibor :attr "resource.ibor.position_source" :value "OMS")
            (resource.activate :instance-id @ibor)

            ;; Create P&L Engine instance
            (resource.create
                :cbu-id @quantum
                :resource-type "PNL_ENGINE"
                :instance-url "https://pnl.platform.com/portfolios/{client_name}-001"
                :instance-id "{client_name}-PNL-001"
                :as @pnl)

            ;; P&L Engine has no required attributes defined
            (resource.activate :instance-id @pnl)

            ;; Record deliveries - IBOR serves both position and trade capture
            (delivery.record :cbu-id @quantum :product "MO_IBOR" :service "POSITION_MGMT" :instance-id @ibor)
            (delivery.record :cbu-id @quantum :product "MO_IBOR" :service "TRADE_CAPTURE" :instance-id @ibor)
            (delivery.record :cbu-id @quantum :product "MO_IBOR" :service "PNL_ATTRIB" :instance-id @pnl)
            (delivery.complete :cbu-id @quantum :product "MO_IBOR" :service "POSITION_MGMT")
            (delivery.complete :cbu-id @quantum :product "MO_IBOR" :service "TRADE_CAPTURE")
            (delivery.complete :cbu-id @quantum :product "MO_IBOR" :service "PNL_ATTRIB")
            "#,
            client_name = client_name
        );

        let result = db.execute_dsl(&dsl).await;
        assert!(result.is_ok(), "IBOR onboarding failed: {:?}", result.err());

        // Verify
        let cbu_id = db.get_cbu_id(&client_name).await?;
        let instance_count = db.count_resource_instances(cbu_id).await?;
        assert_eq!(instance_count, 2, "Expected 2 resource instances");

        let active_count = db.count_active_instances(cbu_id).await?;
        assert_eq!(active_count, 2, "All instances should be ACTIVE");

        let delivery_count = db.count_deliveries(cbu_id).await?;
        assert_eq!(delivery_count, 3, "Expected 3 service deliveries");

        let completed_count = db.count_completed_deliveries(cbu_id).await?;
        assert_eq!(completed_count, 3, "All deliveries should be DELIVERED");

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // TEST SCENARIO 4: MULTI-PRODUCT ONBOARDING
    // =========================================================================

    /// Test Scenario 4: Multi-Product Onboarding
    ///
    /// Client: Atlas Institutional Investors
    /// Products: Global Custody + Fund Accounting
    #[tokio::test]
    async fn test_multi_product_onboarding() -> Result<()> {
        let db = TestDb::new().await?;
        let client_name = db.name("Atlas_Inst");

        let dsl = format!(
            r#"
            ;; Create the CBU
            (cbu.ensure
                :name "{client_name}"
                :jurisdiction "US"
                :client-type "fund"
                :as @atlas)

            ;; === GLOBAL CUSTODY ===
            (resource.create
                :cbu-id @atlas
                :resource-type "CUSTODY_ACCT"
                :instance-url "https://custody.bank.com/accounts/{client_name}-001"
                :instance-id "{client_name}-CUSTODY-001"
                :as @custody)

            (resource.set-attr :instance-id @custody :attr "resource.account.account_number" :value "CUST-ATLAS-001")
            (resource.set-attr :instance-id @custody :attr "resource.account.account_name" :value "Atlas Pension - Main Custody")
            (resource.set-attr :instance-id @custody :attr "resource.account.base_currency" :value "USD")
            (resource.set-attr :instance-id @custody :attr "resource.account.account_type" :value "OMNIBUS")
            (resource.activate :instance-id @custody)

            ;; === FUND ACCOUNTING ===
            (resource.create
                :cbu-id @atlas
                :resource-type "NAV_ENGINE"
                :instance-url "https://nav.fundservices.com/funds/{client_name}-001"
                :instance-id "{client_name}-NAV-001"
                :as @nav)

            (resource.set-attr :instance-id @nav :attr "resource.fund.fund_code" :value "ATLAS-PEN-001")
            (resource.set-attr :instance-id @nav :attr "resource.fund.valuation_frequency" :value "DAILY")
            (resource.set-attr :instance-id @nav :attr "resource.fund.pricing_source" :value "Reuters")
            (resource.set-attr :instance-id @nav :attr "resource.fund.nav_cutoff_time" :value "17:00 EST")
            (resource.activate :instance-id @nav)

            ;; Record all deliveries
            (delivery.record :cbu-id @atlas :product "GLOB_CUSTODY" :service "SAFEKEEPING" :instance-id @custody)
            (delivery.record :cbu-id @atlas :product "FUND_ACCT" :service "NAV_CALC" :instance-id @nav)
            (delivery.complete :cbu-id @atlas :product "GLOB_CUSTODY" :service "SAFEKEEPING")
            (delivery.complete :cbu-id @atlas :product "FUND_ACCT" :service "NAV_CALC")
            "#,
            client_name = client_name
        );

        let result = db.execute_dsl(&dsl).await;
        assert!(
            result.is_ok(),
            "Multi-product onboarding failed: {:?}",
            result.err()
        );

        // Verify resources
        let cbu_id = db.get_cbu_id(&client_name).await?;
        let instance_count = db.count_resource_instances(cbu_id).await?;
        assert_eq!(instance_count, 2, "Expected 2 resource instances");

        let active_count = db.count_active_instances(cbu_id).await?;
        assert_eq!(active_count, 2, "All instances should be ACTIVE");

        // Verify deliveries from different products
        let delivery_count = db.count_deliveries(cbu_id).await?;
        assert_eq!(delivery_count, 2, "Expected 2 service deliveries");

        let completed_count = db.count_completed_deliveries(cbu_id).await?;
        assert_eq!(completed_count, 2, "All deliveries should be DELIVERED");

        // Verify we have deliveries from both products
        let product_count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(DISTINCT product_id) FROM "ob-poc".service_delivery_map WHERE cbu_id = $1"#,
        )
        .bind(cbu_id)
        .fetch_one(&db.pool)
        .await?;
        assert_eq!(
            product_count, 2,
            "Expected deliveries from 2 different products"
        );

        db.cleanup().await?;
        Ok(())
    }
}
