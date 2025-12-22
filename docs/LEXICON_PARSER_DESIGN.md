# Lexicon Parser Design - Architecture Reference

**Status:** Approved architectural pivot for Phase 3

## Problem Statement

The current intent classification system uses **regex pattern matching** against trigger phrases. This approach is fundamentally fragile:

```
Trigger phrase: "add {counterparty} as counterparty"
User input:     "Add Goldman Sachs as a counterparty"
Result:         NO MATCH (missing article "a")
```

### Observed Failure (49% accuracy, 0% OTC)

| Input | Expected | Got | Root Cause |
|-------|----------|-----|------------|
| "Add Goldman Sachs as a counterparty" | `counterparty_create` | `[]` | Article "a" breaks regex |
| "Establish ISDA with Goldman Sachs" | `isda_establish` | `[]` | Pattern not matching |
| "Add BlackRock as investment manager" | `im_assign` | `[]` | Regex too rigid |

### Why Regex Fails for NLU

1. **Combinatorial explosion** - Every variation needs a pattern
2. **Order sensitivity** - "Add X as Y" vs "As Y, add X"
3. **Slot boundary bleeding** - Greedy/non-greedy capture conflicts
4. **No semantic understanding** - "Goldman" and "Goldman Sachs" are different strings
5. **Maintenance nightmare** - Adding synonyms = editing regex

---

## Solution: Lexical Tokenizer + Nom Parser

Replace regex pattern matching with a **two-layer architecture**:

```
┌─────────────────────────────────────────────────────────────────┐
│  Layer 1: LEXICAL TOKENIZER                                     │
│  - Dictionary-backed token classification                       │
│  - Resolves against known vocabularies (DB + YAML + Gateway)   │
│  - Output: Typed token stream                                   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Layer 2: NOM GRAMMAR PARSER                                    │
│  - Parses token stream into IntentAST                          │
│  - Same parser technology as existing DSL parser               │
│  - Compositional, recursive, well-defined                      │
└─────────────────────────────────────────────────────────────────┘
```

---

## Token Types

| Token Type | Source | Examples |
|------------|--------|----------|
| `VERB` | action dictionary | add, establish, configure, set, use |
| `ENTITY` | EntityGateway lookup | Goldman Sachs, BlackRock, JP Morgan |
| `PRODUCT` | OTC derivatives | IRS, CDS, FX_FORWARD, SWAPTION |
| `INSTRUMENT` | Exchange-traded | EQUITY, GOVT_BOND, CORP_BOND |
| `MARKET` | MIC codes | XNYS, XLON, XETR |
| `CURRENCY` | ISO codes | USD, EUR, GBP |
| `ROLE` | Relationship types | counterparty, custodian, investment_manager |
| `CSA_TYPE` | CSA classification | VM, IM |
| `LAW` | Governing law | ENGLISH, NY |
| `PREP` | Prepositions | for, with, as, to, under |
| `CONJ` | Conjunctions | and, or |
| `ARTICLE` | Articles (absorbed) | a, an, the |
| `PRONOUN` | Coreference | them, it, their (resolved from context) |
| `UNKNOWN` | Unrecognized | (triggers clarification) |

---

## Verb Classification

```rust
pub enum VerbClass {
    Create,   // add, establish, create, onboard, set up
    Update,   // set, configure, update, change, modify
    Delete,   // remove, delete, cancel, terminate
    Query,    // show, list, find, who, what, where
    Link,     // connect, link, assign, use, via
}
```

---

## Intent Grammar

The key insight: **intent = verb + role determines intent type, entities fill slots**.

```
counterparty_create = VERB:Create ENTITY? PREP? ROLE:counterparty
isda_establish      = VERB:Create ISDA_VERSION? ENTITY PREP? LAW?
im_assign           = VERB:Create ENTITY? PREP? ROLE:investment_manager SCOPE?
scope               = PREP (PRODUCT | INSTRUMENT | MARKET)+
```

---

## Example Flow

```
Input: "Add Goldman Sachs as a counterparty for IRS"

Tokenize:
  [VERB:add] [ENTITY:Goldman Sachs] [PREP:as] [ART:a] [ROLE:counterparty] [PREP:for] [PRODUCT:IRS]

Parse (article absorbed):
  IntentAst::CounterpartyCreate {
      entity: ResolvedEntity { name: "Goldman Sachs", id: uuid },
      products: [IRS],
  }

Generate DSL:
  (counterparty.ensure
    :name "Goldman Sachs"
    :counterparty-type BANK
    :as @cp-goldman)
```

---

## Coreference Resolution

Pronouns resolved by tokenizer using session salience:

```
Turn 1: "Add BlackRock as investment manager"
        → Session: salient_entities = [BlackRock]

Turn 2: "Set their scope to European equities"
        → Tokenize: [VERB:set] [PRONOUN:their→BlackRock] [scope...]
        → "their" resolved to @im-blackrock
```

---

## Error Handling

```rust
pub enum ParseError {
    Incomplete {
        partial: IntentAst,
        missing: Vec<ExpectedToken>,  // "I need: counterparty name"
    },
    Ambiguous {
        options: Vec<IntentAst>,      // "Did you mean X or Y?"
    },
    UnknownTokens {
        tokens: Vec<Token>,           // "I don't recognize: xyz"
    },
    SyntaxError {
        expected: String,
        found: Token,                 // "Expected ROLE after ENTITY"
    },
}
```

---

## Why This Is Better

| Aspect | Regex Approach | Lexicon + Grammar |
|--------|----------------|-------------------|
| "Add X as a counterparty" | ❌ Fails (article) | ✅ Article absorbed |
| "Goldman" vs "Goldman Sachs" | ❌ Different strings | ✅ Same entity token |
| "Set up ISDA" | ❌ Need alias pattern | ✅ Alias in lexicon |
| Word order variation | ❌ Need multiple patterns | ✅ Grammar handles |
| Unknown entity | ❌ Silent fail | ✅ UNKNOWN token, recovery |
| Error messages | ❌ "No match" | ✅ "Expected ROLE after ENTITY" |
| Testability | ❌ Test regex strings | ✅ Test token stream + AST |
| Maintainability | ❌ Edit regex patterns | ✅ Add to lexicon YAML |

---

## Success Criteria

- [ ] Evaluation accuracy > 85% (was 49%)
- [ ] OTC domain accuracy > 80% (was 0%)
- [ ] Unknown token rate < 10%
- [ ] Parse error rate < 5%
- [ ] Latency < 50ms (tokenize + parse)
- [ ] Zero regex patterns for intent matching

---

## Implementation

See `TODO-PHASE3-COMPLETE.md` for full implementation details.
