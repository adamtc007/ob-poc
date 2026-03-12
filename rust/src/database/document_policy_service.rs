//! Document Policy Service
//!
//! Runtime reader for published SemOS document-policy objects.

use anyhow::{Context, Result};
use sem_os_core::{
    evidence_strategy_def::EvidenceStrategyDefBody, proof_obligation_def::ProofObligationDefBody,
    requirement_profile_def::RequirementProfileDefBody,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
struct PublishedPolicyRow {
    snapshot_set_id: Uuid,
    snapshot_id: Uuid,
    fqn: String,
    payload: JsonValue,
}

/// Published SemOS requirement profile with snapshot provenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishedRequirementProfile {
    pub snapshot_set_id: Uuid,
    pub snapshot_id: Uuid,
    pub body: RequirementProfileDefBody,
}

/// Published SemOS proof obligation with snapshot provenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishedProofObligation {
    pub snapshot_set_id: Uuid,
    pub snapshot_id: Uuid,
    pub body: ProofObligationDefBody,
}

/// Published SemOS evidence strategy with snapshot provenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishedEvidenceStrategy {
    pub snapshot_set_id: Uuid,
    pub snapshot_id: Uuid,
    pub body: EvidenceStrategyDefBody,
}

/// Published document policy bundle for one active requirement profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveDocumentPolicyBundle {
    pub snapshot_set_id: Uuid,
    pub requirement_profile: PublishedRequirementProfile,
    pub proof_obligations: Vec<PublishedProofObligation>,
    pub evidence_strategies: Vec<PublishedEvidenceStrategy>,
}

/// Runtime service for published SemOS document-policy objects.
#[derive(Clone, Debug)]
pub struct DocumentPolicyService {
    pool: PgPool,
}

impl DocumentPolicyService {
    /// Create a new document policy service.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let service = DocumentPolicyService::new(pool.clone());
    /// ```
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get a reference to the underlying connection pool.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let pool = service.pool();
    /// ```
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Load the currently active SemOS snapshot set ID.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let active = service.get_active_snapshot_set_id().await?;
    /// ```
    pub async fn get_active_snapshot_set_id(&self) -> Result<Option<Uuid>> {
        sqlx::query_scalar(
            r#"
            SELECT active_snapshot_set_id
            FROM sem_reg_pub.active_snapshot_set
            LIMIT 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await
        .context("Failed to load active document policy snapshot set")
    }

    /// List all active requirement profiles from the currently published SemOS snapshot.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let profiles = service.list_active_requirement_profiles().await?;
    /// ```
    pub async fn list_active_requirement_profiles(
        &self,
    ) -> Result<Vec<PublishedRequirementProfile>> {
        let Some(snapshot_set_id) = self.get_active_snapshot_set_id().await? else {
            return Ok(Vec::new());
        };

        self.list_requirement_profiles_for_snapshot_set(snapshot_set_id)
            .await
    }

    /// List all active proof obligations from the currently published SemOS snapshot.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let obligations = service.list_active_proof_obligations().await?;
    /// ```
    pub async fn list_active_proof_obligations(&self) -> Result<Vec<PublishedProofObligation>> {
        let Some(snapshot_set_id) = self.get_active_snapshot_set_id().await? else {
            return Ok(Vec::new());
        };

        self.list_proof_obligations_for_snapshot_set(snapshot_set_id)
            .await
    }

    /// List all active evidence strategies from the currently published SemOS snapshot.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let strategies = service.list_active_evidence_strategies().await?;
    /// ```
    pub async fn list_active_evidence_strategies(&self) -> Result<Vec<PublishedEvidenceStrategy>> {
        let Some(snapshot_set_id) = self.get_active_snapshot_set_id().await? else {
            return Ok(Vec::new());
        };

        self.list_evidence_strategies_for_snapshot_set(snapshot_set_id)
            .await
    }

    /// Resolve one active requirement profile bundle with its referenced obligations and strategies.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let bundle = service.resolve_active_policy_bundle("doc.requirement_profile.kyc.entity").await?;
    /// ```
    pub async fn resolve_active_policy_bundle(
        &self,
        requirement_profile_fqn: &str,
    ) -> Result<Option<ActiveDocumentPolicyBundle>> {
        let Some(snapshot_set_id) = self.get_active_snapshot_set_id().await? else {
            return Ok(None);
        };

        let Some(requirement_profile) = self
            .get_requirement_profile_for_snapshot_set(snapshot_set_id, requirement_profile_fqn)
            .await?
        else {
            return Ok(None);
        };

        let all_obligations = self
            .list_proof_obligations_for_snapshot_set(snapshot_set_id)
            .await?;
        let all_strategies = self
            .list_evidence_strategies_for_snapshot_set(snapshot_set_id)
            .await?;

        let obligation_fqns = &requirement_profile.body.obligation_fqns;
        let proof_obligations: Vec<PublishedProofObligation> = all_obligations
            .into_iter()
            .filter(|obligation| obligation_fqns.contains(&obligation.body.fqn))
            .collect();

        let strategy_fqns: std::collections::BTreeSet<&str> = proof_obligations
            .iter()
            .flat_map(|obligation| {
                obligation
                    .body
                    .evidence_strategy_fqns
                    .iter()
                    .map(String::as_str)
            })
            .collect();

        let evidence_strategies: Vec<PublishedEvidenceStrategy> = all_strategies
            .into_iter()
            .filter(|strategy| strategy_fqns.contains(strategy.body.fqn.as_str()))
            .collect();

        Ok(Some(ActiveDocumentPolicyBundle {
            snapshot_set_id,
            requirement_profile,
            proof_obligations,
            evidence_strategies,
        }))
    }

    async fn list_requirement_profiles_for_snapshot_set(
        &self,
        snapshot_set_id: Uuid,
    ) -> Result<Vec<PublishedRequirementProfile>> {
        let rows = sqlx::query_as::<_, PublishedPolicyRow>(
            r#"
            SELECT snapshot_set_id, snapshot_id, fqn, payload
            FROM sem_reg_pub.active_requirement_profiles
            WHERE snapshot_set_id = $1
            ORDER BY fqn
            "#,
        )
        .bind(snapshot_set_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list active requirement profiles")?;

        rows.into_iter()
            .map(Self::decode_requirement_profile)
            .collect()
    }

    async fn get_requirement_profile_for_snapshot_set(
        &self,
        snapshot_set_id: Uuid,
        requirement_profile_fqn: &str,
    ) -> Result<Option<PublishedRequirementProfile>> {
        let row = sqlx::query_as::<_, PublishedPolicyRow>(
            r#"
            SELECT snapshot_set_id, snapshot_id, fqn, payload
            FROM sem_reg_pub.active_requirement_profiles
            WHERE snapshot_set_id = $1
              AND fqn = $2
            "#,
        )
        .bind(snapshot_set_id)
        .bind(requirement_profile_fqn)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get active requirement profile")?;

        row.map(Self::decode_requirement_profile).transpose()
    }

    async fn list_proof_obligations_for_snapshot_set(
        &self,
        snapshot_set_id: Uuid,
    ) -> Result<Vec<PublishedProofObligation>> {
        let rows = sqlx::query_as::<_, PublishedPolicyRow>(
            r#"
            SELECT snapshot_set_id, snapshot_id, fqn, payload
            FROM sem_reg_pub.active_proof_obligations
            WHERE snapshot_set_id = $1
            ORDER BY fqn
            "#,
        )
        .bind(snapshot_set_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list active proof obligations")?;

        rows.into_iter()
            .map(Self::decode_proof_obligation)
            .collect()
    }

    async fn list_evidence_strategies_for_snapshot_set(
        &self,
        snapshot_set_id: Uuid,
    ) -> Result<Vec<PublishedEvidenceStrategy>> {
        let rows = sqlx::query_as::<_, PublishedPolicyRow>(
            r#"
            SELECT snapshot_set_id, snapshot_id, fqn, payload
            FROM sem_reg_pub.active_evidence_strategies
            WHERE snapshot_set_id = $1
            ORDER BY fqn
            "#,
        )
        .bind(snapshot_set_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list active evidence strategies")?;

        rows.into_iter()
            .map(Self::decode_evidence_strategy)
            .collect()
    }

    fn decode_requirement_profile(row: PublishedPolicyRow) -> Result<PublishedRequirementProfile> {
        let body: RequirementProfileDefBody =
            serde_json::from_value(row.payload).with_context(|| {
                format!(
                    "Failed to decode requirement profile payload for {}",
                    row.fqn
                )
            })?;

        Ok(PublishedRequirementProfile {
            snapshot_set_id: row.snapshot_set_id,
            snapshot_id: row.snapshot_id,
            body,
        })
    }

    fn decode_proof_obligation(row: PublishedPolicyRow) -> Result<PublishedProofObligation> {
        let body: ProofObligationDefBody =
            serde_json::from_value(row.payload).with_context(|| {
                format!("Failed to decode proof obligation payload for {}", row.fqn)
            })?;

        Ok(PublishedProofObligation {
            snapshot_set_id: row.snapshot_set_id,
            snapshot_id: row.snapshot_id,
            body,
        })
    }

    fn decode_evidence_strategy(row: PublishedPolicyRow) -> Result<PublishedEvidenceStrategy> {
        let body: EvidenceStrategyDefBody =
            serde_json::from_value(row.payload).with_context(|| {
                format!("Failed to decode evidence strategy payload for {}", row.fqn)
            })?;

        Ok(PublishedEvidenceStrategy {
            snapshot_set_id: row.snapshot_set_id,
            snapshot_id: row.snapshot_id,
            body,
        })
    }
}
