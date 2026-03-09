# Sage Pipeline Fixes + LlmSage Implementation

**Codebase:** `ob-poc` (Rust, `rust/` directory)  
**Build command:** `RUSTC_WRAPPER= cargo check -p ob-poc`  
**Test command:** `RUSTC_WRAPPER= cargo test --lib -p ob-poc`  
**Sage tests:** `RUSTC_WRAPPER= cargo test --lib -p ob-poc -- sage`  

**Context:** The Sage/Coder pipeline exists in `rust/src/sage/`. It runs in shadow mode alongside the existing verb-first pipeline. Current accuracy is 4.48% (6/134) due to two problems: (A) the Coder returns `<none>` for 59/134 utterances (a wiring/threshold bug), and (B) the DeterministicSage gets the domain wrong 48% of the time (needs LLM classifier). This TODO fixes both.

**Do not modify** `rust/src/agent/orchestrator.rs` beyond the changes specified. Do not modify `rust/src/mcp/intent_pipeline.rs` or `rust/src/mcp/verb_search.rs`. The existing pipeline must remain functional.

**Build check after every phase.** If `cargo check -p ob-poc` fails, fix before continuing.

---

## Task 1: Debug and Fix Coder `<none>` Failures

**Problem:** 59 of 134 test utterances produce `sage_verb: <none>`. These include trivial cases like "create a new CBU for Allianz" and "show me all the CBUs" — verbs that obviously exist in the registry. The Coder's `VerbMetadataIndex` is either not finding candidates or the scoring threshold is too strict.

### Step 1.1: Add diagnostic logging to Coder resolution

**File:** `rust/src/sage/coder.rs`

In `generate_dsl_from_step()` (or the function that calls `resolve_verb()`), add `tracing::debug!` logging that prints:

```
Coder resolve: domain_concept={}, action={:?}, plane={:?}, polarity={:?}
  → query returned {} candidates
  → top candidate: {} (score={})
  → param_overlap_score: {} (step has {} params, verb has {} required)
```

This tells us WHERE in the chain the candidates are being lost.

### Step 1.2: Fix VerbMetadataIndex domain matching

**File:** `rust/src/sage/verb_index.rs`

The `query()` method filters by domain. The domain field on `VerbMeta` is the FQN prefix (e.g., `"cbu"` from `"cbu.create"`). The `domain_concept` on `OutcomeStep` comes from `extract_domain_hints()` which uses ECIR noun extraction.

**Likely bug:** The domain matching may be exact-string, but ECIR nouns don't always match verb domain prefixes. For example:
- ECIR extracts noun "fund" → but verbs are in domain "fund" (matches) AND "capital" (doesn't match)
- ECIR extracts noun "screening" → but some verbs are in domain "case-screening" (doesn't match)
- ECIR extracts nothing → domain_hints is empty → `query()` gets `None` for domain → what happens?

**Fix:** When `domain` parameter is `None` in `query()`, return ALL verbs for that plane+polarity combination (don't return empty). When domain is `Some`, also include verbs whose domain starts with the hint OR whose metadata tags contain the hint. Example:

```rust
fn matches_domain(&self, verb: &VerbMeta, domain_hint: Option<&str>) -> bool {
    let Some(hint) = domain_hint else { return true; }; // None = no filter
    if verb.domain == hint { return true; }
    if verb.domain.starts_with(hint) || hint.starts_with(&verb.domain) { return true; }
    // Also check metadata tags
    verb.action_tags.iter().any(|t| t == hint)
}
```

### Step 1.3: Lower Coder scoring threshold

**File:** `rust/src/sage/verb_resolve.rs`

Find the threshold where the Coder rejects candidates (returns `<none>` / error). The current threshold may be too high for the DeterministicSage which provides zero params.

**Fix:** When `step.resolved_params` is empty, use a lower acceptance threshold (0.3 instead of 0.5). The param_overlap_score is 0.0 when no params exist, so the total score ceiling is `0.6 * action_score`. A perfect action match gives 0.6 * 0.8 = 0.48 — which would be BELOW a 0.5 threshold. This is almost certainly the bug.

```rust
let threshold = if step.resolved_params.is_empty() { 0.25 } else { 0.5 };
```

### Step 1.4: Fix DeterministicSage domain_concept propagation

**File:** `rust/src/sage/deterministic.rs`

Check what `domain_concept` the DeterministicSage puts on the OutcomeStep. It should be the FIRST domain hint from `pre_classify()`. If `domain_hints` is empty, `domain_concept` should be `""` or `"unknown"` — and the Coder's `query()` should treat this as "no filter" (Step 1.2 fix).

Verify: for "create a new CBU for Allianz in Luxembourg", does `pre_classify()` produce `domain_hints: ["cbu"]`? If not, the ECIR noun_index isn't extracting "cbu" from this utterance — check that `noun_index.yaml` has "cbu" as an alias and that the NounIndex is being passed to `pre_classify()`.

### Step 1.5: Verify fix

Run: `RUSTC_WRAPPER= cargo test --lib -p ob-poc -- sage`

Then manually test with the server running:
```
POST /api/session/:id/input
{ "kind": "utterance", "message": "create a new CBU for Allianz in Luxembourg" }
```

Check server logs for the Sage shadow classification. The `sage_verb` field in the response should no longer be `<none>` for basic CBU/entity/fund operations.

**Acceptance:** The 59 `<none>` count should drop to <15. Most core domain utterances (cbu, entity, fund, deal, screening) should produce a verb candidate even if it's wrong.

---

## Task 2: Implement LlmSage

**Problem:** The DeterministicSage gets the domain right 64.8% of the time and extracts zero parameters. An LLM-backed Sage can identify the correct domain ~85%+ of the time AND extract parameters from the utterance, giving the Coder the `param_overlap_score` signal it needs.

### Step 2.1: Create LlmSage module

**File:** `rust/src/sage/llm_sage.rs` (new file)

```rust
use std::collections::BTreeMap;
use anyhow::Result;
use async_trait::async_trait;

use ob_agentic::{create_llm_client, LlmClient};

use super::outcome::*;
use super::plane::ObservationPlane;
use super::polarity::{classify_polarity, IntentPolarity};
use super::pre_classify::{pre_classify, SagePreClassification};
use super::SageEngine;
use super::context::SageContext;
use crate::mcp::noun_index::NounIndex;

pub struct LlmSage {
    noun_index: Option<std::sync::Arc<NounIndex>>,
}

impl LlmSage {
    pub fn new(noun_index: Option<std::sync::Arc<NounIndex>>) -> Self {
        Self { noun_index }
    }
}

#[async_trait]
impl SageEngine for LlmSage {
    async fn classify(
        &self,
        utterance: &str,
        context: &SageContext,
    ) -> Result<OutcomeIntent> {
        // 1. Deterministic pre-classification (same as DeterministicSage)
        let pre = pre_classify(
            utterance,
            context.stage_focus.as_deref(),
            self.noun_index.as_deref(),
            context.entity_kind.as_deref(),
            &context.goals,
        );

        // 2. Read+Structure fast path — no LLM needed
        if pre.sage_only && !pre.domain_hints.is_empty() {
            return Ok(build_structure_read_outcome(utterance, &pre));
        }

        // 3. LLM outcome classification
        let llm = create_llm_client()?;
        let prompt = build_sage_prompt(utterance, context, &pre);
        let response = llm.chat(&prompt.system, &prompt.user).await?;

        // 4. Parse LLM response
        let classified = parse_sage_response(&response, &pre)?;

        Ok(classified)
    }
}
```

### Step 2.2: The Sage LLM prompt

This is the critical design artifact. The prompt is pre-constrained by the three deterministic signals to reduce the classification space.

**System prompt:**

```
You are an outcome classifier for a custody banking onboarding platform.

Given a user utterance and context, identify what the user wants to achieve.
You are NOT selecting a function or verb. You are identifying the BUSINESS OUTCOME.

RULES:
1. Respond with ONLY valid JSON. No explanation, no markdown.
2. For "domain": pick from the DOMAIN LIST below, filtered by the pre-classification signals.
3. For "action": pick from the ACTION LIST below.
4. For "params": extract concrete values the user mentioned (names, codes, types, dates). Use the parameter name hints for the domain.
5. For "confidence": "high" if the outcome is unambiguous, "medium" if you're fairly sure, "low" if multiple outcomes could match.
6. Keep "summary" to one sentence describing the outcome in business terms.
```

**User prompt (template — variables filled at runtime):**

```
UTTERANCE: "{utterance}"

CONTEXT:
  Workflow: {stage_focus or "general"}
  Current entity: {dominant_entity_name or "none"}
  Entity type: {entity_kind or "unknown"}
  Recent actions: {last_intents summary or "none"}

PRE-CLASSIFICATION (already determined):
  Observation plane: {plane}
  Intent polarity: {polarity}
  Domain hints: {domain_hints}

DOMAIN LIST (for {polarity} operations):
{domain_list}

ACTION LIST (for {polarity} operations):
{action_list}

PARAMETER HINTS BY DOMAIN:
{param_hints}

Respond with JSON:
{
  "summary": "one sentence business outcome",
  "domain": "domain from list above",
  "action": "action from list above",
  "params": {"param_name": "extracted_value", ...},
  "confidence": "high|medium|low"
}
```

### Step 2.3: Domain and action lists filtered by polarity

These constants reduce the LLM's classification space.

**Read polarity domains:**
```
cbu, entity, fund, deal, document, screening, ubo, ownership, control,
gleif, bods, client-group, session, view, billing, sla, team,
registry, schema, agent
```

**Read polarity actions:**
```
investigate (browse, list, describe, query)
report (summarize, status, compute, count)
trace (navigate relationships, ownership chains)
assess-readonly (check status, review results — no state change)
```

**Write polarity domains:** (same list plus)
```
trading-profile, capital, movement, investor, contract, lifecycle,
service-resource, requirement, settlement-chain, kyc-case
```

**Write polarity actions:**
```
create (new entity, structure, case, record)
modify (update fields, change status, assign roles)
link (assign role, add relationship, bind)
remove (delete, cancel, terminate, revoke, end)
transfer (import, export, upload, move)
assess-mutating (run screening, execute check — creates results)
configure (set preferences, enable/disable, set thresholds)
verify (approve, reject, mark as verified)
```

### Step 2.4: Parameter hints by domain

These tell the LLM what to extract for each domain. Keep concise — the LLM only needs field names, not full schemas.

```
cbu: name, jurisdiction (ISO 2-letter), fund-entity-id, client-type, description
entity: name, entity-type (limited-company|proper-person|trust-discretionary|partnership-limited), jurisdiction
fund: name, fund-type (umbrella|subfund|share-class|standalone|master|feeder), parent-fund, jurisdiction
deal: deal-id, status, client
document: entity-name, document-type, file-reference
screening: entity-name, screening-type (sanctions|pep|adverse-media)
ubo: entity-name, ownership-percentage, relationship-type (ownership|control|trust-role)
ownership: entity-name, issuer, percentage
gleif: lei, entity-name, client-group
client-group: group-name, entity-name, role
session: target (cbu-name|client-name|deal-id), jurisdiction
view: target, level (universe|galaxy|system|planet)
```

### Step 2.5: Parse LLM response into OutcomeIntent

**File:** `rust/src/sage/llm_sage.rs` — add:

```rust
fn parse_sage_response(
    response: &str,
    pre: &SagePreClassification,
) -> Result<OutcomeIntent> {
    // Strip markdown code fences if present
    let json_str = extract_json(response);
    let v: serde_json::Value = serde_json::from_str(json_str)?;

    let domain = v["domain"].as_str().unwrap_or("unknown").to_string();
    let action_str = v["action"].as_str().unwrap_or("investigate");
    let action = parse_outcome_action(action_str);
    let confidence_str = v["confidence"].as_str().unwrap_or("low");
    let summary = v["summary"].as_str().unwrap_or("").to_string();

    // Extract params
    let mut params = BTreeMap::new();
    if let Some(obj) = v["params"].as_object() {
        for (k, val) in obj {
            if let Some(s) = val.as_str() {
                if !s.is_empty() {
                    params.insert(k.clone(), ResolvedParam::String(s.to_string()));
                }
            }
        }
    }

    // Apply asymmetric confidence (v0.3 design principle)
    let raw_confidence = parse_confidence(confidence_str);
    let confidence = apply_asymmetric_risk(raw_confidence, pre.polarity);

    let step = OutcomeStep {
        description: summary.clone(),
        plane: pre.plane,
        domain_concept: domain.clone(),
        action: action.clone(),
        resolved_params: params,
        requires_confirmation: matches!(pre.polarity, IntentPolarity::Write),
        depends_on: vec![],
        execution_mode: if pre.sage_only {
            ExecutionMode::Research
        } else {
            ExecutionMode::Execute
        },
    };

    Ok(OutcomeIntent {
        summary,
        plane: pre.plane,
        polarity: pre.polarity,
        domain_concept: domain,
        action,
        subject: None, // Entity linking happens separately
        steps: vec![step],
        confidence,
        pending_clarifications: vec![],
    })
}

/// Read operations get confidence bumped up (low risk).
/// Write operations get confidence capped down (high risk).
fn apply_asymmetric_risk(
    raw: SageConfidence,
    polarity: IntentPolarity,
) -> SageConfidence {
    match polarity {
        IntentPolarity::Read => match raw {
            SageConfidence::Low => SageConfidence::Medium,    // Reads: low→medium
            other => other,                                     // High/Medium stay
        },
        IntentPolarity::Write => match raw {
            SageConfidence::High => SageConfidence::High,      // Only keep high if LLM is sure
            _ => SageConfidence::Medium,                        // Everything else→medium (confirm)
        },
        IntentPolarity::Ambiguous => raw, // No adjustment
    }
}
```

### Step 2.6: Wire LlmSage into agent_service

**File:** `rust/src/api/agent_service.rs`

Find where `DeterministicSage` is constructed in `build_orchestrator_context()`. Add feature-flag switch:

```rust
let sage: std::sync::Arc<dyn crate::sage::SageEngine> = 
    if std::env::var("SAGE_LLM").unwrap_or_default() == "1" {
        std::sync::Arc::new(crate::sage::llm_sage::LlmSage::new(
            self.noun_index.clone(),
        ))
    } else {
        std::sync::Arc::new(crate::sage::deterministic::DeterministicSage::new(
            self.noun_index.clone(),
        ))
    };
```

### Step 2.7: Register module

**File:** `rust/src/sage/mod.rs` — add `pub mod llm_sage;`

### Step 2.8: Unit tests

**File:** `rust/src/sage/llm_sage.rs` — add `#[cfg(test)]` module:

```rust
#[test]
fn parse_sage_response_extracts_fields() {
    let response = r#"{"summary":"Create a CBU for Allianz in Luxembourg","domain":"cbu","action":"create","params":{"name":"Allianz Global Investors","jurisdiction":"LU"},"confidence":"high"}"#;
    let pre = SagePreClassification {
        plane: ObservationPlane::Instance,
        polarity: IntentPolarity::Write,
        domain_hints: vec!["cbu".into()],
        clue_word: Some("create".into()),
        sage_only: false,
    };
    let result = parse_sage_response(response, &pre).unwrap();
    assert_eq!(result.domain_concept, "cbu");
    assert_eq!(result.steps[0].resolved_params.len(), 2);
    assert!(result.steps[0].resolved_params.contains_key("name"));
    assert!(result.steps[0].resolved_params.contains_key("jurisdiction"));
}

#[test]
fn asymmetric_risk_bumps_read_confidence() {
    assert!(matches!(
        apply_asymmetric_risk(SageConfidence::Low, IntentPolarity::Read),
        SageConfidence::Medium
    ));
}

#[test]
fn asymmetric_risk_caps_write_confidence() {
    assert!(matches!(
        apply_asymmetric_risk(SageConfidence::Low, IntentPolarity::Write),
        SageConfidence::Medium  // not Low — capped to medium for confirmation
    ));
}
```

**Acceptance:** `cargo check -p ob-poc` passes. `cargo test --lib -p ob-poc -- sage` passes. Server starts with `SAGE_LLM=1` and processes utterances through the LLM classifier.

---

## Task 3: Run Comparative Coverage with LlmSage

After Tasks 1 and 2 are complete:

### Step 3.1: Start server with LlmSage enabled

```bash
cd rust
SAGE_LLM=1 DATABASE_URL=postgresql:///data_designer OBPOC_ALLOW_RAW_EXECUTE=1 cargo run -p ob-poc-web
```

### Step 3.2: Run comparative harness

```bash
RUSTC_WRAPPER= cargo test -p ob-poc --test utterance_api_coverage -- --ignored --nocapture
```

### Step 3.3: Record results

Save the output. The key numbers to capture:
- Existing pipeline accuracy (should be ~43%)
- Sage+Coder accuracy (target: >30%, stretch: >50%)
- Sage wins vs Pipeline wins vs Both right vs Both wrong
- `<none>` count (target: <10)

### Step 3.4: Domain accuracy improvement

Run sage coverage harness too:
```bash
SAGE_LLM=1 RUSTC_WRAPPER= cargo test -p ob-poc --test sage_coverage -- --ignored --nocapture
```

Compare domain accuracy to the 64.8% baseline. Target: >80%.

---

## File Summary

| File | Action | Task |
|------|--------|------|
| `rust/src/sage/coder.rs` | Modify — add diagnostic logging | 1.1 |
| `rust/src/sage/verb_index.rs` | Modify — fix domain matching, handle None | 1.2 |
| `rust/src/sage/verb_resolve.rs` | Modify — lower threshold for no-param cases | 1.3 |
| `rust/src/sage/deterministic.rs` | Modify — verify domain_concept propagation | 1.4 |
| `rust/src/sage/llm_sage.rs` | Create — full LlmSage implementation | 2.1-2.5, 2.8 |
| `rust/src/sage/mod.rs` | Modify — add `pub mod llm_sage` | 2.7 |
| `rust/src/api/agent_service.rs` | Modify — feature flag for LlmSage | 2.6 |

## Key Constraints

- The `ob_agentic` crate provides `create_llm_client()` and `LlmClient` trait with `async fn chat(&self, system: &str, user: &str) -> Result<String>`. Use this for the LLM call. Do NOT add new LLM client dependencies.
- The `NounIndex` type is in `rust/src/mcp/noun_index.rs`. Import it, do not copy it.
- All `sage::*` types (`OutcomeIntent`, `OutcomeStep`, `SagePreClassification`, etc.) are in `rust/src/sage/`. Check existing types before creating new ones.
- The `ResolvedParam` type should already exist in `rust/src/sage/outcome.rs`. If it doesn't, create it as: `pub enum ResolvedParam { String(String), Uuid(String), Number(f64), Boolean(bool) }`.
- JSON parsing: use `serde_json`. Strip markdown code fences (` ```json ... ``` `) before parsing — LLMs often wrap JSON in code blocks.
