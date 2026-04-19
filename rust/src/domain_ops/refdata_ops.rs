use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use serde_json::Value as JsonValue;

#[cfg(feature = "database")]
use sqlx::{PgPool, Postgres, QueryBuilder};

use super::sem_os_helpers::{get_bool_arg, get_int_arg, get_string_arg};
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

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

fn resolve_domain_spec(verb_call: &VerbCall) -> Result<&'static RefdataDomainSpec> {
    let domain = get_string_arg(verb_call, "domain")
        .ok_or_else(|| anyhow!("refdata.* requires :domain"))?
        .to_lowercase();
    REFDATA_DOMAINS
        .iter()
        .find(|spec| spec.domain == domain)
        .ok_or_else(|| anyhow!("Unsupported refdata domain: {}", domain))
}

fn resolve_domain_spec_json(args: &serde_json::Value) -> Result<&'static RefdataDomainSpec> {
    let domain = args
        .get("domain")
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow!("refdata.* requires :domain"))?
        .to_lowercase();
    REFDATA_DOMAINS
        .iter()
        .find(|spec| spec.domain == domain)
        .ok_or_else(|| anyhow!("Unsupported refdata domain: {}", domain))
}

fn string_arg_or_default(verb_call: &VerbCall, field: RefdataField) -> Option<String> {
    get_string_arg(verb_call, field.arg)
}

fn bool_arg_or_default(verb_call: &VerbCall, field: RefdataField) -> Option<bool> {
    get_bool_arg(verb_call, field.arg).or(match field.default {
        Some(RefdataDefaultValue::Bool(v)) => Some(v),
        _ => None,
    })
}

fn int_arg_or_default(verb_call: &VerbCall, field: RefdataField) -> Option<i64> {
    get_int_arg(verb_call, field.arg).or(match field.default {
        Some(RefdataDefaultValue::Int(v)) => Some(v),
        _ => None,
    })
}

fn string_json_arg_or_default(args: &serde_json::Value, field: RefdataField) -> Option<String> {
    args.get(field.arg)
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
}

fn bool_json_arg_or_default(args: &serde_json::Value, field: RefdataField) -> Option<bool> {
    args.get(field.arg)
        .and_then(|value| value.as_bool())
        .or(match field.default {
            Some(RefdataDefaultValue::Bool(v)) => Some(v),
            _ => None,
        })
}

fn int_json_arg_or_default(args: &serde_json::Value, field: RefdataField) -> Option<i64> {
    args.get(field.arg)
        .and_then(|value| value.as_i64())
        .or(match field.default {
            Some(RefdataDefaultValue::Int(v)) => Some(v),
            _ => None,
        })
}

#[cfg(feature = "database")]
async fn ensure_refdata(
    verb_call: &VerbCall,
    spec: &RefdataDomainSpec,
    pool: &PgPool,
) -> Result<ExecutionResult> {
    let mut present_fields = Vec::new();
    let mut qb =
        QueryBuilder::<Postgres>::new(format!("INSERT INTO \"{}\".{} (", spec.schema, spec.table));
    {
        let mut cols = qb.separated(", ");
        for field in spec.fields {
            match field.arg_type {
                RefdataArgType::String => {
                    let value = string_arg_or_default(verb_call, *field);
                    if field.required && value.is_none() {
                        bail!("Missing required argument: {}", field.arg);
                    }
                    if value.is_some() {
                        cols.push(field.column);
                        present_fields.push((*field, value.map(RefdataBoundValue::String)));
                    }
                }
                RefdataArgType::Bool => {
                    let value = bool_arg_or_default(verb_call, *field);
                    if field.required && value.is_none() {
                        bail!("Missing required argument: {}", field.arg);
                    }
                    if value.is_some() {
                        cols.push(field.column);
                        present_fields.push((*field, value.map(RefdataBoundValue::Bool)));
                    }
                }
                RefdataArgType::Int => {
                    let value = int_arg_or_default(verb_call, *field);
                    if field.required && value.is_none() {
                        bail!("Missing required argument: {}", field.arg);
                    }
                    if value.is_some() {
                        cols.push(field.column);
                        present_fields.push((*field, value.map(RefdataBoundValue::Int)));
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
    for (field, _) in &present_fields {
        if field.column == spec.key_column {
            continue;
        }
        if !first {
            qb.push(", ");
        }
        first = false;
        qb.push(field.column);
        qb.push(" = EXCLUDED.");
        qb.push(field.column);
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
            let query = qb.build_query_scalar::<uuid::Uuid>();
            Ok(ExecutionResult::Uuid(query.fetch_one(pool).await?))
        }
        RefdataReturnKind::String => {
            let query = qb.build_query_scalar::<String>();
            Ok(ExecutionResult::Record(serde_json::json!({
                spec.return_column: query.fetch_one(pool).await?
            })))
        }
    }
}

#[cfg(feature = "database")]
async fn ensure_refdata_json(
    args: &serde_json::Value,
    spec: &RefdataDomainSpec,
    pool: &PgPool,
) -> Result<dsl_runtime::VerbExecutionOutcome> {
    let mut present_fields = Vec::new();
    let mut qb =
        QueryBuilder::<Postgres>::new(format!("INSERT INTO \"{}\".{} (", spec.schema, spec.table));
    {
        let mut cols = qb.separated(", ");
        for field in spec.fields {
            match field.arg_type {
                RefdataArgType::String => {
                    let value = string_json_arg_or_default(args, *field);
                    if field.required && value.is_none() {
                        bail!("Missing required argument: {}", field.arg);
                    }
                    if value.is_some() {
                        cols.push(field.column);
                        present_fields.push((*field, value.map(RefdataBoundValue::String)));
                    }
                }
                RefdataArgType::Bool => {
                    let value = bool_json_arg_or_default(args, *field);
                    if field.required && value.is_none() {
                        bail!("Missing required argument: {}", field.arg);
                    }
                    if value.is_some() {
                        cols.push(field.column);
                        present_fields.push((*field, value.map(RefdataBoundValue::Bool)));
                    }
                }
                RefdataArgType::Int => {
                    let value = int_json_arg_or_default(args, *field);
                    if field.required && value.is_none() {
                        bail!("Missing required argument: {}", field.arg);
                    }
                    if value.is_some() {
                        cols.push(field.column);
                        present_fields.push((*field, value.map(RefdataBoundValue::Int)));
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
    for (field, _) in &present_fields {
        if field.column == spec.key_column {
            continue;
        }
        if !first {
            qb.push(", ");
        }
        first = false;
        qb.push(field.column);
        qb.push(" = EXCLUDED.");
        qb.push(field.column);
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
            let query = qb.build_query_scalar::<uuid::Uuid>();
            Ok(dsl_runtime::VerbExecutionOutcome::Uuid(
                query.fetch_one(pool).await?,
            ))
        }
        RefdataReturnKind::String => {
            let query = qb.build_query_scalar::<String>();
            Ok(dsl_runtime::VerbExecutionOutcome::Record(
                serde_json::json!({ spec.return_column: query.fetch_one(pool).await? }),
            ))
        }
    }
}

#[cfg(feature = "database")]
async fn read_refdata(
    verb_call: &VerbCall,
    spec: &RefdataDomainSpec,
    pool: &PgPool,
) -> Result<ExecutionResult> {
    let key = get_string_arg(verb_call, spec.key_arg)
        .ok_or_else(|| anyhow!("Missing required argument: {}", spec.key_arg))?;
    let sql = format!(
        "SELECT row_to_json(t) FROM (SELECT * FROM \"{}\".{} WHERE {} = $1) t",
        spec.schema, spec.table, spec.key_column
    );
    let record: Option<JsonValue> = sqlx::query_scalar(&sql)
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(ExecutionResult::Record(record.ok_or_else(|| {
        anyhow!("No {} record found", spec.domain)
    })?))
}

#[cfg(feature = "database")]
async fn read_refdata_json(
    args: &serde_json::Value,
    spec: &RefdataDomainSpec,
    pool: &PgPool,
) -> Result<dsl_runtime::VerbExecutionOutcome> {
    let key = args
        .get(spec.key_arg)
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow!("Missing required argument: {}", spec.key_arg))?;
    let sql = format!(
        "SELECT row_to_json(t) FROM (SELECT * FROM \"{}\".{} WHERE {} = $1) t",
        spec.schema, spec.table, spec.key_column
    );
    let record: Option<JsonValue> = sqlx::query_scalar(&sql)
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(dsl_runtime::VerbExecutionOutcome::Record(
        record.ok_or_else(|| anyhow!("No {} record found", spec.domain))?,
    ))
}

#[cfg(feature = "database")]
async fn list_refdata(
    verb_call: &VerbCall,
    spec: &RefdataDomainSpec,
    pool: &PgPool,
) -> Result<ExecutionResult> {
    let mut qb = QueryBuilder::<Postgres>::new(format!(
        "SELECT COALESCE(json_agg(row_to_json(t)), '[]'::json) FROM (SELECT * FROM \"{}\".{}",
        spec.schema, spec.table
    ));
    let mut first_filter = true;
    for filter_arg in spec.list_filters {
        let field = spec
            .fields
            .iter()
            .find(|field| field.arg == *filter_arg)
            .ok_or_else(|| anyhow!("Missing field metadata for filter {}", filter_arg))?;
        match field.arg_type {
            RefdataArgType::String => {
                if let Some(value) = get_string_arg(verb_call, filter_arg) {
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
                if let Some(value) = get_bool_arg(verb_call, filter_arg) {
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
                if let Some(value) = get_int_arg(verb_call, filter_arg) {
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
    let query = qb.build_query_scalar::<JsonValue>();
    let rows = query.fetch_one(pool).await?;
    match rows {
        JsonValue::Array(items) => Ok(ExecutionResult::RecordSet(items)),
        _ => Ok(ExecutionResult::RecordSet(Vec::new())),
    }
}

#[cfg(feature = "database")]
async fn list_refdata_json(
    args: &serde_json::Value,
    spec: &RefdataDomainSpec,
    pool: &PgPool,
) -> Result<dsl_runtime::VerbExecutionOutcome> {
    let mut qb = QueryBuilder::<Postgres>::new(format!(
        "SELECT COALESCE(json_agg(row_to_json(t)), '[]'::json) FROM (SELECT * FROM \"{}\".{}",
        spec.schema, spec.table
    ));
    let mut first_filter = true;
    for filter_arg in spec.list_filters {
        let field = spec
            .fields
            .iter()
            .find(|field| field.arg == *filter_arg)
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
    let query = qb.build_query_scalar::<JsonValue>();
    let rows = query.fetch_one(pool).await?;
    Ok(dsl_runtime::VerbExecutionOutcome::RecordSet(
        match rows {
            JsonValue::Array(items) => items,
            _ => Vec::new(),
        },
    ))
}

#[cfg(feature = "database")]
async fn deactivate_refdata(
    verb_call: &VerbCall,
    spec: &RefdataDomainSpec,
    pool: &PgPool,
) -> Result<ExecutionResult> {
    let key = get_string_arg(verb_call, spec.key_arg)
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
    sqlx::query(&sql).bind(key).execute(pool).await?;
    Ok(ExecutionResult::Void)
}

#[cfg(feature = "database")]
async fn deactivate_refdata_json(
    args: &serde_json::Value,
    spec: &RefdataDomainSpec,
    pool: &PgPool,
) -> Result<dsl_runtime::VerbExecutionOutcome> {
    let key = args
        .get(spec.key_arg)
        .and_then(|value| value.as_str())
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
    sqlx::query(&sql).bind(key).execute(pool).await?;
    Ok(dsl_runtime::VerbExecutionOutcome::Void)
}

enum RefdataBoundValue {
    String(String),
    Bool(bool),
    Int(i64),
}

#[register_custom_op]
pub struct RefdataEnsureOp;

#[async_trait]
impl CustomOperation for RefdataEnsureOp {
    fn domain(&self) -> &'static str {
        "refdata"
    }

    fn verb(&self) -> &'static str {
        "ensure"
    }

    fn rationale(&self) -> &'static str {
        "Unified refdata ensure needs domain-driven table and column routing across multiple \
         reference tables"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let spec = resolve_domain_spec(verb_call)?;
        ensure_refdata(verb_call, spec, pool).await
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let spec = resolve_domain_spec_json(args)?;
        ensure_refdata_json(args, spec, pool).await
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

#[register_custom_op]
pub struct RefdataReadOp;

#[async_trait]
impl CustomOperation for RefdataReadOp {
    fn domain(&self) -> &'static str {
        "refdata"
    }

    fn verb(&self) -> &'static str {
        "read"
    }

    fn rationale(&self) -> &'static str {
        "Unified refdata read needs domain-driven table and key selection across multiple \
         reference tables"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let spec = resolve_domain_spec(verb_call)?;
        read_refdata(verb_call, spec, pool).await
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let spec = resolve_domain_spec_json(args)?;
        read_refdata_json(args, spec, pool).await
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

#[register_custom_op]
pub struct RefdataListOp;

#[async_trait]
impl CustomOperation for RefdataListOp {
    fn domain(&self) -> &'static str {
        "refdata"
    }

    fn verb(&self) -> &'static str {
        "list"
    }

    fn rationale(&self) -> &'static str {
        "Unified refdata list needs domain-driven table routing and per-domain filter handling"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let spec = resolve_domain_spec(verb_call)?;
        list_refdata(verb_call, spec, pool).await
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let spec = resolve_domain_spec_json(args)?;
        list_refdata_json(args, spec, pool).await
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

#[register_custom_op]
pub struct RefdataDeactivateOp;

#[async_trait]
impl CustomOperation for RefdataDeactivateOp {
    fn domain(&self) -> &'static str {
        "refdata"
    }

    fn verb(&self) -> &'static str {
        "deactivate"
    }

    fn rationale(&self) -> &'static str {
        "Unified refdata deactivate needs domain-driven update or delete semantics"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let spec = resolve_domain_spec(verb_call)?;
        deactivate_refdata(verb_call, spec, pool).await
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let spec = resolve_domain_spec_json(args)?;
        deactivate_refdata_json(args, spec, pool).await
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl_v2::{Argument, AstNode, Literal, Span};
    use serde_json::Value;

    fn string_node(value: &str) -> AstNode {
        AstNode::Literal(Literal::String(value.to_string()), Span::default())
    }

    fn bool_node(value: bool) -> AstNode {
        AstNode::Literal(Literal::Boolean(value), Span::default())
    }

    fn int_node(value: i64) -> AstNode {
        AstNode::Literal(Literal::Integer(value), Span::default())
    }

    fn make_verb_call(verb: &str, args: Vec<(&str, AstNode)>) -> VerbCall {
        VerbCall {
            domain: "refdata".to_string(),
            verb: verb.to_string(),
            arguments: args
                .into_iter()
                .map(|(key, value)| Argument {
                    key: key.to_string(),
                    value,
                    span: Span::default(),
                })
                .collect(),
            binding: None,
            span: Span::default(),
        }
    }

    #[test]
    fn resolve_domain_spec_for_currency() {
        let verb_call = make_verb_call("ensure", vec![("domain", string_node("currency"))]);
        let spec = resolve_domain_spec(&verb_call).expect("currency spec should resolve");
        assert_eq!(spec.table, "currencies");
        assert_eq!(spec.key_arg, "iso-code");
        assert!(matches!(spec.return_kind, RefdataReturnKind::Uuid));
    }

    #[test]
    fn resolve_domain_spec_rejects_unknown_domain() {
        let verb_call = make_verb_call("ensure", vec![("domain", string_node("unknown"))]);
        match resolve_domain_spec(&verb_call) {
            Ok(_) => panic!("unknown domain should fail"),
            Err(err) => assert!(err.to_string().contains("Unsupported refdata domain")),
        }
    }

    #[cfg(feature = "database")]
    fn database_url() -> String {
        std::env::var("TEST_DATABASE_URL")
            .or_else(|_| std::env::var("DATABASE_URL"))
            .unwrap_or_else(|_| "postgresql:///data_designer".to_string())
    }

    #[cfg(feature = "database")]
    async fn connect_pool() -> PgPool {
        PgPool::connect(&database_url())
            .await
            .expect("failed to connect to test database")
    }

    #[cfg(feature = "database")]
    fn record_field<'a>(record: &'a Value, key: &str) -> &'a Value {
        record
            .get(key)
            .unwrap_or_else(|| panic!("missing field {}", key))
    }

    #[cfg(feature = "database")]
    #[tokio::test]
    async fn currency_refdata_round_trips_with_soft_deactivate() {
        let pool = connect_pool().await;
        let code = "ZZT";

        sqlx::query(r#"DELETE FROM "ob-poc".currencies WHERE iso_code = $1"#)
            .bind(code)
            .execute(&pool)
            .await
            .expect("cleanup before test");

        let ensure_call = make_verb_call(
            "ensure",
            vec![
                ("domain", string_node("currency")),
                ("iso-code", string_node(code)),
                ("name", string_node("Phase Zero Test Currency")),
                ("symbol", string_node("Z")),
                ("decimal-places", int_node(3)),
            ],
        );

        let ensure_result = ensure_refdata(
            &ensure_call,
            resolve_domain_spec(&ensure_call).unwrap(),
            &pool,
        )
        .await
        .expect("currency ensure should succeed");
        assert!(matches!(ensure_result, ExecutionResult::Uuid(_)));

        let read_call = make_verb_call(
            "read",
            vec![
                ("domain", string_node("currency")),
                ("iso-code", string_node(code)),
            ],
        );
        let read_result = read_refdata(&read_call, resolve_domain_spec(&read_call).unwrap(), &pool)
            .await
            .expect("currency read should succeed");
        let record = match read_result {
            ExecutionResult::Record(record) => record,
            other => panic!("unexpected read result: {:?}", other),
        };
        assert_eq!(record_field(&record, "iso_code"), "ZZT");
        assert_eq!(record_field(&record, "decimal_places"), 3);

        let list_call = make_verb_call(
            "list",
            vec![
                ("domain", string_node("currency")),
                ("is-active", bool_node(true)),
            ],
        );
        let list_result = list_refdata(&list_call, resolve_domain_spec(&list_call).unwrap(), &pool)
            .await
            .expect("currency list should succeed");
        let rows = match list_result {
            ExecutionResult::RecordSet(rows) => rows,
            other => panic!("unexpected list result: {:?}", other),
        };
        assert!(rows
            .iter()
            .any(|row| record_field(row, "iso_code") == "ZZT"));

        let deactivate_call = make_verb_call(
            "deactivate",
            vec![
                ("domain", string_node("currency")),
                ("iso-code", string_node(code)),
            ],
        );
        let deactivate_result = deactivate_refdata(
            &deactivate_call,
            resolve_domain_spec(&deactivate_call).unwrap(),
            &pool,
        )
        .await
        .expect("currency deactivate should succeed");
        assert!(matches!(deactivate_result, ExecutionResult::Void));

        let read_after = read_refdata(&read_call, resolve_domain_spec(&read_call).unwrap(), &pool)
            .await
            .expect("currency read after deactivate should still succeed");
        let record_after = match read_after {
            ExecutionResult::Record(record) => record,
            other => panic!("unexpected read-after result: {:?}", other),
        };
        assert_eq!(record_field(&record_after, "is_active"), false);

        sqlx::query(r#"DELETE FROM "ob-poc".currencies WHERE iso_code = $1"#)
            .bind(code)
            .execute(&pool)
            .await
            .expect("cleanup after test");
    }

    #[cfg(feature = "database")]
    #[tokio::test]
    async fn jurisdiction_refdata_round_trips_with_delete_deactivate() {
        let pool = connect_pool().await;
        let code = "ZZ-TEST";

        sqlx::query(r#"DELETE FROM "ob-poc".master_jurisdictions WHERE jurisdiction_code = $1"#)
            .bind(code)
            .execute(&pool)
            .await
            .expect("cleanup before test");

        let ensure_call = make_verb_call(
            "ensure",
            vec![
                ("domain", string_node("jurisdiction")),
                ("code", string_node(code)),
                ("name", string_node("Phase Zero Test Jurisdiction")),
                ("country-code", string_node("GB")),
                ("region", string_node("TEST")),
                ("regulatory-framework", string_node("PHASE0")),
            ],
        );
        let ensure_result = ensure_refdata(
            &ensure_call,
            resolve_domain_spec(&ensure_call).unwrap(),
            &pool,
        )
        .await
        .expect("jurisdiction ensure should succeed");
        let record = match ensure_result {
            ExecutionResult::Record(record) => record,
            other => panic!("unexpected jurisdiction ensure result: {:?}", other),
        };
        assert_eq!(record_field(&record, "jurisdiction_code"), code);

        let read_call = make_verb_call(
            "read",
            vec![
                ("domain", string_node("jurisdiction")),
                ("code", string_node(code)),
            ],
        );
        let read_result = read_refdata(&read_call, resolve_domain_spec(&read_call).unwrap(), &pool)
            .await
            .expect("jurisdiction read should succeed");
        let record = match read_result {
            ExecutionResult::Record(record) => record,
            other => panic!("unexpected jurisdiction read result: {:?}", other),
        };
        assert_eq!(record_field(&record, "jurisdiction_code"), code);
        assert_eq!(record_field(&record, "region"), "TEST");

        let list_call = make_verb_call(
            "list",
            vec![
                ("domain", string_node("jurisdiction")),
                ("region", string_node("TEST")),
            ],
        );
        let list_result = list_refdata(&list_call, resolve_domain_spec(&list_call).unwrap(), &pool)
            .await
            .expect("jurisdiction list should succeed");
        let rows = match list_result {
            ExecutionResult::RecordSet(rows) => rows,
            other => panic!("unexpected jurisdiction list result: {:?}", other),
        };
        assert!(rows
            .iter()
            .any(|row| record_field(row, "jurisdiction_code") == code));

        let deactivate_call = make_verb_call(
            "deactivate",
            vec![
                ("domain", string_node("jurisdiction")),
                ("code", string_node(code)),
            ],
        );
        let deactivate_result = deactivate_refdata(
            &deactivate_call,
            resolve_domain_spec(&deactivate_call).unwrap(),
            &pool,
        )
        .await
        .expect("jurisdiction deactivate should succeed");
        assert!(matches!(deactivate_result, ExecutionResult::Void));

        let read_after =
            read_refdata(&read_call, resolve_domain_spec(&read_call).unwrap(), &pool).await;
        assert!(read_after.is_err());
    }
}
