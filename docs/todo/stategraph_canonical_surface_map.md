# StateGraph Canonical Surface Map

## Purpose
Normalize the consolidated StateGraph TODO against the current live verb surface before graph authoring.

## Canonical mappings

- `deal.get` -> `deal.read-record`
- `deal.update-status` -> `deal.update-status`
- `cbu.role.assign` -> `cbu.role.assign`
- `document.missing-for-entity` -> `document.missing-for-entity`
- `document.for-entity` -> `document.for-entity`
- `fund.create` -> `fund.create`
- `entity.list-placeholders` -> `entity.list-placeholders`
- `screening.sanctions` -> `screening.sanctions`
- `screening.pep` -> `screening.pep`
- `screening.adverse-media` -> `screening.adverse-media`
- `ubo.registry.advance` -> not present in current canonical surface; do not reference in graph edges without first adding or remapping it

## Notes

- The current StateGraph implementation should use only canonical IDs from `rust/config/verbs/*.yaml`.
- No stale aliases should be introduced into graph files, validation, or harness expectations.
- Edge-verb validation must reject any graph file referencing non-canonical IDs.
