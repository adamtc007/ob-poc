//! ob-poc impl of [`dsl_runtime::service_traits::TradingProfileDocument`].
//!
//! Bridges the plane-crossing trait (defined in `dsl-runtime`) to
//! [`crate::trading_profile::ast_db`]'s `load_document` /
//! `save_document` pair. The `AstDbError` raised by the in-crate
//! functions is converted to `anyhow::Error` at the boundary so the
//! trait surface stays free of ob-poc-internal error types.

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_types::trading_matrix::TradingMatrixDocument;
use sqlx::PgPool;
use uuid::Uuid;

use dsl_runtime::service_traits::TradingProfileDocument;

use crate::trading_profile::ast_db;

pub struct ObPocTradingProfileDocument {
    pool: PgPool,
}

impl ObPocTradingProfileDocument {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TradingProfileDocument for ObPocTradingProfileDocument {
    async fn load_document(&self, profile_id: Uuid) -> Result<TradingMatrixDocument> {
        ast_db::load_document(&self.pool, profile_id)
            .await
            .map_err(anyhow::Error::from)
    }

    async fn save_document(&self, profile_id: Uuid, doc: &TradingMatrixDocument) -> Result<()> {
        ast_db::save_document(&self.pool, profile_id, doc)
            .await
            .map_err(anyhow::Error::from)
    }
}
