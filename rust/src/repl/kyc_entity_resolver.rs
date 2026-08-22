//! T4.5 (`EOP-PLAN-KYCUBO-KIT-001` §T4.5) — exact-match entity-handle /
//! quoted-entity-name resolver for the KYC super-user workbook surface.
//!
//! Scope ruling (plan v0.6 §T4.5): resolves `@handle` and quoted-entity-name
//! occurrences that appear as the VALUE of one of the five target-binding
//! slots (`subject-id`, `edge-id`, `entity-id`, `person-id`, `obligation-id`)
//! in raw KYC workbook DSL text, into UUID literals — via an EXACT match
//! against `"ob-poc".entities.name_norm`. That column already exists and is
//! already an established exact-match lookup surface: it is maintained by
//! the `"ob-poc".update_entity_name_norm()` trigger (`LOWER(TRIM(REGEXP_REPLACE(
//! name, '[^a-zA-Z0-9 ]', ' ', 'g')))`) and is independently consumed today by
//! `crates/ob-poc-entity-linking` (`compiler.rs`'s `COALESCE(e.name_norm,
//! LOWER(e.name))` snapshot build, `snapshot.rs::lookup_by_name`) — this
//! module adds a second, EXACT (not fuzzy) consumer of the same column,
//! rather than inventing a new lookup table (the mandatory recon halt
//! condition: recon found this source already exists in the tree).
//!
//! Zero fuzzy matching, by construction: the SQL predicate is `name_norm =
//! $1` (equality), never `ILIKE`/similarity/embedding. Unknown name -> error.
//! Ambiguous name (>1 row, e.g. two same-named entities) -> error naming
//! every candidate UUID. This runs at STAGE time, BEFORE DSL recognition:
//! callers pass the RESOLVED text (this function's `Ok` output) into
//! `KycWorkbook::stage`, which re-parses only UUID-literal text (see that
//! module's own scope fence). The canonical resolved render — never a
//! handle — is therefore what ends up in `StagedMove::source_text`.

use regex::Regex;
use sqlx::PgConnection;
use uuid::Uuid;

/// The five target-binding slot names `KycWorkbook`'s parser recognises
/// (`ob-poc-kyc-substrate`'s `TargetBinding` fields, rendered under their
/// canonical hyphenated DSL arg names — see `kyc_workbook.rs`'s own
/// `sexpr_to_parsed_move` doc comment, which this module mirrors rather than
/// re-derives, since the two must never drift).
const TARGET_SLOTS: &str = "subject-id|edge-id|entity-id|person-id|obligation-id";

#[derive(Debug, thiserror::Error)]
pub(crate) enum ResolverError {
    #[error(
        "unknown entity handle/name {name:?} — no row in \"ob-poc\".entities matches \
         (exact match on name_norm; zero fuzzy matching by design)"
    )]
    Unknown { name: String },
    #[error(
        "ambiguous entity handle/name {name:?} — {} candidates match: {candidates:?} \
         (disambiguate by UUID or make the name unique)",
        candidates.len()
    )]
    Ambiguous { name: String, candidates: Vec<Uuid> },
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

/// Mirrors `"ob-poc".update_entity_name_norm()` exactly: replace every
/// character that isn't ASCII alphanumeric or space with a space, collapse
/// whitespace, lowercase. Any drift from the trigger's SQL would make this
/// an approximate match, not an exact one — kept as a literal one-to-one
/// port of the trigger body, not a "close enough" reimplementation.
fn normalize(raw: &str) -> String {
    let replaced: String = raw
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == ' ' { c } else { ' ' })
        .collect();
    replaced
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

async fn lookup_exact(conn: &mut PgConnection, name: &str) -> Result<Uuid, ResolverError> {
    let norm = normalize(name);
    let rows: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT entity_id FROM "ob-poc".entities WHERE name_norm = $1 AND deleted_at IS NULL"#,
    )
    .bind(&norm)
    .fetch_all(conn)
    .await?;

    match rows.len() {
        0 => Err(ResolverError::Unknown {
            name: name.to_string(),
        }),
        1 => Ok(rows[0]),
        _ => Err(ResolverError::Ambiguous {
            name: name.to_string(),
            candidates: rows,
        }),
    }
}

fn slot_value_re() -> Regex {
    // `:(slot) "value"` — the exact syntax KycWorkbook's own test fixtures
    // use (`:edge-id "..."`, `:subject-id "..."`). Constructed fresh per
    // call rather than a `static`/`LazyLock`: this runs once per `stage`
    // command, not in a hot loop, and keeps this module free of any extra
    // synchronization machinery for the zero-inference audit to reason about.
    Regex::new(&format!(r#":({TARGET_SLOTS})\s+"([^"]*)""#)).expect("static pattern is valid")
}

/// Rewrite every `@handle` / quoted-entity-name occurrence found as the
/// value of one of the five target-binding slots into a resolved UUID
/// literal. Values that already parse as a UUID are left untouched
/// (idempotent — resolving already-resolved text is a no-op). Every other
/// slot/payload key is left byte-for-byte untouched: this resolver never
/// touches payload literals (e.g. `:kind "voting_rights"`), only the five
/// target-binding fields.
pub(crate) async fn resolve_handles(
    conn: &mut PgConnection,
    text: &str,
) -> Result<String, ResolverError> {
    let re = slot_value_re();
    let mut result = String::with_capacity(text.len());
    let mut last_end = 0usize;

    for caps in re.captures_iter(text) {
        let whole = caps.get(0).expect("group 0 always present");
        let slot = &caps[1];
        let value = &caps[2];

        result.push_str(&text[last_end..whole.start()]);

        if Uuid::parse_str(value).is_ok() {
            // Already a UUID literal — nothing to resolve, leave verbatim.
            result.push_str(whole.as_str());
        } else {
            let handle = value.strip_prefix('@').unwrap_or(value);
            let resolved = lookup_exact(conn, handle).await?;
            result.push_str(&format!(r#":{slot} "{resolved}""#));
        }

        last_end = whole.end();
    }
    result.push_str(&text[last_end..]);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_mirrors_the_trigger_body() {
        assert_eq!(normalize("Allianz Global Investors, S.A."), "allianz global investors s a");
        assert_eq!(normalize("  Blackrock   Inc.  "), "blackrock inc");
    }

    #[test]
    fn slot_regex_matches_hyphenated_target_slots_only() {
        let re = slot_value_re();
        let text = r#"(kyc_ubo.assert.edge.control :edge_id "@Foo" :entity-id "@Bar" :kind "voting_rights")"#;
        let matches: Vec<&str> = re.captures_iter(text).map(|c| c.get(0).unwrap().as_str()).collect();
        // `:edge_id` (underscore, payload arg) must NOT match; `:entity-id`
        // (hyphenated target-binding slot) must; `:kind` (payload) must not.
        assert_eq!(matches, vec![r#":entity-id "@Bar""#]);
    }
}
