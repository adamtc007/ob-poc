# UBO/KYC DSL Complete Specification

**Version:** 2.0  
**Generated:** 2025-11-10  
**Purpose:** DSL-as-State architecture for financial onboarding workflows  

## Table of Contents

1. [Overview](#overview)
2. [Design Principles](#design-principles)
3. [Architecture Patterns](#architecture-patterns)
4. [EBNF Grammar Specification](#ebnf-grammar-specification)
5. [Domain Vocabularies](#domain-vocabularies)
6. [Examples and Use Cases](#examples-and-use-cases)
7. [Implementation Notes](#implementation-notes)
8. [Areas for Review](#areas-for-review)

## Overview

The UBO/KYC DSL is a declarative domain-specific language designed for modeling complex financial services workflows, particularly Ultimate Beneficial Ownership (UBO) discovery and Know Your Customer (KYC) compliance processes. The language follows a **DSL-as-State** architecture where accumulated DSL documents serve as both state representation and audit trail.

### Key Features

- **S-expression syntax** for homoiconicity and composability
- **Multi-domain support** (KYC, UBO, Onboarding, Compliance)
- **AttributeID-as-Type pattern** using UUID references to universal dictionary
- **Immutable audit trails** through DSL versioning
- **Evidence-based relationships** with explicit source tracking
- **Graph-native entity modeling** for ownership structures
- **Conflict resolution strategies** for multi-source data

## Design Principles

### 1. DSL-as-State Pattern
The fundamental pattern: **The accumulated DSL document IS the state itself**.
- State = Accumulated DSL Document
- Immutable event sourcing through DSL versions
- Executable documentation that serves multiple purposes
- Complete audit trail embedded in language constructs

### 2. AttributeID-as-Type Pattern
Variables are typed by AttributeID (UUID) referencing a universal dictionary, not primitive types.

```lisp
(solicit-attribute :attr-id @attr{456789ab-cdef-1234-5678-9abcdef01201} ...)
```

Benefits:
- Universal data governance
- Privacy classification embedded in type system
- Cross-system data lineage
- Business context preservation

### 3. Evidence-Driven Design
All relationships and data points include explicit evidence tracking:

```lisp
(create-edge
  :from "alpha-holdings-sg"
  :to "company-zenith-spv-001"
  :type HAS_OWNERSHIP
  :properties {:percent 45.0}
  :evidenced-by ["doc-cayman-registry-001" "board-resolution-2023-03"])
```

## Architecture Patterns

### Multi-Domain Support
The DSL supports multiple business domains through a unified grammar with domain-specific vocabulary extensions:

- **Core Domain**: Entity graphs, relationships, evidence
- **KYC Domain**: Identity verification, document collection, risk assessment
- **UBO Domain**: Ownership calculations, beneficial interest discovery
- **Onboarding Domain**: Case management, product provisioning, service planning
- **Compliance Domain**: Regulatory reporting, sanctions screening, monitoring

### Vocabulary Management
Domain-prefixed verb convention: `domain.action`

Examples:
- `kyc.verify` - KYC identity verification
- `case.create` - Onboarding case creation
- `declare-entity` - UBO entity declaration (core domain, no prefix)

## EBNF Grammar Specification

### Top-Level Structure

```ebnf
program = workflow* ;

workflow = '(' 'define-kyc-investigation'
             string                         (* workflow-id *)
             property-list                  (* workflow properties *)
             statement*                     (* workflow statements *)
           ')' ;
```

### Core Statement Types

```ebnf
statement = declare-entity
          | obtain-document
          | create-edge
          | solicit-attribute
          | calculate-ubo
          | resolve-conflict
          | generate-report
          | schedule-monitoring
          | parallel-block
          | sequential-block
          | case-statement
          | product-statement
          | service-statement
          | kyc-statement
          | compliance-statement
          | workflow-transition ;
```

### Entity Declaration

```ebnf
declare-entity = '(' 'declare-entity'
                   ':node-id' identifier
                   ':label' entity-label
                   ':properties' property-map
                 ')' ;

entity-label = 'Company' | 'Person' | 'Trust' | 'Address'
             | 'Document' | 'Officer' | 'Partnership' | 'Foundation' ;
```

### Graph Edge Creation

```ebnf
create-edge = '(' 'create-edge'
                ':from' identifier
                ':to' identifier
                ':type' edge-type
                ':properties' property-map
                ':evidenced-by' evidence-list
              ')' ;

edge-type = 'HAS_OWNERSHIP' | 'HAS_CONTROL' | 'IS_DIRECTOR_OF'
          | 'IS_SECRETARY_OF' | 'HAS_SHAREHOLDER' | 'RESIDES_AT'
          | 'HAS_REGISTERED_OFFICE' | 'EVIDENCED_BY' | 'IS_SUBSIDIARY_OF'
          | 'HAS_VOTING_RIGHTS' | 'HAS_BENEFICIAL_INTEREST' ;
```

### UBO Calculations

```ebnf
calculate-ubo = '(' 'calculate-ubo-prongs'
                  ':target' identifier
                  ':algorithm' ubo-algorithm
                  ':max-depth' integer
                  ':threshold' number
                  ':traversal-rules' rule-map
                  ':output' output-map
                ')' ;

ubo-algorithm = 'direct_ownership' | 'indirect_ownership' | 'voting_control'
              | 'beneficial_interest' | 'combined_calculation' ;
```

### Conflict Resolution

```ebnf
resolve-conflict = '(' 'resolve-conflicts'
                     ':node' identifier
                     ':property' string
                     ':strategy' waterfall-strategy
                     ':resolution' resolution-map
                   ')' ;

waterfall-strategy = '(' 'waterfall'
                       source-priority+
                     ')' ;

source-priority = '(' source-type string ':confidence' number ')' ;

source-type = 'primary-source' | 'government-registry'
            | 'third-party-service' | 'self-declared'
            | 'verified-document' | 'regulatory-filing' ;
```

### Value Types and Primitives

```ebnf
value = string | number | boolean | date | identifier | uuid
      | list | map | multi-value | attribute-reference ;

multi-value = '[' value-with-source+ ']' ;
value-with-source = '{' ':value' value ':source' string ':confidence' number '}' ;

attribute-reference = '@attr{' uuid '}' ;        (* AttributeID reference *)

property-map = '{' (map-entry (',' map-entry)*)? '}' ;
map-entry = keyword value ;

keyword = ':' identifier ('.' identifier)? ;    (* Support namespaced keywords *)
```

### Complete EBNF Grammar Specification

```ebnf
(* ============================================================================
   UBO/KYC DSL - Complete Extended Backus-Naur Form (EBNF) Grammar
   ============================================================================ *)
(* Top-level program structure *)
program = workflow* ;

workflow = '(' 'define-kyc-investigation'
          string                         (* workflow-id *)
          property-list                  (* workflow properties *)
          statement*                     (* workflow statements *)
        ')' ;

(* Extended Statement Types - Multi-Domain Support *)
statement = declare-entity
       | obtain-document
       | create-edge
       | solicit-attribute
       | calculate-ubo
       | resolve-conflict
       | generate-report
       | schedule-monitoring
       | parallel-block
       | sequential-block
       | case-statement
       | product-statement
       | service-statement
       | kyc-statement
       | compliance-statement
       | workflow-transition ;

(* Entity Declaration - Core UBO Domain *)
declare-entity = '(' 'declare-entity'
                ':node-id' identifier
                ':label' entity-label
                ':properties' property-map
              ')' ;

entity-label = 'Company' | 'Person' | 'Trust' | 'Address'
          | 'Document' | 'Officer' | 'Partnership' | 'Foundation' ;

(* Document Operations - KYC Domain *)
obtain-document = '(' 'obtain-document'
                 ':doc-id' identifier
                 ':doc-type' document-type
                 ':issuer' string
                 ':issue-date' date
                 ':confidence' number
                 property-list?
               ')' ;

document-type = 'passport' | 'drivers_license' | 'utility_bill'
           | 'bank_statement' | 'articles_of_incorporation'
           | 'board_resolution' | 'power_of_attorney'
           | 'beneficial_ownership_certificate' | 'kyc_questionnaire' ;

parallel-obtain = '(' 'parallel-obtain'
                 obtain-document+
               ')' ;

(* Graph Edge Creation - UBO Relationships *)
create-edge = '(' 'create-edge'
             ':from' identifier
             ':to' identifier
             ':type' edge-type
             ':properties' property-map
             ':evidenced-by' evidence-list
           ')' ;

edge-type = 'HAS_OWNERSHIP' | 'HAS_CONTROL' | 'IS_DIRECTOR_OF'
       | 'IS_SECRETARY_OF' | 'HAS_SHAREHOLDER' | 'RESIDES_AT'
       | 'HAS_REGISTERED_OFFICE' | 'EVIDENCED_BY' | 'IS_SUBSIDIARY_OF'
       | 'HAS_VOTING_RIGHTS' | 'HAS_BENEFICIAL_INTEREST' ;

evidence-list = '[' (string (',' string)*)? ']' ;

(* Attribute Solicitation - Universal Dictionary Pattern *)
solicit-attribute = '(' 'solicit-attribute'
                   ':attr-id' uuid                    (* AttributeID reference *)
                   ':from' identifier
                   ':value-type' type-spec
                   property-list?
                 ')' ;

(* UBO Calculation - Core Business Logic *)
calculate-ubo = '(' 'calculate-ubo-prongs'
               ':target' identifier
               ':algorithm' ubo-algorithm
               ':max-depth' integer
               ':threshold' number
               ':traversal-rules' rule-map
               ':output' output-map
             ')' ;

ubo-algorithm = 'direct_ownership' | 'indirect_ownership' | 'voting_control'
           | 'beneficial_interest' | 'combined_calculation' ;

(* Conflict Resolution - Data Quality *)
resolve-conflict = '(' 'resolve-conflicts'
                  ':node' identifier
                  ':property' string
                  ':strategy' waterfall-strategy
                  ':resolution' resolution-map
                ')' ;

waterfall-strategy = '(' 'waterfall'
                    source-priority+
                  ')' ;

source-priority = '(' source-type string ':confidence' number ')' ;

source-type = 'primary-source' | 'government-registry'
         | 'third-party-service' | 'self-declared'
         | 'verified-document' | 'regulatory-filing' ;

(* Report Generation - Compliance Output *)
generate-report = '(' 'generate-ubo-report'
                 ':target' identifier
                 ':status' report-status
                 ':identified-ubos' ubo-list
                 ':unresolved-prongs' prong-list
                 property-list?
               ')' ;

report-status = 'COMPLETE' | 'INCOMPLETE' | 'UNDER_REVIEW' | 'REJECTED' ;

(* Monitoring and Alerts *)
schedule-monitoring = '(' 'schedule-monitoring'
                     ':target' identifier
                     ':frequency' monitoring-frequency
                     ':triggers' trigger-list
                     property-list?
                   ')' ;

monitoring-frequency = 'daily' | 'weekly' | 'monthly' | 'quarterly' | 'annually' ;

(* Case Management - Onboarding Domain *)
case-statement = '(' case-verb case-params* ')' ;
case-verb = 'case.create' | 'case.update' | 'case.close' | 'case.reopen' ;
case-params = '(' attribute-name value ')' ;

(* Product Management - Onboarding Domain *)
product-statement = '(' product-verb product-list* ')' ;
product-verb = 'products.add' | 'products.remove' | 'products.validate' ;
product-list = string+ ;

(* Service Planning - Onboarding Domain *)
service-statement = '(' service-verb service-spec* ')' ;
service-verb = 'services.plan' | 'services.provision' | 'services.configure' ;
service-spec = '(' 'service' string service-properties ')' ;
service-properties = '(' service-property* ')' ;
service-property = '(' property-name property-value ')' ;

(* KYC Operations - KYC Domain *)
kyc-statement = '(' kyc-verb kyc-params* ')' ;
kyc-verb = 'kyc.verify' | 'kyc.assess_risk' | 'kyc.collect_document'
      | 'kyc.screen_sanctions' | 'kyc.check_pep' | 'kyc.validate_address' ;
kyc-params = '(' attribute-name value ')' ;

(* Compliance Checks *)
compliance-statement = '(' compliance-verb compliance-params* ')' ;
compliance-verb = 'compliance.fatca_check' | 'compliance.crs_check'
             | 'compliance.sanctions_screen' | 'compliance.aml_check'
             | 'compliance.generate_sar' ;
compliance-params = '(' attribute-name value ')' ;

(* Workflow Transitions - State Management *)
workflow-transition = '(' 'workflow.transition' state-name property-list? ')' ;
state-name = string ;

(* Control Flow *)
parallel-block = '(' 'parallel' statement+ ')' ;
sequential-block = '(' 'sequential' statement+ ')' ;

(* Property Structures *)
property-list = (property-pair)* ;
property-pair = keyword value ;

property-map = '{' (map-entry (',' map-entry)*)? '}' ;
map-entry = keyword value ;

rule-map = '{' (rule-entry (',' rule-entry)*)? '}' ;
rule-entry = string rule-definition ;

output-map = '{' (output-entry (',' output-entry)*)? '}' ;
output-entry = string output-specification ;

resolution-map = '{' (resolution-entry (',' resolution-entry)*)? '}' ;
resolution-entry = string resolution-action ;

(* Value Types - Extended *)
value = string | number | boolean | date | identifier | uuid
   | list | map | multi-value | attribute-reference ;

multi-value = '[' value-with-source+ ']' ;
value-with-source = '{' ':value' value ':source' string ':confidence' number '}' ;

attribute-reference = '@attr{' uuid '}' ;        (* AttributeID reference *)

list = '[' (value (',' value)*)? ']' ;
map = '{' (map-entry (',' map-entry)*)? '}' ;

ubo-list = '[' (ubo-entry (',' ubo-entry)*)? ']' ;
ubo-entry = '{' ':entity-id' string ':percentage' number ':control-type' string '}' ;

prong-list = '[' (prong-entry (',' prong-entry)*)? ']' ;
prong-entry = '{' ':prong-type' string ':status' string ':reason' string '}' ;

trigger-list = '[' (trigger-entry (',' trigger-entry)*)? ']' ;
trigger-entry = '{' ':event-type' string ':threshold' number '}' ;

type-spec = primitive-type | composite-type | custom-type ;
primitive-type = 'string' | 'number' | 'boolean' | 'date' | 'uuid' ;
composite-type = 'list' | 'map' | 'set' ;
custom-type = 'percentage' | 'currency' | 'country-code' | 'entity-type' ;

(* Primitives *)
string = '"' character* '"' ;
number = integer | float | percentage | currency ;
integer = digit+ ;
float = digit+ '.' digit+ ;
percentage = float '%' ;
currency = currency-code space float ;
currency-code = 'USD' | 'EUR' | 'GBP' | 'JPY' | 'CHF' | 'CAD' ;

boolean = 'true' | 'false' ;
date = string ;  (* ISO 8601 format in practice *)
uuid = string ;  (* UUID format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx *)

identifier = (letter | '-' | '_') (letter | digit | '-' | '_')* ;
keyword = ':' identifier ('.' identifier)? ;    (* Support namespaced keywords *)
attribute-name = keyword | attribute-reference ;
property-name = keyword ;
property-value = value ;

letter = 'a'..'z' | 'A'..'Z' ;
digit = '0'..'9' ;
character = ? any character except '"' and '\' ? | '\' escape-sequence ;
escape-sequence = '\"' | '\\' | '\n' | '\r' | '\t' ;
space = ' ' | '\t' ;

(* Rule and Definition Support *)
rule-definition = '{' ':type' string ':parameters' map '}' ;
output-specification = '{' ':format' string ':destination' string '}' ;
resolution-action = '{' ':action' string ':parameters' map '}' ;

(* Advanced Constructs *)
(* Conditional Constructs *)
conditional = if-statement | when-statement | case-condition ;
if-statement = '(' 'if' condition statement statement? ')' ;
when-statement = '(' 'when' condition statement* ')' ;
case-condition = '(' 'case' value case-clause+ default-clause? ')' ;
case-clause = '(' value statement* ')' ;
default-clause = '(' ':default' statement* ')' ;
condition = comparison | logical-expression | predicate-call ;
comparison = '(' comparison-operator value value ')' ;
comparison-operator = '=' | '!=' | '<' | '>' | '<=' | '>=' ;
logical-expression = '(' logical-operator condition+ ')' ;
logical-operator = 'and' | 'or' | 'not' ;
predicate-call = '(' predicate-name argument-list ')' ;
predicate-name = identifier ;
argument-list = value* ;

(* Temporal Constructs *)
temporal-statement = '(' temporal-verb temporal-spec statement* ')' ;
temporal-verb = 'at-time' | 'during-period' | 'every' | 'delay' ;
temporal-spec = date | time-period | duration ;
time-period = '{' ':start' date ':end' date '}' ;
duration = number time-unit ;
time-unit = 'seconds' | 'minutes' | 'hours' | 'days' | 'weeks' | 'months' | 'years' ;

(* Error Handling Constructs *)
try-catch = '(' 'try' statement* '(' 'catch' error-type identifier statement* ')' ')' ;
error-type = 'ValidationError' | 'BusinessRuleError' | 'DataInconsistencyError' ;

(* Comments and Metadata *)
comment = ';;' ? any character except newline ? newline ;
metadata = '^' '{' map-entry* '}' ;              (* Clojure-style metadata *)

(* Grammar Extensions for Domain-Specific Constructs *)
domain-extension = '(' 'defgrammar' string grammar-rule+ ')' ;
grammar-rule = identifier '::=' production-rule ;
production-rule = (terminal | non-terminal | operator)+ ;
terminal = string ;
non-terminal = identifier ;
operator = '|' | '*' | '+' | '?' | '(' | ')' ;

(* Security and Privacy Annotations *)
privacy-annotation = '(' 'privacy' privacy-level statement ')' ;
privacy-level = 'public' | 'internal' | 'confidential' | 'restricted' ;
access-control = '(' 'access' role-list statement ')' ;
role-list = '[' string+ ']' ;

(* Validation Constructs *)
validation-rule = '(' 'validate' validation-spec ')' ;
validation-spec = '{' ':rule' string ':severity' severity-level ':message' string '}' ;
severity-level = 'error' | 'warning' | 'info' ;

(* Internationalization Support *)
i18n-string = '(' 'i18n' locale-map ')' ;
locale-map = '{' (locale-entry (',' locale-entry)*)? '}' ;
locale-entry = locale-code string ;
locale-code = 'en-US' | 'en-GB' | 'fr-FR' | 'de-DE' | 'ja-JP' | 'zh-CN' ;

(* Context-Aware Constructs *)
context-statement = '(' 'with-context' context-map statement* ')' ;
context-map = '{' (context-entry (',' context-entry)*)? '}' ;
context-entry = context-key context-value ;
context-key = 'jurisdiction' | 'regulatory-framework' | 'business-line'
         | 'risk-profile' | 'entity-type' | 'processing-date' ;
context-value = value ;

(* Audit and Compliance Metadata *)
audit-metadata = '(' 'audit' audit-fields ')' ;
audit-fields = '{' (audit-entry (',' audit-entry)*)? '}' ;
audit-entry = audit-key audit-value ;
audit-key = 'created-by' | 'created-at' | 'version' | 'approval-required'
       | 'retention-period' | 'classification' | 'source-system' ;
audit-value = value ;
```
=======
(* --- v3.0 EBNF --- *)
program   = form* ;
form      = "(" verb (key value)* ")" | comment ;
comment   = ";;" ? any character except newline ? ;

key       = ":" identifier ( "." identifier )? ;
value     = literal | identifier | list | map | attr-ref ;

literal   = string | number | boolean | date | uuid ;
list      = "[" (value ("," value)*)? "]" ;
map       = "{" (key value ("," key value)*)? "}" ;
attr-ref  = "@attr{" uuid "}" ;

(* Primitives are unchanged from v2.0 where applicable and fully defined below *)
string    = '"' character* '"' ;
number    = integer | float | percentage | currency ;
boolean   = "true" | "false" ;
date      = string ;
uuid      = string ;
identifier= (letter | '_' | '-') (letter | digit | '_' | '-')* ;

integer = digit+ ;
float = digit+ '.' digit+ ;
percentage = float '%' ;
currency = currency-code space float ;
currency-code = 'USD' | 'EUR' | 'GBP' | 'JPY' | 'CHF' | 'CAD' ;

keyword = ':' identifier ('.' identifier)? ;
attribute-name = keyword | attr-ref ;
property-name = keyword ;
property-value = value ;

letter = 'a'..'z' | 'A'..'Z' ;
digit = '0'..'9' ;
character = ? any character except '"' and '\' ? | '\' escape-sequence ;
escape-sequence = '\"' | '\\' | '\n' | '\r' | '\t' ;
space = ' ' | '\t' ;
(* Top-level program structure *)
program = workflow* ;

workflow = '(' 'define-kyc-investigation'
             string                         (* workflow-id *)
             property-list                  (* workflow properties *)
             statement*                     (* workflow statements *)
           ')' ;

(* Extended Statement Types - Multi-Domain Support *)
statement = declare-entity
          | obtain-document
          | create-edge
          | solicit-attribute
          | calculate-ubo
          | resolve-conflict
          | generate-report
          | schedule-monitoring
          | parallel-block
          | sequential-block
          | case-statement
          | product-statement
          | service-statement
          | kyc-statement
          | compliance-statement
          | workflow-transition ;

(* Entity Declaration - Core UBO Domain *)
declare-entity = '(' 'declare-entity'
                   ':node-id' identifier
                   ':label' entity-label
                   ':properties' property-map
                 ')' ;

entity-label = 'Company' | 'Person' | 'Trust' | 'Address'
             | 'Document' | 'Officer' | 'Partnership' | 'Foundation' ;

(* Document Operations - KYC Domain *)
obtain-document = '(' 'obtain-document'
                    ':doc-id' identifier
                    ':doc-type' document-type
                    ':issuer' string
                    ':issue-date' date
                    ':confidence' number
                    property-list?
                  ')' ;

document-type = 'passport' | 'drivers_license' | 'utility_bill'
              | 'bank_statement' | 'articles_of_incorporation'
              | 'board_resolution' | 'power_of_attorney'
              | 'beneficial_ownership_certificate' | 'kyc_questionnaire' ;

parallel-obtain = '(' 'parallel-obtain'
                    obtain-document+
                  ')' ;

(* Graph Edge Creation - UBO Relationships *)
create-edge = '(' 'create-edge'
                ':from' identifier
                ':to' identifier
                ':type' edge-type
                ':properties' property-map
                ':evidenced-by' evidence-list
              ')' ;

edge-type = 'HAS_OWNERSHIP' | 'HAS_CONTROL' | 'IS_DIRECTOR_OF'
          | 'IS_SECRETARY_OF' | 'HAS_SHAREHOLDER' | 'RESIDES_AT'
          | 'HAS_REGISTERED_OFFICE' | 'EVIDENCED_BY' | 'IS_SUBSIDIARY_OF'
          | 'HAS_VOTING_RIGHTS' | 'HAS_BENEFICIAL_INTEREST' ;

evidence-list = '[' (string (',' string)*)? ']' ;

(* Attribute Solicitation - Universal Dictionary Pattern *)
solicit-attribute = '(' 'solicit-attribute'
                      ':attr-id' uuid                    (* AttributeID reference *)
                      ':from' identifier
                      ':value-type' type-spec
                      property-list?
                    ')' ;

(* UBO Calculation - Core Business Logic *)
calculate-ubo = '(' 'calculate-ubo-prongs'
                  ':target' identifier
                  ':algorithm' ubo-algorithm
                  ':max-depth' integer
                  ':threshold' number
                  ':traversal-rules' rule-map
                  ':output' output-map
                ')' ;

ubo-algorithm = 'direct_ownership' | 'indirect_ownership' | 'voting_control'
              | 'beneficial_interest' | 'combined_calculation' ;

(* Conflict Resolution - Data Quality *)
resolve-conflict = '(' 'resolve-conflicts'
                     ':node' identifier
                     ':property' string
                     ':strategy' waterfall-strategy
                     ':resolution' resolution-map
                   ')' ;

waterfall-strategy = '(' 'waterfall'
                       source-priority+
                     ')' ;

source-priority = '(' source-type string ':confidence' number ')' ;

source-type = 'primary-source' | 'government-registry'
            | 'third-party-service' | 'self-declared'
            | 'verified-document' | 'regulatory-filing' ;

(* Report Generation - Compliance Output *)
generate-report = '(' 'generate-ubo-report'
                    ':target' identifier
                    ':status' report-status
                    ':identified-ubos' ubo-list
                    ':unresolved-prongs' prong-list
                    property-list?
                  ')' ;

report-status = 'COMPLETE' | 'INCOMPLETE' | 'UNDER_REVIEW' | 'REJECTED' ;

(* Monitoring and Alerts *)
schedule-monitoring = '(' 'schedule-monitoring'
                        ':target' identifier
                        ':frequency' monitoring-frequency
                        ':triggers' trigger-list
                        property-list?
                      ')' ;

monitoring-frequency = 'daily' | 'weekly' | 'monthly' | 'quarterly' | 'annually' ;

(* Case Management - Onboarding Domain *)
case-statement = '(' case-verb case-params* ')' ;
case-verb = 'case.create' | 'case.update' | 'case.close' | 'case.reopen' ;
case-params = '(' attribute-name value ')' ;

(* Product Management - Onboarding Domain *)
product-statement = '(' product-verb product-list* ')' ;
product-verb = 'products.add' | 'products.remove' | 'products.validate' ;
product-list = string+ ;

(* Service Planning - Onboarding Domain *)
service-statement = '(' service-verb service-spec* ')' ;
service-verb = 'services.plan' | 'services.provision' | 'services.configure' ;
service-spec = '(' 'service' string service-properties ')' ;
service-properties = '(' service-property* ')' ;
service-property = '(' property-name property-value ')' ;

(* KYC Operations - KYC Domain *)
kyc-statement = '(' kyc-verb kyc-params* ')' ;
kyc-verb = 'kyc.verify' | 'kyc.assess_risk' | 'kyc.collect_document'
         | 'kyc.screen_sanctions' | 'kyc.check_pep' | 'kyc.validate_address' ;
kyc-params = '(' attribute-name value ')' ;

(* Compliance Checks *)
compliance-statement = '(' compliance-verb compliance-params* ')' ;
compliance-verb = 'compliance.fatca_check' | 'compliance.crs_check'
                | 'compliance.sanctions_screen' | 'compliance.aml_check'
                | 'compliance.generate_sar' ;
compliance-params = '(' attribute-name value ')' ;

(* Workflow Transitions - State Management *)
workflow-transition = '(' 'workflow.transition' state-name property-list? ')' ;
state-name = string ;

(* Control Flow *)
parallel-block = '(' 'parallel' statement+ ')' ;
sequential-block = '(' 'sequential' statement+ ')' ;

(* Property Structures *)
property-list = (property-pair)* ;
property-pair = keyword value ;

property-map = '{' (map-entry (',' map-entry)*)? '}' ;
map-entry = keyword value ;

rule-map = '{' (rule-entry (',' rule-entry)*)? '}' ;
rule-entry = string rule-definition ;

output-map = '{' (output-entry (',' output-entry)*)? '}' ;
output-entry = string output-specification ;

resolution-map = '{' (resolution-entry (',' resolution-entry)*)? '}' ;
resolution-entry = string resolution-action ;

(* Value Types - Extended *)
value = string | number | boolean | date | identifier | uuid
      | list | map | multi-value | attribute-reference ;

multi-value = '[' value-with-source+ ']' ;
value-with-source = '{' ':value' value ':source' string ':confidence' number '}' ;

attribute-reference = '@attr{' uuid '}' ;        (* AttributeID reference *)

list = '[' (value (',' value)*)? ']' ;
map = '{' (map-entry (',' map-entry)*)? '}' ;

ubo-list = '[' (ubo-entry (',' ubo-entry)*)? ']' ;
ubo-entry = '{' ':entity-id' string ':percentage' number ':control-type' string '}' ;

prong-list = '[' (prong-entry (',' prong-entry)*)? ']' ;
prong-entry = '{' ':prong-type' string ':status' string ':reason' string '}' ;

trigger-list = '[' (trigger-entry (',' trigger-entry)*)? ']' ;
trigger-entry = '{' ':event-type' string ':threshold' number '}' ;

type-spec = primitive-type | composite-type | custom-type ;
primitive-type = 'string' | 'number' | 'boolean' | 'date' | 'uuid' ;
composite-type = 'list' | 'map' | 'set' ;
custom-type = 'percentage' | 'currency' | 'country-code' | 'entity-type' ;

(* Primitives *)
string = '"' character* '"' ;
number = integer | float | percentage | currency ;
integer = digit+ ;
float = digit+ '.' digit+ ;
percentage = float '%' ;
currency = currency-code space float ;
currency-code = 'USD' | 'EUR' | 'GBP' | 'JPY' | 'CHF' | 'CAD' ;

boolean = 'true' | 'false' ;
date = string ;  (* ISO 8601 format in practice *)
uuid = string ;  (* UUID format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx *)

identifier = (letter | '-' | '_') (letter | digit | '-' | '_')* ;
keyword = ':' identifier ('.' identifier)? ;    (* Support namespaced keywords *)
attribute-name = keyword | attribute-reference ;
property-name = keyword ;
property-value = value ;

letter = 'a'..'z' | 'A'..'Z' ;
digit = '0'..'9' ;
character = ? any character except '"' and '\' ? | '\' escape-sequence ;
escape-sequence = '\"' | '\\' | '\n' | '\r' | '\t' ;
space = ' ' | '\t' ;

(* Rule and Definition Support *)
rule-definition = '{' ':type' string ':parameters' map '}' ;
output-specification = '{' ':format' string ':destination' string '}' ;
resolution-action = '{' ':action' string ':parameters' map '}' ;

(* Advanced Constructs *)
(* Conditional Constructs *)
conditional = if-statement | when-statement | case-condition ;
if-statement = '(' 'if' condition statement statement? ')' ;
when-statement = '(' 'when' condition statement* ')' ;
case-condition = '(' 'case' value case-clause+ default-clause? ')' ;
case-clause = '(' value statement* ')' ;
default-clause = '(' ':default' statement* ')' ;
condition = comparison | logical-expression | predicate-call ;
comparison = '(' comparison-operator value value ')' ;
comparison-operator = '=' | '!=' | '<' | '>' | '<=' | '>=' ;
logical-expression = '(' logical-operator condition+ ')' ;
logical-operator = 'and' | 'or' | 'not' ;
predicate-call = '(' predicate-name argument-list ')' ;
predicate-name = identifier ;
argument-list = value* ;

(* Temporal Constructs *)
temporal-statement = '(' temporal-verb temporal-spec statement* ')' ;
temporal-verb = 'at-time' | 'during-period' | 'every' | 'delay' ;
temporal-spec = date | time-period | duration ;
time-period = '{' ':start' date ':end' date '}' ;
duration = number time-unit ;
time-unit = 'seconds' | 'minutes' | 'hours' | 'days' | 'weeks' | 'months' | 'years' ;

(* Error Handling Constructs *)
try-catch = '(' 'try' statement* '(' 'catch' error-type identifier statement* ')' ')' ;
error-type = 'ValidationError' | 'BusinessRuleError' | 'DataInconsistencyError' ;

(* Comments and Metadata *)
comment = ';;' ? any character except newline ? newline ;
metadata = '^' '{' map-entry* '}' ;              (* Clojure-style metadata *)

(* Grammar Extensions for Domain-Specific Constructs *)
domain-extension = '(' 'defgrammar' string grammar-rule+ ')' ;
grammar-rule = identifier '::=' production-rule ;
production-rule = (terminal | non-terminal | operator)+ ;
terminal = string ;
non-terminal = identifier ;
operator = '|' | '*' | '+' | '?' | '(' | ')' ;

(* Security and Privacy Annotations *)
privacy-annotation = '(' 'privacy' privacy-level statement ')' ;
privacy-level = 'public' | 'internal' | 'confidential' | 'restricted' ;
access-control = '(' 'access' role-list statement ')' ;
role-list = '[' string+ ']' ;

(* Validation Constructs *)
validation-rule = '(' 'validate' validation-spec ')' ;
validation-spec = '{' ':rule' string ':severity' severity-level ':message' string '}' ;
severity-level = 'error' | 'warning' | 'info' ;

(* Internationalization Support *)
i18n-string = '(' 'i18n' locale-map ')' ;
locale-map = '{' (locale-entry (',' locale-entry)*)? '}' ;
locale-entry = locale-code string ;
locale-code = 'en-US' | 'en-GB' | 'fr-FR' | 'de-DE' | 'ja-JP' | 'zh-CN' ;

(* Context-Aware Constructs *)
context-statement = '(' 'with-context' context-map statement* ')' ;
context-map = '{' (context-entry (',' context-entry)*)? '}' ;
context-entry = context-key context-value ;
context-key = 'jurisdiction' | 'regulatory-framework' | 'business-line'
            | 'risk-profile' | 'entity-type' | 'processing-date' ;
context-value = value ;

(* Audit and Compliance Metadata *)
audit-metadata = '(' 'audit' audit-fields ')' ;
audit-fields = '{' (audit-entry (',' audit-entry)*)? '}' ;
audit-entry = audit-key audit-value ;
audit-key = 'created-by' | 'created-at' | 'version' | 'approval-required'
          | 'retention-period' | 'classification' | 'source-system' ;
audit-value = value ;
```

## Domain Vocabularies

### Core/UBO Domain

**Purpose**: Entity relationship modeling and Ultimate Beneficial Ownership calculations

**Complete Verb Definitions**:
- `declare-entity` - Declare an entity in the ownership graph
  - **Signature**: `(declare-entity :node-id string :label symbol :properties map)`
  - **Example**: `(declare-entity :node-id "company-001" :label Company :properties {:legal-name "Acme Corp"})`
  - **Validation**: require_node_id, require_label

- `create-edge` - Create ownership/control relationships
  - **Signature**: `(create-edge :from string :to string :type symbol :properties map)`
  - **Example**: `(create-edge :from "person-001" :to "company-001" :type HAS_OWNERSHIP :properties {:percent 51.0})`
  - **Validation**: require_from_to, validate_percentage

- `calculate-ubo` - Perform UBO calculations with configurable algorithms
  - **Signature**: `(calculate-ubo :target-entity string :jurisdiction string :ubo-threshold number)`
  - **Example**: `(calculate-ubo :target-entity "company-001" :jurisdiction "US" :ubo-threshold 25.0)`
  - **Validation**: require_target_entity, validate_threshold

- `resolve-conflicts` - Handle conflicting data from multiple sources
  - **Signature**: `(resolve-conflicts :node identifier :property string :strategy waterfall-strategy)`
  - **Example**: `(resolve-conflicts :node "entity-001" :property "legal-name" :strategy (waterfall ...))`

**Complete Attribute Definitions**:
- `@attr{789abcde-f012-3456-7890-abcdef123401}` - `ubo.ownership_percentage` (decimal, 0-100)
- `@attr{789abcde-f012-3456-7890-abcdef123402}` - `ubo.threshold` (decimal, 0-100)

**Type Definitions**:
- `ownership_percentage` - decimal with constraints min:0.0, max:100.0, pattern: `^\\d{1,3}(\\.\\d{1,2})?$`

**State Transitions**:
- INITIAL → ENTITIES_DECLARED → RELATIONSHIPS_MAPPED → OWNERSHIP_CALCULATED

**Grammar Extensions**:
```ebnf
ubo_entity ::= "(" "declare-entity" entity_params+ ")"
ubo_edge ::= "(" "create-edge" edge_params+ ")"
entity_params ::= ":" identifier value
```

### KYC Domain

**Purpose**: Customer verification, document collection, and risk assessment

**Complete Verb Definitions**:
- `kyc.verify` - Verify customer identity
  - **Signature**: `(kyc.verify (customer.id string) (verification.method string) ...)`
  - **Example**: `(kyc.verify (customer.id "CUST-001") (verification.method "document_check"))`
  - **Category**: verification
  - **Validation**: require_customer_id

- `kyc.assess_risk` - Assess customer risk level
  - **Signature**: `(kyc.assess_risk (risk.score number) (risk.factors string...) ...)`
  - **Example**: `(kyc.assess_risk (risk.score 25.5) (risk.factors "PEP" "high_value"))`
  - **Category**: risk_assessment
  - **Validation**: validate_risk_score

- `kyc.collect_document` - Collect KYC documentation
  - **Signature**: `(kyc.collect_document (type string) (required boolean) ...)`
  - **Example**: `(kyc.collect_document (type "passport") (required true))`
  - **Category**: document_management
  - **Validation**: require_document_type

- `kyc.screen_sanctions` - Perform sanctions screening
- `kyc.check_pep` - Check Politically Exposed Person status
- `kyc.validate_address` - Validate customer addresses

**Complete Attribute Definitions**:
- `@attr{456789ab-cdef-1234-5678-9abcdef01201}` - `kyc.risk_rating` (decimal, 0-100)
- `@attr{456789ab-cdef-1234-5678-9abcdef01202}` - `kyc.verification_status` (enum: pending, verified, failed)

**Type Definitions**:
- `risk_score` - decimal with constraints min:0, max:100, pattern: `^\\d{1,2}(\\.\\d{1,2})?$`

**Validation Rules**:
- `require_customer_id` - All KYC operations must reference a valid customer (Error)
- `validate_risk_score` - Risk scores must be between 0 and 100 (Error)
- `require_document_type` - Document collection requires valid document type (Error)

**State Transitions**:
- INITIAL → DOCUMENTS_COLLECTED → IDENTITY_VERIFIED → RISK_ASSESSED → COMPLIANCE_CHECKED → UBO_DISCOVERED → APPROVED/REJECTED

**Grammar Extensions**:
```ebnf
kyc_verification ::= "(" "kyc.verify" verification_params+ ")"
verification_params ::= "(" attribute_name value ")"
```

### Onboarding Domain

**Purpose**: Client onboarding workflows and case management

**Complete Verb Definitions**:
- `case.create` - Create a new onboarding case
  - **Signature**: `(case.create (entity.id string) (entity.type string) ...)`
  - **Example**: `(case.create (entity.id "ENT-001") (entity.type "Company"))`
  - **Category**: case_management
  - **Validation**: require_entity_id, require_entity_type

- `products.add` - Add products to an onboarding case
  - **Signature**: `(products.add product...)`
  - **Example**: `(products.add "CUSTODY" "FUND_ACCOUNTING")`
  - **Category**: product_management
  - **Validation**: require_valid_products

- `cbu.associate` - Associate a CBU with an entity
  - **Signature**: `(cbu.associate (cbu.id string) (association.type string) ...)`
  - **Example**: `(cbu.associate (cbu.id "CBU-1234") (association.type "primary"))`
  - **Category**: cbu_management
  - **Validation**: require_cbu_id

- `services.plan` - Plan service provisioning
- `services.provision` - Provision planned services
- `services.configure` - Configure service parameters

**Complete Attribute Definitions**:
- `@attr{123e4567-e89b-12d3-a456-426614174001}` - `onboard.cbu_id` (string, CBU identifier)
- `@attr{123e4567-e89b-12d3-a456-426614174002}` - `onboard.nature_purpose` (string, min length 10)

**Type Definitions**:
- `cbu_id` - string with pattern `^CBU-\\d{4}-\\d{3}$`

**Validation Rules**:
- `require_entity_id` - All onboarding cases must have an entity ID (Error)
- `require_cbu_id` - CBU association requires valid CBU ID (Error)
- `require_entity_type` - Entity type must be specified (Error)
- `require_valid_products` - Products must be from approved list (Error)

**State Transitions**:
- CREATE → PRODUCTS_ADDED (add_products) → KYC_STARTED (start_kyc) → SERVICES_PLANNED → RESOURCES_ALLOCATED → GO_LIVE → COMPLETE

**Grammar Extensions**:
```ebnf
onboarding_case ::= "(" "case.create" entity_definition+ ")"
entity_definition ::= "(" attribute_name value ")"
```

### Compliance Domain

**Purpose**: Regulatory compliance, reporting, and monitoring

**Complete Verb Definitions**:
- `compliance.fatca_check` - FATCA status verification
  - **Signature**: `(compliance.fatca_check (entity.id string) (classification string) ...)`
  - **Example**: `(compliance.fatca_check (entity.id "ENT-001") (classification "NON_US"))`
  - **Category**: regulatory_compliance

- `compliance.crs_check` - Common Reporting Standard compliance
  - **Signature**: `(compliance.crs_check (entity.id string) (reporting.jurisdiction string) ...)`
  - **Category**: regulatory_compliance

- `compliance.sanctions_screen` - Sanctions database screening
  - **Signature**: `(compliance.sanctions_screen (entity.id string) (databases list) ...)`
  - **Example**: `(compliance.sanctions_screen (entity.id "ENT-001") (databases ["OFAC" "EU_SANCTIONS"]))`
  - **Category**: aml_compliance

- `compliance.aml_check` - Anti-Money Laundering checks
  - **Category**: aml_compliance

- `compliance.generate_sar` - Suspicious Activity Report generation
  - **Signature**: `(compliance.generate_sar (entity.id string) (reason string) ...)`
  - **Category**: reporting

- `compliance.monitor_transactions` - Ongoing transaction monitoring
  - **Example**: `(compliance.monitor_transactions (entity.id "ENT-001") (thresholds {:daily 10000}))`

**Compliance Frameworks Supported**:
- FATCA (Foreign Account Tax Compliance Act)
- CRS (Common Reporting Standard)
- EU 5th Money Laundering Directive (5MLD)
- UK Money Laundering Regulations (MLR)
- OFAC Sanctions
- EU Sanctions
- UN Sanctions

**Example**:
```lisp
(compliance.fatca_check
  (entity.id "ENT-001")
  (classification "NON_US")
  (certification_date "2024-01-15"))

(compliance.monitor_transactions
  (entity.id "ENT-001")
  (thresholds {:daily 10000 :monthly 50000})
  (alert_conditions ["unusual_pattern" "high_risk_jurisdiction"]))
```

### Advanced Constructs

#### Multi-Source Data Handling

```lisp
(solicit-attribute
  :attr-id @attr{customer.legal_name}
  :from "entity-001"
  :value-type string
  :sources [
    {:value "Acme Corporation" :source "government-registry" :confidence 95.0}
    {:value "ACME CORP" :source "self-declared" :confidence 60.0}
    {:value "Acme Corp Ltd" :source "third-party-service" :confidence 80.0}
  ])
```

#### Temporal Constructs

```lisp
(schedule-monitoring
  :target "entity-001"
  :frequency monthly
  :triggers [
    {:event-type "ownership_change" :threshold 5.0}
    {:event-type "risk_score_increase" :threshold 10.0}
  ])
```

#### Conditional Logic

```lisp
(when (> (get-risk-score "entity-001") 75.0)
  (compliance.generate_sar
    :entity "entity-001"
    :reason "high_risk_threshold_exceeded")
  (kyc.enhanced_due_diligence
    :level "enhanced"
    :required_documents ["source_of_wealth" "source_of_funds"]))
```

## Examples and Use Cases

### Complete UBO Discovery Workflow

```lisp
(define-kyc-investigation
  :id "zenith-capital-ubo-discovery"
  :target-entity "company-zenith-spv-001"
  :jurisdiction "KY"
  :ubo-threshold 25.0
  :regulatory-framework ["EU5MLD" "UK_MLR"]

  ;; Entity Declaration
  (entity
    :id "company-zenith-spv-001"
    :label "Company"
    :props {
      :legal-name "Zenith Capital Partners LP"
      :registration-number "KY-123456"
      :jurisdiction "KY"
      :incorporation-date "2020-03-15"
    })

  ;; Ownership Structure
  (edge
    :from "alpha-holdings-sg"
    :to "company-zenith-spv-001"
    :type "HAS_OWNERSHIP"
    :props {
      :percent 45.0
      :share-class "Class A Ordinary"
      :voting-rights true
    }
    :evidence ["doc-cayman-registry-001", "share-register-2024"])

  ;; KYC Integration
  (kyc.verify
    :customer-id "company-zenith-spv-001"
    :method "enhanced_due_diligence"
    :required-documents ["certificate_incorporation", "board_resolution"])

  ;; UBO Calculation
  (ubo.calc
    :target "company-zenith-spv-001"
    :algorithm "combined_calculation"
    :max-depth 5
    :threshold 25.0
    :traversal-rules {
      :follow-voting-rights true,
      :consolidate-family-holdings false,
      :trust-beneficial-interest true
    })

  ;; Compliance Checks
  (compliance.sanctions_screen
    :entities ["alpha-holdings-sg", "company-zenith-spv-001"]
    :databases ["OFAC", "EU_SANCTIONS", "UN_SANCTIONS"])

  ;; UBO Outcome (Declarative Result)
  (ubo.outcome
    :target "company-zenith-spv-001"
    :at "2024-11-10T10:30:00Z"
    :threshold 25.0
    :ubos [
      {:entity "person-john-smith", :percentage 45.0, :control-type "direct"}
    ]
    :certification-date "2024-11-10"))
```

### UCITS Fund Onboarding

```lisp
(case.create
  (cbu.id "CBU-UCITS-001")
  (entity.type "Investment_Fund")
  (nature-purpose "UCITS equity fund domiciled in LU"))

(products.add "CUSTODY" "FUND_ACCOUNTING" "REGULATORY_REPORTING" "RISK_MANAGEMENT")

(kyc.start
  (documents (document "CSSF_License") (document "Fund_Prospectus"))
  (jurisdictions (jurisdiction "LU"))
  (regulatory-framework "UCITS_V"))

(services.plan
  (service "Settlement" (sla "T+1") (currencies ["EUR" "USD"]))
  (service "ValuationEngine" (frequency "daily") (cut-off "18:00_CET"))
  (service "RegulatoryReporting" 
    (frameworks ["UCITS_V" "AIFMD" "EMIR"])
    (frequency "monthly")))

(workflow.transition "ONBOARDING_COMPLETE")
```

## Implementation Notes

### Current Implementation Status

**Rust Core Library** (Production Ready):
- ✅ NOM-based EBNF parser with comprehensive error handling
- ✅ Multi-domain AST with type-safe representations
- ✅ PostgreSQL integration with JSON AST storage
- ✅ Execution engine with state management
- ✅ Vocabulary registry with domain-specific extensions

**Go Semantic Agent System** (Production Ready):
- ✅ Database-driven DSL operations
- ✅ AI-assisted workflow generation via Gemini API
- ✅ Complete version history and audit trails
- ✅ 95%+ confidence semantic verb matching

**egui Desktop Visualizer** (In Development):
- ✅ Interactive AST tree visualization
- ✅ Pan/zoom capabilities for complex structures
- ✅ REST API integration for live data
- 🔄 Mock API server for testing

### Database Schema

**Canonical Schema**: PostgreSQL `"ob-poc"` with:
- Universal attribute dictionary with UUID keys
- Immutable DSL version storage as JSON AST
- Graph relationship modeling for ownership structures  
- Comprehensive audit trails with evidence tracking
- Semantic verb registry for AI agent operations

### Performance Considerations

- **Parser Performance**: NOM combinator library optimized for S-expressions
- **Storage Efficiency**: JSON AST with indexed UUID references
- **Query Performance**: Composite indexes on `(cbu_id, created_at DESC)`
- **Memory Usage**: Lazy AST loading with configurable cache size

## Areas for Review

### 1. Grammar Completeness
- **Question**: Are there missing constructs for complex financial workflows?
- **Specific Areas**: 
  - Derivative instruments and complex securities
  - Cross-border regulatory compliance variations
  - Multi-currency calculations and FX handling
  - Time-series data and historical analysis

### 2. Type System Soundness
- **AttributeID-as-Type Pattern**: Is the UUID reference approach too complex?
- **Type Safety**: Should we add compile-time type checking for AttributeID references?
- **Validation**: Are the current validation rules comprehensive enough?

### 3. Vocabulary Domain Boundaries
- **Domain Separation**: Are the current domain boundaries logical and maintainable?
- **Verb Naming**: Is the `domain.action` convention intuitive for business users?
- **Shared Vocabulary**: Which verbs should be promoted to core/shared status?

### 4. Evidence and Audit Design
- **Evidence Tracking**: Is the `:evidenced-by` pattern sufficient for regulatory requirements?
- **Audit Completeness**: Does the DSL capture enough metadata for compliance audits?
- **Data Lineage**: Can we trace data provenance through complex transformations?

### 5. Conflict Resolution Strategies
- **Waterfall Strategy**: Are the current source priority mechanisms robust?
- **Business Rules**: How should conflicts between regulatory requirements be handled?
- **Confidence Scores**: Is the numerical confidence model appropriate for legal/compliance contexts?

### 6. Scalability and Performance
- **Large Graphs**: How does the system perform with complex ownership structures (1000+ entities)?
- **Historical Data**: Can the versioning system handle years of accumulated DSL documents?
- **Concurrent Access**: Are there concurrency issues with multi-user editing scenarios?

### 7. Regulatory Compliance Coverage
- **Jurisdictional Variations**: How should country-specific requirements be modeled?
- **Regulatory Updates**: Can the vocabulary system adapt to changing regulations?
- **Cross-Border Cases**: Are multi-jurisdictional scenarios properly supported?

### 8. User Experience and Adoption
- **Learning Curve**: Is the S-expression syntax too intimidating for business users?
- **Tooling**: What IDE/editor support would accelerate adoption?
- **Error Messages**: Are validation errors clear and actionable?

### 9. Integration Patterns
- **External Systems**: How should the DSL integrate with existing core banking systems?
- **Data Import/Export**: Are there standard formats for bulk data operations?
- **API Design**: Is the REST API sufficient, or do we need GraphQL/gRPC alternatives?

### 10. Security and Privacy
- **Data Classification**: How should PII/PCI/PHI data be handled in the DSL?
- **Access Control**: Should access controls be embedded in the language or handled externally?
- **Encryption**: Are there requirements for field-level encryption in DSL documents?

---

**Recommendation**: Focus review on **Type System Soundness** (#2), **Evidence and Audit Design** (#4), and **Regulatory Compliance Coverage** (#7) as these are foundational to the system's success in production financial services environments.