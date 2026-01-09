# DSL Verb Execution Flow (ASCII)

> How DSL verbs flow from source to database
> Reference doc for Claude Code when working on DSL pipeline

---

## 1. High-Level Pipeline

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         DSL EXECUTION PIPELINE                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   DSL Source                                                                 │
│   ──────────                                                                 │
│   (cbu.ensure :name "Acme" :jurisdiction "LU")                              │
│        │                                                                     │
│        │                                                                     │
│        ▼                                                                     │
│   ┌──────────────────────────────────────────────────────────────────────┐  │
│   │                         PARSER (Nom)                                  │  │
│   │  rust/crates/ob-dsl-parser/src/lib.rs                                │  │
│   │──────────────────────────────────────────────────────────────────────│  │
│   │  DSL Source → Token Stream → AST (DslNode)                           │  │
│   │                                                                       │  │
│   │  Handles:                                                             │  │
│   │   - S-expression parsing: (domain.verb :arg value)                   │  │
│   │   - Capture syntax: :as @variable                                    │  │
│   │   - Interpolation: @{captured_var}                                   │  │
│   │   - Nested expressions                                               │  │
│   │   - Comments: ;; line comments                                       │  │
│   └──────────────────────────────────────────────────────────────────────┘  │
│        │                                                                     │
│        │ AST (Vec<DslNode>)                                                 │
│        ▼                                                                     │
│   ┌──────────────────────────────────────────────────────────────────────┐  │
│   │                      CSG LINTER (Optional)                            │  │
│   │  rust/crates/ob-dsl-parser/src/linter.rs                             │  │
│   │──────────────────────────────────────────────────────────────────────│  │
│   │  Validates:                                                           │  │
│   │   - Unknown verbs                                                     │  │
│   │   - Missing required args                                             │  │
│   │   - Invalid arg types                                                 │  │
│   │   - Undefined capture references                                      │  │
│   └──────────────────────────────────────────────────────────────────────┘  │
│        │                                                                     │
│        │ Validated AST                                                      │
│        ▼                                                                     │
│   ┌──────────────────────────────────────────────────────────────────────┐  │
│   │                         COMPILER                                      │  │
│   │  rust/src/dsl_v2/compiler.rs                                         │  │
│   │──────────────────────────────────────────────────────────────────────│  │
│   │  AST → Execution Plan (Vec<CompiledOp>)                              │  │
│   │                                                                       │  │
│   │  Resolves:                                                            │  │
│   │   - Verb YAML definitions                                             │  │
│   │   - Argument mappings (DSL arg → SQL column)                         │  │
│   │   - Lookups (name → UUID via entity gateway)                         │  │
│   │   - Default values                                                    │  │
│   │   - Behavior routing (crud vs plugin vs template)                    │  │
│   └──────────────────────────────────────────────────────────────────────┘  │
│        │                                                                     │
│        │ Execution Plan                                                     │
│        ▼                                                                     │
│   ┌──────────────────────────────────────────────────────────────────────┐  │
│   │                         EXECUTOR                                      │  │
│   │  rust/src/dsl_v2/executor.rs                                         │  │
│   │──────────────────────────────────────────────────────────────────────│  │
│   │  Runs CompiledOps against database                                   │  │
│   │                                                                       │  │
│   │  Behaviors:                                                           │  │
│   │   - crud: INSERT/UPDATE/DELETE/SELECT via CrudHandler               │  │
│   │   - plugin: Custom Rust handler (custom_ops/*.rs)                    │  │
│   │   - template: Expand + recurse DSL                                   │  │
│   │                                                                       │  │
│   │  Captures:                                                            │  │
│   │   - Stores returned IDs in capture_map                               │  │
│   │   - Available for @{interpolation} in subsequent ops                 │  │
│   └──────────────────────────────────────────────────────────────────────┘  │
│        │                                                                     │
│        │ Results                                                            │
│        ▼                                                                     │
│   ┌──────────────────────────────────────────────────────────────────────┐  │
│   │                       PostgreSQL                                      │  │
│   │  Schemas: "ob-poc", kyc, custody, instruments, teams                 │  │
│   └──────────────────────────────────────────────────────────────────────┘  │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Verb Resolution

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          VERB RESOLUTION                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   DSL Call: (cbu.ensure :name "Acme" :jurisdiction "LU")                    │
│                  │                                                           │
│                  │ domain="cbu", verb="ensure"                              │
│                  ▼                                                           │
│   ┌──────────────────────────────────────────────────────────────────────┐  │
│   │                    VERB REGISTRY                                      │  │
│   │  rust/src/dsl_v2/verb_registry.rs                                    │  │
│   │──────────────────────────────────────────────────────────────────────│  │
│   │  Loads all YAML files from:                                          │  │
│   │   - rust/config/verbs/*.yaml         (94 files)                      │  │
│   │   - rust/config/verbs/registry/*.yaml                                │  │
│   │   - rust/config/verbs/custody/*.yaml                                 │  │
│   │                                                                       │  │
│   │  Index: HashMap<(domain, verb), VerbDefinition>                      │  │
│   └──────────────────────────────────────────────────────────────────────┘  │
│                  │                                                           │
│                  │ VerbDefinition                                           │
│                  ▼                                                           │
│   ┌──────────────────────────────────────────────────────────────────────┐  │
│   │                    VERB YAML STRUCTURE                                │  │
│   │──────────────────────────────────────────────────────────────────────│  │
│   │  domains:                                                             │  │
│   │    cbu:                                                               │  │
│   │      description: "Client Business Unit management"                  │  │
│   │      verbs:                                                           │  │
│   │        ensure:                                                        │  │
│   │          description: "Create or update CBU"                         │  │
│   │          behavior: crud           ◀── crud | plugin | template       │  │
│   │          crud:                                                        │  │
│   │            operation: upsert      ◀── insert|upsert|update|delete    │  │
│   │            table: cbus                                                │  │
│   │            schema: ob-poc                                             │  │
│   │            returning: cbu_id                                          │  │
│   │            conflict_keys: [name]                                      │  │
│   │          args:                                                        │  │
│   │            - name: name                                               │  │
│   │              type: string                                             │  │
│   │              required: true                                           │  │
│   │              maps_to: name        ◀── DSL arg → SQL column           │  │
│   │            - name: jurisdiction                                       │  │
│   │              type: string                                             │  │
│   │              required: true                                           │  │
│   │              maps_to: jurisdiction                                    │  │
│   │          returns:                                                     │  │
│   │            type: uuid                                                 │  │
│   │            name: cbu_id                                               │  │
│   │            capture: true          ◀── Can use :as @var               │  │
│   └──────────────────────────────────────────────────────────────────────┘  │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Behavior Routing

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          BEHAVIOR ROUTING                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   VerbDefinition.behavior                                                    │
│            │                                                                 │
│            ├──────────────┬──────────────┬──────────────┐                   │
│            │              │              │              │                   │
│            ▼              ▼              ▼              ▼                   │
│   ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐           │
│   │    crud    │  │   plugin   │  │  template  │  │   query    │           │
│   └─────┬──────┘  └─────┬──────┘  └─────┬──────┘  └─────┬──────┘           │
│         │               │               │               │                   │
│         ▼               ▼               ▼               ▼                   │
│   ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐           │
│   │CrudHandler │  │PluginDisp  │  │TemplateExp │  │ QueryExec  │           │
│   │            │  │            │  │            │  │            │           │
│   │ Generates  │  │ Routes to  │  │ Loads YAML │  │ Read-only  │           │
│   │ SQL from   │  │ Rust impl  │  │ template,  │  │ SELECT     │           │
│   │ verb def   │  │ in custom_ │  │ expands,   │  │ queries    │           │
│   │            │  │ ops/*.rs   │  │ recurses   │  │            │           │
│   └────────────┘  └────────────┘  └────────────┘  └────────────┘           │
│                                                                              │
│   CRUD Operations:                                                           │
│   ─────────────────                                                          │
│   insert       → INSERT INTO ... VALUES ...                                 │
│   upsert       → INSERT ... ON CONFLICT ... DO UPDATE                       │
│   update       → UPDATE ... SET ... WHERE pk = $1                           │
│   delete       → DELETE FROM ... WHERE pk = $1                              │
│   select       → SELECT * FROM ... WHERE pk = $1                            │
│   list_by_fk   → SELECT * FROM ... WHERE fk = $1                            │
│                                                                              │
│   Plugin Examples:                                                           │
│   ─────────────────                                                          │
│   gleif.fetch-entity       → GleifFetchEntityOp (API call)                  │
│   kyc.open-case            → KycOpenCaseOp (complex logic)                  │
│   screening.run-batch      → ScreeningRunBatchOp (external service)         │
│   isda.create              → IsdaCreateOp (multi-table transaction)         │
│                                                                              │
│   Template Examples:                                                         │
│   ──────────────────                                                         │
│   cbu.create-fund-structure    → Expands to 10+ DSL statements             │
│   kyc.onboard-standard         → Expands case + workstreams + screenings   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Argument Resolution

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        ARGUMENT RESOLUTION                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   DSL: (cbu.add-product :cbu-id "Acme" :product-type "custody")             │
│                              │                                               │
│                              │ "Acme" is a name, not a UUID                 │
│                              ▼                                               │
│   ┌──────────────────────────────────────────────────────────────────────┐  │
│   │                      LOOKUP RESOLUTION                                │  │
│   │──────────────────────────────────────────────────────────────────────│  │
│   │  Arg definition in YAML:                                              │  │
│   │                                                                       │  │
│   │    - name: cbu-id                                                     │  │
│   │      type: uuid                                                       │  │
│   │      required: true                                                   │  │
│   │      lookup:                      ◀── Triggers name→UUID resolution  │  │
│   │        table: cbus                                                    │  │
│   │        schema: ob-poc                                                 │  │
│   │        search_key: name                                               │  │
│   │        primary_key: cbu_id                                            │  │
│   └──────────────────────────────────────────────────────────────────────┘  │
│                              │                                               │
│                              │ EntityGateway lookup                         │
│                              ▼                                               │
│   ┌──────────────────────────────────────────────────────────────────────┐  │
│   │                      ENTITY GATEWAY                                   │  │
│   │  rust/src/dsl_v2/entity_gateway.rs                                   │  │
│   │──────────────────────────────────────────────────────────────────────│  │
│   │  SELECT cbu_id FROM "ob-poc".cbus WHERE name = 'Acme'                │  │
│   │                                                                       │  │
│   │  Returns: 550e8400-e29b-41d4-a716-446655440000                       │  │
│   └──────────────────────────────────────────────────────────────────────┘  │
│                              │                                               │
│                              │ Resolved UUID                                │
│                              ▼                                               │
│   ┌──────────────────────────────────────────────────────────────────────┐  │
│   │  Final SQL:                                                           │  │
│   │  INSERT INTO products (cbu_id, product_type)                         │  │
│   │  VALUES ('550e8400-...', 'custody')                                  │  │
│   └──────────────────────────────────────────────────────────────────────┘  │
│                                                                              │
│   Resolution Types:                                                          │
│   ─────────────────                                                          │
│   1. Direct value    → Use as-is (string, number, boolean)                  │
│   2. Lookup          → Name → UUID via EntityGateway                        │
│   3. Capture ref     → @{variable} → From capture_map                       │
│   4. Default         → From YAML default: clause                            │
│   5. Set value       → From YAML set_values: clause (hardcoded)             │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 5. Capture & Interpolation

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      CAPTURE & INTERPOLATION                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   DSL Script:                                                                │
│   ───────────                                                                │
│   (cbu.ensure :name "Acme" :jurisdiction "LU" :as @acme)                    │
│   (entity.create-proper-person :first-name "John" :as @john)                │
│   (cbu.assign-role :cbu-id @acme :entity-id @john :role "director")         │
│                                                                              │
│   Execution:                                                                 │
│   ──────────                                                                 │
│                                                                              │
│   Step 1: (cbu.ensure ... :as @acme)                                        │
│        │                                                                     │
│        │  Execute INSERT/UPSERT                                             │
│        │  Returns: cbu_id = "550e8400-..."                                  │
│        ▼                                                                     │
│   ┌─────────────────────────────────┐                                       │
│   │         CAPTURE MAP             │                                       │
│   │─────────────────────────────────│                                       │
│   │  @acme → "550e8400-..."         │                                       │
│   └─────────────────────────────────┘                                       │
│                                                                              │
│   Step 2: (entity.create-proper-person ... :as @john)                       │
│        │                                                                     │
│        │  Execute INSERT                                                    │
│        │  Returns: entity_id = "661e8400-..."                               │
│        ▼                                                                     │
│   ┌─────────────────────────────────┐                                       │
│   │         CAPTURE MAP             │                                       │
│   │─────────────────────────────────│                                       │
│   │  @acme → "550e8400-..."         │                                       │
│   │  @john → "661e8400-..."         │                                       │
│   └─────────────────────────────────┘                                       │
│                                                                              │
│   Step 3: (cbu.assign-role :cbu-id @acme :entity-id @john ...)              │
│        │                                                                     │
│        │  Resolve @acme → "550e8400-..."                                    │
│        │  Resolve @john → "661e8400-..."                                    │
│        │  Execute INSERT                                                    │
│        ▼                                                                     │
│   INSERT INTO roles (cbu_id, entity_id, role_type)                          │
│   VALUES ('550e8400-...', '661e8400-...', 'director')                       │
│                                                                              │
│   Interpolation Syntax:                                                      │
│   ─────────────────────                                                      │
│   @variable        → Direct capture reference (in DSL args)                 │
│   @{variable}      → String interpolation (in templates)                    │
│   @{var.field}     → Nested field access (if returned record)               │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 6. Template Expansion

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        TEMPLATE EXPANSION                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   DSL Call:                                                                  │
│   ─────────                                                                  │
│   (cbu.create-fund-structure :name "Acme Fund" :jurisdiction "LU")          │
│                                                                              │
│   Verb Definition (behavior: template):                                      │
│   ─────────────────────────────────────                                      │
│   create-fund-structure:                                                     │
│     behavior: template                                                       │
│     template:                                                                │
│       file: templates/fund-structure.dsl.yaml                               │
│     args:                                                                    │
│       - name: name                                                           │
│         type: string                                                         │
│         required: true                                                       │
│       - name: jurisdiction                                                   │
│         type: string                                                         │
│         required: true                                                       │
│                                                                              │
│   Template File (fund-structure.dsl.yaml):                                   │
│   ─────────────────────────────────────────                                  │
│   statements:                                                                │
│     - "(cbu.ensure :name \"@{name}\" :jurisdiction \"@{jurisdiction}\"      │
│        :as @fund)"                                                           │
│     - "(entity.create-limited-company :name \"@{name} ManCo\"               │
│        :jurisdiction \"@{jurisdiction}\" :as @manco)"                       │
│     - "(cbu.set-entity :cbu-id @fund :entity-id @manco)"                    │
│     - "(share-class.create :cbu-id @fund :name \"Class A\" :as @class_a)"   │
│     - "(share-class.create :cbu-id @fund :name \"Class B\" :as @class_b)"   │
│                                                                              │
│   Expansion Flow:                                                            │
│   ───────────────                                                            │
│                                                                              │
│   ┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐       │
│   │  Template Call  │────▶│  Load Template  │────▶│  Interpolate    │       │
│   │  with args      │     │  YAML file      │     │  @{variables}   │       │
│   └─────────────────┘     └─────────────────┘     └────────┬────────┘       │
│                                                            │                 │
│                                                            ▼                 │
│                                                   ┌─────────────────┐       │
│                                                   │  Parse expanded │       │
│                                                   │  DSL statements │       │
│                                                   └────────┬────────┘       │
│                                                            │                 │
│                                                            ▼                 │
│                                                   ┌─────────────────┐       │
│                                                   │  Execute each   │       │
│                                                   │  recursively    │       │
│                                                   └─────────────────┘       │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 7. Plugin Handler Pattern

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       PLUGIN HANDLER PATTERN                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   Directory: rust/src/dsl_v2/custom_ops/                                    │
│   ──────────────────────────────────────                                     │
│   ├── mod.rs              ← Plugin registry (register_custom_ops)           │
│   ├── helpers.rs          ← Common utilities (get_uuid_arg, etc.)           │
│   ├── gleif_ops.rs        ← GLEIF API integration                           │
│   ├── kyc_ops.rs          ← KYC case operations                             │
│   ├── screening_ops.rs    ← Screening service calls                         │
│   ├── isda_ops.rs         ← ISDA agreement operations                       │
│   ├── cbu_ops.rs          ← CBU complex operations                          │
│   ├── entity_ops.rs       ← Entity management                               │
│   └── ...                                                                    │
│                                                                              │
│   Handler Trait:                                                             │
│   ──────────────                                                             │
│   #[async_trait]                                                             │
│   pub trait PluginOp: Send + Sync {                                         │
│       async fn execute(                                                      │
│           &self,                                                             │
│           args: &HashMap<String, Value>,                                    │
│           ctx: &ExecutionContext,                                           │
│       ) -> Result<ExecutionResult, ExecutionError>;                         │
│   }                                                                          │
│                                                                              │
│   Registration (mod.rs):                                                     │
│   ──────────────────────                                                     │
│   pub fn register_custom_ops(registry: &mut PluginRegistry) {               │
│       registry.register("gleif", "fetch-entity", GleifFetchEntityOp);       │
│       registry.register("gleif", "trace-ownership", GleifTraceOwnershipOp); │
│       registry.register("kyc", "open-case", KycOpenCaseOp);                 │
│       // ... 42 handlers total                                               │
│   }                                                                          │
│                                                                              │
│   Example Handler (GleifFetchEntityOp):                                      │
│   ─────────────────────────────────────                                      │
│                                                                              │
│   pub struct GleifFetchEntityOp;                                            │
│                                                                              │
│   #[async_trait]                                                             │
│   impl PluginOp for GleifFetchEntityOp {                                    │
│       async fn execute(                                                      │
│           &self,                                                             │
│           args: &HashMap<String, Value>,                                    │
│           ctx: &ExecutionContext,                                           │
│       ) -> Result<ExecutionResult, ExecutionError> {                        │
│           // 1. Extract args                                                 │
│           let lei = get_string_arg(args, "lei")?;                           │
│                                                                              │
│           // 2. Call GLEIF API                                               │
│           let gleif_data = ctx.gleif_client.fetch_entity(&lei).await?;      │
│                                                                              │
│           // 3. Upsert to database                                           │
│           sqlx::query!(                                                      │
│               "INSERT INTO gleif_entities (lei, legal_name, ...) ..."       │
│           ).execute(&ctx.pool).await?;                                      │
│                                                                              │
│           // 4. Return result                                                │
│           Ok(ExecutionResult::Record(json!({                                │
│               "lei": lei,                                                    │
│               "legal_name": gleif_data.legal_name                           │
│           })))                                                               │
│       }                                                                      │
│   }                                                                          │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 8. Error Handling Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        ERROR HANDLING FLOW                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   DSL Execution                                                              │
│        │                                                                     │
│        ▼                                                                     │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                      ERROR TYPES                                     │   │
│   │─────────────────────────────────────────────────────────────────────│   │
│   │                                                                      │   │
│   │  ParseError           ← Syntax error in DSL                         │   │
│   │    • Line/column info                                                │   │
│   │    • Expected vs found                                               │   │
│   │                                                                      │   │
│   │  LintError            ← CSG validation failure                       │   │
│   │    • Unknown verb                                                    │   │
│   │    • Missing required arg                                            │   │
│   │    • Type mismatch                                                   │   │
│   │                                                                      │   │
│   │  CompileError         ← Verb resolution failure                      │   │
│   │    • Verb not found in registry                                      │   │
│   │    • Invalid arg mapping                                             │   │
│   │                                                                      │   │
│   │  LookupError          ← Entity resolution failure                    │   │
│   │    • Entity not found by name                                        │   │
│   │    • Ambiguous match (multiple results)                              │   │
│   │                                                                      │   │
│   │  ExecutionError       ← Runtime failure                              │   │
│   │    • Database constraint violation                                   │   │
│   │    • Plugin handler error                                            │   │
│   │    • External API failure (GLEIF, screening)                         │   │
│   │                                                                      │   │
│   │  CaptureError         ← Interpolation failure                        │   │
│   │    • Undefined capture variable                                      │   │
│   │    • Type mismatch in capture                                        │   │
│   │                                                                      │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│        │                                                                     │
│        │ Error bubbles up                                                   │
│        ▼                                                                     │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                     API RESPONSE                                     │   │
│   │─────────────────────────────────────────────────────────────────────│   │
│   │  {                                                                   │   │
│   │    "success": false,                                                 │   │
│   │    "error": {                                                        │   │
│   │      "type": "LookupError",                                          │   │
│   │      "message": "Entity 'Acme' not found",                          │   │
│   │      "context": {                                                    │   │
│   │        "verb": "cbu.add-product",                                    │   │
│   │        "arg": "cbu-id",                                              │   │
│   │        "searched_table": "cbus",                                     │   │
│   │        "searched_column": "name"                                     │   │
│   │      }                                                               │   │
│   │    }                                                                 │   │
│   │  }                                                                   │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 9. Full Example Trace

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       FULL EXAMPLE TRACE                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   Input DSL:                                                                 │
│   ──────────                                                                 │
│   (kyc.open-case :cbu-id "Acme Fund" :case-type "ONBOARDING" :as @case)     │
│                                                                              │
│   Step 1: PARSE                                                              │
│   ─────────────                                                              │
│   DslNode::Call {                                                            │
│       domain: "kyc",                                                         │
│       verb: "open-case",                                                     │
│       args: [                                                                │
│           ("cbu-id", Value::String("Acme Fund")),                           │
│           ("case-type", Value::String("ONBOARDING")),                       │
│       ],                                                                     │
│       capture: Some("@case"),                                               │
│   }                                                                          │
│                                                                              │
│   Step 2: COMPILE                                                            │
│   ──────────────                                                             │
│   Lookup verb: kyc.open-case → Found (behavior: plugin)                     │
│   Resolve args:                                                              │
│     cbu-id: "Acme Fund" → LOOKUP → "550e8400-..." (UUID)                    │
│     case-type: "ONBOARDING" → Direct value                                  │
│                                                                              │
│   CompiledOp {                                                               │
│       verb: "kyc.open-case",                                                │
│       behavior: Plugin,                                                      │
│       resolved_args: {                                                       │
│           "cbu-id": "550e8400-...",                                         │
│           "case-type": "ONBOARDING",                                        │
│       },                                                                     │
│       capture: Some("@case"),                                               │
│   }                                                                          │
│                                                                              │
│   Step 3: EXECUTE                                                            │
│   ──────────────                                                             │
│   Route to: KycOpenCaseOp.execute()                                         │
│                                                                              │
│   Handler does:                                                              │
│     1. INSERT INTO kyc_cases (cbu_id, case_type, status)                    │
│        VALUES ('550e8400-...', 'ONBOARDING', 'OPEN')                        │
│        RETURNING case_id                                                     │
│                                                                              │
│     2. INSERT INTO entity_workstreams (case_id, entity_id, ...)             │
│        for each discovered entity                                            │
│                                                                              │
│     3. Return case_id = "771e8400-..."                                      │
│                                                                              │
│   Step 4: CAPTURE                                                            │
│   ──────────────                                                             │
│   capture_map["@case"] = "771e8400-..."                                     │
│                                                                              │
│   Step 5: RESULT                                                             │
│   ─────────────                                                              │
│   {                                                                          │
│     "success": true,                                                         │
│     "result": {                                                              │
│       "case_id": "771e8400-...",                                            │
│       "status": "OPEN",                                                      │
│       "workstream_count": 3                                                  │
│     },                                                                       │
│     "captured": { "@case": "771e8400-..." }                                 │
│   }                                                                          │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Quick Reference: Key Files

```
Parser:           rust/crates/ob-dsl-parser/src/lib.rs
Compiler:         rust/src/dsl_v2/compiler.rs
Executor:         rust/src/dsl_v2/executor.rs
Verb Registry:    rust/src/dsl_v2/verb_registry.rs
Entity Gateway:   rust/src/dsl_v2/entity_gateway.rs
Crud Handler:     rust/src/dsl_v2/crud_handler.rs
Plugin Registry:  rust/src/dsl_v2/custom_ops/mod.rs
Verb Configs:     rust/config/verbs/**/*.yaml (94 files)
Templates:        rust/config/templates/*.yaml
```

---

Generated: 2026-01-09
