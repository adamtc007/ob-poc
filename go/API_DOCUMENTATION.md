# Multi-Domain DSL System API Documentation

## Overview

The Multi-Domain DSL System provides a RESTful API for managing and interacting with multiple business domains (hedge-fund-investor, onboarding, etc.) through a unified interface. The system uses intelligent routing to automatically direct requests to the appropriate domain based on content analysis.

## Base URL

```
http://localhost:8080/api
```

## Authentication

Currently, no authentication is required. API key authentication may be added in future versions.

## Content Types

All API endpoints accept and return JSON unless otherwise specified.

```http
Content-Type: application/json
Accept: application/json
```

---

## Health & Monitoring Endpoints

### GET /health

Returns the overall health status of the multi-domain system.

**Response:**
```json
{
  "status": "healthy",
  "service": "multi-domain-dsl-agent",
  "registry_healthy": true,
  "domains": 2,
  "time": "2025-11-04T16:16:28.100401Z"
}
```

**Status Codes:**
- `200 OK` - System is healthy
- `503 Service Unavailable` - System is degraded

---

## Domain Management Endpoints

### GET /domains

Returns information about all available domains in the system.

**Response:**
```json
{
  "domains": {
    "hedge-fund-investor": {
      "name": "hedge-fund-investor",
      "version": "1.0.0",
      "description": "Hedge fund investor lifecycle management from opportunity to offboarding",
      "is_healthy": true,
      "verb_count": 17,
      "categories": {
        "kyc": 5,
        "opportunity": 2,
        "subscription": 4,
        "redemption": 2,
        "tax-banking": 2,
        "monitoring": 1,
        "offboarding": 1
      }
    },
    "onboarding": {
      "name": "onboarding",
      "version": "1.0.0",
      "description": "Client onboarding and case management",
      "is_healthy": true,
      "verb_count": 54,
      "categories": {
        "case-management": 5,
        "entity-identity": 5,
        "product-service": 5,
        "kyc-compliance": 6,
        "resource-infrastructure": 5,
        "attribute-data": 5,
        "workflow-state": 5,
        "notification-communication": 4,
        "integration-external": 4,
        "temporal-scheduling": 3,
        "risk-monitoring": 3,
        "data-lifecycle": 4
      }
    }
  },
  "total": 2
}
```

### GET /domains/{domain}/vocabulary

Returns the complete vocabulary for a specific domain.

**Parameters:**
- `domain` (path) - Domain name (e.g., "hedge-fund-investor", "onboarding")

**Example Request:**
```http
GET /api/domains/hedge-fund-investor/vocabulary
```

**Response:**
```json
{
  "domain": "hedge-fund-investor",
  "version": "1.0.0",
  "description": "Hedge fund investor lifecycle management from opportunity to offboarding",
  "verbs": {
    "investor.start-opportunity": {
      "name": "investor.start-opportunity",
      "category": "opportunity",
      "version": "1.0.0",
      "description": "Create initial investor record and start opportunity tracking",
      "arguments": {
        "legal-name": {
          "name": "legal-name",
          "type": "STRING",
          "required": true,
          "description": "Legal name of the investor"
        },
        "type": {
          "name": "type",
          "type": "ENUM",
          "required": true,
          "description": "Type of investor",
          "enum_values": ["PROPER_PERSON", "CORPORATE", "TRUST", "FOHF"]
        },
        "domicile": {
          "name": "domicile",
          "type": "STRING",
          "required": false,
          "description": "Investor domicile (ISO country code)"
        }
      },
      "state_transition": {
        "to_state": "OPPORTUNITY",
        "from_states": [""],
        "conditional": false
      },
      "idempotent": false,
      "examples": [
        "(investor.start-opportunity :legal-name \"Acme Capital LP\" :type \"CORPORATE\" :domicile \"CH\")"
      ]
    }
  },
  "categories": {
    "opportunity": {
      "name": "opportunity",
      "description": "Investor opportunity management",
      "verbs": ["investor.start-opportunity", "investor.qualify"]
    }
  },
  "states": ["OPPORTUNITY", "PRECHECKS", "KYC_PENDING", "KYC_APPROVED", "SUB_PENDING_CASH", "FUNDED_PENDING_NAV", "ISSUED", "ACTIVE", "REDEEM_PENDING", "REDEEMED", "OFFBOARDED"]
}
```

**Status Codes:**
- `200 OK` - Vocabulary returned successfully
- `404 Not Found` - Domain not found

### GET /vocabulary

Returns vocabularies for all domains in a single response.

**Response:**
```json
{
  "hedge-fund-investor": { /* complete vocabulary */ },
  "onboarding": { /* complete vocabulary */ }
}
```

---

## Chat & Interaction Endpoints

### POST /chat

Processes a natural language message and generates appropriate DSL using intelligent domain routing.

**Request Body:**
```json
{
  "session_id": "optional-session-id",
  "message": "Create investor opportunity for Acme Capital LP",
  "domain": "hedge-fund-investor",
  "context": {
    "key": "value"
  }
}
```

**Parameters:**
- `session_id` (optional) - Session identifier. If not provided, a new session is created
- `message` (required) - Natural language instruction
- `domain` (optional) - Preferred domain. If not specified, router will select automatically
- `context` (optional) - Additional context for the request

**Response:**
```json
{
  "session_id": "uuid-session-id",
  "message": "Created investor opportunity for Acme Capital LP as a corporate investor domiciled in Switzerland.",
  "dsl": "(investor.start-opportunity\n  :legal-name \"Acme Capital LP\"\n  :type \"CORPORATE\"\n  :domicile \"CH\")",
  "fragment": "(investor.start-opportunity\n  :legal-name \"Acme Capital LP\"\n  :type \"CORPORATE\"\n  :domicile \"CH\")",
  "domain": "hedge-fund-investor",
  "response": {
    "verb": "investor.start-opportunity",
    "parameters": {
      "legal-name": "Acme Capital LP",
      "type": "CORPORATE",
      "domicile": "CH"
    },
    "from_state": "",
    "to_state": "OPPORTUNITY",
    "is_valid": true,
    "confidence": 0.95,
    "explanation": "Created investor opportunity for Acme Capital LP as a corporate investor domiciled in Switzerland.",
    "next_steps": ["Start KYC process", "Collect required documents"],
    "generation_time": "150ms",
    "timestamp": "2025-11-04T16:30:00Z"
  }
}
```

**Status Codes:**
- `200 OK` - Message processed successfully
- `400 Bad Request` - Invalid request format
- `500 Internal Server Error` - Processing failed

---

## DSL Operations

### POST /dsl/generate

Generates DSL from a natural language instruction without chat context.

**Request Body:**
```json
{
  "instruction": "Create case for CBU-1234",
  "session_id": "optional-session-id",
  "current_domain": "onboarding",
  "context": {},
  "existing_dsl": "previous DSL if any"
}
```

**Response:**
```json
{
  "dsl": "(case.create (cbu.id \"CBU-1234\"))",
  "verb": "case.create",
  "parameters": {
    "cbu.id": "CBU-1234"
  },
  "from_state": "",
  "to_state": "CREATE",
  "is_valid": true,
  "confidence": 0.88,
  "explanation": "Created new onboarding case for CBU-1234"
}
```

### POST /dsl/validate

Validates a DSL string using domain-specific validation rules.

**Request Body:**
```json
{
  "dsl": "(investor.start-opportunity :legal-name \"Test\")",
  "domain": "hedge-fund-investor"
}
```

**Response:**
```json
{
  "valid": true,
  "domain": "hedge-fund-investor",
  "errors": []
}
```

**Invalid DSL Response:**
```json
{
  "valid": false,
  "domain": "hedge-fund-investor",
  "errors": [
    "Missing required argument: type",
    "Invalid verb: investor.invalid-verb"
  ]
}
```

### POST /dsl/execute

Executes a validated DSL operation (placeholder for future implementation).

**Request Body:**
```json
{
  "dsl": "(investor.start-opportunity :legal-name \"Test\" :type \"PROPER_PERSON\")",
  "domain": "hedge-fund-investor",
  "session_id": "optional-session-id"
}
```

**Response:**
```json
{
  "status": "success",
  "message": "DSL execution pending implementation",
  "domain": "hedge-fund-investor"
}
```

---

## Session Management

### GET /session/{id}

Retrieves information about a specific chat session.

**Parameters:**
- `id` (path) - Session identifier

**Response:**
```json
{
  "session_id": "uuid-session-id",
  "current_domain": "hedge-fund-investor",
  "context": {
    "investor_id": "uuid-investor-id",
    "state": "OPPORTUNITY"
  },
  "built_dsl": "complete accumulated DSL",
  "history": [
    {
      "role": "user",
      "content": "Create investor opportunity",
      "timestamp": "2025-11-04T16:25:00Z"
    },
    {
      "role": "agent",
      "content": "Created investor opportunity successfully",
      "dsl": "complete accumulated DSL",
      "fragment": "individual operation DSL",
      "domain": "hedge-fund-investor",
      "timestamp": "2025-11-04T16:25:01Z"
    }
  ],
  "created_at": "2025-11-04T16:25:00Z",
  "last_used": "2025-11-04T16:25:01Z"
}
```

**Status Codes:**
- `200 OK` - Session found
- `404 Not Found` - Session not found

### GET /session/{id}/history

Retrieves the chat history for a specific session.

**Response:**
```json
[
  {
    "role": "user",
    "content": "Create investor opportunity for Acme Corp",
    "domain": "hedge-fund-investor",
    "timestamp": "2025-11-04T16:25:00Z"
  },
  {
    "role": "agent",
    "content": "Created investor opportunity successfully",
    "dsl": "complete accumulated DSL",
    "fragment": "individual operation DSL",
    "domain": "hedge-fund-investor",
    "response": { /* full generation response */ },
    "timestamp": "2025-11-04T16:25:01Z"
  }
]
```

---

## Routing & Metrics

### GET /routing/metrics

Returns routing statistics and performance metrics.

**Response:**
```json
{
  "total_requests": 1542,
  "strategy_usage": {
    "EXPLICIT": 45,
    "CONTEXT": 230,
    "VERB": 180,
    "KEYWORD": 890,
    "DEFAULT": 150,
    "FALLBACK": 47
  },
  "domain_selections": {
    "hedge-fund-investor": 892,
    "onboarding": 650
  },
  "average_confidence": 0.87,
  "average_response_time": "25ms",
  "failed_routings": 12,
  "last_updated": "2025-11-04T16:30:15Z"
}
```

### GET /attributes

Returns available attributes from the data dictionary (placeholder).

**Response:**
```json
{
  "attributes": [
    {
      "id": "uuid-0001",
      "name": "hf.investor.legal-name",
      "type": "string"
    },
    {
      "id": "uuid-0002",
      "name": "hf.investor.type",
      "type": "enum"
    }
  ]
}
```

---

## WebSocket API

### WebSocket Connection: /ws

Establishes a real-time WebSocket connection for interactive chat.

**Connection URL:**
```
ws://localhost:8080/ws
```

**Welcome Message:**
```json
{
  "type": "welcome",
  "payload": {
    "session_id": "uuid-session-id",
    "current_domain": "hedge-fund-investor",
    "message": "Connected to Multi-Domain DSL Agent. How can I help you?",
    "available_domains": ["hedge-fund-investor", "onboarding"]
  }
}
```

### WebSocket Message Types

#### Chat Message
**Client → Server:**
```json
{
  "type": "chat",
  "payload": {
    "message": "Create investor opportunity for ABC Corp",
    "context": {}
  }
}
```

**Server → Client:**
```json
{
  "type": "chat_response",
  "payload": {
    "message": "Created investor opportunity successfully",
    "dsl": "complete accumulated DSL",
    "fragment": "individual operation DSL",
    "domain": "hedge-fund-investor",
    "verb": "investor.start-opportunity",
    "from_state": "",
    "to_state": "OPPORTUNITY",
    "confidence": 0.95,
    "routing_reason": "Detected investor keywords in message",
    "response": { /* full generation response */ }
  }
}
```

#### Domain Switch
**Client → Server:**
```json
{
  "type": "switch_domain",
  "payload": {
    "domain": "onboarding"
  }
}
```

**Server → Client:**
```json
{
  "type": "domain_switched",
  "payload": {
    "domain": "onboarding",
    "message": "Switched to onboarding domain"
  }
}
```

#### Ping/Pong
**Client → Server:**
```json
{
  "type": "ping",
  "payload": {}
}
```

**Server → Client:**
```json
{
  "type": "pong",
  "payload": {
    "time": "2025-11-04T16:30:00Z"
  }
}
```

#### Error Messages
**Server → Client:**
```json
{
  "type": "error",
  "payload": {
    "error": "Domain not found: invalid-domain"
  }
}
```

---

## Domain Routing Logic

The system uses intelligent routing to automatically select the appropriate domain based on message content. The routing strategies are applied in order of priority:

### 1. Explicit Domain Switch
Messages containing explicit domain switch commands:
- `"switch to onboarding domain"`
- `"use hedge fund domain"`

### 2. DSL Verb Detection
Messages containing specific DSL verbs:
- `"investor.start-opportunity"` → hedge-fund-investor
- `"case.create"` → onboarding

### 3. Context-Based Routing
Based on existing session context and state:
- Current domain continuation
- State machine progression logic

### 4. Keyword Matching
Based on domain-specific keywords:
- `"investor"`, `"opportunity"`, `"subscription"` → hedge-fund-investor  
- `"case"`, `"CBU"`, `"onboarding"` → onboarding
- `"KYC"` → either domain (context-dependent)

### 5. Default Domain
Falls back to hedge-fund-investor for backward compatibility

### 6. Fallback Strategy
Error recovery and alternative routing options

---

## Error Handling

### Standard Error Response Format

```json
{
  "error": "Error description",
  "code": "ERROR_CODE",
  "details": {
    "field": "specific field that caused error",
    "value": "invalid value"
  },
  "timestamp": "2025-11-04T16:30:00Z"
}
```

### Common Error Codes

- `DOMAIN_NOT_FOUND` - Specified domain does not exist
- `INVALID_DSL` - DSL validation failed
- `SESSION_NOT_FOUND` - Session ID not found
- `ROUTING_FAILED` - Unable to route message to appropriate domain
- `INVALID_REQUEST` - Malformed request body
- `GENERATION_FAILED` - DSL generation failed

### HTTP Status Codes

- `200 OK` - Request successful
- `400 Bad Request` - Invalid request format or parameters
- `404 Not Found` - Resource not found (domain, session, etc.)
- `422 Unprocessable Entity` - Valid request but business logic error
- `500 Internal Server Error` - Server-side error
- `503 Service Unavailable` - System unhealthy or degraded

---

## Rate Limiting

Currently no rate limiting is implemented. Future versions may include:
- Per-session rate limits
- Per-IP rate limits
- Domain-specific rate limits

---

## Examples

### Complete Hedge Fund Workflow

1. **Create Opportunity:**
```bash
curl -X POST http://localhost:8080/api/chat \
  -H "Content-Type: application/json" \
  -d '{
    "message": "Create investor opportunity for Acme Capital LP, Swiss corporate investor"
  }'
```

2. **Start KYC:**
```bash
curl -X POST http://localhost:8080/api/chat \
  -H "Content-Type: application/json" \
  -d '{
    "session_id": "returned-session-id",
    "message": "Start KYC process for this investor"
  }'
```

3. **Collect Documents:**
```bash
curl -X POST http://localhost:8080/api/chat \
  -H "Content-Type: application/json" \
  -d '{
    "session_id": "returned-session-id", 
    "message": "Collect certificate of incorporation"
  }'
```

### Complete Onboarding Workflow

1. **Create Case:**
```bash
curl -X POST http://localhost:8080/api/chat \
  -H "Content-Type: application/json" \
  -d '{
    "message": "Create case for CBU-1234, UCITS fund in Luxembourg"
  }'
```

2. **Add Products:**
```bash
curl -X POST http://localhost:8080/api/chat \
  -H "Content-Type: application/json" \
  -d '{
    "session_id": "returned-session-id",
    "message": "Add custody and fund accounting products"
  }'
```

3. **Resource Planning:**
```bash
curl -X POST http://localhost:8080/api/chat \
  -H "Content-Type: application/json" \
  -d '{
    "session_id": "returned-session-id",
    "message": "Plan custody account resources"
  }'
```

---

## SDK Usage Examples

### JavaScript/Node.js

```javascript
const axios = require('axios');

const client = axios.create({
  baseURL: 'http://localhost:8080/api',
  headers: { 'Content-Type': 'application/json' }
});

// Create investor opportunity
async function createInvestor() {
  const response = await client.post('/chat', {
    message: 'Create investor opportunity for ABC Corp'
  });
  
  console.log('DSL:', response.data.dsl);
  return response.data.session_id;
}

// Continue conversation
async function startKYC(sessionId) {
  const response = await client.post('/chat', {
    session_id: sessionId,
    message: 'Start KYC process'
  });
  
  console.log('Updated DSL:', response.data.dsl);
}
```

### Python

```python
import requests

class MultiDomainDSLClient:
    def __init__(self, base_url='http://localhost:8080/api'):
        self.base_url = base_url
        self.session = requests.Session()
        self.session.headers.update({'Content-Type': 'application/json'})
    
    def chat(self, message, session_id=None, domain=None):
        payload = {'message': message}
        if session_id:
            payload['session_id'] = session_id
        if domain:
            payload['domain'] = domain
            
        response = self.session.post(f'{self.base_url}/chat', json=payload)
        response.raise_for_status()
        return response.json()
    
    def get_domains(self):
        response = self.session.get(f'{self.base_url}/domains')
        response.raise_for_status()
        return response.json()

# Usage
client = MultiDomainDSLClient()

# Get available domains
domains = client.get_domains()
print(f"Available domains: {list(domains['domains'].keys())}")

# Create investor
result = client.chat("Create investor opportunity for XYZ Fund")
session_id = result['session_id']
print(f"Generated DSL: {result['dsl']}")

# Continue workflow
result = client.chat("Start KYC process", session_id=session_id)
print(f"Complete DSL: {result['dsl']}")
```

---

## Changelog

### Version 1.0.0 (Current)
- Initial multi-domain API implementation
- Support for hedge-fund-investor and onboarding domains
- Intelligent domain routing
- WebSocket real-time chat
- Session management with DSL accumulation
- Comprehensive vocabulary access
- Health monitoring and metrics

### Planned Features
- Authentication and authorization
- Rate limiting
- DSL execution engine
- Additional domains (KYC, Compliance, etc.)
- Enhanced analytics and reporting
- Webhook support for integrations