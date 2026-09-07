# EOP-DD-UBO-CLEANOUT-001 — Clean Start
### Rip the old vocabulary out entirely. No epoch, no tolerance, no lurking.

| | |
|---|---|
| **Document** | EOP-DD-UBO-CLEANOUT-001 |
| **Version** | 0.1 — draft for ratification |
| **Binds to** | EOP-VS-UBO-GAME-001 (the eight moves); the rip-and-replace tranche plan (T0–T6) |
| **Runs** | LAST — after the vocabulary replacement lands. Clearing data while the definition of "current" is still moving means doing it twice. |
| **Status** | DRAFT. §3 is the ruling surface. §4's decision-record question must be settled before anything is cleared. |

---

## §0 The context this rests on

**This is development. Not production, not UAT.** Nothing here is a customer record, a regulatory artifact, or evidence of anything. The events are test data written by test sessions against a dev database.

**So archiving and logging matter only where they are current.** A log of what the system does *now* is a working tool. A log of what a retired vocabulary did is not history worth keeping — it is material that makes present-day questions harder to answer, and the only argument for keeping it is the reflex that data should be kept.

That reflex belongs to production. Applying it here has already cost this programme: 707 unfoldable events made "can this system replay itself" unanswerable, and a reconciliation session had to establish that the answer was *irrelevant* rather than *no*. **Nothing about that data was worth the sessions spent explaining it.**

Every ruling below follows from that. When this system has real data, the disciplines change — append-only, immutable, nothing discarded. Until then, the discipline is that the development environment should contain only what is true today.

## §1 Why a clean start rather than an epoch

An epoch **tolerates** the old stream — declares a boundary and leaves everything before it in place, unfoldable and inert. A clean start **removes** it.

The epoch was the earlier recommendation and it was the weaker one. Three reasons to go further:

**The old data poisons the measurements.** 707 of 710 events cannot be folded under the current registry, which makes "can this system replay its own history" unanswerable rather than answerable-with-a-caveat. Retired FQNs remain live in the search index, so an operator utterance still resolves to a dead verb. Boards exist that nothing can construct. Every one of those is noise a future reconciliation has to explain away, and this programme has already spent sessions doing exactly that.

**The unwind property should hold over everything.** R8's acceptance test — walk the logged move stack backwards and assert every inverse is legal — is a much stronger claim over a whole stream than over a suffix. With an epoch, "the board is unwindable" carries an asterisk forever.

**There is nothing to preserve.** This is a POC with test data. The events were written under three retired vocabularies across as many renames; 691 of 710 carry names that no longer exist. Writing historical fold implementations to recover a stream that produced **zero** evaluation runs and two registry rows is cost with no return.

## §2 What "clean" means

Nothing anywhere refers to, resolves to, or was produced by the old vocabulary. Specifically:

| Surface | What goes |
|---|---|
| Event stream | all `kyc_intent_events` rows and their subject streams |
| Boards | every constructed board and any cached construction |
| Search index | `verb_pattern_embeddings`, `verb_centroids`, `dsl_verbs` rows for retired FQNs |
| Manifests | `kyc_lexicon_manifest` rows for lexicon versions nothing can reconstruct |
| Dead tables | `kyc_ubo_evidence` (0 rows, keyed on the deleted `entity_ubos` path) |
| Dead machinery | `fold/obligation.rs` — 317 lines of a dissolved concept still in the seam's signature |
| Fixtures | every test carrying an old FQN, **including the fuzz target's deliberate retired-name list** — that list exists to exercise historical-event fold safety, which is a property this document removes |
| Vocabulary residue | `structure-class` and everything it drags (T4), retired FQNs still claimed by packs |
| Docs | superseded design docs marked as such rather than left readable as current |

**The test: after this runs, a grep for any retired FQN across source, config and the database returns nothing but deliberate historical notes in change logs.**

## §3 The rulings

**C1 — Clear, do not archive.** No parallel table of old events, no `_archive` suffix, no commented-out fixtures. An archive is a lurking place, and the reason for this document is that lurking poisons the well.

**C2 — Delete the retired-name fuzz list.** It exists to prove old events still fold safely. After a clean start there are no old events, and keeping the list makes the fuzzer assert a property the system no longer has.

**C3 — Registry holds exactly one hash, and that is now true rather than aspirational.** Today `FoldRegistry` registers one hash while the stream carries sixteen. After this, one hash and one vocabulary — and `UnregisteredLexiconHash` becomes an error that genuinely cannot fire rather than one that fires 707 times.

**C4 — The clean-out is itself gated.** A test asserting no retired FQN appears in source, config, or the three index tables. It runs from then on, so the next rename cannot leave residue — which is what the previous three did.

**C5 — Superseded documents are marked, not deleted.** Design docs record how decisions were reached and that history is worth keeping. But a superseded document must say so in its header, or someone reads it as current — which has already happened once in this programme.

## §4 The one thing to settle before anything is cleared

**`kyc_decision_records` holds 5 rows, and `kyc_decisions` 3.** These are *decisions*, and I4 rules that a verdict cites its basis. If any of those cites a determination whose stream is about to be cleared, the citation dangles — a decision of record pointing at nothing.

The prior review flagged this and explicitly did not settle it, because settling it needs a fold attempt per subject rather than a query.

**Ruling needed:** clear them with everything else (they are test decisions on test data, and a dangling citation is worse than no record), or preserve them and accept the dangle. *Recommendation: clear them.* A decision whose basis cannot be reconstructed is not evidence of anything, and preserving it teaches a reader that dangling citations are tolerable.

## §5 Sequence and gates

**Runs after the last vocabulary tranche.** Not alongside — the definition of "current" must have stopped moving.

1. Settle §4. 2. Clear the database surfaces. 3. Delete the dead machinery and fixtures. 4. Land C4's gate. 5. Re-run everything.

**Gates:**
- `no_retired_fqn_anywhere` — source, config, and the three index tables. The permanent guard.
- `registry_hash_count_is_one` — C3 made checkable.
- `fold_registry_has_no_unregistered_hashes_in_the_stream` — every event in the stream folds. Today this fails 707 times; after this it is trivially true and stays true.
- `no_archive_surfaces` — structural: no table, file or fixture holds retired-vocabulary data.
- The full regression, the fuzz harness, and **R8's unwind over the whole stream** — which is the point: after a clean start, "every board is unwindable" holds without an asterisk.

## Change Log
| Version | Date | Note |
|---|---|---|
| 0.1 | 2026-08-27 | Initial draft. Replaces the earlier epoch recommendation with a **clean start** — the epoch tolerates 707 unfoldable events and leaves the unwind property carrying a permanent asterisk. Nine surfaces enumerated, including the fuzz target's retired-name list (C2) and `fold/obligation.rs` (317 lines of dissolved concept in a live signature). C1 forbids archiving, because an archive is a lurking place. C4 makes the clean-out a standing gate rather than a one-off, since the previous three renames each left residue. §4 raises the one thing that must be settled first: five decision records whose cited determinations are about to become unreconstructible. |
