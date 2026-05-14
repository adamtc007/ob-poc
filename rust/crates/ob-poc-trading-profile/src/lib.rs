//! ob-poc-trading-profile — Trading profile AST and resolver shapes.
//!
//! ## Capability claim
//!
//! Owns the trading-profile data plane: AST, builder, DB ops, resolver,
//! validator, document ops. The largest single business capability in
//! the workspace.
//!
//! ## Anti-charter
//!
//! - NOT the higher-level trading-profile orchestration in ob-poc.
//! - NOT TradingMatrix execution (matrix execution stays in ob-poc).
