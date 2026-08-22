# EOP-VS-CONTROLPLANE-001 — AMENDMENT 1 (v0.3 → v0.4)
### The Clearing-House Mandate: Mediation Topology, Leakproof Coverage, Pack Universality
### Status: DRAFT for architect ratification. On ratification, sonnet applies these deltas to the repo copy mechanically (change log entry, new §15, §8 replacement, §12 additions) and the document becomes v0.4.

---

## Architect directive (recorded verbatim in substance)

> The Control Plane is the ONLY clearing house between the agents (Sage, REPL, and any future agent surface) and the guts of the solution — DSL, DAG, SemOS, runtime, stores, and every other capability. To be auditable it must be a leakproof, 100%-coverage gateway, and it MUST cover all packs.

This amendment converts that directive into enforceable architecture. Everything in v0.3 — gates, proofs, envelope lifecycle, snapshot semantics, shadow→enforce graduation — is unchanged. What changes is **topology ownership**: v0.3 described a control plane inserted into an existing pipeline (checkpoint); v0.4 makes the control plane the pipeline (mediator).

---

## NEW §15 — Target Topology: The Clearing House

### 15.1 Mediation, not interception

The Control Plane intercepts at the **utterance** — before parsing, before pack/domain selection — and owns the sequence from that point. The former pipeline stages become capabilities it invokes:

- Sage is not upstream proposing intents to the Control Plane; Sage is an **interpretation capability** the Control Plane invokes with the utterance and the context it grants.
- Pack/domain selection happens **inside** the clearing house (G3 is not consulted about a resolution made elsewhere; it IS the resolution point).
- Compilation, DAG proof, authority, evidence, execution: delegated calls from the hub, returning borrowed proofs (§4/§9.1 unchanged — the decision-assembler law was always hub-shaped; this section makes the call topology match the decision topology).

No capability calls another capability laterally on an agent-originated flow. The point-to-point mesh is not gated; it is **retired**.

### 15.2 Leakproof, defined structurally

"Leakproof" is a compile-time property, not an audit finding:

- **L1 — Dependency direction lock.** Agent crates carry zero dependency edges to capability crates except via `ob-poc-control-plane`. Enforced by a CI dependency-graph gate (the `cargo tree` companion to the pub-surface ratchet). A new lateral edge fails CI the way a new pub item does.
- **L2 — Keyed doors.** Each capability crate exposes exactly one entry surface, and that surface requires a **`CapabilityInvocation` context type constructible only by the Control Plane** — the seal pattern (§9.4) applied to invocation, not just execution. Holding the type proves clearing-house provenance; code that didn't cross the clearing house cannot type-check a capability call.
- **L3 — Lateral surface deletion.** Existing pub items that enabled point-to-point calls are deleted, not deprecated — each deletion a one-commit baseline reduction the ratchet locks (the FIA-4B shrink list is this section's opening backlog).

### 15.3 100% coverage, defined measurably

Coverage is continuously attested, not periodically reviewed:

- **C1 — Compile-time coverage**: L1's graph gate green ⇒ no alternative route exists to compile.
- **C2 — Runtime coverage attestation**: every capability entry point counts invocations by provenance. The metric `capability_invocations_without_cp_provenance` is on the assurance plane (§6.14) with an alert threshold of **zero**. During migration this number is the honest measure of remaining mesh; at completion it is the standing proof of the directive.
- **C3 — Audit closure**: every agent-originated state transition has a Control Plane decision record reachable from its audit trail (§6.11 unchanged; C1+C2 are what make its universality provable rather than asserted).

### 15.4 Pack universality (the ALL-packs clause)

Coverage is inherited by construction, never wired per pack:

- **K1** — Pack resolution executes inside the clearing house; there is no pack-scoped entry that precedes or bypasses it.
- **K2** — A pack cannot register verbs, routes, handlers, or tools that dispatch outside the Control Plane: the registration surfaces themselves require the L2 context type, so an out-of-house dispatch is unregistrable, not merely forbidden.
- **K3** — Pack onboarding requires no coverage work and permits no coverage exemption: pack N+1 is covered by the same compile-time proof as pack 1. Any proposed pack-scoped exception to the clearing house is a V&S amendment, not a configuration.

### 15.5 Reads (architect decision point — recommendation stated)

The mandate's audit property concerns decisions and state movement. Two conformant designs for agent read access (e.g. Sage's interpretation-context reads against SemOS):

- **(R-a, recommended) CP-issued read lenses**: the clearing house grants session-scoped, **typed read-only lenses** — capability views that provably cannot reach a write surface (no write types importable through the lens; enforced by the same visibility discipline). Interpretation stays hot-path-fast; leakproofness holds because a lens cannot move state and cannot be minted outside the clearing house.
- **(R-b) Full mediation**: every read brokered through the hub. Purest form; adds a hop inside interpretation loops; reserved as the fallback if lens discipline proves unenforceable.

Ratify R-a or R-b. Under either, C1–C3 and K1–K3 apply unchanged to all invocations and writes.

### 15.6 Migration posture

Checkpoint topology (v0.3 as built, T0–T10) is the **transitional state**, not a rival design: everything built relocates into the hub unchanged — gates, envelope lifecycle, admission scope, shadow telemetry. The migration ratchet is: C2's without-provenance count falls as lateral surfaces are deleted (L3), tranche by tranche, and CI locks each fall; the terminal state is C2 ≡ 0 locked by C1. The multi-path graduation story of the runbook collapses, at completion, to a single ingress — a simplification of the enforce-mode endgame, not an extension of it.

---

## REPLACEMENT §8 (relationship directions inverted)

§8.1 becomes: "The Control Plane receives the utterance and invokes Sage as its interpretation capability, granting the interpretation context (per §15.5). Sage returns candidate intents with attestations; Sage holds no capability keys and cannot dispatch." §8.5/§8.6 correspondingly: compiler and runtime are invoked by, and only by, the clearing house on agent-originated flows. The v0.3 sentence "The AI may propose intent; the Control Plane decides whether that intent is executable; the runtime, not the AI, moves state" is retained and strengthened by one clause: "…and the Control Plane is the only party that can ask it to."

## ADDITIONS to §12 (success criteria)

13. the dependency-graph gate (L1) is green with zero agent→capability edges outside the Control Plane;
14. `capability_invocations_without_cp_provenance` ≡ 0 over a full graduation window, on all packs, attested on the assurance plane;
15. a newly authored pack demonstrates K3: full coverage with zero pack-specific coverage work, evidenced at onboarding.

## Change-log entry (for the repo copy)

| v0.4 | Amendment 1: Clearing-House Mandate. New §15 (mediation topology; leakproof L1–L3; coverage C1–C3; pack universality K1–K3; read-lens decision §15.5; migration posture). §8 relationship directions inverted. §12 criteria 13–15 added. v0.3 checkpoint topology reclassified as transitional. |

---

## Ratification checklist (architect)

- [ ] §15.1 mediation topology — ratified as the target state
- [ ] §15.5 — R-a (lenses) or R-b (full mediation): ______
- [ ] §15.6 — checkpoint work (T0–T10) confirmed as transitional, relocating not discarded
- [ ] Sonnet session authorized to apply deltas to the repo copy → v0.4

On ratification, the implementation planning question becomes a single new arc — "Tranche series T11+: mesh retirement" — scoped from the FIA's route table and shrink list as the opening inventory of lateral surfaces to key or delete. That plan should not be cut until the FIA report exists: the mesh you retire must be the measured one, not the remembered one.
