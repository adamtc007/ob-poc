/// Prefix used for all eval fixture schemas in shared Postgres instances.
pub const EVAL_FIXTURE_SCHEMA_PREFIX: &str = "eval_fixture_";

/// Errors emitted by fixture naming helpers.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FixtureNameError {
    #[error("fixture name cannot be empty")]
    Empty,
    #[error("fixture name contains unsupported character '{0}'")]
    UnsupportedCharacter(char),
    #[error("fixture schema name exceeds Postgres identifier limit")]
    TooLong,
}

/// Convert a human fixture name into the Postgres schema name used by eval fixtures.
///
/// # Examples
///
/// ```rust
/// use ob_poc_eval_fixtures::eval_fixture_schema_name;
///
/// let schema = eval_fixture_schema_name("dev-baseline").expect("name is valid");
/// assert_eq!(schema, "eval_fixture_dev_baseline");
/// ```
pub fn eval_fixture_schema_name(name: &str) -> Result<String, FixtureNameError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(FixtureNameError::Empty);
    }

    let raw_name = trimmed
        .strip_prefix(EVAL_FIXTURE_SCHEMA_PREFIX)
        .unwrap_or(trimmed);
    let mut normalized = String::with_capacity(raw_name.len());
    for ch in raw_name.chars() {
        match ch {
            'a'..='z' | '0'..='9' | '_' => normalized.push(ch),
            'A'..='Z' => normalized.push(ch.to_ascii_lowercase()),
            '-' => normalized.push('_'),
            unsupported => return Err(FixtureNameError::UnsupportedCharacter(unsupported)),
        }
    }

    if normalized.is_empty() {
        return Err(FixtureNameError::Empty);
    }

    let schema_name = format!("{EVAL_FIXTURE_SCHEMA_PREFIX}{normalized}");
    if schema_name.len() > 63 {
        return Err(FixtureNameError::TooLong);
    }
    Ok(schema_name)
}

/// Return true when a schema name is owned by the eval fixture lifecycle.
///
/// # Examples
///
/// ```rust
/// use ob_poc_eval_fixtures::is_eval_fixture_schema;
///
/// assert!(is_eval_fixture_schema("eval_fixture_dev"));
/// assert!(!is_eval_fixture_schema("public"));
/// ```
pub fn is_eval_fixture_schema(schema_name: &str) -> bool {
    let Some(suffix) = schema_name.strip_prefix(EVAL_FIXTURE_SCHEMA_PREFIX) else {
        return false;
    };
    !suffix.is_empty()
        && suffix
            .chars()
            .all(|ch| matches!(ch, 'a'..='z' | '0'..='9' | '_'))
}

/// SQL used to list all eval fixture schemas in Postgres.
///
/// # Examples
///
/// ```rust
/// use ob_poc_eval_fixtures::list_eval_fixture_schemas_sql;
///
/// assert!(list_eval_fixture_schemas_sql().contains("pg_namespace"));
/// ```
pub fn list_eval_fixture_schemas_sql() -> &'static str {
    "SELECT nspname FROM pg_namespace WHERE nspname LIKE 'eval_fixture_%' ORDER BY nspname"
}

/// Quote a Postgres identifier.
///
/// # Examples
///
/// ```rust
/// use ob_poc_eval_fixtures::quote_postgres_identifier;
///
/// assert_eq!(quote_postgres_identifier("eval_fixture_dev"), "\"eval_fixture_dev\"");
/// ```
pub fn quote_postgres_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

/// Build the SQL statement used to drop one eval fixture schema.
///
/// # Examples
///
/// ```rust
/// use ob_poc_eval_fixtures::drop_eval_fixture_schema_sql;
///
/// let sql = drop_eval_fixture_schema_sql("eval_fixture_dev").expect("schema is valid");
/// assert_eq!(sql, "DROP SCHEMA IF EXISTS \"eval_fixture_dev\" CASCADE");
/// ```
pub fn drop_eval_fixture_schema_sql(schema_name: &str) -> Result<String, FixtureNameError> {
    if !is_eval_fixture_schema(schema_name) {
        return Err(FixtureNameError::UnsupportedCharacter(' '));
    }
    Ok(format!(
        "DROP SCHEMA IF EXISTS {} CASCADE",
        quote_postgres_identifier(schema_name)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_name_normalizes_safe_names() {
        let schema = eval_fixture_schema_name("Dev-Baseline").expect("name should normalize");
        assert_eq!(schema, "eval_fixture_dev_baseline");
    }

    #[test]
    fn schema_name_rejects_unsupported_chars() {
        let err = eval_fixture_schema_name("bad/name").expect_err("slash should fail");
        assert_eq!(err, FixtureNameError::UnsupportedCharacter('/'));
    }

    #[test]
    fn eval_schema_detection_requires_prefix() {
        assert!(is_eval_fixture_schema("eval_fixture_dev"));
        assert!(!is_eval_fixture_schema("eval_dev"));
    }

    #[test]
    fn drop_schema_sql_rejects_non_eval_schema() {
        assert!(drop_eval_fixture_schema_sql("public").is_err());
    }
}
