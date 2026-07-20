//! Research module — slim remnant after T11.1b (2026-07-12, agent-tier
//! extraction).
//!
//! The real content (macro registry, executor, LLM client, definitions,
//! and the `companies_house`/`sec_edgar`/`traits`/`normalized`/`registry`
//! source-loader modules) moved to `ob-poc-agent::research` — see that
//! crate's doc for the full scope/exclusion rationale.
//!
//! What stays here: `sources::gleif`, because `GleifLoader` wraps
//! `GleifClient`, a real HTTP capability — `ob-poc-agent` cannot depend on
//! it directly (L1). Named as a T11.2 keyed-door target in the ownership
//! ledger's T11.1b entry, same class as `mcp::verb_search::HybridVerbSearcher`
//! and `sem_os_runtime::constellation_runtime`.
//!
//! Everything else is re-exported from `ob-poc-agent` so every existing
//! `crate::research::*` caller in `ob-poc` continues to resolve unchanged.

// `executor` re-exported as a module path (not just its flattened types)
// because `session::research_context`'s `#[cfg(test)]` module reaches
// `executor::SearchQuality` directly — invisible to a plain lib build,
// hence the allow.
#[allow(unused_imports)]
pub(crate) use ob_poc_agent::research::{
    executor, ApprovedResearch, ClaudeResearchClient, ResearchExecutor, ResearchMacroRegistry,
    ResearchResult, ReviewRequirement,
};

pub(crate) mod sources;
