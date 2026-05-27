//! Process definition registry — loads compiled BPMN process definitions at
//! startup and provides `start_instance` / `run_instance` for the session
//! pipeline and forms submit route.
//!
//! # Architecture
//! One `RuntimeEngine` per process definition, all sharing a single
//! `PostgresJourneyStore`. Instances of different process definitions are
//! discriminated by `dsl_workflow_instance.journey_name`.
//!
//! # Startup
//! `ProcessRegistry::load_all` queries `process_definitions WHERE enabled = TRUE`,
//! compiles each row via `dsl_migrate_verify::compile_to_spec`, registers the
//! `dsl.form` builtin, and stores `Arc<RuntimeEngine>` per name.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use bpmn_runtime::{register_builtins, JourneyStore, PostgresJourneyStore, RuntimeEngine, ScriptedAdaptor, VerbRegistry};
use dsl_migrate_verify::compile_to_spec;
use dsl_runtime::service_traits::ProcessRegistryService;
use ob_poc_types::chat::BpmnFormPending;
use sqlx::PgPool;
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

pub(crate) struct ProcessRegistry {
    /// One engine per process-definition name.
    engines: HashMap<String, Arc<RuntimeEngine>>,
    /// Shared durable store — also used by forms submit for direct lookups.
    pub(crate) store: Arc<PostgresJourneyStore>,
}

impl ProcessRegistry {
    /// Load all enabled process definitions from DB, compile, and build engines.
    /// Definitions that fail to compile are logged and skipped (server starts
    /// with a partial registry).
    pub(crate) async fn load_all(pool: PgPool) -> Result<Self> {
        let store = Arc::new(PostgresJourneyStore::new(pool.clone()));

        let rows = sqlx::query!(
            "SELECT name, dsl_source FROM process_definitions WHERE enabled = TRUE"
        )
        .fetch_all(&pool)
        .await?;

        let mut engines = HashMap::new();

        for row in rows {
            match compile_to_spec(&row.dsl_source, &row.name) {
                Ok(spec) => {
                    let mut registry = VerbRegistry::new();
                    register_builtins(&mut registry);
                    let engine = RuntimeEngine::new(
                        Arc::clone(&store) as Arc<dyn JourneyStore>,
                        Arc::new(spec),
                        Arc::new(registry),
                        Arc::new(ScriptedAdaptor::default()),
                    );
                    tracing::info!(process = %row.name, "ProcessRegistry: loaded process definition");
                    engines.insert(row.name, Arc::new(engine));
                }
                Err(e) => {
                    tracing::warn!(process = %row.name, error = %e, "ProcessRegistry: failed to compile, skipping");
                }
            }
        }

        tracing::info!(count = engines.len(), "ProcessRegistry: loaded {} process definition(s)", engines.len());
        Ok(Self { engines, store })
    }

    /// Empty registry for environments with no process definitions seeded.
    pub(crate) fn empty(pool: PgPool) -> Self {
        Self {
            engines: HashMap::new(),
            store: Arc::new(PostgresJourneyStore::new(pool)),
        }
    }

    /// Start a new instance of `process_name` and return its initial state.
    ///
    /// Returns `(instance_id, Option<BpmnFormPending>)` — the pending form if
    /// the process immediately parks at a human task.
    pub(crate) async fn start_instance(
        &self,
        process_name: &str,
        initial_data: serde_json::Value,
    ) -> Result<(Uuid, Option<BpmnFormPending>)> {
        let engine = self
            .engines
            .get(process_name)
            .ok_or_else(|| anyhow!("process '{}' not found in registry", process_name))?;

        let instance_id = engine.start_instance(initial_data).await?;
        let pending = self.find_pending_form(instance_id).await?;
        Ok((instance_id, pending))
    }

    /// Drain the event queue for `instance_id` and run to quiescence.
    /// Returns the next pending human task if the process re-parks.
    pub(crate) async fn run_instance(&self, instance_id: Uuid) -> Result<Option<BpmnFormPending>> {
        let engine = self.engine_for_instance(instance_id).await?;
        engine.run_to_quiescence(instance_id).await?;
        self.find_pending_form(instance_id).await
    }

    /// Look up which engine owns `instance_id` via `dsl_workflow_instance.journey_name`.
    async fn engine_for_instance(&self, instance_id: Uuid) -> Result<Arc<RuntimeEngine>> {
        let row = sqlx::query!(
            "SELECT journey_name FROM dsl_workflow_instance WHERE id = $1",
            instance_id
        )
        .fetch_optional(self.store.pool())
        .await?
        .ok_or_else(|| anyhow!("instance {} not found", instance_id))?;

        self.engines
            .get(&row.journey_name)
            .cloned()
            .ok_or_else(|| anyhow!("no engine for process '{}'", row.journey_name))
    }

    /// Query `dsl_pending_wait` for a parked human task on `instance_id`
    /// and deserialise its payload into `BpmnFormPending`.
    async fn find_pending_form(&self, instance_id: Uuid) -> Result<Option<BpmnFormPending>> {
        let row = sqlx::query!(
            r#"SELECT token_id, payload
               FROM dsl_pending_wait
               WHERE instance_id = $1 AND wait_kind = 'human_task'
               LIMIT 1"#,
            instance_id
        )
        .fetch_optional(self.store.pool())
        .await?;

        let Some(row) = row else { return Ok(None) };
        let Some(payload) = row.payload else { return Ok(None) };

        // payload = { form_ref, mode, prefill_data } as stored by DslFormHandler
        let form = BpmnFormPending {
            form_ref: payload
                .get("form_ref")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned(),
            token_id: row.token_id.to_string(),
            mode: payload
                .get("mode")
                .and_then(|v| v.as_str())
                .unwrap_or("capture")
                .to_owned(),
            prefill_data: payload
                .get("prefill_data")
                .cloned()
                .unwrap_or(serde_json::json!({})),
        };

        Ok(Some(form))
    }
}

/// Implement `ProcessRegistryService` so verb ops can call
/// `ctx.service::<dyn ProcessRegistryService>()?.start_process(...)`.
#[async_trait]
impl ProcessRegistryService for ProcessRegistry {
    async fn start_process(
        &self,
        process_name: &str,
        initial_data: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let (instance_id, pending) = self.start_instance(process_name, initial_data).await?;
        Ok(serde_json::json!({
            "instance_id": instance_id,
            "status": if pending.is_some() { "parked" } else { "running" },
            "bpmn_form": pending,
        }))
    }
}
