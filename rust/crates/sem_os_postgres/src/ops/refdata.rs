//! Refdata domain verbs (4 plugin verbs) — YAML-first
//! re-implementation of `rust/config/verbs/refdata.yaml`.
//!
//! Unified ensure / read / list / deactivate across 9 reference
//! domains (jurisdiction, currency, market, settlement-type,
//! ssi-type, client-type, screening-type, risk-rating,
//! case-type) — dispatched from the `domain` arg via a
//! compile-time `REFDATA_DOMAINS` table.

use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use sqlx::{Postgres, QueryBuilder};

use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

#[derive(Clone, Copy)]
enum RefdataArgType {
    String,
    Bool,
    Int,
}

#[derive(Clone, Copy)]
enum RefdataDefaultValue {
    Bool(bool),
    Int(i64),
}

#[derive(Clone, Copy)]
struct RefdataField {
    arg: &'static str,
    column: &'static str,
    arg_type: RefdataArgType,
    required: bool,
    default: Option<RefdataDefaultValue>,
}

#[derive(Clone, Copy)]
enum RefdataReturnKind {
    Uuid,
    String,
}

#[derive(Clone, Copy)]
struct RefdataDomainSpec {
    domain: &'static str,
    schema: &'static str,
    table: &'static str,
    key_arg: &'static str,
    key_column: &'static str,
    return_column: &'static str,
    return_kind: RefdataReturnKind,
    active_column: Option<&'static str>,
    order_by: &'static str,
    fields: &'static [RefdataField],
    list_filters: &'static [&'static str],
}

const JURISDICTION_FIELDS: &[RefdataField] = &[
    RefdataField {
        arg: "code",
        column: "jurisdiction_code",
        arg_type: RefdataArgType::String,
        required: true,
        default: None,
    },
    RefdataField {
        arg: "name",
        column: "jurisdiction_name",
        arg_type: RefdataArgType::String,
        required: true,
        default: None,
    },
    RefdataField {
        arg: "country-code",
        column: "country_code",
        arg_type: RefdataArgType::String,
        required: true,
        default: None,
    },
    RefdataField {
        arg: "region",
        column: "region",
        arg_type: RefdataArgType::String,
        required: false,
        default: None,
    },
    RefdataField {
        arg: "regulatory-framework",
        column: "regulatory_framework",
        arg_type: RefdataArgType::String,
        required: false,
        default: None,
    },
    RefdataField {
        arg: "entity-formation-allowed",
        column: "entity_formation_allowed",
        arg_type: RefdataArgType::Bool,
        required: false,
        default: Some(RefdataDefaultValue::Bool(true)),
    },
    RefdataField {
        arg: "offshore",
        column: "offshore_jurisdiction",
        arg_type: RefdataArgType::Bool,
        required: false,
        default: Some(RefdataDefaultValue::Bool(false)),
    },
    RefdataField {
        arg: "regulatory-authority",
        column: "regulatory_authority",
        arg_type: RefdataArgType::String,
        required: false,
        default: None,
    },
];

const CURRENCY_FIELDS: &[RefdataField] = &[
    RefdataField {
        arg: "iso-code",
        column: "iso_code",
        arg_type: RefdataArgType::String,
        required: true,
        default: None,
    },
    RefdataField {
        arg: "name",
        column: "name",
        arg_type: RefdataArgType::String,
        required: true,
        default: None,
    },
    RefdataField {
        arg: "symbol",
        column: "symbol",
        arg_type: RefdataArgType::String,
        required: false,
        default: None,
    },
    RefdataField {
        arg: "decimal-places",
        column: "decimal_places",
        arg_type: RefdataArgType::Int,
        required: false,
        default: Some(RefdataDefaultValue::Int(2)),
    },
    RefdataField {
        arg: "is-active",
        column: "is_active",
        arg_type: RefdataArgType::Bool,
        required: false,
        default: Some(RefdataDefaultValue::Bool(true)),
    },
];

const SIMPLE_CODE_FIELDS: &[RefdataField] = &[
    RefdataField {
        arg: "code",
        column: "code",
        arg_type: RefdataArgType::String,
        required: true,
        default: None,
    },
    RefdataField {
        arg: "name",
        column: "name",
        arg_type: RefdataArgType::String,
        required: true,
        default: None,
    },
    RefdataField {
        arg: "description",
        column: "description",
        arg_type: RefdataArgType::String,
        required: false,
        default: None,
    },
    RefdataField {
        arg: "display-order",
        column: "display_order",
        arg_type: RefdataArgType::Int,
        required: false,
        default: Some(RefdataDefaultValue::Int(0)),
    },
    RefdataField {
        arg: "is-active",
        column: "is_active",
        arg_type: RefdataArgType::Bool,
        required: false,
        default: Some(RefdataDefaultValue::Bool(true)),
    },
];

const RISK_RATING_FIELDS: &[RefdataField] = &[
    RefdataField {
        arg: "code",
        column: "code",
        arg_type: RefdataArgType::String,
        required: true,
        default: None,
    },
    RefdataField {
        arg: "name",
        column: "name",
        arg_type: RefdataArgType::String,
        required: true,
        default: None,
    },
    RefdataField {
        arg: "description",
        column: "description",
        arg_type: RefdataArgType::String,
        required: false,
        default: None,
    },
    RefdataField {
        arg: "severity-level",
        column: "severity_level",
        arg_type: RefdataArgType::Int,
        required: false,
        default: Some(RefdataDefaultValue::Int(0)),
    },
    RefdataField {
        arg: "display-order",
        column: "display_order",
        arg_type: RefdataArgType::Int,
        required: false,
        default: Some(RefdataDefaultValue::Int(0)),
    },
    RefdataField {
        arg: "is-active",
        column: "is_active",
        arg_type: RefdataArgType::Bool,
        required: false,
        default: Some(RefdataDefaultValue::Bool(true)),
    },
];

const MARKET_FIELDS: &[RefdataField] = &[
    RefdataField {
        arg: "mic",
        column: "mic",
        arg_type: RefdataArgType::String,
        required: true,
        default: None,
    },
    RefdataField {
        arg: "name",
        column: "name",
        arg_type: RefdataArgType::String,
        required: true,
        default: None,
    },
    RefdataField {
        arg: "country-code",
        column: "country_code",
        arg_type: RefdataArgType::String,
        required: true,
        default: None,
    },
    RefdataField {
        arg: "primary-currency",
        column: "primary_currency",
        arg_type: RefdataArgType::String,
        required: true,
        default: None,
    },
    RefdataField {
        arg: "csd-bic",
        column: "csd_bic",
        arg_type: RefdataArgType::String,
        required: false,
        default: None,
    },
    RefdataField {
        arg: "timezone",
        column: "timezone",
        arg_type: RefdataArgType::String,
        required: true,
        default: None,
    },
];

const REFDATA_DOMAINS: &[RefdataDomainSpec] = &[
    RefdataDomainSpec {
        domain: "jurisdiction",
        schema: "ob-poc",
        table: "master_jurisdictions",
        key_arg: "code",
        key_column: "jurisdiction_code",
        return_column: "jurisdiction_code",
        return_kind: RefdataReturnKind::String,
        active_column: None,
        order_by: "jurisdiction_code",
        fields: JURISDICTION_FIELDS,
        list_filters: &["region", "offshore"],
    },
    RefdataDomainSpec {
        domain: "currency",
        schema: "ob-poc",
        table: "currencies",
        key_arg: "iso-code",
        key_column: "iso_code",
        return_column: "currency_id",
        return_kind: RefdataReturnKind::Uuid,
        active_column: Some("is_active"),
        order_by: "iso_code",
        fields: CURRENCY_FIELDS,
        list_filters: &["is-active"],
    },
    RefdataDomainSpec {
        domain: "market",
        schema: "custody",
        table: "markets",
        key_arg: "mic",
        key_column: "mic",
        return_column: "market_id",
        return_kind: RefdataReturnKind::Uuid,
        active_column: None,
        order_by: "mic",
        fields: MARKET_FIELDS,
        list_filters: &["country-code"],
    },
    RefdataDomainSpec {
        domain: "settlement-type",
        schema: "ob-poc",
        table: "settlement_types",
        key_arg: "code",
        key_column: "code",
        return_column: "code",
        return_kind: RefdataReturnKind::String,
        active_column: Some("is_active"),
        order_by: "display_order",
        fields: SIMPLE_CODE_FIELDS,
        list_filters: &["is-active"],
    },
    RefdataDomainSpec {
        domain: "ssi-type",
        schema: "ob-poc",
        table: "ssi_types",
        key_arg: "code",
        key_column: "code",
        return_column: "code",
        return_kind: RefdataReturnKind::String,
        active_column: Some("is_active"),
        order_by: "display_order",
        fields: SIMPLE_CODE_FIELDS,
        list_filters: &["is-active"],
    },
    RefdataDomainSpec {
        domain: "client-type",
        schema: "ob-poc",
        table: "client_types",
        key_arg: "code",
        key_column: "code",
        return_column: "code",
        return_kind: RefdataReturnKind::String,
        active_column: Some("is_active"),
        order_by: "display_order",
        fields: SIMPLE_CODE_FIELDS,
        list_filters: &["is-active"],
    },
    RefdataDomainSpec {
        domain: "screening-type",
        schema: "ob-poc",
        table: "screening_types",
        key_arg: "code",
        key_column: "code",
        return_column: "code",
        return_kind: RefdataReturnKind::String,
        active_column: Some("is_active"),
        order_by: "display_order",
        fields: SIMPLE_CODE_FIELDS,
        list_filters: &["is-active"],
    },
    RefdataDomainSpec {
        domain: "risk-rating",
        schema: "ob-poc",
        table: "risk_ratings",
        key_arg: "code",
        key_column: "code",
        return_column: "code",
        return_kind: RefdataReturnKind::String,
        active_column: Some("is_active"),
        order_by: "severity_level",
        fields: RISK_RATING_FIELDS,
        list_filters: &["is-active"],
    },
    RefdataDomainSpec {
        domain: "case-type",
        schema: "ob-poc",
        table: "case_types",
        key_arg: "code",
        key_column: "code",
        return_column: "code",
        return_kind: RefdataReturnKind::String,
        active_column: Some("is_active"),
        order_by: "display_order",
        fields: SIMPLE_CODE_FIELDS,
        list_filters: &["is-active"],
    },
];

enum RefdataBoundValue {
    String(String),
    Bool(bool),
    Int(i64),
}

fn resolve_domain_spec(args: &JsonValue) -> Result<&'static RefdataDomainSpec> {
    let domain = args
        .get("domain")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("refdata.* requires :domain"))?
        .to_lowercase();
    REFDATA_DOMAINS
        .iter()
        .find(|s| s.domain == domain)
        .ok_or_else(|| anyhow!("Unsupported refdata domain: {}", domain))
}

fn string_arg(args: &JsonValue, f: RefdataField) -> Option<String> {
    args.get(f.arg)
        .and_then(|v| v.as_str().map(ToOwned::to_owned))
}
fn bool_arg(args: &JsonValue, f: RefdataField) -> Option<bool> {
    args.get(f.arg)
        .and_then(|v| v.as_bool())
        .or(match f.default {
            Some(RefdataDefaultValue::Bool(v)) => Some(v),
            _ => None,
        })
}
fn int_arg(args: &JsonValue, f: RefdataField) -> Option<i64> {
    args.get(f.arg)
        .and_then(|v| v.as_i64())
        .or(match f.default {
            Some(RefdataDefaultValue::Int(v)) => Some(v),
            _ => None,
        })
}

async fn do_ensure(
    scope: &mut dyn TransactionScope,
    args: &JsonValue,
    spec: &RefdataDomainSpec,
) -> Result<VerbExecutionOutcome> {
    let mut present_fields = Vec::new();
    let mut qb =
        QueryBuilder::<Postgres>::new(format!("INSERT INTO \"{}\".{} (", spec.schema, spec.table));
    {
        let mut cols = qb.separated(", ");
        for f in spec.fields {
            match f.arg_type {
                RefdataArgType::String => {
                    let v = string_arg(args, *f);
                    if f.required && v.is_none() {
                        bail!("Missing required argument: {}", f.arg);
                    }
                    if v.is_some() {
                        cols.push(f.column);
                        present_fields.push((*f, v.map(RefdataBoundValue::String)));
                    }
                }
                RefdataArgType::Bool => {
                    let v = bool_arg(args, *f);
                    if f.required && v.is_none() {
                        bail!("Missing required argument: {}", f.arg);
                    }
                    if v.is_some() {
                        cols.push(f.column);
                        present_fields.push((*f, v.map(RefdataBoundValue::Bool)));
                    }
                }
                RefdataArgType::Int => {
                    let v = int_arg(args, *f);
                    if f.required && v.is_none() {
                        bail!("Missing required argument: {}", f.arg);
                    }
                    if v.is_some() {
                        cols.push(f.column);
                        present_fields.push((*f, v.map(RefdataBoundValue::Int)));
                    }
                }
            }
        }
    }
    qb.push(") VALUES (");
    {
        let mut vals = qb.separated(", ");
        for (_, value) in &present_fields {
            match value {
                Some(RefdataBoundValue::String(v)) => {
                    vals.push_bind(v);
                }
                Some(RefdataBoundValue::Bool(v)) => {
                    vals.push_bind(v);
                }
                Some(RefdataBoundValue::Int(v)) => {
                    vals.push_bind(v);
                }
                None => {}
            }
        }
    }
    qb.push(") ON CONFLICT (");
    qb.push(spec.key_column);
    qb.push(") DO UPDATE SET ");
    let mut first = true;
    for (f, _) in &present_fields {
        if f.column == spec.key_column {
            continue;
        }
        if !first {
            qb.push(", ");
        }
        first = false;
        qb.push(f.column);
        qb.push(" = EXCLUDED.");
        qb.push(f.column);
    }
    if first {
        qb.push(spec.key_column);
        qb.push(" = EXCLUDED.");
        qb.push(spec.key_column);
    }
    qb.push(" RETURNING ");
    qb.push(spec.return_column);

    match spec.return_kind {
        RefdataReturnKind::Uuid => {
            let q = qb.build_query_scalar::<uuid::Uuid>();
            Ok(VerbExecutionOutcome::Uuid(
                q.fetch_one(scope.executor()).await?,
            ))
        }
        RefdataReturnKind::String => {
            let q = qb.build_query_scalar::<String>();
            Ok(VerbExecutionOutcome::Record(serde_json::json!({
                spec.return_column: q.fetch_one(scope.executor()).await?
            })))
        }
    }
}

async fn do_read(
    scope: &mut dyn TransactionScope,
    args: &JsonValue,
    spec: &RefdataDomainSpec,
) -> Result<VerbExecutionOutcome> {
    let key = args
        .get(spec.key_arg)
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing required argument: {}", spec.key_arg))?;
    let sql = format!(
        "SELECT row_to_json(t) FROM (SELECT * FROM \"{}\".{} WHERE {} = $1) t",
        spec.schema, spec.table, spec.key_column
    );
    let record: Option<JsonValue> = sqlx::query_scalar(&sql)
        .bind(key)
        .fetch_optional(scope.executor())
        .await?;
    Ok(VerbExecutionOutcome::Record(record.ok_or_else(|| {
        anyhow!("No {} record found", spec.domain)
    })?))
}

async fn do_list(
    scope: &mut dyn TransactionScope,
    args: &JsonValue,
    spec: &RefdataDomainSpec,
) -> Result<VerbExecutionOutcome> {
    let mut qb = QueryBuilder::<Postgres>::new(format!(
        "SELECT COALESCE(json_agg(row_to_json(t)), '[]'::json) FROM (SELECT * FROM \"{}\".{}",
        spec.schema, spec.table
    ));
    let mut first_filter = true;
    for filter_arg in spec.list_filters {
        let field = spec
            .fields
            .iter()
            .find(|f| f.arg == *filter_arg)
            .ok_or_else(|| anyhow!("Missing field metadata for filter {}", filter_arg))?;
        match field.arg_type {
            RefdataArgType::String => {
                if let Some(value) = args.get(filter_arg).and_then(|v| v.as_str()) {
                    if first_filter {
                        qb.push(" WHERE ");
                        first_filter = false;
                    } else {
                        qb.push(" AND ");
                    }
                    qb.push(field.column);
                    qb.push(" = ");
                    qb.push_bind(value);
                }
            }
            RefdataArgType::Bool => {
                if let Some(value) = args.get(filter_arg).and_then(|v| v.as_bool()) {
                    if first_filter {
                        qb.push(" WHERE ");
                        first_filter = false;
                    } else {
                        qb.push(" AND ");
                    }
                    qb.push(field.column);
                    qb.push(" = ");
                    qb.push_bind(value);
                }
            }
            RefdataArgType::Int => {
                if let Some(value) = args.get(filter_arg).and_then(|v| v.as_i64()) {
                    if first_filter {
                        qb.push(" WHERE ");
                        first_filter = false;
                    } else {
                        qb.push(" AND ");
                    }
                    qb.push(field.column);
                    qb.push(" = ");
                    qb.push_bind(value);
                }
            }
        }
    }
    qb.push(" ORDER BY ");
    qb.push(spec.order_by);
    qb.push(") t");
    let rows: JsonValue = qb
        .build_query_scalar::<JsonValue>()
        .fetch_one(scope.executor())
        .await?;
    Ok(VerbExecutionOutcome::RecordSet(match rows {
        JsonValue::Array(items) => items,
        _ => Vec::new(),
    }))
}

async fn do_deactivate(
    scope: &mut dyn TransactionScope,
    args: &JsonValue,
    spec: &RefdataDomainSpec,
) -> Result<VerbExecutionOutcome> {
    let key = args
        .get(spec.key_arg)
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing required argument: {}", spec.key_arg))?;
    let sql = if let Some(active_column) = spec.active_column {
        format!(
            "UPDATE \"{}\".{} SET {} = FALSE WHERE {} = $1",
            spec.schema, spec.table, active_column, spec.key_column
        )
    } else {
        format!(
            "DELETE FROM \"{}\".{} WHERE {} = $1",
            spec.schema, spec.table, spec.key_column
        )
    };
    sqlx::query(&sql)
        .bind(key)
        .execute(scope.executor())
        .await?;
    Ok(VerbExecutionOutcome::Void)
}

// ── refdata.{ensure,read,list,deactivate} ─────────────────────────────────────

macro_rules! refdata_op {
    ($struct:ident, $verb:literal, $runner:ident) => {
        pub struct $struct;
        #[async_trait]
        impl SemOsVerbOp for $struct {
            fn fqn(&self) -> &str {
                concat!("refdata.", $verb)
            }
            async fn execute(
                &self,
                args: &JsonValue,
                _ctx: &mut VerbExecutionContext,
                scope: &mut dyn TransactionScope,
            ) -> Result<VerbExecutionOutcome> {
                let spec = resolve_domain_spec(args)?;
                $runner(scope, args, spec).await
            }
        }
    };
}

refdata_op!(Ensure, "ensure", do_ensure);
refdata_op!(Read, "read", do_read);
refdata_op!(List, "list", do_list);
refdata_op!(Deactivate, "deactivate", do_deactivate);
