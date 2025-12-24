# KYC Async Request Patterns

## Fire-and-Forget Pattern

KYC requests use a **fire-and-forget** pattern. You issue a request, move on immediately, and check state later.

```
WRONG: Wait for document, then continue
RIGHT: Request document → Continue with other work → Check state when needed
```

## Request Types

| Type | Subtype | From | Description |
|------|---------|------|-------------|
| DOCUMENT | PASSPORT, PROOF_OF_ADDRESS, BANK_STATEMENT, etc. | Client | Request document from client |
| VERIFICATION | IDENTITY, ADDRESS, SOURCE_OF_FUNDS | Third-party | External verification check |
| APPROVAL | SENIOR_REVIEW, COMPLIANCE_SIGN_OFF, MLRO_DECISION | Internal | Internal approval workflow |
| INFORMATION | UBO_QUESTIONNAIRE, TAX_RESIDENCY, BUSINESS_NATURE | Client | Information request |

## Creating Requests

Use these verbs to create async requests:

```clojure
;; Request document from client (common pattern)
(request.create-document
  :workstream-id @ws
  :subtype "PASSPORT"
  :due-days 7)

;; Request internal approval
(request.create-approval
  :workstream-id @ws
  :subtype "SENIOR_REVIEW"
  :approver-role "SENIOR_ANALYST")

;; Request external verification
(request.create-verification
  :workstream-id @ws
  :subtype "IDENTITY"
  :provider "ONFIDO")
```

After creating a request, **do NOT wait** - continue with other operations.

## Checking State

When you need to see what's pending, use the state query:

```clojure
;; Get case with embedded awaiting requests
(kyc-case.state :case-id @case)
```

The response shows requests as **child nodes of workstreams** in `awaiting` arrays:

```json
{
  "workstreams": [
    {
      "entity": {"name": "John Smith", "role": "DIRECTOR"},
      "status": "COLLECT",
      "awaiting": [
        {"type": "DOCUMENT", "subtype": "PASSPORT", "days_overdue": 3, "overdue": true},
        {"type": "DOCUMENT", "subtype": "PROOF_OF_ADDRESS", "days_overdue": 0, "overdue": false}
      ]
    }
  ],
  "summary": {"total_awaiting": 2, "overdue": 1},
  "attention": [{"entity": "John Smith", "issue": "PASSPORT overdue 3 days", "priority": "MEDIUM"}]
}
```

## Domain Coherence Principle

Requests are **NOT** a separate list. They are embedded in workstreams as `awaiting` arrays.
This is the "domain-coherent" view - one unified state model.

## Handling Overdue Items

When you see overdue items in the context:

| Days Overdue | Action |
|--------------|--------|
| 1-3 | Send reminder: `(request.remind :request-id ...)` |
| 4-7 | Escalate or extend: `(request.escalate ...)` or `(request.extend-due ...)` |
| 7+ | Waive (with justification) or block workstream |

## Auto-Fulfillment

Documents uploaded via `(document.catalog ...)` **automatically match** pending requests.
You don't need to manually link them - the system does request matching.

## Decision Patterns

When KYC case context shows issues:

1. **Overdue documents**: Ask user "Should I send reminders or extend deadlines?"
2. **Blocked workstreams**: Report the blocker and ask for guidance
3. **Complete workstreams with pending requests**: These are ready, requests are for record-keeping
4. **No issues**: Proceed with the user's request

## Example Workflow

```clojure
;; 1. Create workstream for entity
(entity-workstream.create :case-id @case :entity-id @director :as @ws)

;; 2. Request documents (fire-and-forget)
(request.create-document :workstream-id @ws :subtype "PASSPORT" :due-days 7)
(request.create-document :workstream-id @ws :subtype "PROOF_OF_ADDRESS" :due-days 7)

;; 3. Continue with other work immediately
(case-screening.run :workstream-id @ws :screening-type "SANCTIONS")
(case-screening.run :workstream-id @ws :screening-type "PEP")

;; 4. Later, check state to see what's still pending
(kyc-case.state :case-id @case)
```
