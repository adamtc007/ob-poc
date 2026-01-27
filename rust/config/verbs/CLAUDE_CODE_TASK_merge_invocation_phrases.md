# Claude Code Task: Merge Invocation Phrases into Verb YAML Files

## Objective
Merge invocation phrases from `_invocation_phrases_draft.yaml` and `_invocation_phrases_extension.yaml` into the corresponding domain YAML files under `/Users/adamtc007/Developer/ob-poc/rust/config/verbs/`.

## Context
The semantic intent matching system needs 8-15 invocation phrases per verb for BGE embedding model to work effectively. Two draft files contain the phrases to be merged:
- `_invocation_phrases_draft.yaml` - phrases for view, ownership, fund, ubo domains
- `_invocation_phrases_extension.yaml` - phrases for trading-profile, entity, control, gleif, cbu, graph, bods, session, cbu-role-v2, client-group domains

## Rules

1. **Location**: Add `invocation_phrases:` as a property of each verb, immediately after `description:` if present, otherwise right after the verb key.

2. **Format**: 
```yaml
      verb-name:
        description: "..."
        invocation_phrases:
          - "phrase one"
          - "phrase two"
        behavior: ...
```

3. **Skip if exists**: If a verb already has `invocation_phrases:`, do NOT overwrite or duplicate.

4. **Domain to file mapping**:
   - `view` → `view.yaml`
   - `ownership` → `ownership.yaml`
   - `fund` → `fund.yaml`
   - `ubo` → `ubo.yaml`
   - `trading-profile` → `trading-profile.yaml`
   - `entity` → `entity.yaml`
   - `control` → `control.yaml`
   - `gleif` → `gleif.yaml`
   - `cbu` → `cbu.yaml`
   - `graph` → `graph.yaml`
   - `bods` → `bods.yaml`
   - `session` → `session.yaml`
   - `cbu-role-v2` → `cbu-role-v2.yaml`
   - `client-group` → `client-group.yaml`

5. **Validation**: After each file update, validate YAML syntax with `python3 -c "import yaml; yaml.safe_load(open('filename.yaml'))"`

6. **Skip missing verbs**: If a verb from the draft files doesn't exist in the target YAML, skip it silently (the draft was generated speculatively).

## Source Files Structure

Both draft files have this structure:
```yaml
domain-name:
  verb-name:
    invocation_phrases:
      - "phrase 1"
      - "phrase 2"
      ...
```

## Target File Structure

Target YAML files have this structure:
```yaml
domains:
  domain-name:
    description: "..."
    verbs:
      verb-name:
        description: "..."
        # <-- INSERT invocation_phrases HERE
        behavior: ...
        metadata: ...
        args: ...
```

## Execution Steps

1. Parse `_invocation_phrases_draft.yaml` and `_invocation_phrases_extension.yaml`
2. For each domain/verb with phrases:
   a. Open the corresponding target YAML file
   b. Find the verb definition under `domains.{domain}.verbs.{verb}`
   c. Check if `invocation_phrases` already exists - skip if so
   d. Insert `invocation_phrases` list after `description` field
   e. Preserve all existing indentation (typically 8 spaces for verb properties)
3. Validate each modified file
4. Report summary: verbs updated per file, any skipped verbs

## Expected Output

After completion:
- ~141 verbs should have new invocation_phrases added
- All modified YAML files should pass validation
- Priority domains (view, ownership, fund, ubo) should have 100% coverage
- Trading-profile should have ~47 verbs with phrases

## Files Location
```
/Users/adamtc007/Developer/ob-poc/rust/config/verbs/
├── _invocation_phrases_draft.yaml      # SOURCE - priority domains
├── _invocation_phrases_extension.yaml  # SOURCE - sparse domains  
├── view.yaml                           # TARGET
├── ownership.yaml                      # TARGET
├── fund.yaml                           # TARGET
├── ubo.yaml                            # TARGET
├── trading-profile.yaml                # TARGET
├── entity.yaml                         # TARGET
├── control.yaml                        # TARGET
├── gleif.yaml                          # TARGET
├── cbu.yaml                            # TARGET
├── graph.yaml                          # TARGET
├── bods.yaml                           # TARGET
├── session.yaml                        # TARGET
├── cbu-role-v2.yaml                    # TARGET
└── client-group.yaml                   # TARGET
```

## Cleanup After Success
- Delete `merge_phrases.py` (failed attempt)
- Keep `_invocation_phrases_draft.yaml` and `_invocation_phrases_extension.yaml` as reference/audit trail
