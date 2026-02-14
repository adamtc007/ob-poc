# TODO — Enhance Semantic OS: Entity-Kind → Allowed Ops (Verb Linkage) + Evidence-Driven Instance Maps

**Repo:** `ob-poc`  
**Objective:** Make the Semantic OS *decisively* constrain verb selection using **entity kind** (CBU/Company/Person/Fund/Document/…) and **phase**, powered by an online evidence layer (aliases + focus).  
This delivers: **entity hit → entity type → valid ops**, with SemReg as the contract/policy source of truth.

---

## 0) Target behavior

Given an utterance in a scoped session:

1) Resolve **dominant entity** (id + kind) via evidence-driven maps + linker.
2) Use SemReg to compute **allowed verbs** for (kind, phase, scope, actor).
3) Verb discovery is restricted/boosted so only relevant ops appear.
4) ClarifyEntity occurs when ambiguous; selection strengthens alias weights.

---

## 1) SemReg contract extensions (stable, snapshot-published)

### 1.1 Extend VerbContractBody with entity-kind applicability
File(s): `rust/src/sem_reg/verb_contract.rs` (+ YAML scanner)

- [ ] Add fields (serde + schema):
  - `subject_kinds: Vec<EntityKind>`  (required, can be broad initially)
  - `phase_tags: Vec<String>`         (optional; e.g. ubo_discovery, docs, review)
  - `requires_subject: bool`          (default true)
  - `produces_focus: bool`            (whether this verb should update session focus)

**Acceptance**
- Contracts can formally declare “this verb applies to CBU/person/company…”

### 1.2 Update YAML verb registry → SemReg scanner to populate these fields
File: `rust/src/sem_reg/scanner.rs` (or wherever YAML scan occurs)

- [ ] Update YAML format (if needed):
```yaml
fqn: bny.kyc.document.request
subject_kinds: [Person, Company]
phase_tags: [docs]
requires_subject: true
```
- [ ] Publish successor snapshots when this metadata changes (already supported)

**Acceptance**
- SemReg snapshots contain subject_kinds for all core verbs (start with top 100).

---

## 2) SemReg context resolution: compute allowed verbs by entity-kind & phase

### 2.1 Add “verb applicability filter” in SemReg resolution
File: `rust/src/sem_reg/context_resolution.rs`

- [ ] When building allowed verb set:
  - Filter by `subject_kinds` if `SubjectRef::EntityId` is present
  - Filter/boost by `phase_tags` (phase comes from run_sheet state or case model)
- [ ] Output:
  - `allowed_verbs: HashSet<String>`
  - `boosted_verbs: Vec<(verb_fqn, weight, reason)>`

**Acceptance**
- resolve_context returns a non-trivial allowed set when a dominant kind is known.

### 2.2 Define Phase signal (minimal v0)
- [ ] Choose where phase comes from:
  - simplest: derived from run_sheet (recent verbs imply phase)
  - better: explicit `case.phase` field in DB
- [ ] Add helper: `fn derive_phase(run_sheet) -> PhaseTag`

**Acceptance**
- Phase is stable and deterministic.

---

## 3) Evidence-driven instance semantics (online maps; NOT snapshots)

### 3.1 Add `entity_aliases` table (learned mapping)
Migration: `agent.entity_aliases` (or `entity.entity_aliases`)

**Columns**
- `scope_id uuid`
- `alias_norm text`
- `entity_id uuid`
- `entity_kind text`
- `weight double precision`
- `last_seen_at timestamptz`
- `source text` (`user_choice|import|heuristic|system`)
- `evidence jsonb`

Indexes:
- `(scope_id, alias_norm)`
- `(entity_id)`

**Acceptance**
- Fast lookup of alias → top entity candidates.

### 3.2 Add `session_focus` (dominant entity)
Migration: `agent.session_focus`

- `session_id uuid primary key`
- `focus_entity_id uuid`
- `focus_entity_kind text`
- `confidence double precision`
- `updated_at timestamptz`
- `evidence jsonb`

**Acceptance**
- Session carries a deterministic “current focus entity” used for subsequent turns.

### 3.3 Add Entity linker scoring using evidence
File: `rust/src/agent/entity_linker.rs` (or existing service)

- [ ] Input: utterance + scope + session_id
- [ ] Candidate sources:
  - alias map hits (weighted)
  - recent focus entity (recency boost)
  - existing entity search/index (fallback)
- [ ] Output: ranked candidates with evidence
- [ ] Choose dominant if top score gap > threshold, else ClarifyEntity

**Acceptance**
- Common aliases resolve deterministically after a few uses.

---

## 4) Pipeline wiring: entity-kind hard filter before verb selection

### 4.1 Orchestrator stage order
File: `rust/src/agent/orchestrator.rs`

- [ ] Ensure order:
  1) entity linking → dominant entity id+kind (or ClarifyEntity)
  2) SemReg resolve_context(subject=entity_id) → allowed verbs
  3) verb discovery → filter by allowed set and/or subject_kinds
  4) choose verb → forced verb generation → stage DSL

**Acceptance**
- With a known CBU focus, verb candidates are mainly cbu.* and relevant kyc.* etc.

### 4.2 Update forced-verb path to require applicability
- [ ] When user selects a verb in ClarifyVerb:
  - confirm it is in SemReg allowed set for current subject/kind (strict mode)
  - otherwise return NoAllowedVerbs/invalid selection

**Acceptance**
- User cannot force a verb that doesn’t apply to current entity kind.

---

## 5) Learning loop: ClarifyEntity strengthens alias weights

### 5.1 Add ClarifyEntity DecisionPacket
- [ ] New outcome kind: `ClarifyEntity`
  - includes candidate entities (id, label, kind, score)
- [ ] Reply path selects entity → sets session_focus → continues pipeline

### 5.2 Update alias weights on confirmation
- [ ] On successful ClarifyEntity selection:
  - upsert `entity_aliases(scope_id, alias_norm, entity_id)` increasing weight
  - decay competitors for same alias_norm in that scope
- [ ] Record event in `intent_events` (or new `entity_link_events`)

**Acceptance**
- The system “learns” that in Allianz scope, “Acme” means a specific entity.

---

## 6) Telemetry additions (small)

### 6.1 Extend `intent_events` with entity fields (if not already)
- `dominant_entity_id`
- `dominant_entity_kind`
- `entity_candidates_pre` (jsonb)
- `entity_selection_source` (`alias|focus|search|user_choice`)

**Acceptance**
- You can measure ambiguity rate and learning improvements over time.

---

## 7) Scenario harness additions (validate the two-pronged effect)

Add ~10 scenarios to your harness suite:

1) “Inside CBU focus, verbs narrow to cbu.*”
2) “Same utterance with Person focus yields person.* verbs”
3) “Ambiguous ‘Acme’ triggers ClarifyEntity”
4) “After choosing Acme once, next time resolves automatically”
5) “Wrong-kind verb selection rejected in strict mode”
6) “Phase change shifts verb boosts (docs vs ubo)”
7) “CBU + ‘request passport’ boosts document.request with subject person”
8) “Alias differs by scope (Allianz vs BlackRock)”
9) “Recency boost: last-focused entity dominates”
10) “No entity mentioned uses session_focus”

---

## 8) “Done” checklist

- [ ] SemReg contracts include subject_kinds for core verbs
- [ ] SemReg resolve_context filters/boosts verbs by entity kind + phase
- [ ] entity_aliases + session_focus exist and are used by entity linker
- [ ] Orchestrator uses entity-kind gating before verb selection
- [ ] ClarifyEntity loop updates alias weights (learning)
- [ ] Harness scenarios prove verbs narrow based on entity kind

