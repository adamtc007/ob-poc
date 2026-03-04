//! AffinityGraph — Integration Tests
//!
//! Six scenarios proving the AffinityGraph pipeline end-to-end against a live
//! registry populated by the ob-poc seed bundle:
//!
//! 1. `test_affinity_graph_from_live_registry` — build from active snapshots, verify edges
//! 2. `test_verbs_for_table_cbus` — `verbs_for_table("ob-poc", "cbus")` → cbu.* verbs
//! 3. `test_data_for_verb_cbu_create` — `data_for_verb("cbu.create")` → Table("ob-poc", "cbus")
//! 4. `test_adjacent_verbs` — cbu.create and cbu.list share the cbus table
//! 5. `test_governance_gaps` — orphan_verbs() returns an empty-or-non-empty vec without panic
//! 6. `test_diagram_erd_generation` — TableInput → build_diagram_model → render_erd → valid Mermaid
//!
//! All tests require a running PostgreSQL instance with migrations applied AND the
//! seed bundle bootstrapped (i.e. `SemOsClient::bootstrap_seed_bundle()` was called).
//!
//! Run with:
//! ```sh
//! DATABASE_URL="postgresql:///data_designer" \
//!   cargo test --features database --test affinity_integration -- --ignored --nocapture
//! ```

#[cfg(feature = "database")]
mod integration {
    use anyhow::Result;
    use sqlx::PgPool;

    use sem_os_core::affinity::types::{DataRef, TableRef};
    use sem_os_core::affinity::AffinityGraph;
    use sem_os_core::diagram::model::{ColumnInput, RenderOptions, TableInput};
    use sem_os_core::diagram::{build_diagram_model, render_erd};
    use sem_os_core::ports::SnapshotStore;
    use sem_os_postgres::PgStores;

    // ── Test Infrastructure ───────────────────────────────────────────────────

    struct TestDb {
        pool: PgPool,
    }

    impl TestDb {
        async fn new() -> Result<Self> {
            let url = std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgresql:///data_designer".into());
            let pool = PgPool::connect(&url).await?;
            Ok(Self { pool })
        }

        /// Load active snapshots and build an AffinityGraph.
        async fn affinity_graph(&self) -> Result<AffinityGraph> {
            let stores = PgStores::new(self.pool.clone());
            let snapshots = stores.snapshots.load_active_snapshots().await?;
            Ok(AffinityGraph::build(&snapshots))
        }
    }

    // ── Tests ─────────────────────────────────────────────────────────────────

    /// Build AffinityGraph from live registry snapshots and assert it has edges.
    ///
    /// This test confirms that:
    /// - The seed bundle has been bootstrapped (snapshots exist)
    /// - `AffinityGraph::build()` does not panic on real data
    /// - At least one verb→data edge exists
    #[tokio::test]
    #[ignore]
    async fn test_affinity_graph_from_live_registry() -> Result<()> {
        let db = TestDb::new().await?;
        let graph = db.affinity_graph().await?;

        println!(
            "AffinityGraph: {} edges, {} verb→data entries, {} data→verb entries",
            graph.edges.len(),
            graph.verb_to_data.len(),
            graph.data_to_verb.len()
        );

        // After bootstrapping the seed bundle, the graph must have edges.
        // If it's empty the seed bundle wasn't bootstrapped — fail with a clear message.
        assert!(
            !graph.edges.is_empty(),
            "AffinityGraph has no edges — was the seed bundle bootstrapped? \
             Run: DATABASE_URL=... cargo run -p ob-poc-web (starts server and bootstraps)"
        );

        // Verify indexes are internally consistent: every index entry references a valid edge.
        for (verb_fqn, indices) in &graph.verb_to_data {
            for &idx in indices {
                assert!(
                    idx < graph.edges.len(),
                    "verb_to_data[{verb_fqn}] contains out-of-bounds edge index {idx}"
                );
            }
        }
        for (data_key, indices) in &graph.data_to_verb {
            for &idx in indices {
                assert!(
                    idx < graph.edges.len(),
                    "data_to_verb[{data_key}] contains out-of-bounds edge index {idx}"
                );
            }
        }

        println!(
            "✓ AffinityGraph built successfully with {} edges",
            graph.edges.len()
        );
        Ok(())
    }

    /// `verbs_for_table("ob-poc", "cbus")` should return at least cbu.create and cbu.list.
    ///
    /// The seed bundle scanner wires cbu.* verbs to the ob-poc.cbus table via
    /// their CRUD mapping. If no verbs are found, the scanner or CRUD config is broken.
    #[tokio::test]
    #[ignore]
    async fn test_verbs_for_table_cbus() -> Result<()> {
        let db = TestDb::new().await?;
        let graph = db.affinity_graph().await?;

        let verbs = graph.verbs_for_table("ob-poc", "cbus");

        println!("verbs_for_table(ob-poc, cbus): {} results", verbs.len());
        for v in &verbs {
            println!(
                "  {} ({:?} via {:?})",
                v.verb_fqn, v.affinity_kind, v.provenance
            );
        }

        // Must find at least some verbs touching the cbus table.
        assert!(
            !verbs.is_empty(),
            "verbs_for_table(ob-poc, cbus) returned empty — \
             check that cbu.* verbs have CRUD config pointing to ob-poc.cbus"
        );

        // cbu.create must appear (it inserts into cbus).
        let verb_fqns: Vec<&str> = verbs.iter().map(|v| v.verb_fqn.as_str()).collect();
        assert!(
            verb_fqns.contains(&"cbu.create"),
            "cbu.create not found in verbs_for_table(ob-poc, cbus); got: {verb_fqns:?}"
        );

        println!(
            "✓ verbs_for_table returned {} verbs including cbu.create",
            verbs.len()
        );
        Ok(())
    }

    /// `data_for_verb("cbu.create")` should include the ob-poc.cbus table.
    ///
    /// The CRUD mapping for cbu.create should produce a CrudInsert edge to cbus.
    #[tokio::test]
    #[ignore]
    async fn test_data_for_verb_cbu_create() -> Result<()> {
        let db = TestDb::new().await?;
        let graph = db.affinity_graph().await?;

        let data = graph.data_for_verb("cbu.create");

        println!("data_for_verb(cbu.create): {} results", data.len());
        for d in &data {
            println!(
                "  {:?} ({:?} via {:?})",
                d.data_ref, d.affinity_kind, d.provenance
            );
        }

        assert!(
            !data.is_empty(),
            "data_for_verb(cbu.create) returned empty — \
             cbu.create has no affinity edges; check CRUD config or seed bundle"
        );

        // The ob-poc.cbus table must appear.
        let has_cbus_table = data.iter().any(|d| {
            matches!(
                &d.data_ref,
                DataRef::Table(TableRef { schema, table })
                    if schema == "ob-poc" && table == "cbus"
            )
        });
        assert!(
            has_cbus_table,
            "data_for_verb(cbu.create) does not include Table(ob-poc, cbus); \
             got: {:?}",
            data.iter().map(|d| &d.data_ref).collect::<Vec<_>>()
        );

        println!("✓ data_for_verb(cbu.create) includes Table(ob-poc, cbus)");
        Ok(())
    }

    /// `adjacent_verbs("cbu.create")` should include `cbu.list` (both touch cbus).
    ///
    /// Two verbs are adjacent when they share at least one data asset.
    /// cbu.create (insert) and cbu.list (select) both reference ob-poc.cbus.
    #[tokio::test]
    #[ignore]
    async fn test_adjacent_verbs() -> Result<()> {
        let db = TestDb::new().await?;
        let graph = db.affinity_graph().await?;

        let adj = graph.adjacent_verbs("cbu.create");

        println!("adjacent_verbs(cbu.create): {} results", adj.len());
        for (fqn, shared) in &adj {
            println!("  {} (shares {} data assets)", fqn, shared.len());
        }

        // Must return some adjacent verbs — at least cbu.list, cbu.get, etc.
        assert!(
            !adj.is_empty(),
            "adjacent_verbs(cbu.create) returned empty — \
             no other verbs share data with cbu.create"
        );

        // cbu.list must appear (it reads the same cbus table).
        let adj_fqns: Vec<&str> = adj.iter().map(|(fqn, _)| fqn.as_str()).collect();
        assert!(
            adj_fqns.contains(&"cbu.list"),
            "cbu.list not found in adjacent_verbs(cbu.create); got: {adj_fqns:?}"
        );

        println!(
            "✓ adjacent_verbs(cbu.create) includes cbu.list ({} adjacent total)",
            adj.len()
        );
        Ok(())
    }

    /// `orphan_verbs()` should complete without panic on a live graph.
    ///
    /// Governance gap detection must not panic regardless of graph completeness.
    /// We do not assert the count — some verbs intentionally have no data affinity
    /// (e.g. pure navigation/session verbs). We do assert the call succeeds.
    #[tokio::test]
    #[ignore]
    async fn test_governance_gaps() -> Result<()> {
        let db = TestDb::new().await?;
        let graph = db.affinity_graph().await?;

        let orphan_verbs = graph.orphan_verbs();
        let write_only = graph.write_only_attributes();
        let read_before_write = graph.read_before_write_attributes();

        println!(
            "Governance gaps: {} orphan verbs, {} write-only attrs, {} read-before-write attrs",
            orphan_verbs.len(),
            write_only.len(),
            read_before_write.len()
        );

        // Sample some orphan verbs for diagnostic output.
        for fqn in orphan_verbs.iter().take(5) {
            println!("  orphan verb: {fqn}");
        }

        // The governance queries must complete without panic.
        // We accept any result — navigation verbs (session.*, view.*) legitimately
        // have no data affinity.
        println!("✓ governance gap queries completed without panic");
        Ok(())
    }

    /// Full diagram pipeline: TableInput → build_diagram_model → render_erd → valid Mermaid.
    ///
    /// This test does NOT require a live database — it constructs TableInput manually
    /// and runs the pure-Rust diagram pipeline end to end.
    ///
    /// This also validates that the MermaidRenderer produces output the Mermaid parser
    /// can understand (contains `erDiagram` header and at least one entity block).
    #[tokio::test]
    #[ignore]
    async fn test_diagram_erd_generation() -> Result<()> {
        // Build a small TableInput representing the cbus table.
        let tables = vec![
            TableInput {
                schema: "ob-poc".into(),
                table: "cbus".into(),
                columns: vec![
                    ColumnInput {
                        name: "cbu_id".into(),
                        data_type: "uuid".into(),
                        nullable: false,
                        is_primary_key: true,
                        foreign_key: None,
                    },
                    ColumnInput {
                        name: "name".into(),
                        data_type: "text".into(),
                        nullable: false,
                        is_primary_key: false,
                        foreign_key: None,
                    },
                    ColumnInput {
                        name: "jurisdiction_code".into(),
                        data_type: "varchar".into(),
                        nullable: true,
                        is_primary_key: false,
                        foreign_key: None,
                    },
                ],
            },
            TableInput {
                schema: "ob-poc".into(),
                table: "entities".into(),
                columns: vec![
                    ColumnInput {
                        name: "entity_id".into(),
                        data_type: "uuid".into(),
                        nullable: false,
                        is_primary_key: true,
                        foreign_key: None,
                    },
                    ColumnInput {
                        name: "name".into(),
                        data_type: "text".into(),
                        nullable: false,
                        is_primary_key: false,
                        foreign_key: None,
                    },
                ],
            },
        ];

        // Build an empty AffinityGraph (no DB needed for ERD rendering).
        let graph = AffinityGraph::build(&[]);

        let options = RenderOptions {
            schema_filter: None,
            domain_filter: None,
            include_columns: true,
            show_governance: true,
            max_tables: 50,
            format: "mermaid".into(),
        };

        // Build the diagram model.
        let model = build_diagram_model(&tables, &graph, &options);

        println!(
            "DiagramModel: {} entities, {} relationships",
            model.entities.len(),
            model.relationships.len()
        );

        assert_eq!(
            model.entities.len(),
            2,
            "Expected 2 entities (cbus, entities), got {}",
            model.entities.len()
        );

        // Render to Mermaid.
        let mermaid = render_erd(&model, &options);

        println!(
            "Mermaid output ({} chars):\n{}",
            mermaid.len(),
            &mermaid[..mermaid.len().min(500)]
        );

        // Must start with erDiagram.
        assert!(
            mermaid.trim_start().starts_with("erDiagram"),
            "Mermaid output does not start with 'erDiagram':\n{mermaid}"
        );

        // Must contain the sanitized entity names (hyphens → underscores).
        assert!(
            mermaid.contains("ob_poc__cbus") || mermaid.contains("cbus"),
            "Mermaid output does not contain 'cbus' entity:\n{mermaid}"
        );

        assert!(!mermaid.is_empty(), "Mermaid output is empty");

        println!(
            "✓ ERD generation produced valid Mermaid output ({} chars)",
            mermaid.len()
        );
        Ok(())
    }
}
