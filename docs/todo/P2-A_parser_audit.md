# P2-A: Parser (nom Layer) — Architecture Audit

> **Auditor:** Claude Opus 4.6
> **Date:** 2026-03-16 (revised — deep source audit)
> **Scope:** `dsl-core/src/parser.rs`, `ast.rs`, `binding_context.rs`, `diagnostics.rs`, `compiler.rs`, `viewport_parser.rs`, `dsl-lsp/src/analysis/v2_adapter.rs`
> **Parser:** nom 7 combinators with `nom_locate::LocatedSpan` for byte-offset span tracking

---

## 1. Informal Grammar Specification

Derived by reading every nom combinator in `parser.rs`. This is the *implemented* grammar, not a design specification.

```
program        ::= ws* statement* ws*

statement      ::= comment
                 | verb_call

comment        ::= ';;' [^\n]* '\n'?

verb_call      ::= '(' ws* word argument* ws* as_binding? ws* ')'

word           ::= kebab_ident '.' kebab_ident

argument       ::= ws+ keyword ws+ value
                   (* lookahead: keyword must NOT be ':as' followed by whitespace *)

keyword        ::= ':' kebab_ident

as_binding     ::= ':as' ws+ '@' identifier

value          ::= boolean
                 | null
                 | symbol_ref
                 | string
                 | number
                 | nested_verb_call
                 | list
                 | map

boolean        ::= 'true' | 'false'          (* no word-boundary check *)

null           ::= 'nil'                      (* no word-boundary check *)

symbol_ref     ::= '@' identifier

string         ::= '"' string_char+ '"'       (* note: one-or-more, not zero-or-more *)
string_char    ::= [^"\\]
                 | '\\' escape_seq
escape_seq     ::= 'n' | 'r' | 't' | '\\' | '"'

number         ::= '-'? DIGIT+ ('.' DIGIT+)?

nested_verb_call ::= verb_call                (* recursive — enables subexpressions *)

list           ::= '[' ws* (value (ws* ','? ws* value)*)? ws* ']'

map            ::= '{' ws* (map_entry (ws* map_entry)*)? ws* '}'
map_entry      ::= keyword ws+ value

kebab_ident    ::= (ALPHA | '_') (ALPHANUMERIC | '_' | '-')*
identifier     ::= (ALPHA | '_') (ALPHANUMERIC | '_' | '-')*

ws             ::= WHITESPACE+               (* multispace0/multispace1 *)
```

### Key Grammar Notes

1. **Comment delimiter is `;;` (double semicolon)**, not single `;`. This diverges from standard Lisp.
2. **Null is `nil`**, not `null`. The previous audit incorrectly stated `null`.
3. **String requires at least one character** — `""` (empty string) fails to parse. This is a known limitation documented in the test at line 886.
4. **List comma separators are optional** — `[1 2 3]` and `[1, 2, 3]` both parse identically.
5. **Map entries do NOT use commas** — entries are whitespace-separated only.
6. **`EntityRef` does not exist at parse time.** The parser produces only `Literal`, `SymbolRef`, `List`, `Map`, and `Nested` nodes. The `<entity_name>` angle-bracket syntax is NOT in the parser grammar — EntityRef nodes are created during the post-parse enrichment pass when a `Literal::String` matches a verb arg with `lookup:` config.
7. **UUID auto-detection** happens inside `string_literal_with_span` — after parsing the string content, the parser checks `Uuid::parse_str()` and converts to `Literal::Uuid` if valid.

### Two-Phase AST Pipeline

```
Source text
    |
    v
parse_program()  ->  Raw AST (Literal / SymbolRef / List / Map / Nested only)
    |
    v
enrich_program() ->  Enriched AST (String literals -> EntityRef where YAML has lookup config)
    |
    v
compile / execute
```

### Public API

| Function | Wrapping | Purpose |
|----------|----------|---------|
| `parse_program(input)` | `all_consuming(program::<VerboseError>)` | Full program parse; rejects trailing input |
| `parse_single_verb(input)` | `all_consuming(delimited(ws, verb_call, ws))` | Single REPL verb; trims input first |

Both use `VerboseError<NomSpan>` for error reporting with `format_verbose_error()` producing line/column/caret display.

---

## 2. Findings

### P1 — High

#### F-01: Empty string `""` fails to parse

**File:** `parser.rs:340-341`
**Code:**
```rust
escaped_transform(
    none_of("\"\\"),  // requires at least one non-quote, non-backslash char
    '\\',
    alt((...escapes...)),
)
```

**Problem:** `escaped_transform` with `none_of("\"\\")` as the normal-char parser requires at least one normal character between the opening and closing quote. An empty string `""` has zero normal characters, so `none_of` fails immediately, and the entire `delimited(char('"'), ..., char('"'))` fails.

**Evidence:** The test at line 886-896 documents this limitation explicitly:
```rust
// Note: Empty strings are parsed as no value - this is a parser limitation
// For now, test that a single space string works
```

**Impact:** Any DSL expression like `(entity.create :name "")` fails to parse. This affects:
- Default/empty values in template expansion
- Clearing a field to empty
- YAML-generated DSL that may produce empty strings

**Recommendation:** Add an empty-string special case:
```rust
delimited(
    char('"'),
    alt((
        escaped_transform(none_of("\"\\"), '\\', alt((...escapes...))),
        value(String::new(), peek(char('"'))),  // empty string: no content before closing quote
    )),
    char('"'),
)
```

#### F-02: Boolean/null prefix matching without word boundary

**File:** `parser.rs:312, 325`
**Code:**
```rust
alt((value(true, tag("true")), value(false, tag("false"))))(input)?;  // line 312
tag("nil")(input)?;                                                     // line 325
```

**Problem:** `tag("true")` matches the prefix of hypothetical bare words like `truename`. `tag("nil")` matches the prefix of `nilpotent`. There is no word-boundary check after the tag match.

**Practical mitigation:** Currently safe because:
1. Bare identifiers are NOT a valid value production — only quoted strings, numbers, booleans, null, symbols, lists, maps, and nested verbs are values.
2. In `alt()` ordering, booleans/null are tried first; if they match a prefix, the remaining characters would cause the next combinator (e.g., whitespace expectation) to fail, and `alt()` would backtrack.

However, this is fragile. If the grammar ever adds bare-word enum values, this becomes a real bug.

**Recommendation:** Add word-boundary verification:
```rust
terminated(tag("true"), peek(satisfy(|c| c.is_whitespace() || ")],}".contains(c))))
```

---

### P2 — Medium

#### F-03: Single `cut` point — poor error locality

**File:** `parser.rs:177`
**Code:** `cut(context("closing parenthesis", char(')')))`

**Problem:** The entire 500-line parser has exactly ONE `cut` (committed-parse) point: the closing `)` of a verb call. This means:
- Missing opening `(` -- backtracking, `all_consuming` fails with generic "trailing input"
- Typo in verb name (e.g., `cbu-create` without `.`) -- backtracking from `word`, error points at wrong location
- Invalid keyword format -- backtracking from `keyword`, error at unexpected position
- Missing keyword value -- backtracking from `value_parser_with_span`, unclear message

Only after the parser has consumed `(domain.verb args...` does it commit. Everything before silently backtracks.

**Impact:** Users see unhelpful error messages like "expected closing parenthesis" when the real issue is a missing opening paren or misspelled verb.

**Recommendation:** Add `cut` points after the opening `(` and after `word`:
```rust
let (input, _) = char('(')(input)?;
let (input, _) = multispace0(input)?;
let (input, (domain, verb)) = cut(context("verb name (domain.verb)", word))(input)?;
```

This commits the parser once a `(` is seen, producing "expected verb name" instead of backtracking silently.

#### F-04: No error recovery for multi-statement programs

**File:** `parser.rs:124-131`
**Code:** `many0(statement)` in `program`

**Problem:** `many0` stops at the first statement that fails to parse. If statement 2 of 5 has a syntax error, statements 3-5 are never attempted. The entire program parse fails after statement 1 succeeds and statement 2 fails.

**Impact:** In the LSP (`v2_adapter.rs:parse_with_v2()`), a single broken statement causes loss of all diagnostics, completions, and symbols for the remainder of the file. The LSP falls back to extracting an error range from the formatted error string.

**Recommendation:** For LSP mode, implement a recovery combinator that skips to the next `(` on parse failure:
```rust
fn statement_with_recovery(input) -> IResult<_, Result<Statement, ErrorNode>> {
    match statement(input) {
        Ok(s) => Ok(Ok(s)),
        Err(_) => skip_to_next_paren(input).map(|_| Err(ErrorNode { span }))
    }
}
```
This is a medium-effort change best done as an opt-in LSP parse mode.

#### F-05: Map entries reuse keyword syntax — disambiguation relies solely on `{` delimiter

**File:** `parser.rs:456-493`

**Problem:** Map literals use `:keyword value` syntax identical to verb arguments. Inside a verb call, `many0(argument_with_span)` greedily consumes `:key value` pairs. A map `{:a 1 :b 2}` is valid only because of the `{` delimiter. If a user accidentally writes `:config :a 1 :b 2` (omitting `{}`), the parser silently interprets `:a`, `:b` as separate verb arguments.

**Impact:** Silent misparse (wrong AST structure) rather than error. The user would discover the issue at execution time, not parse time. Not a correctness bug in the parser per se, but a poor error experience.

---

### P3 — Low

#### F-06: `:as` lookahead idiom is correct but fragile

**File:** `parser.rs:225`
**Code:**
```rust
if input.starts_with(":as") && input.fragment()[3..].starts_with(|c: char| c.is_whitespace()) {
```

**Analysis:** The guard `input.starts_with(":as")` ensures `fragment()` is at least 3 bytes. The `[3..]` slice on a 3-byte string returns `""`, and `"".starts_with(...)` returns `false`. So input exactly `:as` (at end of stream) correctly falls through to the `keyword` parser, which matches `:as` as a keyword -- and then `multispace1` after the keyword fails because there is no trailing value. The error path is correct.

**Recommendation:** Improve clarity without changing behavior:
```rust
if input.fragment().strip_prefix(":as")
    .map_or(false, |rest| rest.starts_with(char::is_whitespace))
```

#### F-07: Optional comma in list literals

**File:** `parser.rs:436`

**Problem:** Both `[1, 2, 3]` and `[1 2 3]` parse identically. The comma is purely decorative. This means `[1 -2]` parses as a two-element list `[1, -2]` -- correct, since subtraction is not in the grammar.

**Impact:** Negligible. Document that commas are optional and whitespace separation is canonical.

#### F-08: Comment requires `;;` double semicolon

**File:** `parser.rs:150` -- `tag(";;")`

**Problem:** Single `;` is not recognized as a comment. Users familiar with standard Lisp (`;` for comments) will write single-semicolon comments that are silently rejected by the parser, causing `all_consuming` to fail with a "trailing input" error.

**Recommendation:** Either support single `;` as an alias, or detect bare `;` and produce a helpful diagnostic: "Did you mean `;;`? Comments require double semicolon."

#### F-09: `find_unresolved_refs` uses unnecessary unsafe pointer cast

**File:** `ast.rs:759`
**Code:** `unsafe { &*(node as *const AstNode) }`

**Problem:** The `AstVisitor` trait passes `&AstNode` with an anonymous lifetime, but `Collector<'a>` needs `&'a AstNode` tied to the program's lifetime. The unsafe cast extends the borrow lifetime.

**Why it works:** The collector is used only within `find_unresolved_refs`, where `&AstNode` references genuinely borrow from the `&Program` parameter. The cast is sound but bypasses Rust's lifetime checking.

**Recommendation:** Add a lifetime parameter to `AstVisitor`:
```rust
trait AstVisitor<'a> {
    fn visit_entity_ref(&mut self, node: &'a AstNode) {}
}
```
Or collect `(ref_id, span)` tuples instead of references, eliminating the need for lifetime extension entirely.

#### F-10: `with_resolved_key` panics on non-UUID input

**File:** `ast.rs:558-561`
**Code:** `.expect("resolved_key must be a valid UUID")`

**Problem:** Documented intentional panic. The `try_with_resolved_key()` fallible variant exists. This is a programmer assertion, not an input validation issue. Most callers use the try variant.

#### F-11: `Span::synthetic()` uses `usize::MAX` sentinel

**File:** `ast.rs` (Span implementation)

**Problem:** `Span::synthetic()` creates spans with `start: usize::MAX, end: usize::MAX` as a marker for compiler-generated nodes. Arithmetic on these values (e.g., `end - start` for length) would overflow.

**Impact:** Low -- consumers check for synthetic spans. A `SpanKind` enum would be safer but the current approach works.

---

## 3. Dead Parse Arms Detection

### Analysis Method

Traced every `alt()` branch in the parser to determine reachability by examining leading character(s).

### Results

| Parser Function | Branch | Leading Char(s) | Reachable |
|-----------------|--------|------------------|-----------|
| `statement` | `comment` | `;;` | Yes |
| `statement` | `verb_call` | `(` | Yes |
| `value_parser_with_span` | `boolean_literal_with_span` | `t`, `f` | Yes |
| `value_parser_with_span` | `null_literal_with_span` | `n` | Yes |
| `value_parser_with_span` | `symbol_ref_with_span` | `@` | Yes |
| `value_parser_with_span` | `string_literal_with_span` | `"` | Yes |
| `value_parser_with_span` | `number_literal_with_span` | `0-9`, `-` | Yes |
| `value_parser_with_span` | nested `verb_call` | `(` | Yes |
| `value_parser_with_span` | `list_literal_with_span` | `[` | Yes |
| `value_parser_with_span` | `map_literal_with_span` | `{` | Yes |
| `kebab_identifier` start | `alpha1` | `a-zA-Z` | Yes |
| `kebab_identifier` start | `tag("_")` | `_` | Yes |

**No dead parse arms found.** All 8 value branches are reachable via distinct leading characters. The `alt()` ordering is correct -- specific patterns (boolean, null) are tried before more general ones (string, number).

### Shadowing Analysis

| Potential Shadow | Verdict | Reason |
|------------------|---------|--------|
| Boolean `true` shadows identifier | N/A | Bare identifiers are not a valid value production |
| Null `nil` shadows identifier | N/A | Same -- bare words are not values in this grammar |
| Nested verb_call shadows list/map | No | `(` vs `[` vs `{` are distinct leading chars |
| Number shadows symbol_ref | No | `0-9`/`-` vs `@` are distinct leading chars |
| Number negative `-` shadows keyword `:` | No | `-` vs `:` are distinct leading chars |

---

## 4. Ambiguity Analysis

### Unambiguous by Design

The grammar is LL(1) for the value position -- every value production starts with a unique leading character or character pair. The parenthesized S-expression format eliminates all operator precedence and associativity concerns.

### Edge Cases Analyzed

| Input | Parsed As | Surprising? | Notes |
|-------|-----------|-------------|-------|
| `""` (empty string) | Parse failure | Yes | Known limitation (F-01) |
| `[1 -2]` | List `[1, -2]` | No | No subtraction in grammar |
| `[10-2]` | List `[10, -2]` | Slightly | Parser sees `10` then `-2` as separate values |
| `(a.b :as @x)` | 0 args + binding to `@x` | No | `:as` lookahead works correctly |
| `(a.b :x :as)` | Keyword `:x` with value... fails | No | `:as` is not a valid value |
| `(a.b)` | 0 args, no binding | No | `many0` exits, `opt(as_binding)` returns None |
| `(a.b :x true)` | Keyword `:x` with Boolean true | No | Correct -- `true` matched before string |
| `{:a 1 :b {:c 2}}` | Nested map | No | Recursive map grammar works |
| `;;comment\n(a.b :x 1)` | Comment + verb call | No | Two-statement program |
| `;; trailing no newline` | Valid comment (EOF = implicit end) | No | `opt(char('\n'))` handles this |

### Grammar Completeness Gaps

| Missing Production | Impact | Priority |
|--------------------|--------|----------|
| Empty string `""` | Cannot express empty values | P1 |
| Single-line `; comment` | Users confused by Lisp habit | P3 |
| Heredoc/multiline strings | Long descriptions awkward | P3 |
| Scientific notation (`1.5e10`) | Use string literals as workaround | P3 |

---

## 5. Test Coverage Assessment

### Test Inventory

The parser has **30 inline tests** in `parser.rs:505-1037`:

| Test Name | Productions Exercised |
|-----------|----------------------|
| `test_simple_verb_call` | program, statement, verb_call, word, argument, keyword, string |
| `test_symbol_reference` | symbol_ref, string (alongside symbols) |
| `test_number_literals` | number (integer, negative, decimal, negative decimal) |
| `test_symbol_binding` | as_binding, symbol_ref |
| `test_multiple_statements` | program (multi-statement), verb_call x2 |
| `test_comments` | comment, verb_call (mixed program) |
| `test_boolean_literal` | boolean (true, false) |
| `test_null_literal` | null (nil) |
| `test_nested_verb_calls` | nested_verb_call, verb_call recursion |
| `test_list_literal` | list, string, number, boolean mixed |
| `test_map_literal` | map, map_entry, keyword |
| `test_entity_reference` | string with UUID auto-detection |
| `test_spans_are_absolute_offsets` | span tracking on verb_call |
| `test_multiple_statement_spans` | span tracking across statements |
| `test_argument_spans` | span tracking on arguments |
| `test_string_spans` | span tracking on string literals |
| `test_symbol_ref_spans` | span tracking on symbol_ref |
| `test_nested_verb_call_spans` | span tracking on nested verbs |
| `test_list_span` | span tracking on list literals |
| `test_multiline_spans` | span tracking across line breaks |
| `test_complex_multiline_spans` | span tracking in complex multi-line programs |
| `test_escape_sequences` | string escape sequences (\n, \t, \\, \") |
| `test_deeply_nested_lists` | list recursion (depth 3) |
| `test_nested_maps` | map recursion (depth 2) |
| `test_special_characters_in_strings` | string with special chars (!@#$%^&*) |
| `test_unicode_in_strings` | string with CJK and emoji |
| `test_empty_string` | documents known `""` limitation (tests `" "` instead) |
| `test_empty_list` | empty list `[]` |
| `test_empty_map` | empty map `{}` |
| `test_complex_nested_structure` | deeply nested verb + list + map combined |

### Coverage Matrix

| Grammar Production | Direct Test(s) | Gap? |
|--------------------|----------------|------|
| `program` (0 stmts) | None | **Gap** |
| `program` (1 stmt) | `test_simple_verb_call` | Covered |
| `program` (N stmts) | `test_multiple_statements` | Covered |
| `comment` | `test_comments` | Covered |
| `comment` at EOF | None | **Gap** |
| `comment`-only program | None | **Gap** |
| `verb_call` (0 args) | None | **Gap** |
| `verb_call` (1+ args) | `test_simple_verb_call` | Covered |
| `word` (simple) | `test_simple_verb_call` | Covered |
| `word` (kebab-case) | `test_simple_verb_call` (`create-limited-company`) | Covered |
| `argument` | Multiple tests | Covered |
| `:as` binding | `test_symbol_binding` | Covered |
| `:as` lookahead reject | None | **Gap** |
| `boolean` | `test_boolean_literal` | Covered |
| `boolean` prefix negative | None | **Gap** |
| `null` (nil) | `test_null_literal` | Covered |
| `null` prefix negative | None | **Gap** |
| `symbol_ref` | `test_symbol_reference` | Covered |
| `symbol_ref` (kebab name) | None | **Gap** |
| `string` (basic) | `test_simple_verb_call` | Covered |
| `string` (escapes) | `test_escape_sequences` | Covered |
| `string` (unicode) | `test_unicode_in_strings` | Covered |
| `string` (empty `""`) | Documents failure | Known limitation |
| `string` -> UUID auto | `test_entity_reference` | Covered |
| `number` (integer) | `test_number_literals` | Covered |
| `number` (decimal) | `test_number_literals` | Covered |
| `number` (negative) | `test_number_literals` | Covered |
| `number` (overflow) | None | **Gap** |
| `nested_verb_call` | `test_nested_verb_calls` | Covered |
| `list` (empty) | `test_empty_list` | Covered |
| `list` (values) | `test_list_literal` | Covered |
| `list` (nested) | `test_deeply_nested_lists` | Covered |
| `list` (mixed types) | None | **Gap** |
| `map` (empty) | `test_empty_map` | Covered |
| `map` (entries) | `test_map_literal` | Covered |
| `map` (nested) | `test_nested_maps` | Covered |
| `kebab_ident` (alpha) | Implicit in verb tests | Covered |
| `kebab_ident` (`_` start) | None | **Gap** |
| Error formatting | None | **Gap** |
| `parse_single_verb` | None direct | **Gap** (tested indirectly via viewport_parser) |
| Trailing content reject | None | **Gap** |
| Span tracking | 8 dedicated span tests | Well covered |

### Recommended Test Additions (Priority Order)

1. **Empty program:** `parse_program("").unwrap()` returns `Program { statements: vec![] }`
2. **Zero-arg verb:** `parse_program("(session.clear)")` succeeds with empty arguments
3. **`:as` without symbol:** `parse_program("(a.b :as)")` fails
4. **Empty string failure:** `assert!(parse_program(r#"(a.b :x "")"#).is_err())` documents limitation
5. **Boolean prefix:** Verify `(a.b :x truename)` correctly fails (truename is not a valid value)
6. **Integer overflow:** `parse_program("(a.b :x 99999999999999999999)")` fails gracefully
7. **Symbol with hyphens:** `parse_program("(a.b :x @my-fund)")` succeeds
8. **Mixed-type list:** `parse_program(r#"(a.b :items [1 "a" true @x nil])"#)` succeeds
9. **`parse_single_verb` direct:** Verify standalone use and multi-statement rejection
10. **Trailing content:** `parse_program("(a.b :x 1) garbage")` fails via `all_consuming`
11. **Comment-only program:** `parse_program(";; just a comment\n")` returns 1 Comment statement
12. **Comment at EOF:** `parse_program(";; no newline")` succeeds
13. **Underscore-start identifier:** `parse_program("(_domain._verb :_key 1)")` succeeds
14. **Error format output:** Verify `format_verbose_error` produces line/column/caret

---

## 6. Architecture Observations

### Strengths

1. **Clean two-phase pipeline.** The parser is purely syntactic -- no verb metadata, no DB access. This enables the same parser in LSP, agent, REPL, and test contexts without coupling.

2. **Absolute byte-offset spans.** `nom_locate::LocatedSpan` provides zero-cost span tracking. Every AST node carries absolute byte offsets, enabling precise LSP integration (UTF-16 conversion happens in the LSP layer).

3. **`all_consuming` at API boundary.** Both public functions reject trailing unparsed content, preventing silent partial parses.

4. **`VerboseError` for diagnostics.** The error chain preserves context annotations for human-readable messages with line/column/caret display.

5. **Recursive value grammar.** Nested verb calls, lists of lists, maps of maps -- fully recursive without artificial depth limits.

6. **LSP uses the same parser.** `v2_adapter.rs:parse_with_v2()` calls the same `parse_program()` as the agent pipeline, ensuring consistency between IDE diagnostics and runtime behavior.

### Weaknesses

1. **Single cut point.** Error messages for common mistakes (missing `(`, typo in verb name) are poor because the parser backtracks extensively before failing.

2. **No recovery mode.** The LSP must work around total parse failure on multi-statement files. A recovery parser would improve the authoring experience.

3. **Empty string gap.** A fundamental S-expression value (`""`) fails to parse.

4. **Unsafe in AST traversal.** `find_unresolved_refs` uses `unsafe` to extend a borrow lifetime. Sound but unnecessary -- the `AstVisitor` trait should be redesigned with a lifetime parameter.

5. **No formal grammar document.** The grammar specification above was derived by reading combinators. This creates drift risk as the parser evolves.

---

## 7. Summary Table

| ID | Severity | Category | Summary |
|----|----------|----------|---------|
| F-01 | P1 | Completeness | Empty string `""` fails to parse (`escaped_transform` requires 1+ char) |
| F-02 | P1 | Robustness | Boolean/null prefix matching without word boundary (fragile) |
| F-03 | P2 | Error Quality | Single `cut` point -- poor error locality for common mistakes |
| F-04 | P2 | Error Recovery | No recovery for multi-statement programs (LSP impact) |
| F-05 | P2 | Ambiguity | Map `:keyword` syntax overlaps argument syntax (delimiter-dependent) |
| F-06 | P3 | Clarity | `:as` lookahead idiom is correct but should use clearer `strip_prefix` |
| F-07 | P3 | Consistency | Optional comma in list literals (documented, not harmful) |
| F-08 | P3 | Completeness | Comment requires `;;` -- single `;` silently rejected |
| F-09 | P3 | Safety | `find_unresolved_refs` uses unnecessary `unsafe` pointer cast |
| F-10 | P3 | Safety | `with_resolved_key` panics on non-UUID (documented, try variant exists) |
| F-11 | P3 | Safety | `Span::synthetic()` uses `usize::MAX` sentinel -- overflow risk |

**Test coverage:** 30 inline tests with 14 specific gaps identified (Section 5).
**Dead parse arms:** None found -- all `alt()` branches reachable via distinct leading characters.
**Grammar completeness:** 4 missing productions, 1 at P1 (empty string).
**Ambiguity:** Grammar is LL(1) for values. No true ambiguities found.
