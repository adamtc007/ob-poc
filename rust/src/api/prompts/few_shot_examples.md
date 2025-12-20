# Few-Shot Examples for Intent Extraction

## Core Patterns

### 1. Simple Entity Creation

**User**: "Create a fund called Apex Capital in Luxembourg"
```json
{
  "intents": [{
    "verb": "cbu.ensure",
    "params": {"name": "Apex Capital", "jurisdiction": "LU", "client-type": "fund"},
    "refs": {},
    "lookups": null
  }],
  "explanation": "Creating a new fund CBU named 'Apex Capital' domiciled in Luxembourg.",
  "confidence": 0.95
}
```

### 2. Using Session Context (@cbu)

**User**: "Add custody product" (with @cbu in session)
```json
{
  "intents": [{
    "verb": "cbu.add-product",
    "params": {"product": "CUSTODY"},
    "refs": {"cbu-id": "@cbu"},
    "lookups": null
  }],
  "explanation": "Adding Custody product to the active CBU.",
  "confidence": 0.95
}
```

### 3. Entity Lookup Required

**User**: "Add John Smith as director of Apex Capital"
```json
{
  "intents": [
    {
      "verb": "entity.create-proper-person",
      "params": {"first-name": "John", "last-name": "Smith"},
      "refs": {},
      "lookups": null
    },
    {
      "verb": "cbu.assign-role",
      "params": {"role": "DIRECTOR"},
      "refs": {"entity-id": "@result_1"},
      "lookups": {"cbu-id": {"search_text": "Apex Capital", "entity_type": "cbu"}}
    }
  ],
  "explanation": "Creating person 'John Smith' and assigning as DIRECTOR of Apex Capital CBU.",
  "confidence": 0.90
}
```

### 4. Referencing Existing Entity by Name

**User**: "Make Sarah Johnson the compliance officer" (with @cbu in session)
```json
{
  "intents": [{
    "verb": "cbu.assign-role",
    "params": {"role": "COMPLIANCE_OFFICER"},
    "refs": {"cbu-id": "@cbu"},
    "lookups": {"entity-id": {"search_text": "Sarah Johnson", "entity_type": "person"}}
  }],
  "explanation": "Assigning existing person 'Sarah Johnson' as COMPLIANCE_OFFICER.",
  "confidence": 0.85
}
```

### 5. Multi-Step Sequence with References

**User**: "Create a company called Holdings Ltd in Jersey and add it as shareholder"
```json
{
  "intents": [
    {
      "verb": "entity.create-limited-company",
      "params": {"name": "Holdings Ltd", "jurisdiction": "JE"},
      "refs": {},
      "lookups": null
    },
    {
      "verb": "cbu.assign-role",
      "params": {"role": "SHAREHOLDER"},
      "refs": {"cbu-id": "@cbu", "entity-id": "@result_1"},
      "lookups": null
    }
  ],
  "explanation": "Creating 'Holdings Ltd' company in Jersey, then assigning as SHAREHOLDER of the active CBU.",
  "confidence": 0.90
}
```

## Removal Operations

### 6. Remove Product

**User**: "Remove fund accounting" (with @cbu in session)
```json
{
  "intents": [{
    "verb": "cbu.remove-product",
    "params": {"product": "FUND_ACCOUNTING"},
    "refs": {"cbu-id": "@cbu"},
    "lookups": null
  }],
  "explanation": "Removing FUND_ACCOUNTING product from the active CBU.",
  "confidence": 0.95
}
```

### 7. Delete/Unlink Variations

**User**: "Delete the custody product"
```json
{
  "intents": [{
    "verb": "cbu.remove-product",
    "params": {"product": "CUSTODY"},
    "refs": {"cbu-id": "@cbu"},
    "lookups": null
  }],
  "explanation": "Removing CUSTODY product.",
  "confidence": 0.95
}
```

**User**: "Take John off as director"
```json
{
  "intents": [{
    "verb": "cbu.remove-role",
    "params": {"role": "DIRECTOR"},
    "refs": {"cbu-id": "@cbu"},
    "lookups": {"entity-id": {"search_text": "John", "entity_type": "person"}}
  }],
  "explanation": "Removing DIRECTOR role from person 'John'.",
  "confidence": 0.85
}
```

## Ownership & UBO

### 8. Add Ownership

**User**: "John Smith owns 60% of the fund"
```json
{
  "intents": [{
    "verb": "ubo.add-ownership",
    "params": {"percentage": 60, "ownership-type": "DIRECT"},
    "refs": {"owned-entity-id": "@cbu"},
    "lookups": {"owner-entity-id": {"search_text": "John Smith", "entity_type": "person"}}
  }],
  "explanation": "Recording 60% direct ownership by John Smith.",
  "confidence": 0.90
}
```

### 9. Register UBO

**User**: "Register Sarah as beneficial owner with 25% ownership"
```json
{
  "intents": [{
    "verb": "ubo.register-ubo",
    "params": {"qualifying-reason": "OWNERSHIP_25PCT", "ownership-percentage": 25},
    "refs": {"cbu-id": "@cbu"},
    "lookups": {
      "subject-entity-id": {"search_text": "@cbu", "entity_type": "entity"},
      "ubo-person-id": {"search_text": "Sarah", "entity_type": "person"}
    }
  }],
  "explanation": "Registering Sarah as UBO with 25% ownership qualification.",
  "confidence": 0.85
}
```

## Clarification Needed

### 10. Ambiguous Name

**User**: "Create Apex Reg LU fund"
```json
{
  "needs_clarification": true,
  "clarification": {
    "ambiguity_type": "name_parsing",
    "original_text": "Apex Reg LU",
    "interpretations": [
      {"option": 1, "interpretation": "name='Apex Reg LU'", "description": "The entire text is the fund name"},
      {"option": 2, "interpretation": "name='Apex Reg', jurisdiction='LU'", "description": "'Reg' is part of the name, 'LU' is Luxembourg"},
      {"option": 3, "interpretation": "name='Apex', jurisdiction='LU'", "description": "'Reg' means registered in Luxembourg"}
    ],
    "question": "How should I parse this? Is 'Reg LU' part of the name, or does 'LU' indicate Luxembourg?"
  },
  "intents": [],
  "explanation": "The input is ambiguous - need clarification on name boundaries.",
  "confidence": 0.3
}
```

### 11. Missing Context

**User**: "Add the director"
```json
{
  "needs_clarification": true,
  "clarification": {
    "ambiguity_type": "missing_context",
    "original_text": "the director",
    "interpretations": [],
    "question": "Which person should I add as director? Please provide their name."
  },
  "intents": [],
  "explanation": "Need to know who to add as director.",
  "confidence": 0.2
}
```

### 12. Multiple Interpretations

**User**: "Add custody and fund accounting"
```json
{
  "intents": [
    {
      "verb": "cbu.add-product",
      "params": {"product": "CUSTODY"},
      "refs": {"cbu-id": "@cbu"},
      "lookups": null
    },
    {
      "verb": "cbu.add-product",
      "params": {"product": "FUND_ACCOUNTING"},
      "refs": {"cbu-id": "@cbu"},
      "lookups": null
    }
  ],
  "explanation": "Adding both CUSTODY and FUND_ACCOUNTING products to the active CBU.",
  "confidence": 0.95
}
```

## Edge Cases

### 13. Not Supported

**User**: "What's the weather like?"
```json
{
  "intents": [],
  "explanation": "This request is not related to KYC/AML onboarding operations. I can help with creating CBUs, adding entities, assigning roles, and managing products.",
  "confidence": 1.0
}
```

### 14. Partial Information - Proceed with Defaults

**User**: "Create a fund called Test" (no jurisdiction)
```json
{
  "needs_clarification": true,
  "clarification": {
    "ambiguity_type": "missing_context",
    "original_text": "Create a fund called Test",
    "interpretations": [],
    "question": "Which jurisdiction should the fund be domiciled in? (e.g., LU, IE, GB, US)"
  },
  "intents": [],
  "explanation": "Jurisdiction is required for fund creation.",
  "confidence": 0.4
}
```

## Confidence Scoring Guide

| Confidence | When to Use |
|------------|-------------|
| 0.95-1.0 | Unambiguous request with all required info, clear verb match |
| 0.85-0.94 | Clear intent but requires entity lookup that might fail |
| 0.70-0.84 | Some inference required, minor assumptions made |
| 0.50-0.69 | Significant assumptions, should ask for confirmation |
| 0.30-0.49 | Multiple interpretations, ask for clarification |
| 0.0-0.29 | Very unclear, definitely ask for clarification |
