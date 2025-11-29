# CSG Implementation: Steps 3, 4, 5

**Status**: Steps 1-2 complete (compile errors fixed, tests passing)
**Remaining**: Seed data, embeddings generation, pipeline integration

---

## Step 3: Populate Seed Data for Document Type Applicability

### 3.1 Create Seed File for Document Applicability Rules

Create file: `sql/seeds/010_csg_document_applicability.sql`

```sql
-- CSG Document Applicability Seed Data
-- Populates document_types.applicability and semantic_context for CSG linting

BEGIN;

-- PASSPORT - Identity document for natural persons only
UPDATE "ob-poc".document_types
SET 
    applicability = '{
        "entity_types": ["PROPER_PERSON", "PROPER_PERSON_NATURAL", "BENEFICIAL_OWNER"],
        "jurisdictions": [],
        "client_types": [],
        "required_for": ["PROPER_PERSON_NATURAL"],
        "excludes": [],
        "requires": [],
        "category": "IDENTITY"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Primary identity verification for natural persons",
        "synonyms": ["travel document", "ID document", "identity card"],
        "keywords": ["identity", "government issued", "photo ID", "MRZ"],
        "extraction_hints": {
            "ocr_zones": ["mrz", "photo", "personal_data"],
            "expiry_check": true,
            "mrz_validation": true
        }
    }'::jsonb
WHERE type_code = 'PASSPORT';

-- DRIVERS_LICENSE - Identity document for natural persons only
UPDATE "ob-poc".document_types
SET 
    applicability = '{
        "entity_types": ["PROPER_PERSON", "PROPER_PERSON_NATURAL", "BENEFICIAL_OWNER"],
        "jurisdictions": [],
        "client_types": [],
        "required_for": [],
        "excludes": [],
        "requires": [],
        "category": "IDENTITY"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Secondary identity and address verification for natural persons",
        "synonyms": ["driving license", "driver license", "DL"],
        "keywords": ["identity", "government issued", "photo ID", "address"],
        "extraction_hints": {
            "ocr_zones": ["photo", "personal_data", "address"],
            "expiry_check": true
        }
    }'::jsonb
WHERE type_code = 'DRIVERS_LICENSE';

-- NATIONAL_ID - Identity document for natural persons only
UPDATE "ob-poc".document_types
SET 
    applicability = '{
        "entity_types": ["PROPER_PERSON", "PROPER_PERSON_NATURAL", "BENEFICIAL_OWNER"],
        "jurisdictions": [],
        "client_types": [],
        "required_for": [],
        "excludes": [],
        "requires": [],
        "category": "IDENTITY"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Government-issued national identity card",
        "synonyms": ["ID card", "national identity card", "citizen card"],
        "keywords": ["identity", "government issued", "photo ID"],
        "extraction_hints": {
            "ocr_zones": ["photo", "personal_data"],
            "expiry_check": true
        }
    }'::jsonb
WHERE type_code = 'NATIONAL_ID';

-- CERTIFICATE_OF_INCORPORATION - Corporate formation document
UPDATE "ob-poc".document_types
SET 
    applicability = '{
        "entity_types": ["LIMITED_COMPANY", "LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "LLC"],
        "jurisdictions": [],
        "client_types": ["corporate"],
        "required_for": ["LIMITED_COMPANY", "LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "LLC"],
        "excludes": [],
        "requires": [],
        "category": "FORMATION"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Official proof of company incorporation and legal existence",
        "synonyms": ["incorporation certificate", "company registration", "formation document"],
        "keywords": ["incorporation", "registered", "company number", "formation date"],
        "extraction_hints": {
            "key_fields": ["company_name", "company_number", "incorporation_date", "jurisdiction"],
            "registry_validation": true
        }
    }'::jsonb
WHERE type_code = 'CERTIFICATE_OF_INCORPORATION';

-- ARTICLES_OF_ASSOCIATION - Corporate governance document
UPDATE "ob-poc".document_types
SET 
    applicability = '{
        "entity_types": ["LIMITED_COMPANY", "LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "LLC"],
        "jurisdictions": [],
        "client_types": ["corporate"],
        "required_for": ["LIMITED_COMPANY_PUBLIC"],
        "excludes": [],
        "requires": ["CERTIFICATE_OF_INCORPORATION"],
        "category": "GOVERNANCE"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Company constitutional document defining governance structure",
        "synonyms": ["articles of incorporation", "bylaws", "memorandum of association", "constitution"],
        "keywords": ["governance", "directors", "shareholders", "voting rights", "share classes"],
        "extraction_hints": {
            "key_sections": ["directors", "shareholders", "share_capital", "voting"]
        }
    }'::jsonb
WHERE type_code = 'ARTICLES_OF_ASSOCIATION';

-- TRUST_DEED - Trust formation document
UPDATE "ob-poc".document_types
SET 
    applicability = '{
        "entity_types": ["TRUST", "TRUST_DISCRETIONARY", "TRUST_FIXED_INTEREST", "TRUST_UNIT"],
        "jurisdictions": [],
        "client_types": ["trust"],
        "required_for": ["TRUST", "TRUST_DISCRETIONARY", "TRUST_FIXED_INTEREST", "TRUST_UNIT"],
        "excludes": [],
        "requires": [],
        "category": "FORMATION"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Legal document establishing trust structure and terms",
        "synonyms": ["trust agreement", "deed of trust", "trust instrument", "settlement deed"],
        "keywords": ["trustee", "settlor", "beneficiary", "trust property", "discretionary"],
        "extraction_hints": {
            "key_parties": ["trustees", "settlor", "beneficiaries", "protector"],
            "key_sections": ["trust_property", "distributions", "powers"]
        }
    }'::jsonb
WHERE type_code = 'TRUST_DEED';

-- PARTNERSHIP_AGREEMENT - Partnership formation document
UPDATE "ob-poc".document_types
SET 
    applicability = '{
        "entity_types": ["PARTNERSHIP", "PARTNERSHIP_GENERAL", "PARTNERSHIP_LIMITED", "PARTNERSHIP_LLP"],
        "jurisdictions": [],
        "client_types": ["corporate"],
        "required_for": ["PARTNERSHIP", "PARTNERSHIP_GENERAL", "PARTNERSHIP_LIMITED", "PARTNERSHIP_LLP"],
        "excludes": [],
        "requires": [],
        "category": "FORMATION"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Agreement establishing partnership terms and partner relationships",
        "synonyms": ["partnership deed", "LLP agreement", "partner agreement"],
        "keywords": ["partners", "profit sharing", "capital contribution", "management"],
        "extraction_hints": {
            "key_parties": ["general_partners", "limited_partners"],
            "key_sections": ["capital", "profit_loss", "management", "dissolution"]
        }
    }'::jsonb
WHERE type_code = 'PARTNERSHIP_AGREEMENT';

-- PROOF_OF_ADDRESS - Universal document (no entity type restriction)
UPDATE "ob-poc".document_types
SET 
    applicability = '{
        "entity_types": [],
        "jurisdictions": [],
        "client_types": [],
        "required_for": [],
        "excludes": [],
        "requires": [],
        "category": "ADDRESS"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Verification of residential or business address",
        "synonyms": ["utility bill", "bank statement", "address verification"],
        "keywords": ["address", "residence", "utility", "recent"],
        "extraction_hints": {
            "key_fields": ["name", "address", "date"],
            "recency_check": true,
            "max_age_months": 3
        }
    }'::jsonb
WHERE type_code = 'PROOF_OF_ADDRESS';

-- FINANCIAL_STATEMENTS - Corporate financial document
UPDATE "ob-poc".document_types
SET 
    applicability = '{
        "entity_types": ["LIMITED_COMPANY", "LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "PARTNERSHIP", "PARTNERSHIP_GENERAL", "PARTNERSHIP_LIMITED", "TRUST"],
        "jurisdictions": [],
        "client_types": ["corporate", "trust"],
        "required_for": ["LIMITED_COMPANY_PUBLIC"],
        "excludes": [],
        "requires": [],
        "category": "FINANCIAL"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Audited or management financial statements for due diligence",
        "synonyms": ["accounts", "annual report", "financial report", "audited accounts"],
        "keywords": ["balance sheet", "income statement", "cash flow", "audit", "assets", "liabilities"],
        "extraction_hints": {
            "key_sections": ["balance_sheet", "income_statement", "cash_flow", "notes"],
            "audit_check": true
        }
    }'::jsonb
WHERE type_code = 'FINANCIAL_STATEMENTS';

-- BENEFICIAL_OWNERSHIP_DECLARATION - Compliance document
UPDATE "ob-poc".document_types
SET 
    applicability = '{
        "entity_types": ["LIMITED_COMPANY", "LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "TRUST", "PARTNERSHIP", "LLC"],
        "jurisdictions": [],
        "client_types": ["corporate", "trust"],
        "required_for": ["LIMITED_COMPANY", "TRUST", "PARTNERSHIP"],
        "excludes": [],
        "requires": [],
        "category": "COMPLIANCE"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Declaration of ultimate beneficial owners for AML compliance",
        "synonyms": ["UBO declaration", "beneficial owner form", "ownership declaration"],
        "keywords": ["beneficial owner", "UBO", "ownership", "control", "25%", "threshold"],
        "extraction_hints": {
            "key_fields": ["beneficial_owners", "ownership_percentage", "control_type"],
            "threshold_check": true
        }
    }'::jsonb
WHERE type_code = 'BENEFICIAL_OWNERSHIP_DECLARATION';

-- REGISTER_OF_MEMBERS - Corporate register
UPDATE "ob-poc".document_types
SET 
    applicability = '{
        "entity_types": ["LIMITED_COMPANY", "LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"],
        "jurisdictions": [],
        "client_types": ["corporate"],
        "required_for": [],
        "excludes": [],
        "requires": [],
        "category": "REGISTER"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Official register of company shareholders/members",
        "synonyms": ["shareholder register", "member register", "share register"],
        "keywords": ["shareholders", "members", "shares", "ownership"],
        "extraction_hints": {
            "key_fields": ["shareholders", "share_class", "number_of_shares", "percentage"]
        }
    }'::jsonb
WHERE type_code = 'REGISTER_OF_MEMBERS';

-- REGISTER_OF_DIRECTORS - Corporate register
UPDATE "ob-poc".document_types
SET 
    applicability = '{
        "entity_types": ["LIMITED_COMPANY", "LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"],
        "jurisdictions": [],
        "client_types": ["corporate"],
        "required_for": [],
        "excludes": [],
        "requires": [],
        "category": "REGISTER"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Official register of company directors and officers",
        "synonyms": ["director register", "officer register", "board register"],
        "keywords": ["directors", "officers", "board", "appointment", "resignation"],
        "extraction_hints": {
            "key_fields": ["directors", "appointment_date", "role", "nationality"]
        }
    }'::jsonb
WHERE type_code = 'REGISTER_OF_DIRECTORS';

COMMIT;
```

### 3.2 Create Seed File for Entity Type Hierarchy

Create file: `sql/seeds/011_csg_entity_type_hierarchy.sql`

```sql
-- CSG Entity Type Hierarchy Seed Data
-- Populates entity_types.type_code and type_hierarchy_path

BEGIN;

-- First, populate type_code from name (normalized: uppercase, underscores)
UPDATE "ob-poc".entity_types
SET type_code = UPPER(REPLACE(REPLACE(name, ' ', '_'), '-', '_'))
WHERE type_code IS NULL;

-- Set up hierarchy paths
-- Root: ENTITY (abstract)
UPDATE "ob-poc".entity_types
SET 
    type_hierarchy_path = ARRAY['ENTITY'],
    semantic_context = '{
        "category": "ROOT",
        "is_abstract": true,
        "typical_documents": [],
        "typical_attributes": []
    }'::jsonb
WHERE type_code = 'ENTITY';

-- PROPER_PERSON (natural persons)
UPDATE "ob-poc".entity_types
SET 
    type_hierarchy_path = ARRAY['ENTITY', 'PROPER_PERSON'],
    semantic_context = '{
        "category": "NATURAL_PERSON",
        "is_abstract": false,
        "typical_documents": ["PASSPORT", "DRIVERS_LICENSE", "NATIONAL_ID", "PROOF_OF_ADDRESS"],
        "typical_attributes": ["first_name", "last_name", "date_of_birth", "nationality", "residential_address"]
    }'::jsonb
WHERE type_code = 'PROPER_PERSON';

UPDATE "ob-poc".entity_types
SET 
    type_hierarchy_path = ARRAY['ENTITY', 'PROPER_PERSON', 'PROPER_PERSON_NATURAL'],
    semantic_context = '{
        "category": "NATURAL_PERSON",
        "is_abstract": false,
        "typical_documents": ["PASSPORT", "DRIVERS_LICENSE", "NATIONAL_ID", "PROOF_OF_ADDRESS"],
        "typical_attributes": ["first_name", "last_name", "date_of_birth", "nationality", "residential_address", "tax_id"]
    }'::jsonb
WHERE type_code = 'PROPER_PERSON_NATURAL';

UPDATE "ob-poc".entity_types
SET 
    type_hierarchy_path = ARRAY['ENTITY', 'PROPER_PERSON', 'BENEFICIAL_OWNER'],
    semantic_context = '{
        "category": "NATURAL_PERSON",
        "is_abstract": false,
        "typical_documents": ["PASSPORT", "DRIVERS_LICENSE", "NATIONAL_ID", "PROOF_OF_ADDRESS"],
        "typical_attributes": ["first_name", "last_name", "date_of_birth", "nationality", "ownership_percentage", "control_type"]
    }'::jsonb
WHERE type_code = 'BENEFICIAL_OWNER';

-- LEGAL_ENTITY (abstract grouping)
UPDATE "ob-poc".entity_types
SET 
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY'],
    semantic_context = '{
        "category": "LEGAL_ENTITY",
        "is_abstract": true,
        "typical_documents": [],
        "typical_attributes": []
    }'::jsonb
WHERE type_code = 'LEGAL_ENTITY';

-- LIMITED_COMPANY hierarchy
UPDATE "ob-poc".entity_types
SET 
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'LIMITED_COMPANY'],
    semantic_context = '{
        "category": "CORPORATE",
        "is_abstract": false,
        "typical_documents": ["CERTIFICATE_OF_INCORPORATION", "ARTICLES_OF_ASSOCIATION", "REGISTER_OF_MEMBERS", "REGISTER_OF_DIRECTORS", "FINANCIAL_STATEMENTS"],
        "typical_attributes": ["company_name", "company_number", "incorporation_date", "jurisdiction", "registered_address"]
    }'::jsonb
WHERE type_code = 'LIMITED_COMPANY';

UPDATE "ob-poc".entity_types
SET 
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'LIMITED_COMPANY', 'LIMITED_COMPANY_PRIVATE'],
    semantic_context = '{
        "category": "CORPORATE",
        "is_abstract": false,
        "typical_documents": ["CERTIFICATE_OF_INCORPORATION", "ARTICLES_OF_ASSOCIATION", "REGISTER_OF_MEMBERS", "REGISTER_OF_DIRECTORS"],
        "typical_attributes": ["company_name", "company_number", "incorporation_date", "jurisdiction", "registered_address"]
    }'::jsonb
WHERE type_code = 'LIMITED_COMPANY_PRIVATE';

UPDATE "ob-poc".entity_types
SET 
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'LIMITED_COMPANY', 'LIMITED_COMPANY_PUBLIC'],
    semantic_context = '{
        "category": "CORPORATE",
        "is_abstract": false,
        "typical_documents": ["CERTIFICATE_OF_INCORPORATION", "ARTICLES_OF_ASSOCIATION", "REGISTER_OF_MEMBERS", "REGISTER_OF_DIRECTORS", "FINANCIAL_STATEMENTS"],
        "typical_attributes": ["company_name", "company_number", "incorporation_date", "jurisdiction", "registered_address", "stock_exchange", "ticker_symbol"]
    }'::jsonb
WHERE type_code = 'LIMITED_COMPANY_PUBLIC';

-- LLC
UPDATE "ob-poc".entity_types
SET 
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'LLC'],
    semantic_context = '{
        "category": "CORPORATE",
        "is_abstract": false,
        "typical_documents": ["CERTIFICATE_OF_INCORPORATION", "ARTICLES_OF_ASSOCIATION"],
        "typical_attributes": ["company_name", "company_number", "formation_date", "jurisdiction", "registered_address"]
    }'::jsonb
WHERE type_code = 'LLC';

-- PARTNERSHIP hierarchy
UPDATE "ob-poc".entity_types
SET 
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'PARTNERSHIP'],
    semantic_context = '{
        "category": "PARTNERSHIP",
        "is_abstract": false,
        "typical_documents": ["PARTNERSHIP_AGREEMENT", "FINANCIAL_STATEMENTS"],
        "typical_attributes": ["partnership_name", "formation_date", "jurisdiction"]
    }'::jsonb
WHERE type_code = 'PARTNERSHIP';

UPDATE "ob-poc".entity_types
SET 
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'PARTNERSHIP', 'PARTNERSHIP_GENERAL'],
    semantic_context = '{
        "category": "PARTNERSHIP",
        "is_abstract": false,
        "typical_documents": ["PARTNERSHIP_AGREEMENT"],
        "typical_attributes": ["partnership_name", "formation_date", "jurisdiction", "general_partners"]
    }'::jsonb
WHERE type_code = 'PARTNERSHIP_GENERAL';

UPDATE "ob-poc".entity_types
SET 
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'PARTNERSHIP', 'PARTNERSHIP_LIMITED'],
    semantic_context = '{
        "category": "PARTNERSHIP",
        "is_abstract": false,
        "typical_documents": ["PARTNERSHIP_AGREEMENT", "FINANCIAL_STATEMENTS"],
        "typical_attributes": ["partnership_name", "formation_date", "jurisdiction", "general_partners", "limited_partners"]
    }'::jsonb
WHERE type_code = 'PARTNERSHIP_LIMITED';

UPDATE "ob-poc".entity_types
SET 
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'PARTNERSHIP', 'PARTNERSHIP_LLP'],
    semantic_context = '{
        "category": "PARTNERSHIP",
        "is_abstract": false,
        "typical_documents": ["PARTNERSHIP_AGREEMENT", "CERTIFICATE_OF_INCORPORATION"],
        "typical_attributes": ["partnership_name", "registration_number", "formation_date", "jurisdiction"]
    }'::jsonb
WHERE type_code = 'PARTNERSHIP_LLP';

-- TRUST hierarchy
UPDATE "ob-poc".entity_types
SET 
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'TRUST'],
    semantic_context = '{
        "category": "TRUST",
        "is_abstract": false,
        "typical_documents": ["TRUST_DEED", "FINANCIAL_STATEMENTS"],
        "typical_attributes": ["trust_name", "establishment_date", "jurisdiction", "trustees", "settlor"]
    }'::jsonb
WHERE type_code = 'TRUST';

UPDATE "ob-poc".entity_types
SET 
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'TRUST', 'TRUST_DISCRETIONARY'],
    semantic_context = '{
        "category": "TRUST",
        "is_abstract": false,
        "typical_documents": ["TRUST_DEED"],
        "typical_attributes": ["trust_name", "establishment_date", "jurisdiction", "trustees", "settlor", "beneficiary_classes"]
    }'::jsonb
WHERE type_code = 'TRUST_DISCRETIONARY';

UPDATE "ob-poc".entity_types
SET 
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'TRUST', 'TRUST_FIXED_INTEREST'],
    semantic_context = '{
        "category": "TRUST",
        "is_abstract": false,
        "typical_documents": ["TRUST_DEED"],
        "typical_attributes": ["trust_name", "establishment_date", "jurisdiction", "trustees", "settlor", "named_beneficiaries"]
    }'::jsonb
WHERE type_code = 'TRUST_FIXED_INTEREST';

UPDATE "ob-poc".entity_types
SET 
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'TRUST', 'TRUST_UNIT'],
    semantic_context = '{
        "category": "TRUST",
        "is_abstract": false,
        "typical_documents": ["TRUST_DEED", "FINANCIAL_STATEMENTS"],
        "typical_attributes": ["trust_name", "establishment_date", "jurisdiction", "trustees", "unit_holders"]
    }'::jsonb
WHERE type_code = 'TRUST_UNIT';

-- FUND types
UPDATE "ob-poc".entity_types
SET 
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'FUND'],
    semantic_context = '{
        "category": "FUND",
        "is_abstract": false,
        "typical_documents": ["CERTIFICATE_OF_INCORPORATION", "FINANCIAL_STATEMENTS"],
        "typical_attributes": ["fund_name", "formation_date", "jurisdiction", "fund_manager", "investment_strategy"]
    }'::jsonb
WHERE type_code = 'FUND';

COMMIT;
```

### 3.3 Run Seed Files

Execute in order:

```bash
cd /Users/adamtc007/Developer/ob-poc

# Run document applicability seeds
psql -d ob-poc -f sql/seeds/010_csg_document_applicability.sql

# Run entity hierarchy seeds  
psql -d ob-poc -f sql/seeds/011_csg_entity_type_hierarchy.sql
```

### 3.4 Verify Seed Data

```sql
-- Verify document applicability populated
SELECT type_code, 
       applicability->'entity_types' as entity_types,
       applicability->'category' as category
FROM "ob-poc".document_types 
WHERE applicability != '{}'::jsonb
ORDER BY type_code;

-- Verify entity hierarchy populated
SELECT type_code, type_hierarchy_path, 
       semantic_context->'category' as category
FROM "ob-poc".entity_types
WHERE type_hierarchy_path IS NOT NULL
ORDER BY array_length(type_hierarchy_path, 1), type_code;
```

---

## Step 4: Generate Embeddings for Vector Similarity

### 4.1 Create Embedding Generation Script

Create file: `scripts/generate_csg_embeddings.py`

```python
#!/usr/bin/env python3
"""
Generate embeddings for CSG metadata using OpenAI ada-002.

Prerequisites:
    pip install openai psycopg2-binary python-dotenv

Usage:
    export OPENAI_API_KEY=sk-...
    python scripts/generate_csg_embeddings.py
"""

import os
import json
import time
from typing import Optional
import psycopg2
from psycopg2.extras import execute_values
from openai import OpenAI
from dotenv import load_dotenv

load_dotenv()

# Configuration
DATABASE_URL = os.getenv("DATABASE_URL", "postgresql://localhost/ob-poc")
OPENAI_API_KEY = os.getenv("OPENAI_API_KEY")
EMBEDDING_MODEL = "text-embedding-ada-002"
EMBEDDING_DIMENSIONS = 1536
BATCH_SIZE = 20
RATE_LIMIT_DELAY = 0.5  # seconds between batches


def get_embedding(client: OpenAI, text: str) -> list[float]:
    """Get embedding vector for text."""
    response = client.embeddings.create(
        model=EMBEDDING_MODEL,
        input=text
    )
    return response.data[0].embedding


def build_document_type_text(row: dict) -> str:
    """Build text representation for document type embedding."""
    parts = [
        f"Document type: {row['type_code']}",
        f"Display name: {row.get('display_name', row['type_code'])}",
    ]
    
    if row.get('semantic_context'):
        ctx = row['semantic_context']
        if ctx.get('purpose'):
            parts.append(f"Purpose: {ctx['purpose']}")
        if ctx.get('synonyms'):
            parts.append(f"Also known as: {', '.join(ctx['synonyms'])}")
        if ctx.get('keywords'):
            parts.append(f"Keywords: {', '.join(ctx['keywords'])}")
    
    if row.get('applicability'):
        app = row['applicability']
        if app.get('entity_types'):
            parts.append(f"Applicable to: {', '.join(app['entity_types'])}")
        if app.get('category'):
            parts.append(f"Category: {app['category']}")
    
    return "\n".join(parts)


def build_entity_type_text(row: dict) -> str:
    """Build text representation for entity type embedding."""
    parts = [
        f"Entity type: {row['type_code']}",
        f"Name: {row.get('name', row['type_code'])}",
    ]
    
    if row.get('type_hierarchy_path'):
        parts.append(f"Hierarchy: {' > '.join(row['type_hierarchy_path'])}")
    
    if row.get('semantic_context'):
        ctx = row['semantic_context']
        if ctx.get('category'):
            parts.append(f"Category: {ctx['category']}")
        if ctx.get('typical_documents'):
            parts.append(f"Typical documents: {', '.join(ctx['typical_documents'])}")
        if ctx.get('typical_attributes'):
            parts.append(f"Typical attributes: {', '.join(ctx['typical_attributes'])}")
    
    return "\n".join(parts)


def generate_document_type_embeddings(conn, client: OpenAI):
    """Generate embeddings for all document types."""
    print("\n=== Generating Document Type Embeddings ===")
    
    with conn.cursor() as cur:
        # Get document types needing embeddings
        cur.execute("""
            SELECT type_id, type_code, display_name, applicability, semantic_context
            FROM "ob-poc".document_types
            WHERE embedding IS NULL 
               OR embedding_updated_at < NOW() - INTERVAL '7 days'
        """)
        rows = cur.fetchall()
        columns = [desc[0] for desc in cur.description]
        
        print(f"Found {len(rows)} document types to process")
        
        for i, row in enumerate(rows):
            row_dict = dict(zip(columns, row))
            text = build_document_type_text(row_dict)
            
            try:
                embedding = get_embedding(client, text)
                
                cur.execute("""
                    UPDATE "ob-poc".document_types
                    SET embedding = %s::vector,
                        embedding_model = %s,
                        embedding_updated_at = NOW()
                    WHERE type_id = %s
                """, (embedding, EMBEDDING_MODEL, row_dict['type_id']))
                
                print(f"  [{i+1}/{len(rows)}] {row_dict['type_code']} ✓")
                
            except Exception as e:
                print(f"  [{i+1}/{len(rows)}] {row_dict['type_code']} ✗ Error: {e}")
            
            if (i + 1) % BATCH_SIZE == 0:
                conn.commit()
                time.sleep(RATE_LIMIT_DELAY)
        
        conn.commit()


def generate_entity_type_embeddings(conn, client: OpenAI):
    """Generate embeddings for all entity types."""
    print("\n=== Generating Entity Type Embeddings ===")
    
    with conn.cursor() as cur:
        # Get entity types needing embeddings
        cur.execute("""
            SELECT entity_type_id, type_code, name, type_hierarchy_path, semantic_context
            FROM "ob-poc".entity_types
            WHERE type_code IS NOT NULL
              AND (embedding IS NULL 
                   OR embedding_updated_at < NOW() - INTERVAL '7 days')
        """)
        rows = cur.fetchall()
        columns = [desc[0] for desc in cur.description]
        
        print(f"Found {len(rows)} entity types to process")
        
        for i, row in enumerate(rows):
            row_dict = dict(zip(columns, row))
            text = build_entity_type_text(row_dict)
            
            try:
                embedding = get_embedding(client, text)
                
                cur.execute("""
                    UPDATE "ob-poc".entity_types
                    SET embedding = %s::vector,
                        embedding_model = %s,
                        embedding_updated_at = NOW()
                    WHERE entity_type_id = %s
                """, (embedding, EMBEDDING_MODEL, row_dict['entity_type_id']))
                
                print(f"  [{i+1}/{len(rows)}] {row_dict['type_code']} ✓")
                
            except Exception as e:
                print(f"  [{i+1}/{len(rows)}] {row_dict['type_code']} ✗ Error: {e}")
            
            if (i + 1) % BATCH_SIZE == 0:
                conn.commit()
                time.sleep(RATE_LIMIT_DELAY)
        
        conn.commit()


def populate_similarity_cache(conn):
    """Populate the semantic similarity cache."""
    print("\n=== Populating Similarity Cache ===")
    
    with conn.cursor() as cur:
        # Use the database function if it exists
        cur.execute("""
            SELECT EXISTS (
                SELECT 1 FROM pg_proc 
                WHERE proname = 'refresh_document_type_similarities'
            )
        """)
        
        if cur.fetchone()[0]:
            print("Running refresh_document_type_similarities()...")
            cur.execute("SELECT refresh_document_type_similarities()")
            result = cur.fetchone()[0]
            print(f"Inserted {result} similarity records")
        else:
            print("Similarity cache function not found, computing manually...")
            
            # Manual computation
            cur.execute("""
                INSERT INTO "ob-poc".csg_semantic_similarity_cache 
                    (source_type, source_code, target_type, target_code, 
                     cosine_similarity, computed_at, expires_at)
                SELECT 
                    'document_type', d1.type_code,
                    'document_type', d2.type_code,
                    1 - (d1.embedding <=> d2.embedding) as similarity,
                    NOW(), NOW() + INTERVAL '7 days'
                FROM "ob-poc".document_types d1
                CROSS JOIN "ob-poc".document_types d2
                WHERE d1.type_id < d2.type_id
                  AND d1.embedding IS NOT NULL
                  AND d2.embedding IS NOT NULL
                  AND 1 - (d1.embedding <=> d2.embedding) > 0.5
                ON CONFLICT (source_type, source_code, target_type, target_code) 
                DO UPDATE SET 
                    cosine_similarity = EXCLUDED.cosine_similarity,
                    computed_at = NOW(),
                    expires_at = NOW() + INTERVAL '7 days'
            """)
            print(f"Inserted/updated {cur.rowcount} document similarity records")
            
            # Entity type similarities
            cur.execute("""
                INSERT INTO "ob-poc".csg_semantic_similarity_cache 
                    (source_type, source_code, target_type, target_code, 
                     cosine_similarity, computed_at, expires_at)
                SELECT 
                    'entity_type', e1.type_code,
                    'entity_type', e2.type_code,
                    1 - (e1.embedding <=> e2.embedding) as similarity,
                    NOW(), NOW() + INTERVAL '7 days'
                FROM "ob-poc".entity_types e1
                CROSS JOIN "ob-poc".entity_types e2
                WHERE e1.entity_type_id < e2.entity_type_id
                  AND e1.embedding IS NOT NULL
                  AND e2.embedding IS NOT NULL
                  AND e1.type_code IS NOT NULL
                  AND e2.type_code IS NOT NULL
                  AND 1 - (e1.embedding <=> e2.embedding) > 0.5
                ON CONFLICT (source_type, source_code, target_type, target_code) 
                DO UPDATE SET 
                    cosine_similarity = EXCLUDED.cosine_similarity,
                    computed_at = NOW(),
                    expires_at = NOW() + INTERVAL '7 days'
            """)
            print(f"Inserted/updated {cur.rowcount} entity similarity records")
        
        conn.commit()


def main():
    if not OPENAI_API_KEY:
        print("ERROR: OPENAI_API_KEY not set")
        print("Set it via: export OPENAI_API_KEY=sk-...")
        return
    
    print(f"Connecting to database...")
    conn = psycopg2.connect(DATABASE_URL)
    client = OpenAI(api_key=OPENAI_API_KEY)
    
    try:
        generate_document_type_embeddings(conn, client)
        generate_entity_type_embeddings(conn, client)
        populate_similarity_cache(conn)
        
        print("\n=== Complete ===")
        
    finally:
        conn.close()


if __name__ == "__main__":
    main()
```

### 4.2 Run Embedding Generation

```bash
cd /Users/adamtc007/Developer/ob-poc

# Ensure dependencies
pip install openai psycopg2-binary python-dotenv

# Set API key
export OPENAI_API_KEY=sk-...

# Run script
python scripts/generate_csg_embeddings.py
```

### 4.3 Verify Embeddings

```sql
-- Check document type embeddings
SELECT type_code, 
       embedding IS NOT NULL as has_embedding,
       embedding_model,
       embedding_updated_at
FROM "ob-poc".document_types
WHERE applicability != '{}'::jsonb
ORDER BY type_code;

-- Check entity type embeddings
SELECT type_code,
       embedding IS NOT NULL as has_embedding,
       embedding_model,
       embedding_updated_at
FROM "ob-poc".entity_types
WHERE type_code IS NOT NULL
ORDER BY type_code;

-- Check similarity cache
SELECT source_type, source_code, target_type, target_code,
       ROUND(cosine_similarity::numeric, 3) as similarity
FROM "ob-poc".csg_semantic_similarity_cache
ORDER BY cosine_similarity DESC
LIMIT 20;
```

---

## Step 5: Integrate CSG Linter into Pipeline

### 5.1 Update Semantic Validator to Use CSG Linter

Edit file: `rust/src/dsl_v2/semantic_validator.rs`

Add the following integration (find appropriate location):

```rust
use crate::dsl_v2::csg_linter::{CsgLinter, LintResult};

impl SemanticValidator {
    // Add CSG linter as a field
    // csg_linter: Option<CsgLinter>,

    /// Run CSG linting pass before semantic validation
    pub async fn lint_csg(&self, ast: &Program, context: &ValidationContext, source: &str) -> Result<LintResult, String> {
        if let Some(ref linter) = self.csg_linter {
            Ok(linter.lint(ast.clone(), context, source).await)
        } else {
            // Return empty result if linter not initialized
            Ok(LintResult {
                ast: ast.clone(),
                diagnostics: vec![],
                inferred_context: Default::default(),
            })
        }
    }

    /// Full validation pipeline with CSG
    pub async fn validate_with_csg(
        &self,
        request: &ValidationRequest,
    ) -> ValidationResult {
        // Parse
        let ast = match parse_program(&request.source) {
            Ok(ast) => ast,
            Err(e) => return ValidationResult::Err(vec![/* parse error diagnostic */]),
        };

        // CSG Lint (new step)
        let lint_result = self.lint_csg(&ast, &request.context, &request.source).await;
        if let Ok(ref result) = lint_result {
            if result.has_errors() {
                return ValidationResult::Err(result.diagnostics.clone());
            }
        }

        // Continue with existing semantic validation...
        self.validate(request).await
    }
}
```

### 5.2 Add CSG Linter Initialization to Service Startup

Edit the service initialization code to initialize CSG linter:

```rust
// In your service startup (e.g., main.rs or service module)

async fn init_validators(pool: &PgPool) -> Result<SemanticValidator, Error> {
    // Initialize CSG Linter
    let mut csg_linter = CsgLinter::new(pool.clone());
    csg_linter.initialize().await
        .map_err(|e| anyhow::anyhow!("Failed to initialize CSG linter: {}", e))?;
    
    // Create validator with linter
    let validator = SemanticValidator::new(pool.clone())
        .with_csg_linter(csg_linter);
    
    Ok(validator)
}
```

### 5.3 Add Pipeline Integration Test

Create file: `rust/tests/csg_pipeline_integration.rs`

```rust
//! Integration tests for CSG linter in validation pipeline

use ob_poc::dsl_v2::{
    CsgLinter, 
    parse_program,
    validation::{ValidationContext, ValidationRequest, ClientType},
};

#[tokio::test]
#[ignore] // Requires database
async fn test_passport_for_company_rejected() {
    let pool = get_test_pool().await;
    
    let mut linter = CsgLinter::new(pool.clone());
    linter.initialize().await.unwrap();
    
    let source = r#"
        (entity.create-limited-company :name "Acme Corp" :as @company)
        (document.catalog :document-type "PASSPORT" :entity-id @company)
    "#;
    
    let ast = parse_program(source).unwrap();
    let context = ValidationContext::default()
        .with_client_type(ClientType::Corporate);
    
    let result = linter.lint(ast, &context, source).await;
    
    assert!(result.has_errors());
    assert!(result.diagnostics.iter().any(|d| 
        d.code == DiagnosticCode::DocumentNotApplicableToEntityType
    ));
}

#[tokio::test]
#[ignore] // Requires database
async fn test_passport_for_person_accepted() {
    let pool = get_test_pool().await;
    
    let mut linter = CsgLinter::new(pool.clone());
    linter.initialize().await.unwrap();
    
    let source = r#"
        (entity.create-proper-person :name "John Doe" :as @person)
        (document.catalog :document-type "PASSPORT" :entity-id @person)
    "#;
    
    let ast = parse_program(source).unwrap();
    let context = ValidationContext::default()
        .with_client_type(ClientType::Individual);
    
    let result = linter.lint(ast, &context, source).await;
    
    assert!(!result.has_errors());
}

#[tokio::test]
#[ignore] // Requires database  
async fn test_cert_incorporation_for_company_accepted() {
    let pool = get_test_pool().await;
    
    let mut linter = CsgLinter::new(pool.clone());
    linter.initialize().await.unwrap();
    
    let source = r#"
        (entity.create-limited-company :name "Acme Corp" :as @company)
        (document.catalog :document-type "CERTIFICATE_OF_INCORPORATION" :entity-id @company)
    "#;
    
    let ast = parse_program(source).unwrap();
    let context = ValidationContext::default()
        .with_client_type(ClientType::Corporate);
    
    let result = linter.lint(ast, &context, source).await;
    
    assert!(!result.has_errors());
}

#[tokio::test]
#[ignore] // Requires database
async fn test_undefined_symbol_detected() {
    let pool = get_test_pool().await;
    
    let mut linter = CsgLinter::new(pool.clone());
    linter.initialize().await.unwrap();
    
    let source = r#"
        (document.catalog :document-type "PASSPORT" :entity-id @nonexistent)
    "#;
    
    let ast = parse_program(source).unwrap();
    let context = ValidationContext::default();
    
    let result = linter.lint(ast, &context, source).await;
    
    assert!(result.has_errors());
    assert!(result.diagnostics.iter().any(|d| 
        d.code == DiagnosticCode::UndefinedSymbol
    ));
}

async fn get_test_pool() -> sqlx::PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/ob-poc".to_string());
    sqlx::PgPool::connect(&url).await.unwrap()
}
```

### 5.4 Run Integration Tests

```bash
cd /Users/adamtc007/Developer/ob-poc/rust

# Run with database feature and integration tests
DATABASE_URL=postgresql://localhost/ob-poc \
cargo test --features database csg_pipeline -- --ignored
```

---

## Verification Checklist

### Step 3 Verification
- [ ] `sql/seeds/010_csg_document_applicability.sql` created
- [ ] `sql/seeds/011_csg_entity_type_hierarchy.sql` created
- [ ] Seeds executed without error
- [ ] `document_types.applicability` populated (verify with query)
- [ ] `entity_types.type_hierarchy_path` populated (verify with query)

### Step 4 Verification
- [ ] `scripts/generate_csg_embeddings.py` created
- [ ] Script dependencies installed
- [ ] Script executed with valid API key
- [ ] `document_types.embedding` columns populated
- [ ] `entity_types.embedding` columns populated
- [ ] `csg_semantic_similarity_cache` populated

### Step 5 Verification
- [ ] `SemanticValidator` updated with CSG integration
- [ ] Integration tests created
- [ ] `cargo check --features database` passes
- [ ] Integration tests pass (with database)
- [ ] Pipeline correctly rejects PASSPORT for LIMITED_COMPANY
- [ ] Pipeline correctly accepts PASSPORT for PROPER_PERSON

---

## Troubleshooting

### Seed Data Issues

If document types don't exist:
```sql
-- Check what document types exist
SELECT type_code FROM "ob-poc".document_types ORDER BY type_code;

-- Insert missing document type
INSERT INTO "ob-poc".document_types (type_id, type_code, display_name)
VALUES (gen_random_uuid(), 'PASSPORT', 'Passport');
```

### Embedding Generation Issues

If OpenAI rate limited:
- Increase `RATE_LIMIT_DELAY` in script
- Run in smaller batches

If pgvector not installed:
```sql
CREATE EXTENSION IF NOT EXISTS vector;
```

### Integration Test Issues

If tests timeout:
- Ensure database is running
- Check `DATABASE_URL` is correct
- Verify CSG tables exist

---

## Summary

After completing steps 3-5:

1. **Seed data** provides applicability rules for common document types
2. **Embeddings** enable semantic similarity suggestions
3. **Pipeline integration** enforces CSG rules during validation

The CSG linter will now catch errors like:
- Assigning PASSPORT to a company (C001)
- Using documents invalid for jurisdiction (C002)
- Referencing undefined symbols (C007)
- Unused symbol bindings (W002)
