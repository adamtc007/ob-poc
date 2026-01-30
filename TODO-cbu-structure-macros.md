# TODO — CBU Structure Macros (Business Lexicon → DSL Expansion)

**Version**: 2.0 (AST-level expansion)  
**Status**: Draft for implementation

---

## Goal

Implement a stable set of "street-facing" structure macros (business speak) that:

1. Are discoverable via verb-first taxonomy (macro tier)
2. Expand at AST level into existing DSL primitives (execution tier)
3. Produce a deterministic CBU graph skeleton: entities + role edges + product enablement + required docs bundle
4. Integrate as a proper compiler phase: parse → **expand** → resolve → lint → topo → execute

---

## Non-goals

- Don't attempt "AI will work it out" free-form intent mapping
- Don't bake jurisdictional legal nuance into code—keep "structure" as operational onboarding templates + required parties/docs
- Don't rename DSL verb IDs (aliases allowed, canonical IDs remain stable)
- No text templating—expansion operates on typed AST nodes

---

## Architecture

### Compiler Pipeline

```
Source
   │
   ▼
┌─────────┐
│  Lex    │
└────┬────┘
     ▼
┌─────────┐
│  Parse  │  ← nom combinators, eBNF grammar
└────┬────┘
     ▼
┌─────────┐
│   AST   │  ← contains MacroInvocation nodes
└────┬────┘
     ▼
┌──────────────┐
│ Macro Expand │  ← registry lookup, AST → AST transform
└──────┬───────┘
       ▼
┌──────────────┐
│   Resolve    │  ← symbol table construction, type checking
└──────┬───────┘
       ▼
┌──────────────┐
│    Lint      │  ← role validation, cardinality, semantic checks
└──────┬───────┘
       ▼
┌──────────────┐
│  Topo Order  │  ← DAG dependency sort
└──────┬───────┘
       ▼
┌──────────────┐
│   Execute    │  ← CBU graph mutations
└──────────────┘
```

### Two-Tier Verb System

| Tier  | Purpose | Example |
|-------|---------|---------|
| **Macro** | Business-facing, discoverable, stable names | `struct.lux.ucits.sicav` |
| **DSL** | Runtime primitives, mechanical, deterministic | `cbu.ensure`, `role.attach` |

Macros expand to DSL. Only DSL executes.

### Macro Registry (YAML → Runtime)

```
┌─────────────────┐
│  YAML Registry  │  macro definitions, loaded at startup
└────────┬────────┘
         │ deserialize + validate
         ▼
┌─────────────────┐
│  MacroRegistry  │  HashMap<MacroId, MacroDefn>
└────────┬────────┘
         │ expand()
         ▼
┌─────────────────┐
│  Vec<AstNode>   │  typed DSL primitives
└─────────────────┘
```

---

## Concepts and Definitions

### CBU Graph Skeleton

A structure macro stamps a minimal graph:

- **Entities**: Fund vehicle(s), Manager/AIFM/ManCo/GP, key service providers (Depositary, Custodian, Admin, TA, Auditor), optionally Prime Broker, AP
- **Role edges**: typed relationships between entities
- **Products**: custody, fund accounting, transfer agency, collateral, etc.
- **Docs bundle**: named set of required documents as placeholders (pending)

### Symbol Scoping

Macros introduce symbols (`:as @fund @manco`). Rules:

1. **Lexical scope**: Symbols are scoped to the macro invocation that creates them
2. **Explicit export**: Composite macros (M17/M18) that invoke child macros must explicitly import/export symbols
3. **Gensym for internals**: Macro-internal temporaries get unique names to avoid collision
4. **User symbols**: Symbols in user DSL (outside macros) live in module scope

```
(struct.hedge.cross_border
  :fund_vehicle ie_icav_aif
  :manager uk_llp
  :as @cbu)              ; @cbu exported to caller

; internally expands with:
;   @__gensym_fund_42    ; internal, not visible to caller
;   @__gensym_manager_43 ; internal, not visible to caller
```

### Role Taxonomy

Canonical vocabulary for role edges:

| Role ID | Cardinality | Notes |
|---------|-------------|-------|
| `role.fund_vehicle` | 1 | |
| `role.umbrella` | 0..1 | |
| `role.subfund` | 0..n | |
| `role.management_company` | 0..1 | ManCo |
| `role.aifm` | 0..1 | |
| `role.general_partner` | 1 (for LP structures) | |
| `role.investment_manager` | 0..n | |
| `role.administrator` | 0..1 | |
| `role.transfer_agent` | 0..1 | |
| `role.fund_accountant` | 0..1 | |
| `role.depositary` | 1 (regulated funds) | |
| `role.custodian` | 1..n | |
| `role.prime_broker` | 0..n | |
| `role.authorized_participant` | 1..n (ETF only) | |
| `role.auditor` | 1 | |

Cardinality enforced at lint phase.

### Docs Bundle Versioning

Bundles have effective date ranges for auditability:

```yaml
docs.bundle.ucits_baseline:
  version: "2024-03"
  effective_from: 2024-03-01
  effective_to: null  # current
  documents:
    - prospectus
    - kiid
    - annual_report
    - ...
```

Historical onboardings reference the bundle version that applied at execution time.

---

## Deliverables

### D1) YAML Schema — Macro Registry v2

#### Macro Definition Schema

```yaml
macros:
  - id: struct.lux.ucits.sicav
    tier: macro
    display_name: "Set up Lux UCITS SICAV"
    description: "Creates Luxembourg UCITS SICAV structure with required service providers"
    
    aliases:
      - "onboard ucits"
      - "sicav setup"
      - "lux sicav"
    
    taxonomy:
      - structure
      - fund
      - lux
      - ucits
    
    args:
      - name: name
        type: string
        required: true
        description: "Fund name"
      
      - name: umbrella
        type: bool
        required: false
        default: false
        description: "Whether this is an umbrella structure"
      
      - name: subfunds
        type: list<string>
        required_if: umbrella == true
        description: "Subfund names"
      
      - name: manco
        type: entity_ref
        required: false
        placeholder_if_missing: true
        placeholder_kind: management_company
        description: "Management company reference"
      
      - name: depositary
        type: entity_ref
        required: false
        placeholder_if_missing: true
        placeholder_kind: depositary
      
      # ... additional args
    
    required_roles:
      - depositary
      - administrator
      - transfer_agent
      - auditor
    
    optional_roles:
      - management_company
      - distributor
    
    docs_bundle: docs.bundle.ucits_baseline
    
    # AST expansion template (see D2)
    expansion:
      - node: cbu.ensure
        args:
          name: "{{name}}"
          domicile: "LU"
        bind: "@cbu"
      
      - node: fund.ensure
        args:
          wrapper: "sicav"
          domicile: "LU"
          name: "{{name}}"
        bind: "@fund"
      
      - node: role.attach
        args:
          subject: "@fund"
          role: "fund_vehicle"
          object: "@cbu"
      
      # conditional expansion
      - when: "{{umbrella}}"
        nodes:
          - node: fund.umbrella.ensure
            args:
              fund: "@fund"
            bind: "@umbrella"
          
          - foreach: "{{subfunds}}"
            as: sf_name
            node: fund.subfund.ensure
            args:
              umbrella: "@umbrella"
              name: "{{sf_name}}"
      
      # placeholder-or-ref pattern
      - node: entity.ensure
        args:
          ref_or_placeholder: "{{depositary}}"
          placeholder_kind: "depositary"
        bind: "@depositary"
      
      - node: role.attach
        args:
          subject: "@fund"
          role: "depositary"
          object: "@depositary"
      
      # ... remaining role attachments
      
      - node: product.enable
        args:
          cbu: "@cbu"
          products:
            - custody
            - fund_accounting
            - transfer_agency
      
      - node: docs.bundle.apply
        args:
          cbu: "@cbu"
          bundle: "{{docs_bundle}}"
```

#### Arg Types

| Type | Rust Equivalent | Validation |
|------|-----------------|------------|
| `string` | `String` | non-empty if required |
| `bool` | `bool` | |
| `enum {a, b, c}` | `Enum` | must be one of variants |
| `entity_ref` | `SymbolRef` | must resolve to entity |
| `list<T>` | `Vec<T>` | element type validation |
| `int` | `i64` | optional min/max |

### D2) Macro Expander — Compiler Phase

#### Core Types

```rust
/// AST node variants (extend existing)
#[derive(Debug, Clone)]
pub enum AstNode {
    /// User wrote a macro invocation
    MacroInvocation {
        id: MacroId,
        args: HashMap<String, ArgValue>,
        span: Span,
    },
    
    /// DSL primitive (post-expansion, or user-written)
    DslPrimitive {
        verb: VerbId,
        args: HashMap<String, ArgValue>,
        span: Span,
        /// If this node came from macro expansion, track origin
        expanded_from: Option<MacroOrigin>,
    },
    
    // ... other variants
}

#[derive(Debug, Clone)]
pub struct MacroOrigin {
    pub macro_id: MacroId,
    pub macro_span: Span,
    pub expansion_index: usize,  // which node in expansion sequence
}

/// Argument value (typed)
#[derive(Debug, Clone)]
pub enum ArgValue {
    String(String),
    Bool(bool),
    Int(i64),
    Enum(String),
    EntityRef(SymbolRef),
    List(Vec<ArgValue>),
    Symbol(SymbolRef),
}

/// Expansion result
#[derive(Debug)]
pub enum ExpansionResult {
    /// Fully expanded
    Expanded {
        nodes: Vec<AstNode>,
        symbols_exported: HashMap<String, SymbolRef>,
    },
    
    /// Missing required arguments — return to UI for prompting
    NeedsArgs {
        missing: Vec<MissingArg>,
        /// Partial state for resumption (serializable)
        partial: PartialExpansion,
    },
    
    /// Expansion failed
    Error(ExpansionError),
}

#[derive(Debug)]
pub struct MissingArg {
    pub name: String,
    pub arg_type: ArgType,
    pub description: String,
    pub placeholder_available: bool,
}

/// Serializable partial expansion state
#[derive(Debug, Serialize, Deserialize)]
pub struct PartialExpansion {
    pub macro_id: MacroId,
    pub provided_args: HashMap<String, ArgValue>,
    pub inferred_args: HashMap<String, ArgValue>,
}
```

#### Expander Implementation

```rust
pub struct MacroExpander {
    registry: MacroRegistry,
    gensym_counter: AtomicU64,
}

impl MacroExpander {
    /// Main entry point — transform AST, expanding all macro invocations
    pub fn expand(&self, ast: Ast) -> Result<Ast, ExpansionError> {
        let mut expanded_nodes = Vec::new();
        
        for node in ast.nodes {
            match node {
                AstNode::MacroInvocation { id, args, span } => {
                    let defn = self.registry.get(&id)
                        .ok_or_else(|| ExpansionError::UnknownMacro(id.clone()))?;
                    
                    match self.expand_macro(defn, args, span)? {
                        ExpansionResult::Expanded { nodes, .. } => {
                            expanded_nodes.extend(nodes);
                        }
                        ExpansionResult::NeedsArgs { missing, partial } => {
                            return Err(ExpansionError::IncompleteArgs { missing, partial });
                        }
                        ExpansionResult::Error(e) => return Err(e),
                    }
                }
                other => expanded_nodes.push(other),
            }
        }
        
        Ok(Ast { nodes: expanded_nodes, ..ast })
    }
    
    fn expand_macro(
        &self,
        defn: &MacroDefn,
        args: HashMap<String, ArgValue>,
        span: Span,
    ) -> Result<ExpansionResult, ExpansionError> {
        // 1. Validate and collect args
        let (resolved_args, missing) = self.resolve_args(defn, args)?;
        
        if !missing.is_empty() {
            return Ok(ExpansionResult::NeedsArgs {
                missing,
                partial: PartialExpansion {
                    macro_id: defn.id.clone(),
                    provided_args: resolved_args.clone(),
                    inferred_args: HashMap::new(),
                },
            });
        }
        
        // 2. Create symbol scope for this expansion
        let mut scope = SymbolScope::new_child(self.gensym_counter.fetch_add(1, Ordering::SeqCst));
        
        // 3. Expand each template node
        let mut expanded = Vec::new();
        for (idx, template) in defn.expansion.iter().enumerate() {
            let nodes = self.expand_template_node(
                template,
                &resolved_args,
                &mut scope,
                MacroOrigin {
                    macro_id: defn.id.clone(),
                    macro_span: span.clone(),
                    expansion_index: idx,
                },
            )?;
            expanded.extend(nodes);
        }
        
        Ok(ExpansionResult::Expanded {
            nodes: expanded,
            symbols_exported: scope.exports(),
        })
    }
    
    fn expand_template_node(
        &self,
        template: &ExpansionTemplate,
        args: &HashMap<String, ArgValue>,
        scope: &mut SymbolScope,
        origin: MacroOrigin,
    ) -> Result<Vec<AstNode>, ExpansionError> {
        match template {
            ExpansionTemplate::Simple { node, args: tpl_args, bind } => {
                let resolved = self.substitute_args(tpl_args, args, scope)?;
                
                let ast_node = AstNode::DslPrimitive {
                    verb: node.clone(),
                    args: resolved,
                    span: Span::generated(origin.macro_span.clone()),
                    expanded_from: Some(origin),
                };
                
                if let Some(sym) = bind {
                    scope.bind(sym.clone(), /* result of node */);
                }
                
                Ok(vec![ast_node])
            }
            
            ExpansionTemplate::Conditional { when, nodes } => {
                let condition = self.eval_condition(when, args)?;
                if condition {
                    let mut expanded = Vec::new();
                    for (i, node) in nodes.iter().enumerate() {
                        let sub_origin = MacroOrigin {
                            expansion_index: origin.expansion_index * 1000 + i,
                            ..origin.clone()
                        };
                        expanded.extend(self.expand_template_node(node, args, scope, sub_origin)?);
                    }
                    Ok(expanded)
                } else {
                    Ok(vec![])
                }
            }
            
            ExpansionTemplate::ForEach { source, as_var, node: tpl } => {
                let list = args.get(source)
                    .ok_or_else(|| ExpansionError::MissingArg(source.clone()))?;
                
                let items = match list {
                    ArgValue::List(items) => items,
                    _ => return Err(ExpansionError::TypeMismatch {
                        expected: "list".into(),
                        got: list.type_name(),
                    }),
                };
                
                let mut expanded = Vec::new();
                for (i, item) in items.iter().enumerate() {
                    let mut loop_args = args.clone();
                    loop_args.insert(as_var.clone(), item.clone());
                    
                    let sub_origin = MacroOrigin {
                        expansion_index: origin.expansion_index * 1000 + i,
                        ..origin.clone()
                    };
                    expanded.extend(self.expand_template_node(tpl, &loop_args, scope, sub_origin)?);
                }
                Ok(expanded)
            }
            
            // Nested macro invocation (for composite macros M17/M18)
            ExpansionTemplate::InvokeMacro { macro_id, args: tpl_args, import_symbols } => {
                let resolved = self.substitute_args(tpl_args, args, scope)?;
                let defn = self.registry.get(macro_id)
                    .ok_or_else(|| ExpansionError::UnknownMacro(macro_id.clone()))?;
                
                match self.expand_macro(defn, resolved, origin.macro_span.clone())? {
                    ExpansionResult::Expanded { nodes, symbols_exported } => {
                        // Import requested symbols into current scope
                        for sym in import_symbols {
                            if let Some(resolved) = symbols_exported.get(sym) {
                                scope.bind(sym.clone(), resolved.clone());
                            }
                        }
                        Ok(nodes)
                    }
                    other => Err(ExpansionError::NestedExpansionFailed(Box::new(other))),
                }
            }
        }
    }
    
    fn gensym(&self, prefix: &str) -> String {
        let id = self.gensym_counter.fetch_add(1, Ordering::SeqCst);
        format!("@__gensym_{}_{}", prefix, id)
    }
}
```

### D3) Role Validation — Lint Phase

```rust
pub struct RoleValidator {
    role_cardinality: HashMap<RoleId, Cardinality>,
}

#[derive(Debug, Clone)]
pub enum Cardinality {
    One,              // exactly 1
    ZeroOrOne,        // 0..1
    OneOrMore,        // 1..n
    ZeroOrMore,       // 0..n
}

impl RoleValidator {
    pub fn validate(&self, ast: &Ast, ctx: &ValidationContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        
        // Collect all role attachments grouped by (subject, role)
        let mut role_counts: HashMap<(EntityRef, RoleId), usize> = HashMap::new();
        
        for node in &ast.nodes {
            if let AstNode::DslPrimitive { verb, args, span, .. } = node {
                if verb.as_str() == "role.attach" {
                    let subject = args.get("subject");
                    let role = args.get("role");
                    if let (Some(s), Some(r)) = (subject, role) {
                        let key = (s.as_entity_ref(), r.as_role_id());
                        *role_counts.entry(key).or_insert(0) += 1;
                    }
                }
            }
        }
        
        // Check cardinality constraints
        for ((entity, role), count) in &role_counts {
            if let Some(cardinality) = self.role_cardinality.get(role) {
                match cardinality {
                    Cardinality::One if *count != 1 => {
                        diagnostics.push(Diagnostic::error(
                            format!("Role '{}' requires exactly 1 attachment, found {}", role, count)
                        ));
                    }
                    Cardinality::ZeroOrOne if *count > 1 => {
                        diagnostics.push(Diagnostic::error(
                            format!("Role '{}' allows at most 1 attachment, found {}", role, count)
                        ));
                    }
                    Cardinality::OneOrMore if *count < 1 => {
                        diagnostics.push(Diagnostic::error(
                            format!("Role '{}' requires at least 1 attachment", role)
                        ));
                    }
                    _ => {}
                }
            }
        }
        
        // Check required roles from macro definitions
        for node in &ast.nodes {
            if let AstNode::DslPrimitive { expanded_from: Some(origin), .. } = node {
                if let Some(defn) = ctx.macro_registry.get(&origin.macro_id) {
                    for required_role in &defn.required_roles {
                        // Verify this role was attached somewhere in the expansion
                        let found = role_counts.keys()
                            .any(|(_, r)| r == required_role);
                        if !found {
                            diagnostics.push(Diagnostic::error(
                                format!("Macro '{}' requires role '{}' but it was not attached",
                                    origin.macro_id, required_role)
                            ).with_span(origin.macro_span.clone()));
                        }
                    }
                }
            }
        }
        
        diagnostics
    }
}
```

### D4) Docs Bundle Registry

```yaml
docs_bundles:
  - id: docs.bundle.ucits_baseline
    version: "2024-03"
    effective_from: 2024-03-01
    effective_to: null
    documents:
      - id: prospectus
        name: "Fund Prospectus"
        required: true
      - id: kiid
        name: "Key Investor Information Document"
        required: true
      - id: annual_report
        name: "Annual Report"
        required: true
      - id: semi_annual_report
        name: "Semi-Annual Report"
        required: true
      - id: articles_of_incorporation
        name: "Articles of Incorporation"
        required: true
      - id: management_regulations
        name: "Management Regulations"
        required: true

  - id: docs.bundle.aif_baseline
    version: "2024-03"
    effective_from: 2024-03-01
    documents:
      - id: private_placement_memo
        name: "Private Placement Memorandum"
        required: true
      - id: limited_partnership_agreement
        name: "Limited Partnership Agreement"
        required: true
      - id: subscription_agreement
        name: "Subscription Agreement"
        required: true
      - id: aifm_agreement
        name: "AIFM Agreement"
        required: true

  - id: docs.bundle.hedge_baseline
    version: "2024-03"
    effective_from: 2024-03-01
    extends: docs.bundle.aif_baseline
    documents:
      - id: prime_brokerage_agreement
        name: "Prime Brokerage Agreement"
        required_if: has_prime_broker
      - id: isda_master
        name: "ISDA Master Agreement"
        required: false

  - id: docs.bundle.private_equity_baseline
    version: "2024-03"
    effective_from: 2024-03-01
    documents:
      - id: lpa
        name: "Limited Partnership Agreement"
        required: true
      - id: side_letter
        name: "Side Letter Template"
        required: false
      - id: subscription_booklet
        name: "Subscription Booklet"
        required: true
      - id: capital_call_notice
        name: "Capital Call Notice Template"
        required: true
      - id: distribution_notice
        name: "Distribution Notice Template"
        required: true

  - id: docs.bundle.etf_baseline
    version: "2024-03"
    effective_from: 2024-03-01
    extends: docs.bundle.ucits_baseline
    documents:
      - id: ap_agreement
        name: "Authorized Participant Agreement"
        required: true
      - id: pcf_file_spec
        name: "Portfolio Composition File Specification"
        required: true

  - id: docs.bundle.uk_authorised_baseline
    version: "2024-03"
    effective_from: 2024-03-01
    documents:
      - id: prospectus
        name: "Fund Prospectus"
        required: true
      - id: kiid
        name: "Key Investor Information Document"
        required: true
      - id: instrument_of_incorporation
        name: "Instrument of Incorporation"
        required: true
      - id: acd_agreement
        name: "ACD Agreement"
        required: true

  - id: docs.bundle.manager_baseline
    version: "2024-03"
    effective_from: 2024-03-01
    documents:
      - id: ima
        name: "Investment Management Agreement"
        required: true
      - id: compliance_manual
        name: "Compliance Manual"
        required: false

  - id: docs.bundle.us_40act_baseline
    version: "2024-03"
    effective_from: 2024-03-01
    documents:
      - id: registration_statement
        name: "Registration Statement (N-1A/N-2)"
        required: true
      - id: sai
        name: "Statement of Additional Information"
        required: true
      - id: prospectus
        name: "Prospectus"
        required: true
      - id: advisory_agreement
        name: "Investment Advisory Agreement"
        required: true
      - id: custody_agreement
        name: "Custody Agreement"
        required: true

  - id: docs.bundle.ltaf_baseline
    version: "2024-03"
    effective_from: 2024-03-01
    extends: docs.bundle.uk_authorised_baseline
    documents:
      - id: ltaf_disclosure
        name: "LTAF Specific Disclosures"
        required: true
```

---

## Macro Definitions (M1–M18)

### Luxembourg

#### M1: struct.lux.ucits.sicav

```yaml
- id: struct.lux.ucits.sicav
  tier: macro
  display_name: "Set up Lux UCITS SICAV"
  taxonomy: [structure, fund, lux, ucits, sicav]
  
  args:
    - name: name
      type: string
      required: true
    - name: umbrella
      type: bool
      default: false
    - name: subfunds
      type: list<string>
      required_if: umbrella == true
    - name: manco
      type: entity_ref
      placeholder_if_missing: true
    - name: depositary
      type: entity_ref
      placeholder_if_missing: true
    - name: administrator
      type: entity_ref
      placeholder_if_missing: true
    - name: transfer_agent
      type: entity_ref
      placeholder_if_missing: true
    - name: auditor
      type: entity_ref
      placeholder_if_missing: true
  
  required_roles: [depositary, administrator, transfer_agent, auditor]
  optional_roles: [management_company, distributor]
  docs_bundle: docs.bundle.ucits_baseline
  
  expansion:
    - node: cbu.ensure
      args: { name: "{{name}}", domicile: "LU" }
      bind: "@cbu"
    
    - node: fund.ensure
      args: { wrapper: "sicav", domicile: "LU", name: "{{name}}" }
      bind: "@fund"
    
    - node: role.attach
      args: { subject: "@fund", role: "fund_vehicle", object: "@cbu" }
    
    - when: "{{umbrella}}"
      nodes:
        - node: fund.umbrella.ensure
          args: { fund: "@fund" }
          bind: "@umbrella"
        - foreach: "{{subfunds}}"
          as: sf_name
          node: fund.subfund.ensure
          args: { umbrella: "@umbrella", name: "{{sf_name}}" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{manco}}", kind: "management_company" }
      bind: "@manco"
    
    - when: "@manco"
      nodes:
        - node: role.attach
          args: { subject: "@cbu", role: "management_company", object: "@manco" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{depositary}}", kind: "depositary" }
      bind: "@depositary"
    
    - node: role.attach
      args: { subject: "@fund", role: "depositary", object: "@depositary" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{administrator}}", kind: "administrator" }
      bind: "@administrator"
    
    - node: role.attach
      args: { subject: "@cbu", role: "administrator", object: "@administrator" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{transfer_agent}}", kind: "transfer_agent" }
      bind: "@transfer_agent"
    
    - node: role.attach
      args: { subject: "@cbu", role: "transfer_agent", object: "@transfer_agent" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{auditor}}", kind: "auditor" }
      bind: "@auditor"
    
    - node: role.attach
      args: { subject: "@fund", role: "auditor", object: "@auditor" }
    
    - node: product.enable
      args: { cbu: "@cbu", products: [custody, fund_accounting, transfer_agency] }
    
    - node: docs.bundle.apply
      args: { cbu: "@cbu", bundle: "docs.bundle.ucits_baseline" }
```

#### M2: struct.lux.aif.raif

```yaml
- id: struct.lux.aif.raif
  tier: macro
  display_name: "Set up Lux RAIF (Reserved AIF)"
  taxonomy: [structure, fund, lux, aif, raif]
  
  args:
    - name: name
      type: string
      required: true
    - name: aifm
      type: entity_ref
      required: true
      placeholder_if_missing: true
    - name: strategy
      type: enum
      variants: [private_equity, real_assets, hedge, credit]
      default: private_equity
    - name: depositary
      type: entity_ref
      placeholder_if_missing: true
    - name: administrator
      type: entity_ref
      placeholder_if_missing: true
    - name: auditor
      type: entity_ref
      placeholder_if_missing: true
  
  required_roles: [aifm, depositary, administrator, auditor]
  docs_bundle: docs.bundle.aif_baseline
  
  expansion:
    - node: cbu.ensure
      args: { name: "{{name}}", domicile: "LU" }
      bind: "@cbu"
    
    - node: fund.ensure
      args: { wrapper: "raif", domicile: "LU", name: "{{name}}", strategy: "{{strategy}}" }
      bind: "@fund"
    
    - node: role.attach
      args: { subject: "@fund", role: "fund_vehicle", object: "@cbu" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{aifm}}", kind: "aifm" }
      bind: "@aifm"
    
    - node: role.attach
      args: { subject: "@fund", role: "aifm", object: "@aifm" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{depositary}}", kind: "depositary" }
      bind: "@depositary"
    
    - node: role.attach
      args: { subject: "@fund", role: "depositary", object: "@depositary" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{administrator}}", kind: "administrator" }
      bind: "@administrator"
    
    - node: role.attach
      args: { subject: "@cbu", role: "administrator", object: "@administrator" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{auditor}}", kind: "auditor" }
      bind: "@auditor"
    
    - node: role.attach
      args: { subject: "@fund", role: "auditor", object: "@auditor" }
    
    - node: product.enable
      args: { cbu: "@cbu", products: [custody, fund_accounting] }
    
    - node: docs.bundle.apply
      args: { cbu: "@cbu", bundle: "docs.bundle.aif_baseline" }
```

#### M3: struct.lux.pe.scsp

```yaml
- id: struct.lux.pe.scsp
  tier: macro
  display_name: "Set up Lux SCSp (Special Limited Partnership)"
  taxonomy: [structure, fund, lux, private_equity, scsp, partnership]
  
  args:
    - name: name
      type: string
      required: true
    - name: gp
      type: entity_ref
      required: true
    - name: aifm
      type: entity_ref
      placeholder_if_missing: true
    - name: administrator
      type: entity_ref
      placeholder_if_missing: true
    - name: auditor
      type: entity_ref
      placeholder_if_missing: true
  
  required_roles: [general_partner]
  optional_roles: [aifm, administrator, auditor]
  docs_bundle: docs.bundle.private_equity_baseline
  
  expansion:
    - node: cbu.ensure
      args: { name: "{{name}}", domicile: "LU" }
      bind: "@cbu"
    
    - node: fund.ensure
      args: { wrapper: "scsp", domicile: "LU", name: "{{name}}" }
      bind: "@fund"
    
    - node: role.attach
      args: { subject: "@fund", role: "fund_vehicle", object: "@cbu" }
    
    - node: entity.ensure
      args: { ref: "{{gp}}" }
      bind: "@gp"
    
    - node: role.attach
      args: { subject: "@fund", role: "general_partner", object: "@gp" }
    
    - when: "{{aifm}}"
      nodes:
        - node: entity.ensure_or_placeholder
          args: { ref: "{{aifm}}", kind: "aifm" }
          bind: "@aifm"
        - node: role.attach
          args: { subject: "@fund", role: "aifm", object: "@aifm" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{administrator}}", kind: "administrator" }
      bind: "@administrator"
    
    - node: role.attach
      args: { subject: "@cbu", role: "administrator", object: "@administrator" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{auditor}}", kind: "auditor" }
      bind: "@auditor"
    
    - node: role.attach
      args: { subject: "@fund", role: "auditor", object: "@auditor" }
    
    - node: product.enable
      args: { cbu: "@cbu", products: [custody, fund_accounting] }
    
    - node: docs.bundle.apply
      args: { cbu: "@cbu", bundle: "docs.bundle.private_equity_baseline" }
```

### Ireland

#### M4: struct.ie.ucits.icav

```yaml
- id: struct.ie.ucits.icav
  tier: macro
  display_name: "Set up Irish UCITS ICAV"
  taxonomy: [structure, fund, ie, ucits, icav]
  
  args:
    - name: name
      type: string
      required: true
    - name: umbrella
      type: bool
      default: false
    - name: subfunds
      type: list<string>
      required_if: umbrella == true
    - name: manco
      type: entity_ref
      placeholder_if_missing: true
    - name: depositary
      type: entity_ref
      placeholder_if_missing: true
    - name: administrator
      type: entity_ref
      placeholder_if_missing: true
    - name: transfer_agent
      type: entity_ref
      placeholder_if_missing: true
    - name: auditor
      type: entity_ref
      placeholder_if_missing: true
  
  required_roles: [depositary, administrator, transfer_agent, auditor]
  optional_roles: [management_company]
  docs_bundle: docs.bundle.ucits_baseline
  
  expansion:
    - node: cbu.ensure
      args: { name: "{{name}}", domicile: "IE" }
      bind: "@cbu"
    
    - node: fund.ensure
      args: { wrapper: "icav", domicile: "IE", regime: "ucits", name: "{{name}}" }
      bind: "@fund"
    
    - node: role.attach
      args: { subject: "@fund", role: "fund_vehicle", object: "@cbu" }
    
    - when: "{{umbrella}}"
      nodes:
        - node: fund.umbrella.ensure
          args: { fund: "@fund" }
          bind: "@umbrella"
        - foreach: "{{subfunds}}"
          as: sf_name
          node: fund.subfund.ensure
          args: { umbrella: "@umbrella", name: "{{sf_name}}" }
    
    # Service provider attachments (same pattern as M1)
    - node: entity.ensure_or_placeholder
      args: { ref: "{{depositary}}", kind: "depositary" }
      bind: "@depositary"
    - node: role.attach
      args: { subject: "@fund", role: "depositary", object: "@depositary" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{administrator}}", kind: "administrator" }
      bind: "@administrator"
    - node: role.attach
      args: { subject: "@cbu", role: "administrator", object: "@administrator" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{transfer_agent}}", kind: "transfer_agent" }
      bind: "@transfer_agent"
    - node: role.attach
      args: { subject: "@cbu", role: "transfer_agent", object: "@transfer_agent" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{auditor}}", kind: "auditor" }
      bind: "@auditor"
    - node: role.attach
      args: { subject: "@fund", role: "auditor", object: "@auditor" }
    
    - node: product.enable
      args: { cbu: "@cbu", products: [custody, fund_accounting, transfer_agency] }
    
    - node: docs.bundle.apply
      args: { cbu: "@cbu", bundle: "docs.bundle.ucits_baseline" }
```

#### M5: struct.ie.aif.icav

```yaml
- id: struct.ie.aif.icav
  tier: macro
  display_name: "Set up Irish AIF ICAV"
  taxonomy: [structure, fund, ie, aif, icav]
  
  args:
    - name: name
      type: string
      required: true
    - name: aif_category
      type: enum
      variants: [qiaif, riaif, other]
      default: qiaif
    - name: aifm
      type: entity_ref
      required: true
      placeholder_if_missing: true
    - name: depositary
      type: entity_ref
      placeholder_if_missing: true
    - name: administrator
      type: entity_ref
      placeholder_if_missing: true
    - name: auditor
      type: entity_ref
      placeholder_if_missing: true
  
  required_roles: [aifm, depositary, administrator, auditor]
  docs_bundle: docs.bundle.aif_baseline
  
  expansion:
    - node: cbu.ensure
      args: { name: "{{name}}", domicile: "IE" }
      bind: "@cbu"
    
    - node: fund.ensure
      args: { wrapper: "icav", domicile: "IE", regime: "aif", category: "{{aif_category}}", name: "{{name}}" }
      bind: "@fund"
    
    - node: role.attach
      args: { subject: "@fund", role: "fund_vehicle", object: "@cbu" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{aifm}}", kind: "aifm" }
      bind: "@aifm"
    - node: role.attach
      args: { subject: "@fund", role: "aifm", object: "@aifm" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{depositary}}", kind: "depositary" }
      bind: "@depositary"
    - node: role.attach
      args: { subject: "@fund", role: "depositary", object: "@depositary" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{administrator}}", kind: "administrator" }
      bind: "@administrator"
    - node: role.attach
      args: { subject: "@cbu", role: "administrator", object: "@administrator" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{auditor}}", kind: "auditor" }
      bind: "@auditor"
    - node: role.attach
      args: { subject: "@fund", role: "auditor", object: "@auditor" }
    
    - node: product.enable
      args: { cbu: "@cbu", products: [custody, fund_accounting] }
    
    - node: docs.bundle.apply
      args: { cbu: "@cbu", bundle: "docs.bundle.aif_baseline" }
```

#### M6: struct.ie.hedge.icav

```yaml
- id: struct.ie.hedge.icav
  tier: macro
  display_name: "Set up Irish Hedge Fund ICAV"
  taxonomy: [structure, fund, ie, aif, hedge, icav]
  aliases: ["irish hedge fund", "ie hedge"]
  
  args:
    - name: name
      type: string
      required: true
    - name: aifm
      type: entity_ref
      required: true
      placeholder_if_missing: true
    - name: prime_broker
      type: entity_ref
      required: false
      placeholder_if_missing: true
    - name: depositary
      type: entity_ref
      placeholder_if_missing: true
    - name: administrator
      type: entity_ref
      placeholder_if_missing: true
    - name: auditor
      type: entity_ref
      placeholder_if_missing: true
  
  required_roles: [aifm, depositary, administrator, auditor]
  optional_roles: [prime_broker]
  docs_bundle: docs.bundle.hedge_baseline
  
  expansion:
    # Delegate to M5 with hedge defaults
    - invoke_macro: struct.ie.aif.icav
      args:
        name: "{{name}}"
        aif_category: "qiaif"
        aifm: "{{aifm}}"
        depositary: "{{depositary}}"
        administrator: "{{administrator}}"
        auditor: "{{auditor}}"
      import_symbols: ["@cbu", "@fund"]
    
    # Add prime broker if provided
    - when: "{{prime_broker}}"
      nodes:
        - node: entity.ensure_or_placeholder
          args: { ref: "{{prime_broker}}", kind: "prime_broker" }
          bind: "@pb"
        - node: role.attach
          args: { subject: "@fund", role: "prime_broker", object: "@pb" }
    
    # Override docs bundle to hedge-specific
    - node: docs.bundle.apply
      args: { cbu: "@cbu", bundle: "docs.bundle.hedge_baseline" }
```

### United Kingdom

#### M7: struct.uk.authorised.oeic

```yaml
- id: struct.uk.authorised.oeic
  tier: macro
  display_name: "Set up UK Authorised OEIC"
  taxonomy: [structure, fund, uk, authorised, oeic]
  
  args:
    - name: name
      type: string
      required: true
    - name: umbrella
      type: bool
      default: false
    - name: subfunds
      type: list<string>
      required_if: umbrella == true
    - name: acd
      type: entity_ref
      required: true
      placeholder_if_missing: true
      description: "Authorised Corporate Director"
    - name: depositary
      type: entity_ref
      required: true
      placeholder_if_missing: true
    - name: administrator
      type: entity_ref
      placeholder_if_missing: true
    - name: auditor
      type: entity_ref
      placeholder_if_missing: true
  
  required_roles: [acd, depositary]
  optional_roles: [administrator, auditor]
  docs_bundle: docs.bundle.uk_authorised_baseline
  
  expansion:
    - node: cbu.ensure
      args: { name: "{{name}}", domicile: "UK" }
      bind: "@cbu"
    
    - node: fund.ensure
      args: { wrapper: "oeic", domicile: "UK", name: "{{name}}" }
      bind: "@fund"
    
    - node: role.attach
      args: { subject: "@fund", role: "fund_vehicle", object: "@cbu" }
    
    - when: "{{umbrella}}"
      nodes:
        - node: fund.umbrella.ensure
          args: { fund: "@fund" }
          bind: "@umbrella"
        - foreach: "{{subfunds}}"
          as: sf_name
          node: fund.subfund.ensure
          args: { umbrella: "@umbrella", name: "{{sf_name}}" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{acd}}", kind: "acd" }
      bind: "@acd"
    - node: role.attach
      args: { subject: "@fund", role: "management_company", object: "@acd" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{depositary}}", kind: "depositary" }
      bind: "@depositary"
    - node: role.attach
      args: { subject: "@fund", role: "depositary", object: "@depositary" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{administrator}}", kind: "administrator" }
      bind: "@administrator"
    - node: role.attach
      args: { subject: "@cbu", role: "administrator", object: "@administrator" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{auditor}}", kind: "auditor" }
      bind: "@auditor"
    - node: role.attach
      args: { subject: "@fund", role: "auditor", object: "@auditor" }
    
    - node: product.enable
      args: { cbu: "@cbu", products: [custody, fund_accounting, transfer_agency] }
    
    - node: docs.bundle.apply
      args: { cbu: "@cbu", bundle: "docs.bundle.uk_authorised_baseline" }
```

#### M8: struct.uk.authorised.aut

```yaml
- id: struct.uk.authorised.aut
  tier: macro
  display_name: "Set up UK Authorised Unit Trust"
  taxonomy: [structure, fund, uk, authorised, aut, unit_trust]
  
  args:
    - name: name
      type: string
      required: true
    - name: manager
      type: entity_ref
      required: true
      placeholder_if_missing: true
      description: "Authorised Fund Manager"
    - name: trustee
      type: entity_ref
      required: true
      placeholder_if_missing: true
      description: "Trustee (depositary equivalent)"
    - name: administrator
      type: entity_ref
      placeholder_if_missing: true
    - name: auditor
      type: entity_ref
      placeholder_if_missing: true
  
  required_roles: [management_company, depositary]
  docs_bundle: docs.bundle.uk_authorised_baseline
  
  expansion:
    - node: cbu.ensure
      args: { name: "{{name}}", domicile: "UK" }
      bind: "@cbu"
    
    - node: fund.ensure
      args: { wrapper: "aut", domicile: "UK", name: "{{name}}" }
      bind: "@fund"
    
    - node: role.attach
      args: { subject: "@fund", role: "fund_vehicle", object: "@cbu" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{manager}}", kind: "manager" }
      bind: "@manager"
    - node: role.attach
      args: { subject: "@fund", role: "management_company", object: "@manager" }
    
    # Trustee maps to depositary role
    - node: entity.ensure_or_placeholder
      args: { ref: "{{trustee}}", kind: "trustee" }
      bind: "@trustee"
    - node: role.attach
      args: { subject: "@fund", role: "depositary", object: "@trustee" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{administrator}}", kind: "administrator" }
      bind: "@administrator"
    - node: role.attach
      args: { subject: "@cbu", role: "administrator", object: "@administrator" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{auditor}}", kind: "auditor" }
      bind: "@auditor"
    - node: role.attach
      args: { subject: "@fund", role: "auditor", object: "@auditor" }
    
    - node: product.enable
      args: { cbu: "@cbu", products: [custody, fund_accounting, transfer_agency] }
    
    - node: docs.bundle.apply
      args: { cbu: "@cbu", bundle: "docs.bundle.uk_authorised_baseline" }
```

#### M9: struct.uk.authorised.acs

```yaml
- id: struct.uk.authorised.acs
  tier: macro
  display_name: "Set up UK Authorised Contractual Scheme"
  taxonomy: [structure, fund, uk, authorised, acs]
  
  args:
    - name: name
      type: string
      required: true
    - name: operator
      type: entity_ref
      required: true
      placeholder_if_missing: true
      description: "ACS Operator (AFM equivalent)"
    - name: depositary
      type: entity_ref
      required: true
      placeholder_if_missing: true
    - name: administrator
      type: entity_ref
      placeholder_if_missing: true
    - name: auditor
      type: entity_ref
      placeholder_if_missing: true
  
  required_roles: [management_company, depositary]
  docs_bundle: docs.bundle.uk_authorised_baseline
  
  expansion:
    - node: cbu.ensure
      args: { name: "{{name}}", domicile: "UK" }
      bind: "@cbu"
    
    - node: fund.ensure
      args: { wrapper: "acs", domicile: "UK", name: "{{name}}" }
      bind: "@fund"
    
    - node: role.attach
      args: { subject: "@fund", role: "fund_vehicle", object: "@cbu" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{operator}}", kind: "operator" }
      bind: "@operator"
    - node: role.attach
      args: { subject: "@fund", role: "management_company", object: "@operator" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{depositary}}", kind: "depositary" }
      bind: "@depositary"
    - node: role.attach
      args: { subject: "@fund", role: "depositary", object: "@depositary" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{administrator}}", kind: "administrator" }
      bind: "@administrator"
    - node: role.attach
      args: { subject: "@cbu", role: "administrator", object: "@administrator" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{auditor}}", kind: "auditor" }
      bind: "@auditor"
    - node: role.attach
      args: { subject: "@fund", role: "auditor", object: "@auditor" }
    
    - node: product.enable
      args: { cbu: "@cbu", products: [custody, fund_accounting] }
    
    - node: docs.bundle.apply
      args: { cbu: "@cbu", bundle: "docs.bundle.uk_authorised_baseline" }
```

#### M10: struct.uk.authorised.ltaf

```yaml
- id: struct.uk.authorised.ltaf
  tier: macro
  display_name: "Set up UK Long-Term Asset Fund (LTAF)"
  taxonomy: [structure, fund, uk, authorised, ltaf]
  
  args:
    - name: name
      type: string
      required: true
    - name: wrapper
      type: enum
      variants: [oeic, aut, acs]
      required: true
      description: "Underlying vehicle type"
    - name: acd
      type: entity_ref
      placeholder_if_missing: true
      description: "ACD/Manager/Operator depending on wrapper"
    - name: depositary
      type: entity_ref
      placeholder_if_missing: true
    - name: administrator
      type: entity_ref
      placeholder_if_missing: true
    - name: auditor
      type: entity_ref
      placeholder_if_missing: true
  
  required_roles: [management_company, depositary]
  docs_bundle: docs.bundle.ltaf_baseline
  
  expansion:
    # Invoke the appropriate underlying wrapper macro
    - when: "{{wrapper}} == 'oeic'"
      nodes:
        - invoke_macro: struct.uk.authorised.oeic
          args:
            name: "{{name}}"
            acd: "{{acd}}"
            depositary: "{{depositary}}"
            administrator: "{{administrator}}"
            auditor: "{{auditor}}"
          import_symbols: ["@cbu", "@fund"]
    
    - when: "{{wrapper}} == 'aut'"
      nodes:
        - invoke_macro: struct.uk.authorised.aut
          args:
            name: "{{name}}"
            manager: "{{acd}}"
            trustee: "{{depositary}}"
            administrator: "{{administrator}}"
            auditor: "{{auditor}}"
          import_symbols: ["@cbu", "@fund"]
    
    - when: "{{wrapper}} == 'acs'"
      nodes:
        - invoke_macro: struct.uk.authorised.acs
          args:
            name: "{{name}}"
            operator: "{{acd}}"
            depositary: "{{depositary}}"
            administrator: "{{administrator}}"
            auditor: "{{auditor}}"
          import_symbols: ["@cbu", "@fund"]
    
    # Add LTAF-specific metadata
    - node: fund.metadata.set
      args: { fund: "@fund", key: "ltaf_designation", value: true }
    
    # Override to LTAF docs bundle (extends base)
    - node: docs.bundle.apply
      args: { cbu: "@cbu", bundle: "docs.bundle.ltaf_baseline" }
```

#### M11: struct.uk.manager.llp

```yaml
- id: struct.uk.manager.llp
  tier: macro
  display_name: "Set up UK LLP Manager"
  taxonomy: [structure, manager, uk, llp]
  aliases: ["uk llp", "uk manager"]
  
  args:
    - name: name
      type: string
      required: true
    - name: roles
      type: list<enum>
      variants: [investment_manager, advisor, sponsor]
      default: [investment_manager]
    - name: regulated
      type: bool
      default: false
      description: "FCA regulated"
  
  docs_bundle: docs.bundle.manager_baseline
  
  expansion:
    - node: entity.ensure
      args: 
        name: "{{name}}"
        entity_type: "llp"
        domicile: "UK"
        regulated: "{{regulated}}"
      bind: "@manager"
    
    - node: docs.bundle.apply
      args: { entity: "@manager", bundle: "docs.bundle.manager_baseline" }
```

#### M12: struct.uk.private_equity.lp

```yaml
- id: struct.uk.private_equity.lp
  tier: macro
  display_name: "Set up UK Private Equity LP"
  taxonomy: [structure, fund, uk, private_equity, lp, partnership]
  
  args:
    - name: name
      type: string
      required: true
    - name: gp
      type: entity_ref
      required: true
    - name: manager_llp
      type: entity_ref
      required: false
      description: "UK LLP manager (can be created via struct.uk.manager.llp)"
    - name: administrator
      type: entity_ref
      placeholder_if_missing: true
    - name: auditor
      type: entity_ref
      placeholder_if_missing: true
  
  required_roles: [general_partner]
  optional_roles: [investment_manager, administrator, auditor]
  docs_bundle: docs.bundle.private_equity_baseline
  
  expansion:
    - node: cbu.ensure
      args: { name: "{{name}}", domicile: "UK" }
      bind: "@cbu"
    
    - node: fund.ensure
      args: { wrapper: "lp", domicile: "UK", name: "{{name}}" }
      bind: "@fund"
    
    - node: role.attach
      args: { subject: "@fund", role: "fund_vehicle", object: "@cbu" }
    
    - node: entity.ensure
      args: { ref: "{{gp}}" }
      bind: "@gp"
    - node: role.attach
      args: { subject: "@fund", role: "general_partner", object: "@gp" }
    
    - when: "{{manager_llp}}"
      nodes:
        - node: entity.ensure
          args: { ref: "{{manager_llp}}" }
          bind: "@im"
        - node: role.attach
          args: { subject: "@cbu", role: "investment_manager", object: "@im" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{administrator}}", kind: "administrator" }
      bind: "@administrator"
    - node: role.attach
      args: { subject: "@cbu", role: "administrator", object: "@administrator" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{auditor}}", kind: "auditor" }
      bind: "@auditor"
    - node: role.attach
      args: { subject: "@fund", role: "auditor", object: "@auditor" }
    
    - node: product.enable
      args: { cbu: "@cbu", products: [custody, fund_accounting] }
    
    - node: docs.bundle.apply
      args: { cbu: "@cbu", bundle: "docs.bundle.private_equity_baseline" }
```

### United States

#### M13: struct.us.40act.open_end

```yaml
- id: struct.us.40act.open_end
  tier: macro
  display_name: "Set up US '40 Act Open-End Fund"
  taxonomy: [structure, fund, us, 40act, open_end, mutual_fund]
  
  args:
    - name: name
      type: string
      required: true
    - name: investment_adviser
      type: entity_ref
      required: true
      placeholder_if_missing: true
    - name: custodian
      type: entity_ref
      required: true
      placeholder_if_missing: true
    - name: administrator
      type: entity_ref
      placeholder_if_missing: true
    - name: transfer_agent
      type: entity_ref
      placeholder_if_missing: true
    - name: distributor
      type: entity_ref
      placeholder_if_missing: true
    - name: auditor
      type: entity_ref
      placeholder_if_missing: true
  
  required_roles: [investment_manager, custodian]
  optional_roles: [administrator, transfer_agent, distributor, auditor]
  docs_bundle: docs.bundle.us_40act_baseline
  
  expansion:
    - node: cbu.ensure
      args: { name: "{{name}}", domicile: "US" }
      bind: "@cbu"
    
    - node: fund.ensure
      args: { wrapper: "open_end", domicile: "US", regime: "40act", name: "{{name}}" }
      bind: "@fund"
    
    - node: role.attach
      args: { subject: "@fund", role: "fund_vehicle", object: "@cbu" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{investment_adviser}}", kind: "investment_adviser" }
      bind: "@ia"
    - node: role.attach
      args: { subject: "@fund", role: "investment_manager", object: "@ia" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{custodian}}", kind: "custodian" }
      bind: "@custodian"
    - node: role.attach
      args: { subject: "@fund", role: "custodian", object: "@custodian" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{administrator}}", kind: "administrator" }
      bind: "@administrator"
    - node: role.attach
      args: { subject: "@cbu", role: "administrator", object: "@administrator" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{transfer_agent}}", kind: "transfer_agent" }
      bind: "@transfer_agent"
    - node: role.attach
      args: { subject: "@cbu", role: "transfer_agent", object: "@transfer_agent" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{auditor}}", kind: "auditor" }
      bind: "@auditor"
    - node: role.attach
      args: { subject: "@fund", role: "auditor", object: "@auditor" }
    
    - node: product.enable
      args: { cbu: "@cbu", products: [custody, fund_accounting, transfer_agency] }
    
    - node: docs.bundle.apply
      args: { cbu: "@cbu", bundle: "docs.bundle.us_40act_baseline" }
```

#### M14: struct.us.40act.closed_end

```yaml
- id: struct.us.40act.closed_end
  tier: macro
  display_name: "Set up US '40 Act Closed-End Fund"
  taxonomy: [structure, fund, us, 40act, closed_end]
  
  args:
    # Same as M13
    - name: name
      type: string
      required: true
    - name: investment_adviser
      type: entity_ref
      required: true
      placeholder_if_missing: true
    - name: custodian
      type: entity_ref
      required: true
      placeholder_if_missing: true
    - name: administrator
      type: entity_ref
      placeholder_if_missing: true
    - name: transfer_agent
      type: entity_ref
      placeholder_if_missing: true
    - name: auditor
      type: entity_ref
      placeholder_if_missing: true
  
  required_roles: [investment_manager, custodian]
  docs_bundle: docs.bundle.us_40act_baseline
  
  expansion:
    - node: cbu.ensure
      args: { name: "{{name}}", domicile: "US" }
      bind: "@cbu"
    
    - node: fund.ensure
      args: { wrapper: "closed_end", domicile: "US", regime: "40act", name: "{{name}}" }
      bind: "@fund"
    
    - node: role.attach
      args: { subject: "@fund", role: "fund_vehicle", object: "@cbu" }
    
    # Same service provider pattern as M13
    - node: entity.ensure_or_placeholder
      args: { ref: "{{investment_adviser}}", kind: "investment_adviser" }
      bind: "@ia"
    - node: role.attach
      args: { subject: "@fund", role: "investment_manager", object: "@ia" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{custodian}}", kind: "custodian" }
      bind: "@custodian"
    - node: role.attach
      args: { subject: "@fund", role: "custodian", object: "@custodian" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{administrator}}", kind: "administrator" }
      bind: "@administrator"
    - node: role.attach
      args: { subject: "@cbu", role: "administrator", object: "@administrator" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{transfer_agent}}", kind: "transfer_agent" }
      bind: "@transfer_agent"
    - node: role.attach
      args: { subject: "@cbu", role: "transfer_agent", object: "@transfer_agent" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{auditor}}", kind: "auditor" }
      bind: "@auditor"
    - node: role.attach
      args: { subject: "@fund", role: "auditor", object: "@auditor" }
    
    - node: product.enable
      args: { cbu: "@cbu", products: [custody, fund_accounting, transfer_agency] }
    
    - node: docs.bundle.apply
      args: { cbu: "@cbu", bundle: "docs.bundle.us_40act_baseline" }
```

#### M15: struct.us.etf.40act

```yaml
- id: struct.us.etf.40act
  tier: macro
  display_name: "Set up US '40 Act ETF"
  taxonomy: [structure, fund, us, 40act, etf]
  aliases: ["us etf", "40 act etf"]
  
  args:
    - name: name
      type: string
      required: true
    - name: investment_adviser
      type: entity_ref
      required: true
      placeholder_if_missing: true
    - name: custodian
      type: entity_ref
      required: true
      placeholder_if_missing: true
    - name: authorized_participants
      type: list<entity_ref>
      required: true
      placeholder_if_missing: true
      min_length: 1
    - name: administrator
      type: entity_ref
      placeholder_if_missing: true
    - name: transfer_agent
      type: entity_ref
      placeholder_if_missing: true
    - name: auditor
      type: entity_ref
      placeholder_if_missing: true
  
  required_roles: [investment_manager, custodian, authorized_participant]
  docs_bundle: docs.bundle.etf_baseline
  
  expansion:
    # Delegate base structure to open-end
    - invoke_macro: struct.us.40act.open_end
      args:
        name: "{{name}}"
        investment_adviser: "{{investment_adviser}}"
        custodian: "{{custodian}}"
        administrator: "{{administrator}}"
        transfer_agent: "{{transfer_agent}}"
        auditor: "{{auditor}}"
      import_symbols: ["@cbu", "@fund"]
    
    # Add ETF-specific metadata
    - node: fund.metadata.set
      args: { fund: "@fund", key: "etf_designation", value: true }
    
    # Add authorized participants
    - foreach: "{{authorized_participants}}"
      as: ap_ref
      nodes:
        - node: entity.ensure_or_placeholder
          args: { ref: "{{ap_ref}}", kind: "authorized_participant" }
          bind: "@ap"
        - node: role.attach
          args: { subject: "@fund", role: "authorized_participant", object: "@ap" }
    
    # Override to ETF docs bundle
    - node: docs.bundle.apply
      args: { cbu: "@cbu", bundle: "docs.bundle.etf_baseline" }
```

#### M16: struct.us.private_fund.delaware_lp

```yaml
- id: struct.us.private_fund.delaware_lp
  tier: macro
  display_name: "Set up US Delaware LP (Private Fund)"
  taxonomy: [structure, fund, us, private_fund, delaware, lp, partnership]
  aliases: ["delaware lp", "us private fund"]
  
  args:
    - name: name
      type: string
      required: true
    - name: gp
      type: entity_ref
      required: true
    - name: investment_manager
      type: entity_ref
      required: false
      description: "Investment manager (can be UK LLP, US adviser, etc.)"
    - name: strategy
      type: enum
      variants: [private_equity, hedge, credit, real_assets]
      required: true
    - name: administrator
      type: entity_ref
      placeholder_if_missing: true
    - name: custodian
      type: entity_ref
      placeholder_if_missing: true
    - name: prime_broker
      type: entity_ref
      required_if: strategy == "hedge"
      placeholder_if_missing: true
    - name: auditor
      type: entity_ref
      placeholder_if_missing: true
  
  required_roles: [general_partner]
  optional_roles: [investment_manager, administrator, custodian, prime_broker, auditor]
  docs_bundle: docs.bundle.private_equity_baseline  # overridden for hedge
  
  expansion:
    - node: cbu.ensure
      args: { name: "{{name}}", domicile: "US" }
      bind: "@cbu"
    
    - node: fund.ensure
      args: { wrapper: "delaware_lp", domicile: "US", strategy: "{{strategy}}", name: "{{name}}" }
      bind: "@fund"
    
    - node: role.attach
      args: { subject: "@fund", role: "fund_vehicle", object: "@cbu" }
    
    - node: entity.ensure
      args: { ref: "{{gp}}" }
      bind: "@gp"
    - node: role.attach
      args: { subject: "@fund", role: "general_partner", object: "@gp" }
    
    - when: "{{investment_manager}}"
      nodes:
        - node: entity.ensure
          args: { ref: "{{investment_manager}}" }
          bind: "@im"
        - node: role.attach
          args: { subject: "@cbu", role: "investment_manager", object: "@im" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{administrator}}", kind: "administrator" }
      bind: "@administrator"
    - node: role.attach
      args: { subject: "@cbu", role: "administrator", object: "@administrator" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{custodian}}", kind: "custodian" }
      bind: "@custodian"
    - node: role.attach
      args: { subject: "@fund", role: "custodian", object: "@custodian" }
    
    # Prime broker for hedge strategy
    - when: "{{strategy}} == 'hedge'"
      nodes:
        - node: entity.ensure_or_placeholder
          args: { ref: "{{prime_broker}}", kind: "prime_broker" }
          bind: "@pb"
        - node: role.attach
          args: { subject: "@fund", role: "prime_broker", object: "@pb" }
    
    - node: entity.ensure_or_placeholder
      args: { ref: "{{auditor}}", kind: "auditor" }
      bind: "@auditor"
    - node: role.attach
      args: { subject: "@fund", role: "auditor", object: "@auditor" }
    
    - node: product.enable
      args: { cbu: "@cbu", products: [custody, fund_accounting] }
    
    # Docs bundle depends on strategy
    - when: "{{strategy}} == 'hedge'"
      nodes:
        - node: docs.bundle.apply
          args: { cbu: "@cbu", bundle: "docs.bundle.hedge_baseline" }
    
    - when: "{{strategy}} != 'hedge'"
      nodes:
        - node: docs.bundle.apply
          args: { cbu: "@cbu", bundle: "docs.bundle.private_equity_baseline" }
```

### Cross-Border Composite Macros

#### M17: struct.hedge.cross_border

```yaml
- id: struct.hedge.cross_border
  tier: macro
  display_name: "Set up Cross-Border Hedge Fund Structure"
  taxonomy: [structure, fund, hedge, cross_border]
  aliases: ["cross border hedge", "offshore hedge fund"]
  description: "UK/US manager + IE/Lux/US fund vehicle"
  
  args:
    - name: name
      type: string
      required: true
    - name: fund_vehicle
      type: enum
      variants: [ie_icav_aif, lux_raif, us_private_lp]
      required: true
    - name: manager
      type: enum
      variants: [uk_llp, us_adviser]
      required: true
    - name: manager_name
      type: string
      required: true
    - name: manager_regulated
      type: bool
      default: true
    - name: prime_broker
      type: entity_ref
      placeholder_if_missing: true
    - name: depositary
      type: entity_ref
      placeholder_if_missing: true
    - name: administrator
      type: entity_ref
      placeholder_if_missing: true
    - name: auditor
      type: entity_ref
      placeholder_if_missing: true
  
  required_roles: [investment_manager]
  optional_roles: [prime_broker, depositary, administrator, auditor]
  docs_bundle: docs.bundle.hedge_baseline
  
  expansion:
    # Create manager entity first
    - when: "{{manager}} == 'uk_llp'"
      nodes:
        - invoke_macro: struct.uk.manager.llp
          args:
            name: "{{manager_name}}"
            roles: [investment_manager]
            regulated: "{{manager_regulated}}"
          import_symbols: ["@manager"]
    
    - when: "{{manager}} == 'us_adviser'"
      nodes:
        - node: entity.ensure
          args:
            name: "{{manager_name}}"
            entity_type: "investment_adviser"
            domicile: "US"
            regulated: "{{manager_regulated}}"
          bind: "@manager"
    
    # Create fund vehicle based on selection
    - when: "{{fund_vehicle}} == 'ie_icav_aif'"
      nodes:
        - invoke_macro: struct.ie.hedge.icav
          args:
            name: "{{name}}"
            aifm: "@manager"  # manager can act as AIFM or delegate
            prime_broker: "{{prime_broker}}"
            depositary: "{{depositary}}"
            administrator: "{{administrator}}"
            auditor: "{{auditor}}"
          import_symbols: ["@cbu", "@fund"]
    
    - when: "{{fund_vehicle}} == 'lux_raif'"
      nodes:
        - invoke_macro: struct.lux.aif.raif
          args:
            name: "{{name}}"
            aifm: "@manager"
            strategy: "hedge"
            depositary: "{{depositary}}"
            administrator: "{{administrator}}"
            auditor: "{{auditor}}"
          import_symbols: ["@cbu", "@fund"]
        # Add prime broker separately for RAIF
        - when: "{{prime_broker}}"
          nodes:
            - node: entity.ensure_or_placeholder
              args: { ref: "{{prime_broker}}", kind: "prime_broker" }
              bind: "@pb"
            - node: role.attach
              args: { subject: "@fund", role: "prime_broker", object: "@pb" }
    
    - when: "{{fund_vehicle}} == 'us_private_lp'"
      nodes:
        # For US private LP, need a GP (can be related to manager)
        - node: entity.ensure
          args:
            name: "{{name}} GP LLC"
            entity_type: "llc"
            domicile: "US"
          bind: "@gp"
        - invoke_macro: struct.us.private_fund.delaware_lp
          args:
            name: "{{name}}"
            gp: "@gp"
            investment_manager: "@manager"
            strategy: "hedge"
            administrator: "{{administrator}}"
            custodian: "{{depositary}}"  # custodian for US
            prime_broker: "{{prime_broker}}"
            auditor: "{{auditor}}"
          import_symbols: ["@cbu", "@fund"]
    
    # Ensure IM role is attached (may already be done by child macro)
    - node: role.attach
      args: { subject: "@cbu", role: "investment_manager", object: "@manager" }
      # idempotent - won't duplicate if already present
    
    - node: docs.bundle.apply
      args: { cbu: "@cbu", bundle: "docs.bundle.hedge_baseline" }
```

#### M18: struct.pe.cross_border

```yaml
- id: struct.pe.cross_border
  tier: macro
  display_name: "Set up Cross-Border Private Equity Structure"
  taxonomy: [structure, fund, private_equity, cross_border]
  aliases: ["cross border pe", "offshore pe fund"]
  description: "UK/US manager + Lux/IE/US/UK fund vehicle"
  
  args:
    - name: name
      type: string
      required: true
    - name: fund_vehicle
      type: enum
      variants: [lux_scsp, ie_icav_aif, us_delaware_lp, uk_lp]
      required: true
    - name: manager
      type: enum
      variants: [uk_llp, us_adviser]
      required: true
    - name: manager_name
      type: string
      required: true
    - name: manager_regulated
      type: bool
      default: false
    - name: gp_name
      type: string
      required: true
    - name: administrator
      type: entity_ref
      placeholder_if_missing: true
    - name: auditor
      type: entity_ref
      placeholder_if_missing: true
  
  required_roles: [general_partner, investment_manager]
  optional_roles: [administrator, auditor]
  docs_bundle: docs.bundle.private_equity_baseline
  
  expansion:
    # Create manager entity
    - when: "{{manager}} == 'uk_llp'"
      nodes:
        - invoke_macro: struct.uk.manager.llp
          args:
            name: "{{manager_name}}"
            roles: [investment_manager, advisor]
            regulated: "{{manager_regulated}}"
          import_symbols: ["@manager"]
    
    - when: "{{manager}} == 'us_adviser'"
      nodes:
        - node: entity.ensure
          args:
            name: "{{manager_name}}"
            entity_type: "investment_adviser"
            domicile: "US"
            regulated: "{{manager_regulated}}"
          bind: "@manager"
    
    # Create GP entity (separate from manager for PE)
    - node: entity.ensure
      args:
        name: "{{gp_name}}"
        entity_type: "gp_entity"
      bind: "@gp"
    
    # Create fund vehicle
    - when: "{{fund_vehicle}} == 'lux_scsp'"
      nodes:
        - invoke_macro: struct.lux.pe.scsp
          args:
            name: "{{name}}"
            gp: "@gp"
            aifm: "@manager"
            administrator: "{{administrator}}"
            auditor: "{{auditor}}"
          import_symbols: ["@cbu", "@fund"]
    
    - when: "{{fund_vehicle}} == 'ie_icav_aif'"
      nodes:
        - invoke_macro: struct.ie.aif.icav
          args:
            name: "{{name}}"
            aif_category: "qiaif"
            aifm: "@manager"
            depositary: null  # placeholder
            administrator: "{{administrator}}"
            auditor: "{{auditor}}"
          import_symbols: ["@cbu", "@fund"]
        # Attach GP role separately (ICAV doesn't have native GP)
        - node: role.attach
          args: { subject: "@fund", role: "general_partner", object: "@gp" }
    
    - when: "{{fund_vehicle}} == 'us_delaware_lp'"
      nodes:
        - invoke_macro: struct.us.private_fund.delaware_lp
          args:
            name: "{{name}}"
            gp: "@gp"
            investment_manager: "@manager"
            strategy: "private_equity"
            administrator: "{{administrator}}"
            auditor: "{{auditor}}"
          import_symbols: ["@cbu", "@fund"]
    
    - when: "{{fund_vehicle}} == 'uk_lp'"
      nodes:
        - invoke_macro: struct.uk.private_equity.lp
          args:
            name: "{{name}}"
            gp: "@gp"
            manager_llp: "@manager"
            administrator: "{{administrator}}"
            auditor: "{{auditor}}"
          import_symbols: ["@cbu", "@fund"]
    
    # Ensure IM role attached
    - node: role.attach
      args: { subject: "@cbu", role: "investment_manager", object: "@manager" }
    
    - node: docs.bundle.apply
      args: { cbu: "@cbu", bundle: "docs.bundle.private_equity_baseline" }
```

---

## Implementation Plan

### Phase 1: Registry + Parsing (Week 1)

**Tasks:**

1. **YAML schema definition** (`macro_registry_schema.yaml`)
   - JSON Schema or similar for validation
   - Arg type definitions
   - Expansion template schema

2. **Registry loader** (`macro_registry.rs`)
   ```rust
   pub struct MacroRegistry {
       macros: HashMap<MacroId, MacroDefn>,
       aliases: HashMap<String, MacroId>,
       taxonomy_index: TaxonomyIndex,
   }
   
   impl MacroRegistry {
       pub fn load(path: &Path) -> Result<Self, RegistryError>;
       pub fn resolve_alias(&self, alias: &str) -> Option<&MacroId>;
       pub fn get(&self, id: &MacroId) -> Option<&MacroDefn>;
       pub fn search_taxonomy(&self, tags: &[&str]) -> Vec<&MacroDefn>;
   }
   ```

3. **AST extension** — Add `MacroInvocation` variant

4. **Parser extension** — Recognize macro invocations in source

**Acceptance criteria:**
- Can load YAML registry, validate schema
- Alias resolution works
- Taxonomy search returns expected macros

### Phase 2: Macro Expander (Week 2)

**Tasks:**

1. **Expander core** (`macro_expander.rs`)
   - `expand()` method as specified above
   - Symbol scope management
   - Gensym generation

2. **Template expansion**
   - Simple node expansion
   - Conditional (`when:`) expansion
   - Loop (`foreach:`) expansion
   - Nested macro (`invoke_macro:`) expansion

3. **Arg resolution**
   - Type checking
   - Required arg validation
   - `placeholder_if_missing` handling
   - `required_if` conditional requirements

4. **Error types**
   ```rust
   pub enum ExpansionError {
       UnknownMacro(MacroId),
       MissingRequiredArg { name: String, macro_id: MacroId },
       TypeMismatch { arg: String, expected: ArgType, got: ArgType },
       ConditionalRequirementFailed { arg: String, condition: String },
       NestedExpansionFailed { macro_id: MacroId, inner: Box<ExpansionError> },
       SymbolNotFound(String),
   }
   ```

**Acceptance criteria:**
- Given macro invocation with all args, produces correct DSL nodes
- Missing args returns structured `NeedsArgs` result
- Nested macro invocation works (for M6, M10, M15, M17, M18)

### Phase 3: Role + Docs Validation (Week 3)

**Tasks:**

1. **Role cardinality registry**
   - Load from YAML or embed
   - Validation logic

2. **Lint phase integration**
   - Check expanded AST for role coverage
   - Verify required roles from macro definitions
   - Cardinality validation

3. **Docs bundle registry**
   - Load bundles with versioning
   - `extends` inheritance
   - `required_if` conditional docs

4. **Diagnostics**
   - Span tracking through expansion
   - Clear error messages referencing macro source

**Acceptance criteria:**
- Expansion with missing required role fails lint
- Cardinality violations caught
- Docs bundle applied correctly

### Phase 4: Implement Macros M1–M18 (Week 4)

**Tasks:**

1. **Luxembourg macros** (M1–M3)
2. **Ireland macros** (M4–M6)
3. **UK macros** (M7–M12)
4. **US macros** (M13–M16)
5. **Cross-border composites** (M17–M18)

**Acceptance criteria:**
- All macros compile
- All macros expand to valid DSL
- All macros pass lint
- All macros produce expected CBU graph

### Phase 5: Tests + Integration (Week 5)

**Tasks:**

1. **Unit tests**
   - Alias resolution
   - Arg validation (types, required, conditional)
   - Expansion determinism (stable output)
   - Symbol scoping (no collisions)
   - Umbrella/subfund generation

2. **Integration tests**
   - Full pipeline: parse → expand → resolve → lint → topo → execute
   - Idempotency: run twice, no duplicates
   - Cross-border macro composition

3. **Demo scripts** (acceptance tests)
   - "Set up Lux UCITS SICAV umbrella with 2 subfunds"
   - "Set up Irish ICAV AIF (QIAIF) hedge with prime broker placeholder"
   - "Set up US ETF '40 Act with 2 authorized participants"
   - "Set up UK LLP manager + US Delaware LP PE fund; link IM + GP roles"

**Acceptance criteria:**
- All tests pass
- Demo scripts execute successfully
- Audit trail captures macro invocations + expansions

---

## LSP Integration

### Completion

```rust
// On typing "struct."
fn completions_for_prefix(prefix: &str, registry: &MacroRegistry) -> Vec<CompletionItem> {
    registry.macros.values()
        .filter(|m| m.id.starts_with(prefix))
        .map(|m| CompletionItem {
            label: m.display_name.clone(),
            insert_text: m.id.clone(),
            detail: Some(m.description.clone()),
            kind: CompletionItemKind::Function,
        })
        .collect()
}
```

### Hover

```rust
// On hovering over macro invocation
fn hover_for_macro(id: &MacroId, registry: &MacroRegistry) -> Option<Hover> {
    let defn = registry.get(id)?;
    Some(Hover {
        contents: format!(
            "**{}**\n\n{}\n\n**Args:**\n{}\n\n**Required roles:** {}\n\n**Docs bundle:** {}",
            defn.display_name,
            defn.description,
            defn.args.iter().map(|a| format!("- `{}`: {} {}", a.name, a.arg_type, if a.required { "(required)" } else { "" })).collect::<Vec<_>>().join("\n"),
            defn.required_roles.join(", "),
            defn.docs_bundle,
        ),
    })
}
```

### Diagnostics

```rust
// Missing required arg
Diagnostic {
    range: macro_span,
    severity: Error,
    message: format!("Macro '{}' requires argument ':{}' ({})", macro_id, arg.name, arg.description),
    related_information: Some(vec![
        DiagnosticRelatedInformation {
            location: macro_definition_location,
            message: "Macro defined here".into(),
        }
    ]),
}
```

---

## Storage / Audit

### Schema additions

```sql
CREATE TABLE macro_invocations (
    id UUID PRIMARY KEY,
    cbu_id UUID REFERENCES cbus(id),
    macro_id TEXT NOT NULL,
    macro_version TEXT NOT NULL,
    args JSONB NOT NULL,
    expanded_dsl TEXT NOT NULL,  -- or hash + reference
    expanded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    executed_at TIMESTAMPTZ,
    execution_status TEXT  -- 'pending', 'success', 'failed'
);

CREATE INDEX idx_macro_invocations_cbu ON macro_invocations(cbu_id);
CREATE INDEX idx_macro_invocations_macro ON macro_invocations(macro_id);
```

### Audit trail

On macro expansion:
1. Store original invocation (macro_id, args)
2. Store expanded DSL (full text or content-addressed hash)
3. Store macro registry version

On execution:
1. Update execution_status
2. Link to CBU/entities created

This enables runbook replay: given a macro invocation record, re-expand with same registry version to reproduce identical DSL.

---

## Clarifications

### Placeholder Entity Lifecycle

When an optional argument like `:depositary` is missing and `placeholder_if_missing: true` is set, the expander creates a placeholder entity. These placeholders require explicit lifecycle management.

#### Placeholder States

```
┌─────────────┐     resolve()     ┌────────────┐     verify()     ┌──────────┐
│  PENDING    │ ─────────────────▶│  RESOLVED  │ ────────────────▶│ VERIFIED │
└─────────────┘                   └────────────┘                  └──────────┘
       │                                │
       │ expire()                       │ reject()
       ▼                                ▼
┌─────────────┐                   ┌────────────┐
│   EXPIRED   │                   │  REJECTED  │
└─────────────┘                   └────────────┘
```

#### Schema

```sql
CREATE TABLE placeholder_entities (
    id UUID PRIMARY KEY,
    cbu_id UUID REFERENCES cbus(id),
    placeholder_kind TEXT NOT NULL,  -- 'depositary', 'administrator', etc.
    display_name TEXT NOT NULL,      -- 'TBD Depositary'
    state TEXT NOT NULL DEFAULT 'pending',
    resolved_entity_id UUID REFERENCES entities(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,          -- optional deadline
    created_by_macro TEXT,           -- macro_id that created it
    CONSTRAINT valid_state CHECK (state IN ('pending', 'resolved', 'verified', 'expired', 'rejected'))
);

CREATE INDEX idx_placeholders_cbu ON placeholder_entities(cbu_id);
CREATE INDEX idx_placeholders_state ON placeholder_entities(state) WHERE state = 'pending';
```

#### Resolution Workflow

```rust
/// Resolve a placeholder to a real entity
pub fn resolve_placeholder(
    placeholder_id: Uuid,
    entity_id: Uuid,
    ctx: &mut DbContext,
) -> Result<(), PlaceholderError> {
    let placeholder = ctx.get_placeholder(placeholder_id)?;
    
    // Validate entity matches expected kind
    let entity = ctx.get_entity(entity_id)?;
    if !entity.matches_kind(&placeholder.placeholder_kind) {
        return Err(PlaceholderError::KindMismatch {
            expected: placeholder.placeholder_kind,
            got: entity.entity_type,
        });
    }
    
    // Update placeholder state
    ctx.update_placeholder(placeholder_id, |p| {
        p.state = PlaceholderState::Resolved;
        p.resolved_entity_id = Some(entity_id);
        p.resolved_at = Some(Utc::now());
    })?;
    
    // Update all role edges pointing to placeholder
    ctx.repoint_role_edges(placeholder_id, entity_id)?;
    
    Ok(())
}
```

#### Execution Blocking

Placeholders **do not block macro expansion or DSL execution**. They block downstream operations:

| Operation | Placeholder Behavior |
|-----------|---------------------|
| Macro expansion | Creates placeholder, continues |
| Role attachment | Attaches to placeholder entity |
| Product enablement | Proceeds (placeholder is valid target) |
| Document generation | Proceeds with "TBD" values |
| **Account opening** | **BLOCKED** — requires all service providers resolved |
| **Transaction processing** | **BLOCKED** — requires custodian resolved |
| **Regulatory filing** | **BLOCKED** — requires all required roles resolved |

#### UI Integration

The UI should:
1. Display pending placeholders on CBU dashboard with visual indicator
2. Provide "Resolve Placeholder" action → entity search/create flow
3. Show expiration warnings (if `expires_at` set)
4. Block progression to "Go Live" if required placeholders unresolved

```typescript
// Example UI query
const pendingPlaceholders = await api.getPlaceholders({
  cbuId: cbu.id,
  state: 'pending',
});

// Render as action items
pendingPlaceholders.map(p => ({
  title: `Assign ${p.placeholder_kind}`,
  description: `Required for ${p.created_by_macro}`,
  action: () => openEntityPicker(p.id, p.placeholder_kind),
  urgent: p.expires_at && isWithinDays(p.expires_at, 7),
}));
```

---

### Macro Versioning and Migration

#### Version Identification

Each macro definition carries a semantic version:

```yaml
- id: struct.lux.ucits.sicav
  version: "1.2.0"  # semver
  min_compatible_version: "1.0.0"  # oldest version that produces compatible output
```

The registry tracks version history:

```rust
pub struct MacroRegistry {
    current: HashMap<MacroId, MacroDefn>,
    history: HashMap<MacroId, Vec<(SemVer, MacroDefn)>>,
}

impl MacroRegistry {
    /// Get macro at specific version (for replay)
    pub fn get_versioned(&self, id: &MacroId, version: &SemVer) -> Option<&MacroDefn>;
    
    /// Get current version
    pub fn get(&self, id: &MacroId) -> Option<&MacroDefn>;
}
```

#### Invocation Storage

Every macro invocation stores the version used:

```sql
CREATE TABLE macro_invocations (
    -- ... existing columns ...
    macro_version TEXT NOT NULL,           -- version at expansion time
    registry_checksum TEXT NOT NULL,       -- hash of full registry for reproducibility
);
```

#### Migration Strategy

**Principle**: CBUs are **frozen to the macro version at creation**. Migrations are explicit operations.

```
┌──────────────────────────────────────────────────────────────────┐
│                     Macro Version Lifecycle                       │
├──────────────────────────────────────────────────────────────────┤
│  v1.0.0                v1.1.0                v2.0.0              │
│    │                     │                     │                 │
│    │  (compatible)       │  (breaking)         │                 │
│    ├─────────────────────┤                     │                 │
│    │                     │                     │                 │
│  CBU-A created         CBU-B created        CBU-C created       │
│  (stays v1.0.0)        (uses v1.1.0)        (uses v2.0.0)       │
│                                                                  │
│  Migration available: v1.x → v2.0.0                             │
└──────────────────────────────────────────────────────────────────┘
```

**Migration types:**

| Change Type | Example | Migration Required |
|-------------|---------|-------------------|
| Additive (new optional arg) | Add `:distributor` arg | No — old CBUs still valid |
| Default change | `:umbrella` default false→true | No — old CBUs explicit |
| New required role | Add `compliance_officer` | Yes — old CBUs need update |
| Structural change | Subfund schema redesign | Yes — requires transformation |

**Migration definition:**

```yaml
migrations:
  - from_version: "1.x"
    to_version: "2.0.0"
    macro_id: struct.lux.ucits.sicav
    description: "Add compliance officer role requirement"
    
    # Transformation rules
    transforms:
      - type: add_role
        role: compliance_officer
        target: "@cbu"
        placeholder_if_missing: true
      
      - type: add_doc
        doc_id: compliance_manual
        bundle: docs.bundle.ucits_baseline
    
    # Validation after migration
    post_conditions:
      - role_exists: [compliance_officer]
      - doc_exists: [compliance_manual]
```

**Migration execution:**

```rust
pub fn migrate_cbu(
    cbu_id: Uuid,
    target_version: SemVer,
    ctx: &mut DbContext,
    registry: &MacroRegistry,
) -> Result<MigrationResult, MigrationError> {
    let invocation = ctx.get_macro_invocation(cbu_id)?;
    let current_version = invocation.macro_version.parse::<SemVer>()?;
    
    // Find migration path
    let migrations = registry.find_migration_path(
        &invocation.macro_id,
        &current_version,
        &target_version,
    )?;
    
    // Execute in transaction
    ctx.transaction(|tx| {
        for migration in migrations {
            for transform in &migration.transforms {
                transform.apply(cbu_id, tx)?;
            }
            
            // Validate post-conditions
            for condition in &migration.post_conditions {
                condition.validate(cbu_id, tx)?;
            }
        }
        
        // Update stored version
        tx.update_macro_invocation(cbu_id, |inv| {
            inv.macro_version = target_version.to_string();
        })?;
        
        Ok(MigrationResult::Success { 
            from: current_version, 
            to: target_version,
            transforms_applied: migrations.len(),
        })
    })
}
```

**UI for migrations:**

When a user opens a CBU created with an old macro version:
1. Show banner: "This structure was created with v1.0.0. Current version is v2.0.0."
2. Offer "Review Changes" → diff view of what migration would do
3. Offer "Migrate" → runs migration with confirmation
4. Offer "Keep Current" → dismisses (can always migrate later)

---

### Docs Bundle Inheritance

#### Merge Semantics

When bundle A `extends` bundle B:

```yaml
# Bundle B (parent)
docs.bundle.aif_baseline:
  version: "2024-03"
  documents:
    - id: ppm
      name: "Private Placement Memorandum"
      required: true
    - id: lpa
      name: "Limited Partnership Agreement"
      required: true
    - id: subscription
      name: "Subscription Agreement"
      required: true

# Bundle A (child) extends B
docs.bundle.hedge_baseline:
  version: "2024-03"
  extends: docs.bundle.aif_baseline
  documents:
    - id: pba
      name: "Prime Brokerage Agreement"
      required_if: has_prime_broker
    - id: isda
      name: "ISDA Master Agreement"
      required: false
    # Override parent doc
    - id: ppm
      name: "Private Placement Memorandum (Hedge)"
      required: true
      template: hedge_ppm_template  # adds field not in parent
```

**Merge rules (explicit):**

1. **Document list**: Union of parent + child documents
2. **Override by `id`**: If child defines doc with same `id` as parent, child wins entirely (no field-level merge)
3. **`required` field**: Child can override parent's `required` status
4. **`required_if` conditions**: Evaluated against expansion context at apply time
5. **No deep nesting**: `extends` is single-level only (no `extends` of `extends`)

```rust
impl DocsBundle {
    pub fn resolve(&self, registry: &DocsBundleRegistry) -> ResolvedBundle {
        let mut documents = HashMap::new();
        
        // Load parent first (if extends)
        if let Some(parent_id) = &self.extends {
            let parent = registry.get(parent_id)
                .expect("parent bundle must exist");
            let resolved_parent = parent.resolve(registry);
            documents.extend(resolved_parent.documents);
        }
        
        // Child overrides parent (by id)
        for doc in &self.documents {
            documents.insert(doc.id.clone(), doc.clone());
        }
        
        ResolvedBundle {
            id: self.id.clone(),
            version: self.version.clone(),
            documents,
        }
    }
}
```

**`required_if` evaluation:**

```rust
pub fn evaluate_required_if(
    condition: &str,
    context: &ExpansionContext,
) -> Result<bool, ConditionError> {
    // Simple expression evaluator
    // Supports: has_<role>, arg_equals, arg_exists
    match condition {
        s if s.starts_with("has_") => {
            let role = s.strip_prefix("has_").unwrap();
            Ok(context.has_role(role))
        }
        s if s.contains("==") => {
            let parts: Vec<_> = s.split("==").collect();
            let arg_name = parts[0].trim();
            let expected = parts[1].trim().trim_matches('"');
            Ok(context.arg_equals(arg_name, expected))
        }
        _ => Err(ConditionError::UnknownCondition(condition.to_string())),
    }
}
```

---

### Partial Expansion and UI Integration

#### Serialization Format

`PartialExpansion` must be serializable for round-trip through the UI:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialExpansion {
    /// Unique ID for this expansion session
    pub session_id: Uuid,
    
    /// Macro being expanded
    pub macro_id: MacroId,
    
    /// Macro version at start of expansion
    pub macro_version: String,
    
    /// Args provided so far (user input)
    pub provided_args: HashMap<String, SerializableArgValue>,
    
    /// Args inferred from context (not prompted)
    pub inferred_args: HashMap<String, SerializableArgValue>,
    
    /// Args still needed
    pub missing_args: Vec<MissingArg>,
    
    /// Timestamp for expiration
    pub created_at: DateTime<Utc>,
    
    /// Optional: partial AST if we want to resume mid-expansion
    pub partial_ast: Option<Vec<SerializedAstNode>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SerializableArgValue {
    String(String),
    Bool(bool),
    Int(i64),
    Enum(String),
    EntityRef(Uuid),  // entity ID, not symbol
    List(Vec<SerializableArgValue>),
}
```

#### UI Flow

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Partial Expansion UI Flow                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  1. User invokes macro (e.g., via search or template selection)     │
│     │                                                                │
│     ▼                                                                │
│  2. Backend: expand_macro(id, initial_args)                         │
│     │                                                                │
│     ├─── Expanded? ──▶ Execute DSL, done                            │
│     │                                                                │
│     └─── NeedsArgs? ──▶ Return PartialExpansion + missing list      │
│                          │                                           │
│                          ▼                                           │
│  3. UI: Render form for missing args                                │
│     │   - Show arg name, type, description                          │
│     │   - Entity refs → entity picker                               │
│     │   - Enums → dropdown                                          │
│     │   - Lists → multi-select or dynamic add                       │
│     │                                                                │
│     ▼                                                                │
│  4. User fills in missing args, submits                             │
│     │                                                                │
│     ▼                                                                │
│  5. Backend: resume_expansion(session_id, new_args)                 │
│     │                                                                │
│     ├─── Still NeedsArgs? ──▶ Loop back to step 3                   │
│     │                                                                │
│     └─── Expanded? ──▶ Execute DSL, done                            │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

#### API Endpoints

```rust
/// Start macro expansion
/// POST /api/macros/{macro_id}/expand
/// Body: { "args": { "name": "Alpha Fund", ... } }
/// Response: ExpansionResponse

#[derive(Serialize)]
#[serde(tag = "status")]
pub enum ExpansionResponse {
    #[serde(rename = "complete")]
    Complete {
        cbu_id: Uuid,
        entities_created: Vec<EntitySummary>,
        roles_attached: Vec<RoleSummary>,
    },
    
    #[serde(rename = "needs_args")]
    NeedsArgs {
        session_id: Uuid,
        missing_args: Vec<MissingArgResponse>,
        provided_args: HashMap<String, serde_json::Value>,
        expires_at: DateTime<Utc>,
    },
    
    #[serde(rename = "error")]
    Error {
        code: String,
        message: String,
        details: Option<serde_json::Value>,
    },
}

#[derive(Serialize)]
pub struct MissingArgResponse {
    pub name: String,
    pub arg_type: String,  // "string", "bool", "enum", "entity_ref", "list"
    pub description: String,
    pub required: bool,
    pub enum_variants: Option<Vec<String>>,  // if enum
    pub entity_kind: Option<String>,         // if entity_ref, for picker filtering
    pub list_item_type: Option<String>,      // if list
    pub default: Option<serde_json::Value>,
}

/// Resume partial expansion
/// POST /api/macros/sessions/{session_id}/resume
/// Body: { "args": { "depositary": "uuid-of-entity", ... } }
/// Response: ExpansionResponse (same as above)
```

#### Session Storage

```sql
CREATE TABLE expansion_sessions (
    session_id UUID PRIMARY KEY,
    macro_id TEXT NOT NULL,
    macro_version TEXT NOT NULL,
    state JSONB NOT NULL,  -- serialized PartialExpansion
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,  -- e.g., 24 hours from creation
    completed_at TIMESTAMPTZ,
    user_id UUID NOT NULL
);

-- Clean up expired sessions
CREATE INDEX idx_expansion_sessions_expires ON expansion_sessions(expires_at) 
    WHERE completed_at IS NULL;
```

---

### Error Handling and Diagnostics

#### Error Taxonomy

Errors are categorized by phase and severity:

```rust
/// Phase where error occurred
#[derive(Debug, Clone, Copy)]
pub enum ErrorPhase {
    Parse,      // Syntax errors
    Resolve,    // Unknown macros, undefined symbols
    Expand,     // Macro expansion failures
    Lint,       // Semantic validation errors
    Topo,       // Dependency cycle detection
    Execute,    // Runtime failures
}

/// Severity level
#[derive(Debug, Clone, Copy)]
pub enum Severity {
    Error,      // Blocks progression
    Warning,    // Allows progression with caution
    Info,       // Informational
    Hint,       // Suggestion for improvement
}
```

#### Diagnostic Structure

```rust
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub phase: ErrorPhase,
    pub severity: Severity,
    pub code: DiagnosticCode,
    pub message: String,
    pub span: Option<Span>,
    pub related: Vec<RelatedInfo>,
    pub fixes: Vec<SuggestedFix>,
}

#[derive(Debug, Clone)]
pub enum DiagnosticCode {
    // Parse phase
    E0001_SyntaxError,
    E0002_UnexpectedToken,
    E0003_UnterminatedString,
    
    // Resolve phase
    E0100_UnknownMacro { id: String },
    E0101_UndefinedSymbol { name: String },
    E0102_UnknownArg { macro_id: String, arg: String },
    E0103_UnknownDomicile { domicile: String },
    E0104_UnknownEntityKind { kind: String },
    
    // Expand phase
    E0200_MissingRequiredArg { arg: String },
    E0201_TypeMismatch { expected: String, got: String },
    E0202_ConditionalRequirementFailed { arg: String, condition: String },
    E0203_NestedMacroFailed { macro_id: String },
    E0204_InvalidEnumVariant { arg: String, variant: String, allowed: Vec<String> },
    
    // Lint phase
    E0300_MissingRequiredRole { role: String },
    E0301_CardinalityViolation { role: String, expected: String, got: usize },
    E0302_DuplicateRoleAttachment { role: String, subject: String },
    E0303_InvalidRoleTarget { role: String, target_kind: String },
    
    // Lint warnings
    W0300_PlaceholderUnresolved { kind: String },
    W0301_DeprecatedMacro { macro_id: String, replacement: Option<String> },
    W0302_UnusedArg { arg: String },
    
    // Topo phase
    E0400_DependencyCycle { cycle: Vec<String> },
    
    // Execute phase
    E0500_EntityAlreadyExists { name: String },
    E0501_RoleAlreadyAttached { role: String },
    E0502_DatabaseError { details: String },
}
```

#### Unknown Macro Handling

At **resolve phase** (before expansion):

```rust
pub fn resolve_macro_invocation(
    node: &AstNode,
    registry: &MacroRegistry,
) -> Result<(), Diagnostic> {
    if let AstNode::MacroInvocation { id, span, .. } = node {
        // Check if macro exists
        if registry.get(id).is_none() {
            // Try alias resolution
            if let Some(canonical) = registry.resolve_alias(&id.to_string()) {
                return Ok(()); // Will be normalized in expansion
            }
            
            // Try fuzzy match for suggestions
            let suggestions = registry.fuzzy_match(&id.to_string(), 3);
            
            return Err(Diagnostic {
                phase: ErrorPhase::Resolve,
                severity: Severity::Error,
                code: DiagnosticCode::E0100_UnknownMacro { id: id.to_string() },
                message: format!("Unknown macro '{}'", id),
                span: Some(span.clone()),
                related: vec![],
                fixes: suggestions.into_iter().map(|s| SuggestedFix {
                    message: format!("Did you mean '{}'?", s),
                    edits: vec![TextEdit { span: span.clone(), new_text: s }],
                }).collect(),
            });
        }
    }
    Ok(())
}
```

#### Unknown Argument Values

At **expand phase**:

```rust
fn validate_enum_arg(
    arg_name: &str,
    value: &str,
    arg_spec: &ArgSpec,
) -> Result<(), Diagnostic> {
    if let ArgType::Enum { variants } = &arg_spec.arg_type {
        if !variants.contains(&value.to_string()) {
            return Err(Diagnostic {
                phase: ErrorPhase::Expand,
                severity: Severity::Error,
                code: DiagnosticCode::E0204_InvalidEnumVariant {
                    arg: arg_name.to_string(),
                    variant: value.to_string(),
                    allowed: variants.clone(),
                },
                message: format!(
                    "Invalid value '{}' for argument '{}'. Expected one of: {}",
                    value, arg_name, variants.join(", ")
                ),
                span: None, // populated from context
                related: vec![],
                fixes: variants.iter().map(|v| SuggestedFix {
                    message: format!("Use '{}'", v),
                    edits: vec![], // would need span context
                }).collect(),
            });
        }
    }
    Ok(())
}
```

#### LSP Diagnostic Surfacing

```rust
impl Diagnostic {
    pub fn to_lsp_diagnostic(&self) -> lsp_types::Diagnostic {
        lsp_types::Diagnostic {
            range: self.span.as_ref()
                .map(|s| s.to_lsp_range())
                .unwrap_or_default(),
            severity: Some(match self.severity {
                Severity::Error => lsp_types::DiagnosticSeverity::ERROR,
                Severity::Warning => lsp_types::DiagnosticSeverity::WARNING,
                Severity::Info => lsp_types::DiagnosticSeverity::INFORMATION,
                Severity::Hint => lsp_types::DiagnosticSeverity::HINT,
            }),
            code: Some(lsp_types::NumberOrString::String(
                format!("{:?}", self.code)
            )),
            source: Some("ob-dsl".to_string()),
            message: self.message.clone(),
            related_information: Some(
                self.related.iter().map(|r| r.to_lsp()).collect()
            ),
            ..Default::default()
        }
    }
}
```

---

### Cross-Border Macro Extensibility

#### Dispatcher Pattern

Refactor cross-border macros as **dispatchers** that select and compose underlying macros:

```yaml
- id: struct.cross_border
  tier: macro
  display_name: "Set up Cross-Border Fund Structure"
  taxonomy: [structure, fund, cross_border]
  
  args:
    - name: name
      type: string
      required: true
    
    - name: strategy
      type: enum
      variants: [hedge, private_equity, credit, real_assets]
      required: true
    
    - name: fund_domicile
      type: enum
      variants: [ie, lux, us, uk, jersey, cayman, guernsey]
      required: true
    
    - name: fund_vehicle
      type: enum
      # Variants filtered by domicile in UI
      variants: [icav, sicav, raif, scsp, delaware_lp, uk_lp, jersey_lp, cayman_spc, guernsey_pcc]
      required: true
    
    - name: manager_domicile
      type: enum
      variants: [uk, us, ie, lux, jersey, guernsey]
      required: true
    
    - name: manager_vehicle
      type: enum
      variants: [llp, ltd, adviser, manco]
      required: true
    
    - name: manager_name
      type: string
      required: true
    
    - name: manager_regulated
      type: bool
      default: true
    
    # Optional service providers
    - name: prime_broker
      type: entity_ref
      required_if: strategy == "hedge"
      placeholder_if_missing: true
    
    - name: depositary
      type: entity_ref
      placeholder_if_missing: true
    
    - name: administrator
      type: entity_ref
      placeholder_if_missing: true
    
    - name: auditor
      type: entity_ref
      placeholder_if_missing: true
```

#### Vehicle Registry

Instead of hardcoding combinations, maintain a registry of fund vehicles and manager types:

```yaml
fund_vehicles:
  - id: ie_icav
    domicile: IE
    wrapper: icav
    regimes: [ucits, aif]
    macro: struct.ie.ucits.icav  # or struct.ie.aif.icav based on regime
    supports_umbrella: true
    requires_depositary: true
  
  - id: lux_raif
    domicile: LU
    wrapper: raif
    regimes: [aif]
    macro: struct.lux.aif.raif
    supports_umbrella: false
    requires_depositary: true
  
  - id: jersey_lp
    domicile: JE
    wrapper: lp
    regimes: [unregulated, jpf]
    macro: struct.jersey.lp  # to be implemented
    supports_umbrella: false
    requires_depositary: false
  
  - id: cayman_spc
    domicile: KY
    wrapper: spc
    regimes: [unregulated, cima]
    macro: struct.cayman.spc  # to be implemented
    supports_umbrella: true  # via segregated portfolios
    requires_depositary: false

manager_vehicles:
  - id: uk_llp
    domicile: UK
    entity_type: llp
    macro: struct.uk.manager.llp
    regulator: FCA
  
  - id: us_adviser
    domicile: US
    entity_type: investment_adviser
    macro: struct.us.manager.adviser  # to be implemented
    regulator: SEC
  
  - id: jersey_manco
    domicile: JE
    entity_type: manco
    macro: struct.jersey.manager  # to be implemented
    regulator: JFSC
```

#### Dispatcher Implementation

```rust
fn expand_cross_border(
    args: &HashMap<String, ArgValue>,
    registry: &MacroRegistry,
    scope: &mut SymbolScope,
    origin: MacroOrigin,
) -> Result<Vec<AstNode>, ExpansionError> {
    let fund_domicile = args.get_enum("fund_domicile")?;
    let fund_vehicle = args.get_enum("fund_vehicle")?;
    let manager_domicile = args.get_enum("manager_domicile")?;
    let manager_vehicle = args.get_enum("manager_vehicle")?;
    let strategy = args.get_enum("strategy")?;
    
    // Look up vehicle definitions
    let fund_def = registry.fund_vehicles
        .get(&format!("{}_{}", fund_domicile.to_lowercase(), fund_vehicle))
        .ok_or_else(|| ExpansionError::UnsupportedVehicle {
            domicile: fund_domicile.clone(),
            vehicle: fund_vehicle.clone(),
        })?;
    
    let manager_def = registry.manager_vehicles
        .get(&format!("{}_{}", manager_domicile.to_lowercase(), manager_vehicle))
        .ok_or_else(|| ExpansionError::UnsupportedManager {
            domicile: manager_domicile.clone(),
            vehicle: manager_vehicle.clone(),
        })?;
    
    let mut nodes = Vec::new();
    
    // 1. Expand manager macro
    let manager_macro = registry.get(&manager_def.macro_id)?;
    let manager_args = build_manager_args(args, &manager_def)?;
    let manager_result = expand_macro(manager_macro, manager_args, scope, origin.clone())?;
    nodes.extend(manager_result.nodes);
    let manager_symbol = manager_result.symbols_exported.get("@manager")
        .ok_or(ExpansionError::MissingExportedSymbol("@manager".into()))?;
    
    // 2. Expand fund macro
    let fund_macro = registry.get(&fund_def.macro_id)?;
    let mut fund_args = build_fund_args(args, &fund_def)?;
    
    // Wire manager as AIFM/IM based on fund type
    if fund_def.regimes.contains(&"aif") {
        fund_args.insert("aifm".into(), ArgValue::Symbol(manager_symbol.clone()));
    } else {
        fund_args.insert("investment_manager".into(), ArgValue::Symbol(manager_symbol.clone()));
    }
    
    let fund_result = expand_macro(fund_macro, fund_args, scope, origin.clone())?;
    nodes.extend(fund_result.nodes);
    
    // 3. Strategy-specific additions
    if strategy == "hedge" {
        if let Some(pb) = args.get("prime_broker") {
            nodes.extend(expand_prime_broker_attachment(pb, scope, origin.clone())?);
        }
    }
    
    // 4. Import symbols to caller scope
    scope.bind("@cbu", fund_result.symbols_exported.get("@cbu").cloned());
    scope.bind("@fund", fund_result.symbols_exported.get("@fund").cloned());
    scope.bind("@manager", Some(manager_symbol.clone()));
    
    Ok(nodes)
}
```

#### Adding New Jurisdictions

To add Jersey manager + Cayman fund support:

1. **Add fund vehicle YAML:**
```yaml
- id: cayman_spc
  domicile: KY
  wrapper: spc
  regimes: [unregulated]
  macro: struct.cayman.spc
```

2. **Implement the macro:**
```yaml
- id: struct.cayman.spc
  tier: macro
  display_name: "Set up Cayman SPC"
  # ... standard structure
```

3. **Cross-border dispatcher automatically supports it** — no changes needed to M17/M18.

---

### Testing: Negative Cases

Add these negative test cases to the test plan:

#### Expansion Phase

```rust
#[cfg(test)]
mod expansion_tests {
    #[test]
    fn test_unknown_macro_error() {
        let registry = MacroRegistry::load_test();
        let ast = parse("(struct.nonexistent.macro :name \"Test\")").unwrap();
        
        let result = expand_macros(ast, &registry);
        assert!(matches!(
            result,
            Err(ExpansionError::UnknownMacro(id)) if id == "struct.nonexistent.macro"
        ));
    }
    
    #[test]
    fn test_missing_required_arg_error() {
        let registry = MacroRegistry::load_test();
        // UCITS requires :name
        let ast = parse("(struct.lux.ucits.sicav :umbrella true)").unwrap();
        
        let result = expand_macros(ast, &registry);
        assert!(matches!(
            result,
            Err(ExpansionError::IncompleteArgs { missing, .. }) 
                if missing.iter().any(|a| a.name == "name")
        ));
    }
    
    #[test]
    fn test_invalid_enum_variant_error() {
        let registry = MacroRegistry::load_test();
        let ast = parse(r#"(struct.us.private_fund.delaware_lp 
            :name "Test" 
            :gp @some_gp 
            :strategy "invalid_strategy")"#).unwrap();
        
        let result = expand_macros(ast, &registry);
        assert!(matches!(
            result,
            Err(ExpansionError::InvalidEnumVariant { arg, variant, .. })
                if arg == "strategy" && variant == "invalid_strategy"
        ));
    }
    
    #[test]
    fn test_conditional_required_arg_error() {
        let registry = MacroRegistry::load_test();
        // hedge strategy requires prime_broker
        let ast = parse(r#"(struct.us.private_fund.delaware_lp 
            :name "Test" 
            :gp @some_gp 
            :strategy "hedge")"#).unwrap();
        
        let result = expand_macros(ast, &registry);
        assert!(matches!(
            result,
            Err(ExpansionError::IncompleteArgs { missing, .. })
                if missing.iter().any(|a| a.name == "prime_broker")
        ));
    }
    
    #[test]
    fn test_type_mismatch_error() {
        let registry = MacroRegistry::load_test();
        // :umbrella expects bool, not string
        let ast = parse(r#"(struct.lux.ucits.sicav 
            :name "Test" 
            :umbrella "yes")"#).unwrap();
        
        let result = expand_macros(ast, &registry);
        assert!(matches!(
            result,
            Err(ExpansionError::TypeMismatch { expected, got, .. })
                if expected == "bool" && got == "string"
        ));
    }
}
```

#### Lint Phase

```rust
#[cfg(test)]
mod lint_tests {
    #[test]
    fn test_duplicate_depositary_error() {
        let ast = parse(r#"
            (cbu.ensure :name "Test" :as @cbu)
            (fund.ensure :wrapper "sicav" :as @fund)
            (entity.ensure :name "Dep1" :as @dep1)
            (entity.ensure :name "Dep2" :as @dep2)
            (role.attach :subject @fund :role "depositary" :object @dep1)
            (role.attach :subject @fund :role "depositary" :object @dep2)
        "#).unwrap();
        
        let diagnostics = lint(ast, &LintConfig::default());
        
        assert!(diagnostics.iter().any(|d| matches!(
            &d.code,
            DiagnosticCode::E0301_CardinalityViolation { role, expected, got }
                if role == "depositary" && expected == "0..1" && *got == 2
        )));
    }
    
    #[test]
    fn test_missing_required_role_error() {
        // UCITS must have depositary
        let ast = expand_macro("struct.lux.ucits.sicav", hashmap!{
            "name" => "Test Fund",
            // depositary intentionally omitted and placeholder disabled
        }, &MacroRegistry::load_test()).unwrap();
        
        // Remove depositary attachment from expanded AST (simulate)
        let ast_without_depositary = remove_role_attachment(ast, "depositary");
        
        let diagnostics = lint(ast_without_depositary, &LintConfig::default());
        
        assert!(diagnostics.iter().any(|d| matches!(
            &d.code,
            DiagnosticCode::E0300_MissingRequiredRole { role }
                if role == "depositary"
        )));
    }
    
    #[test]
    fn test_invalid_role_target_error() {
        // Can't attach "depositary" role to an entity of kind "auditor"
        let ast = parse(r#"
            (fund.ensure :wrapper "sicav" :as @fund)
            (entity.ensure :name "Big Four" :kind "auditor" :as @auditor)
            (role.attach :subject @fund :role "depositary" :object @auditor)
        "#).unwrap();
        
        let diagnostics = lint(ast, &LintConfig::strict());
        
        assert!(diagnostics.iter().any(|d| matches!(
            &d.code,
            DiagnosticCode::E0303_InvalidRoleTarget { role, target_kind }
                if role == "depositary" && target_kind == "auditor"
        )));
    }
    
    #[test]
    fn test_etf_missing_ap_warning() {
        // ETF should have at least one authorized participant
        let ast = expand_macro("struct.us.etf.40act", hashmap!{
            "name" => "Test ETF",
            "investment_adviser" => "@ia",
            "custodian" => "@cust",
            // authorized_participants missing
        }, &MacroRegistry::load_test());
        
        // With placeholder, this should be a warning not error
        let diagnostics = lint(ast.unwrap(), &LintConfig::default());
        
        assert!(diagnostics.iter().any(|d| 
            d.severity == Severity::Warning &&
            matches!(&d.code, DiagnosticCode::W0300_PlaceholderUnresolved { kind }
                if kind == "authorized_participant")
        ));
    }
}
```

#### Idempotency Tests

```rust
#[cfg(test)]
mod idempotency_tests {
    #[test]
    fn test_double_expansion_no_duplicates() {
        let registry = MacroRegistry::load_test();
        let mut db = TestDb::new();
        
        let args = hashmap!{
            "name" => "Alpha Fund",
            "umbrella" => false,
        };
        
        // First expansion + execution
        let ast1 = expand_macro("struct.lux.ucits.sicav", args.clone(), &registry).unwrap();
        execute(ast1, &mut db).unwrap();
        
        let entities_after_first = db.count_entities();
        let roles_after_first = db.count_role_edges();
        
        // Second expansion + execution (same args)
        let ast2 = expand_macro("struct.lux.ucits.sicav", args, &registry).unwrap();
        execute(ast2, &mut db).unwrap();
        
        let entities_after_second = db.count_entities();
        let roles_after_second = db.count_role_edges();
        
        // No new entities or roles should be created
        assert_eq!(entities_after_first, entities_after_second);
        assert_eq!(roles_after_first, roles_after_second);
    }
    
    #[test]
    fn test_expansion_determinism() {
        let registry = MacroRegistry::load_test();
        
        let args = hashmap!{
            "name" => "Beta Fund",
            "umbrella" => true,
            "subfunds" => vec!["EQ", "FI", "MM"],
        };
        
        // Expand multiple times
        let ast1 = expand_macro("struct.lux.ucits.sicav", args.clone(), &registry).unwrap();
        let ast2 = expand_macro("struct.lux.ucits.sicav", args.clone(), &registry).unwrap();
        let ast3 = expand_macro("struct.lux.ucits.sicav", args, &registry).unwrap();
        
        // All expansions should produce identical AST (ignoring gensyms)
        assert!(ast_equal_ignoring_gensyms(&ast1, &ast2));
        assert!(ast_equal_ignoring_gensyms(&ast2, &ast3));
    }
}
```

#### Edge Cases

```rust
#[cfg(test)]
mod edge_case_tests {
    #[test]
    fn test_empty_subfunds_list() {
        let registry = MacroRegistry::load_test();
        
        let args = hashmap!{
            "name" => "Empty Umbrella",
            "umbrella" => true,
            "subfunds" => Vec::<String>::new(), // empty list
        };
        
        let result = expand_macro("struct.lux.ucits.sicav", args, &registry);
        
        // Should either error (subfunds required when umbrella=true) 
        // or warn (umbrella with no subfunds is unusual)
        assert!(result.is_err() || {
            let ast = result.unwrap();
            let diagnostics = lint(ast, &LintConfig::default());
            diagnostics.iter().any(|d| d.severity == Severity::Warning)
        });
    }
    
    #[test]
    fn test_circular_nested_macro() {
        // Hypothetical: macro A invokes macro B which invokes macro A
        // Should detect cycle and error, not stack overflow
        
        let mut registry = MacroRegistry::new();
        registry.add(MacroDefn {
            id: "test.macro.a".into(),
            expansion: vec![
                ExpansionTemplate::InvokeMacro { 
                    macro_id: "test.macro.b".into(), 
                    args: hashmap!{},
                    import_symbols: vec![],
                },
            ],
            ..Default::default()
        });
        registry.add(MacroDefn {
            id: "test.macro.b".into(),
            expansion: vec![
                ExpansionTemplate::InvokeMacro { 
                    macro_id: "test.macro.a".into(), 
                    args: hashmap!{},
                    import_symbols: vec![],
                },
            ],
            ..Default::default()
        });
        
        let result = expand_macro("test.macro.a", hashmap!{}, &registry);
        
        assert!(matches!(
            result,
            Err(ExpansionError::MacroCycle { path })
                if path.contains(&"test.macro.a".into()) && path.contains(&"test.macro.b".into())
        ));
    }
    
    #[test]
    fn test_symbol_scope_isolation() {
        // Ensure symbols from one macro don't leak to sibling macro invocations
        let registry = MacroRegistry::load_test();
        
        let ast = parse(r#"
            (struct.uk.manager.llp :name "Manager1" :as @mgr1)
            (struct.uk.manager.llp :name "Manager2" :as @mgr2)
        "#).unwrap();
        
        let expanded = expand_macros(ast, &registry).unwrap();
        
        // Both should create separate entities, neither should reference the other's internals
        let mgr1_entity = find_entity_by_symbol(&expanded, "@mgr1");
        let mgr2_entity = find_entity_by_symbol(&expanded, "@mgr2");
        
        assert_ne!(mgr1_entity.name, mgr2_entity.name);
        // Internal gensyms should be different
        assert!(!expanded.contains_symbol("@__gensym") || 
                count_unique_gensyms(&expanded) >= 2);
    }
}
```

---

## Appendix: DSL Verb Reference

Expected DSL primitives (canonical names—adjust to match your actual verbs):

| Verb | Description |
|------|-------------|
| `cbu.ensure` | Create or fetch CBU by name/domicile |
| `fund.ensure` | Create or fetch fund vehicle |
| `fund.umbrella.ensure` | Mark fund as umbrella |
| `fund.subfund.ensure` | Create subfund under umbrella |
| `fund.metadata.set` | Set key-value metadata on fund |
| `entity.ensure` | Create or fetch entity by ref |
| `entity.ensure_or_placeholder` | Create entity or placeholder if ref missing |
| `role.attach` | Attach role edge between entities |
| `product.enable` | Enable products on CBU |
| `docs.bundle.apply` | Apply docs bundle to CBU/entity |
