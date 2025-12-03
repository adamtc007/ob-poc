//! Execution Plan - Compiled representation of DSL for dependency-ordered execution
//!
//! The DSL compiler transforms a declarative AST (tree) into an ExecutionPlan (linear)
//! that respects data dependencies. This is analogous to:
//! - Lisp's evaluation order with let-bindings
//! - SQL query planner's CTE ordering
//! - Stack-based compile-time analysis
//!
//! # Architecture
//!
//! ```text
//! DSL Source (declarative)     AST (tree)           Plan (linear)
//! ─────────────────────────    ──────────────       ─────────────────
//! (cbu.create :name "Fund"  →  VerbCall {        →  Step 0: cbu.create → $0
//!   :roles [                     children: [        Step 1: assign-role($0, aviva)
//!     (cbu.assign-role ...)      VerbCall...        Step 2: assign-role($0, bob)
//!   ])                         ]
//!                            }
//! ```

use super::ast::{Argument, Program, Statement, Value, VerbCall};
use super::verb_registry::{registry, VerbBehavior};

/// A compiled execution plan - dependency sorted sequence of steps
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    pub steps: Vec<ExecutionStep>,
}

/// A single step in the execution plan
#[derive(Debug, Clone)]
pub struct ExecutionStep {
    /// The verb call to execute (with nested children removed)
    pub verb_call: VerbCall,

    /// Values to inject from previous steps' results
    pub injections: Vec<Injection>,

    /// Optional symbol binding (from :as @name syntax)
    pub bind_as: Option<String>,

    /// Step index (for debugging/logging)
    pub step_index: usize,

    /// How this step should be executed (CRUD, CustomOp, Composite)
    pub behavior: VerbBehavior,

    /// For custom ops, the handler ID (e.g., "document.catalog")
    pub custom_op_id: Option<String>,
}

/// Instruction to inject a previous step's result into this step's arguments
#[derive(Debug, Clone)]
pub struct Injection {
    /// Index of the step that produces the value
    pub from_step: usize,

    /// Argument key to inject into (e.g., "cbu-id")
    pub into_arg: String,
}

/// Compilation errors
#[derive(Debug, Clone)]
pub enum CompileError {
    /// Verb not found in registry
    UnknownVerb {
        domain: String,
        verb: String,
        suggestions: Vec<String>,
    },

    /// Circular dependency detected
    CircularDependency { steps: Vec<usize> },

    /// Cannot determine parent FK for nested verb
    UnknownParentRelation {
        parent_domain: String,
        child_domain: String,
    },
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileError::UnknownVerb {
                domain,
                verb,
                suggestions,
            } => {
                write!(f, "Unknown verb: {}.{}", domain, verb)?;
                if !suggestions.is_empty() {
                    write!(f, "\n  Did you mean: {}?", suggestions[0])?;
                    write!(f, "\n  Available verbs: {}", suggestions.join(", "))?;
                }
                Ok(())
            }
            CompileError::CircularDependency { steps } => {
                write!(f, "Circular dependency between steps: {:?}", steps)
            }
            CompileError::UnknownParentRelation {
                parent_domain,
                child_domain,
            } => {
                write!(
                    f,
                    "Cannot determine FK relation: {} → {}",
                    parent_domain, child_domain
                )
            }
        }
    }
}

impl std::error::Error for CompileError {}

// ============================================================================
// Parent-Child FK Relationships
// ============================================================================

/// Mapping of (parent_domain, child_domain) → child's FK argument name
/// This is the "schema" of how domains relate
static PARENT_FK_MAP: &[(&str, &str, &str)] = &[
    // (parent_domain, child_domain, fk_arg_in_child)

    // Same-domain operations (self-referential)
    ("cbu", "cbu", "cbu-id"),                // cbu.assign-role needs cbu-id
    ("entity", "entity", "entity-id"),       // entity operations
    ("document", "document", "document-id"), // document operations
    ("investigation", "investigation", "investigation-id"),
    ("decision", "decision", "decision-id"),
    ("screening", "screening", "screening-id"),
    ("product", "product", "product-id"),
    ("service", "service", "service-id"),
    ("monitoring", "monitoring", "monitoring-id"),
    ("risk", "risk", "risk-id"),
    // CBU as parent → various child domains
    ("cbu", "document", "cbu-id"),      // document.link-cbu
    ("cbu", "investigation", "cbu-id"), // investigation.create
    ("cbu", "decision", "cbu-id"),      // decision.record
    ("cbu", "monitoring", "cbu-id"),    // monitoring.setup, monitoring.record-event
    ("cbu", "risk", "cbu-id"),          // risk.set-rating, risk.add-flag
    ("cbu", "screening", "cbu-id"),     // screening operations on CBU
    // Entity as parent → various child domains
    ("entity", "document", "entity-id"),  // document.link-entity
    ("entity", "screening", "entity-id"), // screening.pep, screening.sanctions
    ("entity", "risk", "entity-id"),      // risk operations on entity
    // Investigation as parent
    ("investigation", "screening", "investigation-id"), // screening within investigation
    ("investigation", "decision", "investigation-id"),  // decision.record for investigation
    // Product/Service relationships
    ("product", "service", "product-id"), // service.link-product
    ("service", "product", "service-id"), // product.link-service (reverse)
];

/// Look up which argument a child verb needs from its parent
fn infer_parent_fk(parent_domain: &str, child_domain: &str) -> Option<&'static str> {
    PARENT_FK_MAP
        .iter()
        .find(|(p, c, _)| *p == parent_domain && *c == child_domain)
        .map(|(_, _, fk)| *fk)
}

// ============================================================================
// Compiler
// ============================================================================

/// Minimum similarity threshold for verb suggestions
const VERB_SIMILARITY_THRESHOLD: f64 = 0.5;

/// Get verb suggestions for an unknown verb using Jaro-Winkler similarity
fn get_verb_suggestions(domain: &str, verb: &str) -> Vec<String> {
    let reg = registry();

    // First, check if the domain exists and get verbs from that domain
    let domain_verbs: Vec<String> = reg
        .verbs_for_domain(domain)
        .into_iter()
        .map(|v| format!("{}.{}", v.domain, v.verb))
        .collect();

    // If domain exists, suggest verbs from that domain
    if !domain_verbs.is_empty() {
        // Sort by Jaro-Winkler similarity (higher is better)
        let mut scored: Vec<(String, f64)> = domain_verbs
            .into_iter()
            .map(|full_verb| {
                let v = full_verb.split('.').nth(1).unwrap_or("");
                let score = strsim::jaro_winkler(verb, v);
                (full_verb, score)
            })
            .filter(|(_, score)| *score >= VERB_SIMILARITY_THRESHOLD)
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        return scored.into_iter().take(5).map(|(v, _)| v).collect();
    }

    // Domain doesn't exist - suggest similar verbs from all domains
    let full_verb = format!("{}.{}", domain, verb);

    let mut scored: Vec<(String, f64)> = reg
        .all_verbs()
        .map(|v| {
            let full = format!("{}.{}", v.domain, v.verb);
            let score = strsim::jaro_winkler(&full_verb, &full);
            (full, score)
        })
        .filter(|(_, score)| *score >= VERB_SIMILARITY_THRESHOLD)
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().take(5).map(|(v, _)| v).collect()
}

/// Compile an AST into an execution plan
pub fn compile(program: &Program) -> Result<ExecutionPlan, CompileError> {
    let mut compiler = Compiler::new();

    for statement in &program.statements {
        if let Statement::VerbCall(vc) = statement {
            compiler.compile_verb_call(vc, None)?;
        }
        // Comments are ignored at compile time
    }

    Ok(ExecutionPlan {
        steps: compiler.steps,
    })
}

/// Compiler state
struct Compiler {
    steps: Vec<ExecutionStep>,
}

impl Compiler {
    fn new() -> Self {
        Self { steps: Vec::new() }
    }

    /// Compile a verb call, potentially with nested children
    /// Returns the step index of this verb call
    fn compile_verb_call(
        &mut self,
        vc: &VerbCall,
        parent: Option<ParentInfo>,
    ) -> Result<usize, CompileError> {
        // Look up verb in unified registry (includes both CRUD and custom ops)
        let verb_def = registry().get(&vc.domain, &vc.verb).ok_or_else(|| {
            // Get suggestions for the unknown verb
            let suggestions = get_verb_suggestions(&vc.domain, &vc.verb);
            CompileError::UnknownVerb {
                domain: vc.domain.clone(),
                verb: vc.verb.clone(),
                suggestions,
            }
        })?;

        // Build injections from parent
        let mut injections = Vec::new();
        if let Some(parent_info) = &parent {
            let fk_arg = infer_parent_fk(&parent_info.domain, &vc.domain).ok_or_else(|| {
                CompileError::UnknownParentRelation {
                    parent_domain: parent_info.domain.clone(),
                    child_domain: vc.domain.clone(),
                }
            })?;

            injections.push(Injection {
                from_step: parent_info.step_index,
                into_arg: fk_arg.to_string(),
            });
        }

        // Extract nested children and create a "flattened" verb call
        let (flat_vc, nested_children) = extract_nested_children(vc);

        // Add this step
        let my_step_index = self.steps.len();
        self.steps.push(ExecutionStep {
            verb_call: flat_vc,
            injections,
            bind_as: vc.as_binding.clone(),
            step_index: my_step_index,
            behavior: verb_def.behavior,
            custom_op_id: verb_def.custom_op_id.clone(),
        });

        // Recursively compile children with this step as parent
        let parent_info = ParentInfo {
            domain: vc.domain.clone(),
            step_index: my_step_index,
        };

        for child_vc in nested_children {
            self.compile_verb_call(&child_vc, Some(parent_info.clone()))?;
        }

        Ok(my_step_index)
    }
}

/// Info about the parent verb call for injection
#[derive(Clone)]
struct ParentInfo {
    domain: String,
    step_index: usize,
}

// ============================================================================
// Nested Children Extraction
// ============================================================================

/// Extract nested VerbCalls from a verb call's arguments
/// Returns (flattened_verb_call, nested_children)
fn extract_nested_children(vc: &VerbCall) -> (VerbCall, Vec<VerbCall>) {
    let mut flat_args = Vec::new();
    let mut nested = Vec::new();

    for arg in &vc.arguments {
        match &arg.value {
            // List might contain nested verb calls
            Value::List(items) => {
                let mut flat_items = Vec::new();
                for item in items {
                    if let Value::NestedCall(child_vc) = item {
                        nested.push(*child_vc.clone());
                    } else {
                        flat_items.push(item.clone());
                    }
                }
                // Keep non-nested items in the list
                if !flat_items.is_empty() {
                    flat_args.push(Argument {
                        key: arg.key.clone(),
                        key_span: arg.key_span,
                        value: Value::List(flat_items),
                        value_span: arg.value_span,
                    });
                }
                // If list was purely nested calls, we might want to track the key
                // for semantic purposes (e.g., :roles, :children)
            }
            // Single nested call
            Value::NestedCall(child_vc) => {
                nested.push(*child_vc.clone());
            }
            // Regular values pass through
            _ => {
                flat_args.push(arg.clone());
            }
        }
    }

    let flat_vc = VerbCall {
        domain: vc.domain.clone(),
        verb: vc.verb.clone(),
        verb_span: vc.verb_span,
        arguments: flat_args,
        as_binding: vc.as_binding.clone(),
        as_binding_span: vc.as_binding_span,
        span: vc.span,
    };

    (flat_vc, nested)
}

// ============================================================================
// Plan Inspection / Debug
// ============================================================================

impl ExecutionPlan {
    /// Pretty print the plan for debugging
    pub fn debug_print(&self) -> String {
        let mut out = String::new();
        out.push_str("=== Execution Plan ===\n");

        for step in &self.steps {
            out.push_str(&format!(
                "Step {}: {}.{}",
                step.step_index, step.verb_call.domain, step.verb_call.verb
            ));

            if let Some(ref binding) = step.bind_as {
                out.push_str(&format!(" → @{}", binding));
            }
            out.push('\n');

            // Show injections
            for inj in &step.injections {
                out.push_str(&format!(
                    "  ← inject ${} as :{}\n",
                    inj.from_step, inj.into_arg
                ));
            }

            // Show args
            for arg in &step.verb_call.arguments {
                out.push_str(&format!("  :{} = {:?}\n", arg.key.canonical(), arg.value));
            }
        }

        out
    }

    /// Get execution order as domain.verb sequence (for logging)
    pub fn execution_sequence(&self) -> Vec<String> {
        self.steps
            .iter()
            .map(|s| format!("{}.{}", s.verb_call.domain, s.verb_call.verb))
            .collect()
    }

    /// Count of steps
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl_v2::ast::{Key, Span};

    fn make_verb_call(domain: &str, verb: &str, args: Vec<(&str, Value)>) -> VerbCall {
        VerbCall {
            domain: domain.to_string(),
            verb: verb.to_string(),
            verb_span: Span::default(),
            arguments: args
                .into_iter()
                .map(|(k, v)| Argument {
                    key: Key::Simple(k.to_string()),
                    key_span: Span::default(),
                    value: v,
                    value_span: Span::default(),
                })
                .collect(),
            as_binding: None,
            as_binding_span: None,
            span: Span::default(),
        }
    }

    #[test]
    fn test_compile_flat_program() {
        // Simple sequential DSL - no nesting
        let program = Program {
            statements: vec![
                Statement::VerbCall(make_verb_call(
                    "cbu",
                    "create",
                    vec![("name", Value::String("Test Fund".into()))],
                )),
                Statement::VerbCall(make_verb_call(
                    "entity",
                    "read",
                    vec![("entity-id", Value::String("some-uuid".into()))],
                )),
            ],
        };

        let plan = compile(&program).unwrap();

        assert_eq!(plan.len(), 2);
        assert!(plan.steps[0].injections.is_empty());
        assert!(plan.steps[1].injections.is_empty());

        let seq = plan.execution_sequence();
        assert_eq!(seq, vec!["cbu.create", "entity.read"]);
    }

    #[test]
    fn test_compile_nested_children() {
        // Nested DSL - CBU with role assignments
        let child1 = make_verb_call(
            "cbu",
            "assign-role",
            vec![
                ("entity-id", Value::String("entity-uuid-1".into())),
                ("role", Value::String("Manager".into())),
            ],
        );
        let child2 = make_verb_call(
            "cbu",
            "assign-role",
            vec![
                ("entity-id", Value::String("entity-uuid-2".into())),
                ("role", Value::String("Director".into())),
            ],
        );

        let parent = VerbCall {
            domain: "cbu".to_string(),
            verb: "create".to_string(),
            verb_span: Span::default(),
            arguments: vec![
                Argument {
                    key: Key::Simple("name".into()),
                    key_span: Span::default(),
                    value: Value::String("Test Fund".into()),
                    value_span: Span::default(),
                },
                Argument {
                    key: Key::Simple("roles".into()),
                    key_span: Span::default(),
                    value: Value::List(vec![
                        Value::NestedCall(Box::new(child1)),
                        Value::NestedCall(Box::new(child2)),
                    ]),
                    value_span: Span::default(),
                },
            ],
            as_binding: None,
            as_binding_span: None,
            span: Span::default(),
        };

        let program = Program {
            statements: vec![Statement::VerbCall(parent)],
        };

        let plan = compile(&program).unwrap();

        // Should have 3 steps: create + 2 assign-role
        assert_eq!(plan.len(), 3);

        // First step (create) has no injections
        assert!(plan.steps[0].injections.is_empty());
        assert_eq!(plan.steps[0].verb_call.verb, "create");

        // Second step (assign-role) injects cbu-id from step 0
        assert_eq!(plan.steps[1].injections.len(), 1);
        assert_eq!(plan.steps[1].injections[0].from_step, 0);
        assert_eq!(plan.steps[1].injections[0].into_arg, "cbu-id");

        // Third step also injects from step 0
        assert_eq!(plan.steps[2].injections.len(), 1);
        assert_eq!(plan.steps[2].injections[0].from_step, 0);

        println!("{}", plan.debug_print());
    }

    #[test]
    fn test_infer_parent_fk() {
        assert_eq!(infer_parent_fk("cbu", "cbu"), Some("cbu-id"));
        assert_eq!(infer_parent_fk("cbu", "investigation"), Some("cbu-id"));
        assert_eq!(infer_parent_fk("entity", "document"), Some("entity-id"));
        assert_eq!(infer_parent_fk("unknown", "unknown"), None);
    }

    #[test]
    fn test_unknown_verb_error() {
        let program = Program {
            statements: vec![Statement::VerbCall(make_verb_call("fake", "verb", vec![]))],
        };

        let result = compile(&program);
        assert!(matches!(result, Err(CompileError::UnknownVerb { .. })));
    }

    #[test]
    fn test_product_ensure_compiles() {
        // product.ensure should compile successfully
        let program = Program {
            statements: vec![Statement::VerbCall(make_verb_call(
                "product",
                "ensure",
                vec![
                    ("name", Value::String("Test Product".into())),
                    ("product-code", Value::String("TEST".into())),
                ],
            ))],
        };

        let result = compile(&program);
        assert!(
            result.is_ok(),
            "product.ensure should compile: {:?}",
            result
        );
        let plan = result.unwrap();
        assert_eq!(plan.len(), 1);
        assert_eq!(plan.steps[0].verb_call.domain, "product");
        assert_eq!(plan.steps[0].verb_call.verb, "ensure");
    }
}
