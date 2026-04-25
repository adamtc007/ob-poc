//! SqlPredicateResolver — production PredicateResolver for the
//! canonical foreign-key-equality predicates used in v1.3 DAGs.
//!
//! The PredicateResolver trait answers "given a `source_predicate`
//! string and the target entity_id, which source row's state should we
//! look up?" Most v1.3 cross-workspace constraints use a single shape:
//!
//!     `{source_table}.{source_column} = this_{slot_alias}.{target_column}`
//!
//! e.g.:
//!   `cases.client_group_id = this_deal.primary_client_group_id`
//!   `deals.primary_client_group_id = this_cbu.primary_client_group_id`
//!
//! This resolver:
//!   1. Parses that pattern via regex.
//!   2. Looks up `target_column` from the target slot's table for
//!      target_entity_id.
//!   3. Looks up the source-table primary key whose `source_column`
//!      matches the value from step 2.
//!   4. Returns that pk as the resolved source_entity_id.
//!
//! Unparseable predicates (EXISTS / ALL...HAVE / multi-clause) return
//! Ok(None) — the gate checker treats this as "predicate didn't
//! resolve → constraint violated", which is the conservative fail-closed
//! behaviour. Callers that need richer predicate semantics can layer
//! their own resolver in front.

use anyhow::Result;
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::gate_checker::PredicateResolver;
use super::slot_state::resolve_slot_table;

// ---------------------------------------------------------------------------
// Resolver
// ---------------------------------------------------------------------------

/// SqlPredicateResolver — handles `src.col = this_X.col` predicates by
/// running two SELECTs against Postgres.
#[derive(Debug, Default, Clone)]
pub struct SqlPredicateResolver;

#[async_trait]
impl PredicateResolver for SqlPredicateResolver {
    async fn resolve_source_entity(
        &self,
        predicate: &str,
        target_entity_id: Uuid,
        target_workspace: &str,
        target_slot: &str,
        pool: &PgPool,
    ) -> Result<Option<Uuid>> {
        let parsed = match parse_simple_equality(predicate) {
            Some(p) => p,
            None => {
                tracing::debug!(
                    predicate = %predicate,
                    "SqlPredicateResolver: unparseable predicate; returning None"
                );
                return Ok(None);
            }
        };

        // Resolve target table + pk column for the target slot.
        let (target_table, _state_col, target_pk) =
            resolve_slot_table(target_workspace, target_slot)?;

        // Step 1: read the target column off the target row.
        // Identifiers are sourced from the static slot dispatch table OR
        // the parsed predicate (column name); both are alphanumeric +
        // underscore by structural construction (validator + lint), so
        // safe to interpolate.
        let stage1_sql = format!(
            r#"SELECT {col}::text AS v FROM "ob-poc".{tbl} WHERE {pk} = $1"#,
            col = parsed.target_column,
            tbl = target_table,
            pk = target_pk,
        );
        let target_value: Option<String> = sqlx::query(&stage1_sql)
            .bind(target_entity_id)
            .fetch_optional(pool)
            .await?
            .and_then(|r| r.try_get::<Option<String>, _>("v").unwrap_or(None));

        let target_value = match target_value {
            Some(v) => v,
            None => return Ok(None),
        };

        // Step 2: find a source-table row whose source_column matches.
        // The source row's PK is what we return.
        let source_pk = guess_source_pk(&parsed.source_table);
        let stage2_sql = format!(
            r#"SELECT {pk}::text AS v FROM "ob-poc".{tbl} WHERE {col} = $1::uuid LIMIT 1"#,
            pk = source_pk,
            tbl = parsed.source_table,
            col = parsed.source_column,
        );
        let row = sqlx::query(&stage2_sql)
            .bind(&target_value)
            .fetch_optional(pool)
            .await?;
        match row {
            None => Ok(None),
            Some(r) => {
                let v: Option<String> = r.try_get::<Option<String>, _>("v").unwrap_or(None);
                match v {
                    None => Ok(None),
                    Some(s) => Ok(Some(Uuid::parse_str(&s)?)),
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Predicate parsing
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
struct ParsedPredicate {
    /// e.g. `"cases"`
    source_table: String,
    /// e.g. `"client_group_id"`
    source_column: String,
    /// e.g. `"primary_client_group_id"`
    target_column: String,
}

/// Parse `{src_table}.{src_col} = this_{anything}.{tgt_col}` (whitespace-
/// and quoting-tolerant). Returns `None` for any other shape.
fn parse_simple_equality(predicate: &str) -> Option<ParsedPredicate> {
    // Strip surrounding whitespace + trailing semicolon if any.
    let p = predicate.trim().trim_end_matches(';');

    // Split on '=' — must have exactly one
    let parts: Vec<&str> = p.split('=').collect();
    if parts.len() != 2 {
        return None;
    }

    let lhs = parts[0].trim();
    let rhs = parts[1].trim();

    let (lhs_table, lhs_col) = split_qualified(lhs)?;
    let (rhs_table, rhs_col) = split_qualified(rhs)?;

    // Determine which side is `this_*` (the target reference).
    let (source_table, source_column, target_column) =
        if rhs_table.starts_with("this_") {
            (lhs_table, lhs_col, rhs_col)
        } else if lhs_table.starts_with("this_") {
            (rhs_table, rhs_col, lhs_col)
        } else {
            return None;
        };

    // Identifier hygiene: alphanumeric + underscore only.
    if !is_safe_ident(&source_table)
        || !is_safe_ident(&source_column)
        || !is_safe_ident(&target_column)
    {
        return None;
    }

    Some(ParsedPredicate {
        source_table,
        source_column,
        target_column,
    })
}

fn split_qualified(s: &str) -> Option<(String, String)> {
    let mut parts = s.splitn(2, '.');
    let table = parts.next()?.trim().to_string();
    let col = parts.next()?.trim().to_string();
    if table.is_empty() || col.is_empty() {
        None
    } else {
        Some((table, col))
    }
}

fn is_safe_ident(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Best-effort source-PK column inference. Most ob-poc tables use
/// `{singular}_id` as the PK (e.g. cases.case_id, deals.deal_id).
/// If the table name doesn't follow the convention, the resolver will
/// surface a SQL error which the caller turns into a constraint
/// violation.
fn guess_source_pk(table: &str) -> String {
    // Special-case the few that diverge from the {singular}_id pattern
    // (manco_regulatory_status keys by manco_entity_id, etc.).
    match table {
        "manco_regulatory_status" => "manco_entity_id".to_string(),
        "cbu_trading_activity" => "cbu_id".to_string(),
        "cbu_service_consumption" => "consumption_id".to_string(),
        // {plural}s → {singular}_id
        t if t.ends_with('s') => format!("{}_id", t.trim_end_matches('s')),
        // Fallback: id
        _ => "id".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_canonical_kyc_deal_predicate() {
        let p = parse_simple_equality(
            "cases.client_group_id = this_deal.primary_client_group_id",
        )
        .expect("should parse");
        assert_eq!(p.source_table, "cases");
        assert_eq!(p.source_column, "client_group_id");
        assert_eq!(p.target_column, "primary_client_group_id");
    }

    #[test]
    fn parses_lhs_target_form() {
        // this_X on the LHS instead of RHS — should still parse.
        let p = parse_simple_equality(
            "this_cbu.primary_client_group_id = deals.primary_client_group_id",
        )
        .expect("should parse");
        assert_eq!(p.source_table, "deals");
        assert_eq!(p.source_column, "primary_client_group_id");
        assert_eq!(p.target_column, "primary_client_group_id");
    }

    #[test]
    fn rejects_complex_predicates() {
        // Multi-clause / EXISTS / ALL...HAVE — we don't try to parse.
        assert!(parse_simple_equality(
            "EXISTS cases WHERE client_group_id = this_deal.primary_client_group_id"
        )
        .is_none());
        assert!(parse_simple_equality(
            "trading_profiles.cbu_id = this_cbu.cbu_id AND status = 'ACTIVE'"
        )
        .is_none());
        assert!(parse_simple_equality(
            "ALL cbu_evidence WHERE cbu_id = this_cbu.cbu_id HAVE verification_status = 'VERIFIED'"
        )
        .is_none());
    }

    #[test]
    fn rejects_unsafe_identifiers() {
        // SQL-injection-ish identifiers blocked by is_safe_ident.
        assert!(parse_simple_equality(
            "cases.\"client; DROP TABLE\" = this_x.y"
        )
        .is_none());
        assert!(parse_simple_equality("cases.col = this_x.col WHERE 1=1").is_none());
    }

    #[test]
    fn rejects_no_this_marker() {
        // Both sides are concrete tables — we can't tell which is the target.
        assert!(parse_simple_equality(
            "cases.client_group_id = client_group.client_group_id"
        )
        .is_none());
    }

    #[test]
    fn guess_source_pk_canonical() {
        assert_eq!(guess_source_pk("cases"), "case_id");
        assert_eq!(guess_source_pk("deals"), "deal_id");
        assert_eq!(guess_source_pk("cbus"), "cbu_id");
        assert_eq!(
            guess_source_pk("manco_regulatory_status"),
            "manco_entity_id"
        );
        assert_eq!(guess_source_pk("cbu_trading_activity"), "cbu_id");
    }
}
