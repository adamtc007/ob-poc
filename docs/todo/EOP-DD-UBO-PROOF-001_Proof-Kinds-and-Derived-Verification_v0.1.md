# EOP-DD-UBO-PROOF-001 — Proof Kinds
### The board collects facts. The policy rules on adequacy.

| | |
|---|---|
| **Document** | EOP-DD-UBO-PROOF-001 |
| **Version** | 0.2 — the requirement table is withdrawn (ruled 2026-08-28) |
| **Binds to** | EOP-VS-UBO-GAME-001 §2, §3.1 (`evidence`/`retract`), §7 (`verification` removed); EOP-VS-UBO-INSPECT-001 §5 (quality is a check; the inspect game owns thresholds) |
| **Status** | DRAFT. §2 (the proof kinds) is now the only ruling surface this document carries. §5 Q1 reaches a previously-ratified ruling and needs Adam. |

---

## §1 What a proof is

**Usually a document. Not always.** A registry lookup performed on a date, a screening provider's response, an analyst's inspection of an original, a recorded call — none is a document reference, and all are proof.

So a proof is not "a document with a type attached". It is **a kind, a source, and a date**:

| Field | Meaning |
|---|---|
| **kind** | what sort of proof this is (§2) |
| **source** | where it came from — a document reference, a registry and the query run, a provider, a person |
| **date** | when it was obtained |

`evidence` logs one. `retract` withdraws one. Nothing sets a status.

## §2 The proof kinds — proposed, for ratification

| Kind | Typical source | Notes |
|---|---|---|
| `registry-extract` | a companies registry, GLEIF, a regulator's register | a lookup, not necessarily a document |
| `constitutional-document` | articles, trust deed, partnership agreement, foundation charter | what the vehicle *is* and how it is governed |
| `share-register` | the register itself, or a certified extract | who holds what |
| `filed-document` | board minute, appointment filing, statutory return | an act recorded with an authority |
| `contract` | management agreement, nominee declaration, mandate | a relationship created by agreement |
| `identity-document` | passport, national identity document | natural persons |
| `attestation` | certification by a regulated third party, or by the client | someone stands behind a fact |

Seven. **This list is the ruling** — add, cut, rename.

## §3 There is no requirement table in the build game — RULED 2026-08-28

**The board collects facts. The policy rules on adequacy.**

`evidence` logs a proof against an assertion. The board reports, per assertion, **what proofs have been logged**. It does not decide whether they are enough — not for a type, not for a shareholding, not for an officer appointment, not for anything.

**Why an earlier draft of this document was wrong.** It proposed a per-assertion requirement table — *voting shares need the register AND the articles* — expressed as alternative satisfying sets. That is a policy parameter sitting in the layer that changes slowest. Adequacy varies with region, product, entity type and risk: the same variables that decide which policies apply. A requirement table here would freeze one standard for every client, which the domain does not have. **It is the same error as building thresholds into the assurance profile**, ruled against on 2026-08-27 for the same reason.

| | |
|---|---|
| **Build game records** | which proofs exist against which assertion — kind, source, date |
| **Inspect game rules** | whether those proofs are adequate, under the policy that applies |

**"Verified" is therefore not a build-game concept at all.** Adequacy is what verification means, and adequacy is policy. The board carries citations; a check reads them and says whether they meet the standard in force. Nothing in the build game declares an assertion proven — only what was obtained, and when.

This makes the tranche **smaller**: no requirement grammar, no alternatives-of-sets, no per-pipe table. Those become check content, which the inspect V&S already places out of architectural scope.

## §4 What follows for the code

**`EdgeStatus::Verified` goes**, and `TypeProofStatus::Proved` stops being a *state*. What replaces them is the citation set itself — proofs per assertion, each with kind, source and date.

**The assurance profile stays and gets more honest.** It reports alleged-versus-verified today, which is a judgment wearing a measurement's clothes. It should report **what proofs exist**: this edge has a share register dated March; this type has a registry extract dated January; this one has nothing. Factual, readable by a policy check, and useful to an operator.

**Demotion needs no mechanism.** `retract` removes a proof, the citation set shrinks, and any check reading it re-decides. There is no state to demote.

## §5 Gate tests (RED first)

- `the_board_reports_proofs_not_adequacy` — **structural, and the point of the tranche**: no build-game path returns a verified/proven boolean for an assertion. Grep-proof and type-level; the concept is absent, not merely unused.
- `evidence_logs_kind_source_and_date` — all three recorded and readable.
- `retract_removes_a_proof_and_nothing_else` — the set shrinks; no status changes, because none exists.
- `assurance_reports_the_citation_set` — the profile names which proofs exist per assertion, with kinds and dates, not a verdict.
- `no_move_sets_verified` — no verb writes a proof status.
- `proof_kinds_are_exhaustive` — a new kind is a compile error until ruled.

## §6 Open questions

**Q1 — TS.2 Ruling 2f, and it needs Adam because it was ratified.** That ruling gates `freeze` on mandate contract evidence: a fund pivot with no contract computes provisionally but may not freeze. Under §3, "has enough evidence" is a policy judgment — so either that gate is a deliberate build-game exception, or it moves to the inspect game and `freeze` stops gating on evidence entirely. **This ruling reaches a ratified one; not deciding it silently.**
**Q2 — §2's seven kinds.** Now the only vocabulary this document contributes, so it carries more weight than it did.
**Q3 — Does a proof's source need structure?** Free text today. A registry lookup arguably wants the registry named and the query recorded so it can be repeated. *Recommendation: free text now; structure when a check needs to read it.*

## Change Log
| Version | Date | Note |
|---|---|---|
| 0.1 | 2026-08-28 | Proof kinds, plus a per-assertion requirement table expressed as alternative satisfying sets. |
| 0.2 | 2026-08-28 | **The requirement table is withdrawn.** RULED: the board collects facts; the policy rules on adequacy. Adequacy varies with region, product, entity type and risk — the same variables that decide which policies apply — so a table here would freeze one standard for every client. Same error as building thresholds into the assurance profile, ruled against a day earlier for the same reason. Consequences: **"verified" is not a build-game concept**, `EdgeStatus::Verified` and `TypeProofStatus::Proved`-as-state go, the citation set replaces them, and the assurance profile reports which proofs exist rather than a verdict. Demotion needs no mechanism. Materially smaller than v0.1. Q1 raises the one ratified ruling this reaches — `freeze`'s unevidenced-mandate gate. |
