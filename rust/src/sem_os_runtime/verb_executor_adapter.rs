//! Adapter implementing `sem_os_core::execution::VerbExecutionPort` over
//! the existing `DslExecutor` dispatch chain.
//!
//! This is the bridge between SemOS's execution contract and ob-poc's
//! concrete verb execution infrastructure (CustomOperationRegistry +
//! GenericCrudExecutor). It translates:
//!
//! - `VerbExecutionContext` ↔ `dsl_v2::ExecutionContext` (30-field)
//! - `serde_json::Value` args → `VerbCall` with `Argument` list
//! - `dsl_v2::ExecutionResult` → `VerbExecutionOutcome`
//! - pending_* side-channel state → `VerbSideEffects.platform_state`

use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use sem_os_core::execution::{
    VerbExecutionContext, VerbExecutionOutcome, VerbExecutionPort, VerbExecutionResult,
    VerbSideEffects,
};
use sem_os_core::error::SemOsError;

use crate::dsl_v2::executor::{DslExecutor, ExecutionContext, ExecutionResult};
use dsl_core::ast::{Argument, AstNode, Literal, Span, VerbCall};

/// Adapter implementing the SemOS execution port over ob-poc's DslExecutor.
///
/// Routes verb execution based on contract behavior:
/// - **CRUD** → `CrudExecutionPort` when available, otherwise DslExecutor fallback
/// - **Plugin** → DslExecutor (CustomOperationRegistry)
/// - **GraphQuery/Durable** → DslExecutor
pub struct ObPocVerbExecutor {
    executor: Arc<DslExecutor>,
    /// Optional SemOS-native CRUD executor. When set, CRUD verbs bypass
    /// the GenericCrudExecutor and route through the SemOS contract.
    /// Set via `with_crud_port()`. None = all verbs go through DslExecutor.
    crud_port: Option<Arc<dyn sem_os_core::execution::CrudExecutionPort>>,
}

impl ObPocVerbExecutor {
    pub fn new(executor: Arc<DslExecutor>) -> Self {
        Self {
            executor,
            crud_port: None,
        }
    }

    /// Create an executor from a database pool.
    ///
    /// Constructs the underlying `DslExecutor` (which auto-registers all
    /// `CustomOperation` implementations and verifies plugin verb coverage).
    #[cfg(feature = "database")]
    pub fn from_pool(pool: sqlx::PgPool) -> Self {
        Self {
            executor: Arc::new(DslExecutor::new(pool)),
            crud_port: None,
        }
    }

    /// Attach a SemOS-native CRUD executor.
    ///
    /// When set, CRUD verbs route through `CrudExecutionPort::execute_crud()`
    /// using `VerbContractBody` metadata, bypassing the legacy GenericCrudExecutor.
    pub fn with_crud_port(
        mut self,
        port: Arc<dyn sem_os_core::execution::CrudExecutionPort>,
    ) -> Self {
        self.crud_port = Some(port);
        self
    }
}

#[cfg(feature = "database")]
#[async_trait]
impl VerbExecutionPort for ObPocVerbExecutor {
    async fn execute_verb(
        &self,
        verb_fqn: &str,
        args: serde_json::Value,
        ctx: &mut VerbExecutionContext,
    ) -> sem_os_core::execution::Result<VerbExecutionResult> {
        // 1. Split FQN into domain.verb
        let (domain, verb) = split_fqn(verb_fqn)?;

        // 2. Resolve behavior from RuntimeVerbRegistry (contract-aware routing)
        use crate::dsl_v2::runtime_registry::{runtime_registry, RuntimeBehavior};
        let registry = runtime_registry();
        let runtime_verb = registry.get(&domain, &verb);

        let is_crud = runtime_verb
            .as_ref()
            .map(|rv| matches!(rv.behavior, RuntimeBehavior::Crud(_)))
            .unwrap_or(false);

        let behavior_label = match runtime_verb.as_ref().map(|rv| &rv.behavior) {
            Some(RuntimeBehavior::Crud(_)) => "crud",
            Some(RuntimeBehavior::Plugin(_)) => "plugin",
            Some(RuntimeBehavior::GraphQuery(_)) => "graph_query",
            Some(RuntimeBehavior::Durable(_)) => "durable",
            None => "unknown",
        };
        tracing::debug!(
            verb_fqn,
            behavior = behavior_label,
            has_crud_port = self.crud_port.is_some(),
            "VerbExecutionPort: routing verb"
        );

        // 3. CRUD fast path — route through CrudExecutionPort when available
        if is_crud {
            if let Some(ref crud_port) = self.crud_port {
                if let Some(rv) = runtime_verb.as_ref() {
                    let contract = runtime_verb_to_contract(rv);
                    match crud_port.execute_crud(&contract, args.clone(), ctx).await {
                        Ok(outcome) => {
                            return Ok(VerbExecutionResult::from_outcome(outcome));
                        }
                        Err(SemOsError::InvalidInput(msg)) if msg.contains("not yet migrated") => {
                            // Fall through to DslExecutor for unmigrated operations
                            tracing::debug!(verb_fqn, "CRUD port: falling through to DslExecutor");
                        }
                        Err(e) => return Err(e),
                    }
                }
            }
        }

        // 4. Default path — DslExecutor dispatch chain (plugin, graph_query, durable,
        //    or CRUD without crud_port / unmigrated operations)
        let vc = build_verb_call(&domain, &verb, &args);
        let mut exec_ctx = to_dsl_context(ctx);

        let result = self
            .executor
            .execute_verb(&vc, &mut exec_ctx)
            .await
            .map_err(|e| SemOsError::Internal(anyhow::anyhow!("Verb execution failed: {e}")))?;

        // 5. Collect side effects (new bindings + platform state)
        let side_effects = collect_side_effects(ctx, &exec_ctx);

        // 6. Propagate new bindings back to SemOS context
        for (name, uuid) in &side_effects.new_bindings {
            ctx.symbols.insert(name.clone(), *uuid);
        }
        for (name, entity_type) in &side_effects.new_binding_types {
            ctx.symbol_types.insert(name.clone(), entity_type.clone());
        }

        // 7. Convert result
        let outcome = to_verb_outcome(&result);

        Ok(VerbExecutionResult {
            outcome,
            side_effects,
        })
    }
}

// ── Conversion helpers ──────────────────────────────────────────

/// Convert a RuntimeVerb to a minimal VerbContractBody for CRUD execution.
/// Only populates the fields needed by CrudExecutionPort.
fn runtime_verb_to_contract(
    rv: &crate::dsl_v2::runtime_registry::RuntimeVerb,
) -> sem_os_core::verb_contract::VerbContractBody {
    use crate::dsl_v2::runtime_registry::RuntimeBehavior;
    use sem_os_core::verb_contract::{VerbArgDef, VerbContractBody, VerbCrudMapping, VerbReturnSpec};

    let crud_mapping = if let RuntimeBehavior::Crud(ref crud) = rv.behavior {
        Some(VerbCrudMapping {
            operation: format!("{:?}", crud.operation).to_lowercase(),
            table: Some(crud.table.clone()),
            schema: Some(crud.schema.clone()),
            key_column: crud.key.clone(),
            returning: crud.returning.clone(),
            conflict_keys: crud.conflict_keys.clone(),
            conflict_constraint: crud.conflict_constraint.clone(),
            junction: crud.junction.clone(),
            from_col: crud.from_col.clone(),
            to_col: crud.to_col.clone(),
            role_table: crud.role_table.clone(),
            role_col: crud.role_col.clone(),
            fk_col: crud.fk_col.clone(),
            filter_col: crud.filter_col.clone(),
            primary_table: crud.primary_table.clone(),
            join_table: crud.join_table.clone(),
            join_col: crud.join_col.clone(),
        })
    } else {
        None
    };

    let args: Vec<VerbArgDef> = rv
        .args
        .iter()
        .map(|a| VerbArgDef {
            name: a.name.clone(),
            arg_type: format!("{:?}", a.arg_type).to_lowercase(),
            required: a.required,
            description: a.description.clone(),
            lookup: None, // Lookups resolved before reaching CrudExecutionPort
            valid_values: a.valid_values.clone(),
            default: None,
            maps_to: a.maps_to.clone(),
        })
        .collect();

    let returns = Some(VerbReturnSpec {
        return_type: format!("{:?}", rv.returns.return_type).to_lowercase(),
        schema: None,
    });

    VerbContractBody {
        fqn: rv.full_name.clone(),
        domain: rv.domain.clone(),
        action: rv.verb.clone(),
        description: rv.description.clone(),
        behavior: "crud".to_string(),
        args,
        returns,
        crud_mapping,
        preconditions: vec![],
        postconditions: vec![],
        produces: None,
        consumes: vec![],
        invocation_phrases: vec![],
        subject_kinds: rv.subject_kinds.clone(),
        phase_tags: vec![],
        harm_class: rv.harm_class.map(|h| match h {
            dsl_core::config::types::HarmClass::ReadOnly => sem_os_core::verb_contract::HarmClass::ReadOnly,
            dsl_core::config::types::HarmClass::Reversible => sem_os_core::verb_contract::HarmClass::Reversible,
            dsl_core::config::types::HarmClass::Irreversible => sem_os_core::verb_contract::HarmClass::Irreversible,
            dsl_core::config::types::HarmClass::Destructive => sem_os_core::verb_contract::HarmClass::Destructive,
        }),
        action_class: None,
        precondition_states: vec![],
        requires_subject: true,
        produces_focus: false,
        metadata: None,
        reads_from: vec![],
        writes_to: vec![],
        outputs: vec![],
        produces_shared_facts: vec![],
    }
}

fn split_fqn(fqn: &str) -> sem_os_core::execution::Result<(String, String)> {
    let parts: Vec<&str> = fqn.splitn(2, '.').collect();
    if parts.len() != 2 {
        return Err(SemOsError::InvalidInput(format!(
            "Invalid verb FQN '{}': expected 'domain.verb'",
            fqn
        )));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

fn build_verb_call(domain: &str, verb: &str, args: &serde_json::Value) -> VerbCall {
    let arguments = match args.as_object() {
        Some(map) => map
            .iter()
            .map(|(key, value)| Argument {
                key: key.clone(),
                value: json_value_to_ast_node(value),
                span: Span::default(),
            })
            .collect(),
        None => vec![],
    };

    VerbCall {
        domain: domain.to_string(),
        verb: verb.to_string(),
        arguments,
        binding: None,
        span: Span::default(),
    }
}

fn json_value_to_ast_node(value: &serde_json::Value) -> AstNode {
    let span = Span::default();
    match value {
        serde_json::Value::String(s) => {
            // Check if it's a UUID
            if let Ok(uuid) = uuid::Uuid::parse_str(s) {
                AstNode::Literal(Literal::Uuid(uuid), span)
            } else {
                AstNode::Literal(Literal::String(s.clone()), span)
            }
        }
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                AstNode::Literal(Literal::Integer(i), span)
            } else {
                AstNode::Literal(Literal::String(n.to_string()), span)
            }
        }
        serde_json::Value::Bool(b) => AstNode::Literal(Literal::Boolean(*b), span),
        serde_json::Value::Null => AstNode::Literal(Literal::Null, span),
        // Arrays and objects: serialize as string (verb handlers parse as needed)
        other => AstNode::Literal(Literal::String(other.to_string()), span),
    }
}

fn to_dsl_context(ctx: &VerbExecutionContext) -> ExecutionContext {
    let mut exec_ctx = ExecutionContext {
        symbols: ctx.symbols.clone(),
        symbol_types: ctx.symbol_types.clone(),
        execution_id: ctx.execution_id,
        ..Default::default()
    };

    // Unpack platform extensions if present
    if let Some(obj) = ctx.extensions.as_object() {
        if let Some(audit_user) = obj.get("audit_user").and_then(|v| v.as_str()) {
            exec_ctx.audit_user = Some(audit_user.to_string());
        }
        if let Some(session_id) = obj.get("session_id").and_then(|v| v.as_str()) {
            if let Ok(uuid) = Uuid::parse_str(session_id) {
                exec_ctx.session_id = Some(uuid);
            }
        }
        if let Some(group_id) = obj.get("client_group_id").and_then(|v| v.as_str()) {
            if let Ok(uuid) = Uuid::parse_str(group_id) {
                exec_ctx.client_group_id = Some(uuid);
            }
        }
        if let Some(group_name) = obj.get("client_group_name").and_then(|v| v.as_str()) {
            exec_ctx.client_group_name = Some(group_name.to_string());
        }
        if let Some(persona) = obj.get("persona").and_then(|v| v.as_str()) {
            exec_ctx.persona = Some(persona.to_string());
        }
        // Session CBU IDs
        if let Some(cbu_ids) = obj.get("session_cbu_ids").and_then(|v| v.as_array()) {
            exec_ctx.session_cbu_ids = cbu_ids
                .iter()
                .filter_map(|v| v.as_str().and_then(|s| Uuid::parse_str(s).ok()))
                .collect();
        }
    }

    exec_ctx
}

fn collect_side_effects(
    original_ctx: &VerbExecutionContext,
    exec_ctx: &ExecutionContext,
) -> VerbSideEffects {
    // Find new bindings (symbols that weren't in the original context)
    let mut new_bindings = std::collections::HashMap::new();
    let mut new_binding_types = std::collections::HashMap::new();

    for (name, uuid) in &exec_ctx.symbols {
        if original_ctx.symbols.get(name) != Some(uuid) {
            new_bindings.insert(name.clone(), *uuid);
        }
    }
    for (name, entity_type) in &exec_ctx.symbol_types {
        if original_ctx.symbol_types.get(name) != Some(entity_type) {
            new_binding_types.insert(name.clone(), entity_type.clone());
        }
    }

    // Pack pending_* fields back into platform state
    let mut platform = serde_json::Map::new();

    if exec_ctx.pending_view_state.is_some() {
        platform.insert(
            "has_pending_view_state".to_string(),
            serde_json::Value::Bool(true),
        );
    }
    if exec_ctx.pending_scope_change.is_some() {
        platform.insert(
            "has_pending_scope_change".to_string(),
            serde_json::Value::Bool(true),
        );
    }
    if exec_ctx.pending_session.is_some() {
        platform.insert(
            "has_pending_session".to_string(),
            serde_json::Value::Bool(true),
        );
    }
    if !exec_ctx.pending_dag_flags.is_empty() {
        let flags: Vec<serde_json::Value> = exec_ctx
            .pending_dag_flags
            .iter()
            .map(|(k, v)| serde_json::json!({"key": k, "value": v}))
            .collect();
        platform.insert("pending_dag_flags".to_string(), serde_json::Value::Array(flags));
    }

    VerbSideEffects {
        new_bindings,
        new_binding_types,
        platform_state: serde_json::Value::Object(platform),
    }
}

fn to_verb_outcome(result: &ExecutionResult) -> VerbExecutionOutcome {
    match result {
        ExecutionResult::Uuid(id) => VerbExecutionOutcome::Uuid(*id),
        ExecutionResult::Record(v) => VerbExecutionOutcome::Record(v.clone()),
        ExecutionResult::RecordSet(v) => VerbExecutionOutcome::RecordSet(v.clone()),
        ExecutionResult::Affected(n) => VerbExecutionOutcome::Affected(*n),
        ExecutionResult::Void => VerbExecutionOutcome::Void,
        // Domain-specific result types — serialize via Debug repr until
        // these types gain Serialize derives (Phase 2 migration)
        ExecutionResult::EntityQuery(r) => VerbExecutionOutcome::Record(
            serde_json::json!({"_type": "entity_query", "_debug": format!("{r:?}")}),
        ),
        ExecutionResult::TemplateInvoked(r) => VerbExecutionOutcome::Record(
            serde_json::json!({"_type": "template_invoked", "_debug": format!("{r:?}")}),
        ),
        ExecutionResult::TemplateBatch(r) => VerbExecutionOutcome::Record(
            serde_json::json!({"_type": "template_batch", "_debug": format!("{r:?}")}),
        ),
        ExecutionResult::BatchControl(r) => VerbExecutionOutcome::Record(
            serde_json::json!({"_type": "batch_control", "_debug": format!("{r:?}")}),
        ),
    }
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use sem_os_core::principal::Principal;

    #[test]
    fn split_fqn_valid() {
        let (domain, verb) = split_fqn("cbu.create").unwrap();
        assert_eq!(domain, "cbu");
        assert_eq!(verb, "create");
    }

    #[test]
    fn split_fqn_with_hyphen() {
        let (domain, verb) = split_fqn("kyc-case.create-case").unwrap();
        assert_eq!(domain, "kyc-case");
        assert_eq!(verb, "create-case");
    }

    #[test]
    fn split_fqn_invalid() {
        assert!(split_fqn("noperiod").is_err());
    }

    #[test]
    fn build_verb_call_from_json() {
        let args = serde_json::json!({"name": "Acme Fund", "kind": "pe"});
        let vc = build_verb_call("cbu", "create", &args);

        assert_eq!(vc.domain, "cbu");
        assert_eq!(vc.verb, "create");
        assert_eq!(vc.arguments.len(), 2);
    }

    #[test]
    fn build_verb_call_empty_args() {
        let vc = build_verb_call("session", "info", &serde_json::json!({}));
        assert_eq!(vc.arguments.len(), 0);
    }

    #[test]
    fn build_verb_call_uuid_arg() {
        let id = Uuid::new_v4();
        let args = serde_json::json!({"entity-id": id.to_string()});
        let vc = build_verb_call("entity", "ghost", &args);

        assert_eq!(vc.arguments.len(), 1);
        assert!(matches!(&vc.arguments[0].value, AstNode::Literal(Literal::Uuid(u), _) if *u == id));
    }

    #[test]
    fn to_dsl_context_copies_symbols() {
        let mut ctx = VerbExecutionContext::new(Principal::system());
        let id = Uuid::new_v4();
        ctx.bind_typed("cbu", id, "cbu");
        ctx.execution_id = Uuid::nil();

        let exec_ctx = to_dsl_context(&ctx);
        assert_eq!(exec_ctx.symbols.get("cbu"), Some(&id));
        assert_eq!(exec_ctx.symbol_types.get("cbu").map(|s| s.as_str()), Some("cbu"));
        assert_eq!(exec_ctx.execution_id, Uuid::nil());
    }

    #[test]
    fn to_dsl_context_unpacks_extensions() {
        let mut ctx = VerbExecutionContext::new(Principal::system());
        ctx.extensions = serde_json::json!({
            "audit_user": "alice",
            "session_id": Uuid::nil().to_string(),
            "persona": "kyc"
        });

        let exec_ctx = to_dsl_context(&ctx);
        assert_eq!(exec_ctx.audit_user.as_deref(), Some("alice"));
        assert_eq!(exec_ctx.session_id, Some(Uuid::nil()));
        assert_eq!(exec_ctx.persona.as_deref(), Some("kyc"));
    }

    #[test]
    fn collect_side_effects_detects_new_bindings() {
        let ctx = VerbExecutionContext::new(Principal::system());
        let mut exec_ctx = ExecutionContext::default();
        let new_id = Uuid::new_v4();
        exec_ctx.symbols.insert("cbu".to_string(), new_id);
        exec_ctx.symbol_types.insert("cbu".to_string(), "cbu".to_string());

        let fx = collect_side_effects(&ctx, &exec_ctx);
        assert_eq!(fx.new_bindings.get("cbu"), Some(&new_id));
        assert_eq!(fx.new_binding_types.get("cbu").map(|s| s.as_str()), Some("cbu"));
    }

    #[test]
    fn collect_side_effects_ignores_unchanged_bindings() {
        let mut ctx = VerbExecutionContext::new(Principal::system());
        let existing_id = Uuid::new_v4();
        ctx.bind("cbu", existing_id);

        let mut exec_ctx = ExecutionContext::default();
        exec_ctx.symbols.insert("cbu".to_string(), existing_id);

        let fx = collect_side_effects(&ctx, &exec_ctx);
        assert!(fx.new_bindings.is_empty());
    }

    #[test]
    fn to_verb_outcome_all_variants() {
        let id = Uuid::new_v4();
        assert!(matches!(to_verb_outcome(&ExecutionResult::Uuid(id)), VerbExecutionOutcome::Uuid(u) if u == id));
        assert!(matches!(to_verb_outcome(&ExecutionResult::Record(serde_json::json!({"a":1}))), VerbExecutionOutcome::Record(_)));
        assert!(matches!(to_verb_outcome(&ExecutionResult::RecordSet(vec![])), VerbExecutionOutcome::RecordSet(v) if v.is_empty()));
        assert!(matches!(to_verb_outcome(&ExecutionResult::Affected(5)), VerbExecutionOutcome::Affected(5)));
        assert!(matches!(to_verb_outcome(&ExecutionResult::Void), VerbExecutionOutcome::Void));
    }

    #[test]
    fn behavior_routing_resolves_known_verbs() {
        use crate::dsl_v2::runtime_registry::{runtime_registry, RuntimeBehavior};

        let registry = runtime_registry();

        // cbu.show should be CRUD (SELECT)
        if let Some(rv) = registry.get("cbu", "show") {
            assert!(
                matches!(rv.behavior, RuntimeBehavior::Crud(_)),
                "cbu.show should be CRUD, got {:?}",
                std::mem::discriminant(&rv.behavior)
            );
        }

        // cbu.create should be Plugin
        if let Some(rv) = registry.get("cbu", "create") {
            assert!(
                matches!(rv.behavior, RuntimeBehavior::Plugin(_)),
                "cbu.create should be Plugin, got {:?}",
                std::mem::discriminant(&rv.behavior)
            );
        }
    }

    #[test]
    fn crud_port_is_optional() {
        // ObPocVerbExecutor without crud_port should still be constructable
        // (all CRUD verbs fall through to DslExecutor)
        // This just verifies the type compiles — actual execution needs a pool.
        let _has_method = ObPocVerbExecutor::with_crud_port;
    }
}
