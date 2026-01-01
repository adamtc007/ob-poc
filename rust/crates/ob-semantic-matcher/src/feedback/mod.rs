//! Intent feedback capture for ML continuous learning
//!
//! This module provides:
//! - Capture of user interactions with intent matching
//! - Outcome tracking (executed, corrected, abandoned)
//! - Batch analysis for pattern discovery
//! - Input sanitization for privacy
//! - Automatic pattern learning from feedback
//!
//! ## Architecture
//!
//! ```text
//! User Input → Matcher → Result
//!                 │
//!                 ├── INSERT intent_feedback (match data)
//!                 │
//!           [user action]
//!                 │
//!                 └── UPDATE intent_feedback (outcome)
//!
//!                         │
//!                         │ (batch job)
//!                         ▼
//!
//! Analysis → Pattern Discovery → Auto-Apply → Rebuild Embeddings
//! ```

mod analysis;
mod learner;
mod repository;
mod sanitize;
mod service;
mod types;

pub use analysis::{AnalysisReport, FeedbackAnalyzer};
pub use learner::PatternLearner;
pub use repository::FeedbackRepository;
pub use sanitize::sanitize_input;
pub use service::FeedbackService;
pub use types::*;
