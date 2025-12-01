# CBU Entity Hierarchy - Migration Task

**Task**: Add entity hierarchy support to schema and DSL  
**Created**: 2025-12-01  
**Status**: READY FOR IMPLEMENTATION  
**Priority**: High

---

## Overview

Add three schema changes to support the CBU entity hierarchy model:
1. Commercial client link on CBU
2. Issuing entity link on share classes
3. Class category (CORPORATE vs FUND) on share classes

Then update the DSL verbs to expose these new columns.

---

## Part 1: SQL Migration

Create and run this migration:

```sql
-- Migration: Add entity hierarchy support
-- Date: 2025-12-01

-- =============================================================================
-- 1. Add commercial client to CBU
-- =============================================================================

ALTER TABLE "ob-poc".cbus 
ADD COLUMN IF NOT EXISTS commercial_client_entity_id UUID REFERENCES "ob-poc".entities(entity_id);

COMMENT ON COLUMN "ob-poc".cbus.commercial_client_entity_id IS 
'Head office entity that contracted with the bank (e.g., Blackrock Inc). Convenience field - actual ownership is in holdings chain.';

-- =============================================================================
-- 2. Add issuing entity to share_classes
-- =============================================================================

ALTER TABLE kyc.share_classes 
ADD COLUMN IF NOT EXISTS entity_id UUID REFERENCES "ob-poc".entities(entity_id);

COMMENT ON COLUMN kyc.share_classes.entity_id IS 
'The legal entity that issues this share class';

-- =============================================================================
-- 3. Add class category to share_classes
-- =============================================================================

ALTER TABLE kyc.share_classes 
ADD COLUMN IF NOT EXISTS class_category VARCHAR(20) DEFAULT 'FUND';

COMMENT ON COLUMN kyc.share_classes.class_category IS 
'CORPORATE = company ownership shares, FUND = investment fund shares';

-- Add check constraint (drop first if exists to make idempotent)
ALTER TABLE kyc.share_classes 
DROP CONSTRAINT IF EXISTS chk_class_category;

ALTER TABLE kyc.share_classes 
ADD CONSTRAINT chk_class_category 
CHECK (class_category IN ('CORPORATE', 'FUND'));

-- =============================================================================
-- 4. Add index on entity_id for share_classes
-- =============================================================================

CREATE INDEX IF NOT EXISTS idx_share_classes_entity 
ON kyc.share_classes(entity_id) 
WHERE entity_id IS NOT NULL;

-- =============================================================================
-- Done
-- =============================================================================
```

---

## Part 2: Verb YAML Updates

Update `rust/config/verbs.yaml` with the following changes:

### 2.1 Update cbu.create

Find the `cbu.create` verb and add this arg after the existing args:

```yaml
        - name: commercial-client-entity-id
          type: uuid
          required: false
          maps_to: commercial_client_entity_id
```

### 2.2 Update cbu.ensure

Find the `cbu.ensure` verb and add this arg after the existing args:

```yaml
        - name: commercial-client-entity-id
          type: uuid
          required: false
          maps_to: commercial_client_entity_id
```

### 2.3 Update cbu.update

Find the `cbu.update` verb and add this arg after the existing args:

```yaml
        - name: commercial-client-entity-id
          type: uuid
          required: false
          maps_to: commercial_client_entity_id
```

### 2.4 Update share-class.create

Find the `share-class.create` verb and add these args.

Add `entity-id` right after `cbu-id`:

```yaml
          - name: entity-id
            type: uuid
            required: false
            maps_to: entity_id
```

Add `class-category` after the existing args (before `returns:`):

```yaml
          - name: class-category
            type: string
            required: false
            maps_to: class_category
            valid_values: [CORPORATE, FUND]
```

### 2.5 Update share-class.ensure

Find the `share-class.ensure` verb and add these args.

Add `entity-id` right after `cbu-id`:

```yaml
          - name: entity-id
            type: uuid
            required: false
            maps_to: entity_id
```

Add `class-category` after the existing args (before `returns:`):

```yaml
          - name: class-category
            type: string
            required: false
            maps_to: class_category
            valid_values: [CORPORATE, FUND]
```

---

## Part 3: Update DATABASE_SCHEMA.md

Add these columns to the documentation:

### In cbus table section:

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| commercial_client_entity_id | uuid | YES | | FK to entities - head office that contracted with bank |

### In share_classes table section:

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| entity_id | uuid | YES | | FK to entities - legal entity that issues this share class |
| class_category | varchar(20) | NO | 'FUND' | CORPORATE = company ownership, FUND = investment fund |

---

## Part 4: Update CLAUDE.md

In the "Investor Registry DSL" section, update the share-class.create example to show new args:

```clojure
;; Create share class with issuing entity
(share-class.create :cbu-id @fund :entity-id @fund-entity :name "Class A EUR" 
  :isin "LU0123456789" :currency "EUR" :class-category "FUND"
  :nav-per-share 100.00 :management-fee-bps 150 :as @class-a)

;; Create corporate share class (for ManCo ownership)
(share-class.create :cbu-id @cbu :entity-id @manco :name "Ordinary Shares"
  :currency "EUR" :class-category "CORPORATE" :as @manco-shares)
```

---

## Verification

After implementation, verify:

1. **SQL**: 
   ```sql
   \d "ob-poc".cbus  -- should show commercial_client_entity_id
   \d kyc.share_classes  -- should show entity_id and class_category
   ```

2. **DSL**: Test these commands work:
   ```clojure
   (cbu.create :name "Test" :jurisdiction "US" :commercial-client-entity-id @head-office)
   
   (share-class.create :cbu-id @fund :entity-id @fund-entity :name "Class A" 
     :currency "EUR" :class-category "FUND")
   ```

3. **Validation**: Verify `class-category` rejects invalid values:
   ```clojure
   ;; This should fail validation
   (share-class.create :cbu-id @fund :entity-id @entity :name "Bad" 
     :currency "EUR" :class-category "INVALID")
   ```

---

## Summary

| Change | Table | Column | DSL Args |
|--------|-------|--------|----------|
| Commercial client | cbus | commercial_client_entity_id | cbu.create, cbu.ensure, cbu.update |
| Issuing entity | share_classes | entity_id | share-class.create, share-class.ensure |
| Class category | share_classes | class_category | share-class.create, share-class.ensure |

---

*End of Migration Task*
