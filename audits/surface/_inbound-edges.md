# Inbound edges (split-completeness) — HEAD=86031a08098ee6b86f2f5c3a07acf3ab929d9c3c

Direct (depth-1) inbound workspace dependents only — `cargo tree -i <pkg> --workspace --depth 1`.
HUB-ONLY = yes when the only direct inbound workspace crate is `ob-poc` (cosmetic split, namespace not boundary).

| crate | direct inbound workspace crates | HUB-ONLY? |
|---|---|---|
| ob-poc-boundary | ob-poc,ob-poc-agent,xtask | no |
| ob-poc-sage | ob-poc,ob-poc-agent | no |
| ob-poc-journey | ob-poc,ob-poc-agent,ob-poc-boundary | no |
| ob-poc-agent | xtask | no |
| ob-poc-authoring | ob-poc | yes |
| ob-poc-bods | ob-poc | yes |
| ob-poc-deal | ob-poc | yes |
| ob-poc-trading-profile | ob-poc | yes |
| ob-poc-taxonomy | ob-poc | yes |
| ob-poc-ontology | ob-poc | yes |
| ob-poc-semtaxonomy | ob-poc | yes |
| ob-poc-derived-attributes | ob-poc | yes |
| ob-poc-entity-linking | ob-poc | yes |
