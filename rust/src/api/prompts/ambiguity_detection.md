# Ambiguity Detection Rules

## When to Ask for Clarification

Before generating intents, check for ambiguous patterns. If detected, return a clarification request instead.

### Ambiguous Patterns

1. **Name vs Keyword confusion**:
   - `Reg`, `reg` adjacent to jurisdiction → could mean "region" or be part of name
   - `In`, `in` adjacent to location → could mean "in [place]" or be part of name
   - `For`, `for` adjacent to market → could mean "for [market]" or be part of name

2. **Multiple entity matches**:
   - When a name could match multiple existing entities
   - When disambiguation is needed between similar names

3. **Implicit vs explicit scope**:
   - "Add product" without specifying which CBU
   - "Create director" without specifying which entity

### Unambiguous Patterns (DO NOT ask for clarification)

- Quoted names: `"Apex Capital" in LU` → name is clearly "Apex Capital"
- Explicit keywords: `name: Apex Capital, jurisdiction: LU`
- Full country names: `Apex Capital in Luxembourg`
- Clear context from session: if @cbu is bound, "add product" applies to it

## Clarification Response Format

When ambiguity is detected, include in your response:

```json
{
  "needs_clarification": true,
  "ambiguity": {
    "type": "name_parsing" | "entity_match" | "missing_context",
    "original_text": "the ambiguous input",
    "interpretations": [
      {"option": 1, "interpretation": "...", "description": "..."},
      {"option": 2, "interpretation": "...", "description": "..."}
    ],
    "question": "A clear question for the user"
  },
  "intents": [],
  "explanation": "I need clarification before proceeding."
}
```

## Examples

**Ambiguous**: "Create Apex Capital Reg LU"
→ Ask: "Does 'Reg' belong to the name 'Apex Capital Reg', or did you mean region 'LU'?"

**Unambiguous**: "Create 'Apex Capital' in LU" 
→ Proceed with name="Apex Capital", jurisdiction="LU"

**Ambiguous**: "Add John as director"
→ Ask: "Add John as director of which entity? Please specify the CBU or company."

**Unambiguous** (with context): Session has @cbu bound
→ Proceed with adding director to @cbu
