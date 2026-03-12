//! Projection harness tests (Stage 2.2).
//!
//! Tests that the outbox → projection pipeline works:
//! - Watermark advances after drain.
//! - sem_reg_pub.active_* tables contain expected entries.

use sem_os_client::SemOsClient;
use sem_os_core::principal::Principal;
use sem_os_core::proof_obligation_def::ProofStrength;
use sem_os_core::seeds::*;
use sqlx::PgPool;
use uuid::Uuid;

fn test_principal() -> Principal {
    Principal::in_process("harness-agent", vec!["admin".into(), "analyst".into()])
}

fn make_verb_contract_seed(fqn: &str, domain: &str, description: &str) -> VerbContractSeed {
    VerbContractSeed {
        fqn: fqn.into(),
        payload: serde_json::json!({
            "fqn": fqn,
            "domain": domain,
            "description": description,
            "subject_kinds": [],
            "preconditions": [],
            "postconditions": [],
            "required_attributes": [],
        }),
    }
}

fn make_entity_type_seed(fqn: &str, domain: &str, name: &str) -> EntityTypeSeed {
    EntityTypeSeed {
        fqn: fqn.into(),
        payload: serde_json::json!({
            "fqn": fqn,
            "domain": domain,
            "name": name,
            "required_attributes": [],
            "optional_attributes": [],
        }),
    }
}

fn make_taxonomy_seed(fqn: &str, domain: &str, name: &str) -> TaxonomySeed {
    TaxonomySeed {
        fqn: fqn.into(),
        payload: serde_json::json!({
            "fqn": fqn,
            "domain": domain,
            "name": name,
            "description": format!("Taxonomy: {name}"),
        }),
    }
}

fn make_requirement_profile_seed(fqn: &str) -> RequirementProfileSeed {
    RequirementProfileSeed {
        fqn: fqn.into(),
        payload: serde_json::json!({
            "fqn": fqn,
            "name": "Projection test requirement profile",
            "description": "Projection test requirement profile",
            "entity_types": ["entity.legal-entity"],
            "jurisdictions": ["GB"],
            "client_types": ["institutional"],
            "contexts": ["kyc_entity"],
            "obligation_fqns": [],
        }),
    }
}

fn make_proof_obligation_seed(fqn: &str, strategy_fqn: &str) -> ProofObligationSeed {
    ProofObligationSeed {
        fqn: fqn.into(),
        payload: serde_json::json!({
            "fqn": fqn,
            "name": "Projection test proof obligation",
            "description": "Projection test proof obligation",
            "category": "identity",
            "strength_required": ProofStrength::Primary,
            "is_mandatory": true,
            "evidence_strategy_fqns": [strategy_fqn],
            "conditions": [],
        }),
    }
}

fn make_evidence_strategy_seed(fqn: &str, obligation_fqn: &str) -> EvidenceStrategySeed {
    EvidenceStrategySeed {
        fqn: fqn.into(),
        payload: serde_json::json!({
            "fqn": fqn,
            "name": "Projection test evidence strategy",
            "description": "Projection test evidence strategy",
            "obligation_fqn": obligation_fqn,
            "priority": 1,
            "proof_strength": ProofStrength::Primary,
            "components": [{
                "document_type_fqn": "doc.type.passport",
                "role": "primary",
                "required": true,
                "attributes_proven": ["entity.name"]
            }],
            "enabled": true,
        }),
    }
}

/// Run the projection scenario suite against any SemOsClient + pool.
///
/// The pool is needed to query sem_reg_pub tables directly for assertions.
pub async fn run_projection_scenario_suite(client: &dyn SemOsClient, pool: &PgPool) {
    test_projection_watermark_advances(client, pool).await;
}

/// Publish a seed bundle, drain outbox, verify watermark advanced and
/// sem_reg_pub.active_* tables contain the expected entries.
async fn test_projection_watermark_advances(client: &dyn SemOsClient, pool: &PgPool) {
    tracing::info!("test_projection_watermark_advances: starting");
    let principal = test_principal();

    // Read watermark before.
    let before_wm: Option<i64> = sqlx::query_scalar(
        "SELECT last_outbox_seq FROM sem_reg_pub.projection_watermark WHERE projection_name = 'active_snapshot_set'"
    )
    .fetch_optional(pool)
    .await
    .expect("watermark query failed")
    .flatten();

    // Create unique seed data.
    let unique = Uuid::new_v4().simple().to_string();
    let verb_fqn = format!("proj-test.verb-{unique}");
    let entity_fqn = format!("proj-test.entity-{unique}");
    let taxonomy_fqn = format!("proj-test.taxonomy-{unique}");
    let requirement_profile_fqn = format!("proj-test.requirement-profile-{unique}");
    let proof_obligation_fqn = format!("proj-test.proof-obligation-{unique}");
    let evidence_strategy_fqn = format!("proj-test.evidence-strategy-{unique}");

    let verb_contracts = vec![make_verb_contract_seed(
        &verb_fqn,
        "proj-test",
        "Projection test verb",
    )];
    let entity_types = vec![make_entity_type_seed(
        &entity_fqn,
        "proj-test",
        "Projection test entity",
    )];
    let taxonomies = vec![make_taxonomy_seed(
        &taxonomy_fqn,
        "proj-test",
        "Projection test taxonomy",
    )];
    let requirement_profiles = vec![make_requirement_profile_seed(&requirement_profile_fqn)];
    let proof_obligations = vec![make_proof_obligation_seed(
        &proof_obligation_fqn,
        &evidence_strategy_fqn,
    )];
    let evidence_strategies = vec![make_evidence_strategy_seed(
        &evidence_strategy_fqn,
        &proof_obligation_fqn,
    )];

    let bundle_hash = SeedBundle::compute_hash(
        &verb_contracts,
        &[],
        &entity_types,
        &taxonomies,
        &[],
        &[],
        &[],
        &requirement_profiles,
        &proof_obligations,
        &evidence_strategies,
    )
    .expect("test seed bundle hash");

    let bundle = SeedBundle {
        bundle_hash,
        verb_contracts,
        attributes: vec![],
        entity_types,
        taxonomies,
        policies: vec![],
        views: vec![],
        derivation_specs: vec![],
        requirement_profiles,
        proof_obligations,
        evidence_strategies,
    };

    // Bootstrap — creates snapshots + enqueues outbox events.
    let resp = client
        .bootstrap_seed_bundle(&principal, bundle)
        .await
        .expect("bootstrap for projection test");
    assert_eq!(resp.created, 6, "expected 6 items created");

    // Drain outbox — processes events through the projection writer.
    client
        .drain_outbox_for_test()
        .await
        .expect("drain_outbox_for_test should succeed now that projection writer is real");

    // Read watermark after — should have advanced.
    let after_wm: Option<i64> = sqlx::query_scalar(
        "SELECT last_outbox_seq FROM sem_reg_pub.projection_watermark WHERE projection_name = 'active_snapshot_set'"
    )
    .fetch_optional(pool)
    .await
    .expect("watermark query after drain failed")
    .flatten();

    let after_wm = after_wm.expect("watermark should exist after drain");
    match before_wm {
        Some(bw) => {
            assert!(
                after_wm > bw,
                "watermark should advance: before={bw}, after={after_wm}"
            );
        }
        None => {
            // First time — watermark should be positive.
            assert!(
                after_wm > 0,
                "watermark should be positive after first drain, got {after_wm}"
            );
        }
    }

    // Verify sem_reg_pub.active_verb_contracts contains our verb.
    let verb_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sem_reg_pub.active_verb_contracts WHERE fqn = $1")
            .bind(&verb_fqn)
            .fetch_one(pool)
            .await
            .expect("verb contract query failed");
    assert!(verb_count >= 1, "expected verb contract in sem_reg_pub.active_verb_contracts for {verb_fqn}, found {verb_count}");

    // Verify sem_reg_pub.active_entity_types contains our entity type.
    let entity_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sem_reg_pub.active_entity_types WHERE fqn = $1")
            .bind(&entity_fqn)
            .fetch_one(pool)
            .await
            .expect("entity type query failed");
    assert!(entity_count >= 1, "expected entity type in sem_reg_pub.active_entity_types for {entity_fqn}, found {entity_count}");

    // Verify sem_reg_pub.active_taxonomies contains our taxonomy.
    let taxonomy_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sem_reg_pub.active_taxonomies WHERE fqn = $1")
            .bind(&taxonomy_fqn)
            .fetch_one(pool)
            .await
            .expect("taxonomy query failed");
    assert!(taxonomy_count >= 1, "expected taxonomy in sem_reg_pub.active_taxonomies for {taxonomy_fqn}, found {taxonomy_count}");

    let requirement_profile_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sem_reg_pub.active_requirement_profiles WHERE fqn = $1",
    )
    .bind(&requirement_profile_fqn)
    .fetch_one(pool)
    .await
    .expect("requirement profile query failed");
    assert!(
        requirement_profile_count >= 1,
        "expected requirement profile in sem_reg_pub.active_requirement_profiles for {requirement_profile_fqn}, found {requirement_profile_count}"
    );

    let proof_obligation_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sem_reg_pub.active_proof_obligations WHERE fqn = $1",
    )
    .bind(&proof_obligation_fqn)
    .fetch_one(pool)
    .await
    .expect("proof obligation query failed");
    assert!(
        proof_obligation_count >= 1,
        "expected proof obligation in sem_reg_pub.active_proof_obligations for {proof_obligation_fqn}, found {proof_obligation_count}"
    );

    let evidence_strategy_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sem_reg_pub.active_evidence_strategies WHERE fqn = $1",
    )
    .bind(&evidence_strategy_fqn)
    .fetch_one(pool)
    .await
    .expect("evidence strategy query failed");
    assert!(
        evidence_strategy_count >= 1,
        "expected evidence strategy in sem_reg_pub.active_evidence_strategies for {evidence_strategy_fqn}, found {evidence_strategy_count}"
    );

    tracing::info!("test_projection_watermark_advances: passed (watermark={after_wm})");
}
