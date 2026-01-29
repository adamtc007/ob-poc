warning: variable `files_with_issues` is assigned to, but never used
    --> xtask/src/main.rs:1686:13
     |
1686 |     let mut files_with_issues = 0;
     |             ^^^^^^^^^^^^^^^^^
     |
     = note: consider using `_files_with_issues` instead
     = note: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default

warning: field `investor_type` is never read
   --> xtask/src/fund_programme.rs:115:9
    |
105 | pub struct FundRecord {
    |            ---------- field in this struct
...
115 |     pub investor_type: Option<String>,
    |         ^^^^^^^^^^^^^
    |
    = note: `FundRecord` has derived impls for the traits `Clone` and `Debug`, but these are intentionally ignored during dead code analysis
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: struct `LoadResult` is never constructed
   --> xtask/src/fund_programme.rs:122:12
    |
122 | pub struct LoadResult {
    |            ^^^^^^^^^^

warning: struct `DraftPhraseFile` is never constructed
   --> xtask/src/verb_migrate.rs:396:12
    |
396 | pub struct DraftPhraseFile {
    |            ^^^^^^^^^^^^^^^

warning: struct `DraftDomainPhrases` is never constructed
   --> xtask/src/verb_migrate.rs:402:12
    |
402 | pub struct DraftDomainPhrases {
    |            ^^^^^^^^^^^^^^^^^^

warning: struct `DraftVerbPhrases` is never constructed
   --> xtask/src/verb_migrate.rs:408:12
    |
408 | pub struct DraftVerbPhrases {
    |            ^^^^^^^^^^^^^^^^

warning: field `version` is never read
    --> xtask/src/verb_migrate.rs:1072:9
     |
1070 | pub struct V1SchemaFile {
     |            ------------ field in this struct
1071 |     #[serde(default)]
1072 |     pub version: String,
     |         ^^^^^^^
     |
     = note: `V1SchemaFile` has derived impls for the traits `Clone` and `Debug`, but these are intentionally ignored during dead code analysis

warning: field `behavior` is never read
    --> xtask/src/verb_migrate.rs:1092:9
     |
1086 | pub struct V1VerbContent {
     |            ------------- field in this struct
...
1092 |     pub behavior: String,
     |         ^^^^^^^^
     |
     = note: `V1VerbContent` has derived impls for the traits `Clone` and `Debug`, but these are intentionally ignored during dead code analysis

warning: `xtask` (bin "xtask") generated 8 warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.18s
     Running `target/debug/xtask playbook-check ../data/playbooks/pe-fund-setup.playbook.yaml ../data/playbooks/test-errors.playbook.yaml -v`
==========================================
  Playbook Check
==========================================

Checking: ../data/playbooks/pe-fund-setup.playbook.yaml
  Playbook: Private Equity Fund Setup
  Slots: 5
  Steps: 6
  Slot details:
    - fund_name (required)
    - fund_jurisdiction (required) = String("LU")
    - management_fee_bps (optional) = Number(200)
    - target_size_usd (optional)
    - gp_entity_name (required)
  WARNING: Missing required slots:
    - slot in step 0 (create-fund-structure)
    - slot in step 0 (create-fund-structure)
    - slot in step 1 (create-gp-entity)
    - slot in step 1 (create-gp-entity)
    - slot in step 2 (assign-gp-role)
    - slot in step 2 (assign-gp-role)
    - slot in step 3 (create-trading-profile)
    - slot in step 3 (create-trading-profile)
    - slot in step 4 (open-kyc-case)
    - slot in step 5 (request-documents)
  Generated 6 DSL statements

Checking: ../data/playbooks/test-errors.playbook.yaml
  Playbook: Test Playbook with Errors
  Slots: 2
  Steps: 4
  Slot details:
    - optional_value (optional) = String("default-value")
    - client_name (required)
  WARNING: Missing required slots:
    - slot in step 0 (step-ok)
    - slot in step 1 (step-missing-slot)
    - slot in step 1 (step-missing-slot)
    - slot in step 2 (step-with-default)
    - slot in step 2 (step-with-default)
    - slot in step 3 (step-more-missing)
    - slot in step 3 (step-more-missing)
  Generated 4 DSL statements

==========================================
Summary: 2 files checked, 0 errors, 17 warnings
==========================================
