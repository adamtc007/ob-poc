//! R6 (`EOP-VS-UBO-GAME-001` §3.4): "Every surface builds the same event.
//! One function maps declared arguments to a stored event; every surface
//! calls it. A second constructor is how two surfaces come to disagree,
//! and it is forbidden rather than merely discouraged."
//!
//! Before this module: three constructors existed —
//! `IntentEventDraft::into_event` (the op layer, via `stream_append` in
//! `ob-poc`'s `kyc_stream_ops.rs`), a second `IntentEvent::new` call inside
//! `KycWorkbook::stage` (`ob-poc`'s `kyc_workbook.rs`), and a third,
//! `placement::probe_event`, inside the board enumerator itself. The op
//! layer built a correct target/payload split per verb, via five shared
//! `normalize_*` functions plus three more verbs' worth of ad-hoc inline
//! construction (`assert-type`, `type-correction`, `enquiry`,
//! `member-withdrawal`) — 13 of 19 verbs needed one or the other. The
//! workbook built target/payload by a single fixed rule (five hyphenated
//! slot names always go to the target, everything else to the payload),
//! with no per-verb knowledge at all. Where a verb's fold reads a
//! target-slot-named field back out of the *payload* (`register`,
//! `type`, `type-correction`, `member-withdrawal`, `structure-class`'s
//! `entity_id`, `economic-interest`/`control`'s `edge_id` fallback, the
//! three obligation-track verbs' `obligation_id`/`subject_id`), the two
//! surfaces silently disagreed about where the same declared argument
//! belongs. That disagreement is what this module deletes.
//!
//! `canonical_event_shape` is now the **only** place that decides.

use anyhow::{anyhow, bail, Result};
use uuid::Uuid;

use ob_poc_kyc_substrate::{
    EdgeId, EntityId, ObligationId, SubjectId, TargetBinding, EDGE_KIND_WIRE_VALUES,
    ENTITY_TYPE_WIRE_VALUES,
};

// ── Pure arg-extraction (no ctx, no @symbol — see the module-level scope note) ──

/// Parse a plain UUID-string literal from `args`. Deliberately no
/// `@symbol`/session-binding resolution: `canonical_event_shape` is called
/// from the workbook (which resolves entity handles into UUID literals
/// during recognition, before staging, and has no session-binding concept
/// at all) as well as the op layer. A caller that needs `@symbol` support
/// for a field resolves it into its own `args` clone before calling this
/// function — exactly the contract `subject` itself already has to satisfy
/// (it arrives here as a resolved `SubjectId`, never a raw arg).
fn uuid_arg(args: &serde_json::Value, name: &str) -> Option<Uuid> {
    args.get(name)?.as_str().and_then(|s| Uuid::parse_str(s).ok())
}

fn required_uuid_arg(verb_fqn: &str, args: &serde_json::Value, name: &str) -> Result<Uuid> {
    uuid_arg(args, name).ok_or_else(|| anyhow!("{verb_fqn}: missing or invalid `{name}` argument"))
}

fn string_arg<'a>(args: &'a serde_json::Value, name: &str) -> Option<&'a str> {
    args.get(name)?.as_str()
}

fn required_string_arg<'a>(verb_fqn: &str, args: &'a serde_json::Value, name: &str) -> Result<&'a str> {
    string_arg(args, name).ok_or_else(|| anyhow!("{verb_fqn}: missing `{name}` argument"))
}

// ── The one constructor ──────────────────────────────────────────────────────

/// Map one verb's declared arguments to `(target, payload, edge_id)`.
///
/// `edge_id` is `Some` exactly when this call creates or addresses an edge
/// (`connect` always mints one — EOP-VS-UBO-GAME-001 §8 Q3, RATIFIED
/// (worded as a Recommendation in the source document, not marked RULED
/// like Q1, but stated with no dissenting alternative and applied here as
/// such — flagged, not silently resolved): "the system mints it and returns
/// it; a caller-chosen id is a stored identifier the board cannot predict."
/// T3 (2026-08-27) closes the gap this doc used to describe: a
/// caller-supplied `edge-id` on `connect` is now refused outright, not
/// merely ignored) or `evidence`/`verification`/`disconnect` target an
/// existing one.
///
/// Deliberately excluded, not silently missing — see the module doc:
/// - `kyc_ubo.decide.determination.freeze` (§3.2: computes a verdict from
///   live state, not from declared arguments — call it, don't route it here).
/// - `kyc_ubo.decide.subject.{approve,reject}` / `.decide.obligation.waiver`
///   — never build an `IntentEvent` (TS.6 P2); calling this with one of
///   these FQNs is a caller bug, not a shape question, hence `bail!`.
/// - `kyc_ubo.assert.edge.connect`'s `pierced-from` existence/kind/active
///   check — needs a live DB read; stays a separate op-layer step around
///   this call, same principle.
pub fn canonical_event_shape(
    verb_fqn: &str,
    subject: SubjectId,
    args: &serde_json::Value,
) -> Result<(TargetBinding, serde_json::Value, Option<EdgeId>)> {
    let subj_target = TargetBinding::for_subject(subject);

    match verb_fqn {
        // EOP-VS-UBO-GAME-001 T2, §3.2: `place` absorbs register +
        // assert-type — one move, one event, both membership and type.
        // Entity-scoped target (unlike register's old bare-subject target):
        // needed so `placement.rs` can offer real, non-vacuous per-entity
        // `place` candidates (T1's P4 deferral, closed for the re-placeable
        // population). `entity-id` still defaults to the subject's own
        // entity — the one candidate that needs no discovery data.
        "kyc_ubo.assert.subject.place" => {
            let entity = EntityId(uuid_arg(args, "entity-id").unwrap_or(subject.0));
            let entity_type = required_string_arg(verb_fqn, args, "entity-type")?;
            if !ENTITY_TYPE_WIRE_VALUES.contains(&entity_type) {
                bail!(
                    "{verb_fqn}: unrecognized entity-type '{entity_type}' — valid wire values: {}",
                    ENTITY_TYPE_WIRE_VALUES.join(", ")
                );
            }
            let target = TargetBinding { entity_id: Some(entity), ..subj_target };
            // `is_natural_person` REMOVED (2026-09-07, audit A1b): the
            // entity type is the single source of truth for personhood —
            // the fold's traversal terminus derives it from `entity_type`,
            // never from a parallel payload flag. A caller still passing
            // the retired arg is ignored here, same as any other
            // undeclared argument; the historical flag remains readable on
            // pre-existing events via `natural_persons_from_events`'s
            // R5 no-type fallback.
            let payload = serde_json::json!({ "entity_id": entity.0, "entity_type": entity_type });
            Ok((target, payload, None))
        }

        // `kyc_ubo.assert.subject.structure-class` RETIRED
        // (EOP-DD-UBO-DISPATCH-001 T4, 2026-08-28) — no live arm here, same
        // as `type`/`type-correction` below. The fold-side parser
        // (`ob-poc-kyc-substrate::fold::control::structure_class_from_payload`)
        // stays R5-historical for replay of the 10 real committed events
        // (P0 census); this write-path constructor has nothing left to
        // build for.
        //
        // `kyc_ubo.assert.subject.type` retired by T2 (absorbed into
        // `place`, above) — no longer a live arm here.
        //
        // `kyc_ubo.assert.subject.type-correction` DISSOLVED by T2 (§8 Q1,
        // 2026-08-27) — no live arm here either. Correcting a type is
        // `remove` then `place`, two ordinary moves through the arms
        // already above, never a superseding placement.

        // EOP-VS-UBO-GAME-001 T2, §3.2: `remove` absorbs member-withdrawal
        // — identical shape, new name.
        "kyc_ubo.assert.subject.remove" => {
            let entity = EntityId(required_uuid_arg(verb_fqn, args, "entity-id")?);
            let target = TargetBinding { entity_id: Some(entity), ..subj_target };
            let payload = serde_json::json!({ "entity_id": entity.0 });
            Ok((target, payload, None))
        }

        "kyc_ubo.assert.subject.enquiry" => {
            let payload = serde_json::json!({
                "sources_consulted": args.get("sources-consulted").cloned().unwrap_or_else(|| serde_json::json!([])),
                "searches_run": args.get("searches-run").cloned().unwrap_or_else(|| serde_json::json!([])),
            });
            Ok((subj_target, payload, None))
        }

        // EOP-VS-UBO-GAME-001 T3, §3.2: `connect` absorbs assert-control +
        // assert-economic-interest — they differ by kind, and geometry
        // already validates the classified pipe (TS.5 R1); their
        // preconditions are identical (P0b's merge test, confirmed from
        // source). §8 Q3: the system mints the link id and returns it — a
        // caller-supplied `edge-id` is refused, not silently accepted or
        // ignored: it is a stored identifier the board cannot predict,
        // which is what made the old verb unstageable through the board.
        "kyc_ubo.assert.edge.connect" => {
            if args.get("edge-id").is_some() {
                bail!(
                    "{verb_fqn}: a caller-supplied edge id is refused (§8 Q3) — the system \
                     mints the link id and returns it"
                );
            }
            let edge = EdgeId(Uuid::new_v4());
            let from = required_uuid_arg(verb_fqn, args, "from_entity_id")?;
            let to = required_uuid_arg(verb_fqn, args, "to_entity_id")?;
            let kind = required_string_arg(verb_fqn, args, "kind")?;
            let pierced_from = uuid_arg(args, "pierced-from");
            if kind == "nominee" && pierced_from.is_some() {
                bail!(
                    "{verb_fqn}: a pierce cannot produce another nominee edge (K-8, fail-closed) \
                     — `kind` must be the UNDERLYING kind the nominator actually holds"
                );
            }
            if !EDGE_KIND_WIRE_VALUES.contains(&kind) {
                bail!(
                    "{verb_fqn}: unrecognized kind '{kind}' — valid wire values: {}",
                    EDGE_KIND_WIRE_VALUES.join(", ")
                );
            }
            let mut payload = serde_json::json!({
                "edge_id": edge.0,
                "from_entity_id": from,
                "to_entity_id": to,
                "kind": kind,
            });
            if let Some(p) = args.get("percentage") {
                payload["percentage"] = p.clone();
            }
            if let Some(v) = args.get("trust-revocable") {
                payload["trust_revocable"] = v.clone();
            }
            if let Some(pf) = pierced_from {
                payload["pierced_from"] = serde_json::Value::String(pf.to_string());
            }
            // `connect` is geometry-gated (TS.5 R1), not edge-scoped:
            // `enumerate_placement_set`'s per-triple candidates all share
            // the bare `TargetBinding::for_subject(subject)` (the specific
            // triple lives in the move's `proposed_edge`/payload, not the
            // target — there is no way to build a per-triple target ahead
            // of the mint, since §8 Q3 means the edge id doesn't exist
            // until this call runs). Setting `target.edge_id` to the freshly
            // minted id here would make this event's own target
            // unmatchable against any legal move (`KycWorkbook::stage`
            // compares `target` verbatim) and would re-render as a
            // caller-supplied `:edge-id`, which a fresh connect call must
            // refuse (§8 Q3) — both wrong for the event that MINTS the id.
            // The mint still flows via `payload["edge_id"]` (read by the
            // fold) and this function's own `Some(edge)` return.
            let target = TargetBinding::for_subject(subject);
            Ok((target, payload, Some(edge)))
        }

        // `kyc_ubo.assert.edge.economic-interest` retired T3 (§3.2, absorbed
        // into `connect` above) — no live arm here.

        "kyc_ubo.assert.edge.evidence" => {
            // EOP-DD-UBO-PROOF-001 §1/§2 (T5, 2026-08-28): "a proof is a
            // kind, a source, and a date." All three ride in the payload
            // (fold::control::proof_record_from_payload's contract) —
            // `kind` is one of the seven ratified `ProofKind` wire values,
            // `source` free text (§6 Q3: "structure when a check needs to
            // read it"), `date` when the proof was obtained.
            let kind = required_string_arg(verb_fqn, args, "kind")?;
            let source = required_string_arg(verb_fqn, args, "source")?;
            let date = required_string_arg(verb_fqn, args, "date")?;
            let proof_payload = serde_json::json!({ "kind": kind, "source": source, "date": date });
            let edge = uuid_arg(args, "edge-id").map(EdgeId);
            let entity = uuid_arg(args, "entity-id").map(EntityId);
            match (edge, entity) {
                (Some(edge), None) => {
                    let target = TargetBinding::for_edge(subject, edge);
                    Ok((target, proof_payload, Some(edge)))
                }
                (None, Some(entity)) => {
                    let target = TargetBinding { entity_id: Some(entity), ..subj_target };
                    Ok((target, proof_payload, None))
                }
                (Some(_), Some(_)) => bail!(
                    "{verb_fqn}: supply exactly one of edge-id (evidences an edge) or entity-id \
                     (evidences a type), never both"
                ),
                (None, None) => bail!("{verb_fqn}: missing edge-id or entity-id argument"),
            }
        }

        // `kyc_ubo.assert.edge.verification` RETIRED (EOP-DD-UBO-PROOF-001
        // §3/§4, T5, 2026-08-28) — "the board collects facts; the policy
        // rules on adequacy," so there is no ratchet left to move an edge
        // into. K-G7: 0 real committed events under this FQN.

        // Move 5 (§3.1's move table: "that proof no longer stands — group,
        // citation", T5): the citation alone identifies the proof —
        // neither an edge nor an entity target is needed, `retract`'s own
        // fold arms (both `fold::control` and `fold::type_registry`) scan
        // whichever axis actually holds it.
        "kyc_ubo.assert.edge.retract" => {
            let citation = required_uuid_arg(verb_fqn, args, "citation-id")?;
            let payload = serde_json::json!({ "citation_id": citation.to_string() });
            Ok((subj_target, payload, None))
        }

        // EOP-VS-UBO-GAME-001 T3, §3.2: `disconnect` absorbs supersession —
        // "the link is no longer on the board" (K-13 supersede-never-delete
        // still holds: the edge stays, status flips). Pure rename, shape
        // unchanged.
        "kyc_ubo.assert.edge.disconnect" => {
            let edge = EdgeId(required_uuid_arg(verb_fqn, args, "edge-id")?);
            let target = TargetBinding::for_edge(subject, edge);
            Ok((target, serde_json::json!({}), Some(edge)))
        }

        // `kyc_ubo.assert.edge.reconciliation` RETIRED (EOP-VS-UBO-GAME-001
        // T3, §3.3, 2026-08-27, K-G7 full deletion — see dsl-kyc.yaml's
        // retirement comment at this verb's former entry for the full
        // reasoning) — no live arm here.

        "kyc_ubo.assert.entity.identity"
        | "kyc_ubo.assert.entity.screening"
        | "kyc_ubo.assert.entity.risk" => {
            let obligation = ObligationId(required_uuid_arg(verb_fqn, args, "obligation-id")?);
            let mut payload = args.clone();
            if let Some(obj) = payload.as_object_mut() {
                obj.remove("obligation-id");
                obj.insert("obligation_id".into(), serde_json::Value::String(obligation.0.to_string()));
                obj.remove("subject-id");
                obj.insert("subject_id".into(), serde_json::Value::String(subject.0.to_string()));
            }
            Ok((subj_target, payload, None))
        }

        "kyc_ubo.decide.determination.freeze" => bail!(
            "{verb_fqn}: exempt by design (EOP-VS-UBO-GAME-001 §3.2) — a surface that cannot \
             compute the determination basis may not produce the verdict; freeze's payload is \
             assembled by the caller from a live fold, never from declared arguments alone"
        ),

        "kyc_ubo.decide.subject.approve"
        | "kyc_ubo.decide.subject.reject"
        | "kyc_ubo.decide.obligation.waiver" => bail!(
            "{verb_fqn}: never builds an IntentEvent (TS.6 P2) — it writes directly to \
             kyc_decision_records/kyc_evaluation_runs, not the fact stream; calling \
             canonical_event_shape with this FQN is a caller error, not a shape question"
        ),

        other => bail!("canonical_event_shape: unrecognized verb_fqn '{other}'"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subj() -> SubjectId {
        SubjectId(Uuid::new_v4())
    }

    /// The exact defect P1's RED test proved: `register`'s `entity-id`
    /// must land in the PAYLOAD (as `entity_id`), never only the target —
    /// the fold reads it from the payload. This is the one assertion that
    /// must never regress.
    #[test]
    fn place_entity_id_and_type_land_in_target_and_payload() {
        let s = subj();
        let entity = Uuid::new_v4();
        let args = serde_json::json!({
            "subject-id": s.0, "entity-id": entity, "entity-type": "private_limited_company"
        });
        let (target, payload, edge) =
            canonical_event_shape("kyc_ubo.assert.subject.place", s, &args).unwrap();
        assert_eq!(target.entity_id, Some(EntityId(entity)));
        assert_eq!(payload["entity_id"], entity.to_string());
        assert_eq!(payload["entity_type"], "private_limited_company");
        assert!(edge.is_none());
    }

    #[test]
    fn place_defaults_entity_id_to_subject() {
        let s = subj();
        let args = serde_json::json!({ "subject-id": s.0, "entity-type": "natural_person" });
        let (target, payload, _) =
            canonical_event_shape("kyc_ubo.assert.subject.place", s, &args).unwrap();
        assert_eq!(target.entity_id, Some(EntityId(s.0)));
        assert_eq!(payload["entity_id"], s.0.to_string());
    }

    #[test]
    fn place_rejects_unknown_entity_type() {
        let s = subj();
        let args = serde_json::json!({
            "subject-id": s.0, "entity-id": Uuid::new_v4(), "entity-type": "spaceship"
        });
        assert!(canonical_event_shape("kyc_ubo.assert.subject.place", s, &args).is_err());
    }

    #[test]
    fn place_requires_entity_type() {
        let s = subj();
        let args = serde_json::json!({ "subject-id": s.0, "entity-id": Uuid::new_v4() });
        assert!(canonical_event_shape("kyc_ubo.assert.subject.place", s, &args).is_err());
    }

    // `structure_class_kebab_renamed_to_snake` and
    // `structure_class_rejects_unknown_wire_value` RETIRED
    // (EOP-DD-UBO-DISPATCH-001 T4-close, 2026-08-28): both drove
    // `canonical_event_shape("kyc_ubo.assert.subject.structure-class", ...)`
    // to prove the verb's own kebab→snake payload normalization and its
    // wire-value validation — the arm they targeted is gone (see the
    // `RETIRED` comment on the match, above), so the first now fails
    // outright (`unrecognized verb_fqn`) and the second was passing
    // VACUOUSLY (an unrecognized FQN is `is_err()` too, but for the wrong
    // reason — not a real proof of wire-value rejection). Coverage for
    // "this FQN is no longer recognized" is not lost: it is folded into
    // `retired_verbs_are_no_longer_recognized`, below, alongside every
    // other retired verb this file already tracks the same way.

    #[test]
    fn remove_entity_id_in_target_and_payload() {
        let s = subj();
        let entity = Uuid::new_v4();
        let args = serde_json::json!({ "subject-id": s.0, "entity-id": entity });
        let (target, payload, _) =
            canonical_event_shape("kyc_ubo.assert.subject.remove", s, &args).unwrap();
        assert_eq!(target.entity_id, Some(EntityId(entity)));
        assert_eq!(payload["entity_id"], entity.to_string());
    }

    #[test]
    fn enquiry_kebab_lists_renamed_to_snake() {
        let s = subj();
        let args = serde_json::json!({
            "subject-id": s.0, "sources-consulted": ["GLEIF"], "searches-run": ["registry"]
        });
        let (_, payload, _) =
            canonical_event_shape("kyc_ubo.assert.subject.enquiry", s, &args).unwrap();
        assert_eq!(payload["sources_consulted"], serde_json::json!(["GLEIF"]));
        assert_eq!(payload["searches_run"], serde_json::json!(["registry"]));
    }

    #[test]
    fn connect_mints_edge_id_when_absent_and_returns_it() {
        let s = subj();
        let (from, to) = (Uuid::new_v4(), Uuid::new_v4());
        let args = serde_json::json!({
            "subject-id": s.0, "from_entity_id": from, "to_entity_id": to, "kind": "board_appointment"
        });
        let (target, payload, edge) =
            canonical_event_shape("kyc_ubo.assert.edge.connect", s, &args).unwrap();
        let minted = edge.expect("connect must mint and return an edge id");
        // `connect` is geometry-gated, not edge-scoped — its own target is
        // the bare subject (matching `enumerate_placement_set`'s per-triple
        // candidates); the mint flows via the payload and the return value.
        assert_eq!(target.edge_id, None);
        assert_eq!(payload["edge_id"], minted.0.to_string());
    }

    /// §8 Q3: a caller-supplied edge id is REFUSED — the system mints it.
    #[test]
    fn connect_rejects_caller_supplied_edge_id() {
        let s = subj();
        let chosen = Uuid::new_v4();
        let (from, to) = (Uuid::new_v4(), Uuid::new_v4());
        let args = serde_json::json!({
            "subject-id": s.0, "edge-id": chosen, "from_entity_id": from, "to_entity_id": to,
            "kind": "voting_rights"
        });
        assert!(canonical_event_shape("kyc_ubo.assert.edge.connect", s, &args).is_err());
    }

    #[test]
    fn connect_kebab_trust_revocable_and_pierced_from_renamed() {
        let s = subj();
        let (from, to, pierced) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let args = serde_json::json!({
            "subject-id": s.0, "from_entity_id": from, "to_entity_id": to,
            "kind": "trust_settlor", "trust-revocable": true, "pierced-from": pierced
        });
        let (_, payload, _) =
            canonical_event_shape("kyc_ubo.assert.edge.connect", s, &args).unwrap();
        assert_eq!(payload["trust_revocable"], true);
        assert_eq!(payload["pierced_from"], pierced.to_string());
    }

    #[test]
    fn connect_rejects_nominee_kind_with_pierced_from() {
        let s = subj();
        let (from, to, pierced) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let args = serde_json::json!({
            "subject-id": s.0, "from_entity_id": from, "to_entity_id": to,
            "kind": "nominee", "pierced-from": pierced
        });
        assert!(canonical_event_shape("kyc_ubo.assert.edge.connect", s, &args).is_err());
    }

    #[test]
    fn connect_rejects_unknown_kind() {
        let s = subj();
        let (from, to) = (Uuid::new_v4(), Uuid::new_v4());
        let args = serde_json::json!({
            "subject-id": s.0, "from_entity_id": from, "to_entity_id": to, "kind": "made_up_kind"
        });
        assert!(canonical_event_shape("kyc_ubo.assert.edge.connect", s, &args).is_err());
    }

    /// §3.2: connect absorbs assert-economic-interest — same verb, kind
    /// "economic_interest", percentage attribute.
    #[test]
    fn connect_economic_interest_kind_mints_and_returns_edge_id() {
        let s = subj();
        let (from, to) = (Uuid::new_v4(), Uuid::new_v4());
        let args = serde_json::json!({
            "subject-id": s.0, "from_entity_id": from, "to_entity_id": to,
            "kind": "economic_interest", "percentage": 25.0
        });
        let (target, payload, edge) =
            canonical_event_shape("kyc_ubo.assert.edge.connect", s, &args).unwrap();
        let minted = edge.unwrap();
        assert_eq!(target.edge_id, None);
        assert!(minted.0 != Uuid::nil());
        assert_eq!(payload["percentage"], 25.0);
        assert_eq!(payload["kind"], "economic_interest");
    }

    /// EOP-DD-UBO-PROOF-001 §1/§2 (T5): the three proof fields every
    /// `evidence` call must carry, regardless of edge- or entity-scoping.
    fn proof_args() -> serde_json::Value {
        serde_json::json!({ "kind": "filed-document", "source": "test fixture", "date": "2026-08-28" })
    }

    fn merge(mut base: serde_json::Value, extra: serde_json::Value) -> serde_json::Value {
        base.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        base
    }

    #[test]
    fn evidence_edge_scoped() {
        let s = subj();
        let edge = Uuid::new_v4();
        let args = merge(proof_args(), serde_json::json!({ "subject-id": s.0, "edge-id": edge }));
        let (target, payload, returned_edge) =
            canonical_event_shape("kyc_ubo.assert.edge.evidence", s, &args).unwrap();
        assert_eq!(target.edge_id, Some(EdgeId(edge)));
        assert_eq!(returned_edge, Some(EdgeId(edge)));
        assert_eq!(payload["kind"], "filed-document");
        assert_eq!(payload["source"], "test fixture");
        assert_eq!(payload["date"], "2026-08-28");
    }

    #[test]
    fn evidence_entity_scoped() {
        let s = subj();
        let entity = Uuid::new_v4();
        let args = merge(proof_args(), serde_json::json!({ "subject-id": s.0, "entity-id": entity }));
        let (target, payload, returned_edge) =
            canonical_event_shape("kyc_ubo.assert.edge.evidence", s, &args).unwrap();
        assert_eq!(target.entity_id, Some(EntityId(entity)));
        assert!(returned_edge.is_none());
        assert_eq!(payload["kind"], "filed-document");
    }

    #[test]
    fn evidence_rejects_both_edge_and_entity() {
        let s = subj();
        let args = merge(
            proof_args(),
            serde_json::json!({
                "subject-id": s.0, "edge-id": Uuid::new_v4(), "entity-id": Uuid::new_v4()
            }),
        );
        assert!(canonical_event_shape("kyc_ubo.assert.edge.evidence", s, &args).is_err());
    }

    #[test]
    fn evidence_rejects_neither_edge_nor_entity() {
        let s = subj();
        let args = merge(proof_args(), serde_json::json!({ "subject-id": s.0 }));
        assert!(canonical_event_shape("kyc_ubo.assert.edge.evidence", s, &args).is_err());
    }

    #[test]
    fn evidence_rejects_missing_proof_kind() {
        let s = subj();
        let edge = Uuid::new_v4();
        let args = serde_json::json!({
            "subject-id": s.0, "edge-id": edge, "source": "test fixture", "date": "2026-08-28"
        });
        assert!(canonical_event_shape("kyc_ubo.assert.edge.evidence", s, &args).is_err());
    }

    /// `kyc_ubo.assert.edge.verification` RETIRED (EOP-DD-UBO-PROOF-001
    /// §3/§4, T5, 2026-08-28) alongside its sub-test here — no ratchet
    /// left to target the edge with (K-G7: 0 real committed events).
    #[test]
    fn disconnect_targets_the_edge() {
        let s = subj();
        let edge = Uuid::new_v4();
        let args = serde_json::json!({ "subject-id": s.0, "edge-id": edge });
        let (target, _, returned_edge) =
            canonical_event_shape("kyc_ubo.assert.edge.disconnect", s, &args).unwrap();
        assert_eq!(target.edge_id, Some(EdgeId(edge)));
        assert_eq!(returned_edge, Some(EdgeId(edge)));
    }

    #[test]
    fn retract_targets_the_subject_with_a_citation_payload() {
        let s = subj();
        let citation = Uuid::new_v4();
        let args = serde_json::json!({ "subject-id": s.0, "citation-id": citation });
        let (target, payload, returned_edge) =
            canonical_event_shape("kyc_ubo.assert.edge.retract", s, &args).unwrap();
        assert_eq!(target.entity_id, None);
        assert!(returned_edge.is_none());
        assert_eq!(payload["citation_id"], citation.to_string());
    }

    #[test]
    fn retract_rejects_missing_citation() {
        let s = subj();
        let args = serde_json::json!({ "subject-id": s.0 });
        assert!(canonical_event_shape("kyc_ubo.assert.edge.retract", s, &args).is_err());
    }

    /// EOP-VS-UBO-GAME-001 T3, §3.3: reconciliation is retired, not merged —
    /// `canonical_event_shape` must refuse the FQN outright now.
    #[test]
    fn reconciliation_is_gone() {
        let s = subj();
        let args = serde_json::json!({ "subject-id": s.0 });
        assert!(canonical_event_shape("kyc_ubo.assert.edge.reconciliation", s, &args).is_err());
    }

    #[test]
    fn obligation_track_verbs_rename_obligation_and_subject_id() {
        let s = subj();
        let obligation = Uuid::new_v4();
        let args = serde_json::json!({ "subject-id": s.0, "obligation-id": obligation });
        for fqn in [
            "kyc_ubo.assert.entity.identity",
            "kyc_ubo.assert.entity.screening",
            "kyc_ubo.assert.entity.risk",
        ] {
            let (_, payload, _) = canonical_event_shape(fqn, s, &args).unwrap();
            assert_eq!(payload["obligation_id"], obligation.to_string(), "{fqn}");
            assert_eq!(payload["subject_id"], s.0.to_string(), "{fqn}");
        }
    }

    /// The three verbs this function must refuse, by design, not omission.
    #[test]
    fn excluded_verbs_bail_with_a_named_reason() {
        let s = subj();
        let args = serde_json::json!({ "subject-id": s.0 });
        for fqn in [
            "kyc_ubo.decide.determination.freeze",
            "kyc_ubo.decide.subject.approve",
            "kyc_ubo.decide.subject.reject",
            "kyc_ubo.decide.obligation.waiver",
        ] {
            let err = canonical_event_shape(fqn, s, &args).unwrap_err();
            assert!(!err.to_string().is_empty(), "{fqn}");
        }
    }

    #[test]
    fn unknown_verb_fqn_bails() {
        let s = subj();
        let args = serde_json::json!({});
        assert!(canonical_event_shape("not.a.real.verb", s, &args).is_err());
    }

    /// T2 (EOP-VS-UBO-GAME-001 §3.2) retired `register`/`type` (merged into
    /// `place`) and `member-withdrawal` (renamed `remove`). A stale match
    /// arm silently reappearing for any of these would let a second
    /// constructor back in through the side door; this guards that.
    #[test]
    fn retired_verbs_are_no_longer_recognized() {
        let s = subj();
        let args = serde_json::json!({ "subject-id": s.0, "entity-id": Uuid::new_v4() });
        for fqn in [
            "kyc_ubo.assert.subject.register",
            "kyc_ubo.assert.subject.type",
            "kyc_ubo.assert.subject.type-correction",
            "kyc_ubo.assert.subject.member-withdrawal",
            "kyc_ubo.assert.subject.structure-class",
            "kyc_ubo.assert.edge.economic-interest",
            "kyc_ubo.assert.edge.supersession",
            "kyc_ubo.assert.edge.reconciliation",
            "kyc_ubo.assert.edge.verification",
        ] {
            assert!(canonical_event_shape(fqn, s, &args).is_err(), "{fqn} must no longer be recognized");
        }
    }
}
