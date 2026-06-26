package com.ob.poc.cbu.db;

import com.ob.poc.cbu.model.*;

import java.sql.Connection;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.SQLException;
import java.sql.Timestamp;
import java.sql.Date;
import java.time.Instant;
import java.time.LocalDate;
import java.util.UUID;
import java.util.List;
import java.util.ArrayList;

public final class CbuReadProjection {

    private CbuReadProjection() {}

    public static CbuDto read(Connection conn, UUID cbuId) throws SQLException {
        String sql = "SELECT cbu_id, name, description, nature_purpose, client_type, jurisdiction, " +
                     "cbu_category, status, operational_status, disposition_status, cbu_discovery_state, " +
                     "created_at, updated_at " +
                     "FROM \"ob-poc\".cbus WHERE cbu_id = ? AND deleted_at IS NULL";
        
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setObject(1, cbuId);
            try (ResultSet rs = ps.executeQuery()) {
                if (rs.next()) {
                    return mapRow(rs);
                }
            }
        }
        return null;
    }

    public static List<CbuDto> list(
        Connection conn,
        String status,
        String clientType,
        String jurisdiction,
        Integer limit,
        Integer offset
    ) throws SQLException {
        StringBuilder sql = new StringBuilder("SELECT cbu_id, name, description, nature_purpose, client_type, jurisdiction, " +
                     "cbu_category, status, operational_status, disposition_status, cbu_discovery_state, " +
                     "created_at, updated_at " +
                     "FROM \"ob-poc\".cbus WHERE deleted_at IS NULL");
        List<Object> binds = new ArrayList<>();
        if (status != null) {
            sql.append(" AND status = ?");
            binds.add(status);
        }
        if (clientType != null) {
            sql.append(" AND client_type = ?");
            binds.add(clientType);
        }
        if (jurisdiction != null) {
            sql.append(" AND jurisdiction = ?");
            binds.add(jurisdiction);
        }
        sql.append(" ORDER BY name");
        if (limit != null) {
            sql.append(" LIMIT ?");
            binds.add(limit);
        } else {
            sql.append(" LIMIT 100");
        }
        if (offset != null) {
            sql.append(" OFFSET ?");
            binds.add(offset);
        }

        List<CbuDto> list = new ArrayList<>();
        try (PreparedStatement ps = conn.prepareStatement(sql.toString())) {
            for (int i = 0; i < binds.size(); i++) {
                ps.setObject(i + 1, binds.get(i));
            }
            try (ResultSet rs = ps.executeQuery()) {
                while (rs.next()) {
                    list.add(mapRow(rs));
                }
            }
        }
        return list;
    }

    public static CbuInspectDto inspect(Connection conn, UUID cbuId, LocalDate asOfDate) throws SQLException {
        LocalDate date = (asOfDate == null) ? LocalDate.now() : asOfDate;
        
        // 1. Fetch CBU details
        String cbuSql = "SELECT cbu_id, name, jurisdiction, client_type, cbu_category, " +
                        "nature_purpose, description, created_at, updated_at " +
                        "FROM \"ob-poc\".cbus WHERE cbu_id = ? AND deleted_at IS NULL";
        
        UUID id = null;
        String name = null;
        String jurisdiction = null;
        String clientType = null;
        String category = null;
        String naturePurpose = null;
        String description = null;
        String createdAt = null;
        String updatedAt = null;
        
        try (PreparedStatement ps = conn.prepareStatement(cbuSql)) {
            ps.setObject(1, cbuId);
            try (ResultSet rs = ps.executeQuery()) {
                if (rs.next()) {
                    id = (UUID) rs.getObject("cbu_id");
                    name = rs.getString("name");
                    jurisdiction = rs.getString("jurisdiction");
                    clientType = rs.getString("client_type");
                    category = rs.getString("cbu_category");
                    naturePurpose = rs.getString("nature_purpose");
                    description = rs.getString("description");
                    Timestamp cat = rs.getTimestamp("created_at");
                    createdAt = cat != null ? cat.toInstant().toString() : null;
                    Timestamp uat = rs.getTimestamp("updated_at");
                    updatedAt = uat != null ? uat.toInstant().toString() : null;
                } else {
                    return null;
                }
            }
        }
        
        // 2. Fetch entities
        String entitiesSql = 
            "SELECT DISTINCT e.entity_id, e.name, et.type_code as entity_type, " +
            "COALESCE(lc.jurisdiction, pp.nationality, p.jurisdiction, t.jurisdiction) as jurisdiction " +
            "FROM \"ob-poc\".cbu_entity_roles cer " +
            "JOIN \"ob-poc\".entities e ON cer.entity_id = e.entity_id " +
            "JOIN \"ob-poc\".entity_types et ON e.entity_type_id = et.entity_type_id " +
            "LEFT JOIN \"ob-poc\".entity_limited_companies lc ON e.entity_id = lc.entity_id " +
            "LEFT JOIN \"ob-poc\".entity_proper_persons pp ON e.entity_id = pp.entity_id " +
            "LEFT JOIN \"ob-poc\".entity_partnerships p ON e.entity_id = p.entity_id " +
            "LEFT JOIN \"ob-poc\".entity_trusts t ON e.entity_id = t.entity_id " +
            "WHERE cer.cbu_id = ? AND e.deleted_at IS NULL " +
            "  AND (cer.effective_from IS NULL OR cer.effective_from <= ?) " +
            "  AND (cer.effective_to IS NULL OR cer.effective_to >= ?) " +
            "ORDER BY e.name";
            
        List<CbuInspectDto.EntityDetail> entities = new ArrayList<>();
        try (PreparedStatement ps = conn.prepareStatement(entitiesSql)) {
            ps.setObject(1, cbuId);
            ps.setObject(2, Date.valueOf(date));
            ps.setObject(3, Date.valueOf(date));
            try (ResultSet rs = ps.executeQuery()) {
                while (rs.next()) {
                    UUID entityId = (UUID) rs.getObject("entity_id");
                    String entityName = rs.getString("name");
                    String entityType = rs.getString("entity_type");
                    String entityJurisdiction = rs.getString("jurisdiction");
                    entities.add(new CbuInspectDto.EntityDetail(entityId, entityName, entityType, entityJurisdiction, new ArrayList<>()));
                }
            }
        }
        
        // 3. Fetch roles and bind to entities
        String rolesSql = 
            "SELECT cer.entity_id, r.name as role_name " +
            "FROM \"ob-poc\".cbu_entity_roles cer " +
            "JOIN \"ob-poc\".roles r ON cer.role_id = r.role_id " +
            "WHERE cer.cbu_id = ? " +
            "  AND (cer.effective_from IS NULL OR cer.effective_from <= ?) " +
            "  AND (cer.effective_to IS NULL OR cer.effective_to >= ?) " +
            "ORDER BY cer.entity_id, r.name";
            
        try (PreparedStatement ps = conn.prepareStatement(rolesSql)) {
            ps.setObject(1, cbuId);
            ps.setObject(2, Date.valueOf(date));
            ps.setObject(3, Date.valueOf(date));
            try (ResultSet rs = ps.executeQuery()) {
                while (rs.next()) {
                    UUID entityId = (UUID) rs.getObject("entity_id");
                    String roleName = rs.getString("role_name");
                    for (CbuInspectDto.EntityDetail entity : entities) {
                        if (entity.entityId().equals(entityId)) {
                            entity.roles().add(roleName);
                            break;
                        }
                    }
                }
            }
        }
        
        // 4. Fetch documents
        String docsSql = 
            "SELECT dc.doc_id, dc.document_name, dt.type_code, dt.display_name, dc.status " +
            "FROM \"ob-poc\".document_catalog dc " +
            "LEFT JOIN \"ob-poc\".document_types dt ON dc.document_type_id = dt.type_id " +
            "WHERE dc.cbu_id = ? ORDER BY dt.type_code";
            
        List<CbuInspectDto.DocumentDetail> documents = new ArrayList<>();
        try (PreparedStatement ps = conn.prepareStatement(docsSql)) {
            ps.setObject(1, cbuId);
            try (ResultSet rs = ps.executeQuery()) {
                while (rs.next()) {
                    UUID docId = (UUID) rs.getObject("doc_id");
                    String docName = rs.getString("document_name");
                    String typeCode = rs.getString("type_code");
                    String displayName = rs.getString("display_name");
                    String status = rs.getString("status");
                    documents.add(new CbuInspectDto.DocumentDetail(docId, docName, typeCode, displayName, status));
                }
            }
        }
        
        // 5. Fetch services
        String servicesSql = 
            "SELECT sdm.delivery_id, p.name as product_name, p.product_code, " +
            "s.name as service_name, sdm.delivery_status " +
            "FROM \"ob-poc\".service_delivery_map sdm " +
            "JOIN \"ob-poc\".products p ON p.product_id = sdm.product_id " +
            "JOIN \"ob-poc\".services s ON s.service_id = sdm.service_id " +
            "WHERE sdm.cbu_id = ? ORDER BY p.name, s.name";
            
        List<CbuInspectDto.ServiceDetail> services = new ArrayList<>();
        try (PreparedStatement ps = conn.prepareStatement(servicesSql)) {
            ps.setObject(1, cbuId);
            try (ResultSet rs = ps.executeQuery()) {
                while (rs.next()) {
                    UUID deliveryId = (UUID) rs.getObject("delivery_id");
                    String productName = rs.getString("product_name");
                    String productCode = rs.getString("product_code");
                    String serviceName = rs.getString("service_name");
                    String status = rs.getString("delivery_status");
                    services.add(new CbuInspectDto.ServiceDetail(deliveryId, productName, productCode, serviceName, status));
                }
            }
        }
        
        CbuInspectDto.Summary summary = new CbuInspectDto.Summary(entities.size(), documents.size(), services.size());
        
        return new CbuInspectDto(
            id, name, jurisdiction, clientType, category, naturePurpose, description,
            createdAt, updatedAt, date.toString(), entities, documents, services, summary
        );
    }

    public static List<CbuPartyDto> parties(Connection conn, UUID cbuId, LocalDate asOfDate) throws SQLException {
        LocalDate date = (asOfDate == null) ? LocalDate.now() : asOfDate;
        String sql = 
            "SELECT cer.cbu_id, cer.entity_id, cer.role_id, " +
            "e.name as entity_name, et.name as entity_type, r.name as role_name, " +
            "cer.effective_from, cer.effective_to " +
            "FROM \"ob-poc\".cbu_entity_roles cer " +
            "JOIN \"ob-poc\".entities e ON e.entity_id = cer.entity_id " +
            "JOIN \"ob-poc\".entity_types et ON et.entity_type_id = e.entity_type_id " +
            "JOIN \"ob-poc\".roles r ON r.role_id = cer.role_id " +
            "WHERE cer.cbu_id = ? " +
            "AND (cer.effective_from IS NULL OR cer.effective_from <= ?) " +
            "AND (cer.effective_to IS NULL OR cer.effective_to >= ?) " +
            "AND e.deleted_at IS NULL " +
            "ORDER BY e.name, r.name";

        List<CbuPartyDto> list = new ArrayList<>();
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setObject(1, cbuId);
            ps.setObject(2, Date.valueOf(date));
            ps.setObject(3, Date.valueOf(date));
            try (ResultSet rs = ps.executeQuery()) {
                while (rs.next()) {
                    UUID cid = (UUID) rs.getObject("cbu_id");
                    UUID eid = (UUID) rs.getObject("entity_id");
                    UUID rid = (UUID) rs.getObject("role_id");
                    String entityName = rs.getString("entity_name");
                    String entityType = rs.getString("entity_type");
                    String roleName = rs.getString("role_name");
                    
                    Date fromDateVal = rs.getDate("effective_from");
                    LocalDate effectiveFrom = fromDateVal != null ? fromDateVal.toLocalDate() : null;
                    
                    Date toDateVal = rs.getDate("effective_to");
                    LocalDate effectiveTo = toDateVal != null ? toDateVal.toLocalDate() : null;
                    
                    list.add(new CbuPartyDto(cid, eid, rid, entityName, entityType, roleName, effectiveFrom, effectiveTo));
                }
            }
        }
        return list;
    }

    public static List<CbuSubscriptionDto> listSubscriptions(Connection conn, UUID cbuId, String product) throws SQLException {
        StringBuilder sql = new StringBuilder(
            "SELECT cbu_id, cbu_name, contract_client, contract_id, product_code, " +
            "subscribed_at, rate_card_id, rate_card_name, rate_card_currency " +
            "FROM \"ob-poc\".v_cbu_subscriptions " +
            "WHERE cbu_id = ?"
        );
        List<Object> binds = new ArrayList<>();
        binds.add(cbuId);
        if (product != null) {
            sql.append(" AND product_code = ?");
            binds.add(product);
        }
        sql.append(" ORDER BY product_code");

        List<CbuSubscriptionDto> list = new ArrayList<>();
        try (PreparedStatement ps = conn.prepareStatement(sql.toString())) {
            for (int i = 0; i < binds.size(); i++) {
                ps.setObject(i + 1, binds.get(i));
            }
            try (ResultSet rs = ps.executeQuery()) {
                while (rs.next()) {
                    UUID cid = (UUID) rs.getObject("cbu_id");
                    String cbuName = rs.getString("cbu_name");
                    String contractClient = rs.getString("contract_client");
                    UUID contractId = (UUID) rs.getObject("contract_id");
                    String productCode = rs.getString("product_code");
                    
                    Timestamp sat = rs.getTimestamp("subscribed_at");
                    Instant subscribedAt = sat != null ? sat.toInstant() : null;
                    
                    UUID rcid = (UUID) rs.getObject("rate_card_id");
                    String rcname = rs.getString("rate_card_name");
                    String rccurrency = rs.getString("rate_card_currency");
                    
                    list.add(new CbuSubscriptionDto(
                        cid, cbuName, contractClient, contractId, productCode,
                        subscribedAt, rcid, rcname, rccurrency
                    ));
                }
            }
        }
        return list;
    }

    public static List<CbuSubscriptionDto> products(Connection conn, UUID cbuId, String product) throws SQLException {
        return listSubscriptions(conn, cbuId, product);
    }

    public static List<CbuEvidenceDto> listEvidence(Connection conn, UUID cbuId, String evidenceType, String verificationStatus) throws SQLException {
        StringBuilder sql = new StringBuilder(
            "SELECT evidence_id, cbu_id, document_id, attestation_ref, evidence_type, " +
            "evidence_category, description, attached_at, attached_by, verified_at, verified_by, " +
            "verification_status, verification_notes " +
            "FROM \"ob-poc\".cbu_evidence " +
            "WHERE cbu_id = ?"
        );
        List<Object> binds = new ArrayList<>();
        binds.add(cbuId);
        if (evidenceType != null) {
            sql.append(" AND evidence_type = ?");
            binds.add(evidenceType);
        }
        if (verificationStatus != null) {
            sql.append(" AND verification_status = ?");
            binds.add(verificationStatus);
        }
        sql.append(" ORDER BY attached_at DESC");

        List<CbuEvidenceDto> list = new ArrayList<>();
        try (PreparedStatement ps = conn.prepareStatement(sql.toString())) {
            for (int i = 0; i < binds.size(); i++) {
                ps.setObject(i + 1, binds.get(i));
            }
            try (ResultSet rs = ps.executeQuery()) {
                while (rs.next()) {
                    UUID evidenceId = (UUID) rs.getObject("evidence_id");
                    UUID cid = (UUID) rs.getObject("cbu_id");
                    UUID documentId = (UUID) rs.getObject("document_id");
                    String attestationRef = rs.getString("attestation_ref");
                    String etype = rs.getString("evidence_type");
                    String ecat = rs.getString("evidence_category");
                    String desc = rs.getString("description");
                    
                    Timestamp aat = rs.getTimestamp("attached_at");
                    Instant attachedAt = aat != null ? aat.toInstant() : null;
                    String attachedBy = rs.getString("attached_by");
                    
                    Timestamp vat = rs.getTimestamp("verified_at");
                    Instant verifiedAt = vat != null ? vat.toInstant() : null;
                    String verifiedBy = rs.getString("verified_by");
                    
                    String vstatus = rs.getString("verification_status");
                    String vnotes = rs.getString("verification_notes");
                    
                    list.add(new CbuEvidenceDto(
                        evidenceId, cid, documentId, attestationRef, etype, ecat, desc,
                        attachedAt, attachedBy, verifiedAt, verifiedBy, vstatus, vnotes
                    ));
                }
            }
        }
        return list;
    }

    public static List<CbuStructureLinkDto> listStructureLinks(
        Connection conn,
        UUID parentCbuId,
        UUID childCbuId,
        UUID cbuId,
        String direction,
        String status
    ) throws SQLException {
        String dir = (direction == null) ? "parent" : direction.trim().toLowerCase();
        if (!dir.equals("parent") && !dir.equals("child")) {
            throw new IllegalArgumentException("cbu.list-structure-links: direction must be 'parent' or 'child'");
        }

        UUID parent = parentCbuId;
        UUID child = childCbuId;
        if (parent == null && child == null && cbuId != null) {
            if (dir.equals("child")) {
                child = cbuId;
            } else {
                parent = cbuId;
            }
        }

        if (parent == null && child == null) {
            throw new IllegalArgumentException("cbu.list-structure-links: one of :cbu-id, :parent-cbu-id or :child-cbu-id is required");
        }

        StringBuilder sql = new StringBuilder(
            "SELECT l.link_id, l.parent_cbu_id, p.name as parent_name, " +
            "l.child_cbu_id, c.name as child_name, l.relationship_type, " +
            "l.relationship_selector, l.status, l.capital_flow, " +
            "l.effective_from, l.effective_to " +
            "FROM \"ob-poc\".cbu_structure_links l " +
            "JOIN \"ob-poc\".cbus p ON p.cbu_id = l.parent_cbu_id " +
            "JOIN \"ob-poc\".cbus c ON c.cbu_id = l.child_cbu_id " +
            "WHERE 1=1"
        );
        List<Object> binds = new ArrayList<>();
        if (parent != null) {
            sql.append(" AND l.parent_cbu_id = ?");
            binds.add(parent);
        }
        if (child != null) {
            sql.append(" AND l.child_cbu_id = ?");
            binds.add(child);
        }
        if (status != null) {
            sql.append(" AND l.status = ?");
            binds.add(status.toUpperCase().trim());
        }
        sql.append(" ORDER BY l.created_at DESC");

        List<CbuStructureLinkDto> list = new ArrayList<>();
        try (PreparedStatement ps = conn.prepareStatement(sql.toString())) {
            for (int i = 0; i < binds.size(); i++) {
                ps.setObject(i + 1, binds.get(i));
            }
            try (ResultSet rs = ps.executeQuery()) {
                while (rs.next()) {
                    UUID linkId = (UUID) rs.getObject("link_id");
                    UUID pid = (UUID) rs.getObject("parent_cbu_id");
                    String parentName = rs.getString("parent_name");
                    UUID cid = (UUID) rs.getObject("child_cbu_id");
                    String childName = rs.getString("child_name");
                    String relationshipType = rs.getString("relationship_type");
                    String relationshipSelector = rs.getString("relationship_selector");
                    String linkStatus = rs.getString("status");
                    String capitalFlow = rs.getString("capital_flow");
                    
                    Date fromDateVal = rs.getDate("effective_from");
                    LocalDate effectiveFrom = fromDateVal != null ? fromDateVal.toLocalDate() : null;
                    
                    Date toDateVal = rs.getDate("effective_to");
                    LocalDate effectiveTo = toDateVal != null ? toDateVal.toLocalDate() : null;
                    
                    list.add(new CbuStructureLinkDto(
                        linkId, pid, parentName, cid, childName, relationshipType,
                        relationshipSelector, linkStatus, capitalFlow, effectiveFrom, effectiveTo
                    ));
                }
            }
        }
        return list;
    }

    public static CbuOptionCoverageDto validateOptionCoverage(
        Connection conn,
        UUID cbuId,
        UUID serviceId,
        String serviceCode,
        UUID serviceVersionId
    ) throws SQLException {
        UUID finalServiceId = serviceId;
        if (finalServiceId == null) {
            if (serviceCode == null) {
                throw new IllegalArgumentException("validateOptionCoverage requires serviceId or serviceCode");
            }
            try (PreparedStatement ps = conn.prepareStatement("SELECT service_id FROM \"ob-poc\".services WHERE service_code = ?")) {
                ps.setString(1, serviceCode);
                try (ResultSet rs = ps.executeQuery()) {
                    if (rs.next()) {
                        finalServiceId = (UUID) rs.getObject("service_id");
                    } else {
                        throw new SQLException("Unknown service: " + serviceCode);
                    }
                }
            }
        }

        UUID finalVersionId = serviceVersionId;
        if (finalVersionId == null) {
            String versionSql = "SELECT id FROM \"ob-poc\".service_versions " +
                                "WHERE service_id = ? AND lifecycle_status = 'published' " +
                                "ORDER BY published_at DESC NULLS LAST, created_at DESC LIMIT 1";
            try (PreparedStatement ps = conn.prepareStatement(versionSql)) {
                ps.setObject(1, finalServiceId);
                try (ResultSet rs = ps.executeQuery()) {
                    if (rs.next()) {
                        finalVersionId = (UUID) rs.getObject("id");
                    } else {
                        throw new SQLException("No published version for service: " + finalServiceId);
                    }
                }
            }
        }

        String query = "SELECT d.option_key FROM \"ob-poc\".service_option_defs d " +
                       "LEFT JOIN \"ob-poc\".cbu_service_option_bindings b " +
                       "  ON b.cbu_id = ? AND b.service_id = d.service_id " +
                       " AND b.service_option_def_id = d.service_option_def_id AND b.valid_to IS NULL " +
                       "WHERE d.service_version_id = ? AND d.lifecycle_status = 'active' " +
                       "  AND d.is_required AND (b.binding_id IS NULL OR b.value::text = 'null') " +
                       "ORDER BY d.option_key";

        List<CbuOptionCoverageDto.Gap> gaps = new ArrayList<>();
        try (PreparedStatement ps = conn.prepareStatement(query)) {
            ps.setObject(1, cbuId);
            ps.setObject(2, finalVersionId);
            try (ResultSet rs = ps.executeQuery()) {
                while (rs.next()) {
                    gaps.add(new CbuOptionCoverageDto.Gap(1, "missing_required_option_binding", rs.getString("option_key")));
                }
            }
        }

        String status = gaps.isEmpty() ? "clean" : "gapped";
        return new CbuOptionCoverageDto(cbuId, finalServiceId, status, gaps);
    }

    private static CbuDto mapRow(ResultSet rs) throws SQLException {
        UUID cbuId = (UUID) rs.getObject("cbu_id");
        String name = rs.getString("name");
        String description = rs.getString("description");
        String naturePurpose = rs.getString("nature_purpose");
        String clientType = rs.getString("client_type");
        String jurisdiction = rs.getString("jurisdiction");
        String category = rs.getString("cbu_category");
        String status = rs.getString("status");
        String operationalStatus = rs.getString("operational_status");
        String dispositionStatus = rs.getString("disposition_status");
        String discoveryState = rs.getString("cbu_discovery_state");
        
        Timestamp createdAtTs = rs.getTimestamp("created_at");
        Instant createdAt = createdAtTs != null ? createdAtTs.toInstant() : null;
        
        Timestamp updatedAtTs = rs.getTimestamp("updated_at");
        Instant updatedAt = updatedAtTs != null ? updatedAtTs.toInstant() : null;

        return new CbuDto(
            cbuId,
            name,
            description,
            naturePurpose,
            clientType,
            jurisdiction,
            category,
            status,
            operationalStatus,
            dispositionStatus,
            discoveryState,
            createdAt,
            updatedAt
        );
    }
}
