//! Type→linkage matrix — EOP-DD-KYCUBO-TS.1 §2/§2a (D1 only).
//!
//! **What this module is.** The type geometry: whether a linkage *kind*
//! (a `Pipe`, TS.0 §3) is possible at all between two entity types (TS.0
//! §4). This is the FIRST of TS.1 §1's two constraint layers — structural,
//! not positional. The SECOND layer (the ratified studs: registration
//! before assertion, no duplicate active linkage, …) is unchanged,
//! existing code (`fold::control::check_preconditions`) and is applied on
//! top of this one by the placement-set generator (TS.1 §5).
//!
//! **What this module is not.** Not an assessment of whether a linkage is
//! *legal* in this position (that is the stud layer) and not any part of
//! D2 (compliance/policy/regulatory judgement) — TS.1 §0 is explicit that
//! D1 establishes facts, and this module answers exactly one structural
//! question: "is this triple a move at all."
//!
//! **Deliberately decoupled from `EdgeKind`.** `fold::control::EdgeKind`
//! (8 variants) is the fold's own storage vocabulary for `ubo.edge.assert-
//! control`/`assert-economic-interest` and is unchanged by this tranche —
//! rewriting it to match `Pipe`'s 17-variant granularity would touch the
//! fold, the wire-value tables, and every existing T2/T3/T4/T4.5/T6.1-T6.4
//! test fixture for no D1 requirement (D1 only needs to ask "is this pipe
//! possible here", not "what is stored on the edge"). `Pipe` is a new,
//! finer vocabulary used only by this matrix; a small classifier
//! (`pipe_for_edge_kind`, added in the P4 re-key) maps a probed edge's
//! `EdgeKind` onto the nearest `Pipe` for the geometry check only. See the
//! P1 recon note (EOP-PLAN's D1 tranche) for the reasoning.
//!
//! **One corrective fix, one confirmed-consistent absence, both transcribed
//! literally, not silently resolved:**
//! - Pipe 14 (employment / delegated authority) was **found and fixed in
//!   the corrective D1 tranche** (2026-08-21): the doc's own "§2 v0.3
//!   corrections" block (dated 2026-08-19, i.e. landed the same day as the
//!   ratified grid but missed by this crate's first transcription) states
//!   pipe 14 was omitted from every target row "in v0.2" — an editorial
//!   defect, not a ruling — and has since been added to the eight
//!   operating types that have employees: private limited company, public
//!   listed company, LLC (US), general partnership, LLP, cooperative/
//!   mutual, charity/not-for-profit, government dept/statutory
//!   corporation. **Deliberately NOT added to funds** (externally managed,
//!   via pipe 7 — a fund does not employ) or to limited partnership
//!   (fund-like, GP-managed). `target_permits` now transcribes the
//!   corrected 8-row set; see `matrix_is_exactly_known` (`ts1_assembly_
//!   board.rs`) for the re-pinned count, derived from TS.1 §2 v0.3.
//! - Pipe 17 (nominee holding) appears in ZERO target rows in §2. This
//!   *is* consistent with the ratified design, not a gap: TS.0 §5 already
//!   characterises nominee as "a capacity held in a particular edge...
//!   available during any strategy," not an ordinary typed linkage — its
//!   traversal (pierce-and-substitute) is governed by the separate,
//!   already-implemented `kyc_ubo.assert.edge.nominee-piercing` mechanism, not by
//!   ordinary type-geometry-gated `assert-linkage`. Its absence from the
//!   grid is therefore read as consistent with the ratified design.

use serde::{Deserialize, Serialize};

// ── Entity types (TS.0 §4 catalogue) ────────────────────────────────────────

/// One block type — TS.0 §4. Natural person is a type like any other
/// (TS.0 §6.4), never a target (§2, this module's `target_permits`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EntityType {
    // §4.1 Natural persons
    NaturalPerson,
    SoleTrader,
    // §4.2 Corporates
    PrivateLimitedCompany,
    PublicListedCompany,
    LlcUs,
    // §4.3 Partnerships
    GeneralPartnership,
    LimitedPartnership,
    Llp,
    // §4.4 Funds and collective investment
    OeicIcvc,
    Sicav,
    UnitTrust,
    FortyActFund,
    LpFund,
    UmbrellaWithSubFunds,
    // §4.5 Trusts and fiduciary structures
    DiscretionaryTrust,
    FixedBareTrust,
    Foundation,
    PensionScheme,
    // §4.6 Mutual, sovereign and not-for-profit
    CooperativeMutual,
    CharityNotForProfit,
    GovernmentDeptStatutoryCorporation,
    SovereignWealthVehicle,
}

/// All 22 catalogued entity types (TS.0 §4), canonical order matching the
/// catalogue's own family grouping. Used by `matrix_is_exactly_known`-style
/// exhaustiveness tests so a new variant forces a conscious edit here too.
pub const ALL_ENTITY_TYPES: &[EntityType] = &[
    EntityType::NaturalPerson,
    EntityType::SoleTrader,
    EntityType::PrivateLimitedCompany,
    EntityType::PublicListedCompany,
    EntityType::LlcUs,
    EntityType::GeneralPartnership,
    EntityType::LimitedPartnership,
    EntityType::Llp,
    EntityType::OeicIcvc,
    EntityType::Sicav,
    EntityType::UnitTrust,
    EntityType::FortyActFund,
    EntityType::LpFund,
    EntityType::UmbrellaWithSubFunds,
    EntityType::DiscretionaryTrust,
    EntityType::FixedBareTrust,
    EntityType::Foundation,
    EntityType::PensionScheme,
    EntityType::CooperativeMutual,
    EntityType::CharityNotForProfit,
    EntityType::GovernmentDeptStatutoryCorporation,
    EntityType::SovereignWealthVehicle,
];

// ── Control pipes (TS.0 §3 vocabulary) ──────────────────────────────────────

/// One control pipe — TS.0 §3. Distinct from `fold::control::EdgeKind`
/// (see module doc).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Pipe {
    VotingShares,
    NonVotingShares,
    GpDesignation,
    LimitedPartnershipInterest,
    BoardAppointment,
    OfficerAppointment,
    ManagementMandate,
    TrusteePowers,
    ReservedPowers,
    BeneficiaryEntitlement,
    MembershipRights,
    StatutoryAuthority,
    ContractualControl,
    EmploymentDelegatedAuthority,
    UnitIssuance,
    PooledAssetContainment,
    NomineeHolding,
}

/// All 17 catalogued pipes (TS.0 §3), in the doc's own numbering order.
pub const ALL_PIPES: &[Pipe] = &[
    Pipe::VotingShares,
    Pipe::NonVotingShares,
    Pipe::GpDesignation,
    Pipe::LimitedPartnershipInterest,
    Pipe::BoardAppointment,
    Pipe::OfficerAppointment,
    Pipe::ManagementMandate,
    Pipe::TrusteePowers,
    Pipe::ReservedPowers,
    Pipe::BeneficiaryEntitlement,
    Pipe::MembershipRights,
    Pipe::StatutoryAuthority,
    Pipe::ContractualControl,
    Pipe::EmploymentDelegatedAuthority,
    Pipe::UnitIssuance,
    Pipe::PooledAssetContainment,
    Pipe::NomineeHolding,
];

// ── Source/target endpoints ─────────────────────────────────────────────────

/// What may sit at the source end of a pipe. A described class (TS.1 §3a)
/// is a real source for pipe 10 and is deliberately NOT an `EntityType`
/// variant — "modelling it as a pseudo-entity would put a fiction in the
/// block set" (§3a, verbatim).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkageSource {
    Entity(EntityType),
    ClassOfBeneficiaries,
}

/// A refusal from the type-geometry layer — always distinct from a stud
/// (positional) violation (TS.1 §6 `type_geometry_refuses_impossible_linkage`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryError {
    /// Natural persons and sole traders are never a target (TS.1 §2 rows
    /// 21/22).
    PersonIsNeverATarget { pipe: Pipe },
    /// This (source, pipe, target) triple has no row in the ratified §2/§2a
    /// grid — not a wrong move, not a move at all.
    NotAMove {
        source: LinkageSource,
        pipe: Pipe,
        target: EntityType,
    },
}

// ── The matrix ───────────────────────────────────────────────────────────────

/// §2 — permitted incoming pipes for `target`. Transcribed literally,
/// row by row, against TS.1 §2 v0.3 (the "§2 v0.3 corrections" block,
/// which added pipe 14 to 8 target rows). Pipe 17 is absent from every row
/// here by ratified-text fidelity — see the module doc for why that is
/// consistent with the design, not a gap.
///
/// Sovereign wealth vehicle (TS.0 §4.6: "whatever vehicle it actually is
/// ... plus 12") is not compositional in this D1 model — `EntityType` has
/// no parametrisation over "the vehicle it actually is." Modelled here as
/// admitting only pipe 12 (statutory authority); the fuller "plus its
/// actual vehicle type's row" composition is an explicitly open design
/// question, not silently resolved (see tranche receipts).
fn target_permits(target: EntityType, pipe: Pipe) -> bool {
    use EntityType::*;
    use Pipe::*;
    let permitted: &[Pipe] = match target {
        PrivateLimitedCompany => &[VotingShares, NonVotingShares, BoardAppointment, OfficerAppointment, ContractualControl, EmploymentDelegatedAuthority],
        PublicListedCompany => &[VotingShares, NonVotingShares, BoardAppointment, OfficerAppointment, ContractualControl, EmploymentDelegatedAuthority],
        LlcUs => &[VotingShares, BoardAppointment, OfficerAppointment, ContractualControl, EmploymentDelegatedAuthority],
        GeneralPartnership => &[GpDesignation, BoardAppointment, ContractualControl, EmploymentDelegatedAuthority],
        LimitedPartnership => &[GpDesignation, LimitedPartnershipInterest, ManagementMandate, ContractualControl],
        Llp => &[GpDesignation, BoardAppointment, ContractualControl, EmploymentDelegatedAuthority],
        OeicIcvc => &[BoardAppointment, ManagementMandate, UnitIssuance, PooledAssetContainment],
        Sicav => &[BoardAppointment, ManagementMandate, UnitIssuance, PooledAssetContainment],
        UnitTrust => &[ManagementMandate, TrusteePowers, UnitIssuance, PooledAssetContainment],
        FortyActFund => &[BoardAppointment, ManagementMandate, UnitIssuance, PooledAssetContainment],
        LpFund => &[GpDesignation, LimitedPartnershipInterest, ManagementMandate, UnitIssuance],
        UmbrellaWithSubFunds => &[PooledAssetContainment],
        DiscretionaryTrust => &[TrusteePowers, ReservedPowers, BeneficiaryEntitlement],
        FixedBareTrust => &[TrusteePowers, BeneficiaryEntitlement],
        Foundation => &[TrusteePowers, ReservedPowers, BeneficiaryEntitlement],
        PensionScheme => &[TrusteePowers, BeneficiaryEntitlement],
        CooperativeMutual => &[BoardAppointment, OfficerAppointment, MembershipRights, EmploymentDelegatedAuthority],
        CharityNotForProfit => &[BoardAppointment, OfficerAppointment, TrusteePowers, EmploymentDelegatedAuthority],
        GovernmentDeptStatutoryCorporation => &[BoardAppointment, OfficerAppointment, StatutoryAuthority, EmploymentDelegatedAuthority],
        SovereignWealthVehicle => &[StatutoryAuthority],
        NaturalPerson | SoleTrader => &[],
    };
    permitted.contains(&pipe)
}

/// §2a — permitted source endpoints for `pipe`. Transcribed literally
/// against the ratified table text; category groups are spelled out
/// (not inferred) so a future entity-type addition cannot silently widen
/// a source rule.
fn source_permits(pipe: Pipe, source: LinkageSource) -> bool {
    use EntityType::*;
    use Pipe::*;

    // Shorthand groups exactly as named in §2a's prose.
    const CORPORATES: &[EntityType] = &[PrivateLimitedCompany, PublicListedCompany, LlcUs];
    const PARTNERSHIPS: &[EntityType] = &[GeneralPartnership, LimitedPartnership, Llp];
    const FUNDS: &[EntityType] = &[OeicIcvc, Sicav, UnitTrust, FortyActFund, LpFund, UmbrellaWithSubFunds];
    const TRUSTS: &[EntityType] = &[DiscretionaryTrust, FixedBareTrust, Foundation, PensionScheme];
    const NATURAL: &[EntityType] = &[NaturalPerson, SoleTrader];

    let LinkageSource::Entity(source_type) = source else {
        // Only pipe 10 (beneficiary entitlement) admits a described class
        // as a source (§2a row 10, §3a).
        return pipe == BeneficiaryEntitlement;
    };

    match pipe {
        // 1/2: "any type that can hold property: all corporates,
        // partnerships, funds, trusts (via trustee), natural persons."
        VotingShares | NonVotingShares => {
            CORPORATES.contains(&source_type)
                || PARTNERSHIPS.contains(&source_type)
                || FUNDS.contains(&source_type)
                || TRUSTS.contains(&source_type)
                || NATURAL.contains(&source_type)
        }
        // 3: "corporates and natural persons only — a fund is not a GP of
        // itself."
        GpDesignation => CORPORATES.contains(&source_type) || NATURAL.contains(&source_type),
        // 4: "any holder, including funds and persons."
        LimitedPartnershipInterest => true,
        // 5: "natural persons; corporate directors where the jurisdiction
        // permits (attribute on the source)." The corporate-director
        // extension is an attribute-conditional refinement (TS.1 §7 Q2:
        // "an attribute on the source, not a separate pipe") — out of this
        // structural matrix; base rule is natural-person-sourced.
        BoardAppointment => NATURAL.contains(&source_type),
        // 6: "natural persons (ruled 2026-08-19)."
        OfficerAppointment => NATURAL.contains(&source_type),
        // 7: "corporate only (ruled 2026-08-19) — a regulated management
        // entity, never a natural person."
        ManagementMandate => CORPORATES.contains(&source_type),
        // 8: "corporates (corporate trustee) and natural persons."
        TrusteePowers => CORPORATES.contains(&source_type) || NATURAL.contains(&source_type),
        // 9: "natural persons, and corporates acting as protector."
        ReservedPowers => NATURAL.contains(&source_type) || CORPORATES.contains(&source_type),
        // 10: "natural persons, corporates, charities, and classes"
        // (class handled above, before the `Entity(..)` match).
        BeneficiaryEntitlement => {
            NATURAL.contains(&source_type)
                || CORPORATES.contains(&source_type)
                || source_type == CharityNotForProfit
        }
        // 11: "natural persons and corporates."
        MembershipRights => NATURAL.contains(&source_type) || CORPORATES.contains(&source_type),
        // 12: "government departments and sovereign bodies only."
        StatutoryAuthority => {
            matches!(source_type, GovernmentDeptStatutoryCorporation | SovereignWealthVehicle)
        }
        // 13: "any type."
        ContractualControl => true,
        // 14: "natural persons only (ruled 2026-08-19)." Target side was a
        // grid gap in this crate's first transcription, corrected in the
        // D1 corrective tranche (2026-08-21) — see module doc and
        // `target_permits`. This arm is reachable now for the 8 operating
        // types with employees.
        EmploymentDelegatedAuthority => NATURAL.contains(&source_type),
        // 15: "any investor type."
        UnitIssuance => true,
        // 16: "umbrella -> sub-fund only; both must be fund types."
        PooledAssetContainment => source_type == UmbrellaWithSubFunds,
        // 17: absent from the target grid by design (see module doc);
        // source rule recorded for completeness only.
        NomineeHolding => CORPORATES.contains(&source_type) || NATURAL.contains(&source_type),
    }
}

/// The type-geometry admissibility check (TS.1 §1's FIRST constraint
/// layer). Pure, total, and independent of any live state — the second
/// (positional/stud) layer is `fold::control::check_preconditions`,
/// applied on top by the placement-set generator, never by this function.
pub fn check_type_geometry(
    source: LinkageSource,
    pipe: Pipe,
    target: EntityType,
) -> Result<(), GeometryError> {
    if matches!(target, EntityType::NaturalPerson | EntityType::SoleTrader) {
        return Err(GeometryError::PersonIsNeverATarget { pipe });
    }
    if target_permits(target, pipe) && source_permits(pipe, source) {
        Ok(())
    } else {
        Err(GeometryError::NotAMove { source, pipe, target })
    }
}

// ── Pipe derivation (TS.2 §3) ───────────────────────────────────────────────
//
// **Pulled forward from the D1-Part-B tranche.** `kyc_ubo.assert.subject.type-correction`'s
// §4 cascade (`fold::type_registry::edges_invalidated_by_correction`) needs a
// `Pipe` classification for each edge touching the corrected entity — that is
// exactly TS.2's `pipe_of(edge_kind, target_entity_type) -> Pipe` deliverable.
// Built here, now, against TODAY's 11-variant `EdgeKind` so Part A's
// `correct-type` op is genuinely real rather than stubbed; Part B's B2 step
// widens the match for the 6 `EdgeKind` variants it adds and does not need to
// invent this function fresh.

/// A `Pipe` classification derived from a stored edge. `provisional: true`
/// means the classification is genuinely UNRESOLVED — `pipe` is `None`, not
/// a best-effort guess dressed up as a flagged value (D1 corrective
/// tranche Item 4, 2026-08-21: the prior shape always carried a concrete
/// `Pipe` even when provisional, defaulting to `NonVotingShares` for
/// exactly the case this doc now calls out as wrong — "unknown is
/// alleged, not silently classified"). Unresolved when the target's
/// entity type is not definitively known — either because no type has
/// been asserted at all (`target_type: None`) or because the known type
/// falls outside the three buckets TS.2 §3 ratifies for `EconomicInterest`
/// (corporate → 2, LP/LP-fund → 4, fund → 15). The second case is a
/// genuine open question this function does not silently resolve: flagged
/// here and in the tranche receipts as a candidate for a TS.2 amendment,
/// not guessed away. `provisional == false` iff `pipe.is_some()` — the
/// two fields are redundant by construction, kept both for call-site
/// clarity (`.provisional` reads better in an `if`; `.pipe` is what a
/// caller actually needs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipeClassification {
    pub pipe: Option<Pipe>,
    pub provisional: bool,
}

/// TS.2 §3 — derive the `Pipe` a stored `EdgeKind` represents, given the
/// target entity's type where known. Total: every `(EdgeKind, Option<EntityType>)`
/// pair classifies, never panics — but classification is not always
/// resolved to a concrete `Pipe`; ambiguity is expressed via
/// `provisional: true` + `pipe: None`, never a silent default masquerading
/// as certain (TS.2 §6 `pipe_of_is_total`).
pub fn pipe_of(
    kind: &crate::fold::control::EdgeKind,
    target_type: Option<EntityType>,
) -> PipeClassification {
    use crate::fold::control::{EdgeKind::*, TrustRoleKind::*};

    let certain = |pipe: Pipe| PipeClassification { pipe: Some(pipe), provisional: false };
    match kind {
        VotingRights => certain(Pipe::VotingShares),
        BoardAppointment => certain(Pipe::BoardAppointment),
        GpStatutory => certain(Pipe::GpDesignation),
        // TS.2 §7 Q2 (RATIFIED): DesignatedMember (LLP) shares the GP
        // designation pipe — statutory naming for the same control shape,
        // not a distinct mechanism.
        DesignatedMember => certain(Pipe::GpDesignation),
        TrustRole(Trustee) => certain(Pipe::TrusteePowers),
        TrustRole(Settlor) | TrustRole(Protector) => certain(Pipe::ReservedPowers),
        TrustRole(Beneficiary) => certain(Pipe::BeneficiaryEntitlement),
        // Capacity overlay (TS.0 §5, TS.1 §2 v0.3) — recorded for
        // completeness; pierce-and-substitute governs its real traversal.
        Nominee => certain(Pipe::NomineeHolding),
        // TS.2 §2 defect: only "genuine dominant influence, deliberately
        // asserted" reaches the fold as this variant on the live append
        // path post-B3 (Part B splits the parse-failure fallback that used
        // to also land here) — classified as contractual control.
        DominantInfluence => certain(Pipe::ContractualControl),
        EconomicInterest => classify_economic_interest(target_type),
        // TS.2 §5 growth — direct 1:1 mappings, no target-type dependence.
        OfficerAppointment => certain(Pipe::OfficerAppointment),
        ManagementMandate => certain(Pipe::ManagementMandate),
        MembershipRights => certain(Pipe::MembershipRights),
        StatutoryAuthority => certain(Pipe::StatutoryAuthority),
        Employment => certain(Pipe::EmploymentDelegatedAuthority),
        Containment => certain(Pipe::PooledAssetContainment),
    }
}

/// TS.2 §3: `EconomicInterest` alone cannot say which pipe it is; the
/// target's entity type can, but only for the three ratified buckets.
fn classify_economic_interest(target_type: Option<EntityType>) -> PipeClassification {
    use EntityType::*;
    match target_type {
        Some(PrivateLimitedCompany | PublicListedCompany | LlcUs) => {
            PipeClassification { pipe: Some(Pipe::NonVotingShares), provisional: false }
        }
        Some(LimitedPartnership | LpFund) => {
            PipeClassification { pipe: Some(Pipe::LimitedPartnershipInterest), provisional: false }
        }
        Some(OeicIcvc | Sicav | UnitTrust | FortyActFund | UmbrellaWithSubFunds) => {
            PipeClassification { pipe: Some(Pipe::UnitIssuance), provisional: false }
        }
        // `None` (type still alleged, CTN-2h weakest-link) or a known type
        // outside the ratified three buckets (TS.2 §3 does not name a pipe
        // for, e.g., an EconomicInterest edge into a GeneralPartnership —
        // an open question, not silently resolved). D1 corrective tranche
        // Item 4 (2026-08-21): this used to default to `NonVotingShares`
        // marked provisional — a guess dressed up as a flagged value,
        // contradicting the ratified "unknown is alleged, not silently
        // classified". Now genuinely unresolved: no pipe is invented.
        _ => PipeClassification { pipe: None, provisional: true },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_entity_type_appears_exactly_once_in_all_entity_types() {
        let mut seen = std::collections::BTreeSet::new();
        for t in ALL_ENTITY_TYPES {
            assert!(seen.insert(*t), "duplicate entity type in ALL_ENTITY_TYPES: {t:?}");
        }
        assert_eq!(ALL_ENTITY_TYPES.len(), 22);
    }

    #[test]
    fn every_pipe_appears_exactly_once_in_all_pipes() {
        let mut seen = std::collections::BTreeSet::new();
        for p in ALL_PIPES {
            assert!(seen.insert(*p), "duplicate pipe in ALL_PIPES: {p:?}");
        }
        assert_eq!(ALL_PIPES.len(), 17);
    }
}
