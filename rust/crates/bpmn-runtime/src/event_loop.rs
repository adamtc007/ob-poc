//! Main event loop / engine facade for the bpmn-lite runtime (§6.4).
//!
//! [`RuntimeEngine`] is the public API surface. It holds the spec, store, verb
//! registry, and switch adaptor as shared references and provides the
//! hydrate/dehydrate event-loop via [`RuntimeEngine::run_to_quiescence`].

use crate::{
    processor::{process_event, RuntimeContext},
    store::JourneyStore,
    switch::SwitchAdaptor,
    types::{ActiveToken, EventKind, InstanceId, InstanceStatus},
    verb::VerbRegistry,
};
use anyhow::Result;
use dsl_lowering::bpmn::JourneySpec;
use std::sync::Arc;
use uuid::Uuid;

/// The runtime engine. Cheap to clone — all fields are `Arc`.
pub struct RuntimeEngine {
    pub store: Arc<dyn JourneyStore>,
    pub spec: Arc<JourneySpec>,
    pub verb_registry: Arc<VerbRegistry>,
    pub switch_adaptor: Arc<dyn SwitchAdaptor>,
}

impl RuntimeEngine {
    pub fn new(
        store: Arc<dyn JourneyStore>,
        spec: Arc<JourneySpec>,
        verb_registry: Arc<VerbRegistry>,
        switch_adaptor: Arc<dyn SwitchAdaptor>,
    ) -> Self {
        Self { store, spec, verb_registry, switch_adaptor }
    }

    /// Start a new process instance and run until no more events remain.
    ///
    /// Returns the new instance ID.
    pub async fn start_instance(
        &self,
        initial_data: serde_json::Value,
    ) -> Result<InstanceId> {
        let inst = self.store.create_instance(&self.spec.name, initial_data.clone()).await?;
        self.store
            .enqueue_event(inst.id, EventKind::InstanceStart, initial_data)
            .await?;
        self.run_to_quiescence(inst.id).await?;
        Ok(inst.id)
    }

    /// Deliver an external verb-completion result and run to quiescence.
    pub async fn complete_task(
        &self,
        instance_id: InstanceId,
        node_name: &str,
        token_id: Uuid,
        output: serde_json::Value,
    ) -> Result<()> {
        self.store
            .enqueue_event(
                instance_id,
                EventKind::VerbCompletion,
                serde_json::json!({
                    "node_name": node_name,
                    "token_id": token_id.to_string(),
                    "output_data": output,
                }),
            )
            .await?;
        self.run_to_quiescence(instance_id).await
    }

    /// Deliver a gateway switch reply and run to quiescence.
    pub async fn gateway_reply(
        &self,
        instance_id: InstanceId,
        gateway_name: &str,
        token_id: Uuid,
        selected_targets: Vec<String>,
    ) -> Result<()> {
        self.store
            .enqueue_event(
                instance_id,
                EventKind::SwitchDecisionReply,
                serde_json::json!({
                    "gateway_name": gateway_name,
                    "token_id": token_id.to_string(),
                    "selected_targets": selected_targets,
                }),
            )
            .await?;
        self.run_to_quiescence(instance_id).await
    }

    /// Fire a timer for the given token/node and run to quiescence.
    pub async fn fire_timer(
        &self,
        instance_id: InstanceId,
        token_id: Uuid,
        node_name: &str,
    ) -> Result<()> {
        self.store
            .enqueue_event(
                instance_id,
                EventKind::TimerFired,
                serde_json::json!({
                    "token_id": token_id.to_string(),
                    "node_name": node_name,
                }),
            )
            .await?;
        self.run_to_quiescence(instance_id).await
    }

    // --- Query helpers ---

    pub async fn get_instance_status(
        &self,
        id: InstanceId,
    ) -> Result<Option<InstanceStatus>> {
        Ok(self.store.get_instance(id).await?.map(|i| i.status))
    }

    pub async fn get_tokens(&self, id: InstanceId) -> Result<Vec<ActiveToken>> {
        self.store.get_tokens_for_instance(id).await
    }

    // --- Internal ---

    /// Drain the event queue for `instance_id` until it is empty.
    pub(crate) async fn run_to_quiescence(&self, instance_id: InstanceId) -> Result<()> {
        loop {
            let events = self.store.dequeue_events(10).await?;
            if events.is_empty() {
                break;
            }
            for event in events {
                if event.instance_id != instance_id {
                    // Re-queue events that belong to a different instance.
                    self.store
                        .enqueue_event(
                            event.instance_id,
                            event.event_kind,
                            event.payload,
                        )
                        .await?;
                    continue;
                }
                let ctx = RuntimeContext {
                    store: self.store.as_ref(),
                    spec: &self.spec,
                    verb_registry: &self.verb_registry,
                    switch_adaptor: self.switch_adaptor.as_ref(),
                };
                process_event(&ctx, &event).await?;
                self.store.ack_event(event.id).await?;
            }
        }
        Ok(())
    }
}
