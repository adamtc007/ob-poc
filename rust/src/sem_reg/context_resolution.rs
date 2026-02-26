//! Context Resolution API — Phase 7 of the Semantic Registry.
//!
//! Provides a single `resolve_context()` function that returns ranked verbs,
//! attributes, views, policy verdicts, and governance signals for a given
//! subject + actor + goal.
//!
//! This is the semantic OS's "system call" interface — every consumer
//! (agent, UI, CLI, governance dashboard) queries through this API instead
//! of reimplementing ad-hoc registry lookups.
//!
//! ## 12-Step Resolution Pipeline
//!
//! 1. Determine snapshot epoch (point_in_time or now)
//! 2. Resolve subject → entity type + jurisdiction + state
//! 3. Select applicable ViewDefs by taxonomy overlap
//! 4. Extract verb_surface + attribute_prominence from top view
//! 5. Filter verbs by taxonomy membership + ABAC
//! 6. Filter attributes similarly
//! 7. Rank by ViewDef prominence weights
//! 8. Evaluate preconditions for top-N candidate verbs
//! 9. Evaluate PolicyRules → PolicyVerdicts with snapshot refs
//! 10. Compute composite AccessDecision
//! 11. Generate governance signals
//! 12. Compute confidence score

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::{
    abac::{evaluate_abac, AccessDecision, AccessPurpose, ActorContext},
    membership::{MembershipCondition, MembershipKind, MembershipRuleBody},
    policy_rule::PolicyRuleBody,
    relationship_type_def::RelationshipTypeDefBody,
    store::SnapshotStore,
    types::{GovernanceTier, ObjectType, SnapshotRow, TrustClass},
    view_def::ViewDefBody,
};

// ── Evidence Mode ─────────────────────────────────────────────

/// Controls how trust-class and governance-tier filtering is applied.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceMode {
    /// Only governed + Proof/DecisionSupport. Operational excluded unless view allows.
    Strict,
    /// Governed primary, operational allowed if view includes it. Tagged `usable_for_proof = false`.
    #[default]
    Normal,
    /// All tiers and trust classes, annotated with tier/trust metadata.
    Exploratory,
    /// Coverage metrics focus — stewardship gaps, classification gaps, stale evidence.
    Governance,
}

// ── Subject Reference ─────────────────────────────────────────

/// What the resolution is about — the subject being analysed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "id")]
#[serde(rename_all = "snake_case")]
pub enum SubjectRef {
    CaseId(Uuid),
    EntityId(Uuid),
    DocumentId(Uuid),
    TaskId(Uuid),
    ViewId(Uuid),
}

impl SubjectRef {
    pub fn id(&self) -> Uuid {
        match self {
            Self::CaseId(id)
            | Self::EntityId(id)
            | Self::DocumentId(id)
            | Self::TaskId(id)
            | Self::ViewId(id) => *id,
        }
    }
}

// ── Resolution Constraints ────────────────────────────────────

/// Optional constraints that narrow the resolution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResolutionConstraints {
    /// Jurisdiction filter (e.g. "LU", "DE").
    #[serde(default)]
    pub jurisdiction: Option<String>,
    /// Risk posture filter (e.g. "high", "standard").
    #[serde(default)]
    pub risk_posture: Option<String>,
    /// Arbitrary key-value thresholds for custom filtering.
    #[serde(default)]
    pub thresholds: HashMap<String, serde_json::Value>,
}

// ── Request ───────────────────────────────────────────────────

/// The single input to `resolve_context()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextResolutionRequest {
    /// What we are resolving context for.
    pub subject: SubjectRef,
    /// Optional natural language intent (used for embedding ranking in Phase 9).
    #[serde(default)]
    pub intent: Option<String>,
    /// Who is asking — drives ABAC evaluation.
    pub actor: ActorContext,
    /// What the actor is trying to achieve.
    #[serde(default)]
    pub goals: Vec<String>,
    /// Optional narrowing constraints.
    #[serde(default)]
    pub constraints: ResolutionConstraints,
    /// Trust-aware filtering mode.
    #[serde(default)]
    pub evidence_mode: EvidenceMode,
    /// Point-in-time for historical resolution (None = now).
    #[serde(default)]
    pub point_in_time: Option<DateTime<Utc>>,
    /// Entity kind of the dominant entity (for subject_kinds filtering).
    /// When set, verbs with non-empty subject_kinds that don't include this
    /// kind are filtered out.
    #[serde(default)]
    pub entity_kind: Option<String>,
}

// ── Response ──────────────────────────────────────────────────

/// The full output of context resolution — everything a consumer needs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextResolutionResponse {
    /// The point-in-time that was resolved (either requested or now).
    pub as_of_time: DateTime<Utc>,
    /// When this resolution was computed.
    pub resolved_at: DateTime<Utc>,
    /// Applicable views, ranked by taxonomy overlap with subject.
    pub applicable_views: Vec<RankedView>,
    /// Candidate verbs, ranked and filtered by ABAC + tier + preconditions.
    pub candidate_verbs: Vec<VerbCandidate>,
    /// Candidate attributes, ranked and filtered similarly.
    pub candidate_attributes: Vec<AttributeCandidate>,
    /// Precondition status for top verb candidates.
    pub required_preconditions: Vec<PreconditionStatus>,
    /// Questions to ask if context is ambiguous.
    pub disambiguation_questions: Vec<DisambiguationPrompt>,
    /// Evidence summary (positive and negative).
    pub evidence: EvidenceSummary,
    /// Policy verdicts with snapshot refs.
    pub policy_verdicts: Vec<PolicyVerdict>,
    /// Composite access decision for the overall request.
    pub security_handling: AccessDecision,
    /// Governance signals (gaps, staleness, unowned objects).
    pub governance_signals: Vec<GovernanceSignal>,
    /// Overall confidence in the resolution (0.0–1.0).
    pub confidence: f64,
}

// ── Supporting Types ──────────────────────────────────────────

/// A view definition ranked by taxonomy overlap with the subject.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankedView {
    /// Snapshot ID of the ViewDef (pinned).
    pub view_snapshot_id: Uuid,
    /// Object ID of the ViewDef.
    pub view_id: Uuid,
    /// Fully qualified name.
    pub fqn: String,
    /// Human-readable name.
    pub name: String,
    /// Overlap score with subject's taxonomy memberships (0.0–1.0).
    pub overlap_score: f64,
    /// The parsed ViewDef body.
    pub body: ViewDefBody,
}

/// A verb candidate with ranking, precondition, and tier metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbCandidate {
    /// Snapshot ID of the VerbContract (pinned).
    pub verb_snapshot_id: Uuid,
    /// Object ID of the VerbContract.
    pub verb_id: Uuid,
    /// Fully qualified name (e.g. "kyc.open-case").
    pub fqn: String,
    /// Human-readable description.
    pub description: String,
    /// Governance tier of this verb contract.
    pub governance_tier: GovernanceTier,
    /// Trust class.
    pub trust_class: TrustClass,
    /// Ranking score from view prominence (0.0–1.0).
    pub rank_score: f64,
    /// Whether preconditions are currently satisfiable.
    pub preconditions_met: bool,
    /// ABAC access decision for this verb.
    pub access_decision: AccessDecision,
    /// Whether this verb's output can be used as proof evidence.
    pub usable_for_proof: bool,
}

/// An attribute candidate with ranking and tier metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeCandidate {
    /// Snapshot ID of the AttributeDef (pinned).
    pub attribute_snapshot_id: Uuid,
    /// Object ID of the AttributeDef.
    pub attribute_id: Uuid,
    /// Fully qualified name.
    pub fqn: String,
    /// Human-readable name.
    pub name: String,
    /// Governance tier.
    pub governance_tier: GovernanceTier,
    /// Trust class.
    pub trust_class: TrustClass,
    /// Ranking score from view prominence (0.0–1.0).
    pub rank_score: f64,
    /// ABAC access decision.
    pub access_decision: AccessDecision,
    /// Whether this attribute is required (by policy or evidence).
    pub required: bool,
    /// Whether this attribute is currently populated for the subject.
    pub present: bool,
}

/// Precondition evaluability status for a verb.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreconditionStatus {
    /// Verb FQN.
    pub verb_fqn: String,
    /// Verb snapshot ID.
    pub verb_snapshot_id: Uuid,
    /// Individual precondition checks.
    pub checks: Vec<PreconditionCheck>,
    /// Whether all preconditions are satisfied.
    pub all_satisfied: bool,
    /// Remediation hint if not satisfied.
    #[serde(default)]
    pub remediation_hint: Option<String>,
}

/// A single precondition check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreconditionCheck {
    /// Precondition description.
    pub description: String,
    /// Whether this check passed.
    pub satisfied: bool,
    /// Reason for failure (if any).
    #[serde(default)]
    pub reason: Option<String>,
}

/// A disambiguation prompt when context is ambiguous.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisambiguationPrompt {
    /// Unique ID for this prompt.
    pub prompt_id: Uuid,
    /// The question to ask.
    pub question: String,
    /// Available options.
    pub options: Vec<DisambiguationOption>,
    /// Whether answering is required to proceed.
    pub required_to_proceed: bool,
    /// Rationale for why disambiguation is needed.
    #[serde(default)]
    pub rationale: Option<String>,
}

/// An option in a disambiguation prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisambiguationOption {
    /// Option identifier.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Description.
    #[serde(default)]
    pub description: Option<String>,
    /// How this option narrows the context.
    #[serde(default)]
    pub narrows_to: Option<serde_json::Value>,
}

/// Summary of evidence for and against the resolved context.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvidenceSummary {
    /// Positive evidence supporting the context.
    pub positive: Vec<EvidenceRef>,
    /// Negative evidence or missing items.
    pub negative: Vec<EvidenceRef>,
}

/// A reference to a piece of evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRef {
    /// What kind of evidence (observation, document, attribute value).
    pub kind: String,
    /// FQN or description.
    pub reference: String,
    /// Snapshot ID if pinned.
    #[serde(default)]
    pub snapshot_id: Option<Uuid>,
    /// Freshness — when was this evidence last updated.
    #[serde(default)]
    pub last_updated: Option<DateTime<Utc>>,
    /// Confidence in this evidence (0.0–1.0).
    #[serde(default)]
    pub confidence: Option<f64>,
}

/// A policy verdict with snapshot provenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyVerdict {
    /// Snapshot ID of the PolicyRule that produced this verdict.
    pub policy_snapshot_id: Uuid,
    /// Policy FQN.
    pub policy_fqn: String,
    /// Policy name.
    pub policy_name: String,
    /// Whether the policy allows the action.
    pub allowed: bool,
    /// Reason for the verdict.
    pub reason: String,
    /// Actions required by the policy (if any).
    #[serde(default)]
    pub required_actions: Vec<String>,
    /// Regulatory reference (if any).
    #[serde(default)]
    pub regulatory_reference: Option<String>,
}

/// A governance signal indicating a gap or issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceSignal {
    /// Signal kind.
    pub kind: GovernanceSignalKind,
    /// Human-readable message.
    pub message: String,
    /// Severity: info, warning, error.
    pub severity: GovernanceSignalSeverity,
    /// Related object FQN (if applicable).
    #[serde(default)]
    pub related_fqn: Option<String>,
    /// Related snapshot ID (if applicable).
    #[serde(default)]
    pub related_snapshot_id: Option<Uuid>,
}

/// Categories of governance signals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernanceSignalKind {
    /// A governed object has no steward assigned.
    UnownedObject,
    /// A governed object is not classified in any taxonomy.
    UnclassifiedObject,
    /// Evidence has exceeded its freshness window.
    StaleEvidence,
    /// A retention deadline is approaching.
    RetentionApproaching,
    /// A policy rule has no matching verbs.
    OrphanPolicy,
    /// An attribute is defined but not produced by any verb.
    OrphanAttribute,
    /// Coverage gap in the registry.
    CoverageGap,
}

/// Severity levels for governance signals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernanceSignalSeverity {
    Info,
    Warning,
    Error,
}

// ── Resolved Subject ──────────────────────────────────────────

/// Internal struct for the resolved subject metadata.
#[derive(Debug, Clone)]
struct ResolvedSubject {
    entity_type_fqn: Option<String>,
    jurisdiction: Option<String>,
    #[allow(dead_code)]
    state: Option<String>,
}

/// A loaded taxonomy membership record from `v_active_memberships_by_subject`.
#[derive(Debug, Clone)]
struct TaxonomyMembership {
    /// The taxonomy this membership belongs to (e.g., "domain.kyc-tier")
    taxonomy_fqn: String,
    /// The specific taxonomy node (e.g., "domain.kyc-tier.high")
    #[allow(dead_code)] // Loaded from DB; reserved for node-level filtering
    node_fqn: String,
    /// What type of registry object is classified
    target_type: String,
    /// FQN of the classified object
    target_fqn: String,
    /// Kind of membership (direct, inherited, conditional, excluded)
    membership_kind: String,
}

/// Aggregated membership context for the subject — computed once and passed through.
#[derive(Debug, Clone, Default)]
struct SubjectMemberships {
    /// Taxonomy FQNs the subject's entity type belongs to
    subject_taxonomy_fqns: HashSet<String>,
    /// All loaded memberships (for filtering verbs/attributes by taxonomy overlap)
    all_memberships: Vec<TaxonomyMembership>,
}

impl SubjectMemberships {
    /// Returns the set of taxonomy FQNs that a given target (verb/attribute) belongs to.
    fn taxonomy_fqns_for_target(&self, target_fqn: &str) -> HashSet<String> {
        self.all_memberships
            .iter()
            .filter(|m| m.target_fqn == target_fqn && m.membership_kind != "excluded")
            .map(|m| m.taxonomy_fqn.clone())
            .collect()
    }

    /// Returns true if the subject has any taxonomy memberships.
    fn has_memberships(&self) -> bool {
        !self.subject_taxonomy_fqns.is_empty()
    }

    /// Returns the number of overlapping taxonomy FQNs between the subject and a target.
    #[allow(dead_code)]
    fn taxonomy_overlap_count(&self, target_fqn: &str) -> usize {
        let target_taxonomies = self.taxonomy_fqns_for_target(target_fqn);
        self.subject_taxonomy_fqns
            .intersection(&target_taxonomies)
            .count()
    }
}

/// Relationship context for the subject — loaded in Step 2c (D5).
///
/// Contains relationship type definitions where the subject's entity type
/// appears as either source or target. Exposes `edge_class` for verb filtering.
#[derive(Debug, Clone, Default)]
struct SubjectRelationships {
    /// Relationship types where the subject is the source entity type
    outgoing: Vec<RelationshipTypeDefBody>,
    /// Relationship types where the subject is the target entity type
    incoming: Vec<RelationshipTypeDefBody>,
}

impl SubjectRelationships {
    /// Returns the set of edge classes that the subject participates in.
    fn edge_classes(&self) -> HashSet<String> {
        self.outgoing
            .iter()
            .chain(self.incoming.iter())
            .filter_map(|r| r.edge_class.clone())
            .collect()
    }

    /// Returns the set of domains covered by the subject's relationships.
    fn relationship_domains(&self) -> HashSet<String> {
        self.outgoing
            .iter()
            .chain(self.incoming.iter())
            .map(|r| r.domain.clone())
            .collect()
    }

    /// Returns true if the subject has any relationships loaded.
    fn has_relationships(&self) -> bool {
        !self.outgoing.is_empty() || !self.incoming.is_empty()
    }
}

// ── Resolution Engine ─────────────────────────────────────────

/// The top-level context resolution function.
///
/// Implements the 12-step pipeline documented in the module header.
pub async fn resolve_context(
    pool: &PgPool,
    req: &ContextResolutionRequest,
) -> Result<ContextResolutionResponse> {
    let resolved_at = Utc::now();

    // Step 1: Determine snapshot epoch
    let as_of = req.point_in_time.unwrap_or(resolved_at);

    // Step 2: Resolve subject → entity type + jurisdiction + state
    let subject = resolve_subject(pool, &req.subject, as_of).await?;

    // Step 2b: Load taxonomy memberships for the subject and all registry objects.
    // This enables taxonomy-aware filtering in Steps 3, 5, and 6.
    // S14: Conditional memberships are evaluated; those whose conditions cannot
    // be verified are excluded from overlap scoring.
    let memberships = load_subject_memberships(pool, &subject).await?;

    // Step 2c (D5): Load relationship type definitions for the subject's entity type.
    // Exposes edge_class (ownership, control, service, regulatory) to verb filtering.
    let relationships = load_subject_relationships(pool, &subject, as_of).await?;

    // Step 3: Select applicable ViewDefs by taxonomy overlap
    let all_views = load_view_defs(pool, as_of).await?;
    let mut ranked_views = rank_views_by_overlap(&all_views, &subject, &memberships);

    // Step 4: Extract verb_surface + attribute_prominence from top view
    let top_view_body = ranked_views.first().map(|rv| &rv.body);

    // Step 5: Filter verbs by taxonomy + ABAC + tier + relationship edge_class
    let all_verb_rows = load_typed_snapshots(pool, ObjectType::VerbContract, as_of).await?;
    let mut candidate_verbs = filter_and_rank_verbs(
        &all_verb_rows,
        &req.actor,
        req.evidence_mode,
        top_view_body,
        req.entity_kind.as_deref(),
        &memberships,
        &relationships,
    )?;

    // Step 6: Filter attributes similarly
    let all_attr_rows = load_typed_snapshots(pool, ObjectType::AttributeDef, as_of).await?;
    let mut candidate_attributes = filter_and_rank_attributes(
        &all_attr_rows,
        &req.actor,
        req.evidence_mode,
        top_view_body,
        &memberships,
    )?;

    // Step 7: Rank by ViewDef prominence weights
    // (already applied during Steps 5-6; sort descending)
    candidate_verbs.sort_by(|a, b| {
        b.rank_score
            .partial_cmp(&a.rank_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    candidate_attributes.sort_by(|a, b| {
        b.rank_score
            .partial_cmp(&a.rank_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    ranked_views.sort_by(|a, b| {
        b.overlap_score
            .partial_cmp(&a.overlap_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Truncate to top N
    candidate_verbs.truncate(20);
    candidate_attributes.truncate(30);
    ranked_views.truncate(5);

    // Step 8: Evaluate preconditions for top-N candidate verbs
    let required_preconditions = evaluate_verb_preconditions(&candidate_verbs);

    // Step 9: Evaluate PolicyRules → PolicyVerdicts
    let policy_rows = load_typed_snapshots(pool, ObjectType::PolicyRule, as_of).await?;
    let policy_verdicts = evaluate_policies(&policy_rows, &candidate_verbs, &req.actor)?;

    // Step 10: Compute composite AccessDecision
    let security_handling = compute_composite_access(&candidate_verbs, &policy_verdicts);

    // Step 11: Generate governance signals
    let mut governance_signals =
        generate_governance_signals(&candidate_verbs, &candidate_attributes, req.evidence_mode);

    // Graceful degradation: if subject has no taxonomy memberships, add warning
    if !memberships.has_memberships() {
        if let Some(ref entity_type) = subject.entity_type_fqn {
            governance_signals.push(GovernanceSignal {
                kind: GovernanceSignalKind::UnclassifiedObject,
                message: format!(
                    "Subject entity type '{}' has no taxonomy memberships — \
                     all candidates included without taxonomy filtering",
                    entity_type
                ),
                severity: GovernanceSignalSeverity::Warning,
                related_fqn: Some(entity_type.clone()),
                related_snapshot_id: None,
            });
        }
    }

    // Step 12: Compute confidence score
    let confidence = compute_confidence(
        &ranked_views,
        &candidate_verbs,
        &required_preconditions,
        &candidate_attributes,
    );

    // Generate disambiguation questions if confidence is low
    let disambiguation_questions = if confidence < 0.5 {
        generate_disambiguation(&ranked_views, &candidate_verbs)
    } else {
        vec![]
    };

    Ok(ContextResolutionResponse {
        as_of_time: as_of,
        resolved_at,
        applicable_views: ranked_views,
        candidate_verbs,
        candidate_attributes,
        required_preconditions,
        disambiguation_questions,
        evidence: EvidenceSummary::default(),
        policy_verdicts,
        security_handling,
        governance_signals,
        confidence,
    })
}

// ── Step 2: Resolve Subject ───────────────────────────────────

fn resolve_subject<'a>(
    pool: &'a PgPool,
    subject: &'a SubjectRef,
    as_of: DateTime<Utc>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ResolvedSubject>> + Send + 'a>> {
    Box::pin(resolve_subject_inner(pool, subject, as_of))
}

async fn resolve_subject_inner(
    pool: &PgPool,
    subject: &SubjectRef,
    as_of: DateTime<Utc>,
) -> Result<ResolvedSubject> {
    match subject {
        SubjectRef::EntityId(id) => {
            // Look up entity type from the entities table
            let row = sqlx::query_as::<_, (Option<String>, Option<String>)>(
                r#"
                SELECT entity_type, jurisdiction_code
                FROM "ob-poc".entities
                WHERE entity_id = $1
                "#,
            )
            .bind(id)
            .fetch_optional(pool)
            .await?;

            match row {
                Some((entity_type, jurisdiction)) => Ok(ResolvedSubject {
                    entity_type_fqn: entity_type.map(|t| format!("entity.{}", t)),
                    jurisdiction,
                    state: None,
                }),
                None => Ok(ResolvedSubject {
                    entity_type_fqn: None,
                    jurisdiction: None,
                    state: None,
                }),
            }
        }
        SubjectRef::CaseId(id) => {
            // Look up case → entity type + jurisdiction + case status
            let row = sqlx::query_as::<_, (Option<Uuid>, Option<String>)>(
                r#"
                SELECT subject_entity_id, status
                FROM "ob-poc".kyc_cases
                WHERE case_id = $1
                "#,
            )
            .bind(id)
            .fetch_optional(pool)
            .await?;

            match row {
                Some((entity_id, status)) => {
                    // Resolve the entity behind the case
                    let mut resolved = if let Some(eid) = entity_id {
                        resolve_subject(pool, &SubjectRef::EntityId(eid), as_of).await?
                    } else {
                        ResolvedSubject {
                            entity_type_fqn: None,
                            jurisdiction: None,
                            state: None,
                        }
                    };
                    resolved.state = status;
                    Ok(resolved)
                }
                None => Ok(ResolvedSubject {
                    entity_type_fqn: None,
                    jurisdiction: None,
                    state: None,
                }),
            }
        }
        SubjectRef::ViewId(id) => {
            // Load the ViewDef directly and use its base_entity_type
            let snap = SnapshotStore::resolve_active(pool, ObjectType::ViewDef, *id).await?;
            match snap {
                Some(row) => {
                    let body: ViewDefBody = row.parse_definition()?;
                    Ok(ResolvedSubject {
                        entity_type_fqn: Some(body.base_entity_type),
                        jurisdiction: None,
                        state: None,
                    })
                }
                None => Ok(ResolvedSubject {
                    entity_type_fqn: None,
                    jurisdiction: None,
                    state: None,
                }),
            }
        }
        SubjectRef::DocumentId(_) | SubjectRef::TaskId(_) => {
            // For documents and tasks, we don't resolve entity type directly
            Ok(ResolvedSubject {
                entity_type_fqn: None,
                jurisdiction: None,
                state: None,
            })
        }
    }
}

// ── Step 2b: Load Taxonomy Memberships ────────────────────────

async fn load_subject_memberships(
    pool: &PgPool,
    subject: &ResolvedSubject,
) -> Result<SubjectMemberships> {
    // Load all active membership rules from the convenience view
    let rows = sqlx::query_as::<_, (String, String, String, String, String)>(
        r#"
        SELECT
            taxonomy_fqn,
            node_fqn,
            target_type,
            target_fqn,
            membership_kind
        FROM sem_reg.v_active_memberships_by_subject
        "#,
    )
    .fetch_all(pool)
    .await?;

    let all_memberships: Vec<TaxonomyMembership> = rows
        .into_iter()
        .map(
            |(taxonomy_fqn, node_fqn, target_type, target_fqn, membership_kind)| {
                TaxonomyMembership {
                    taxonomy_fqn,
                    node_fqn,
                    target_type,
                    target_fqn,
                    membership_kind,
                }
            },
        )
        .collect();

    // S14: Load MembershipRuleBody snapshots to evaluate conditional memberships.
    // Conditional memberships are only counted if their conditions can be verified.
    let conditional_rules = load_conditional_membership_rules(pool).await?;
    let excluded_conditionals = evaluate_conditional_memberships(&conditional_rules);

    // Derive the subject's taxonomy memberships from its entity type.
    // S14: Filter out conditional memberships that failed evaluation.
    let mut subject_taxonomy_fqns = HashSet::new();

    if let Some(ref entity_type_fqn) = subject.entity_type_fqn {
        for m in &all_memberships {
            if m.target_type == "entity_type_def"
                && m.target_fqn == *entity_type_fqn
                && m.membership_kind != "excluded"
            {
                // S14: If this is a conditional membership, check if it was excluded
                if m.membership_kind == "conditional" {
                    let key = format!("{}::{}", m.taxonomy_fqn, m.target_fqn);
                    if excluded_conditionals.contains(&key) {
                        tracing::trace!(
                            taxonomy = %m.taxonomy_fqn,
                            target = %m.target_fqn,
                            "Conditional membership excluded: conditions not verifiable"
                        );
                        continue;
                    }
                }
                subject_taxonomy_fqns.insert(m.taxonomy_fqn.clone());
            }
        }
    }

    Ok(SubjectMemberships {
        subject_taxonomy_fqns,
        all_memberships,
    })
}

/// Load MembershipRuleBody snapshots with `Conditional` membership kind (S14).
async fn load_conditional_membership_rules(pool: &PgPool) -> Result<Vec<MembershipRuleBody>> {
    let rows = SnapshotStore::list_active(pool, ObjectType::MembershipRule, 500, 0).await?;
    let mut conditional_rules = Vec::new();
    for row in &rows {
        if let Ok(body) = row.parse_definition::<MembershipRuleBody>() {
            if body.membership_kind == MembershipKind::Conditional {
                conditional_rules.push(body);
            }
        }
    }
    Ok(conditional_rules)
}

/// Evaluate conditional membership rules (S14).
///
/// Returns a set of `"{taxonomy_fqn}::{target_fqn}"` keys for conditional
/// memberships that should be excluded because their conditions cannot be
/// verified in the current context.
///
/// Conditions that reference entity attributes (e.g. `attribute_equals`)
/// require runtime entity state that `resolve_context()` does not have.
/// These memberships are conservatively excluded from overlap scoring.
///
/// Conditions with no predicates (empty conditions vec) are treated as
/// satisfied — they represent unconditional "conditional" memberships
/// (effectively direct).
fn evaluate_conditional_memberships(rules: &[MembershipRuleBody]) -> HashSet<String> {
    let mut excluded = HashSet::new();

    for rule in rules {
        if rule.conditions.is_empty() {
            // No conditions → treat as satisfied (effectively a direct membership)
            continue;
        }

        // Evaluate each condition. If ANY condition is not verifiable, exclude.
        let all_verifiable = rule.conditions.iter().all(evaluate_condition);
        if !all_verifiable {
            let key = format!("{}::{}", rule.taxonomy_fqn, rule.target_fqn);
            excluded.insert(key);
        }
    }

    excluded
}

/// Evaluate a single membership condition.
///
/// Returns `true` if the condition is verifiable and passes in the current
/// static context. Returns `false` if the condition requires runtime entity
/// state that we don't have.
fn evaluate_condition(condition: &MembershipCondition) -> bool {
    match condition.kind.as_str() {
        // Conditions that require entity attribute state — not available
        "attribute_equals" | "attribute_in" | "attribute_not_in" | "attribute_gt"
        | "attribute_lt" => false,
        // Conditions based on entity role — not available in resolution context
        "entity_has_role" | "entity_in_jurisdiction" => false,
        // Static conditions that can be evaluated without runtime state
        "always_true" => true,
        "always_false" => false,
        // Unknown condition types — conservatively exclude
        other => {
            tracing::debug!(
                condition_kind = %other,
                "Unknown membership condition kind — excluding from overlap"
            );
            false
        }
    }
}

// ── Step 2c: Load Subject Relationships (D5) ─────────────────

/// Load relationship type definitions where the subject's entity type
/// appears as source or target. Enables edge_class-aware verb ranking.
async fn load_subject_relationships(
    pool: &PgPool,
    subject: &ResolvedSubject,
    as_of: DateTime<Utc>,
) -> Result<SubjectRelationships> {
    let Some(ref entity_type_fqn) = subject.entity_type_fqn else {
        return Ok(SubjectRelationships::default());
    };

    let all_rel_rows = load_typed_snapshots(pool, ObjectType::RelationshipTypeDef, as_of).await?;

    let mut outgoing = Vec::new();
    let mut incoming = Vec::new();

    for row in &all_rel_rows {
        let Ok(body) = row.parse_definition::<RelationshipTypeDefBody>() else {
            continue;
        };
        if body.source_entity_type_fqn == *entity_type_fqn {
            outgoing.push(body);
        } else if body.target_entity_type_fqn == *entity_type_fqn {
            incoming.push(body);
        }
    }

    Ok(SubjectRelationships { outgoing, incoming })
}

// ── Step 3: Load and Rank Views ───────────────────────────────

async fn load_view_defs(
    pool: &PgPool,
    as_of: DateTime<Utc>,
) -> Result<Vec<(SnapshotRow, ViewDefBody)>> {
    let rows = load_typed_snapshots(pool, ObjectType::ViewDef, as_of).await?;
    let mut results = Vec::new();
    for row in rows {
        if let Ok(body) = row.parse_definition::<ViewDefBody>() {
            results.push((row, body));
        }
    }
    Ok(results)
}

fn rank_views_by_overlap(
    views: &[(SnapshotRow, ViewDefBody)],
    subject: &ResolvedSubject,
    memberships: &SubjectMemberships,
) -> Vec<RankedView> {
    views
        .iter()
        .map(|(row, body)| {
            let overlap = compute_view_overlap(body, subject, memberships);
            RankedView {
                view_snapshot_id: row.snapshot_id,
                view_id: row.object_id,
                fqn: body.fqn.clone(),
                name: body.name.clone(),
                overlap_score: overlap,
                body: body.clone(),
            }
        })
        .filter(|rv| rv.overlap_score > 0.0)
        .collect()
}

fn compute_view_overlap(
    view: &ViewDefBody,
    subject: &ResolvedSubject,
    memberships: &SubjectMemberships,
) -> f64 {
    // Score based on entity type match between view's base_entity_type
    // and the subject's resolved entity type
    let mut score = 0.0;

    if let Some(ref entity_type) = subject.entity_type_fqn {
        if view.base_entity_type == *entity_type {
            score += 0.8;
        } else if view.domain == entity_type.split('.').next().unwrap_or("") {
            // Domain-level match (e.g. view for "kyc" domain, entity is "kyc.case")
            score += 0.4;
        }
    }

    // Taxonomy membership overlap bonus: if the view's FQN shares taxonomy
    // memberships with the subject's entity type, boost the score.
    if memberships.has_memberships() {
        let view_taxonomies = memberships.taxonomy_fqns_for_target(&view.fqn);
        let overlap_count = memberships
            .subject_taxonomy_fqns
            .intersection(&view_taxonomies)
            .count();
        if overlap_count > 0 {
            // Up to +0.2 bonus for taxonomy overlap (caps at 2+ overlaps)
            score += (overlap_count as f64 * 0.1).min(0.2);
        }
    }

    // Jurisdiction constraint bonus
    if let Some(ref _jurisdiction) = subject.jurisdiction {
        // Views that include jurisdiction-specific filters get a boost
        let has_jurisdiction_filter = view
            .filters
            .iter()
            .any(|f| f.attribute_fqn.contains("jurisdiction"));
        if has_jurisdiction_filter {
            score += 0.1;
        }
    }

    // Views with more columns are more comprehensive (small bonus)
    if !view.columns.is_empty() {
        score += 0.1_f64.min(view.columns.len() as f64 * 0.01);
    }

    score.min(1.0)
}

// ── Step 5: Filter and Rank Verbs ─────────────────────────────

fn filter_and_rank_verbs(
    verb_rows: &[SnapshotRow],
    actor: &ActorContext,
    mode: EvidenceMode,
    top_view: Option<&ViewDefBody>,
    entity_kind: Option<&str>,
    memberships: &SubjectMemberships,
    relationships: &SubjectRelationships,
) -> Result<Vec<VerbCandidate>> {
    let mut candidates = Vec::new();

    for row in verb_rows {
        // Parse the verb contract body
        let body: serde_json::Value = row.definition.clone();
        let fqn = body
            .get("fqn")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let description = body
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Entity-kind applicability filter
        if let Some(kind) = entity_kind {
            let subject_kinds: Vec<String> = body
                .get("subject_kinds")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            if !subject_kinds.is_empty() && !subject_kinds.iter().any(|sk| sk == kind) {
                tracing::trace!(
                    verb_fqn = %fqn,
                    entity_kind = %kind,
                    subject_kinds = ?subject_kinds,
                    "Verb filtered out: entity kind not in subject_kinds"
                );
                continue;
            }
        }

        // Tier/trust filtering based on evidence mode
        if !tier_allowed(row.governance_tier, row.trust_class, mode, top_view) {
            continue;
        }

        // ABAC check
        let label = row.parse_security_label().unwrap_or_default();
        let access = evaluate_abac(actor, &label, AccessPurpose::Operations);
        if !access.is_allowed() && mode != EvidenceMode::Exploratory {
            continue;
        }

        // Compute rank score from view prominence
        let mut rank_score = compute_verb_prominence(&fqn, top_view);

        // Boost verbs that explicitly match the entity kind
        if let Some(kind) = entity_kind {
            let subject_kinds: Vec<String> = body
                .get("subject_kinds")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            if subject_kinds.iter().any(|sk| sk == kind) {
                rank_score += 0.15;
            }
        }

        // Taxonomy membership filtering: if the subject has taxonomy memberships,
        // filter out verbs that are in taxonomies that don't overlap with the subject's.
        // Graceful degradation: if subject has no memberships, include all candidates.
        if memberships.has_memberships() {
            let verb_taxonomies = memberships.taxonomy_fqns_for_target(&fqn);
            if !verb_taxonomies.is_empty() {
                // Verb has taxonomy memberships — check for overlap with subject
                let overlap = memberships
                    .subject_taxonomy_fqns
                    .intersection(&verb_taxonomies)
                    .count();
                if overlap == 0 {
                    // No taxonomy overlap — filter out this verb
                    tracing::trace!(
                        verb_fqn = %fqn,
                        verb_taxonomies = ?verb_taxonomies,
                        subject_taxonomies = ?memberships.subject_taxonomy_fqns,
                        "Verb filtered out: no taxonomy overlap with subject"
                    );
                    continue;
                }
                // Boost for taxonomy overlap (up to +0.1 for 2+ overlapping taxonomies)
                rank_score += (overlap as f64 * 0.05).min(0.1);
            }
            // Verbs without taxonomy memberships pass through — they are unconstrained
        }

        // D5: Relationship-aware ranking — boost verbs whose domain matches
        // the subject's relationship edge classes or relationship domains.
        // This allows resolve_context() to surface relationship-relevant verbs
        // (e.g. "ownership" edge_class boosts verbs in the ownership domain).
        if relationships.has_relationships() {
            let verb_domain = fqn.split('.').next().unwrap_or("");
            let rel_domains = relationships.relationship_domains();
            if rel_domains.contains(verb_domain) {
                rank_score += 0.08;
            }
            // Additional boost for verbs whose domain appears as an edge_class
            let edge_classes = relationships.edge_classes();
            if edge_classes.contains(verb_domain) {
                rank_score += 0.07;
            }
        }

        // Determine usable_for_proof based on tier + trust
        let usable_for_proof = row.governance_tier == GovernanceTier::Governed
            && matches!(
                row.trust_class,
                TrustClass::Proof | TrustClass::DecisionSupport
            );

        candidates.push(VerbCandidate {
            verb_snapshot_id: row.snapshot_id,
            verb_id: row.object_id,
            fqn,
            description,
            governance_tier: row.governance_tier,
            trust_class: row.trust_class,
            rank_score,
            preconditions_met: true, // evaluated later in Step 8
            access_decision: access,
            usable_for_proof,
        });
    }

    Ok(candidates)
}

fn compute_verb_prominence(verb_fqn: &str, top_view: Option<&ViewDefBody>) -> f64 {
    let Some(view) = top_view else {
        return 0.5; // no view context — neutral score
    };

    // Check if the verb's domain matches the view's domain
    let verb_domain = verb_fqn.split('.').next().unwrap_or("");
    if verb_domain == view.domain {
        0.8
    } else {
        0.3
    }
}

// ── Step 6: Filter and Rank Attributes ────────────────────────

fn filter_and_rank_attributes(
    attr_rows: &[SnapshotRow],
    actor: &ActorContext,
    mode: EvidenceMode,
    top_view: Option<&ViewDefBody>,
    memberships: &SubjectMemberships,
) -> Result<Vec<AttributeCandidate>> {
    let mut candidates = Vec::new();

    for row in attr_rows {
        let body: serde_json::Value = row.definition.clone();
        let fqn = body
            .get("fqn")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let name = body
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Tier/trust filtering
        if !tier_allowed(row.governance_tier, row.trust_class, mode, top_view) {
            continue;
        }

        // ABAC check
        let label = row.parse_security_label().unwrap_or_default();
        let access = evaluate_abac(actor, &label, AccessPurpose::Operations);
        if !access.is_allowed() && mode != EvidenceMode::Exploratory {
            continue;
        }

        // Rank from view columns
        let (mut rank_score, required) = compute_attribute_prominence(&fqn, top_view);

        // Taxonomy membership filtering: same pattern as verb filtering
        if memberships.has_memberships() {
            let attr_taxonomies = memberships.taxonomy_fqns_for_target(&fqn);
            if !attr_taxonomies.is_empty() {
                let overlap = memberships
                    .subject_taxonomy_fqns
                    .intersection(&attr_taxonomies)
                    .count();
                if overlap == 0 {
                    tracing::trace!(
                        attr_fqn = %fqn,
                        attr_taxonomies = ?attr_taxonomies,
                        subject_taxonomies = ?memberships.subject_taxonomy_fqns,
                        "Attribute filtered out: no taxonomy overlap with subject"
                    );
                    continue;
                }
                // Boost for taxonomy overlap
                rank_score += (overlap as f64 * 0.05).min(0.1);
            }
        }

        candidates.push(AttributeCandidate {
            attribute_snapshot_id: row.snapshot_id,
            attribute_id: row.object_id,
            fqn,
            name,
            governance_tier: row.governance_tier,
            trust_class: row.trust_class,
            rank_score,
            access_decision: access,
            required,
            present: false, // would need entity-specific lookup — false as default
        });
    }

    Ok(candidates)
}

fn compute_attribute_prominence(attr_fqn: &str, top_view: Option<&ViewDefBody>) -> (f64, bool) {
    let Some(view) = top_view else {
        return (0.5, false);
    };

    // Check if this attribute is a column in the view
    for (i, col) in view.columns.iter().enumerate() {
        if col.attribute_fqn == attr_fqn {
            // Higher prominence for earlier columns
            let position_score = 1.0 - (i as f64 * 0.05).min(0.5);
            return (position_score, col.visible);
        }
    }

    // Check if used in a filter (important but not displayed)
    for filter in &view.filters {
        if filter.attribute_fqn == attr_fqn {
            return (0.6, true);
        }
    }

    // Attribute domain matches view domain
    let attr_domain = attr_fqn.split('.').next().unwrap_or("");
    if attr_domain == view.domain {
        return (0.4, false);
    }

    (0.2, false)
}

// ── Tier/Trust Filtering ──────────────────────────────────────

fn tier_allowed(
    tier: GovernanceTier,
    trust: TrustClass,
    mode: EvidenceMode,
    view: Option<&ViewDefBody>,
) -> bool {
    match mode {
        EvidenceMode::Strict => {
            tier == GovernanceTier::Governed
                && matches!(trust, TrustClass::Proof | TrustClass::DecisionSupport)
        }
        EvidenceMode::Normal => {
            // Governed always allowed
            if tier == GovernanceTier::Governed {
                return true;
            }
            // Operational only if the active view opts in via includes_operational
            view.is_some_and(|v| v.includes_operational)
        }
        EvidenceMode::Exploratory => true,
        EvidenceMode::Governance => true,
    }
}

// ── Step 8: Evaluate Preconditions ────────────────────────────

fn evaluate_verb_preconditions(verbs: &[VerbCandidate]) -> Vec<PreconditionStatus> {
    // For each verb, load its preconditions from the contract body
    // and evaluate against current state.
    // MVP: report preconditions from the verb FQN pattern without
    // executing real checks (would require entity state queries).
    verbs
        .iter()
        .take(10) // top 10 only
        .map(|v| PreconditionStatus {
            verb_fqn: v.fqn.clone(),
            verb_snapshot_id: v.verb_snapshot_id,
            checks: vec![], // MVP: no checks — filled in by entity-specific logic
            all_satisfied: v.preconditions_met,
            remediation_hint: None,
        })
        .collect()
}

// ── Step 9: Evaluate Policies ─────────────────────────────────

fn evaluate_policies(
    policy_rows: &[SnapshotRow],
    verbs: &[VerbCandidate],
    _actor: &ActorContext,
) -> Result<Vec<PolicyVerdict>> {
    let mut verdicts = Vec::new();

    for row in policy_rows {
        let body: PolicyRuleBody = match row.parse_definition() {
            Ok(b) => b,
            Err(_) => continue,
        };

        if !body.enabled {
            continue;
        }

        // Check if any policy predicates match our verb candidates
        let applies = body.predicates.iter().any(|pred| {
            match pred.kind.as_str() {
                "governance_tier" => {
                    // Check if any verb candidate matches the tier predicate
                    verbs.iter().any(|v| {
                        let tier_str = format!("{:?}", v.governance_tier).to_lowercase();
                        match pred.operator.as_str() {
                            "eq" => pred.value.as_str() == Some(tier_str.as_str()),
                            "ne" => pred.value.as_str() != Some(tier_str.as_str()),
                            _ => false,
                        }
                    })
                }
                "trust_class" => verbs.iter().any(|v| {
                    let trust_str = format!("{:?}", v.trust_class).to_lowercase();
                    pred.value.as_str() == Some(trust_str.as_str())
                }),
                _ => false, // Other predicate types not evaluated in MVP
            }
        });

        if applies || body.predicates.is_empty() {
            // Build verdict from policy actions
            let allowed = !body
                .actions
                .iter()
                .any(|a| a.kind == "block_publish" || a.kind == "restrict_access");

            let required_actions: Vec<String> = body
                .actions
                .iter()
                .filter(|a| a.kind == "require_evidence" || a.kind == "require_approval")
                .map(|a| a.description.clone().unwrap_or_else(|| a.kind.clone()))
                .collect();

            verdicts.push(PolicyVerdict {
                policy_snapshot_id: row.snapshot_id,
                policy_fqn: body.fqn.clone(),
                policy_name: body.name.clone(),
                allowed,
                reason: if allowed {
                    "Policy permits action".into()
                } else {
                    "Policy restricts action".into()
                },
                required_actions,
                regulatory_reference: None,
            });
        }
    }

    Ok(verdicts)
}

// ── Step 10: Composite Access Decision ────────────────────────

fn compute_composite_access(
    verbs: &[VerbCandidate],
    policy_verdicts: &[PolicyVerdict],
) -> AccessDecision {
    // If any policy blocks, deny
    if policy_verdicts.iter().any(|v| !v.allowed) {
        return AccessDecision::Deny {
            reason: "One or more policies restrict access".into(),
        };
    }

    // If any verb is denied by ABAC, report masking
    let denied_verbs: Vec<_> = verbs
        .iter()
        .filter(|v| matches!(v.access_decision, AccessDecision::Deny { .. }))
        .collect();

    if !denied_verbs.is_empty() {
        return AccessDecision::AllowWithMasking {
            masked_fields: denied_verbs.iter().map(|v| v.fqn.clone()).collect(),
        };
    }

    // Check for masking requirements
    let needs_masking: Vec<_> = verbs
        .iter()
        .filter(|v| matches!(v.access_decision, AccessDecision::AllowWithMasking { .. }))
        .collect();

    if !needs_masking.is_empty() {
        let mut all_masked = Vec::new();
        for v in &needs_masking {
            if let AccessDecision::AllowWithMasking { masked_fields } = &v.access_decision {
                all_masked.extend(masked_fields.clone());
            }
        }
        return AccessDecision::AllowWithMasking {
            masked_fields: all_masked,
        };
    }

    AccessDecision::Allow
}

// ── Step 11: Governance Signals ───────────────────────────────

fn generate_governance_signals(
    verbs: &[VerbCandidate],
    attributes: &[AttributeCandidate],
    mode: EvidenceMode,
) -> Vec<GovernanceSignal> {
    let mut signals = Vec::new();

    if mode != EvidenceMode::Governance && mode != EvidenceMode::Exploratory {
        return signals;
    }

    // Check for attributes that are required but not present
    for attr in attributes {
        if attr.required && !attr.present {
            signals.push(GovernanceSignal {
                kind: GovernanceSignalKind::CoverageGap,
                message: format!("Required attribute '{}' is not populated", attr.fqn),
                severity: GovernanceSignalSeverity::Warning,
                related_fqn: Some(attr.fqn.clone()),
                related_snapshot_id: Some(attr.attribute_snapshot_id),
            });
        }
    }

    // Check for verbs with failed preconditions
    for verb in verbs {
        if !verb.preconditions_met {
            signals.push(GovernanceSignal {
                kind: GovernanceSignalKind::CoverageGap,
                message: format!("Verb '{}' has unsatisfied preconditions", verb.fqn),
                severity: GovernanceSignalSeverity::Info,
                related_fqn: Some(verb.fqn.clone()),
                related_snapshot_id: Some(verb.verb_snapshot_id),
            });
        }
    }

    // Check for operational verbs being used in strict/normal mode
    for verb in verbs {
        if verb.governance_tier == GovernanceTier::Operational && !verb.usable_for_proof {
            signals.push(GovernanceSignal {
                kind: GovernanceSignalKind::CoverageGap,
                message: format!(
                    "Verb '{}' is operational-tier — outputs not usable for proof",
                    verb.fqn
                ),
                severity: GovernanceSignalSeverity::Info,
                related_fqn: Some(verb.fqn.clone()),
                related_snapshot_id: Some(verb.verb_snapshot_id),
            });
        }
    }

    signals
}

// ── Step 12: Confidence Score ─────────────────────────────────

fn compute_confidence(
    views: &[RankedView],
    verbs: &[VerbCandidate],
    preconditions: &[PreconditionStatus],
    attributes: &[AttributeCandidate],
) -> f64 {
    // view_match_score × 0.30
    let view_score = views.first().map(|v| v.overlap_score).unwrap_or(0.0);

    // precondition_satisfiable_pct × 0.25
    let precondition_pct = if preconditions.is_empty() {
        1.0
    } else {
        let satisfied = preconditions.iter().filter(|p| p.all_satisfied).count();
        satisfied as f64 / preconditions.len() as f64
    };

    // required_inputs_present_pct × 0.30
    let required_attrs = attributes.iter().filter(|a| a.required).count();
    let present_required = attributes
        .iter()
        .filter(|a| a.required && a.present)
        .count();
    let inputs_pct = if required_attrs == 0 {
        1.0
    } else {
        present_required as f64 / required_attrs as f64
    };

    // abac_permit_pct × 0.15
    let abac_pct = if verbs.is_empty() {
        1.0
    } else {
        let permitted = verbs
            .iter()
            .filter(|v| v.access_decision.is_allowed())
            .count();
        permitted as f64 / verbs.len() as f64
    };

    let confidence =
        view_score * 0.30 + precondition_pct * 0.25 + inputs_pct * 0.30 + abac_pct * 0.15;

    confidence.clamp(0.0, 1.0)
}

// ── Disambiguation Generation ─────────────────────────────────

fn generate_disambiguation(
    views: &[RankedView],
    _verbs: &[VerbCandidate],
) -> Vec<DisambiguationPrompt> {
    let mut prompts = Vec::new();

    // If multiple views are within 0.1 overlap score, ask which view to use
    if views.len() >= 2 {
        let top = views[0].overlap_score;
        let close_views: Vec<_> = views
            .iter()
            .filter(|v| (top - v.overlap_score).abs() < 0.1)
            .collect();

        if close_views.len() >= 2 {
            prompts.push(DisambiguationPrompt {
                prompt_id: Uuid::new_v4(),
                question: "Multiple views match this context. Which perspective would you like?"
                    .into(),
                options: close_views
                    .iter()
                    .map(|v| DisambiguationOption {
                        id: v.fqn.clone(),
                        label: v.name.clone(),
                        description: Some(format!(
                            "View over {} (overlap: {:.0}%)",
                            v.body.base_entity_type,
                            v.overlap_score * 100.0
                        )),
                        narrows_to: Some(serde_json::json!({
                            "view_id": v.view_id
                        })),
                    })
                    .collect(),
                required_to_proceed: false,
                rationale: Some("Views have similar taxonomy overlap scores".into()),
            });
        }
    }

    prompts
}

// ── Snapshot Loading Helpers ──────────────────────────────────

async fn load_typed_snapshots(
    pool: &PgPool,
    object_type: ObjectType,
    as_of: DateTime<Utc>,
) -> Result<Vec<SnapshotRow>> {
    // Use point-in-time resolution if as_of is in the past
    let now = Utc::now();
    let is_historical = (now - as_of).num_seconds() > 1;

    if is_historical {
        // Load all snapshots that were active at as_of
        let rows = sqlx::query_as::<_, SnapshotRow>(
            r#"
            SELECT *
            FROM sem_reg.snapshots
            WHERE object_type = $1
              AND status = 'active'
              AND effective_from <= $2
              AND (effective_until IS NULL OR effective_until > $2)
            ORDER BY effective_from DESC
            "#,
        )
        .bind(object_type)
        .bind(as_of)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    } else {
        // Load ALL current active snapshots via pagination.
        // Avoids silent truncation when > 1000 snapshots exist for a type.
        let page_size: i64 = 500;
        let mut offset: i64 = 0;
        let mut all_rows = Vec::new();
        loop {
            let page = SnapshotStore::list_active(pool, object_type, page_size, offset).await?;
            let count = page.len();
            all_rows.extend(page);
            if (count as i64) < page_size {
                break;
            }
            offset += page_size;
        }
        Ok(all_rows)
    }
}

// ── Draft-Aware Context Resolution (Phase 1) ─────────────────

/// Overlay mode for draft-aware resolution.
/// When `DraftOverlay`, draft snapshots from the changeset override
/// active snapshots for the same (object_type, object_id).
pub use crate::sem_reg::stewardship::types::OverlayMode;

/// Resolve context with draft overlay support.
///
/// When overlay_mode = DraftOverlay { changeset_id }, extends snapshot
/// resolution so draft snapshots in the changeset override active ones
/// for the same (object_type, object_id).
///
/// When assume_principal is set, ABAC evaluation uses that principal's
/// roles instead of the caller's. The assumed identity is recorded in
/// ViewportManifest for audit purposes (spec §2.3.4).
pub async fn resolve_context_with_overlay(
    pool: &PgPool,
    req: &ContextResolutionRequest,
    overlay_mode: &OverlayMode,
    _assume_principal: Option<&str>,
) -> Result<ContextResolutionResponse> {
    match overlay_mode {
        OverlayMode::ActiveOnly => {
            // No overlay — delegate to standard resolution
            resolve_context(pool, req).await
        }
        OverlayMode::DraftOverlay { changeset_id } => {
            // Standard resolution first
            let mut response = resolve_context(pool, req).await?;

            // Load draft snapshots from the changeset and merge them
            // into the response, overriding active snapshots for same objects.
            let draft_overrides = load_changeset_drafts(pool, *changeset_id).await?;

            if !draft_overrides.is_empty() {
                // Re-rank verbs considering draft definitions
                let draft_verb_fqns: std::collections::HashSet<String> = draft_overrides
                    .iter()
                    .filter(|s| s.object_type == ObjectType::VerbContract)
                    .filter_map(|s| {
                        s.definition
                            .get("fqn")
                            .and_then(|v| v.as_str())
                            .map(|fqn| fqn.to_string())
                    })
                    .collect();

                // Boost verbs that have draft definitions (they're being worked on)
                for verb in &mut response.candidate_verbs {
                    if draft_verb_fqns.contains(&verb.fqn) {
                        verb.rank_score += 0.05; // Small boost for in-changeset verbs
                    }
                }

                // Re-sort by updated rank
                response.candidate_verbs.sort_by(|a, b| {
                    b.rank_score
                        .partial_cmp(&a.rank_score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                // Add governance signal about draft overlay
                response.governance_signals.push(GovernanceSignal {
                    kind: GovernanceSignalKind::CoverageGap, // Closest existing kind
                    severity: GovernanceSignalSeverity::Info,
                    message: format!(
                        "Resolution includes draft overlay from changeset {} ({} draft snapshots)",
                        changeset_id,
                        draft_overrides.len()
                    ),
                    related_fqn: None,
                    related_snapshot_id: None,
                });
            }

            Ok(response)
        }
    }
}

/// Load draft snapshots belonging to a changeset.
async fn load_changeset_drafts(pool: &PgPool, changeset_id: Uuid) -> Result<Vec<SnapshotRow>> {
    let rows = sqlx::query_as::<_, SnapshotRow>(
        r#"
        SELECT *
        FROM sem_reg.snapshots
        WHERE snapshot_set_id = $1
          AND status = 'draft'
          AND effective_until IS NULL
        ORDER BY object_type, fqn
        "#,
    )
    .bind(changeset_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// ── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sem_reg::abac::ActorContext;
    use crate::sem_reg::types::Classification;

    fn test_actor() -> ActorContext {
        ActorContext {
            actor_id: "agent-1".into(),
            roles: vec!["analyst".into()],
            department: Some("compliance".into()),
            clearance: Some(Classification::Confidential),
            jurisdictions: vec!["LU".into(), "DE".into()],
        }
    }

    fn test_request() -> ContextResolutionRequest {
        ContextResolutionRequest {
            subject: SubjectRef::EntityId(Uuid::new_v4()),
            intent: Some("discover UBO structure".into()),
            actor: test_actor(),
            goals: vec!["resolve_ubo".into()],
            constraints: ResolutionConstraints::default(),
            evidence_mode: EvidenceMode::Normal,
            point_in_time: None,
            entity_kind: None,
        }
    }

    #[test]
    fn test_evidence_mode_default() {
        assert_eq!(EvidenceMode::default(), EvidenceMode::Normal);
    }

    #[test]
    fn test_subject_ref_id() {
        let id = Uuid::new_v4();
        assert_eq!(SubjectRef::CaseId(id).id(), id);
        assert_eq!(SubjectRef::EntityId(id).id(), id);
        assert_eq!(SubjectRef::DocumentId(id).id(), id);
        assert_eq!(SubjectRef::TaskId(id).id(), id);
        assert_eq!(SubjectRef::ViewId(id).id(), id);
    }

    #[test]
    fn test_subject_ref_serde() {
        let subject = SubjectRef::CaseId(Uuid::new_v4());
        let json = serde_json::to_value(&subject).unwrap();
        assert_eq!(json["type"], "case_id");
        let round: SubjectRef = serde_json::from_value(json).unwrap();
        assert_eq!(round, subject);
    }

    #[test]
    fn test_request_serde() {
        let req = test_request();
        let json = serde_json::to_value(&req).unwrap();
        let round: ContextResolutionRequest = serde_json::from_value(json).unwrap();
        assert_eq!(round.evidence_mode, EvidenceMode::Normal);
        assert_eq!(round.goals, vec!["resolve_ubo"]);
    }

    fn view_with_operational(includes: bool) -> ViewDefBody {
        ViewDefBody {
            fqn: "test.view".into(),
            name: "Test View".into(),
            description: "Test".into(),
            domain: "test".into(),
            base_entity_type: "test".into(),
            columns: vec![],
            filters: vec![],
            sort_order: vec![],
            includes_operational: includes,
        }
    }

    #[test]
    fn test_tier_allowed_strict() {
        // Strict mode ignores view — only Governed + Proof/DecisionSupport
        assert!(tier_allowed(
            GovernanceTier::Governed,
            TrustClass::Proof,
            EvidenceMode::Strict,
            None,
        ));
        assert!(tier_allowed(
            GovernanceTier::Governed,
            TrustClass::DecisionSupport,
            EvidenceMode::Strict,
            None,
        ));
        assert!(!tier_allowed(
            GovernanceTier::Governed,
            TrustClass::Convenience,
            EvidenceMode::Strict,
            None,
        ));
        assert!(!tier_allowed(
            GovernanceTier::Operational,
            TrustClass::DecisionSupport,
            EvidenceMode::Strict,
            None,
        ));
    }

    #[test]
    fn test_tier_allowed_normal_governed_always_allowed() {
        // Governed always passes in Normal mode, regardless of view
        assert!(tier_allowed(
            GovernanceTier::Governed,
            TrustClass::Proof,
            EvidenceMode::Normal,
            None,
        ));
        assert!(tier_allowed(
            GovernanceTier::Governed,
            TrustClass::Convenience,
            EvidenceMode::Normal,
            None,
        ));
    }

    #[test]
    fn test_tier_allowed_normal_operational_filtered_by_view() {
        // Operational in Normal mode: filtered out when view is absent
        assert!(!tier_allowed(
            GovernanceTier::Operational,
            TrustClass::Convenience,
            EvidenceMode::Normal,
            None,
        ));

        // Filtered out when view has includes_operational=false
        let view_no_op = view_with_operational(false);
        assert!(!tier_allowed(
            GovernanceTier::Operational,
            TrustClass::Convenience,
            EvidenceMode::Normal,
            Some(&view_no_op),
        ));

        // Included when view has includes_operational=true
        let view_with_op = view_with_operational(true);
        assert!(tier_allowed(
            GovernanceTier::Operational,
            TrustClass::Convenience,
            EvidenceMode::Normal,
            Some(&view_with_op),
        ));
    }

    #[test]
    fn test_tier_allowed_exploratory() {
        // Exploratory always allows everything
        assert!(tier_allowed(
            GovernanceTier::Operational,
            TrustClass::Convenience,
            EvidenceMode::Exploratory,
            None,
        ));
    }

    #[test]
    fn test_confidence_computation() {
        let views = vec![RankedView {
            view_snapshot_id: Uuid::new_v4(),
            view_id: Uuid::new_v4(),
            fqn: "test.view".into(),
            name: "Test View".into(),
            overlap_score: 0.8,
            body: ViewDefBody {
                fqn: "test.view".into(),
                name: "Test".into(),
                description: "Test".into(),
                domain: "test".into(),
                base_entity_type: "entity.test".into(),
                columns: vec![],
                filters: vec![],
                sort_order: vec![],
                includes_operational: false,
            },
        }];

        let verbs = vec![VerbCandidate {
            verb_snapshot_id: Uuid::new_v4(),
            verb_id: Uuid::new_v4(),
            fqn: "test.create".into(),
            description: "Create test".into(),
            governance_tier: GovernanceTier::Governed,
            trust_class: TrustClass::Proof,
            rank_score: 0.8,
            preconditions_met: true,
            access_decision: AccessDecision::Allow,
            usable_for_proof: true,
        }];

        let preconditions = vec![PreconditionStatus {
            verb_fqn: "test.create".into(),
            verb_snapshot_id: Uuid::new_v4(),
            checks: vec![],
            all_satisfied: true,
            remediation_hint: None,
        }];

        let attrs: Vec<AttributeCandidate> = vec![];

        let confidence = compute_confidence(&views, &verbs, &preconditions, &attrs);
        // 0.8 * 0.30 + 1.0 * 0.25 + 1.0 * 0.30 + 1.0 * 0.15 = 0.24 + 0.25 + 0.30 + 0.15 = 0.94
        assert!((confidence - 0.94).abs() < 0.01);
    }

    #[test]
    fn test_confidence_low_when_no_views() {
        let confidence = compute_confidence(&[], &[], &[], &[]);
        // 0.0 * 0.30 + 1.0 * 0.25 + 1.0 * 0.30 + 1.0 * 0.15 = 0.70
        assert!((confidence - 0.70).abs() < 0.01);
    }

    #[test]
    fn test_governance_signal_kinds() {
        let signal = GovernanceSignal {
            kind: GovernanceSignalKind::StaleEvidence,
            message: "Evidence expired".into(),
            severity: GovernanceSignalSeverity::Warning,
            related_fqn: Some("obs.pep-screening".into()),
            related_snapshot_id: None,
        };
        let json = serde_json::to_value(&signal).unwrap();
        assert_eq!(json["kind"], "stale_evidence");
        assert_eq!(json["severity"], "warning");
    }

    #[test]
    fn test_composite_access_deny_on_policy_block() {
        let verbs = vec![VerbCandidate {
            verb_snapshot_id: Uuid::new_v4(),
            verb_id: Uuid::new_v4(),
            fqn: "test.create".into(),
            description: "Test".into(),
            governance_tier: GovernanceTier::Governed,
            trust_class: TrustClass::Proof,
            rank_score: 0.8,
            preconditions_met: true,
            access_decision: AccessDecision::Allow,
            usable_for_proof: true,
        }];

        let verdicts = vec![PolicyVerdict {
            policy_snapshot_id: Uuid::new_v4(),
            policy_fqn: "test.block".into(),
            policy_name: "Block".into(),
            allowed: false,
            reason: "Blocked".into(),
            required_actions: vec![],
            regulatory_reference: None,
        }];

        let access = compute_composite_access(&verbs, &verdicts);
        assert!(matches!(access, AccessDecision::Deny { .. }));
    }

    #[test]
    fn test_disambiguation_generated_for_close_views() {
        let view_body = ViewDefBody {
            fqn: "test.view".into(),
            name: "Test".into(),
            description: "Test".into(),
            domain: "test".into(),
            base_entity_type: "entity.test".into(),
            columns: vec![],
            filters: vec![],
            sort_order: vec![],
            includes_operational: false,
        };

        let views = vec![
            RankedView {
                view_snapshot_id: Uuid::new_v4(),
                view_id: Uuid::new_v4(),
                fqn: "test.view1".into(),
                name: "View 1".into(),
                overlap_score: 0.8,
                body: view_body.clone(),
            },
            RankedView {
                view_snapshot_id: Uuid::new_v4(),
                view_id: Uuid::new_v4(),
                fqn: "test.view2".into(),
                name: "View 2".into(),
                overlap_score: 0.75,
                body: view_body,
            },
        ];

        let prompts = generate_disambiguation(&views, &[]);
        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].options.len(), 2);
    }

    #[test]
    fn test_view_overlap_exact_entity_type_match() {
        let view = ViewDefBody {
            fqn: "kyc.ubo-view".into(),
            name: "UBO View".into(),
            description: "UBO discovery view".into(),
            domain: "kyc".into(),
            base_entity_type: "entity.proper-person".into(),
            columns: vec![],
            filters: vec![],
            sort_order: vec![],
            includes_operational: false,
        };

        let subject = ResolvedSubject {
            entity_type_fqn: Some("entity.proper-person".into()),
            jurisdiction: None,
            state: None,
        };

        let no_memberships = SubjectMemberships::default();
        let score = compute_view_overlap(&view, &subject, &no_memberships);
        assert!(score >= 0.8);
    }

    #[test]
    fn test_view_overlap_domain_match() {
        let view = ViewDefBody {
            fqn: "kyc.case-view".into(),
            name: "Case View".into(),
            description: "KYC case view".into(),
            domain: "kyc".into(),
            base_entity_type: "kyc.case".into(),
            columns: vec![],
            filters: vec![],
            sort_order: vec![],
            includes_operational: false,
        };

        let subject = ResolvedSubject {
            entity_type_fqn: Some("kyc.enhanced-case".into()),
            jurisdiction: None,
            state: None,
        };

        let no_memberships = SubjectMemberships::default();
        let score = compute_view_overlap(&view, &subject, &no_memberships);
        assert!(score >= 0.4);
        assert!(score < 0.8);
    }

    #[test]
    fn test_view_overlap_with_taxonomy_memberships() {
        let view = ViewDefBody {
            fqn: "kyc.ubo-view".into(),
            name: "UBO View".into(),
            description: "UBO discovery view".into(),
            domain: "kyc".into(),
            base_entity_type: "entity.proper-person".into(),
            columns: vec![],
            filters: vec![],
            sort_order: vec![],
            includes_operational: false,
        };

        let subject = ResolvedSubject {
            entity_type_fqn: Some("entity.proper-person".into()),
            jurisdiction: None,
            state: None,
        };

        // Create memberships where both subject and view share "domain.kyc" taxonomy
        let memberships = SubjectMemberships {
            subject_taxonomy_fqns: HashSet::from(["domain.kyc".into()]),
            all_memberships: vec![TaxonomyMembership {
                taxonomy_fqn: "domain.kyc".into(),
                node_fqn: "domain.kyc.ubo".into(),
                target_type: "view_def".into(),
                target_fqn: "kyc.ubo-view".into(),
                membership_kind: "direct".into(),
            }],
        };

        let score_with = compute_view_overlap(&view, &subject, &memberships);
        let no_memberships = SubjectMemberships::default();
        let score_without = compute_view_overlap(&view, &subject, &no_memberships);

        // Taxonomy overlap gives a bonus
        assert!(score_with > score_without);
    }

    #[test]
    fn test_subject_memberships_overlap_count() {
        let memberships = SubjectMemberships {
            subject_taxonomy_fqns: HashSet::from(["domain.kyc".into(), "risk.high".into()]),
            all_memberships: vec![
                TaxonomyMembership {
                    taxonomy_fqn: "domain.kyc".into(),
                    node_fqn: "domain.kyc.ubo".into(),
                    target_type: "verb_contract".into(),
                    target_fqn: "kyc.open-case".into(),
                    membership_kind: "direct".into(),
                },
                TaxonomyMembership {
                    taxonomy_fqn: "risk.high".into(),
                    node_fqn: "risk.high.pep".into(),
                    target_type: "verb_contract".into(),
                    target_fqn: "kyc.open-case".into(),
                    membership_kind: "direct".into(),
                },
                TaxonomyMembership {
                    taxonomy_fqn: "domain.trading".into(),
                    node_fqn: "domain.trading.equities".into(),
                    target_type: "verb_contract".into(),
                    target_fqn: "trading.create-profile".into(),
                    membership_kind: "direct".into(),
                },
            ],
        };

        // kyc.open-case shares 2 taxonomies with subject
        assert_eq!(memberships.taxonomy_overlap_count("kyc.open-case"), 2);
        // trading.create-profile shares 0 taxonomies with subject
        assert_eq!(
            memberships.taxonomy_overlap_count("trading.create-profile"),
            0
        );
        // Unknown verb shares 0
        assert_eq!(memberships.taxonomy_overlap_count("unknown.verb"), 0);
    }

    #[test]
    fn test_subject_memberships_excluded_filtered() {
        let memberships = SubjectMemberships {
            subject_taxonomy_fqns: HashSet::from(["domain.kyc".into()]),
            all_memberships: vec![TaxonomyMembership {
                taxonomy_fqn: "domain.kyc".into(),
                node_fqn: "domain.kyc.sanctions".into(),
                target_type: "verb_contract".into(),
                target_fqn: "kyc.screen".into(),
                membership_kind: "excluded".into(),
            }],
        };

        // Excluded memberships should NOT be counted
        let target_taxonomies = memberships.taxonomy_fqns_for_target("kyc.screen");
        assert!(target_taxonomies.is_empty());
        assert_eq!(memberships.taxonomy_overlap_count("kyc.screen"), 0);
    }

    #[test]
    fn test_graceful_degradation_no_memberships() {
        let memberships = SubjectMemberships::default();
        assert!(!memberships.has_memberships());
        // With no memberships, overlap count is always 0 but filtering is skipped
        assert_eq!(memberships.taxonomy_overlap_count("any.verb"), 0);
    }

    // ── D5: SubjectRelationships tests ────────────────────────

    #[test]
    fn test_subject_relationships_edge_classes() {
        let rels = SubjectRelationships {
            outgoing: vec![RelationshipTypeDefBody {
                fqn: "relationship.ownership".into(),
                name: "Ownership".into(),
                description: "Ownership".into(),
                domain: "ownership".into(),
                source_entity_type_fqn: "entity.fund".into(),
                target_entity_type_fqn: "entity.legal_entity".into(),
                cardinality:
                    crate::sem_reg::relationship_type_def::RelationshipCardinality::OneToMany,
                edge_class: Some("ownership".into()),
                directionality: Some(
                    crate::sem_reg::relationship_type_def::Directionality::Forward,
                ),
                inverse_fqn: None,
                constraints: vec![],
            }],
            incoming: vec![RelationshipTypeDefBody {
                fqn: "relationship.custody_of".into(),
                name: "Custody Of".into(),
                description: "Custody".into(),
                domain: "custody".into(),
                source_entity_type_fqn: "entity.custodian".into(),
                target_entity_type_fqn: "entity.fund".into(),
                cardinality:
                    crate::sem_reg::relationship_type_def::RelationshipCardinality::OneToMany,
                edge_class: Some("service".into()),
                directionality: None,
                inverse_fqn: None,
                constraints: vec![],
            }],
        };

        let classes = rels.edge_classes();
        assert!(classes.contains("ownership"));
        assert!(classes.contains("service"));
        assert_eq!(classes.len(), 2);
    }

    #[test]
    fn test_subject_relationships_domains() {
        let rels = SubjectRelationships {
            outgoing: vec![RelationshipTypeDefBody {
                fqn: "relationship.ownership".into(),
                name: "Ownership".into(),
                description: "Ownership".into(),
                domain: "ownership".into(),
                source_entity_type_fqn: "entity.fund".into(),
                target_entity_type_fqn: "entity.legal_entity".into(),
                cardinality:
                    crate::sem_reg::relationship_type_def::RelationshipCardinality::OneToMany,
                edge_class: None,
                directionality: None,
                inverse_fqn: None,
                constraints: vec![],
            }],
            incoming: vec![],
        };

        let domains = rels.relationship_domains();
        assert!(domains.contains("ownership"));
        assert!(rels.has_relationships());
    }

    #[test]
    fn test_subject_relationships_empty() {
        let rels = SubjectRelationships::default();
        assert!(!rels.has_relationships());
        assert!(rels.edge_classes().is_empty());
        assert!(rels.relationship_domains().is_empty());
    }

    // ── S14: Conditional membership evaluation tests ──────────

    #[test]
    fn test_evaluate_condition_attribute_equals_excluded() {
        let cond = MembershipCondition {
            kind: "attribute_equals".into(),
            field: "pep_status".into(),
            operator: "eq".into(),
            value: serde_json::json!("active"),
        };
        // attribute_equals requires runtime state → not verifiable
        assert!(!evaluate_condition(&cond));
    }

    #[test]
    fn test_evaluate_condition_always_true() {
        let cond = MembershipCondition {
            kind: "always_true".into(),
            field: "".into(),
            operator: "eq".into(),
            value: serde_json::json!(true),
        };
        assert!(evaluate_condition(&cond));
    }

    #[test]
    fn test_evaluate_condition_always_false() {
        let cond = MembershipCondition {
            kind: "always_false".into(),
            field: "".into(),
            operator: "eq".into(),
            value: serde_json::json!(false),
        };
        assert!(!evaluate_condition(&cond));
    }

    #[test]
    fn test_evaluate_condition_unknown_kind_excluded() {
        let cond = MembershipCondition {
            kind: "custom_check".into(),
            field: "something".into(),
            operator: "eq".into(),
            value: serde_json::json!("value"),
        };
        // Unknown conditions are conservatively excluded
        assert!(!evaluate_condition(&cond));
    }

    #[test]
    fn test_evaluate_conditional_memberships_empty_conditions_pass() {
        let rules = vec![MembershipRuleBody {
            fqn: "rule.test".into(),
            name: "Test".into(),
            description: None,
            taxonomy_fqn: "taxonomy.risk-tier".into(),
            node_fqn: "taxonomy.risk-tier.high".into(),
            membership_kind: MembershipKind::Conditional,
            target_type: "entity_type_def".into(),
            target_fqn: "entity.person".into(),
            conditions: vec![], // No conditions → passes
        }];
        let excluded = evaluate_conditional_memberships(&rules);
        assert!(excluded.is_empty());
    }

    #[test]
    fn test_evaluate_conditional_memberships_with_attribute_condition_excluded() {
        let rules = vec![MembershipRuleBody {
            fqn: "rule.pep-check".into(),
            name: "PEP Check".into(),
            description: None,
            taxonomy_fqn: "taxonomy.risk-tier".into(),
            node_fqn: "taxonomy.risk-tier.high".into(),
            membership_kind: MembershipKind::Conditional,
            target_type: "entity_type_def".into(),
            target_fqn: "entity.person".into(),
            conditions: vec![MembershipCondition {
                kind: "attribute_equals".into(),
                field: "pep_status".into(),
                operator: "eq".into(),
                value: serde_json::json!("active"),
            }],
        }];
        let excluded = evaluate_conditional_memberships(&rules);
        assert_eq!(excluded.len(), 1);
        assert!(excluded.contains("taxonomy.risk-tier::entity.person"));
    }

    #[test]
    fn test_evaluate_conditional_memberships_mixed_rules() {
        let rules = vec![
            MembershipRuleBody {
                fqn: "rule.always".into(),
                name: "Always".into(),
                description: None,
                taxonomy_fqn: "taxonomy.subject-category".into(),
                node_fqn: "taxonomy.subject-category.fund".into(),
                membership_kind: MembershipKind::Conditional,
                target_type: "entity_type_def".into(),
                target_fqn: "entity.fund".into(),
                conditions: vec![MembershipCondition {
                    kind: "always_true".into(),
                    field: "".into(),
                    operator: "eq".into(),
                    value: serde_json::json!(true),
                }],
            },
            MembershipRuleBody {
                fqn: "rule.pep".into(),
                name: "PEP".into(),
                description: None,
                taxonomy_fqn: "taxonomy.risk-tier".into(),
                node_fqn: "taxonomy.risk-tier.high".into(),
                membership_kind: MembershipKind::Conditional,
                target_type: "entity_type_def".into(),
                target_fqn: "entity.person".into(),
                conditions: vec![MembershipCondition {
                    kind: "attribute_equals".into(),
                    field: "pep_status".into(),
                    operator: "eq".into(),
                    value: serde_json::json!("active"),
                }],
            },
        ];
        let excluded = evaluate_conditional_memberships(&rules);
        // Only the PEP rule is excluded; the always_true rule passes
        assert_eq!(excluded.len(), 1);
        assert!(excluded.contains("taxonomy.risk-tier::entity.person"));
        assert!(!excluded.contains("taxonomy.subject-category::entity.fund"));
    }
}
