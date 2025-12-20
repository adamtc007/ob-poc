# KYC Case Management

KYC case management provides workflow for client onboarding and periodic review.

## Case State Machine
```
INTAKE → DISCOVERY → ASSESSMENT → REVIEW → APPROVED/REJECTED
                                    ↓
                                 BLOCKED (if hard stops)
```

## Entity Workstream States
```
PENDING → COLLECT → VERIFY → SCREEN → ASSESS → COMPLETE
                                 ↓
                          ENHANCED_DD (if PEP/high-risk)
                              ↓
                           BLOCKED (if sanctions match)
```

## Key Tables (kyc schema)
- kyc.cases - Main KYC case for a CBU
- kyc.entity_workstreams - Per-entity work items
- kyc.red_flags - Risk indicators and issues
- kyc.doc_requests - Document requirements
- kyc.screenings - Sanctions/PEP/adverse media checks
- kyc.case_events - Audit trail

## DSL Verbs
- kyc-case.create, update-status, escalate, assign, close
- entity-workstream.create, update-status, block, complete
- red-flag.raise, mitigate, waive, dismiss
- doc-request.create, receive, verify, reject
- case-screening.run, complete, review-hit

## Observation Model
- client_allegations - Unverified claims (starting point)
- attribute_observations - Evidence from various sources
- observation_discrepancies - Conflicts requiring resolution

Read CLAUDE.md sections "KYC Case Management DSL" and "KYC Observation Model".
