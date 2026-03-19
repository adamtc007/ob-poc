//! Sem OS discovery-stage hit rate harness.
//!
//! Measures utterance -> domain/family/constellation/readiness quality against
//! the Sem OS `resolve_context()` discovery path using active snapshots built
//! from the real authored seed bundle for this repo.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use dsl_core::config::loader::ConfigLoader;
use sem_os_core::abac::ActorContext;
use sem_os_core::context_resolution::{
    ContextResolutionRequest, DiscoveryContext, DiscoverySurface, EvidenceMode,
    ResolutionConstraints, ResolutionStage, SubjectRef,
};
use sem_os_core::error::SemOsError;
use sem_os_core::ids::object_id_for;
use sem_os_core::ports::{
    AuditStore, BootstrapAuditStore, ChangesetStore, EvidenceInstanceStore, ObjectStore,
    OutboxStore, ProjectionWriter, SnapshotStore,
};
use sem_os_core::principal::Principal;
use sem_os_core::service::{CoreService, CoreServiceImpl};
use sem_os_core::types::{
    AuditEntry, ChangeType, Changeset, ChangesetEntry, ChangesetReview, ChangesetStatus,
    CreateChangesetInput, DependentSnapshot, EventId, EvidenceInstance, Fqn, GovernanceTier,
    Manifest, OutboxEvent, PublishInput, SecurityLabel, SnapshotExport, SnapshotId, SnapshotMeta,
    SnapshotRow, SnapshotSetId, SnapshotStatus, SnapshotSummary, TrustClass, TypedObject,
};
use sem_os_obpoc_adapter::build_seed_bundle_with_metadata;
use sem_os_obpoc_adapter::metadata::DomainMetadata;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct DiscoveryFixture {
    #[serde(rename = "test")]
    tests: Vec<DiscoveryCase>,
}

#[derive(Debug, Deserialize, Clone)]
struct DiscoveryCase {
    name: String,
    utterance: String,
    #[serde(default)]
    intent_summary: Option<String>,
    #[serde(default)]
    jurisdiction_hint: Option<String>,
    #[serde(default)]
    entity_kind_hint: Option<String>,
    #[serde(default)]
    selected_domain: Option<String>,
    #[serde(default)]
    selected_family: Option<String>,
    #[serde(default)]
    known_inputs: HashMap<String, String>,
    #[serde(default)]
    expected_domain: Option<String>,
    #[serde(default)]
    expected_family: Option<String>,
    #[serde(default)]
    expected_constellation: Option<String>,
    #[serde(default)]
    expected_readiness: Option<String>,
    #[serde(default)]
    notes: Option<String>,
}

#[derive(Debug, Clone)]
struct DiscoveryResult {
    case: DiscoveryCase,
    top_domain: Option<String>,
    top_family: Option<String>,
    top_constellation: Option<String>,
    top3_constellations: Vec<String>,
    readiness: String,
}

#[derive(Clone)]
struct StaticSnapshotStore {
    active: Vec<SnapshotRow>,
}

#[async_trait]
impl SnapshotStore for StaticSnapshotStore {
    async fn resolve(
        &self,
        _fqn: &Fqn,
        _as_of: Option<&SnapshotSetId>,
    ) -> Result<SnapshotRow, SemOsError> {
        unimplemented!("resolve is not used by semos_discovery_hit_rate")
    }

    async fn publish(
        &self,
        _principal: &Principal,
        _req: PublishInput,
    ) -> Result<SnapshotSetId, SemOsError> {
        unimplemented!("publish is not used by semos_discovery_hit_rate")
    }

    async fn list_as_of(&self, _as_of: &SnapshotSetId) -> Result<Vec<SnapshotSummary>, SemOsError> {
        unimplemented!("list_as_of is not used by semos_discovery_hit_rate")
    }

    async fn get_manifest(&self, _id: &SnapshotSetId) -> Result<Manifest, SemOsError> {
        unimplemented!("get_manifest is not used by semos_discovery_hit_rate")
    }

    async fn export(&self, _id: &SnapshotSetId) -> Result<Vec<SnapshotExport>, SemOsError> {
        unimplemented!("export is not used by semos_discovery_hit_rate")
    }

    async fn publish_into_set(
        &self,
        _meta: &SnapshotMeta,
        _definition: &serde_json::Value,
        _snapshot_set_id: uuid::Uuid,
        _correlation_id: uuid::Uuid,
    ) -> Result<uuid::Uuid, SemOsError> {
        unimplemented!("publish_into_set is not used by semos_discovery_hit_rate")
    }

    async fn publish_batch_into_set(
        &self,
        _items: Vec<(SnapshotMeta, serde_json::Value)>,
        _snapshot_set_id: uuid::Uuid,
        _correlation_id: uuid::Uuid,
    ) -> Result<Vec<uuid::Uuid>, SemOsError> {
        unimplemented!("publish_batch_into_set is not used by semos_discovery_hit_rate")
    }

    async fn find_dependents(
        &self,
        _fqn: &str,
        _limit: i64,
    ) -> Result<Vec<DependentSnapshot>, SemOsError> {
        Ok(Vec::new())
    }

    async fn load_active_snapshots(&self) -> Result<Vec<SnapshotRow>, SemOsError> {
        Ok(self.active.clone())
    }
}

struct NoopObjectStore;
struct NoopChangesetStore;
struct NoopAuditStore;
struct NoopOutboxStore;
struct NoopEvidenceStore;
struct NoopProjectionWriter;
struct NoopBootstrapAuditStore;

#[async_trait]
impl ObjectStore for NoopObjectStore {
    async fn load_typed(
        &self,
        _snapshot_id: &SnapshotId,
        _fqn: &Fqn,
    ) -> Result<TypedObject, SemOsError> {
        unimplemented!("load_typed is not used by semos_discovery_hit_rate")
    }
}

#[async_trait]
impl ChangesetStore for NoopChangesetStore {
    async fn create_changeset(
        &self,
        _input: CreateChangesetInput,
    ) -> Result<Changeset, SemOsError> {
        unimplemented!("create_changeset is not used by semos_discovery_hit_rate")
    }

    async fn get_changeset(&self, _changeset_id: uuid::Uuid) -> Result<Changeset, SemOsError> {
        unimplemented!("get_changeset is not used by semos_discovery_hit_rate")
    }

    async fn list_changesets(
        &self,
        _status: Option<&str>,
        _owner: Option<&str>,
        _scope: Option<&str>,
    ) -> Result<Vec<Changeset>, SemOsError> {
        unimplemented!("list_changesets is not used by semos_discovery_hit_rate")
    }

    async fn update_status(
        &self,
        _changeset_id: uuid::Uuid,
        _new_status: ChangesetStatus,
    ) -> Result<(), SemOsError> {
        unimplemented!("update_status is not used by semos_discovery_hit_rate")
    }

    async fn add_entry(
        &self,
        _changeset_id: uuid::Uuid,
        _input: sem_os_core::types::AddChangesetEntryInput,
    ) -> Result<ChangesetEntry, SemOsError> {
        unimplemented!("add_entry is not used by semos_discovery_hit_rate")
    }

    async fn list_entries(
        &self,
        _changeset_id: uuid::Uuid,
    ) -> Result<Vec<ChangesetEntry>, SemOsError> {
        unimplemented!("list_entries is not used by semos_discovery_hit_rate")
    }

    async fn submit_review(
        &self,
        _changeset_id: uuid::Uuid,
        _input: sem_os_core::types::SubmitReviewInput,
    ) -> Result<ChangesetReview, SemOsError> {
        unimplemented!("submit_review is not used by semos_discovery_hit_rate")
    }

    async fn list_reviews(
        &self,
        _changeset_id: uuid::Uuid,
    ) -> Result<Vec<ChangesetReview>, SemOsError> {
        unimplemented!("list_reviews is not used by semos_discovery_hit_rate")
    }
}

#[async_trait]
impl AuditStore for NoopAuditStore {
    async fn append(&self, _principal: &Principal, _entry: AuditEntry) -> Result<(), SemOsError> {
        Ok(())
    }
}

#[async_trait]
impl OutboxStore for NoopOutboxStore {
    async fn enqueue(&self, _event: OutboxEvent) -> Result<(), SemOsError> {
        Ok(())
    }

    async fn claim_next(&self, _claimer_id: &str) -> Result<Option<OutboxEvent>, SemOsError> {
        Ok(None)
    }

    async fn mark_processed(&self, _event_id: &EventId) -> Result<(), SemOsError> {
        Ok(())
    }

    async fn record_failure(&self, _event_id: &EventId, _error: &str) -> Result<(), SemOsError> {
        Ok(())
    }

    async fn mark_dead_letter(&self, _event_id: &EventId, _error: &str) -> Result<(), SemOsError> {
        Ok(())
    }
}

#[async_trait]
impl EvidenceInstanceStore for NoopEvidenceStore {
    async fn record(
        &self,
        _principal: &Principal,
        _instance: EvidenceInstance,
    ) -> Result<(), SemOsError> {
        Ok(())
    }
}

#[async_trait]
impl ProjectionWriter for NoopProjectionWriter {
    async fn write_active_snapshot_set(&self, _event: &OutboxEvent) -> Result<(), SemOsError> {
        Ok(())
    }
}

#[async_trait]
impl BootstrapAuditStore for NoopBootstrapAuditStore {
    async fn check_bootstrap(
        &self,
        _bundle_hash: &str,
    ) -> Result<Option<(String, Option<uuid::Uuid>)>, SemOsError> {
        Ok(None)
    }

    async fn start_bootstrap(
        &self,
        _bundle_hash: &str,
        _actor_id: &str,
        _bundle_counts: serde_json::Value,
    ) -> Result<(), SemOsError> {
        Ok(())
    }

    async fn mark_published(&self, _bundle_hash: &str) -> Result<(), SemOsError> {
        Ok(())
    }

    async fn mark_failed(&self, _bundle_hash: &str, _error: &str) -> Result<(), SemOsError> {
        Ok(())
    }
}

fn test_principal() -> Principal {
    Principal::in_process(
        "sem-os-discovery-harness",
        vec!["admin".into(), "analyst".into()],
    )
}

fn test_actor() -> ActorContext {
    ActorContext {
        actor_id: "sem-os-discovery-harness".into(),
        roles: vec!["analyst".into()],
        department: Some("operations".into()),
        clearance: None,
        jurisdictions: vec!["IE".into(), "LU".into()],
    }
}

fn fixture_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sem_os_discovery_utterances.toml")
}

fn metadata_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("config/sem_os_seeds/domain_metadata.yaml")
}

fn load_fixture() -> DiscoveryFixture {
    let content =
        std::fs::read_to_string(fixture_path()).expect("failed to read discovery fixture");
    toml::from_str(&content).expect("failed to parse discovery fixture")
}

fn build_service() -> CoreServiceImpl {
    let verbs = ConfigLoader::from_env()
        .load_verbs()
        .expect("verbs config should load");
    let metadata = DomainMetadata::from_file(&metadata_path())
        .expect("domain metadata should load for discovery harness");
    let bundle = build_seed_bundle_with_metadata(&verbs, Some(&metadata));

    let mut active = Vec::new();
    for seed in &bundle.universes {
        active.push(snapshot_row(
            sem_os_core::types::ObjectType::UniverseDef,
            &seed.fqn,
            seed.payload.clone(),
        ));
    }
    for seed in &bundle.constellation_families {
        active.push(snapshot_row(
            sem_os_core::types::ObjectType::ConstellationFamilyDef,
            &seed.fqn,
            seed.payload.clone(),
        ));
    }

    CoreServiceImpl::new(
        Arc::new(StaticSnapshotStore { active }),
        Arc::new(NoopObjectStore),
        Arc::new(NoopChangesetStore),
        Arc::new(NoopAuditStore),
        Arc::new(NoopOutboxStore),
        Arc::new(NoopEvidenceStore),
        Arc::new(NoopProjectionWriter),
    )
    .with_bootstrap_audit(Arc::new(NoopBootstrapAuditStore))
}

fn snapshot_row(
    object_type: sem_os_core::types::ObjectType,
    fqn: &str,
    definition: serde_json::Value,
) -> SnapshotRow {
    SnapshotRow {
        snapshot_id: Uuid::new_v4(),
        snapshot_set_id: None,
        object_type,
        object_id: object_id_for(object_type, fqn),
        version_major: 1,
        version_minor: 0,
        status: SnapshotStatus::Active,
        governance_tier: GovernanceTier::Operational,
        trust_class: TrustClass::Convenience,
        security_label: serde_json::to_value(SecurityLabel::default())
            .expect("security label json"),
        effective_from: Utc::now(),
        effective_until: None,
        predecessor_id: None,
        change_type: ChangeType::Created,
        change_rationale: None,
        created_by: "sem-os-discovery-harness".into(),
        approved_by: Some("auto".into()),
        definition,
        created_at: Utc::now(),
    }
}

fn build_request(case: &DiscoveryCase) -> ContextResolutionRequest {
    let mut known_inputs = case.known_inputs.clone();
    if let Some(jurisdiction) = &case.jurisdiction_hint {
        known_inputs
            .entry("jurisdiction".to_string())
            .or_insert_with(|| jurisdiction.clone());
    }
    if let Some(entity_kind) = &case.entity_kind_hint {
        known_inputs
            .entry("entity_kind".to_string())
            .or_insert_with(|| entity_kind.clone());
    }

    ContextResolutionRequest {
        subject: SubjectRef::TaskId(Uuid::new_v4()),
        intent_summary: case.intent_summary.clone(),
        raw_utterance: Some(case.utterance.clone()),
        actor: test_actor(),
        goals: vec![],
        constraints: ResolutionConstraints {
            jurisdiction: case.jurisdiction_hint.clone(),
            risk_posture: None,
            thresholds: HashMap::new(),
        },
        evidence_mode: EvidenceMode::Normal,
        point_in_time: None,
        entity_kind: None,
        entity_confidence: None,
        discovery: DiscoveryContext {
            selected_domain_id: case.selected_domain.clone(),
            selected_family_id: case.selected_family.clone(),
            selected_constellation_id: None,
            known_inputs,
        },
    }
}

fn extract_result(case: &DiscoveryCase, surface: &DiscoverySurface) -> DiscoveryResult {
    DiscoveryResult {
        case: case.clone(),
        top_domain: surface
            .matched_domains
            .first()
            .map(|value| value.domain_id.clone()),
        top_family: surface
            .matched_families
            .first()
            .map(|value| value.family_id.clone()),
        top_constellation: surface
            .matched_constellations
            .first()
            .map(|value| value.constellation_id.clone()),
        top3_constellations: surface
            .matched_constellations
            .iter()
            .take(3)
            .map(|value| value.constellation_id.clone())
            .collect(),
        readiness: format!("{:?}", surface.grounding_readiness).to_lowercase(),
    }
}

fn parse_threshold(name: &str, default: f64) -> f64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(default)
}

fn normalized_readiness(value: &str) -> String {
    value.trim().replace(['-', '_'], "").to_ascii_lowercase()
}

#[tokio::test]
async fn semos_discovery_hit_rate() {
    let filter = std::env::var("SEMOS_DISCOVERY_FILTER").ok();
    let fixture = load_fixture();
    let cases: Vec<DiscoveryCase> = fixture
        .tests
        .into_iter()
        .filter(|case| {
            filter
                .as_deref()
                .map(|value| case.name.contains(value))
                .unwrap_or(true)
        })
        .collect();

    let service = build_service();
    let mut results = Vec::new();

    println!("\n=======================================================================");
    println!("  SEM OS DISCOVERY HIT RATE -- {} utterances", cases.len());
    println!("=======================================================================\n");

    for case in &cases {
        let response = service
            .resolve_context(&test_principal(), build_request(case))
            .await
            .unwrap_or_else(|error| panic!("resolve_context failed for {}: {error}", case.name));

        assert_eq!(
            response.resolution_stage,
            ResolutionStage::Discovery,
            "case {} should remain in discovery stage",
            case.name
        );

        let surface = response
            .discovery_surface
            .as_ref()
            .unwrap_or_else(|| panic!("case {} missing discovery surface", case.name));
        let result = extract_result(case, surface);

        println!(
            "[{}] top_domain={:?} top_family={:?} top_constellation={:?} readiness={}{}",
            case.name,
            result.top_domain,
            result.top_family,
            result.top_constellation,
            result.readiness,
            case.notes
                .as_deref()
                .map(|notes| format!(" -- {notes}"))
                .unwrap_or_default()
        );

        results.push(result);
    }

    let domain_total = results
        .iter()
        .filter(|result| result.case.expected_domain.is_some())
        .count();
    let domain_hits = results
        .iter()
        .filter(|result| result.top_domain == result.case.expected_domain)
        .count();

    let family_total = results
        .iter()
        .filter(|result| result.case.expected_family.is_some())
        .count();
    let family_hits = results
        .iter()
        .filter(|result| result.top_family == result.case.expected_family)
        .count();

    let constellation_total = results
        .iter()
        .filter(|result| result.case.expected_constellation.is_some())
        .count();
    let constellation_top1_hits = results
        .iter()
        .filter(|result| result.top_constellation == result.case.expected_constellation)
        .count();
    let constellation_top3_hits = results
        .iter()
        .filter(|result| {
            result
                .case
                .expected_constellation
                .as_ref()
                .is_some_and(|expected| {
                    result
                        .top3_constellations
                        .iter()
                        .any(|value| value == expected)
                })
        })
        .count();

    let readiness_total = results
        .iter()
        .filter(|result| result.case.expected_readiness.is_some())
        .count();
    let readiness_hits = results
        .iter()
        .filter(|result| {
            result
                .case
                .expected_readiness
                .as_ref()
                .is_some_and(|expected| normalized_readiness(expected) == result.readiness)
        })
        .count();

    let domain_rate = if domain_total == 0 {
        1.0
    } else {
        domain_hits as f64 / domain_total as f64
    };
    let family_rate = if family_total == 0 {
        1.0
    } else {
        family_hits as f64 / family_total as f64
    };
    let constellation_top1_rate = if constellation_total == 0 {
        1.0
    } else {
        constellation_top1_hits as f64 / constellation_total as f64
    };
    let constellation_top3_rate = if constellation_total == 0 {
        1.0
    } else {
        constellation_top3_hits as f64 / constellation_total as f64
    };
    let readiness_rate = if readiness_total == 0 {
        1.0
    } else {
        readiness_hits as f64 / readiness_total as f64
    };

    println!("\nSummary");
    println!("  domain top1:          {:.1}%", domain_rate * 100.0);
    println!("  family top1:          {:.1}%", family_rate * 100.0);
    println!(
        "  constellation top1:   {:.1}%",
        constellation_top1_rate * 100.0
    );
    println!(
        "  constellation top3:   {:.1}%",
        constellation_top3_rate * 100.0
    );
    println!("  readiness exact:      {:.1}%", readiness_rate * 100.0);

    assert!(
        domain_rate >= parse_threshold("SEMOS_DISCOVERY_DOMAIN_THRESHOLD", 0.80),
        "domain top1 hit rate below threshold: {:.1}%",
        domain_rate * 100.0
    );
    assert!(
        family_rate >= parse_threshold("SEMOS_DISCOVERY_FAMILY_THRESHOLD", 0.80),
        "family top1 hit rate below threshold: {:.1}%",
        family_rate * 100.0
    );
    assert!(
        constellation_top1_rate
            >= parse_threshold("SEMOS_DISCOVERY_CONSTELLATION_TOP1_THRESHOLD", 0.60),
        "constellation top1 hit rate below threshold: {:.1}%",
        constellation_top1_rate * 100.0
    );
    assert!(
        constellation_top3_rate
            >= parse_threshold("SEMOS_DISCOVERY_CONSTELLATION_TOP3_THRESHOLD", 0.90),
        "constellation top3 hit rate below threshold: {:.1}%",
        constellation_top3_rate * 100.0
    );
    assert!(
        readiness_rate >= parse_threshold("SEMOS_DISCOVERY_READINESS_THRESHOLD", 0.80),
        "readiness hit rate below threshold: {:.1}%",
        readiness_rate * 100.0
    );
}
