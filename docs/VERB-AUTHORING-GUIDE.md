# Verb Authoring Guide

Quick reference for adding new DSL verbs that Claude can discover and use.

## Minimum Viable Verb

```yaml
# config/verbs/{domain}.yaml
domains:
  my-domain:
    verbs:
      my-verb:
        description: "What this verb does"    # Required
        invocation_phrases:                   # REQUIRED for discovery
          - "primary phrase"
          - "alternative phrase"
          - "common abbreviation"
        args:
          - name: required_arg
            type: String
            required: true
          - name: optional_arg
            type: String
            required: false
```

## Invocation Phrases Checklist

Good `invocation_phrases` include:

- [ ] **Primary action phrase**: "create cbu", "add entity"
- [ ] **Natural language variants**: "spin up a fund", "set up client"
- [ ] **Domain jargon**: "onboard", "KYC", "AML"
- [ ] **Abbreviations**: "add sig" for "add signatory"
- [ ] **Common misspellings**: (optional, learning handles this)

## Anti-Patterns

❌ **No invocation_phrases**
```yaml
verbs:
  create:
    description: "Creates something"
    args: [...]
    # Missing invocation_phrases = invisible to verb_search
```

❌ **Too generic**
```yaml
invocation_phrases:
  - "do it"
  - "make"
  - "add"
# Conflicts with every other verb
```

❌ **Only formal language**
```yaml
invocation_phrases:
  - "instantiate client business unit"
# Users say "create cbu" or "spin up fund"
```

## Discovery Priority

```
1. Learned phrases (DB)      ← Corrections from users
2. YAML exact match          ← Your invocation_phrases
3. YAML substring match      ← Partial matches
4. pgvector semantic         ← Embedding similarity (future)
```

Your `invocation_phrases` are the **primary discovery mechanism** until users teach the system their vocabulary.

## After Adding a Verb

```bash
# 1. Restart MCP server (required until hot reload is added)
pkill -f dsl_mcp && ./target/debug/dsl_mcp

# 2. Verify discovery
# In Claude conversation:
verb_search query="your primary phrase"

# Expected: your verb appears with score 1.0, source: phrase_exact
```

## Testing Coverage

Run `verbs_coverage` (future tool) or manually check:

```sql
-- Find verbs without invocation_phrases
SELECT v.domain || '.' || v.name as verb
FROM dsl_verbs v
WHERE NOT EXISTS (
  SELECT 1 FROM verb_invocation_phrases p
  WHERE p.verb_id = v.id
);
```

## Domain Hints

Add `invocation_hints` at domain level for disambiguation:

```yaml
domains:
  trading-profile:
    invocation_hints:        # Keywords that suggest this domain
      - "trading"
      - "custody"
      - "settlement"
      - "counterparty"
    verbs:
      ...
```

When user says "set up trading custody", the hint "trading" + "custody" boosts `trading-profile` domain verbs.

## Complete Example

```yaml
# config/verbs/isda.yaml
domains:
  isda:
    invocation_hints:
      - "isda"
      - "master agreement"
      - "derivatives"
      - "swap"
    verbs:
      create:
        description: "Create an ISDA Master Agreement"
        invocation_phrases:
          - "create isda"
          - "new isda"
          - "set up isda"
          - "add isda master agreement"
          - "create master agreement"
          - "new derivatives agreement"
        args:
          - name: cbu_id
            type: Uuid
            required: true
            description: "The CBU this ISDA belongs to"
          - name: counterparty_id
            type: Uuid
            required: true
            description: "The counterparty entity"
          - name: governing_law
            type: String
            required: false
            description: "Jurisdiction (defaults to English law)"
        
      add-schedule:
        description: "Add a schedule/annex to an existing ISDA"
        invocation_phrases:
          - "add isda schedule"
          - "add schedule to isda"
          - "attach annex"
          - "add isda annex"
        args:
          - name: isda_id
            type: Uuid
            required: true
          - name: schedule_type
            type: String
            required: true
            description: "Type of schedule (Part 1-6, Credit Support Annex, etc.)"
```

## See Also

- `/CLAUDE.md` - Full Claude integration guide
- `/docs/TODO-LEARNING-ENHANCEMENTS-PGVECTOR.md` - Future enhancements including hot reload
