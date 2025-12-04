# KYC DSL Implementation Plan: Phases 1-4

## Overview

This plan consolidates requirements from:
- `docs/KYC_DSL_LIFECYCLE_TODO.md` - Implementation TODO
- `docs/docs_KYC_UBO_DSL_SPEC.md` - DSL Specification
- Current implementation analysis

**Goal**: Implement the Threshold Decision Matrix (Phase 2), RFI System (Phase 3), and UBO Enhancement (Phase 4) features.

---

## Executive Summary

| Phase | Feature | New Tables | New Verbs | Plugins |
|-------|---------|------------|-----------|---------|
| 1 | DocumentTypeCode | - | - | - |
| 2 | Threshold Matrix | 5 | 3 | 3 |
| 3 | RFI System | 5 | 8 | 3 |
| 4 | UBO Enhancement | 2 + ALTER | 10 | 8 |

---

## Phase 1: DocumentTypeCode Validation

**Purpose**: Enable compile-time validation of document type codes.

**Changes**:
- Add `DocumentTypeCode` and `DocumentTypeList` to `ArgType` enum
- Load document types from DB at registry startup
- Validate in CSG linter with suggestions for typos

---

## Phase 2: Threshold Decision Matrix

**Purpose**: Compute KYC requirements dynamically based on CBU risk factors.

**Database Tables**:
- `threshold_factors` - Risk factors (CBU_TYPE, SOURCE_OF_FUNDS, etc.)
- `risk_bands` - LOW/MEDIUM/HIGH/ENHANCED mapping
- `threshold_requirements` - Requirements per role + risk band
- `requirement_acceptable_docs` - Acceptable doc types per requirement
- `screening_requirements` - Screening requirements per risk band

**Verbs**:
- `threshold.derive` - Compute requirements from risk factors
- `threshold.evaluate` - Check CBU meets requirements
- `threshold.check-entity` - Check single entity

---

## Phase 3: RFI System

**Purpose**: Formal Request for Information workflow for document collection.

**Database Tables**:
- `rfis` - RFI master (DRAFT/SENT/PARTIAL/COMPLETE)
- `rfi_items` - Line items per entity/attribute
- `rfi_item_acceptable_docs` - Acceptable doc types per item
- `rfi_item_documents` - Documents received
- `rfi_delivery_log` - Delivery audit trail

**Verbs**:
- `rfi.generate` - Auto-generate from threshold gaps
- `rfi.create` - Manual creation
- `rfi.request-document` - Add item to RFI
- `rfi.finalize` - Lock for sending
- `rfi.receive` - Record document received
- `rfi.close` - Close RFI
- `rfi.list-by-case`, `rfi.get-items` - Queries

---

## Phase 4: UBO Enhancement

**Purpose**: Incremental UBO discovery, versioning, and snapshots.

**Database Changes**:
- ALTER `ubo_registry`: Add case_id, workstream_id, discovery_method, superseded_by/at, closed_at/reason
- NEW `ubo_snapshots` - Point-in-time snapshots
- NEW `ubo_snapshot_comparisons` - Comparison results
- NEW SQL function `compute_ownership_chains()`

**New Verbs (from SPEC Section 8)**:
- `ubo.discover-owner` - Record discovered ownership
- `ubo.infer-chain` - Compute ownership chains
- `ubo.trace-chains` - Return chain structure
- `ubo.check-completeness` - Completeness check
- `ubo.supersede-ubo` - Mark as superseded
- `ubo.close-ubo` - Close determination
- `ubo.snapshot-cbu` - Create snapshot
- `ubo.compare-snapshot` - Compare snapshots
- `ubo.list-snapshots` - List snapshots

**Updated Verbs**:
- `ubo.register-ubo` - Add case-id, workstream-id, discovery-method args

---

## Implementation Order

### Week 1: Foundation
1. Phase 1 - DocumentTypeCode validation
2. Phase 2 - Threshold schema + seeds
3. Phase 2 - threshold.derive plugin

### Week 2: Evaluation + RFI
4. Phase 2 - threshold.evaluate plugin
5. Phase 3 - RFI schema
6. Phase 3 - RFI CRUD verbs + plugins

### Week 3: UBO Enhancement
7. Phase 4 - UBO schema changes
8. Phase 4 - compute_ownership_chains() SQL function
9. Phase 4 - Discovery verbs
10. Phase 4 - Lifecycle verbs
11. Phase 4 - Snapshot verbs

### Week 4: Integration Testing
12. End-to-end scenarios

---

## File Checklist

### SQL Migrations
- [ ] sql/migrations/009_threshold_matrix.sql
- [ ] sql/migrations/010_rfi_system.sql
- [ ] sql/migrations/011_ubo_enhancements.sql

### SQL Seeds
- [ ] sql/seeds/threshold_matrix.sql

### Rust Code
- [ ] rust/src/dsl_v2/config/types.rs - DocumentTypeCode
- [ ] rust/src/dsl_v2/csg_linter.rs - validation
- [ ] rust/src/dsl_v2/custom_ops/threshold.rs
- [ ] rust/src/dsl_v2/custom_ops/rfi.rs
- [ ] rust/src/dsl_v2/custom_ops/ubo_analysis.rs
- [ ] rust/src/dsl_v2/custom_ops/mod.rs - register ops
- [ ] rust/config/verbs.yaml - all new domains

---

## Key Design Decisions

1. **Threshold is data-driven**: Risk bands and requirements in DB, not hardcoded
2. **RFI integrates with threshold**: `rfi.generate` takes gaps from `threshold.evaluate`
3. **UBO discovery vs assertion**: `discovery_method` field distinguishes upfront vs found
4. **Snapshots enable periodic review**: Compare baseline to current for material changes
5. **All new UBO verbs link to case**: Traceability for audit

---

## Alignment with SPEC Document

The SPEC (docs_KYC_UBO_DSL_SPEC.md) defines the UBO grammar in Section 8:
- Discovery verbs (8.1): discover-owner, infer-chain
- Versioning verbs (8.2): supersede-ubo, close-ubo
- Snapshot verbs (8.3): snapshot-cbu, compare-snapshots

This plan implements all SPEC-defined verbs plus additional features from the TODO:
- Threshold domain (not in SPEC)
- RFI domain (not in SPEC)
- Additional UBO analysis verbs (trace-chains, check-completeness)

**Recommendation**: Update the SPEC document to include threshold and RFI domains.
