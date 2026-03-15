use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Comparison RHS value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Literal(Literal),
    Param(usize),
}

/// Literal value used in reducer expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Str(String),
    Num(f64),
    Bool(bool),
    Null,
    List(Vec<Literal>),
}

/// Comparison operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
    In,
    NotIn,
    Like,
}

/// Aggregate function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggFn {
    Count,
    Any,
    All,
}

/// Leaf expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Existence {
        source: String,
        field: String,
        negated: bool,
    },
    Comparison {
        source: String,
        field: String,
        op: CompareOp,
        value: Value,
    },
    Aggregate {
        function: AggFn,
        source: String,
        filter: Option<Predicate>,
    },
    ScopeComparison {
        path: Vec<String>,
        op: CompareOp,
        value: Value,
    },
    SlotAggregate {
        function: AggFn,
        filter: SlotPredicate,
    },
}

/// Parsed condition body.
#[derive(Debug, Clone, PartialEq)]
pub enum ConditionBody {
    Leaf {
        expr: Expr,
        compare: Option<(CompareOp, Value)>,
    },
    Ref {
        name: String,
    },
    Call {
        name: String,
        args: Vec<Value>,
    },
    And(Vec<ConditionBody>),
    Or(Vec<ConditionBody>),
    Not(Box<ConditionBody>),
}

/// WHERE predicate.
#[derive(Debug, Clone, PartialEq)]
pub enum Predicate {
    Atom {
        field: String,
        op: CompareOp,
        value: Value,
    },
    IsNull {
        field: String,
        negated: bool,
    },
    And(Vec<Predicate>),
    Or(Vec<Predicate>),
    Not(Box<Predicate>),
}

/// Slot predicate tree for `scope.slots WHERE ...`.
#[derive(Debug, Clone, PartialEq)]
pub enum SlotPredicate {
    Atom {
        field: SlotField,
        op: CompareOp,
        value: Value,
    },
    And(Vec<SlotPredicate>),
    Or(Vec<SlotPredicate>),
    Not(Box<SlotPredicate>),
}

/// Fields supported in slot predicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlotField {
    Type,
    Cardinality,
    EffectiveState,
    ComputedState,
}

/// Aggregate evaluation result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AggResult {
    Count(usize),
    Boolean(bool),
}

impl AggResult {
    /// Compare the aggregate result against a literal.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::sem_reg::reducer::{AggResult, CompareOp, Literal};
    ///
    /// assert!(AggResult::Count(2).compare(CompareOp::Gt, &Literal::Num(1.0)));
    /// ```
    pub fn compare(self, op: CompareOp, rhs: &Literal) -> bool {
        match (self, rhs) {
            (Self::Count(lhs), Literal::Num(rhs)) => compare_numbers(lhs as f64, *rhs, op),
            (Self::Boolean(lhs), Literal::Bool(rhs)) => compare_bools(lhs, *rhs, op),
            _ => false,
        }
    }

    /// Convert the aggregate result to boolean semantics.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::sem_reg::reducer::AggResult;
    ///
    /// assert!(AggResult::Count(1).as_bool());
    /// assert!(!AggResult::Count(0).as_bool());
    /// ```
    pub fn as_bool(self) -> bool {
        match self {
            Self::Count(value) => value > 0,
            Self::Boolean(value) => value,
        }
    }
}

fn compare_numbers(lhs: f64, rhs: f64, op: CompareOp) -> bool {
    match op {
        CompareOp::Eq => lhs == rhs,
        CompareOp::Neq => lhs != rhs,
        CompareOp::Gt => lhs > rhs,
        CompareOp::Gte => lhs >= rhs,
        CompareOp::Lt => lhs < rhs,
        CompareOp::Lte => lhs <= rhs,
        CompareOp::In | CompareOp::NotIn | CompareOp::Like => false,
    }
}

fn compare_bools(lhs: bool, rhs: bool, op: CompareOp) -> bool {
    match op {
        CompareOp::Eq => lhs == rhs,
        CompareOp::Neq => lhs != rhs,
        CompareOp::Gt
        | CompareOp::Gte
        | CompareOp::Lt
        | CompareOp::Lte
        | CompareOp::In
        | CompareOp::NotIn
        | CompareOp::Like => false,
    }
}

/// Runtime field value used by tests and evaluator stubs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FieldValue {
    Str(String),
    Num(f64),
    Bool(bool),
    Null,
}

/// Overlay row used by evaluator tests.
#[derive(Debug, Clone, PartialEq)]
pub struct OverlayRow {
    pub fields: HashMap<String, FieldValue>,
}

/// Scope data used by scope comparisons.
#[derive(Debug, Clone, PartialEq)]
pub struct ScopeData {
    pub fields: serde_json::Value,
}

/// Slot record used by slot aggregates.
#[derive(Debug, Clone, PartialEq)]
pub struct SlotRecord {
    pub slot_type: String,
    pub cardinality: String,
    pub effective_state: String,
    pub computed_state: String,
}

/// Overlay and scope bundle used by evaluator tests.
#[derive(Debug, Clone, PartialEq)]
pub struct SlotOverlayData {
    pub sources: HashMap<String, Vec<OverlayRow>>,
    pub scope: ScopeData,
    pub slots: Vec<SlotRecord>,
}

/// Evaluation scope built from the parent constellation context.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvalScope {
    pub cbu_id: Option<Uuid>,
    pub case_id: Option<Uuid>,
    pub case_status: Option<String>,
    pub fields: HashMap<String, String>,
}

impl EvalScope {
    /// Convert the evaluation scope into reducer-visible scope data.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::sem_reg::reducer::EvalScope;
    ///
    /// let scope = EvalScope::default();
    /// let json = scope.as_scope_data();
    /// assert!(json.fields.is_object());
    /// ```
    pub fn as_scope_data(&self) -> ScopeData {
        let mut map = serde_json::Map::new();
        if let Some(cbu_id) = self.cbu_id {
            map.insert(
                "cbu_id".into(),
                serde_json::Value::String(cbu_id.to_string()),
            );
        }
        if let Some(case_id) = self.case_id {
            map.insert(
                "case_id".into(),
                serde_json::Value::String(case_id.to_string()),
            );
        }
        if let Some(case_status) = &self.case_status {
            map.insert(
                "case_status".into(),
                serde_json::Value::String(case_status.clone()),
            );
        }
        for (key, value) in &self.fields {
            map.insert(key.clone(), serde_json::Value::String(value.clone()));
        }
        ScopeData {
            fields: serde_json::Value::Object(map),
        }
    }
}

/// Per-condition evaluation output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConditionEvaluation {
    pub name: String,
    pub result: bool,
}

/// Per-rule evaluation output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleEvaluation {
    pub state: String,
    pub matched: bool,
}

/// Why a verb is blocked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockReason {
    pub message: String,
}

/// Blocked verb with reasons.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockedVerb {
    pub verb: String,
    pub reasons: Vec<BlockReason>,
}

/// Result payload for `state.blocked-why`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockedWhyResult {
    pub blocked: bool,
    pub verb: String,
    pub reasons: Vec<BlockReason>,
}

/// Consistency warning emitted by reducer scans.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsistencyWarning {
    pub slot_path: String,
    pub warning: String,
    pub detail: Option<String>,
}

/// Public override summary used in traces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OverrideInfo {
    pub override_state: String,
    pub authority: String,
    pub justification: String,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Trace payload returned by `state.diagnose`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DerivationTrace {
    pub reducer_revision: String,
    pub slot_path: String,
    pub entity_id: Option<Uuid>,
    pub state_machine: String,
    pub computed_state: String,
    pub override_entry: Option<OverrideInfo>,
    pub effective_state: String,
    pub conditions_evaluated: Vec<ConditionEvaluation>,
    pub rules_evaluated: Vec<RuleEvaluation>,
    pub available_verbs: Vec<String>,
    pub blocked_verbs: Vec<BlockedVerb>,
    pub consistency_warnings: Vec<String>,
}

/// Reduced state result for a single slot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlotReduceResult {
    pub slot_path: String,
    pub computed_state: String,
    pub effective_state: String,
    pub override_entry: Option<crate::sem_reg::reducer::overrides::StateOverride>,
    pub available_verbs: Vec<String>,
    pub blocked_verbs: Vec<BlockedVerb>,
    pub consistency_warnings: Vec<String>,
    pub reducer_revision: String,
}
