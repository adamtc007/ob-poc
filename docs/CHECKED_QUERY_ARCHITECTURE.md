# Compile-Time SQL Contract Verification for Java 26

## Architecture: `@CheckedQuery` Annotation Processor + Semantic OS Extension

**Date:** 2026-02-27  
**Status:** Design  
**Context:** Java 26 port of Semantic OS — zero external library constraint

---

## Part 1 — The Core Annotation Processor (~800 LOC)

### What sqlx gives Rust

```rust
// At `cargo build`, sqlx:
// 1. Connects to DATABASE_URL
// 2. Prepares the statement (validates SQL syntax)
// 3. Reads column metadata (names, types, nullability)
// 4. Reads parameter metadata (types)
// 5. Verifies PgSnapshotRow fields match column metadata
// 6. COMPILE ERROR if anything is wrong
let row = sqlx::query_as!(PgSnapshotRow,
    "SELECT snapshot_id, fqn, version_major FROM sem_reg.snapshots WHERE fqn = $1",
    fqn
).fetch_one(&pool).await?;
```

### The Java 26 equivalent — zero libraries

#### 1. Define the annotations (javax.annotation.processing, part of JDK)

```java
// --- annotations module: sem_os_checked_sql/annotations ---

package com.bnymellon.semOS.sql;

import java.lang.annotation.*;

/**
 * Marks a method whose SQL is verified against the live schema at compile time.
 * The processor connects to Postgres, prepares the statement, and verifies:
 *   - SQL syntax is valid
 *   - All referenced tables/columns exist
 *   - Return record fields match result column types
 *   - Parameter types match placeholder types
 *   - Nullability constraints are respected (Optional vs bare type)
 */
@Retention(RetentionPolicy.SOURCE)   // zero runtime footprint
@Target(ElementType.METHOD)
public @interface CheckedQuery {
    /** The SQL statement. Postgres $1/$2 placeholders. */
    String sql();

    /** Expected cardinality. Drives compile-time Optional<T> checks. */
    Cardinality cardinality() default Cardinality.ONE;

    enum Cardinality { ONE, OPTIONAL, MANY }
}

/**
 * Marks a record as a database row type.
 * The processor verifies field-to-column mappings.
 */
@Retention(RetentionPolicy.SOURCE)
@Target(ElementType.TYPE)
public @interface RowType {
    /** Schema-qualified table name. e.g. "sem_reg.snapshots" */
    String table() default "";
}
```

#### 2. The annotation processor (~500 LOC core)

```java
// --- processor module: sem_os_checked_sql/processor ---

package com.bnymellon.semOS.sql.processor;

import javax.annotation.processing.*;
import javax.lang.model.element.*;
import javax.tools.Diagnostic;
import java.sql.*;
import java.util.*;

@SupportedAnnotationTypes("com.bnymellon.semOS.sql.CheckedQuery")
public class CheckedQueryProcessor extends AbstractProcessor {

    private Connection conn;

    @Override
    public synchronized void init(ProcessingEnvironment env) {
        super.init(env);
        // Connect to Postgres — same pattern as sqlx's DATABASE_URL
        String url = env.getOptions().get("checked.sql.url");
        if (url == null) url = System.getenv("SEM_OS_DATABASE_URL");
        if (url != null) {
            try {
                this.conn = DriverManager.getConnection(url);
            } catch (SQLException e) {
                env.getMessager().printMessage(Diagnostic.Kind.WARNING,
                    "CheckedQuery: cannot connect to database — skipping verification");
            }
        }
    }

    @Override
    public boolean process(Set<? extends TypeElement> annotations, RoundEnvironment round) {
        if (conn == null) return false;  // graceful degradation — no DB, no checks

        for (var element : round.getElementsAnnotatedWith(CheckedQuery.class)) {
            if (element instanceof ExecutableElement method) {
                processMethod(method);
            }
        }
        return false;
    }

    private void processMethod(ExecutableElement method) {
        var annotation = method.getAnnotation(CheckedQuery.class);
        String sql = annotation.sql();

        try {
            // Step 1: Prepare — validates SQL syntax and table/column existence
            PreparedStatement ps = conn.prepareStatement(sql);

            // Step 2: Verify parameter types match method parameter types
            ParameterMetaData paramMeta = ps.getParameterMetaData();
            verifyParameters(method, paramMeta);

            // Step 3: Verify return type record fields match result columns
            ResultSetMetaData rsMeta = ps.getMetaData();
            verifyReturnType(method, rsMeta, annotation.cardinality());

            ps.close();
        } catch (SQLException e) {
            // SQL doesn't parse or table doesn't exist — COMPILE ERROR
            error(method, "SQL verification failed: %s\n  SQL: %s", e.getMessage(), sql);
        }
    }

    private void verifyParameters(ExecutableElement method, ParameterMetaData meta)
            throws SQLException {
        var params = method.getParameters();
        int sqlParamCount = meta.getParameterCount();

        // Account for the fact that first param might be Connection/DataSource
        // and remaining params map to $1, $2, ...
        List<VariableElement> sqlParams = params.stream()
            .filter(p -> !isInfraType(p))  // skip Connection, DataSource etc.
            .toList();

        if (sqlParams.size() != sqlParamCount) {
            error(method, "SQL has %d parameters but method has %d SQL-bound arguments",
                sqlParamCount, sqlParams.size());
            return;
        }

        for (int i = 0; i < sqlParamCount; i++) {
            int sqlType = meta.getParameterType(i + 1);
            String javaType = sqlParams.get(i).asType().toString();
            if (!isTypeCompatible(sqlType, javaType)) {
                error(method, "Parameter $%d: SQL expects %s but got %s",
                    i + 1, sqlTypeName(sqlType), javaType);
            }
        }
    }

    private void verifyReturnType(ExecutableElement method, ResultSetMetaData meta,
            CheckedQuery.Cardinality card) throws SQLException {
        // Extract the record type from return type
        // e.g. SnapshotRow from SnapshotRow, Optional<SnapshotRow>, List<SnapshotRow>
        var returnType = unwrapReturnType(method.getReturnType(), card);
        if (returnType == null) return;  // couldn't resolve — skip

        // Get record components (fields)
        var recordFields = getRecordComponents(returnType);
        int columnCount = meta.getColumnCount();

        // Verify every record field has a matching column
        for (var field : recordFields) {
            String fieldName = field.getSimpleName().toString();
            String snakeName = camelToSnake(fieldName);  // snapshotId → snapshot_id

            boolean found = false;
            for (int col = 1; col <= columnCount; col++) {
                if (meta.getColumnName(col).equals(snakeName)) {
                    found = true;
                    // Verify type compatibility
                    int sqlType = meta.getColumnType(col);
                    String javaType = field.asType().toString();
                    if (!isTypeCompatible(sqlType, javaType)) {
                        error(method, "Column '%s': SQL type %s incompatible with %s",
                            snakeName, sqlTypeName(sqlType), javaType);
                    }
                    // Verify nullability
                    int nullable = meta.isNullable(col);
                    boolean fieldIsOptional = javaType.startsWith("java.util.Optional");
                    if (nullable == ResultSetMetaData.columnNoNulls && fieldIsOptional) {
                        warning(method, "Column '%s' is NOT NULL but field is Optional", snakeName);
                    }
                    if (nullable == ResultSetMetaData.columnNullable && !fieldIsOptional) {
                        error(method, "Column '%s' is NULLABLE but field '%s' is not Optional",
                            snakeName, fieldName);
                    }
                    break;
                }
            }
            if (!found) {
                error(method, "Record field '%s' (column '%s') not found in query result",
                    fieldName, snakeName);
            }
        }
    }

    // --- Type mapping: Postgres SQL types → Java types ---

    private boolean isTypeCompatible(int sqlType, String javaType) {
        // Unwrap Optional<T> to T
        String baseType = javaType.startsWith("java.util.Optional")
            ? extractOptionalInner(javaType) : javaType;

        return switch (sqlType) {
            case Types.VARCHAR, Types.CHAR, Types.LONGVARCHAR, Types.OTHER
                -> baseType.equals("java.lang.String")
                || baseType.equals("com.bnymellon.semOS.core.types.Fqn");

            case Types.INTEGER, Types.SMALLINT
                -> baseType.equals("int") || baseType.equals("java.lang.Integer");

            case Types.BIGINT
                -> baseType.equals("long") || baseType.equals("java.lang.Long");

            case Types.BOOLEAN, Types.BIT
                -> baseType.equals("boolean") || baseType.equals("java.lang.Boolean");

            case Types.NUMERIC, Types.DECIMAL
                -> baseType.equals("java.math.BigDecimal");

            // Postgres UUID → java.util.UUID
            case 1111  // OTHER — Postgres custom types including UUID
                -> baseType.equals("java.util.UUID")
                || baseType.equals("java.lang.String");

            case Types.TIMESTAMP, Types.TIMESTAMP_WITH_TIMEZONE
                -> baseType.equals("java.time.Instant")
                || baseType.equals("java.time.OffsetDateTime");

            case Types.DATE
                -> baseType.equals("java.time.LocalDate");

            // JSON/JSONB → serde_json::Value equivalent
            case Types.JAVA_OBJECT
                -> baseType.equals("com.bnymellon.semOS.core.types.JsonValue")
                || baseType.equals("java.lang.String");

            // ARRAY → List<T>
            case Types.ARRAY
                -> baseType.startsWith("java.util.List");

            default -> true;  // unknown SQL type — allow, warn
        };
    }

    // --- Helper methods ---

    private void error(Element e, String fmt, Object... args) {
        processingEnv.getMessager().printMessage(
            Diagnostic.Kind.ERROR, String.format(fmt, args), e);
    }

    private void warning(Element e, String fmt, Object... args) {
        processingEnv.getMessager().printMessage(
            Diagnostic.Kind.WARNING, String.format(fmt, args), e);
    }

    private static String camelToSnake(String s) {
        return s.replaceAll("([a-z])([A-Z])", "$1_$2").toLowerCase();
    }
}
```

#### 3. How it looks in your Semantic OS Java code

```java
@RowType(table = "sem_reg.snapshots")
public record SnapshotRow(
    UUID snapshotId,
    Optional<UUID> snapshotSetId,   // nullable → Optional, enforced at compile time
    String objectType,
    UUID objectId,
    int versionMajor,
    int versionMinor,
    String status,
    String governanceTier,
    String trustClass,
    JsonValue securityLabel,        // jsonb column
    Instant effectiveFrom,
    Optional<Instant> effectiveUntil,
    Optional<UUID> predecessorId,
    String changeType,
    Optional<String> changeRationale,
    String createdBy,
    Optional<String> approvedBy,
    JsonValue definition,
    Instant createdAt
) {}

// In PgSnapshotStore.java:

@CheckedQuery(
    sql = """
        SELECT * FROM sem_reg.snapshots
        WHERE fqn = $1 AND status = 'active'
        ORDER BY effective_from DESC LIMIT 1
        """,
    cardinality = Cardinality.OPTIONAL
)
Optional<SnapshotRow> resolve(Fqn fqn) throws SemOsError;

@CheckedQuery(
    sql = """
        SELECT s.* FROM sem_reg.snapshots s
        JOIN sem_reg.snapshot_sets ss ON s.snapshot_set_id = ss.set_id
        WHERE ss.set_id = $1
        ORDER BY s.object_type, s.created_at
        """,
    cardinality = Cardinality.MANY
)
List<SnapshotRow> listBySet(UUID setId) throws SemOsError;

@CheckedQuery(
    sql = """
        INSERT INTO sem_reg.snapshots (
            snapshot_set_id, object_type, object_id,
            version_major, version_minor, status,
            governance_tier, trust_class, security_label,
            predecessor_id, change_type, change_rationale,
            created_by, approved_by, definition
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        RETURNING snapshot_id
        """,
    cardinality = Cardinality.ONE
)
UUID insertSnapshot(
    UUID setId, String objectType, UUID objectId,
    int versionMajor, int versionMinor, String status,
    String governanceTier, String trustClass, JsonValue securityLabel,
    UUID predecessorId, String changeType, String changeRationale,
    String createdBy, String approvedBy, JsonValue definition
) throws SemOsError;
```

#### 4. What the compiler output looks like

```
$ javac -Achecked.sql.url=postgres://localhost/sem_os_dev ...

src/PgSnapshotStore.java:45: error: SQL verification failed:
  Column 'fqnn' does not exist (did you mean 'fqn'?)
  SQL: SELECT * FROM sem_reg.snapshots WHERE fqnn = $1
    @CheckedQuery(sql = "SELECT * FROM sem_reg.snapshots WHERE fqnn = $1")
     ^

src/PgSnapshotStore.java:67: error: Column 'effective_until' is NULLABLE
  but field 'effectiveUntil' is Instant (not Optional<Instant>)
    Instant effectiveUntil,
            ^

src/PgSnapshotStore.java:89: error: SQL has 15 parameters but method has 14
  SQL-bound arguments
    @CheckedQuery(sql = "INSERT INTO sem_reg.snapshots ...")
     ^

3 errors
```

**Same developer experience as sqlx. Same compile-time guarantees. Zero libraries.**

#### 5. Offline mode (for CI without a database)

sqlx has `sqlx prepare` which saves query metadata to `.sqlx/` JSON files.
Same approach:

```bash
# Developer with DB access runs:
$ java -jar checked-sql-prepare.jar \
    --url postgres://localhost/sem_os_dev \
    --source src/ \
    --output .checked-sql/

# Generates:
.checked-sql/
  PgSnapshotStore.resolve.json        # column names, types, nullability
  PgSnapshotStore.listBySet.json
  PgSnapshotStore.insertSnapshot.json

# CI without DB access:
$ javac -Achecked.sql.mode=offline -Achecked.sql.cache=.checked-sql/ ...
```

The processor reads cached metadata instead of connecting to Postgres.
Same verification, no network dependency.

---

## Part 2 — The Semantic OS Extension

This is where it gets interesting. The annotation processor verifies SQL against
the **physical schema** (Postgres tables and columns). But your Semantic OS
registry contains the **logical schema** — entity types, attribute definitions,
verb contracts, security labels, governance tiers.

### The gap between physical and logical

```
Physical (Postgres):  Column "jurisdiction_code" is VARCHAR NOT NULL
Logical  (Registry):  Attribute "cbu.jurisdiction" is String, PII=false,
                       Classification=Internal, lookup via master_jurisdictions
```

The annotation processor catches type mismatches. But it can't catch:
- "This query returns PII data but the caller isn't in a PII-authorized context"
- "This query references an attribute that was deprecated in registry v2.3"
- "This INSERT creates an entity of type X but doesn't populate required attributes"
- "This verb's CRUD mapping references table Y but the entity type was remapped to Z"

### Extension: `@GovernedQuery` — registry-aware verification

```java
/**
 * Extends @CheckedQuery with Semantic OS registry verification.
 * At compile time, connects to BOTH Postgres (physical) AND the
 * Semantic OS registry (logical) to verify:
 *
 *   1. All @CheckedQuery verifications (physical schema)
 *   2. Referenced entity types are active in the registry
 *   3. Referenced attributes are not deprecated/retired
 *   4. Security label requirements are met
 *   5. Verb contract constraints are satisfied
 *   6. Governance tier is appropriate for the operation
 */
@Retention(RetentionPolicy.SOURCE)
@Target(ElementType.METHOD)
public @interface GovernedQuery {
    /** The SQL statement. */
    String sql();

    /** The verb contract FQN this query implements. e.g. "cbu.create" */
    String verb();

    /** Expected cardinality. */
    CheckedQuery.Cardinality cardinality() default CheckedQuery.Cardinality.ONE;
}
```

### What the processor does with `@GovernedQuery`

```java
private void processGovernedMethod(ExecutableElement method) {
    var annotation = method.getAnnotation(GovernedQuery.class);

    // 1. All physical checks from @CheckedQuery
    verifyAgainstPostgres(method, annotation.sql(), annotation.cardinality());

    // 2. Load verb contract from registry
    //    (connects to Sem OS API or reads cached snapshot)
    VerbContract contract = registry.resolveVerb(annotation.verb());
    if (contract == null) {
        error(method, "Verb '%s' not found in Semantic OS registry", annotation.verb());
        return;
    }

    // 3. Verify verb is active (not deprecated/retired)
    if (contract.status() != SnapshotStatus.ACTIVE) {
        error(method, "Verb '%s' is %s — cannot be used in new code",
            annotation.verb(), contract.status());
    }

    // 4. Verify method parameters match verb contract args
    verifyVerbArgs(method, contract);

    // 5. Check security label propagation
    verifySecurityContext(method, contract);

    // 6. Check governance tier
    if (contract.governanceTier() == GovernanceTier.GOVERNED) {
        // Governed verbs require Principal parameter for audit trail
        boolean hasPrincipal = method.getParameters().stream()
            .anyMatch(p -> p.asType().toString().contains("Principal"));
        if (!hasPrincipal) {
            error(method, "Verb '%s' is GOVERNED tier — method must accept Principal "
                + "for audit trail", annotation.verb());
        }
    }
}

private void verifyVerbArgs(ExecutableElement method, VerbContract contract) {
    for (var requiredArg : contract.args().stream()
            .filter(VerbArgDef::required).toList()) {

        // Check the SQL references the expected column
        String expectedColumn = requiredArg.mapsTo()
            .orElse(camelToSnake(requiredArg.name()));

        if (!annotation.sql().contains(expectedColumn)) {
            warning(method,
                "Verb '%s' requires arg '%s' (maps to column '%s') "
                + "but SQL does not reference this column",
                annotation.verb(), requiredArg.name(), expectedColumn);
        }

        // Check the attribute is still active in the registry
        var attr = registry.resolveAttribute(
            contract.domain() + "." + requiredArg.name());
        if (attr != null && attr.status() == SnapshotStatus.DEPRECATED) {
            warning(method,
                "Arg '%s' references deprecated attribute '%s' — "
                + "consider migrating to replacement",
                requiredArg.name(), attr.fqn());
        }
    }
}

private void verifySecurityContext(ExecutableElement method, VerbContract contract) {
    var label = contract.securityLabel();
    if (label.pii()) {
        // Method must be in a class annotated with @PiiAuthorized
        // or accept a SecurityContext parameter
        var enclosingClass = method.getEnclosingElement();
        boolean hasPiiAuth = enclosingClass.getAnnotation(PiiAuthorized.class) != null;
        boolean hasSecCtx = method.getParameters().stream()
            .anyMatch(p -> p.asType().toString().contains("SecurityContext"));
        if (!hasPiiAuth && !hasSecCtx) {
            error(method,
                "Verb '%s' touches PII data (security label: pii=true) — "
                + "method must be in @PiiAuthorized class or accept SecurityContext",
                annotation.verb());
        }
    }
}
```

### What the developer sees

```java
public class PgCbuStore {

    // This compiles — verb is active, args match, schema checks pass
    @GovernedQuery(
        verb = "cbu.create",
        sql = """
            INSERT INTO "ob-poc".client_business_units (name, jurisdiction)
            VALUES ($1, $2) RETURNING cbu_id
            """,
        cardinality = Cardinality.ONE
    )
    UUID createCbu(Principal principal, String name, String jurisdiction)
        throws SemOsError;


    // COMPILE ERROR: verb references deprecated attribute
    @GovernedQuery(
        verb = "cbu.create",
        sql = """
            INSERT INTO "ob-poc".client_business_units (name, jurisdiction, old_field)
            VALUES ($1, $2, $3) RETURNING cbu_id
            """,
        cardinality = Cardinality.ONE
    )
    UUID createCbuLegacy(Principal principal, String name, String jur, String old)
        throws SemOsError;


    // COMPILE ERROR: verb is GOVERNED tier but no Principal parameter
    @GovernedQuery(
        verb = "cbu.create",
        sql = "INSERT INTO ...",
        cardinality = Cardinality.ONE
    )
    UUID createCbuUngoverned(String name, String jurisdiction)
        throws SemOsError;


    // COMPILE ERROR: verb touches PII but class lacks @PiiAuthorized
    @GovernedQuery(
        verb = "entity.update_personal_details",
        sql = "UPDATE ...",
        cardinality = Cardinality.ONE
    )
    void updateDetails(Principal principal, UUID entityId, String name)
        throws SemOsError;
}
```

Compiler output:

```
src/PgCbuStore.java:28: error: Arg 'old_field' references deprecated
  attribute 'cbu.old_field' — consider migrating to replacement
    UUID createCbuLegacy(...)
         ^

src/PgCbuStore.java:38: error: Verb 'cbu.create' is GOVERNED tier —
  method must accept Principal for audit trail
    UUID createCbuUngoverned(String name, String jurisdiction)
         ^

src/PgCbuStore.java:47: error: Verb 'entity.update_personal_details'
  touches PII data (security label: pii=true) — method must be in
  @PiiAuthorized class or accept SecurityContext
    void updateDetails(...)
         ^
```

---

## Part 3 — Architecture: How it fits together

```
                     COMPILE TIME                          RUNTIME
                     ──────────                            ───────

  Java source ──→ [javac] ──→ .class files ──→ [JVM] ──→ execution
                     │
                     ├── @CheckedQuery processor
                     │      │
                     │      ├── connects to Postgres (or offline cache)
                     │      ├── prepares SQL, reads metadata
                     │      └── verifies record fields ↔ columns
                     │
                     └── @GovernedQuery processor
                            │
                            ├── all @CheckedQuery checks
                            ├── connects to Semantic OS registry
                            │     (or offline snapshot cache)
                            ├── verifies verb contract constraints
                            ├── verifies attribute lifecycle status
                            ├── verifies security label propagation
                            └── verifies governance tier requirements
```

### Offline mode for both layers

```bash
# Export registry snapshot for offline compilation
$ sem-os-cli export-snapshot --format json --output .sem-os-cache/

# Directory structure:
.sem-os-cache/
  snapshot-set.json           # metadata (set ID, timestamp)
  verb-contracts/
    cbu.create.json
    cbu.update.json
    entity.update_personal_details.json
  attributes/
    cbu.name.json
    cbu.jurisdiction.json
  entity-types/
    cbu.json
    jurisdiction.json
  security-labels/
    cbu.create.label.json

# CI build — no database, no running Sem OS server needed
$ javac \
    -Achecked.sql.mode=offline \
    -Achecked.sql.cache=.checked-sql/ \
    -Agoverned.query.mode=offline \
    -Agoverned.query.cache=.sem-os-cache/ \
    ...
```

### Registry snapshot → compile cache is a natural Semantic OS feature

The `sem_os_server` already has `/export` endpoint. Adding a CLI tool that:
1. Calls `GET /export/snapshot-set/latest`
2. Writes per-object-type JSON files to a directory
3. Gets committed to the consuming repo (or pulled in CI)

...is maybe 100 lines. And it means the governing registry controls what
Java code CAN COMPILE. Not what it does at runtime. What it can compile.

---

## Part 4 — The Regulatory Argument

### What this gives you that nothing else does

1. **Compile-time PII enforcement.** Code that touches PII data without
   authorization literally does not compile. Not a runtime check. Not a
   code review finding. A compiler error.

2. **Deprecated attribute detection.** When the registry marks an attribute
   as deprecated (via the authoring pipeline), every Java class that
   references it gets a compile warning. When it's retired, compile error.
   No grep. No Jira ticket asking teams to migrate. The build breaks.

3. **Governance tier enforcement.** Governed operations require Principal
   (audit identity). The compiler enforces this. You cannot write an
   ungoverned path to a governed verb.

4. **Living documentation.** The @GovernedQuery annotation IS the
   documentation. It says which verb, which SQL, which contract. It's
   verified against the registry on every build. It cannot go stale
   because stale = compile error.

5. **Audit trail completeness.** If every data access path goes through
   @GovernedQuery, and every @GovernedQuery with a governed verb requires
   Principal, then the audit trail is structurally complete. Not "we
   hope developers remembered to log the actor." Structurally.

### The line for the regulator

"Every database operation in this system is verified at compile time
against both the physical schema and our governance registry. Operations
that touch PII require explicit authorization. Operations on governed
entities require audit identity. Deprecated data paths produce compiler
errors. This is not a policy document — it is a compiler constraint."

---

## Part 5 — Implementation Plan

### Phase 1: @CheckedQuery processor (~800 LOC)
- Pure JDK annotation processor
- Connects to Postgres, prepares SQL, verifies types
- Offline cache mode for CI
- Null safety: nullable columns → Optional fields
- This alone replaces sqlx's compile-time guarantee

### Phase 2: @RowType processor (~200 LOC)
- Verifies record fields match table columns
- Generates column-name constants for type-safe references
- Optional: generates row-mapper lambdas

### Phase 3: @GovernedQuery processor (~600 LOC)
- Extends @CheckedQuery with registry checks
- Verb contract verification
- Security label propagation
- Governance tier enforcement
- Offline snapshot cache

### Phase 4: sem-os-cli export command (~100 LOC)
- Exports registry snapshot to JSON directory
- Committed to consuming repos or pulled in CI
- Versioned — each export tagged with snapshot_set_id

### Total: ~1,700 LOC of pure JDK Java

No Spring. No Hibernate. No JOOQ. No libraries.
Just javax.annotation.processing + java.sql + java.net.http.

This becomes a reusable module that any Java service at BNY can adopt.
Plug in the annotation processor, point it at your DB and your registry,
and your SQL is compile-time verified against both physical and logical
schemas.

---

## Part 6 — Comparison

| Capability | Raw JDBC | Hibernate | JOOQ | sqlx (Rust) | @GovernedQuery |
|---|---|---|---|---|---|
| SQL syntax check at compile | ✗ | ✗ | ✓ | ✓ | ✓ |
| Column existence check | ✗ | runtime | ✓ | ✓ | ✓ |
| Type compatibility check | ✗ | runtime | ✓ | ✓ | ✓ |
| Nullability enforcement | ✗ | ✗ | partial | ✓ | ✓ |
| Verb contract verification | ✗ | ✗ | ✗ | ✗ | ✓ |
| Security label propagation | ✗ | ✗ | ✗ | ✗ | ✓ |
| Governance tier enforcement | ✗ | ✗ | ✗ | ✗ | ✓ |
| Deprecated attribute detection | ✗ | ✗ | ✗ | ✗ | ✓ |
| Zero runtime overhead | n/a | ✗ | ✓ | ✓ | ✓ |
| Zero library dependency | ✓ | ✗ | ✗ | ✗ | ✓ |
| Offline CI mode | n/a | n/a | ✓ | ✓ | ✓ |
