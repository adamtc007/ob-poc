//! Structured byte-tape generator for the `ob-poc-kyc-substrate` fuzz
//! targets (EOP-FUZZ-KYCUBO-001 F1/F2), modeled on bpmn-lite's
//! `bpmn-lite-kernel-fuzz` `Tape` (fork F-A: structured generators over the
//! libFuzzer byte tape, not naive `derive(Arbitrary)` on production types —
//! naive arbitrary JSON yields ~100% verifier-rejected garbage and near-zero
//! interesting-branch coverage).
//!
//! `gen_events` builds a plausible-but-adversarial `IntentEvent` sequence:
//! verb FQNs drawn from the 16 known dsl.kyc verbs (plus deliberately
//! unknown ones, to keep `apply_one_control_event`'s total-dispatch
//! fallthrough hot), payloads carrying every field any control-fold verb
//! reads, and a small reused id pool so edges/entities/persons actually
//! chain together across events instead of every lookup missing.

use ob_poc_kyc_substrate::{
    AuthorityRef, EdgeId, IntentEvent, Principal, SubjectId, TargetBinding, VerbFqn,
    EDGE_KIND_WIRE_VALUES,
};
use uuid::Uuid;

/// All 14 live dsl.kyc verb FQNs (control-fold verbs, obligation-fold verbs,
/// and determination-layer verbs the pure folds ignore) plus a couple of
/// deliberately-unknown FQNs, so every fold's total-dispatch fallthrough
/// (D2) stays hot alongside its real match arms. `fold_control` and
/// `fold_obligations` each only react to their own subset — every other
/// verb here exercises their harmless `_ => {}` arm instead.
/// `select-strategy`/`compute-fold` (TS.6 P2, K-G7) and `pierce-nominee`
/// (TS.6 P2, folded into a macro composing `assert-control`+`supersede`)
/// dropped from this list — none of the three has a dedicated fold arm any
/// more, so their strings would just be more `not.a.real.verb`-style
/// fallthrough noise, indistinguishable from the two deliberate unknowns
/// already below. `kyc.person.approve`/`.reject` also dropped (TS.6 P1/P2)
/// — renamed `decide.approve`/`.reject` and moved to `ob-poc-kyc-decide`,
/// which never appends to the fact stream at all, so neither fold has ever
/// had a dispatch arm for them (this fuzz target only exercises
/// `apply_one_control_event`/`apply_one_obligation_event`, never
/// `ob-poc-kyc-decide`'s DB-backed ops).
const ALL_VERBS: &[&str] = &[
    "kyc.subject.register",
    "kyc.subject.classify-structure",
    "ubo.edge.assert-economic-interest",
    "ubo.edge.assert-control",
    "ubo.edge.attach-evidence",
    "ubo.edge.verify",
    "ubo.edge.supersede",
    "ubo.edge.reconcile-conflict",
    "ubo.determination.apply-smo-fallback",
    "ubo.determination.freeze",
    "kyc.obligation.create",
    "assert.identity",
    "assert.screening",
    "assert.risk",
    "kyc.obligation.satisfy",
    "kyc.obligation.waive",
    // Genuinely unknown FQNs — historical/garbage-event fallthrough (D2).
    "not.a.real.verb",
    "",
];

/// Structure-class wire strings `structure_class_from_payload` recognizes
/// (private in the substrate — mirrored here since only the match arms
/// are visible, not the list itself; see EOP-FUZZ-KYCUBO-001 §5 for the
/// asymmetry this partly probes: unlike `EDGE_KIND_WIRE_VALUES`, there is
/// no exported/const source of truth for this set).
const STRUCTURE_CLASS_VALUES: &[&str] = &[
    "private_company",
    "multi_tier_holding",
    "listed_entity",
    "lp_fund",
    "llp",
    "trust",
    "foundation",
    "investment_fund",
    "state_owned",
    "cooperative",
    "nominee",
];

pub struct Tape<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Tape<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn u8(&mut self) -> u8 {
        let byte = self.data.get(self.pos).copied().unwrap_or(0);
        self.pos += 1;
        byte
    }

    pub fn bool(&mut self) -> bool {
        self.u8() & 1 == 1
    }

    /// Uniform choice in `0..n` (`n` must be > 0).
    pub fn choice(&mut self, n: usize) -> usize {
        debug_assert!(n > 0);
        self.u8() as usize % n
    }

    /// Bounded f64 in `[-50.0, 1050.0]` — deliberately allows values outside
    /// the sane `[0, 100]` percentage range (adversarial), never NaN/inf
    /// (JSON can't carry those, and a fuzz target should probe the fold's
    /// own logic, not `serde_json::Number::from_f64`'s float-encoding
    /// behavior).
    pub fn percentage(&mut self) -> f64 {
        (self.u8() as f64) * (1100.0 / 255.0) - 50.0
    }

    fn garbage_string(&mut self) -> String {
        let len = self.choice(12);
        (0..len).map(|_| self.u8() as char).collect()
    }
}

/// A UUID-ish string: sometimes a fresh valid UUID, sometimes reused from
/// `pool` (so edges/entities actually chain across events), sometimes
/// garbage (exercises the `Uuid::parse_str(..).ok()` `None` path).
fn pick_uuid_like(tape: &mut Tape, pool: &[String]) -> Option<String> {
    match tape.choice(4) {
        0 => None,
        1 if !pool.is_empty() => Some(pool[tape.choice(pool.len())].clone()),
        2 => Some(Uuid::new_v4().to_string()),
        _ => Some(tape.garbage_string()),
    }
}

fn pick_str(tape: &mut Tape, known: &[&str]) -> Option<String> {
    match tape.choice(3) {
        0 => None,
        1 => Some(known[tape.choice(known.len())].to_string()),
        _ => Some(tape.garbage_string()),
    }
}

/// Track-state wire strings `track_state_from_event` recognizes
/// (`fold/obligation.rs::track_state_from_event`, private).
const TRACK_STATE_VALUES: &[&str] =
    &["in_progress", "satisfied", "waived", "deferred", "expired"];

#[allow(clippy::too_many_arguments)]
fn build_payload(
    tape: &mut Tape,
    entity_pool: &[String],
    edge_pool: &[String],
    person_pool: &[String],
    obligation_pool: &[String],
    subject_root: &str,
) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    let mut put = |key: &str, val: Option<serde_json::Value>| {
        if let Some(v) = val {
            map.insert(key.to_string(), v);
        }
    };
    put(
        "entity_id",
        pick_uuid_like(tape, entity_pool).map(serde_json::Value::from),
    );
    put(
        "from_entity_id",
        pick_uuid_like(tape, entity_pool).map(serde_json::Value::from),
    );
    put(
        "to_entity_id",
        pick_uuid_like(tape, entity_pool).map(serde_json::Value::from),
    );
    put(
        "nominator_entity_id",
        pick_uuid_like(tape, entity_pool).map(serde_json::Value::from),
    );
    put(
        "edge_id",
        pick_uuid_like(tape, edge_pool).map(serde_json::Value::from),
    );
    // TS.6 P2: `ubo.edge.assert-control`'s new provenance field — the
    // `pierce-nominee` macro's `pierced-from` arg, normalized to this key.
    put(
        "pierced_from",
        pick_uuid_like(tape, edge_pool).map(serde_json::Value::from),
    );
    if tape.bool() {
        put("percentage", Some(serde_json::json!(tape.percentage())));
    }
    if tape.bool() {
        put("trust_revocable", Some(serde_json::json!(tape.bool())));
    }
    put(
        "kind",
        pick_str(tape, EDGE_KIND_WIRE_VALUES).map(serde_json::Value::from),
    );
    put(
        "structure_class",
        pick_str(tape, STRUCTURE_CLASS_VALUES).map(serde_json::Value::from),
    );
    put(
        "strategy",
        pick_str(tape, &["ownership_prong_strategy", "control_prong_strategy"])
            .map(serde_json::Value::from),
    );
    put(
        "smo_person_id",
        pick_uuid_like(tape, person_pool).map(serde_json::Value::from),
    );
    // Obligation-fold fields (`fold/obligation.rs::apply_one_obligation_event`).
    put(
        "obligation_id",
        pick_uuid_like(tape, obligation_pool).map(serde_json::Value::from),
    );
    // `subject_id_from_event` falls back to this payload field when
    // `target.subject_root` is unset — usually the real subject_root (so
    // obligations actually attach to the subject `gen_events` registered),
    // sometimes fresh/garbage (miss-lookup path).
    put(
        "subject_id",
        pick_uuid_like(tape, &[subject_root.to_string()]).map(serde_json::Value::from),
    );
    put(
        "role",
        pick_str(tape, &["controller", "beneficial_owner", "signatory"])
            .map(serde_json::Value::from),
    );
    put(
        "jurisdiction",
        pick_str(tape, &["GB", "US", "KY"]).map(serde_json::Value::from),
    );
    put(
        "cbu_role",
        pick_str(tape, &["asset_owner", "manco"]).map(serde_json::Value::from),
    );
    put(
        "state",
        pick_str(tape, TRACK_STATE_VALUES).map(serde_json::Value::from),
    );
    if tape.bool() {
        put(
            "reason",
            Some(serde_json::Value::from(tape.garbage_string())),
        );
    }
    serde_json::Value::Object(map)
}

fn build_target(tape: &mut Tape, edge_pool: &[String], subject_root: SubjectId) -> TargetBinding {
    let edge_id = match tape.choice(3) {
        0 => None,
        1 if !edge_pool.is_empty() => Uuid::parse_str(&edge_pool[tape.choice(edge_pool.len())])
            .ok()
            .map(EdgeId),
        _ => Some(EdgeId(Uuid::new_v4())),
    };
    // Real callers set `target.subject_root` from session scope; leaving it
    // unset sometimes exercises `subject_id_from_event`'s payload-field
    // fallback instead.
    let target_subject_root = if tape.bool() {
        Some(subject_root)
    } else {
        None
    };
    TargetBinding {
        edge_id,
        subject_root: target_subject_root,
        ..Default::default()
    }
}

/// Build a plausible-but-adversarial event sequence (1..=20 events) sharing
/// one `subject_root` and small entity/edge/person/obligation id pools.
/// Drives both the control fold and the obligation fold — `ALL_VERBS`
/// spans both verb sets (plus the determination-layer verbs neither fold
/// reacts to, and a couple of genuinely-unknown FQNs).
pub fn gen_events(tape: &mut Tape) -> Vec<IntentEvent> {
    let subject_root = SubjectId(Uuid::new_v4());
    let entity_pool: Vec<String> = (0..4).map(|_| Uuid::new_v4().to_string()).collect();
    let edge_pool: Vec<String> = (0..3).map(|_| Uuid::new_v4().to_string()).collect();
    let person_pool: Vec<String> = (0..2).map(|_| Uuid::new_v4().to_string()).collect();
    let obligation_pool: Vec<String> = (0..3).map(|_| Uuid::new_v4().to_string()).collect();
    let subject_root_str = subject_root.0.to_string();

    let n = 1 + tape.choice(20);
    let mut events = Vec::with_capacity(n);
    for i in 0..n {
        let verb: VerbFqn = ALL_VERBS[tape.choice(ALL_VERBS.len())].into();
        let payload = build_payload(
            tape,
            &entity_pool,
            &edge_pool,
            &person_pool,
            &obligation_pool,
            &subject_root_str,
        );
        let target = build_target(tape, &edge_pool, subject_root);
        let actor = Principal {
            actor_id: Uuid::new_v4(),
            role: "fuzz".to_string(),
        };
        let authority = AuthorityRef("fuzz-authority".to_string());
        let as_of = chrono::DateTime::from_timestamp(1_700_000_000 + i as i64, 0)
            .expect("fixed-base timestamp is always in range");
        let event = IntentEvent::new(subject_root, verb, actor, authority, target, payload, as_of)
            .with_seq(i as u64);
        events.push(event);
    }
    events
}
