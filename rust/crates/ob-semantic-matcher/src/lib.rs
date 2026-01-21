//! Semantic Voice Command Matcher
//!
//! Uses Candle ML to embed voice transcripts and pgvector for similarity search.
//! Falls back to Double Metaphone phonetic matching for misheard words.
//!
//! # Architecture
//!
//! ```text
//! Voice Transcript
//!       │
//!       ▼
//! ┌─────────────────────────────────────────┐
//! │  Embedder (all-MiniLM-L6-v2)           │
//! │  "show me who owns this" → [384 dims]  │
//! └─────────────────────────────────────────┘
//!       │
//!       ▼
//! ┌─────────────────────────────────────────┐
//! │  pgvector Similarity Search             │
//! │  SELECT ... ORDER BY embedding <=> $1   │
//! │  → top-5 candidates with scores         │
//! └─────────────────────────────────────────┘
//!       │
//!       ├─── High confidence (>0.85) ───► Return match
//!       │
//!       ▼
//! ┌─────────────────────────────────────────┐
//! │  Phonetic Fallback (Double Metaphone)   │
//! │  "enhawnce" → ENNS → matches "enhance"  │
//! └─────────────────────────────────────────┘
//!       │
//!       ▼
//! ┌─────────────────────────────────────────┐
//! │  Feedback Capture (ML Learning Loop)    │
//! │  Capture → Analyze → Learn → Rebuild    │
//! └─────────────────────────────────────────┘
//! ```

pub mod embedder;
pub mod feedback;
pub mod matcher;
pub mod phonetic;
pub mod types;

pub use embedder::Embedder;
pub use matcher::SemanticMatcher;
pub use phonetic::PhoneticMatcher;
pub use types::*;

// Re-export key feedback types for convenience
pub use feedback::{
    AnalysisReport, FeedbackAnalyzer, FeedbackRepository, FeedbackService, InputSource,
    MatchConfidence, Outcome, PatternLearner, PipelineStatus, PromotableCandidate, PromotionReport,
    PromotionService, ReviewCandidate, WeeklyHealthMetrics,
};
