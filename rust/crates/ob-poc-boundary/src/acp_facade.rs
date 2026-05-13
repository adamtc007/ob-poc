//! Single-path domain facade for ACP operations.
//!
//! `AcpFacade` is the **only** type that REST handlers and the stdio
//! JSON-RPC dispatcher should call when they need ACP domain semantics.
//! Each method maps a `(session_id, …)` HTTP/stdio request to the
//! corresponding `crate::acp::*` domain function, owning the manifest
//! load and the throwaway session synthesis.
//!
//! ## Why this exists
//!
//! Before this facade, both transports inlined the same three-step
//! pattern at every call site:
//!
//! ```text
//!     let manifest = load_ob_poc_kyc_domain_pack()?;
//!     let session  = acp::open_acp_session(session_id, …);
//!     acp::<domain_fn>(&session, &manifest, …)
//! ```
//!
//! Duplicating that pattern across REST handlers (`repl_routes_v2.rs`)
//! and stdio dispatch (`acp_protocol.rs`) violated the R8 single-path
//! invariant: two transports built the same envelopes through two
//! parallel code paths. The facade collapses that to one path.
//!
//! ## What it does not do
//!
//! The HTTP-only live overlay (`build_live_acp_projection` in
//! `api::repl_routes_v2`) is still REST-specific because it depends on
//! `ReplSessionV2`. Stdio receives the declared-source view via
//! `projection_get`; REST overlays live session data on top of it.
//! See the doc comments on those two functions for the design intent.
//!
//! ## What lives where
//!
//! - **`acp.rs`** — pure domain functions; no transport concerns.
//! - **`acp_facade.rs`** (this file) — single entry-point for transports.
//! - **`acp_protocol.rs`** — stdio JSON-RPC dispatch; calls into facade.
//! - **`api::repl_routes_v2`** — REST handlers; call into facade.

use uuid::Uuid;

use sem_os_core::acp_projection::AcpProjectionEnvelope;
use sem_os_core::domain_pack::{
    DiscoveryRequest, DiscoveryResponse, DomainPackManifest, ProjectionCatalogEntry,
};

use crate::acp::{
    self, AcpAdapterError, AcpAdapterKind, AcpKycCaseStateSnapshot, AcpKycLanguageLoopTimedOutcome,
    AcpPersonaMode, AcpPolicyCapabilities, AcpProjectionRequest, AcpSageContextBundle, AcpSession,
};
use crate::language_pack::{
    KycLanguagePackRequest, SemOsLanguagePack, UpdateStatusLanguagePackRequest,
};
use crate::workbook_revision::KycUpdateStatusWorkbookDraft;

/// Domain facade for ACP operations. Owns the manifest; mediates between
/// transport handlers and `crate::acp::*` domain functions.
pub struct AcpFacade {
    manifest: DomainPackManifest,
    adapter: AcpAdapterKind,
}

impl AcpFacade {
    pub fn new(manifest: DomainPackManifest, adapter: AcpAdapterKind) -> Self {
        Self { manifest, adapter }
    }

    /// Construct a facade against the bundled `ob-poc.kyc` Domain Pack.
    /// Used by both REST and stdio for their per-request facade instances.
    pub fn for_default_pack(adapter: AcpAdapterKind) -> Result<Self, AcpAdapterError> {
        load_ob_poc_kyc_domain_pack().map(|manifest| Self::new(manifest, adapter))
    }

    pub fn manifest(&self) -> &DomainPackManifest {
        &self.manifest
    }

    pub fn adapter(&self) -> AcpAdapterKind {
        self.adapter
    }

    fn session(&self, session_id: Uuid) -> AcpSession {
        acp::open_acp_session(session_id, self.adapter)
    }

    /// Open a session with an explicit persona (default: SagePlanning).
    /// Used by the `/acp/open` HTTP route and `session/new` stdio method.
    pub fn open_session_with_persona(
        &self,
        session_id: Uuid,
        persona: AcpPersonaMode,
    ) -> AcpSession {
        acp::open_acp_session_with_persona(session_id, self.adapter, persona)
    }

    // Domain operations are exposed in two variants:
    //
    // - `<op>(session_id, …)` synthesizes a fresh session. REST handlers
    //   use these because they do not maintain a per-request session cache.
    //
    // - `<op>_for(session, …)` operates on a caller-owned session. Stdio
    //   (`AcpJsonRpcAgent`) uses these because it caches sessions in a
    //   `BTreeMap<Uuid, AcpSession>` so it can enforce the closed-state
    //   transition across multiple JSON-RPC requests in the same session.

    pub fn policy(&self, session_id: Uuid) -> Result<AcpPolicyCapabilities, AcpAdapterError> {
        self.policy_for(&self.session(session_id))
    }

    pub fn policy_for(
        &self,
        session: &AcpSession,
    ) -> Result<AcpPolicyCapabilities, AcpAdapterError> {
        acp::acp_policy_capabilities(session, &self.manifest)
    }

    pub fn projections_list(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<ProjectionCatalogEntry>, AcpAdapterError> {
        self.projections_list_for(&self.session(session_id))
    }

    pub fn projections_list_for(
        &self,
        session: &AcpSession,
    ) -> Result<Vec<ProjectionCatalogEntry>, AcpAdapterError> {
        acp::list_acp_projections(session, &self.manifest)
    }

    pub fn projection_get(
        &self,
        session_id: Uuid,
        request: AcpProjectionRequest,
    ) -> Result<AcpProjectionEnvelope, AcpAdapterError> {
        self.projection_get_for(&self.session(session_id), request)
    }

    pub fn projection_get_for(
        &self,
        session: &AcpSession,
        request: AcpProjectionRequest,
    ) -> Result<AcpProjectionEnvelope, AcpAdapterError> {
        acp::build_acp_projection(session, &self.manifest, request)
    }

    pub fn context_assemble(
        &self,
        session_id: Uuid,
        request: DiscoveryRequest,
        response: DiscoveryResponse,
    ) -> Result<AcpSageContextBundle, AcpAdapterError> {
        self.context_assemble_for(&self.session(session_id), request, response)
    }

    pub fn context_assemble_for(
        &self,
        session: &AcpSession,
        request: DiscoveryRequest,
        response: DiscoveryResponse,
    ) -> Result<AcpSageContextBundle, AcpAdapterError> {
        acp::assemble_sage_context_for_acp(session, &self.manifest, request, response)
    }

    pub fn kyc_case_state_discover_for(
        &self,
        session: &AcpSession,
        case_id: Uuid,
        response: DiscoveryResponse,
    ) -> Result<AcpKycCaseStateSnapshot, AcpAdapterError> {
        acp::acp_discover_kyc_case_state(session, &self.manifest, case_id, response)
    }

    pub fn language_pack_for(
        &self,
        session: &AcpSession,
        request: UpdateStatusLanguagePackRequest,
    ) -> Result<SemOsLanguagePack, AcpAdapterError> {
        acp::acp_update_status_language_pack(session, &self.manifest, request)
    }

    pub fn kyc_language_pack_for(
        &self,
        session: &AcpSession,
        request: KycLanguagePackRequest,
    ) -> Result<SemOsLanguagePack, AcpAdapterError> {
        acp::acp_kyc_update_status_language_pack(session, &self.manifest, request)
    }

    pub fn kyc_language_loop_timed_for(
        &self,
        session: &AcpSession,
        request: KycLanguagePackRequest,
        draft: KycUpdateStatusWorkbookDraft,
    ) -> Result<AcpKycLanguageLoopTimedOutcome, AcpAdapterError> {
        acp::acp_run_kyc_update_status_language_loop_timed(session, &self.manifest, request, draft)
    }
}

/// Load the bundled `ob-poc.kyc` Domain Pack manifest from the
/// `rust/config/` tree. Single source of truth; previously duplicated
/// in `acp_protocol.rs` and `api::repl_routes_v2`.
pub fn load_ob_poc_kyc_domain_pack() -> Result<DomainPackManifest, AcpAdapterError> {
    serde_yaml::from_str(include_str!(
        "../../../config/sem_os_seeds/domain_packs/ob_poc_kyc.yaml"
    ))
    .map_err(|err| AcpAdapterError::PackInvalid {
        reason: err.to_string(),
    })
}
