//! F3 target (EOP-FUZZ-KYCUBO-001 §4 P0): O1 (no-panic) + O4 (cycle/resource
//! safety) over all 8 `DeterminationStrategy` implementations.
//!
//! `determination.rs:131` and `:212` both DFS a `ControlState`'s edges with
//! an explicit path-based cycle guard (`path.iter().filter(|&&e| e ==
//! entity).count() > 1 { continue }`). Both DFS loops use an explicit `Vec`
//! stack, not recursion, so a broken guard would show up as unbounded
//! candidate growth / non-termination under `-timeout`, not a call-stack
//! overflow — the generator's small, heavily-reused entity pool
//! (`gen_events`, 4 entities shared across `from_entity_id`/`to_entity_id`)
//! makes real cycles (including direct self-loops, `from == to`) common,
//! not rare, so this target spends most of its cycles exactly where the
//! guard has to hold.
#![no_main]

use std::collections::BTreeSet;

use libfuzzer_sys::fuzz_target;
use ob_poc_kyc_substrate::{
    fold_control, natural_persons_from_events, ControlProngStrategy, CooperativeMemberStrategy,
    DeterminationStrategy, EntityId, FoundationCouncilStrategy, FundControlStrategy,
    NomineePierceStrategy, OwnershipProngStrategy, StateOwnedStrategy, TrustRoleStrategy,
};
use ob_poc_kyc_substrate_fuzz::{gen_events, Tape};
use uuid::Uuid;

static OWNERSHIP: OwnershipProngStrategy = OwnershipProngStrategy;
static CONTROL: ControlProngStrategy = ControlProngStrategy;
static TRUST: TrustRoleStrategy = TrustRoleStrategy;
static FUND: FundControlStrategy = FundControlStrategy;
static FOUNDATION: FoundationCouncilStrategy = FoundationCouncilStrategy;
static STATE_OWNED: StateOwnedStrategy = StateOwnedStrategy;
static COOPERATIVE: CooperativeMemberStrategy = CooperativeMemberStrategy;
static NOMINEE: NomineePierceStrategy = NomineePierceStrategy;

fn strategies() -> [&'static dyn DeterminationStrategy; 8] {
    [
        &OWNERSHIP,
        &CONTROL,
        &TRUST,
        &FUND,
        &FOUNDATION,
        &STATE_OWNED,
        &COOPERATIVE,
        &NOMINEE,
    ]
}

fuzz_target!(|data: &[u8]| {
    let mut tape = Tape::new(data);
    let events = gen_events(&mut tape);
    let refs: Vec<&_> = events.iter().collect();

    let control = fold_control(&refs);
    let natural_persons = natural_persons_from_events(&refs);

    // Distinct entities actually appearing in the graph — a hard upper
    // bound on distinct-person candidates any strategy can legitimately
    // return (every candidate merges by PersonId in a BTreeMap), used
    // below as the O4 resource-safety check.
    let entity_count: BTreeSet<EntityId> = control
        .edges
        .values()
        .flat_map(|e| [e.from, e.to])
        .collect();
    let bound = entity_count.len() + 1;

    // Subject entity: an arbitrary edge endpoint from the folded graph (so
    // DFS actually starts inside whatever cycles/chains got built), or a
    // fresh disconnected entity when the graph is empty (a boring but
    // valid input — DFS from an isolated node with no edges).
    let subject_entity_id = control
        .edges
        .values()
        .next()
        .map(|e| e.to)
        .unwrap_or_else(|| EntityId(Uuid::new_v4()));
    let threshold_pct = tape.percentage();

    for strategy in strategies() {
        // O1: must not panic on any adversarially-cyclic graph.
        let candidates = strategy.resolve(&control, subject_entity_id, &natural_persons, threshold_pct);
        // O4: the cycle guard must actually bound the traversal — no
        // strategy should ever return more candidates than there are
        // distinct entities in the graph.
        assert!(
            candidates.len() <= bound,
            "O4: {} returned {} candidates over only {} distinct graph entities \
             — cycle guard did not bound the traversal",
            strategy.name(),
            candidates.len(),
            entity_count.len()
        );
    }
});
