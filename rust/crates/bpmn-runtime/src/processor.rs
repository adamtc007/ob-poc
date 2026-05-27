//! Core event processor for the bpmn-lite runtime (§6.4).
//!
//! [`process_event`] is the single dispatch point. It loads state from the
//! store, performs one state transition, and writes back. No long-lived
//! threads or fibers are involved — this is pure hydrate/dehydrate.

use crate::{
    metrics::RuntimeMetrics,
    store::{JourneyLogEntry, JourneyStore},
    switch::{EdgeInfo, SwitchAdaptor, SwitchRequest},
    types::*,
    verb::{VerbContext, VerbEffect, VerbError, VerbRegistry},
};
use anyhow::Result;
use dsl_lowering::bpmn::{JourneyEdge, JourneyNode, JourneyParallelJoin, JourneySpec};
use std::collections::{BTreeMap, HashMap};

/// Everything the processor needs to handle one event.
pub struct RuntimeContext<'a> {
    pub store: &'a dyn JourneyStore,
    pub spec: &'a JourneySpec,
    pub verb_registry: &'a VerbRegistry,
    pub switch_adaptor: &'a dyn SwitchAdaptor,
    /// Shared metrics — processor increments granular counters here.
    pub metrics: &'a RuntimeMetrics,
}

/// Dispatch one event. This is the only public entry point.
pub async fn process_event(ctx: &RuntimeContext<'_>, event: &EventEnvelope) -> Result<()> {
    match &event.event_kind {
        EventKind::InstanceStart => handle_instance_start(ctx, event).await,
        EventKind::VerbCompletion => handle_verb_completion(ctx, event).await,
        EventKind::SwitchDecisionReply => handle_switch_reply(ctx, event).await,
        EventKind::HumanTaskComplete => handle_verb_completion(ctx, event).await,
        EventKind::TimerFired => handle_timer_fired(ctx, event).await,
        EventKind::MessageArrived => handle_verb_completion(ctx, event).await,
        EventKind::ErrorRaised => handle_error_raised(ctx, event).await,
        EventKind::SubProcessComplete => handle_verb_completion(ctx, event).await,
        EventKind::CancellationTriggered => handle_cancellation(ctx, event).await,
    }
}

// ---------------------------------------------------------------------------
// Event handlers
// ---------------------------------------------------------------------------

async fn handle_instance_start(ctx: &RuntimeContext<'_>, event: &EventEnvelope) -> Result<()> {
    let start_node = ctx.spec.start_node.clone();
    if start_node.is_empty() {
        tracing::warn!(instance_id = %event.instance_id, "no start node in spec");
        return Ok(());
    }

    let token = ctx
        .store
        .create_token(event.instance_id, &start_node, None, vec![])
        .await?;

    ctx.store
        .append_journey_log(JourneyLogEntry {
            instance_id: event.instance_id,
            token_id: Some(token.id),
            event_kind: "token_created".to_string(),
            from_node: None,
            to_node: Some(start_node.clone()),
            data_delta: Some(event.payload.clone()),
        })
        .await?;

    advance_token(ctx, event.instance_id, token.id, &start_node).await
}

async fn handle_verb_completion(ctx: &RuntimeContext<'_>, event: &EventEnvelope) -> Result<()> {
    let node_name = event.payload["node_name"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let token_id: TokenId = event.payload["token_id"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .unwrap_or_default();
    let output = event
        .payload
        .get("output_data")
        .cloned()
        .unwrap_or(serde_json::Value::Object(Default::default()));

    let token_opt = ctx
        .store
        .get_tokens_for_instance(event.instance_id)
        .await?
        .into_iter()
        .find(|t| t.id == token_id);
    let in_branch = token_opt.is_some_and(|t| !t.branch_lineage.is_empty());

    if let Some(obj) = output.as_object() {
        for (k, v) in obj {
            if !in_branch {
                ctx.store
                    .write_instance_data(event.instance_id, k, v.clone())
                    .await?;
            }
            ctx.store
                .append_to_write_log(
                    token_id,
                    WriteLogEntry {
                        location: k.clone(),
                        value: v.clone(),
                    },
                )
                .await?;
        }
    }
    complete_task(ctx, event.instance_id, token_id, &node_name, output).await
}

async fn handle_switch_reply(ctx: &RuntimeContext<'_>, event: &EventEnvelope) -> Result<()> {
    let token_id: TokenId = event.payload["token_id"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .unwrap_or_default();
    let selected: Vec<String> = event.payload["selected_targets"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    for target in selected {
        ctx.store.advance_token(token_id, &target).await?;
        advance_token_boxed(ctx, event.instance_id, token_id, &target).await?;
    }
    Ok(())
}

async fn handle_timer_fired(ctx: &RuntimeContext<'_>, event: &EventEnvelope) -> Result<()> {
    let token_id: TokenId = event.payload["token_id"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .unwrap_or_default();
    let node_name = event.payload["node_name"]
        .as_str()
        .unwrap_or("")
        .to_string();
    RuntimeMetrics::increment(&ctx.metrics.timer_events_fired);
    complete_task(
        ctx,
        event.instance_id,
        token_id,
        &node_name,
        serde_json::json!({}),
    )
    .await
}

async fn handle_error_raised(ctx: &RuntimeContext<'_>, event: &EventEnvelope) -> Result<()> {
    ctx.store
        .update_instance_status(
            event.instance_id,
            InstanceStatus::Failed,
            Some(chrono::Utc::now()),
        )
        .await?;
    ctx.store
        .append_journey_log(JourneyLogEntry {
            instance_id: event.instance_id,
            token_id: None,
            event_kind: "instance_failed".to_string(),
            from_node: event.payload["node"].as_str().map(String::from),
            to_node: None,
            data_delta: Some(event.payload.clone()),
        })
        .await
}

async fn handle_cancellation(ctx: &RuntimeContext<'_>, event: &EventEnvelope) -> Result<()> {
    ctx.store
        .update_instance_status(
            event.instance_id,
            InstanceStatus::Cancelled,
            Some(chrono::Utc::now()),
        )
        .await
}

// ---------------------------------------------------------------------------
// Token advancement
// ---------------------------------------------------------------------------

/// Wrapper to enable recursive async calls via Box::pin (required because Rust
/// cannot have directly recursive async fns without boxing the future).
async fn advance_token_boxed(
    ctx: &RuntimeContext<'_>,
    instance_id: InstanceId,
    token_id: TokenId,
    node_name: &str,
) -> Result<()> {
    Box::pin(advance_token(ctx, instance_id, token_id, node_name)).await
}

/// Advance token `token_id` from its current position at `current_node`.
///
/// The token advances through non-blocking nodes automatically and stops at:
/// - A task node (verb invocation required or token left waiting)
/// - A decision gateway (switch adaptor consulted)
/// - An end-event (instance completed)
async fn advance_token(
    ctx: &RuntimeContext<'_>,
    instance_id: InstanceId,
    token_id: TokenId,
    current_node: &str,
) -> Result<()> {
    let node = match find_node(ctx.spec, current_node) {
        Some(n) => n.clone(),
        None => {
            tracing::warn!(%instance_id, %token_id, node = %current_node,
                "advance_token: node not found in spec");
            return Ok(());
        }
    };

    match node.kind.as_str() {
        // --- Start events: auto-advance to first task ---
        k if is_start_event(k) => {
            if let Some(next) = single_outgoing(ctx.spec, current_node) {
                ctx.store.advance_token(token_id, &next).await?;
                advance_token_boxed(ctx, instance_id, token_id, &next).await?;
            }
        }

        // --- Task nodes: invoke verb or leave token waiting ---
        k if is_task_kind(k) => {
            invoke_verb_for_task(ctx, instance_id, token_id, &node).await?;
        }

        // --- Decision gateways: consult switch adaptor ---
        "exclusive" | "event-based" | "parallel-event-based" => {
            handle_decision_gateway(ctx, instance_id, token_id, &node).await?;
        }

        // --- Inclusive gateway: dynamic fan-out with join count tracking ---
        "inclusive" => {
            handle_inclusive_fork(ctx, instance_id, token_id, current_node).await?;
        }

        // --- Parallel fork: create N child tokens ---
        "parallel" => {
            handle_parallel_fork(ctx, instance_id, token_id, current_node).await?;
        }

        // --- Parallel join: accumulate; fire when all branches arrived ---
        "parallel-join" => {
            handle_join_arrival(ctx, instance_id, token_id, current_node).await?;
        }

        // --- End events: complete the instance ---
        k if is_end_event(k) => {
            handle_end_event(ctx, instance_id, token_id, current_node).await?;
        }

        other => {
            tracing::debug!(
                %instance_id, %token_id, kind = %other,
                "token at unhandled node kind — leaving in place"
            );
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Gateway handlers
// ---------------------------------------------------------------------------

async fn handle_decision_gateway(
    ctx: &RuntimeContext<'_>,
    instance_id: InstanceId,
    token_id: TokenId,
    node: &JourneyNode,
) -> Result<()> {
    let outgoing: Vec<EdgeInfo> = outgoing_edges(ctx.spec, &node.name)
        .into_iter()
        .map(|e| EdgeInfo {
            target: e.target.clone(),
            condition: e.condition.clone(),
            is_default: e.is_default,
        })
        .collect();

    // INVARIANT (C1): Gateway evaluation currently receives an empty context `json!({})`.
    // If this ever changes to include `instance_data`, the C1 parallel state isolation fix
    // (deferring branch writes to `write_log` instead of global state) MUST be updated to Variant B
    // (overlaying the branch's write_log over global state) so gateways can read in-flight branch writes.
    let request = SwitchRequest {
        instance_id,
        gateway_name: node.name.clone(),
        gateway_kind: node.kind.clone(),
        context_data: serde_json::json!({}),
        outgoing_edges: outgoing,
    };

    match ctx.switch_adaptor.handle(request).await {
        Ok(reply) => {
            let targets = reply.selected_targets;

            // C4: Gateway Multiplicity Policy Check
            if let Err(violation) = check_gateway_multiplicity(&node.kind, targets.len()) {
                let error_msg = match violation {
                    GatewayViolation::ExpectedExactlyOne(got) => {
                        format!("expected exactly 1 target, got {}", got)
                    }
                    GatewayViolation::ExpectedAtLeastOne => {
                        "expected at least 1 target, got 0".to_string()
                    }
                    GatewayViolation::UnknownKind(k) => {
                        format!("unknown or unsupported gateway kind '{}'", k)
                    }
                };
                tracing::error!(%instance_id, gateway = %node.name, kind = %node.kind, "gateway {}", error_msg);
                ctx.store
                    .append_journey_log(JourneyLogEntry {
                        instance_id,
                        token_id: Some(token_id),
                        event_kind: "gateway_dead_end".to_string(),
                        from_node: Some(node.name.clone()),
                        to_node: None,
                        data_delta: Some(serde_json::json!({"error": error_msg})),
                    })
                    .await?;
                ctx.store
                    .update_instance_status(
                        instance_id,
                        InstanceStatus::Failed,
                        Some(chrono::Utc::now()),
                    )
                    .await?;
                return Ok(());
            }

            RuntimeMetrics::increment(&ctx.metrics.gateway_decisions);
            ctx.store
                .append_journey_log(JourneyLogEntry {
                    instance_id,
                    token_id: Some(token_id),
                    event_kind: "gateway_decided".to_string(),
                    from_node: Some(node.name.clone()),
                    to_node: Some(targets.join(",")),
                    data_delta: None,
                })
                .await?;

            for (i, target) in targets.iter().enumerate() {
                if i == 0 {
                    // Reuse the existing token for the first target.
                    ctx.store.advance_token(token_id, target).await?;
                    advance_token_boxed(ctx, instance_id, token_id, target).await?;
                } else {
                    // Create a new token for each additional target (inclusive gateway).
                    let new_token = ctx
                        .store
                        .create_token(instance_id, target, None, vec![])
                        .await?;
                    advance_token_boxed(ctx, instance_id, new_token.id, target).await?;
                }
            }
        }
        Err(e) => {
            tracing::error!(%instance_id, gateway = %node.name, "switch adaptor error: {}", e);
            ctx.store
                .update_instance_status(
                    instance_id,
                    InstanceStatus::Failed,
                    Some(chrono::Utc::now()),
                )
                .await?;
        }
    }
    Ok(())
}

async fn handle_parallel_fork(
    ctx: &RuntimeContext<'_>,
    instance_id: InstanceId,
    token_id: TokenId,
    gateway_name: &str,
) -> Result<()> {
    let outgoing = outgoing_edges(ctx.spec, gateway_name);
    // Consume the parent token.
    ctx.store.delete_token(token_id).await?;
    RuntimeMetrics::increment(&ctx.metrics.parallel_forks);

    ctx.store
        .append_journey_log(JourneyLogEntry {
            instance_id,
            token_id: Some(token_id),
            event_kind: "parallel_fork".to_string(),
            from_node: Some(gateway_name.to_string()),
            to_node: Some(
                outgoing
                    .iter()
                    .map(|e| e.target.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
            ),
            data_delta: None,
        })
        .await?;

    for edge in &outgoing {
        let child = ctx
            .store
            .create_token(
                instance_id,
                &edge.target,
                Some(token_id), // fork_ref retains original token id (historical)
                vec![gateway_name.to_string()], // branch_lineage[0] = fork gateway name
            )
            .await?;
        advance_token_boxed(ctx, instance_id, child.id, &edge.target).await?;
    }
    Ok(())
}

/// Handle an inclusive gateway fork: consult the switch adaptor to pick which
/// branches to activate, then store the dynamic expected count on all matching
/// parallel-join declarations so the join knows how many arrivals to wait for.
async fn handle_inclusive_fork(
    ctx: &RuntimeContext<'_>,
    instance_id: InstanceId,
    token_id: TokenId,
    gateway_name: &str,
) -> Result<()> {
    let outgoing: Vec<EdgeInfo> = outgoing_edges(ctx.spec, gateway_name)
        .into_iter()
        .map(|e| EdgeInfo {
            target: e.target.clone(),
            condition: e.condition.clone(),
            is_default: e.is_default,
        })
        .collect();

    let request = crate::switch::SwitchRequest {
        instance_id,
        gateway_name: gateway_name.to_string(),
        gateway_kind: "inclusive".to_string(),
        context_data: serde_json::json!({}),
        outgoing_edges: outgoing,
    };

    match ctx.switch_adaptor.handle(request).await {
        Ok(reply) => {
            let selected = reply.selected_targets;
            let branch_count = selected.len();

            ctx.store
                .append_journey_log(JourneyLogEntry {
                    instance_id,
                    token_id: Some(token_id),
                    event_kind: "inclusive_fork".to_string(),
                    from_node: Some(gateway_name.to_string()),
                    to_node: Some(selected.join(",")),
                    data_delta: Some(serde_json::json!({ "branch_count": branch_count })),
                })
                .await?;

            // Store dynamic expected count on every join that expects this fork.
            for pj in ctx
                .spec
                .parallel_joins
                .iter()
                .filter(|j| j.expects.contains(&gateway_name.to_string()))
            {
                ctx.store
                    .set_expected_join_count(&pj.name, instance_id, branch_count)
                    .await?;
            }

            // Delete the parent token and spawn one child per selected branch.
            ctx.store.delete_token(token_id).await?;

            for (i, target) in selected.iter().enumerate() {
                let child = ctx
                    .store
                    .create_token(
                        instance_id,
                        target,
                        Some(token_id),
                        vec![gateway_name.to_string()],
                    )
                    .await?;
                let _ = i; // all targets get fresh tokens
                advance_token_boxed(ctx, instance_id, child.id, target).await?;
            }
        }
        Err(e) => {
            tracing::error!(%instance_id, gateway = %gateway_name, "inclusive fork switch error: {}", e);
            ctx.store
                .update_instance_status(
                    instance_id,
                    InstanceStatus::Failed,
                    Some(chrono::Utc::now()),
                )
                .await?;
        }
    }
    Ok(())
}

async fn handle_join_arrival(
    ctx: &RuntimeContext<'_>,
    instance_id: InstanceId,
    token_id: TokenId,
    join_name: &str,
) -> Result<()> {
    // Move the token to the join node so get_tokens_at_join can find it.
    ctx.store.advance_token(token_id, join_name).await?;

    // Record this arrival.
    let arrivals = ctx
        .store
        .record_join_arrival(join_name, instance_id, token_id)
        .await?;

    // Determine how many branches we expect.
    let join_spec = ctx.spec.parallel_joins.iter().find(|j| j.name == join_name);
    let expected_count = resolve_expected_count(ctx, join_name, instance_id, join_spec).await?;

    ctx.store
        .append_journey_log(JourneyLogEntry {
            instance_id,
            token_id: Some(token_id),
            event_kind: "join_token_arrived".to_string(),
            from_node: None,
            to_node: Some(join_name.to_string()),
            data_delta: Some(serde_json::json!({
                "arrivals": arrivals,
                "expected": expected_count
            })),
        })
        .await?;

    if expected_count > 0 && arrivals >= expected_count {
        fire_join(ctx, instance_id, token_id, join_name, join_spec).await?;
    }
    Ok(())
}

/// Determine how many arrivals to wait for at `join_name`.
async fn resolve_expected_count(
    ctx: &RuntimeContext<'_>,
    join_name: &str,
    instance_id: InstanceId,
    join_spec: Option<&JourneyParallelJoin>,
) -> Result<usize> {
    // Check for dynamic count first (set by inclusive gateway fork).
    if let Some(dynamic) = ctx
        .store
        .get_expected_join_count(join_name, instance_id)
        .await?
    {
        return Ok(dynamic);
    }
    // Fall back to static: count outgoing edges from each fork gateway in `expects`.
    if let Some(js) = join_spec {
        let count: usize = js
            .expects
            .iter()
            .map(|fork_name| outgoing_edges(ctx.spec, fork_name).len())
            .sum();
        return Ok(count);
    }
    Ok(0)
}

/// All expected branches have arrived: apply merge protocol, clean up branch
/// tokens, and continue with a fresh unified token.
async fn fire_join(
    ctx: &RuntimeContext<'_>,
    instance_id: InstanceId,
    arriving_token_id: TokenId,
    join_name: &str,
    join_spec: Option<&JourneyParallelJoin>,
) -> Result<()> {
    // Collect all branch tokens sitting at this join.
    let branch_tokens = ctx.store.get_tokens_at_join(join_name, instance_id).await?;

    // Apply the merge protocol.
    match apply_merge_protocol(&branch_tokens, join_spec) {
        MergeResult::Ok(merged_data) => {
            for (key, val) in merged_data {
                ctx.store
                    .write_instance_data(instance_id, &key, val)
                    .await?;
            }
        }
        MergeResult::Conflict { location, values } => {
            RuntimeMetrics::increment(&ctx.metrics.merge_conflicts);
            ctx.store
                .append_journey_log(JourneyLogEntry {
                    instance_id,
                    token_id: Some(arriving_token_id),
                    event_kind: "merge_conflict".to_string(),
                    from_node: Some(join_name.to_string()),
                    to_node: None,
                    data_delta: Some(serde_json::json!({
                        "location": location,
                        "conflicting_values": values,
                    })),
                })
                .await?;
            ctx.store
                .update_instance_status(
                    instance_id,
                    InstanceStatus::Failed,
                    Some(chrono::Utc::now()),
                )
                .await?;
            return Ok(());
        }
    }

    // Delete all branch tokens at the join.
    for t in &branch_tokens {
        ctx.store.delete_token(t.id).await?;
    }

    RuntimeMetrics::increment(&ctx.metrics.joins_fired);
    ctx.store
        .append_journey_log(JourneyLogEntry {
            instance_id,
            token_id: Some(arriving_token_id),
            event_kind: "join_fired".to_string(),
            from_node: None,
            to_node: Some(join_name.to_string()),
            data_delta: None,
        })
        .await?;

    // Continue with a fresh unified token.
    if let Some(next) = single_outgoing(ctx.spec, join_name) {
        let continuation = ctx
            .store
            .create_token(instance_id, &next, None, vec![])
            .await?;
        advance_token_boxed(ctx, instance_id, continuation.id, &next).await?;
    }
    Ok(())
}

async fn handle_end_event(
    ctx: &RuntimeContext<'_>,
    instance_id: InstanceId,
    token_id: TokenId,
    node_name: &str,
) -> Result<()> {
    // Token-death short-circuit: if this is a branch token that terminated
    // before reaching a join, reduce the expected count for any join that
    // expects the branch's fork gateway.
    let token_opt = ctx
        .store
        .get_tokens_for_instance(instance_id)
        .await?
        .into_iter()
        .find(|t| t.id == token_id);

    if let Some(ref token) = token_opt {
        if let Some(fork_gateway_name) = token.branch_lineage.first() {
            let fork_name = fork_gateway_name.clone();
            // Find joins that expect this fork gateway.
            let matching_joins: Vec<String> = ctx
                .spec
                .parallel_joins
                .iter()
                .filter(|j| j.expects.contains(&fork_name))
                .map(|j| j.name.clone())
                .collect();

            for join_name in matching_joins {
                // Only short-circuit if the join hasn't fired yet (check if
                // there are still living branch tokens for this fork).
                let new_expected = ctx
                    .store
                    .reduce_expected_join_count(&join_name, instance_id)
                    .await?;
                let arrivals = ctx
                    .store
                    .record_join_arrival(&join_name, instance_id, token_id)
                    .await?;

                let join_spec = ctx.spec.parallel_joins.iter().find(|j| j.name == join_name);
                if new_expected > 0 && arrivals >= new_expected {
                    // All remaining branches have arrived — fire the join.
                    ctx.store.advance_token(token_id, &join_name).await?;
                    fire_join(ctx, instance_id, token_id, &join_name, join_spec).await?;
                    return Ok(());
                }
            }
        }
    }

    ctx.store.delete_token(token_id).await?;
    ctx.store
        .append_journey_log(JourneyLogEntry {
            instance_id,
            token_id: Some(token_id),
            event_kind: "instance_completed".to_string(),
            from_node: Some(node_name.to_string()),
            to_node: None,
            data_delta: None,
        })
        .await?;
    ctx.store
        .update_instance_status(
            instance_id,
            InstanceStatus::Completed,
            Some(chrono::Utc::now()),
        )
        .await
}

// ---------------------------------------------------------------------------
// Verb invocation
// ---------------------------------------------------------------------------

async fn invoke_verb_for_task(
    ctx: &RuntimeContext<'_>,
    instance_id: InstanceId,
    token_id: TokenId,
    node: &JourneyNode,
) -> Result<()> {
    let verb_ref = match node.verb_ref.as_deref() {
        Some(v) if !v.is_empty() => v.to_string(),
        _ => {
            // No verb bound to this node: treat as a synchronous pass-through.
            return complete_task(
                ctx,
                instance_id,
                token_id,
                &node.name,
                serde_json::json!({}),
            )
            .await;
        }
    };

    if let Some(handler) = ctx.verb_registry.get(&verb_ref) {
        RuntimeMetrics::increment(&ctx.metrics.verbs_invoked);
        let verb_ctx = VerbContext {
            at_slots: BTreeMap::new(),
            inputs: BTreeMap::new(),
            outputs: BTreeMap::new(),
            effects: Vec::new(),
            token_id,
            instance_id,
        };
        match handler.invoke(verb_ctx).await {
            Ok(output) => {
                let token_opt = ctx
                    .store
                    .get_tokens_for_instance(instance_id)
                    .await?
                    .into_iter()
                    .find(|t| t.id == token_id);
                let in_branch = token_opt.is_some_and(|t| !t.branch_lineage.is_empty());

                for (k, v) in &output.data {
                    if !in_branch {
                        ctx.store
                            .write_instance_data(instance_id, k, v.clone())
                            .await?;
                    }
                    ctx.store
                        .append_to_write_log(
                            token_id,
                            WriteLogEntry {
                                location: k.clone(),
                                value: v.clone(),
                            },
                        )
                        .await?;
                }
                // Dispatch effects. RequestHumanTask parks the fiber instead of
                // completing it — the token waits for a HumanTaskComplete event.
                let mut human_task_parked = false;
                for effect in &output.effects {
                    if let VerbEffect::RequestHumanTask { role: _, form_data } = effect {
                        // Clone payload before form_data is moved into the log entry below.
                        let payload = Some(form_data.clone());
                        // correlation_key = token_id so HumanTaskComplete can address it
                        ctx.store
                            .create_pending_wait(
                                instance_id,
                                token_id,
                                "human_task",
                                &node.name,
                                Some(token_id.to_string()),
                                None,
                                payload,
                            )
                            .await?;
                        ctx.store
                            .append_journey_log(JourneyLogEntry {
                                instance_id,
                                token_id: Some(token_id),
                                event_kind: "human_task_pending".to_string(),
                                from_node: None,
                                to_node: Some(node.name.clone()),
                                data_delta: Some(form_data.clone()),
                            })
                            .await?;
                        human_task_parked = true;
                        break;
                    }
                }
                if !human_task_parked {
                    let output_value = serde_json::to_value(&output.data)?;
                    complete_task(ctx, instance_id, token_id, &node.name, output_value).await?;
                }
            }
            Err(VerbError::Domain { code, message }) => {
                ctx.store
                    .enqueue_event(
                        instance_id,
                        EventKind::ErrorRaised,
                        serde_json::json!({
                            "node": node.name,
                            "code": code,
                            "message": message,
                            "token_id": token_id.to_string(),
                        }),
                    )
                    .await?;
            }
            Err(e) => return Err(e.into()),
        }
    } else {
        // Verb not registered: leave token waiting for an external VerbCompletion event.
        ctx.store
            .create_pending_wait(instance_id, token_id, "verb", &node.name, None, None, None)
            .await?;
        ctx.store
            .append_journey_log(JourneyLogEntry {
                instance_id,
                token_id: Some(token_id),
                event_kind: "token_waiting".to_string(),
                from_node: None,
                to_node: Some(node.name.clone()),
                data_delta: None,
            })
            .await?;
    }
    Ok(())
}

async fn complete_task(
    ctx: &RuntimeContext<'_>,
    instance_id: InstanceId,
    token_id: TokenId,
    node_name: &str,
    output_data: serde_json::Value,
) -> Result<()> {
    ctx.store
        .append_journey_log(JourneyLogEntry {
            instance_id,
            token_id: Some(token_id),
            event_kind: "task_completed".to_string(),
            from_node: Some(node_name.to_string()),
            to_node: None,
            data_delta: Some(output_data),
        })
        .await?;

    if let Some(next) = single_outgoing(ctx.spec, node_name) {
        ctx.store.advance_token(token_id, &next).await?;
        advance_token_boxed(ctx, instance_id, token_id, &next).await?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Merge protocol
// ---------------------------------------------------------------------------

/// Result of applying the merge protocol to arriving branch tokens.
pub enum MergeResult {
    /// All conflicts were resolved; map contains the final values to write.
    Ok(HashMap<String, serde_json::Value>),
    /// An unresolvable conflict was detected (no merge clause, different values).
    Conflict {
        location: String,
        values: Vec<serde_json::Value>,
    },
}

/// Collect write-logs from all branch tokens and resolve conflicts using the
/// join's merge clauses. Returns `MergeResult::Conflict` on the first
/// unresolvable conflict.
pub fn apply_merge_protocol(
    tokens: &[ActiveToken],
    join_spec: Option<&JourneyParallelJoin>,
) -> MergeResult {
    // Collect the latest write from each token grouped by location.
    let mut writes_by_location: HashMap<String, Vec<serde_json::Value>> = HashMap::new();
    for token in tokens {
        // Find the last write to each location by this token
        let mut token_latest: HashMap<String, serde_json::Value> = HashMap::new();
        for entry in &token.write_log {
            token_latest.insert(entry.location.clone(), entry.value.clone());
        }
        for (location, value) in token_latest {
            writes_by_location.entry(location).or_default().push(value);
        }
    }

    let mut merged: HashMap<String, serde_json::Value> = HashMap::new();

    for (location, values) in writes_by_location {
        let merge_op = join_spec.and_then(|j| j.merge.iter().find(|m| m.location == location));
        match merge_op {
            Some(clause) => match apply_merge_operator(&clause.operator, values.clone()) {
                Ok(v) => {
                    merged.insert(location, v);
                }
                Err(_) => {
                    return MergeResult::Conflict { location, values };
                }
            },
            None => {
                // If there's no merge operator, require all values to be exactly the same
                // or just one branch wrote it.
                if values.len() == 1 || values.windows(2).all(|w| w[0] == w[1]) {
                    merged.insert(location, values.into_iter().next().unwrap());
                } else {
                    return MergeResult::Conflict { location, values };
                }
            }
        }
    }
    MergeResult::Ok(merged)
}

fn apply_merge_operator(
    operator: &str,
    values: Vec<serde_json::Value>,
) -> Result<serde_json::Value, ()> {
    if values.is_empty() {
        return Ok(serde_json::Value::Null);
    }
    match operator {
        "latest" => Ok(values.into_iter().last().unwrap_or(serde_json::Value::Null)),
        "union" => {
            let mut strings = Vec::new();
            for v in values {
                if let Some(s) = v.as_str() {
                    strings.push(serde_json::Value::String(s.to_string()));
                } else {
                    return Err(()); // Type mismatch
                }
            }
            Ok(serde_json::Value::Array(strings))
        }
        "max" => {
            let mut max = f64::NEG_INFINITY;
            for v in values {
                if let Some(f) = v.as_f64() {
                    if f > max {
                        max = f;
                    }
                } else {
                    return Err(());
                }
            }
            Ok(serde_json::json!(max))
        }
        "min" => {
            let mut min = f64::INFINITY;
            for v in values {
                if let Some(f) = v.as_f64() {
                    if f < min {
                        min = f;
                    }
                } else {
                    return Err(());
                }
            }
            Ok(serde_json::json!(min))
        }
        "sum" => {
            let mut sum: f64 = 0.0;
            for v in values {
                if let Some(f) = v.as_f64() {
                    sum += f;
                } else {
                    return Err(()); // Type mismatch
                }
            }
            Ok(serde_json::json!(sum))
        }
        "concat" => {
            let mut parts = Vec::new();
            for v in values {
                if let Some(s) = v.as_str() {
                    parts.push(s.to_string());
                } else {
                    return Err(());
                }
            }
            Ok(serde_json::Value::String(parts.join("")))
        }
        _ => Ok(values.into_iter().last().unwrap_or(serde_json::Value::Null)),
    }
}

// ---------------------------------------------------------------------------
// Spec helpers
// ---------------------------------------------------------------------------

fn find_node<'a>(spec: &'a JourneySpec, name: &str) -> Option<&'a JourneyNode> {
    spec.nodes.iter().find(|n| n.name == name)
}

fn outgoing_edges<'a>(spec: &'a JourneySpec, source: &str) -> Vec<&'a JourneyEdge> {
    spec.edges.iter().filter(|e| e.source == source).collect()
}

fn single_outgoing(spec: &JourneySpec, source: &str) -> Option<String> {
    let edges: Vec<_> = spec.edges.iter().filter(|e| e.source == source).collect();
    if edges.len() == 1 {
        Some(edges[0].target.clone())
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Node-kind predicates
// ---------------------------------------------------------------------------

fn is_start_event(k: &str) -> bool {
    k == "start-event" || k.starts_with("start-event-")
}

fn is_task_kind(k: &str) -> bool {
    matches!(
        k,
        "service-task"
            | "user-task"
            | "send-task"
            | "receive-task"
            | "manual-task"
            | "business-rule-task"
            | "script-task"
            | "call-activity"
            | "subprocess"
    ) || is_intermediate_event(k)
}

/// Intermediate events (catch/throw) are treated as pass-through tasks when
/// no verb is registered. They receive a token, log it, and advance.
fn is_intermediate_event(k: &str) -> bool {
    k.starts_with("intermediate-")
}

fn is_end_event(k: &str) -> bool {
    k == "end-event" || k.starts_with("end-event-")
}

// ---------------------------------------------------------------------------
// Gateway Multiplicity Policy
// ---------------------------------------------------------------------------

pub enum GatewayViolation {
    ExpectedExactlyOne(usize),
    ExpectedAtLeastOne,
    UnknownKind(String),
}

pub fn check_gateway_multiplicity(kind: &str, target_count: usize) -> Result<(), GatewayViolation> {
    match kind {
        "exclusive" | "event-based" => {
            if target_count != 1 {
                Err(GatewayViolation::ExpectedExactlyOne(target_count))
            } else {
                Ok(())
            }
        }
        "parallel-event-based" => {
            if target_count == 0 {
                Err(GatewayViolation::ExpectedAtLeastOne)
            } else {
                Ok(())
            }
        }
        _ => Err(GatewayViolation::UnknownKind(kind.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_c4_gateway_multiplicity_policy() {
        // exclusive
        assert!(matches!(
            check_gateway_multiplicity("exclusive", 0),
            Err(GatewayViolation::ExpectedExactlyOne(0))
        ));
        assert!(matches!(check_gateway_multiplicity("exclusive", 1), Ok(())));
        assert!(matches!(
            check_gateway_multiplicity("exclusive", 2),
            Err(GatewayViolation::ExpectedExactlyOne(2))
        ));

        // event-based
        assert!(matches!(
            check_gateway_multiplicity("event-based", 0),
            Err(GatewayViolation::ExpectedExactlyOne(0))
        ));
        assert!(matches!(
            check_gateway_multiplicity("event-based", 1),
            Ok(())
        ));
        assert!(matches!(
            check_gateway_multiplicity("event-based", 2),
            Err(GatewayViolation::ExpectedExactlyOne(2))
        ));

        // parallel-event-based
        assert!(matches!(
            check_gateway_multiplicity("parallel-event-based", 0),
            Err(GatewayViolation::ExpectedAtLeastOne)
        ));
        assert!(matches!(
            check_gateway_multiplicity("parallel-event-based", 1),
            Ok(())
        ));
        assert!(matches!(
            check_gateway_multiplicity("parallel-event-based", 2),
            Ok(())
        ));

        // unknown/unsupported gateway kind (fail closed)
        assert!(matches!(
            check_gateway_multiplicity("unknown-future-gateway", 0),
            Err(GatewayViolation::UnknownKind(_))
        ));
        assert!(matches!(
            check_gateway_multiplicity("unknown-future-gateway", 5),
            Err(GatewayViolation::UnknownKind(_))
        ));
    }
}
