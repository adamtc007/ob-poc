use anyhow::Result;
use k8s_openapi::api::apps::v1::{Deployment, DeploymentSpec};
use k8s_openapi::api::autoscaling::v2::{
    CrossVersionObjectReference, HorizontalPodAutoscaler, HorizontalPodAutoscalerSpec, MetricSpec,
    MetricTarget, ResourceMetricSource,
};
use k8s_openapi::api::core::v1::{
    Container, EnvVar, EnvVarSource, PodSpec, PodTemplateSpec, ResourceRequirements,
    SecretKeySelector,
};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta};
use kube::api::{Api, DeleteParams, Patch, PatchParams};
use ob_poc_types::PoolConfig;
use std::collections::BTreeMap;
use tracing::{info, warn};

use crate::k8s::K8sClient;

// ── Name helpers ──────────────────────────────────────────────────────────────

/// Sanitise pool_id to a valid K8s resource name and prefix it.
///
/// Rules: lowercase alphanumeric + hyphens, max 63 chars.
pub(crate) fn deployment_name(pool_id: &str) -> String {
    let sanitized: String = pool_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();

    let suffix = sanitized.trim_matches('-');
    let suffix = if suffix.is_empty() { "default" } else { suffix };

    let prefix = "bpmn-lite-pool-";
    let max_suffix = 63 - prefix.len();
    let suffix = &suffix[..suffix.len().min(max_suffix)];

    format!("{}{}", prefix, suffix)
}

// ── Spec builders ─────────────────────────────────────────────────────────────

fn pool_labels(pool_id: &str, pool_type_str: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("app".to_string(), "bpmn-lite".to_string()),
        ("pool-id".to_string(), pool_id.to_string()),
        ("pool-type".to_string(), pool_type_str.to_string()),
    ])
}

fn selector_labels(pool_id: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("app".to_string(), "bpmn-lite".to_string()),
        ("pool-id".to_string(), pool_id.to_string()),
    ])
}

fn secret_ref(secret_name: &str, key: &str, optional: bool) -> EnvVar {
    EnvVar {
        name: key.to_string(),
        value_from: Some(EnvVarSource {
            secret_key_ref: Some(SecretKeySelector {
                name: Some(secret_name.to_string()),
                key: key.to_string(),
                optional: Some(optional),
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// Build the Deployment spec for a bpmn-lite worker pool.
///
/// Mounts `DATABASE_URL` and `DATABASE_ADMIN_URL` from Secret `bpmn-lite-db-secret`.
/// Injects `BPMN_LITE_POOL_ID` so workers can identify their pool.
fn build_deployment(
    pool_id: &str,
    pool_type_str: &str,
    config: &PoolConfig,
    namespace: &str,
) -> Deployment {
    let name = deployment_name(pool_id);
    let labels = pool_labels(pool_id, pool_type_str);
    let selector = selector_labels(pool_id);

    Deployment {
        metadata: ObjectMeta {
            name: Some(name),
            namespace: Some(namespace.to_string()),
            labels: Some(labels.clone()),
            ..Default::default()
        },
        spec: Some(DeploymentSpec {
            replicas: Some(config.replicas as i32),
            selector: LabelSelector {
                match_labels: Some(selector),
                ..Default::default()
            },
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    labels: Some(labels),
                    ..Default::default()
                }),
                spec: Some(PodSpec {
                    containers: vec![Container {
                        name: "bpmn-lite-worker".to_string(),
                        image: Some(config.image.clone()),
                        env: Some(vec![
                            EnvVar {
                                name: "BPMN_LITE_POOL_ID".to_string(),
                                value: Some(pool_id.to_string()),
                                ..Default::default()
                            },
                            // DATABASE_URL required; DATABASE_ADMIN_URL optional
                            // (workers that skip migrations don't need it).
                            secret_ref("bpmn-lite-db-secret", "DATABASE_URL", false),
                            secret_ref("bpmn-lite-db-secret", "DATABASE_ADMIN_URL", true),
                        ]),
                        resources: Some(ResourceRequirements {
                            requests: Some(BTreeMap::from([
                                ("cpu".to_string(), Quantity(config.cpu_request.clone())),
                                (
                                    "memory".to_string(),
                                    Quantity(config.memory_request.clone()),
                                ),
                            ])),
                            limits: Some(BTreeMap::from([
                                ("cpu".to_string(), Quantity(config.cpu_limit.clone())),
                                ("memory".to_string(), Quantity(config.memory_limit.clone())),
                            ])),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
            },
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// Build the HPA spec targeting the pool's Deployment.
///
/// Scales on CPU utilisation (70% target). Min/max replicas come from
/// `PoolConfig`. Callers provision the HPA alongside the Deployment.
fn build_hpa(pool_id: &str, config: &PoolConfig, namespace: &str) -> HorizontalPodAutoscaler {
    let name = deployment_name(pool_id);
    HorizontalPodAutoscaler {
        metadata: ObjectMeta {
            name: Some(name.clone()),
            namespace: Some(namespace.to_string()),
            ..Default::default()
        },
        spec: Some(HorizontalPodAutoscalerSpec {
            scale_target_ref: CrossVersionObjectReference {
                api_version: Some("apps/v1".to_string()),
                kind: "Deployment".to_string(),
                name,
            },
            min_replicas: Some(config.min_replicas as i32),
            max_replicas: config.max_replicas as i32,
            metrics: Some(vec![MetricSpec {
                type_: "Resource".to_string(),
                resource: Some(ResourceMetricSource {
                    name: "cpu".to_string(),
                    target: MetricTarget {
                        type_: "Utilization".to_string(),
                        average_utilization: Some(70),
                        ..Default::default()
                    },
                }),
                ..Default::default()
            }]),
            ..Default::default()
        }),
        ..Default::default()
    }
}

// ── K8s API operations ────────────────────────────────────────────────────────

/// Create or update the pool's Deployment via server-side apply (idempotent).
pub(crate) async fn create_deployment(
    k8s: &K8sClient,
    pool_id: &str,
    pool_type_str: &str,
    config: &PoolConfig,
) -> Result<()> {
    let client = match k8s.inner.as_ref() {
        Some(c) => c,
        None => {
            warn!(
                pool_id,
                "K8sClient::placeholder — skipping create_deployment"
            );
            return Ok(());
        }
    };

    let deployment = build_deployment(pool_id, pool_type_str, config, &k8s.namespace);
    let name = deployment_name(pool_id);
    let api: Api<Deployment> = Api::namespaced(client.clone(), &k8s.namespace);

    api.patch(
        &name,
        &PatchParams::apply("bpmn-controller").force(),
        &Patch::Apply(&deployment),
    )
    .await?;

    info!(pool_id, name, "Deployment created/updated");
    Ok(())
}

/// Create or update the pool's HPA via server-side apply (idempotent).
pub(crate) async fn create_hpa(k8s: &K8sClient, pool_id: &str, config: &PoolConfig) -> Result<()> {
    let client = match k8s.inner.as_ref() {
        Some(c) => c,
        None => {
            warn!(pool_id, "K8sClient::placeholder — skipping create_hpa");
            return Ok(());
        }
    };

    let hpa = build_hpa(pool_id, config, &k8s.namespace);
    let name = deployment_name(pool_id);
    let api: Api<HorizontalPodAutoscaler> = Api::namespaced(client.clone(), &k8s.namespace);

    api.patch(
        &name,
        &PatchParams::apply("bpmn-controller").force(),
        &Patch::Apply(&hpa),
    )
    .await?;

    info!(pool_id, name, "HPA created/updated");
    Ok(())
}

/// Delete the pool's Deployment. Returns Ok if already absent (404-tolerant).
pub(crate) async fn delete_deployment(k8s: &K8sClient, pool_id: &str) -> Result<()> {
    let client = match k8s.inner.as_ref() {
        Some(c) => c,
        None => {
            warn!(
                pool_id,
                "K8sClient::placeholder — skipping delete_deployment"
            );
            return Ok(());
        }
    };

    let name = deployment_name(pool_id);
    let api: Api<Deployment> = Api::namespaced(client.clone(), &k8s.namespace);

    match api.delete(&name, &DeleteParams::default()).await {
        Ok(_) => {
            info!(pool_id, name, "Deployment deleted");
            Ok(())
        }
        Err(kube::Error::Api(e)) if e.code == 404 => {
            info!(pool_id, name, "Deployment already absent");
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

/// Delete the pool's HPA. Returns Ok if already absent (404-tolerant).
pub(crate) async fn delete_hpa(k8s: &K8sClient, pool_id: &str) -> Result<()> {
    let client = match k8s.inner.as_ref() {
        Some(c) => c,
        None => {
            warn!(pool_id, "K8sClient::placeholder — skipping delete_hpa");
            return Ok(());
        }
    };

    let name = deployment_name(pool_id);
    let api: Api<HorizontalPodAutoscaler> = Api::namespaced(client.clone(), &k8s.namespace);

    match api.delete(&name, &DeleteParams::default()).await {
        Ok(_) => {
            info!(pool_id, name, "HPA deleted");
            Ok(())
        }
        Err(kube::Error::Api(e)) if e.code == 404 => {
            info!(pool_id, name, "HPA already absent");
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

/// Current replica counts for a pool's Deployment.
pub(crate) struct DeploymentReplicaStatus {
    pub desired: Option<i32>,
    pub ready: Option<i32>,
}

/// Query replica counts from K8s. Returns `None` if not provisioned or
/// if the client is a placeholder.
pub(crate) async fn get_deployment_status(
    k8s: &K8sClient,
    pool_id: &str,
) -> Result<Option<DeploymentReplicaStatus>> {
    let client = match k8s.inner.as_ref() {
        Some(c) => c,
        None => return Ok(None),
    };

    let name = deployment_name(pool_id);
    let api: Api<Deployment> = Api::namespaced(client.clone(), &k8s.namespace);

    match api.get_opt(&name).await? {
        None => Ok(None),
        Some(dep) => {
            let desired = dep.spec.as_ref().and_then(|s| s.replicas);
            let ready = dep.status.as_ref().and_then(|s| s.ready_replicas);
            Ok(Some(DeploymentReplicaStatus { desired, ready }))
        }
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ob_poc_types::PoolConfig;

    fn test_config() -> PoolConfig {
        PoolConfig {
            image: "ghcr.io/adamtc007/bpmn-lite:latest".to_string(),
            replicas: 2,
            min_replicas: 1,
            max_replicas: 5,
            cpu_request: "250m".to_string(),
            memory_request: "256Mi".to_string(),
            cpu_limit: "1000m".to_string(),
            memory_limit: "512Mi".to_string(),
            namespace: "bpmn-lite".to_string(),
        }
    }

    #[test]
    fn deployment_name_basic() {
        assert_eq!(deployment_name("default"), "bpmn-lite-pool-default");
        assert_eq!(deployment_name("tenant-1"), "bpmn-lite-pool-tenant-1");
    }

    #[test]
    fn deployment_name_sanitizes_underscores() {
        assert_eq!(deployment_name("my_pool"), "bpmn-lite-pool-my-pool");
    }

    #[test]
    fn deployment_name_sanitizes_uppercase() {
        assert_eq!(deployment_name("MyPool"), "bpmn-lite-pool-mypool");
    }

    #[test]
    fn deployment_name_truncates_long_ids() {
        let long = "a".repeat(100);
        let name = deployment_name(&long);
        assert!(name.len() <= 63, "name must not exceed 63 chars");
        assert!(name.starts_with("bpmn-lite-pool-"));
    }

    #[test]
    fn deployment_name_empty_fallback() {
        assert_eq!(deployment_name("---"), "bpmn-lite-pool-default");
    }

    #[test]
    fn build_deployment_sets_replicas() {
        let dep = build_deployment("test-pool", "default", &test_config(), "bpmn-lite");
        let replicas = dep.spec.as_ref().and_then(|s| s.replicas).unwrap();
        assert_eq!(replicas, 2);
    }

    #[test]
    fn build_deployment_sets_image() {
        let dep = build_deployment("test-pool", "default", &test_config(), "bpmn-lite");
        let image = dep
            .spec
            .as_ref()
            .and_then(|s| s.template.spec.as_ref())
            .and_then(|p| p.containers.first())
            .and_then(|c| c.image.as_deref())
            .unwrap();
        assert_eq!(image, "ghcr.io/adamtc007/bpmn-lite:latest");
    }

    #[test]
    fn build_deployment_has_pool_id_env() {
        let dep = build_deployment("test-pool", "default", &test_config(), "bpmn-lite");
        let env = dep
            .spec
            .as_ref()
            .and_then(|s| s.template.spec.as_ref())
            .and_then(|p| p.containers.first())
            .and_then(|c| c.env.as_ref())
            .unwrap();
        let var = env.iter().find(|e| e.name == "BPMN_LITE_POOL_ID").unwrap();
        assert_eq!(var.value.as_deref(), Some("test-pool"));
    }

    #[test]
    fn build_deployment_has_db_secret_refs() {
        let dep = build_deployment("test-pool", "default", &test_config(), "bpmn-lite");
        let env = dep
            .spec
            .as_ref()
            .and_then(|s| s.template.spec.as_ref())
            .and_then(|p| p.containers.first())
            .and_then(|c| c.env.as_ref())
            .unwrap();
        assert!(env.iter().any(|e| {
            e.name == "DATABASE_URL"
                && e.value_from
                    .as_ref()
                    .and_then(|vf| vf.secret_key_ref.as_ref())
                    .and_then(|s| s.name.as_deref())
                    .map(|n| n == "bpmn-lite-db-secret")
                    .unwrap_or(false)
        }));
    }

    #[test]
    fn build_deployment_sets_labels() {
        let dep = build_deployment("test-pool", "dedicated", &test_config(), "bpmn-lite");
        let labels = dep.metadata.labels.as_ref().unwrap();
        assert_eq!(labels.get("app").map(String::as_str), Some("bpmn-lite"));
        assert_eq!(labels.get("pool-id").map(String::as_str), Some("test-pool"));
        assert_eq!(
            labels.get("pool-type").map(String::as_str),
            Some("dedicated")
        );
    }

    #[test]
    fn build_deployment_selector_matches_template_labels() {
        let dep = build_deployment("test-pool", "default", &test_config(), "bpmn-lite");
        let spec = dep.spec.as_ref().unwrap();
        let selector = spec.selector.match_labels.as_ref().unwrap();
        let template_labels = spec
            .template
            .metadata
            .as_ref()
            .and_then(|m| m.labels.as_ref())
            .unwrap();
        for (k, v) in selector {
            assert_eq!(
                template_labels.get(k).map(String::as_str),
                Some(v.as_str()),
                "selector key '{k}' must match template label"
            );
        }
    }

    #[test]
    fn build_hpa_targets_correct_deployment() {
        let hpa = build_hpa("test-pool", &test_config(), "bpmn-lite");
        let spec = hpa.spec.as_ref().unwrap();
        assert_eq!(spec.scale_target_ref.kind, "Deployment");
        assert_eq!(
            spec.scale_target_ref.api_version.as_deref(),
            Some("apps/v1")
        );
        assert_eq!(spec.scale_target_ref.name, deployment_name("test-pool"));
    }

    #[test]
    fn build_hpa_sets_min_max_replicas() {
        let hpa = build_hpa("test-pool", &test_config(), "bpmn-lite");
        let spec = hpa.spec.as_ref().unwrap();
        assert_eq!(spec.min_replicas, Some(1));
        assert_eq!(spec.max_replicas, 5);
    }

    #[test]
    fn build_hpa_has_cpu_metric() {
        let hpa = build_hpa("test-pool", &test_config(), "bpmn-lite");
        let metrics = hpa.spec.as_ref().and_then(|s| s.metrics.as_ref()).unwrap();
        assert_eq!(metrics.len(), 1);
        let m = &metrics[0];
        assert_eq!(m.type_, "Resource");
        let resource = m.resource.as_ref().unwrap();
        assert_eq!(resource.name, "cpu");
        assert_eq!(resource.target.type_, "Utilization");
        assert_eq!(resource.target.average_utilization, Some(70));
    }
}
