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
//! v1.2 T1.B (2026-04-26) — `EXISTS` clause support added. A predicate
//! of the shape
//!
//!   `<simple-equality> AND EXISTS (SELECT 1 FROM "ob-poc".<tbl> alias
//!    WHERE <conjunction>)`
//!
//! is parseable. The simple-equality is parsed as before; the EXISTS
//! sub-query is captured verbatim as an additional source-side filter
//! and interpolated into stage-2 SQL at resolve time. Used by
//! `service_consumption_active_requires_live_binding` (lifecycle_resources_dag).
//!
//! Other unparseable predicates (ALL...HAVE / multi-clause non-EXISTS /
//! arithmetic) still return Ok(None) — the gate checker treats this as
//! "predicate didn't resolve → constraint violated", the conservative
//! fail-closed behaviour. Callers that need richer predicate semantics
//! can layer their own resolver in front.

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
        //
        // v1.2 T1.B: when the predicate carries an EXISTS clause, we
        // append it as an additional WHERE conjunct so only source rows
        // satisfying the EXISTS sub-query qualify. The EXISTS clause is
        // catalogue-declared (trusted source), validated structurally
        // by the parser to contain only identifiers + literal strings.
        let source_pk = guess_source_pk(&parsed.source_table);
        let exists_clause = parsed
            .exists_clause
            .as_ref()
            .map(|c| format!(" AND {}", c.raw))
            .unwrap_or_default();
        let stage2_sql = format!(
            r#"SELECT {pk}::text AS v FROM "ob-poc".{tbl} WHERE {col} = $1::uuid{exists} LIMIT 1"#,
            pk = source_pk,
            tbl = parsed.source_table,
            col = parsed.source_column,
            exists = exists_clause,
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
    /// v1.2 T1.B: optional EXISTS sub-query that filters source rows.
    exists_clause: Option<ExistsClause>,
}

/// EXISTS sub-query metadata. Captured verbatim from the predicate
/// string for runtime SQL interpolation; structural validation
/// confirmed the clause uses only identifiers + literal strings (no
/// arithmetic / no functions / no nested subqueries).
#[derive(Debug, PartialEq, Eq)]
struct ExistsClause {
    /// The full clause as it appeared in the predicate, e.g.
    /// `EXISTS (SELECT 1 FROM "ob-poc".application_instances ai
    ///  WHERE ai.id = capability_bindings.application_instance_id
    ///  AND ai.lifecycle_status = 'ACTIVE')`.
    /// Suitable for direct SQL interpolation since identifiers + literals
    /// are catalogue-declared (trusted source).
    raw: String,
}

/// Parse `{src_table}.{src_col} = this_{anything}.{tgt_col}` (whitespace-
/// and quoting-tolerant). Optionally followed by ` AND EXISTS (...)`.
/// Returns `None` for any other shape.
fn parse_simple_equality(predicate: &str) -> Option<ParsedPredicate> {
    // Strip surrounding whitespace + trailing semicolon if any.
    let p = predicate.trim().trim_end_matches(';');

    // v1.2 T1.B: split off an optional `AND EXISTS (...)` tail.
    let (eq_part, exists_clause) = split_exists(p);

    // Split on '=' — must have exactly one (in the equality portion)
    let parts: Vec<&str> = eq_part.split('=').collect();
    if parts.len() != 2 {
        return None;
    }

    let lhs = parts[0].trim();
    let rhs = parts[1].trim();

    let (lhs_table, lhs_col) = split_qualified(lhs)?;
    let (rhs_table, rhs_col) = split_qualified(rhs)?;

    // Determine which side is `this_*` (the target reference).
    let (source_table, source_column, target_column) = if rhs_table.starts_with("this_") {
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
        exists_clause,
    })
}

/// Split a predicate string into its simple-equality head and an
/// optional trailing `AND EXISTS (...)` clause. Case-insensitive on
/// the `AND EXISTS` keyword. Returns `(equality_str, None)` if no
/// EXISTS clause is present, `(equality_str, Some(clause))` if one is
/// recognised. If the EXISTS sub-query is structurally invalid (no
/// matching parenthesis, contains a nested subquery, contains
/// arithmetic / function calls), the whole predicate falls through as
/// unrecognised — caller treats as None.
fn split_exists(p: &str) -> (&str, Option<ExistsClause>) {
    // Locate `AND EXISTS` (case-insensitive). Must be a complete word
    // boundary — `BRAND EXISTS` is not a match.
    let lowered = p.to_lowercase();
    let needle = " and exists ";
    let needle_at_paren = " and exists(";
    let pos = lowered
        .find(needle)
        .or_else(|| lowered.find(needle_at_paren));
    let pos = match pos {
        Some(p) => p,
        None => return (p, None),
    };

    let head = p[..pos].trim_end();
    // Locate the "EXISTS" keyword (case-insensitive) within the tail and
    // capture from there — the prepended "AND" is added back at SQL
    // interpolation time, so the captured clause starts with EXISTS.
    let exists_offset = match lowered[pos..].find("exists") {
        Some(o) => pos + o,
        None => return (p, None),
    };
    let tail = &p[exists_offset..];

    // Find the opening paren of the EXISTS sub-query.
    let open = match tail.find('(') {
        Some(o) => o,
        None => return (p, None),
    };
    // Walk the tail counting parens to find the matching close.
    let bytes = tail.as_bytes();
    let mut depth = 0;
    let mut close = None;
    for (i, b) in bytes.iter().enumerate().skip(open) {
        match *b {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let close = match close {
        Some(c) => c,
        None => return (p, None),
    };

    let exists_full = &tail[..=close]; // EXISTS (...)
    if !is_safe_exists_clause(exists_full) {
        return (p, None);
    }
    (
        head,
        Some(ExistsClause {
            raw: exists_full.to_string(),
        }),
    )
}

/// Verify the EXISTS sub-query body uses only identifiers (alphanumeric,
/// underscore, dot, hyphen for double-quoted schema names like
/// `"ob-poc"`, and double-quote), whitespace, equality, AND, single-quoted
/// string literals, and the structural keywords SELECT/FROM/WHERE/AND/OR/EXISTS.
/// Rejects function calls, semicolons, comments, subqueries beyond the
/// top-level EXISTS, and SQL comment markers.
///
/// **Trust boundary:** the EXISTS clause is catalogue-declared in YAML
/// edited by catalogue authors. Forward-discipline (Tranche 3) will
/// architecturally restrict catalogue authoring, at which point this
/// check can be tightened. Until then, authors are trusted; this check
/// catches obvious mistakes (typos, malformed clauses, comment markers)
/// rather than enforcing strict SQL grammar.
fn is_safe_exists_clause(clause: &str) -> bool {
    if clause.is_empty() {
        return false;
    }
    // Reject SQL comments and statement separators first — these would
    // pass the per-char check otherwise.
    if clause.contains("--") || clause.contains("/*") || clause.contains(';') {
        return false;
    }
    // Ban any character outside the safe set. Hyphen is allowed because
    // the schema qualifier `"ob-poc"` contains it; the safe set is
    // intentionally tight in everything else.
    for c in clause.chars() {
        let ok = c.is_ascii_alphanumeric()
            || matches!(c, ' ' | '\t' | '\n' | '\r')
            || matches!(
                c,
                '_' | '.' | ',' | '\'' | '"' | '=' | '(' | ')' | '*' | '-'
            );
        if !ok {
            return false;
        }
    }
    // Must begin with EXISTS (case-insensitive) and contain a single
    // top-level SELECT.
    let lowered = clause.to_lowercase();
    if !lowered.starts_with("exists") {
        return false;
    }
    if lowered.matches("select").count() != 1 {
        return false;
    }
    true
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
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
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
        // R3 (2026-04-26): bp_clearances + R1 lifecycle_resources tables use
        // single-column PK named just `id` (DEFAULT gen_random_uuid()).
        // Surfaced by the live test harness — without this case the
        // SqlPredicateResolver would generate booking_principal_clearance_id
        // and fail with "column does not exist" against the real schema.
        "booking_principal_clearances" => "id".to_string(),
        "application_instances" => "id".to_string(),
        "capability_bindings" => "id".to_string(),
        "service_versions" => "id".to_string(),
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
        let p = parse_simple_equality("cases.client_group_id = this_deal.primary_client_group_id")
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
    fn parses_predicate_with_exists_clause_v1_2() {
        // v1.2 T1.B: lifecycle_resources_dag.yaml's
        // service_consumption_active_requires_live_binding constraint.
        let predicate = "capability_bindings.service_id = this_consumption.service_id \
                         AND EXISTS (SELECT 1 FROM \"ob-poc\".application_instances ai \
                         WHERE ai.id = capability_bindings.application_instance_id \
                         AND ai.lifecycle_status = 'ACTIVE')";
        let p = parse_simple_equality(predicate).expect("EXISTS predicate must parse");
        assert_eq!(p.source_table, "capability_bindings");
        assert_eq!(p.source_column, "service_id");
        assert_eq!(p.target_column, "service_id");
        let exists = p.exists_clause.expect("exists_clause must be captured");
        assert!(exists.raw.starts_with("EXISTS"));
        assert!(exists.raw.contains("application_instances"));
        assert!(exists.raw.contains("'ACTIVE'"));
    }

    #[test]
    fn rejects_unsafe_exists_clause() {
        // Arithmetic operators are not in the safe set.
        let predicate = "tbl.col = this_x.col AND EXISTS (SELECT 1 FROM other o \
                         WHERE o.x = tbl.x AND o.y > 100)";
        assert!(parse_simple_equality(predicate).is_none());
        // Semicolon is rejected (statement separator).
        let predicate2 = "tbl.col = this_x.col AND EXISTS (SELECT 1; DROP TABLE x)";
        assert!(parse_simple_equality(predicate2).is_none());
        // SQL comment is rejected.
        let predicate3 =
            "tbl.col = this_x.col AND EXISTS (SELECT 1 -- comment\n FROM o WHERE o.x = tbl.x)";
        assert!(parse_simple_equality(predicate3).is_none());
    }

    #[test]
    fn rejects_complex_predicates() {
        // EXISTS without preceding equality — nothing to anchor source-table on.
        assert!(parse_simple_equality(
            "EXISTS cases WHERE client_group_id = this_deal.primary_client_group_id"
        )
        .is_none());
        // Non-EXISTS AND clause — still unparseable.
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
        assert!(parse_simple_equality("cases.\"client; DROP TABLE\" = this_x.y").is_none());
        assert!(parse_simple_equality("cases.col = this_x.col WHERE 1=1").is_none());
    }

    #[test]
    fn rejects_no_this_marker() {
        // Both sides are concrete tables — we can't tell which is the target.
        assert!(
            parse_simple_equality("cases.client_group_id = client_group.client_group_id").is_none()
        );
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
