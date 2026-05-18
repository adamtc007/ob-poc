use anyhow::Result;

/// K8s client for pool operations.
///
/// Wraps `kube::Client` plus the target namespace. The `inner` field is
/// `Option<kube::Client>` so that `placeholder()` (used in DB-only tests and
/// in the read operations that don't need K8s) works without a real cluster.
///
/// In production use `from_infer()` which discovers in-cluster config or
/// falls back to the local kubeconfig file.
pub struct K8sClient {
    pub(crate) inner: Option<kube::Client>,
    pub(crate) namespace: String,
}

impl K8sClient {
    /// Infer cluster connection (in-cluster → kubeconfig) and read namespace
    /// from `BPMN_LITE_K8S_NAMESPACE` env var (default `"default"`).
    pub async fn from_infer() -> Result<Self> {
        let client = kube::Client::try_default().await?;
        let namespace =
            std::env::var("BPMN_LITE_K8S_NAMESPACE").unwrap_or_else(|_| "default".to_string());
        Ok(Self {
            inner: Some(client),
            namespace,
        })
    }

    /// No-op placeholder — K8s status queries return `None`. Use this in
    /// contexts where K8s operations are not required (DB-only reads, tests).
    pub fn placeholder() -> Self {
        Self {
            inner: None,
            namespace: "default".to_string(),
        }
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Returns true if this client can make real K8s API calls.
    pub fn is_connected(&self) -> bool {
        self.inner.is_some()
    }
}
