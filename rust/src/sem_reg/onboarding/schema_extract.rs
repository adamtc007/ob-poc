//! Step 2: Introspect PostgreSQL schema via `information_schema`.
//!
//! Queries `information_schema.columns`, `information_schema.table_constraints`,
//! `information_schema.key_column_usage`, and `pg_catalog.pg_constraint` for
//! the `ob-poc`, `kyc`, and `sem_reg` schemas to produce `Vec<TableExtract>`.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

/// Default schemas to introspect.
pub const DEFAULT_SCHEMAS: &[&str] = &["ob-poc", "kyc", "sem_reg"];

/// Extracted table metadata from PostgreSQL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableExtract {
    /// Schema name (e.g. "ob-poc")
    pub schema: String,
    /// Table name
    pub table_name: String,
    /// Columns in this table
    pub columns: Vec<ColumnExtract>,
    /// Primary key column names
    pub primary_keys: Vec<String>,
    /// Foreign key relationships
    pub foreign_keys: Vec<ForeignKeyExtract>,
    /// Unique constraint column sets
    pub unique_constraints: Vec<Vec<String>>,
    /// Table comment (if any)
    pub comment: Option<String>,
}

/// Extracted column metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnExtract {
    /// Column name
    pub name: String,
    /// SQL data type (e.g. "uuid", "text", "timestamp with time zone")
    pub sql_type: String,
    /// Whether the column is nullable
    pub is_nullable: bool,
    /// Default value expression (if any)
    pub default_value: Option<String>,
    /// Ordinal position (1-based)
    pub ordinal_position: i32,
}

/// Extracted foreign key relationship.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeignKeyExtract {
    /// Constraint name
    pub constraint_name: String,
    /// Column in the source table
    pub from_column: String,
    /// Schema of the target table
    pub target_schema: String,
    /// Target table name
    pub target_table: String,
    /// Target column name
    pub target_column: String,
}

/// Extract schema metadata from PostgreSQL.
///
/// Queries information_schema for the specified schemas (defaults to ob-poc, kyc, sem_reg).
#[cfg(feature = "database")]
pub async fn extract_schema(
    pool: &PgPool,
    schemas: &[&str],
) -> Result<Vec<TableExtract>, anyhow::Error> {
    let schema_list: Vec<String> = schemas.iter().map(|s| s.to_string()).collect();

    // Step 1: Get all tables and their columns
    let columns = load_columns(pool, &schema_list).await?;

    // Step 2: Get primary keys
    let pks = load_primary_keys(pool, &schema_list).await?;

    // Step 3: Get foreign keys
    let fks = load_foreign_keys(pool, &schema_list).await?;

    // Step 4: Get unique constraints
    let uniques = load_unique_constraints(pool, &schema_list).await?;

    // Step 5: Get table comments
    let comments = load_table_comments(pool, &schema_list).await?;

    // Assemble into TableExtract records grouped by (schema, table)
    let mut tables: std::collections::BTreeMap<(String, String), TableExtract> =
        std::collections::BTreeMap::new();

    for col in columns {
        let key = (col.schema.clone(), col.table.clone());
        let table = tables.entry(key.clone()).or_insert_with(|| TableExtract {
            schema: col.schema.clone(),
            table_name: col.table.clone(),
            columns: Vec::new(),
            primary_keys: Vec::new(),
            foreign_keys: Vec::new(),
            unique_constraints: Vec::new(),
            comment: None,
        });
        table.columns.push(ColumnExtract {
            name: col.column_name,
            sql_type: col.data_type,
            is_nullable: col.is_nullable,
            default_value: col.column_default,
            ordinal_position: col.ordinal_position,
        });
    }

    // Sort columns by ordinal position
    for table in tables.values_mut() {
        table
            .columns
            .sort_by_key(|c| c.ordinal_position);
    }

    // Attach primary keys
    for pk in pks {
        let key = (pk.schema, pk.table);
        if let Some(table) = tables.get_mut(&key) {
            table.primary_keys.push(pk.column_name);
        }
    }

    // Attach foreign keys
    for fk in fks {
        let key = (fk.source_schema.clone(), fk.source_table.clone());
        if let Some(table) = tables.get_mut(&key) {
            table.foreign_keys.push(ForeignKeyExtract {
                constraint_name: fk.constraint_name,
                from_column: fk.from_column,
                target_schema: fk.target_schema,
                target_table: fk.target_table,
                target_column: fk.target_column,
            });
        }
    }

    // Attach unique constraints
    for uc in uniques {
        let key = (uc.schema, uc.table);
        if let Some(table) = tables.get_mut(&key) {
            table.unique_constraints.push(uc.columns);
        }
    }

    // Attach comments
    for comment in comments {
        let key = (comment.schema, comment.table);
        if let Some(table) = tables.get_mut(&key) {
            table.comment = Some(comment.comment);
        }
    }

    Ok(tables.into_values().collect())
}

// ── Internal query types ─────────────────────────────────────

#[cfg(feature = "database")]
struct ColumnRow {
    schema: String,
    table: String,
    column_name: String,
    data_type: String,
    is_nullable: bool,
    column_default: Option<String>,
    ordinal_position: i32,
}

#[cfg(feature = "database")]
struct PkRow {
    schema: String,
    table: String,
    column_name: String,
}

#[cfg(feature = "database")]
struct FkRow {
    constraint_name: String,
    source_schema: String,
    source_table: String,
    from_column: String,
    target_schema: String,
    target_table: String,
    target_column: String,
}

#[cfg(feature = "database")]
struct UniqueRow {
    schema: String,
    table: String,
    columns: Vec<String>,
}

#[cfg(feature = "database")]
struct CommentRow {
    schema: String,
    table: String,
    comment: String,
}

// ── Query functions ──────────────────────────────────────────

#[cfg(feature = "database")]
async fn load_columns(
    pool: &PgPool,
    schemas: &[String],
) -> Result<Vec<ColumnRow>, anyhow::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, Option<String>, i32)>(
        r#"
        SELECT
            table_schema,
            table_name,
            column_name,
            COALESCE(udt_name, data_type) AS data_type,
            is_nullable,
            column_default,
            ordinal_position
        FROM information_schema.columns
        WHERE table_schema = ANY($1)
          AND table_name NOT LIKE 'pg_%'
          AND table_name NOT LIKE 'v_%'
        ORDER BY table_schema, table_name, ordinal_position
        "#,
    )
    .bind(schemas)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ColumnRow {
            schema: r.0,
            table: r.1,
            column_name: r.2,
            data_type: r.3,
            is_nullable: r.4 == "YES",
            column_default: r.5,
            ordinal_position: r.6,
        })
        .collect())
}

#[cfg(feature = "database")]
async fn load_primary_keys(
    pool: &PgPool,
    schemas: &[String],
) -> Result<Vec<PkRow>, anyhow::Error> {
    let rows = sqlx::query_as::<_, (String, String, String)>(
        r#"
        SELECT
            tc.table_schema,
            tc.table_name,
            kcu.column_name
        FROM information_schema.table_constraints tc
        JOIN information_schema.key_column_usage kcu
            ON tc.constraint_name = kcu.constraint_name
            AND tc.table_schema = kcu.table_schema
        WHERE tc.constraint_type = 'PRIMARY KEY'
          AND tc.table_schema = ANY($1)
        ORDER BY tc.table_schema, tc.table_name, kcu.ordinal_position
        "#,
    )
    .bind(schemas)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| PkRow {
            schema: r.0,
            table: r.1,
            column_name: r.2,
        })
        .collect())
}

#[cfg(feature = "database")]
async fn load_foreign_keys(
    pool: &PgPool,
    schemas: &[String],
) -> Result<Vec<FkRow>, anyhow::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String)>(
        r#"
        SELECT
            tc.constraint_name,
            tc.table_schema AS source_schema,
            tc.table_name AS source_table,
            kcu.column_name AS from_column,
            ccu.table_schema AS target_schema,
            ccu.table_name AS target_table,
            ccu.column_name AS target_column
        FROM information_schema.table_constraints tc
        JOIN information_schema.key_column_usage kcu
            ON tc.constraint_name = kcu.constraint_name
            AND tc.table_schema = kcu.table_schema
        JOIN information_schema.constraint_column_usage ccu
            ON tc.constraint_name = ccu.constraint_name
            AND tc.table_schema = ccu.constraint_schema
        WHERE tc.constraint_type = 'FOREIGN KEY'
          AND tc.table_schema = ANY($1)
        ORDER BY tc.table_schema, tc.table_name, kcu.ordinal_position
        "#,
    )
    .bind(schemas)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| FkRow {
            constraint_name: r.0,
            source_schema: r.1,
            source_table: r.2,
            from_column: r.3,
            target_schema: r.4,
            target_table: r.5,
            target_column: r.6,
        })
        .collect())
}

#[cfg(feature = "database")]
async fn load_unique_constraints(
    pool: &PgPool,
    schemas: &[String],
) -> Result<Vec<UniqueRow>, anyhow::Error> {
    // Get unique constraint columns grouped by constraint name
    let rows = sqlx::query_as::<_, (String, String, String, String)>(
        r#"
        SELECT
            tc.table_schema,
            tc.table_name,
            tc.constraint_name,
            kcu.column_name
        FROM information_schema.table_constraints tc
        JOIN information_schema.key_column_usage kcu
            ON tc.constraint_name = kcu.constraint_name
            AND tc.table_schema = kcu.table_schema
        WHERE tc.constraint_type = 'UNIQUE'
          AND tc.table_schema = ANY($1)
        ORDER BY tc.table_schema, tc.table_name, tc.constraint_name, kcu.ordinal_position
        "#,
    )
    .bind(schemas)
    .fetch_all(pool)
    .await?;

    // Group by (schema, table, constraint_name)
    let mut grouped: std::collections::BTreeMap<(String, String, String), Vec<String>> =
        std::collections::BTreeMap::new();
    for r in rows {
        grouped
            .entry((r.0, r.1, r.2))
            .or_default()
            .push(r.3);
    }

    Ok(grouped
        .into_iter()
        .map(|((schema, table, _), columns)| UniqueRow {
            schema,
            table,
            columns,
        })
        .collect())
}

#[cfg(feature = "database")]
async fn load_table_comments(
    pool: &PgPool,
    schemas: &[String],
) -> Result<Vec<CommentRow>, anyhow::Error> {
    let rows = sqlx::query_as::<_, (String, String, String)>(
        r#"
        SELECT
            n.nspname AS schema_name,
            c.relname AS table_name,
            d.description
        FROM pg_catalog.pg_description d
        JOIN pg_catalog.pg_class c ON d.objoid = c.oid AND d.objsubid = 0
        JOIN pg_catalog.pg_namespace n ON c.relnamespace = n.oid
        WHERE n.nspname = ANY($1)
          AND c.relkind IN ('r', 'p')
          AND d.description IS NOT NULL
        ORDER BY n.nspname, c.relname
        "#,
    )
    .bind(schemas)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| CommentRow {
            schema: r.0,
            table: r.1,
            comment: r.2,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_extract_serialization() {
        let table = TableExtract {
            schema: "ob-poc".into(),
            table_name: "cbus".into(),
            columns: vec![
                ColumnExtract {
                    name: "cbu_id".into(),
                    sql_type: "uuid".into(),
                    is_nullable: false,
                    default_value: Some("gen_random_uuid()".into()),
                    ordinal_position: 1,
                },
                ColumnExtract {
                    name: "name".into(),
                    sql_type: "text".into(),
                    is_nullable: false,
                    default_value: None,
                    ordinal_position: 2,
                },
            ],
            primary_keys: vec!["cbu_id".into()],
            foreign_keys: vec![ForeignKeyExtract {
                constraint_name: "fk_cbus_apex".into(),
                from_column: "apex_entity_id".into(),
                target_schema: "ob-poc".into(),
                target_table: "entities".into(),
                target_column: "entity_id".into(),
            }],
            unique_constraints: vec![],
            comment: Some("Client business units".into()),
        };

        let json = serde_json::to_value(&table).unwrap();
        let back: TableExtract = serde_json::from_value(json).unwrap();
        assert_eq!(back.table_name, "cbus");
        assert_eq!(back.columns.len(), 2);
        assert_eq!(back.primary_keys, vec!["cbu_id"]);
        assert_eq!(back.foreign_keys.len(), 1);
    }
}
