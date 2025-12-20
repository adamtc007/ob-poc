# Adversarial Verification Model

The verification module implements a "Trust But Verify → Distrust And Verify" model where every piece of information is a CLAIM that must be VERIFIED.

## Core Principle
Standard: "Would this process catch a sophisticated liar?"

## Key Components
- rust/src/verification/types.rs - Claim, Evidence, Challenge types
- rust/src/verification/confidence.rs - ConfidenceCalculator with source weighting
- rust/src/verification/patterns.rs - PatternDetector (circular ownership, layering)
- rust/src/verification/evasion.rs - EvasionDetector (behavioral analysis)
- rust/src/dsl_v2/custom_ops/verify_ops.rs - DSL verb handlers

## Database Tables
- ob-poc.verification_challenges - Challenge/response workflow
- ob-poc.verification_escalations - Risk-based escalation routing
- ob-poc.detected_patterns - Pattern detection audit trail

## DSL Verbs (verify.* domain)
- verify.detect-patterns - Run adversarial pattern detection
- verify.detect-evasion - Analyze doc_request history for evasion signals
- verify.challenge - Raise formal challenge requiring response
- verify.calculate-confidence - Aggregate confidence across observations
- verify.assert - Declarative gate for confidence thresholds

## Confidence Thresholds
- 0.80+ = verified (high confidence)
- 0.60-0.80 = provisional (acceptable with caveats)
- 0.40-0.60 = suspect (requires investigation)
- <0.40 = rejected (insufficient evidence)

Read CLAUDE.md section on "KYC Observation Model" for evidence-based verification.
