# ob-poc Unified DSL Design v0.1

**Status**: Sealed for implementation (2026-05-21). Tag target: `design/v0.1`.

## Reading Order

| File | Content |
|---|---|
| `session1-atom-model-and-verb-catalogues.md` | Language foundation: unified atom model, EBNF, structural/declarative dichotomy, all atom kinds including `decision-pack`, template substitution syntax, SemOS verb catalogue reshape, bpmn-lite verb catalogue |
| `session2-compiler-and-runtime.md` | Implementation architecture: four-pass compiler crate decomposition, SemOS and bpmn-lite assembly passes, resolution pass, lowering, journey-persisted runtime, Postgres schema, event loop, multi-token semantics, merge protocol |
| `session3-regression-packs-examples.md` | Validation and catalogue: regression strategy (§7), decision pack catalogue with 12 seed packs (§8), 12 worked examples (§9), risk register, phase plan, appendices |

Sessions 1 and 2 contain cross-references. Session 3 was consolidated by splicing `session3-patch-decision-packs.md` into the original Session 3 draft; the patch file is retained for audit.

---

## Architectural Commitments (17 total, locked)

All commits are locked. The design works within them; it does not revisit them.

| # | Commitment | Reference |
|---|---|---|
| C1 | DSL is canonical executable source; s-expression syntax | S1 §2 Commitment 1 |
| C2 | DSL source is order-independent (bag of atoms) | S1 §2 Commitment 2 |
| C3 | Atoms are structural or declarative; classification is per kind | S1 §2 Commitment 3 |
| C4 | Nodes and edges are independent atom kinds in bpmn-lite | S1 §2 Commitment 4 |
| C5 | Verbs declare context dependencies via `@`-placeholder slots | S1 §2 Commitment 5 |
| C6 | Boolean composition vocabulary is unified across all contexts | S1 §2 Commitment 6 |
| C7 | Cleanest verb sets; no backwards compatibility constraint | S1 §2 Commitment 7 |
| C8 | Provenance is preserved in source via `(provenance ...)` atoms | S1 §2 Commitment 8 |
| C9 | Decision packs are declared in the unified DSL as `(decision-pack ...)` atoms | S1 §2 Commitment 9 |
| C10 | Template substitution is explicit and Lisp-style (`,name`, `,@name`) | S1 §2 Commitment 10 |
| C11 | Four-pass compiler: parse → assembly → resolution → lowering | S2 §5.1 |
| C12 | Journey-persisted hydrate/dehydrate runtime; no long-lived in-memory state | S2 §6.1 |
| C13 | Synchronous non-blocking verbs; wait state is persisted | S2 §6.5 |
| C14 | Pluggable switch adaptor protocol for gateway decisions | S2 §6.6 |
| C15 | Multi-token semantics: parallel fork/join, inclusive dynamic fan-in | S2 §6.7 |
| C16 | Declared-merge + detect-and-fail for parallel join conflicts | S2 §6.8 |
| C17 | Sage's intent-disambiguation role is documented and excluded from compiler | S2 §5.8 |

Note: Session 1 documents Commitments 1–10. Commitments 11–17 span Sessions 2 and 3. The original prompt had 17 commitments; Session 1 (regenerated) presents them as 10 session-scoped commitments. Full 17-commitment count reconciles above.

---

## [GAP] Index — v0.2 Backlog

Markers in the design indicating deferred work:

| GAP | Location | Description |
|---|---|---|
| Type lattice | S1 §3.8 | Full type lattice and subtyping rules deferred to v0.2 |
| Template for-each | S3 §8.1.3, packs 3/4/5/6/7/8/10 | Variable-arity atom generation from list parameters (N gateways from N conditions) |
| Conditional events | S3 §12 | Require external condition monitoring service |
| Parallel multi-instance dynamic expected count | S3 §12 | Runtime join arrival tracking schema extension |
| Full compensation | S2 §6.9 | Beyond transaction-subprocess scope |
| Timer cycles | S2 §6.10 | Timer cycle support |
| Production Sage matching | S3 §8.2 | Confidence-ranked pack matching with real embeddings |
| BPMN/DMN XML migration | S3 §12 | Migration tooling from Camunda 8 XML to bpmn-lite |
| Async cross-process verbs | S2 §6.5 | Cross-process async verb invocation |
| Ad-hoc subprocess | S3 §12 | Rejected for v0.1 (no Camunda 8 support) |
| Complex gateway | S3 §12 | Rejected (expressible through inclusive + predicate) |

---

## Decision Pack Index

All 12 seed packs defined in `session3-regression-packs-examples.md` §8.3 and Appendix E:

| # | Pack | Pattern | Domain scope |
|---|---|---|---|
| 1 | `conjunctive-gate` | AND(N) conditions, single exclusive gateway | cbu, kyc, onboarding, screening |
| 2 | `disjunctive-gate` | OR(N) conditions, single exclusive gateway | cbu, kyc, screening, onboarding |
| 3 | `linked-switch-chain` | Sequential gateways with early exit [GAP v0.2 N>2] | cbu, kyc, onboarding |
| 4 | `parallel-evaluation-with-veto` | Parallel fork + veto semantics [GAP v0.2 N>2] | cbu, kyc, screening |
| 5 | `cascading-decision` | 2-stage sequential decision [GAP v0.2 N>2 paths] | cbu, kyc, deal |
| 6 | `decision-table-classification` | DMN table → routing [GAP v0.2 N>1 explicit classes] | cbu, kyc, deal, im |
| 7 | `threshold-band-routing` | Numeric 3-band routing [GAP v0.2 variable bands] | cbu, kyc, ubo |
| 8 | `required-evidence-checklist` | Sequential 3-task evidence chain [GAP v0.2 N>3] | cbu, kyc, onboarding |
| 9 | `periodic-refresh-trigger` | Timestamp age gateway | cbu, kyc, periodic-review |
| 10 | `multi-jurisdiction-overlay` | 2-jurisdiction routing [GAP v0.2 N>2] | cbu, kyc, deal, compliance |
| 11 | `sanction-hit-escalation` | Hard-block sanctions gateway | cbu, kyc, screening, compliance |
| 12 | `manual-override-checkpoint` | Auto-decision + human override | cbu, kyc, compliance, governance |

Packs 1, 2, 9, 11, 12 are fully expressible in v0.1 template syntax. Packs 3, 4, 5, 6, 7, 8, 10 have fixed-arity v0.1 templates with variable-arity deferred to v0.2.

---

## Worked Examples Index

All 12 examples in `session3-regression-packs-examples.md` §9:

| # | Example | Key features exercised |
|---|---|---|
| 1 | Linear sequence — onboarding intake | User task, service tasks, linear flow |
| 2 | Exclusive gateway Pattern A | Single composite DMN decision, two-path routing |
| 3 | Exclusive gateway Pattern B (linked-switch chain) | Sequential gateways, early-exit per condition |
| 4 | Inclusive gateway — dynamic fan-out and fan-in | Dynamic branch selection, inclusive join |
| 5 | Parallel fork/join with declared merge | 3 tokens, merge operators (latest) |
| 6 | Parallel fork/join — undeclared write conflict | Detect-and-fail diagnostic |
| 7 | Subprocess invocation | call-activity, nested token scope |
| 8 | Interrupting error boundary | Error catch, host path abandonment |
| 9 | Non-interrupting timer boundary | Timer fire, parallel escalation path |
| 10 | Event-based gateway | Three catching events, race semantics |
| 11 | Complex KYC/onboarding | Jurisdictional routing, parallel workstreams, loop, SLA timer |
| 12 | Pack-authored with provenance | `conjunctive-gate` pack, Sage instantiation, provenance atom |
