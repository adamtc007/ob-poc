# Repo-Derived Graph Correction Table

Date: 2026-03-11

This table is intentionally conservative. It only records corrections or findings that are provable from the current checked-in codebase.

## A. Checked-in graph verb references

| Graph file | Edge | Current verb IDs | Repo-derived status |
|---|---|---|---|
| `cbu.yaml` | `cbu.create` | `cbu.create`, `cbu.create-from-client-group` | Canonical |
| `cbu.yaml` | `cbu.list` | `cbu.list`, `cbu.read` | Canonical |
| `cbu.yaml` | `cbu.docs` | `document.missing-for-entity`, `document.for-entity` | Canonical |
| `cbu.yaml` | `cbu.ubo` | `ubo.list-ubos`, `ubo.list-owners` | Canonical |
| `deal.yaml` | `deal.read` | `deal.list`, `deal.read-record` | Canonical |
| `deal.yaml` | `deal.progress` | `deal.update-status`, `deal.propose-rate-card`, `deal.counter-rate-card`, `deal.add-rate-card-line` | Canonical |
| `document.yaml` | `document.read` | `document.for-entity`, `document.missing-for-entity` | Canonical |
| `document.yaml` | `document.progress` | `document.solicit`, `document.upload-version`, `document.verify`, `document.reject` | Canonical |
| `entity.yaml` | `entity.create` | `entity.create`, `entity.ensure` | Canonical |
| `entity.yaml` | `entity.read` | `entity.read`, `entity.list`, `entity.list-placeholders` | Canonical |
| `entity.yaml` | `entity.update` | `entity.update` | Canonical |
| `fund.yaml` | `fund.create` | `fund.create`, `fund.ensure`, `fund.upsert-compartment` | Canonical |
| `fund.yaml` | `fund.read` | `fund.list-subfunds`, `fund.list-share-classes`, `fund.list-investors` | Canonical |
| `fund.yaml` | `fund.allocate` | `fund.add-investment` | Canonical |
| `screening.yaml` | `screening.start` | `screening.sanctions`, `screening.pep`, `screening.adverse-media`, `screening.run` | Canonical |
| `screening.yaml` | `screening.review` | `screening.list-by-workstream`, `screening.review-hit`, `screening.complete` | Canonical |
| `ubo.yaml` | `ubo.read` | `ubo.list-owners`, `ubo.list-ubos` | Canonical |
| `ubo.yaml` | `ubo.advance` | `ubo.add-ownership`, `ubo.update-ownership`, `ubo.add-control`, `ubo.add-trust-role` | Canonical |

## B. Fixture/corpus corrections already applied

| Stale expected verb | Current canonical verb |
|---|---|
| `case.open` | `kyc.open-case` |
| `screening.pep-check` | `screening.pep` |
| `screening.sanctions-check` | `screening.sanctions` |
| `screening.media-check` | `screening.adverse-media` |

## C. Fixture/corpus expectations still blocked

| Expected verb | Repo-derived status | Why blocked |
|---|---|---|
| `screening.full` | Unresolved | Current canonical replacement not provable from repo alone |
| `struct.lux.ucits.sicav` | Unresolved | `struct.*` family absent from current registry |
| `struct.ie.ucits.icav` | Unresolved | `struct.*` family absent from current registry |
| `struct.uk.authorised.oeic` | Unresolved | `struct.*` family absent from current registry |
| `struct.us.40act.open-end` | Unresolved | `struct.*` family absent from current registry |
| `struct.lux.pe.scsp` | Unresolved | `struct.*` family absent from current registry |
| `struct.lux.aif.raif` | Unresolved | `struct.*` family absent from current registry |
| `struct.hedge.cross-border` | Unresolved | `struct.*` family absent from current registry |
| `struct.pe.cross-border` | Unresolved | `struct.*` family absent from current registry |

## D. External inputs still required

1. authoritative replacement mapping for `struct.*`
2. authoritative replacement mapping for `screening.full`
3. authoritative generated graph corrections if they differ from the checked-in graph set
4. authoritative phase enum correction table, if any
