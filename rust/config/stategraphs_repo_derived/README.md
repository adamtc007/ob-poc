# Repo-Derived StateGraph Staging

This directory is a staging area for repo-derived reconciliation output.

Current status:
- no graph YAML replacements have been staged here yet

Reason:
- the checked-in graph files under `rust/config/stategraphs/` already reference canonical verbs based on the current repo truth
- the remaining blocked corrections require the external authoritative reconciliation artifacts

Use this directory only when one of the following becomes available:
1. corrected generated graph YAMLs from the external reconciliation run
2. an explicit edge-by-edge correction table that differs from the checked-in graph set

Until then, the repo-derived reconciliation pack is:
- [repo_derived_reconciliation_report.md](/Users/adamtc007/Developer/ob-poc/docs/todo/repo_derived_reconciliation_report.md)
- [repo_derived_graph_correction_table.md](/Users/adamtc007/Developer/ob-poc/docs/todo/repo_derived_graph_correction_table.md)
