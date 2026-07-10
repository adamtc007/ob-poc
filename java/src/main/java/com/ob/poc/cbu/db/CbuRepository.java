package com.ob.poc.cbu.db;

import com.ob.poc.cbu.model.CbuStatus;
import com.ob.poc.cbu.model.OperationalState;
import com.ob.poc.cbu.model.ValidationState;
import com.ob.poc.cbu.model.StructuralState;
import com.ob.poc.cbu.model.DispositionState;
import com.ob.poc.cbu.model.Effect;

import java.sql.*;
import java.util.UUID;
import java.util.List;
import java.util.ArrayList;
import java.util.Map;
import java.util.HashMap;

public final class CbuRepository {

    private CbuRepository() {}

    public static CbuStatus recover(Connection conn, UUID cbuId) throws SQLException {
        String sql = "SELECT cbu_id, name, status, operational_status, disposition_status, deleted_at, client_type, jurisdiction " +
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

    public static CbuStatus recoverByNameAndJurisdiction(Connection conn, String name, String jurisdiction) throws SQLException {
        String sql = "SELECT cbu_id, name, status, operational_status, disposition_status, deleted_at, client_type, jurisdiction " +
                     "FROM \"ob-poc\".cbus WHERE name = ? AND (jurisdiction = ? OR (jurisdiction IS NULL AND ? IS NULL)) AND deleted_at IS NULL";
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setString(1, name);
            ps.setString(2, jurisdiction);
            ps.setString(3, jurisdiction);
            try (ResultSet rs = ps.executeQuery()) {
                if (rs.next()) {
                    return mapRow(rs);
                }
            }
        }
        return null;
    }

    public static boolean isFundLinkedAlready(Connection conn, UUID fundEntityId) throws SQLException {
        String sql = "SELECT 1 FROM \"ob-poc\".cbu_entity_roles cer " +
                     "JOIN \"ob-poc\".cbus c ON c.cbu_id = cer.cbu_id " +
                     "JOIN \"ob-poc\".roles r ON r.role_id = cer.role_id " +
                     "WHERE cer.entity_id = ? AND c.deleted_at IS NULL AND r.name = 'ASSET_OWNER' " +
                     "AND (cer.effective_to IS NULL OR cer.effective_to > CURRENT_DATE) LIMIT 1";
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setObject(1, fundEntityId);
            try (ResultSet rs = ps.executeQuery()) {
                return rs.next();
            }
        }
    }

    public static UUID getLinkedCbuIdForFund(Connection conn, UUID fundEntityId) throws SQLException {
        String sql = "SELECT c.cbu_id FROM \"ob-poc\".cbu_entity_roles cer " +
                     "JOIN \"ob-poc\".cbus c ON c.cbu_id = cer.cbu_id " +
                     "JOIN \"ob-poc\".roles r ON r.role_id = cer.role_id " +
                     "WHERE cer.entity_id = ? AND c.deleted_at IS NULL AND r.name = 'ASSET_OWNER' " +
                     "AND (cer.effective_to IS NULL OR cer.effective_to > CURRENT_DATE) LIMIT 1";
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setObject(1, fundEntityId);
            try (ResultSet rs = ps.executeQuery()) {
                if (rs.next()) {
                    return (UUID) rs.getObject("cbu_id");
                }
            }
        }
        return null;
    }

    public static boolean linkExists(Connection conn, UUID linkId, boolean activeOnly) throws SQLException {
        String sql = activeOnly 
            ? "SELECT 1 FROM \"ob-poc\".cbu_structure_links WHERE link_id = ? AND status = 'ACTIVE'"
            : "SELECT 1 FROM \"ob-poc\".cbu_structure_links WHERE link_id = ?";
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setObject(1, linkId);
            try (ResultSet rs = ps.executeQuery()) {
                return rs.next();
            }
        }
    }

    public static boolean roleExists(Connection conn, UUID cbuId, boolean activeOnly) throws SQLException {
        String sql = activeOnly
            ? "SELECT 1 FROM \"ob-poc\".cbu_entity_roles WHERE cbu_id = ? AND effective_to IS NULL"
            : "SELECT 1 FROM \"ob-poc\".cbu_entity_roles WHERE cbu_id = ?";
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setObject(1, cbuId);
            try (ResultSet rs = ps.executeQuery()) {
                return rs.next();
            }
        }
    }

    public static boolean memberExists(Connection conn, UUID cbuId, boolean activeOnly) throws SQLException {
        String sql = activeOnly
            ? "SELECT 1 FROM \"ob-poc\".cbu_group_members WHERE cbu_id = ? AND effective_to IS NULL"
            : "SELECT 1 FROM \"ob-poc\".cbu_group_members WHERE cbu_id = ?";
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setObject(1, cbuId);
            try (ResultSet rs = ps.executeQuery()) {
                return rs.next();
            }
        }
    }

    public static int applyEffect(Connection conn, Effect effect, List<Object> outEvents) throws SQLException {
        return switch (effect) {
            case Effect.UpdateOperationalStatus e -> applyUpdateOperationalStatus(conn, e);
            case Effect.InsertCbu e -> applyInsertCbu(conn, e, outEvents);
            case Effect.AssignFundRole e -> applyAssignFundRole(conn, e);
            case Effect.LinkCbu e -> applyLinkCbu(conn, e, outEvents);
            case Effect.EmitPendingStateAdvance e -> {
                outEvents.add(e);
                yield 1;
            }
            case Effect.UpdateValidationStatus e -> applyUpdateValidationStatus(conn, e);
            case Effect.UpdateDispositionStatus e -> applyUpdateDispositionStatus(conn, e);
            case Effect.LinkStructure e -> applyLinkStructure(conn, e);
            case Effect.UnlinkStructure e -> applyUnlinkStructure(conn, e);
            case Effect.TerminateRole e -> applyTerminateRole(conn, e);
            case Effect.RemoveMember e -> applyRemoveMember(conn, e);
            case Effect.UpdateCbuName e -> applyUpdateCbuName(conn, e);
            case Effect.UpdateCbuFields e -> applyUpdateCbuFields(conn, e);
            case Effect.UpdateCbuJurisdiction e -> applyUpdateCbuJurisdiction(conn, e);
            case Effect.UpdateCbuClientType e -> applyUpdateCbuClientType(conn, e);
            case Effect.UpdateCbuCommercialClient e -> applyUpdateCbuCommercialClient(conn, e);
            case Effect.UpdateCbuCategory e -> applyUpdateCbuCategory(conn, e);
            case Effect.AddProduct e -> applyAddProduct(conn, e);
            case Effect.RemoveProduct e -> applyRemoveProduct(conn, e);
            case Effect.UpdateCaStatus e -> applyUpdateCaStatus(conn, e);
            case Effect.AssignRoleEffect e -> applyAssignRoleEffect(conn, e, outEvents);
            case Effect.RemoveRoleEffect e -> applyRemoveRoleEffect(conn, e);
            case Effect.AttachEvidence e -> applyAttachEvidence(conn, e, outEvents);
            case Effect.VerifyEvidence e -> applyVerifyEvidence(conn, e);
            case Effect.BindServiceOptions e -> applyBindServiceOptions(conn, e);
            case Effect.OverrideOptionBinding e -> applyOverrideOptionBinding(conn, e, outEvents);
            case Effect.DirtyFlagBindings e -> applyDirtyFlagBindings(conn, e);
            case Effect.RecomputeBindings e -> applyRecomputeBindings(conn, e);
            case Effect.CreateFromClientGroup e -> applyCreateFromClientGroup(conn, e, outEvents);
            case Effect.DeleteCascade e -> applyDeleteCascade(conn, e, outEvents);
            case Effect.EnsureCbu e -> applyEnsureCbu(conn, e, outEvents);
        };
    }

    private static int applyUpdateOperationalStatus(Connection conn, Effect.UpdateOperationalStatus e) throws SQLException {
        String sql = "UPDATE \"ob-poc\".cbus SET operational_status = ?, updated_at = NOW() " +
                     "WHERE cbu_id = ? AND (operational_status = ? OR (operational_status IS NULL AND ? IS NULL))";
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setString(1, e.toStatus());
            ps.setObject(2, e.cbuId());
            ps.setString(3, e.fromStatus());
            ps.setString(4, e.fromStatus());
            return ps.executeUpdate();
        }
    }

    private static int applyUpdateValidationStatus(Connection conn, Effect.UpdateValidationStatus e) throws SQLException {
        String sql = "UPDATE \"ob-poc\".cbus SET status = ?, updated_at = NOW() " +
                     "WHERE cbu_id = ? AND (status = ? OR (status IS NULL AND ? IS NULL))";
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setString(1, e.toStatus());
            ps.setObject(2, e.cbuId());
            ps.setString(3, e.fromStatus());
            ps.setString(4, e.fromStatus());
            return ps.executeUpdate();
        }
    }

    private static int applyUpdateDispositionStatus(Connection conn, Effect.UpdateDispositionStatus e) throws SQLException {
        String sql = "UPDATE \"ob-poc\".cbus SET disposition_status = ?, updated_at = NOW() " +
                     "WHERE cbu_id = ? AND (disposition_status = ? OR (disposition_status IS NULL AND ? IS NULL))";
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setString(1, e.toStatus());
            ps.setObject(2, e.cbuId());
            ps.setString(3, e.fromStatus());
            ps.setString(4, e.fromStatus());
            return ps.executeUpdate();
        }
    }

    private static int applyInsertCbu(Connection conn, Effect.InsertCbu e, List<Object> outEvents) throws SQLException {
        String sql = "INSERT INTO \"ob-poc\".cbus (cbu_id, name, jurisdiction, client_type, nature_purpose, description, commercial_client_entity_id) " +
                     "VALUES (?, ?, ?, ?, ?, ?, ?) " +
                     "ON CONFLICT (name, jurisdiction) " +
                     "DO UPDATE SET updated_at = NOW() " +
                     "RETURNING cbu_id, (xmax = 0) AS is_insert";
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            UUID id = e.cbuId() != null ? e.cbuId() : UUID.randomUUID();
            ps.setObject(1, id);
            ps.setString(2, e.name());
            ps.setString(3, e.jurisdiction());
            ps.setString(4, e.clientType());
            ps.setString(5, e.naturePurpose());
            ps.setString(6, e.description());
            ps.setObject(7, e.commercialClientEntityId());

            try (ResultSet rs = ps.executeQuery()) {
                if (rs.next()) {
                    UUID resultId = (UUID) rs.getObject("cbu_id");
                    boolean isInsert = rs.getBoolean("is_insert");
                    outEvents.add(new InsertResult(resultId, isInsert));
                    return 1;
                }
            }
        }
        return 0;
    }

    public record InsertResult(UUID cbuId, boolean created) {}

    private static int applyAssignFundRole(Connection conn, Effect.AssignFundRole e) throws SQLException {
        UUID roleId = null;
        String roleSql = "SELECT role_id FROM \"ob-poc\".roles WHERE name = ?";
        try (PreparedStatement ps = conn.prepareStatement(roleSql)) {
            ps.setString(1, e.role());
            try (ResultSet rs = ps.executeQuery()) {
                if (rs.next()) {
                    roleId = (UUID) rs.getObject("role_id");
                }
            }
        }
        if (roleId == null) {
            throw new SQLException("Unknown role: " + e.role());
        }

        String sql = "INSERT INTO \"ob-poc\".cbu_entity_roles (cbu_id, entity_id, role_id) " +
                     "VALUES (?, ?, ?) " +
                     "ON CONFLICT (cbu_id, entity_id, role_id) DO NOTHING";
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setObject(1, e.cbuId());
            ps.setObject(2, e.entityId());
            ps.setObject(3, roleId);
            return ps.executeUpdate();
        }
    }

    private static int applyLinkCbu(Connection conn, Effect.LinkCbu e, List<Object> outEvents) throws SQLException {
        String sql = "UPDATE \"ob-poc\".client_group_entity " +
                     "SET cbu_id = ?, updated_at = NOW() " +
                     "WHERE entity_id = ? " +
                     "  AND membership_type NOT IN ('historical', 'rejected') " +
                     "  AND cbu_id IS NULL";
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setObject(1, e.cbuId());
            ps.setObject(2, e.entityId());
            int affected = ps.executeUpdate();
            if (affected > 0) {
                outEvents.add(new Effect.EmitPendingStateAdvance(
                    e.entityId(),
                    "client-group-membership:cbu-linked",
                    "client-group/membership",
                    "client-group.link-cbu — entity " + e.entityId() + " linked to CBU " + e.cbuId()
                ));
            }
            return affected;
        }
    }

    private static CbuStatus mapRow(ResultSet rs) throws SQLException {
        UUID id = (UUID) rs.getObject("cbu_id");
        String name = rs.getString("name");
        String statusStr = rs.getString("status");
        String opStatusStr = rs.getString("operational_status");
        String dispStatusStr = rs.getString("disposition_status");
        Timestamp deletedAt = rs.getTimestamp("deleted_at");
        String clientType = rs.getString("client_type");
        String jurisdiction = rs.getString("jurisdiction");

        StructuralState struct;
        ValidationState val;
        if (statusStr == null || statusStr.trim().isEmpty()) {
            struct = new StructuralState.Structured();
            val = null;
        } else {
            String upperStatus = statusStr.toUpperCase().trim();
            switch (upperStatus) {
                case "DISCOVERED" -> {
                    struct = new StructuralState.Discovered();
                    val = null;
                }
                case "CONFIGURING" -> {
                    struct = new StructuralState.Configuring();
                    val = null;
                }
                case "VALIDATION_PENDING" -> {
                    struct = new StructuralState.Structured();
                    val = new ValidationState.ValidationPending();
                }
                case "VALIDATED" -> {
                    struct = new StructuralState.Structured();
                    val = new ValidationState.Validated();
                }
                case "VALIDATION_FAILED" -> {
                    struct = new StructuralState.Structured();
                    val = new ValidationState.ValidationFailed();
                }
                case "UPDATE_PENDING_PROOF" -> {
                    struct = new StructuralState.Structured();
                    val = new ValidationState.UpdatePendingProof();
                }
                case "EVIDENCED" -> {
                    struct = new StructuralState.Structured();
                    val = new ValidationState.Evidenced();
                }
                default -> throw new IllegalArgumentException("Unexpected status value: " + statusStr);
            }
        }

        OperationalState op;
        if (opStatusStr == null || opStatusStr.trim().isEmpty()) {
            op = new OperationalState.PreValidated();
        } else {
            op = switch (opStatusStr.toLowerCase().trim()) {
                case "actively_trading", "trade_permissioned" -> new OperationalState.OperationallyActive();
                case "suspended" -> new OperationalState.Suspended();
                case "restricted" -> new OperationalState.Restricted();
                case "winding_down" -> new OperationalState.WindingDown();
                case "offboarded" -> new OperationalState.Offboarded();
                case "dormant" -> new OperationalState.Dormant();
                case "archived" -> new OperationalState.Archived();
                default -> throw new IllegalArgumentException("Unexpected operational status value: " + opStatusStr);
            };
        }

        DispositionState disposition;
        if (dispStatusStr == null || dispStatusStr.trim().isEmpty()) {
            disposition = new DispositionState.Active();
        } else {
            String lowerDisp = dispStatusStr.toLowerCase().trim();
            disposition = switch (lowerDisp) {
                case "active" -> new DispositionState.Active();
                case "under_remediation" -> new DispositionState.UnderRemediation();
                case "soft_deleted" -> new DispositionState.SoftDeleted();
                case "hard_deleted" -> new DispositionState.HardDeleted();
                default -> throw new IllegalArgumentException("Unexpected disposition status value: " + dispStatusStr);
            };
        }
        if (deletedAt != null) {
            disposition = new DispositionState.SoftDeleted();
        }

        return new CbuStatus(id, name, op, val, struct, disposition, clientType, jurisdiction, statusStr, opStatusStr, dispStatusStr);
    }

    private static int applyLinkStructure(Connection conn, Effect.LinkStructure e) throws SQLException {
        if (e.existingLinkId() != null) {
            String sql = "UPDATE \"ob-poc\".cbu_structure_links " +
                         "SET relationship_selector = ?, capital_flow = ?, effective_from = ?, effective_to = ?, status = 'ACTIVE', updated_at = NOW() " +
                         "WHERE link_id = ?";
            try (PreparedStatement ps = conn.prepareStatement(sql)) {
                ps.setString(1, e.relationshipSelector());
                ps.setString(2, e.capitalFlow());
                ps.setObject(3, e.effectiveFrom() != null ? java.sql.Date.valueOf(e.effectiveFrom()) : null);
                ps.setObject(4, e.effectiveTo() != null ? java.sql.Date.valueOf(e.effectiveTo()) : null);
                ps.setObject(5, e.existingLinkId());
                return ps.executeUpdate();
            }
        } else {
            String sql = "INSERT INTO \"ob-poc\".cbu_structure_links (parent_cbu_id, child_cbu_id, relationship_type, relationship_selector, status, capital_flow, effective_from, effective_to) " +
                         "VALUES (?, ?, ?, ?, 'ACTIVE', ?, ?, ?)";
            try (PreparedStatement ps = conn.prepareStatement(sql)) {
                ps.setObject(1, e.parentCbuId());
                ps.setObject(2, e.childCbuId());
                ps.setString(3, e.relationshipType());
                ps.setString(4, e.relationshipSelector());
                ps.setString(5, e.capitalFlow());
                ps.setObject(6, e.effectiveFrom() != null ? java.sql.Date.valueOf(e.effectiveFrom()) : null);
                ps.setObject(7, e.effectiveTo() != null ? java.sql.Date.valueOf(e.effectiveTo()) : null);
                return ps.executeUpdate();
            }
        }
    }

    private static int applyUnlinkStructure(Connection conn, Effect.UnlinkStructure e) throws SQLException {
        if (e.hardDelete()) {
            String sql = "DELETE FROM \"ob-poc\".cbu_structure_links WHERE link_id = ?";
            try (PreparedStatement ps = conn.prepareStatement(sql)) {
                ps.setObject(1, e.linkId());
                return ps.executeUpdate();
            }
        } else {
            String sql = "UPDATE \"ob-poc\".cbu_structure_links " +
                         "SET status = 'TERMINATED', terminated_at = NOW(), terminated_reason = ?, updated_at = NOW() " +
                         "WHERE link_id = ? AND status = 'ACTIVE'";
            try (PreparedStatement ps = conn.prepareStatement(sql)) {
                ps.setString(1, e.reason());
                ps.setObject(2, e.linkId());
                return ps.executeUpdate();
            }
        }
    }

    private static int applyTerminateRole(Connection conn, Effect.TerminateRole e) throws SQLException {
        if (e.hardDelete()) {
            String sql = "DELETE FROM \"ob-poc\".cbu_entity_roles WHERE cbu_id = ?";
            try (PreparedStatement ps = conn.prepareStatement(sql)) {
                ps.setObject(1, e.cbuId());
                return ps.executeUpdate();
            }
        } else {
            String sql = "UPDATE \"ob-poc\".cbu_entity_roles " +
                         "SET effective_to = CURRENT_DATE, updated_at = NOW() " +
                         "WHERE cbu_id = ? AND effective_to IS NULL";
            try (PreparedStatement ps = conn.prepareStatement(sql)) {
                ps.setObject(1, e.cbuId());
                return ps.executeUpdate();
            }
        }
    }

    private static int applyRemoveMember(Connection conn, Effect.RemoveMember e) throws SQLException {
        if (e.hardDelete()) {
            String sql = "DELETE FROM \"ob-poc\".cbu_group_members WHERE cbu_id = ?";
            try (PreparedStatement ps = conn.prepareStatement(sql)) {
                ps.setObject(1, e.cbuId());
                return ps.executeUpdate();
            }
        } else {
            String sql = "UPDATE \"ob-poc\".cbu_group_members " +
                         "SET effective_to = CURRENT_DATE " +
                         "WHERE cbu_id = ? AND effective_to IS NULL";
            try (PreparedStatement ps = conn.prepareStatement(sql)) {
                ps.setObject(1, e.cbuId());
                return ps.executeUpdate();
            }
        }
    }

    private static int applyUpdateCbuName(Connection conn, Effect.UpdateCbuName e) throws SQLException {
        String sql = "UPDATE \"ob-poc\".cbus SET name = ?, updated_at = NOW() WHERE cbu_id = ?";
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setString(1, e.name());
            ps.setObject(2, e.cbuId());
            return ps.executeUpdate();
        }
    }

    private static int applyUpdateCbuFields(Connection conn, Effect.UpdateCbuFields e) throws SQLException {
        String sql = "UPDATE \"ob-poc\".cbus SET description = ?, nature_purpose = ?, cbu_category = ?, updated_at = NOW() WHERE cbu_id = ?";
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setString(1, e.description());
            ps.setString(2, e.naturePurpose());
            ps.setString(3, e.category());
            ps.setObject(4, e.cbuId());
            return ps.executeUpdate();
        }
    }

    private static int applyUpdateCbuJurisdiction(Connection conn, Effect.UpdateCbuJurisdiction e) throws SQLException {
        String sql = "UPDATE \"ob-poc\".cbus SET jurisdiction = ?, updated_at = NOW() WHERE cbu_id = ?";
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setString(1, e.jurisdiction());
            ps.setObject(2, e.cbuId());
            return ps.executeUpdate();
        }
    }

    private static int applyUpdateCbuClientType(Connection conn, Effect.UpdateCbuClientType e) throws SQLException {
        String sql = "UPDATE \"ob-poc\".cbus SET client_type = ?, updated_at = NOW() WHERE cbu_id = ?";
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setString(1, e.clientType());
            ps.setObject(2, e.cbuId());
            return ps.executeUpdate();
        }
    }

    private static int applyUpdateCbuCommercialClient(Connection conn, Effect.UpdateCbuCommercialClient e) throws SQLException {
        String sql = "UPDATE \"ob-poc\".cbus SET commercial_client_entity_id = ?, updated_at = NOW() WHERE cbu_id = ?";
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setObject(1, e.commercialClientEntityId());
            ps.setObject(2, e.cbuId());
            return ps.executeUpdate();
        }
    }

    private static int applyUpdateCbuCategory(Connection conn, Effect.UpdateCbuCategory e) throws SQLException {
        String sql = "UPDATE \"ob-poc\".cbus SET cbu_category = ?, updated_at = NOW() WHERE cbu_id = ?";
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setString(1, e.category());
            ps.setObject(2, e.cbuId());
            return ps.executeUpdate();
        }
    }

    private static int applyAddProduct(Connection conn, Effect.AddProduct e) throws SQLException {
        UUID productId = null;
        String prodSql = "SELECT product_id FROM \"ob-poc\".products WHERE product_code = ? OR name = ? LIMIT 1";
        try (PreparedStatement ps = conn.prepareStatement(prodSql)) {
            ps.setString(1, e.product());
            ps.setString(2, e.product());
            try (ResultSet rs = ps.executeQuery()) {
                if (rs.next()) {
                    productId = (UUID) rs.getObject("product_id");
                }
            }
        }
        if (productId == null) {
            throw new SQLException("Product not found: " + e.product());
        }

        String subSql = "INSERT INTO \"ob-poc\".cbu_product_subscriptions (cbu_id, product_id, status, config) " +
                        "VALUES (?, ?, 'ACTIVE', ?::jsonb) " +
                        "ON CONFLICT (cbu_id, product_id) DO UPDATE SET status = 'ACTIVE', effective_to = NULL, updated_at = NOW()";
        try (PreparedStatement ps = conn.prepareStatement(subSql)) {
            ps.setObject(1, e.cbuId());
            ps.setObject(2, productId);
            ps.setString(3, e.configJson() != null ? e.configJson() : "{}");
            ps.executeUpdate();
        }

        String svcSql = "SELECT service_id FROM \"ob-poc\".product_services WHERE product_id = ?";
        List<UUID> services = new ArrayList<>();
        try (PreparedStatement ps = conn.prepareStatement(svcSql)) {
            ps.setObject(1, productId);
            try (ResultSet rs = ps.executeQuery()) {
                while (rs.next()) {
                    services.add((UUID) rs.getObject("service_id"));
                }
            }
        }

        int inserted = 0;
        String checkSql = "SELECT 1 FROM \"ob-poc\".service_delivery_map WHERE cbu_id = ? AND service_id = ?";
        String insSvcSql = "INSERT INTO \"ob-poc\".service_delivery_map (delivery_id, cbu_id, product_id, service_id, delivery_status, service_config, requested_at, created_at, updated_at) " +
                           "VALUES (?, ?, ?, ?, 'DELIVERED', '{}'::jsonb, NOW(), NOW(), NOW())";
        for (UUID serviceId : services) {
            boolean exists = false;
            try (PreparedStatement ps = conn.prepareStatement(checkSql)) {
                ps.setObject(1, e.cbuId());
                ps.setObject(2, serviceId);
                try (ResultSet rs = ps.executeQuery()) {
                    exists = rs.next();
                }
            }
            if (!exists) {
                try (PreparedStatement ps = conn.prepareStatement(insSvcSql)) {
                    ps.setObject(1, UUID.randomUUID());
                    ps.setObject(2, e.cbuId());
                    ps.setObject(3, productId);
                    ps.setObject(4, serviceId);
                    ps.executeUpdate();
                    inserted++;
                }
            }
        }
        return 1;
    }

    private static int applyRemoveProduct(Connection conn, Effect.RemoveProduct e) throws SQLException {
        UUID productId = null;
        String prodSql = "SELECT product_id FROM \"ob-poc\".products WHERE product_code = ? OR name = ? LIMIT 1";
        try (PreparedStatement ps = conn.prepareStatement(prodSql)) {
            ps.setString(1, e.product());
            ps.setString(2, e.product());
            try (ResultSet rs = ps.executeQuery()) {
                if (rs.next()) {
                    productId = (UUID) rs.getObject("product_id");
                }
            }
        }
        if (productId == null) {
            return 0;
        }

        String delSql = "DELETE FROM \"ob-poc\".service_delivery_map WHERE cbu_id = ? AND product_id = ?";
        int affected = 0;
        try (PreparedStatement ps = conn.prepareStatement(delSql)) {
            ps.setObject(1, e.cbuId());
            ps.setObject(2, productId);
            affected += ps.executeUpdate();
        }

        String termSql = "UPDATE \"ob-poc\".cbu_product_subscriptions SET status = 'TERMINATED', effective_to = CURRENT_DATE, updated_at = NOW() " +
                         "WHERE cbu_id = ? AND product_id = ? AND status = 'ACTIVE'";
        try (PreparedStatement ps = conn.prepareStatement(termSql)) {
            ps.setObject(1, e.cbuId());
            ps.setObject(2, productId);
            affected += ps.executeUpdate();
        }

        return affected;
    }

    public static UUID existingLinkId(Connection conn, UUID parentCbuId, UUID childCbuId, String relationshipType) throws SQLException {
        String sql = "SELECT link_id FROM \"ob-poc\".cbu_structure_links " +
                     "WHERE parent_cbu_id = ? AND child_cbu_id = ? AND relationship_type = ? AND status = 'ACTIVE' LIMIT 1";
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setObject(1, parentCbuId);
            ps.setObject(2, childCbuId);
            ps.setString(3, relationshipType);
            try (ResultSet rs = ps.executeQuery()) {
                if (rs.next()) {
                    return (UUID) rs.getObject("link_id");
                }
            }
        }
        return null;
    }

    public static String recoverCaStatus(Connection conn, UUID eventId) throws SQLException {
        String sql = "SELECT ca_status FROM \"ob-poc\".cbu_corporate_action_events WHERE event_id = ?";
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setObject(1, eventId);
            try (ResultSet rs = ps.executeQuery()) {
                if (rs.next()) {
                    return rs.getString("ca_status");
                }
            }
        }
        return null;
    }

    private static int applyUpdateCaStatus(Connection conn, Effect.UpdateCaStatus e) throws SQLException {
        if (e.fromStatus() == null) {
            String sql = "UPDATE \"ob-poc\".cbu_corporate_action_events SET ca_status = ?, rejected_reason = ?, updated_at = NOW() WHERE event_id = ?";
            try (PreparedStatement ps = conn.prepareStatement(sql)) {
                ps.setString(1, e.toStatus());
                ps.setString(2, e.rejectedReason());
                ps.setObject(3, e.eventId());
                return ps.executeUpdate();
            }
        } else {
            String sql = "UPDATE \"ob-poc\".cbu_corporate_action_events SET ca_status = ?, rejected_reason = ?, updated_at = NOW() WHERE event_id = ? AND ca_status = ?";
            try (PreparedStatement ps = conn.prepareStatement(sql)) {
                ps.setString(1, e.toStatus());
                ps.setString(2, e.rejectedReason());
                ps.setObject(3, e.eventId());
                ps.setString(4, e.fromStatus());
                return ps.executeUpdate();
            }
        }
    }

    private static UUID insertEntityRelationship(Connection conn, UUID from, UUID to, String relType, String pct, String ownType, String ctrlType, String tRole, String tIntType, String tClsDesc, Boolean isReg, String regJur, String effFrom, String effTo, String source, String conf) throws SQLException {
        java.sql.Date fDate = effFrom != null ? java.sql.Date.valueOf(effFrom) : null;
        java.sql.Date tDate = effTo != null ? java.sql.Date.valueOf(effTo) : null;
        java.math.BigDecimal percentage = pct != null ? new java.math.BigDecimal(pct) : null;
        
        String sql;
        if (fDate != null) {
            sql = "INSERT INTO \"ob-poc\".entity_relationships " +
                  "(from_entity_id, to_entity_id, relationship_type, percentage, ownership_type, control_type, trust_role, trust_interest_type, trust_class_description, is_regulated, regulatory_jurisdiction, effective_from, effective_to, source, confidence, updated_at) " +
                  "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, COALESCE(?, true), ?, ?, ?, ?, ?, NOW()) " +
                  "ON CONFLICT (from_entity_id, to_entity_id, relationship_type, effective_from) WHERE effective_from IS NOT NULL " +
                  "DO UPDATE SET percentage = EXCLUDED.percentage, ownership_type = EXCLUDED.ownership_type, control_type = EXCLUDED.control_type, trust_role = EXCLUDED.trust_role, trust_interest_type = EXCLUDED.trust_interest_type, trust_class_description = EXCLUDED.trust_class_description, is_regulated = EXCLUDED.is_regulated, regulatory_jurisdiction = EXCLUDED.regulatory_jurisdiction, effective_to = EXCLUDED.effective_to, source = EXCLUDED.source, confidence = EXCLUDED.confidence, version = \"ob-poc\".entity_relationships.version + 1, updated_at = NOW() " +
                  "RETURNING relationship_id";
        } else {
            sql = "INSERT INTO \"ob-poc\".entity_relationships " +
                  "(from_entity_id, to_entity_id, relationship_type, percentage, ownership_type, control_type, trust_role, trust_interest_type, trust_class_description, is_regulated, regulatory_jurisdiction, effective_from, effective_to, source, confidence, updated_at) " +
                  "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, COALESCE(?, true), ?, NULL, ?, ?, ?, NOW()) " +
                  "ON CONFLICT (from_entity_id, to_entity_id, relationship_type) WHERE effective_from IS NULL AND effective_to IS NULL " +
                  "DO UPDATE SET percentage = EXCLUDED.percentage, ownership_type = EXCLUDED.ownership_type, control_type = EXCLUDED.control_type, trust_role = EXCLUDED.trust_role, trust_interest_type = EXCLUDED.trust_interest_type, trust_class_description = EXCLUDED.trust_class_description, is_regulated = EXCLUDED.is_regulated, regulatory_jurisdiction = EXCLUDED.regulatory_jurisdiction, source = EXCLUDED.source, confidence = EXCLUDED.confidence, version = \"ob-poc\".entity_relationships.version + 1, updated_at = NOW() " +
                  "RETURNING relationship_id";
        }
        
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setObject(1, from);
            ps.setObject(2, to);
            ps.setString(3, relType);
            ps.setBigDecimal(4, percentage);
            ps.setString(5, ownType);
            ps.setString(6, ctrlType);
            ps.setString(7, tRole);
            ps.setString(8, tIntType);
            ps.setString(9, tClsDesc);
            ps.setObject(10, isReg);
            ps.setString(11, regJur);
            if (fDate != null) {
                ps.setDate(12, fDate);
                ps.setDate(13, tDate);
                ps.setString(14, source);
                ps.setString(15, conf);
            } else {
                ps.setDate(12, tDate);
                ps.setString(13, source);
                ps.setString(14, conf);
            }
            try (ResultSet rs = ps.executeQuery()) {
                if (rs.next()) {
                    return (UUID) rs.getObject("relationship_id");
                }
            }
        }
        return null;
    }

    private static UUID getRoleId(Connection conn, String roleName) throws SQLException {
        String sql = "SELECT role_id FROM \"ob-poc\".roles WHERE name = ?";
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setString(1, roleName.toUpperCase());
            try (ResultSet rs = ps.executeQuery()) {
                if (rs.next()) {
                    return (UUID) rs.getObject("role_id");
                }
            }
        }
        throw new SQLException("Unknown role: " + roleName);
    }

    private static int applyAssignRoleEffect(Connection conn, Effect.AssignRoleEffect e, List<Object> outEvents) throws SQLException {
        String roleType = e.roleType() != null ? e.roleType().toUpperCase() : "ROLE";
        UUID roleId = null;
        UUID cbuEntityRoleId = null;
        UUID relationshipId = null;

        if ("OWNERSHIP".equals(roleType)) {
            String roleName = e.role() != null ? e.role().toUpperCase() : "SHAREHOLDER";
            roleId = getRoleId(conn, roleName);
            
            String sql = "INSERT INTO \"ob-poc\".cbu_entity_roles " +
                         "(cbu_id, entity_id, role_id, target_entity_id, ownership_percentage, effective_from, created_at, updated_at) " +
                         "VALUES (?, ?, ?, ?, ?, ?, NOW(), NOW()) " +
                         "ON CONFLICT (cbu_id, entity_id, role_id) " +
                         "DO UPDATE SET target_entity_id = EXCLUDED.target_entity_id, ownership_percentage = EXCLUDED.ownership_percentage, effective_from = EXCLUDED.effective_from, version = \"ob-poc\".cbu_entity_roles.version + 1, updated_at = NOW() " +
                         "RETURNING cbu_entity_role_id";
            try (PreparedStatement ps = conn.prepareStatement(sql)) {
                ps.setObject(1, e.cbuId());
                ps.setObject(2, e.ownerEntityId());
                ps.setObject(3, roleId);
                ps.setObject(4, e.ownedEntityId());
                ps.setBigDecimal(5, e.percentage() != null ? new java.math.BigDecimal(e.percentage()) : null);
                ps.setDate(6, e.effectiveFrom() != null ? java.sql.Date.valueOf(e.effectiveFrom()) : null);
                try (ResultSet rs = ps.executeQuery()) {
                    if (rs.next()) {
                        cbuEntityRoleId = (UUID) rs.getObject("cbu_entity_role_id");
                    }
                }
            }
            relationshipId = insertEntityRelationship(
                conn, e.ownerEntityId(), e.ownedEntityId(), "ownership", e.percentage(),
                e.ownershipType(), null, null, null, null, null, null, e.effectiveFrom(), null,
                "cbu.assign-ownership", "HIGH"
            );
        } else if ("CONTROL".equals(roleType)) {
            roleId = getRoleId(conn, e.role());
            String sql = "INSERT INTO \"ob-poc\".cbu_entity_roles " +
                         "(cbu_id, entity_id, role_id, target_entity_id, effective_from, created_at, updated_at) " +
                         "VALUES (?, ?, ?, ?, ?, NOW(), NOW()) " +
                         "ON CONFLICT (cbu_id, entity_id, role_id) " +
                         "DO UPDATE SET target_entity_id = EXCLUDED.target_entity_id, effective_from = EXCLUDED.effective_from, version = \"ob-poc\".cbu_entity_roles.version + 1, updated_at = NOW() " +
                         "RETURNING cbu_entity_role_id";
            try (PreparedStatement ps = conn.prepareStatement(sql)) {
                ps.setObject(1, e.cbuId());
                ps.setObject(2, e.controllerEntityId());
                ps.setObject(3, roleId);
                ps.setObject(4, e.controlledEntityId());
                ps.setDate(5, e.appointmentDate() != null ? java.sql.Date.valueOf(e.appointmentDate()) : null);
                try (ResultSet rs = ps.executeQuery()) {
                    if (rs.next()) {
                        cbuEntityRoleId = (UUID) rs.getObject("cbu_entity_role_id");
                    }
                }
            }
            relationshipId = insertEntityRelationship(
                conn, e.controllerEntityId(), e.controlledEntityId(), "control", null,
                null, e.controlType(), null, null, null, null, null, e.appointmentDate(), null,
                "cbu.assign-control", "HIGH"
            );
        } else if ("TRUST".equals(roleType) || "TRUST_ROLE".equals(roleType)) {
            roleId = getRoleId(conn, e.role());
            String trustRole = "trust_role";
            switch (e.role().toUpperCase()) {
                case "SETTLOR" -> trustRole = "trust_settlor";
                case "TRUSTEE" -> trustRole = "trust_trustee";
                case "PROTECTOR" -> trustRole = "trust_protector";
                case "BENEFICIARY_FIXED", "BENEFICIARY_DISCRETIONARY", "BENEFICIARY_CONTINGENT" -> trustRole = "trust_beneficiary";
                case "ENFORCER" -> trustRole = "trust_enforcer";
                case "APPOINTOR" -> trustRole = "trust_appointor";
            }
            String sql = "INSERT INTO \"ob-poc\".cbu_entity_roles " +
                         "(cbu_id, entity_id, role_id, target_entity_id, ownership_percentage, created_at, updated_at) " +
                         "VALUES (?, ?, ?, ?, ?, NOW(), NOW()) " +
                         "ON CONFLICT (cbu_id, entity_id, role_id) " +
                         "DO UPDATE SET target_entity_id = EXCLUDED.target_entity_id, ownership_percentage = EXCLUDED.ownership_percentage, version = \"ob-poc\".cbu_entity_roles.version + 1, updated_at = NOW() " +
                         "RETURNING cbu_entity_role_id";
            try (PreparedStatement ps = conn.prepareStatement(sql)) {
                ps.setObject(1, e.cbuId());
                ps.setObject(2, e.participantEntityId());
                ps.setObject(3, roleId);
                ps.setObject(4, e.trustEntityId());
                ps.setBigDecimal(5, e.interestPercentage() != null ? new java.math.BigDecimal(e.interestPercentage()) : null);
                try (ResultSet rs = ps.executeQuery()) {
                    if (rs.next()) {
                        cbuEntityRoleId = (UUID) rs.getObject("cbu_entity_role_id");
                    }
                }
            }
            relationshipId = insertEntityRelationship(
                conn, e.participantEntityId(), e.trustEntityId(), "trust_role", e.interestPercentage(),
                null, null, trustRole, e.interestType(), e.classDescription(), null, null, null, null,
                "cbu.assign-trust-role", "HIGH"
            );
        } else if ("FUND".equals(roleType) || "FUND_ROLE".equals(roleType)) {
            roleId = getRoleId(conn, e.role());
            String sql = "INSERT INTO \"ob-poc\".cbu_entity_roles " +
                         "(cbu_id, entity_id, role_id, target_entity_id, ownership_percentage, created_at, updated_at) " +
                         "VALUES (?, ?, ?, ?, ?, NOW(), NOW()) " +
                         "ON CONFLICT (cbu_id, entity_id, role_id) " +
                         "DO UPDATE SET target_entity_id = EXCLUDED.target_entity_id, ownership_percentage = EXCLUDED.ownership_percentage, version = \"ob-poc\".cbu_entity_roles.version + 1, updated_at = NOW() " +
                         "RETURNING cbu_entity_role_id";
            try (PreparedStatement ps = conn.prepareStatement(sql)) {
                ps.setObject(1, e.cbuId());
                ps.setObject(2, e.entityId());
                ps.setObject(3, roleId);
                ps.setObject(4, e.fundEntityId());
                ps.setBigDecimal(5, e.investmentPercentage() != null ? new java.math.BigDecimal(e.investmentPercentage()) : null);
                try (ResultSet rs = ps.executeQuery()) {
                    if (rs.next()) {
                        cbuEntityRoleId = (UUID) rs.getObject("cbu_entity_role_id");
                    }
                }
            }
            if (e.fundEntityId() != null) {
                String relType = "fund_role";
                switch (e.role().toUpperCase()) {
                    case "FEEDER_FUND" -> relType = "master_feeder";
                    case "SUB_FUND" -> relType = "umbrella_subfund";
                    case "PARALLEL_FUND" -> relType = "parallel";
                    case "FUND_INVESTOR" -> relType = "investment";
                    case "MANAGEMENT_COMPANY", "INVESTMENT_MANAGER" -> relType = "management";
                }
                relationshipId = insertEntityRelationship(
                    conn, e.entityId(), e.fundEntityId(), relType, e.investmentPercentage(),
                    null, null, null, null, null, e.isRegulated(), e.regulatoryJurisdiction(), null, null,
                    "cbu.assign-fund-role", "HIGH"
                );
            }
        } else if ("SERVICE_PROVIDER".equals(roleType) || "SP".equals(roleType)) {
            roleId = getRoleId(conn, e.role());
            String sql = "INSERT INTO \"ob-poc\".cbu_entity_roles " +
                         "(cbu_id, entity_id, role_id, target_entity_id, effective_from, created_at, updated_at) " +
                         "VALUES (?, ?, ?, ?, ?, NOW(), NOW()) " +
                         "ON CONFLICT (cbu_id, entity_id, role_id) " +
                         "DO UPDATE SET target_entity_id = EXCLUDED.target_entity_id, effective_from = EXCLUDED.effective_from, version = \"ob-poc\".cbu_entity_roles.version + 1, updated_at = NOW() " +
                         "RETURNING cbu_entity_role_id";
            try (PreparedStatement ps = conn.prepareStatement(sql)) {
                ps.setObject(1, e.cbuId());
                ps.setObject(2, e.providerEntityId());
                ps.setObject(3, roleId);
                ps.setObject(4, e.clientEntityId());
                ps.setDate(5, e.serviceAgreementDate() != null ? java.sql.Date.valueOf(e.serviceAgreementDate()) : null);
                try (ResultSet rs = ps.executeQuery()) {
                    if (rs.next()) {
                        cbuEntityRoleId = (UUID) rs.getObject("cbu_entity_role_id");
                    }
                }
            }
        } else if ("SIGNATORY".equals(roleType)) {
            roleId = getRoleId(conn, e.role());
            String sql = "INSERT INTO \"ob-poc\".cbu_entity_roles " +
                         "(cbu_id, entity_id, role_id, target_entity_id, authority_limit, authority_currency, requires_co_signatory, created_at, updated_at) " +
                         "VALUES (?, ?, ?, ?, ?, ?, ?, NOW(), NOW()) " +
                         "ON CONFLICT (cbu_id, entity_id, role_id) " +
                         "DO UPDATE SET target_entity_id = EXCLUDED.target_entity_id, authority_limit = EXCLUDED.authority_limit, authority_currency = EXCLUDED.authority_currency, requires_co_signatory = EXCLUDED.requires_co_signatory, version = \"ob-poc\".cbu_entity_roles.version + 1, updated_at = NOW() " +
                         "RETURNING cbu_entity_role_id";
            try (PreparedStatement ps = conn.prepareStatement(sql)) {
                ps.setObject(1, e.cbuId());
                ps.setObject(2, e.personEntityId());
                ps.setObject(3, roleId);
                ps.setObject(4, e.forEntityId());
                ps.setBigDecimal(5, e.authorityLimit() != null ? new java.math.BigDecimal(e.authorityLimit()) : null);
                ps.setString(6, e.authorityCurrency());
                ps.setObject(7, e.requiresCoSignatory());
                try (ResultSet rs = ps.executeQuery()) {
                    if (rs.next()) {
                        cbuEntityRoleId = (UUID) rs.getObject("cbu_entity_role_id");
                    }
                }
            }
        } else {
            roleId = getRoleId(conn, e.role());
            String sql = "INSERT INTO \"ob-poc\".cbu_entity_roles " +
                         "(cbu_id, entity_id, role_id, created_at, updated_at) " +
                         "VALUES (?, ?, ?, NOW(), NOW()) " +
                         "ON CONFLICT (cbu_id, entity_id, role_id) " +
                         "DO UPDATE SET version = \"ob-poc\".cbu_entity_roles.version + 1, updated_at = NOW() " +
                         "RETURNING cbu_entity_role_id";
            try (PreparedStatement ps = conn.prepareStatement(sql)) {
                ps.setObject(1, e.cbuId());
                ps.setObject(2, e.entityId());
                ps.setObject(3, roleId);
                try (ResultSet rs = ps.executeQuery()) {
                    if (rs.next()) {
                        cbuEntityRoleId = (UUID) rs.getObject("cbu_entity_role_id");
                    }
                }
            }
        }
        
        if (cbuEntityRoleId != null) {
            outEvents.add(new AssignRoleResult(cbuEntityRoleId, relationshipId));
            return 1;
        }
        return 0;
    }

    public record AssignRoleResult(UUID roleId, UUID relationshipId) {}

    private static int applyRemoveRoleEffect(Connection conn, Effect.RemoveRoleEffect e) throws SQLException {
        UUID roleId = getRoleId(conn, e.role());
        String sql = "DELETE FROM \"ob-poc\".cbu_entity_roles WHERE cbu_id = ? AND entity_id = ? AND role_id = ?";
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setObject(1, e.cbuId());
            ps.setObject(2, e.entityId());
            ps.setObject(3, roleId);
            return ps.executeUpdate();
        }
    }

    private static int applyAttachEvidence(Connection conn, Effect.AttachEvidence e, List<Object> outEvents) throws SQLException {
        String sql = "INSERT INTO \"ob-poc\".cbu_evidence " +
                     "(evidence_id, cbu_id, document_id, attestation_ref, evidence_type, evidence_category, description, attached_by, attached_at, verification_status) " +
                     "VALUES (?, ?, ?, ?, ?, ?, ?, ?, NOW(), 'PENDING') " +
                     "RETURNING evidence_id";
        UUID evId = UUID.randomUUID();
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setObject(1, evId);
            ps.setObject(2, e.cbuId());
            ps.setObject(3, e.documentId());
            ps.setString(4, e.attestationRef());
            ps.setString(5, e.evidenceType());
            ps.setString(6, e.evidenceCategory());
            ps.setString(7, e.description());
            ps.setString(8, e.attachedBy());
            try (ResultSet rs = ps.executeQuery()) {
                if (rs.next()) {
                    UUID resId = (UUID) rs.getObject("evidence_id");
                    outEvents.add(new AttachEvidenceResult(resId));
                    return 1;
                }
            }
        }
        return 0;
    }

    public record AttachEvidenceResult(UUID evidenceId) {}

    private static int applyVerifyEvidence(Connection conn, Effect.VerifyEvidence e) throws SQLException {
        String sql = "UPDATE \"ob-poc\".cbu_evidence " +
                     "SET verification_status = ?, verified_by = ?, verification_notes = ?, verified_at = NOW() " +
                     "WHERE evidence_id = ?";
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setString(1, e.verificationStatus());
            ps.setString(2, e.verifiedBy());
            ps.setString(3, e.verificationNotes());
            ps.setObject(4, e.evidenceId());
            return ps.executeUpdate();
        }
    }

    public static String hashCanonicalJson(String jsonStr) {
        if (jsonStr == null || jsonStr.equals("null")) {
            return "n177y";
        }
        try {
            com.fasterxml.jackson.databind.ObjectMapper mapper = new com.fasterxml.jackson.databind.ObjectMapper();
            Object obj = mapper.readValue(jsonStr, Object.class);
            mapper.configure(com.fasterxml.jackson.databind.SerializationFeature.ORDER_MAP_ENTRIES_BY_KEYS, true);
            String sortedJson = mapper.writeValueAsString(obj);
            java.security.MessageDigest digest = java.security.MessageDigest.getInstance("SHA-256");
            byte[] hash = digest.digest(sortedJson.getBytes(java.nio.charset.StandardCharsets.UTF_8));
            StringBuilder hexString = new StringBuilder();
            for (byte b : hash) {
                String hex = Integer.toHexString(0xff & b);
                if (hex.length() == 1) hexString.append('0');
                hexString.append(hex);
            }
            return hexString.toString();
        } catch (Exception ex) {
            return "error-hash-" + jsonStr.hashCode();
        }
    }

    private static int applyBindServiceOptions(Connection conn, Effect.BindServiceOptions e) throws SQLException {
        List<UUID> optionIds = new ArrayList<>();
        List<String> optionKeys = new ArrayList<>();
        List<String> defaultValues = new ArrayList<>();
        List<Boolean> isRequireds = new ArrayList<>();
        List<String> defaultSourceKinds = new ArrayList<>();
        
        String defSql = "SELECT service_option_def_id, option_key, default_value, is_required, default_source_kind " +
                         "FROM \"ob-poc\".service_option_defs " +
                         "WHERE service_version_id = ? AND lifecycle_status = 'active'";
        try (PreparedStatement ps = conn.prepareStatement(defSql)) {
            ps.setObject(1, e.serviceVersionId());
            try (ResultSet rs = ps.executeQuery()) {
                while (rs.next()) {
                    optionIds.add((UUID) rs.getObject("service_option_def_id"));
                    optionKeys.add(rs.getString("option_key"));
                    Object dv = rs.getObject("default_value");
                    defaultValues.add(dv != null ? dv.toString() : null);
                    isRequireds.add(rs.getBoolean("is_required"));
                    defaultSourceKinds.add(rs.getString("default_source_kind"));
                }
            }
        }
        
        com.fasterxml.jackson.databind.ObjectMapper mapper = new com.fasterxml.jackson.databind.ObjectMapper();
        com.fasterxml.jackson.databind.node.ObjectNode optionsObj = null;
        if (e.optionsJson() != null && !e.optionsJson().isBlank()) {
            try {
                optionsObj = (com.fasterxml.jackson.databind.node.ObjectNode) mapper.readTree(e.optionsJson());
            } catch (Exception ex) {
                throw new SQLException("Invalid optionsJson", ex);
            }
        }
        
        int inserted = 0;
        for (int i = 0; i < optionIds.size(); i++) {
            UUID optionId = optionIds.get(i);
            String optionKey = optionKeys.get(i);
            String defaultValue = defaultValues.get(i);
            boolean isRequired = isRequireds.get(i);
            String defaultSourceKind = defaultSourceKinds.get(i);
            
            String valStr = null;
            String sourceKind = defaultSourceKind;
            if (optionsObj != null && optionsObj.has(optionKey)) {
                valStr = optionsObj.get(optionKey).toString();
                sourceKind = "manual";
            } else if (defaultValue != null) {
                valStr = defaultValue;
            }
            
            if (valStr == null) {
                if (isRequired) {
                    throw new SQLException("required option `" + optionKey + "` has no value");
                }
                continue;
            }
            
            String valHash = hashCanonicalJson(valStr);
            String insSql = "INSERT INTO \"ob-poc\".cbu_service_option_bindings " +
                            "(cbu_id, product_id, service_id, service_version_id, service_option_def_id, option_key, value, source_kind, source_ref, source_version, value_hash, coherence_status, activation_run_id) " +
                            "VALUES (?, ?, ?, ?, ?, ?, ?::jsonb, ?, ?::jsonb, ?, ?, 'clean', ?) " +
                            "ON CONFLICT (cbu_id, service_id, service_option_def_id) WHERE valid_to IS NULL " +
                            "DO UPDATE SET value = EXCLUDED.value, source_kind = EXCLUDED.source_kind, source_ref = EXCLUDED.source_ref, source_version = EXCLUDED.source_version, value_hash = EXCLUDED.value_hash, coherence_status = 'clean', activation_run_id = EXCLUDED.activation_run_id, updated_at = now() " +
                            "RETURNING binding_id";
            try (PreparedStatement ps = conn.prepareStatement(insSql)) {
                ps.setObject(1, e.cbuId());
                ps.setObject(2, e.productId());
                ps.setObject(3, e.serviceId());
                ps.setObject(4, e.serviceVersionId());
                ps.setObject(5, optionId);
                ps.setString(6, optionKey);
                ps.setString(7, valStr);
                ps.setString(8, sourceKind);
                ps.setString(9, e.sourceRef());
                ps.setString(10, e.sourceVersion());
                ps.setString(11, valHash);
                ps.setObject(12, e.activationRunId());
                try (ResultSet rs = ps.executeQuery()) {
                    if (rs.next()) {
                        inserted++;
                    }
                }
            }
        }
        return inserted;
    }

    private static int applyOverrideOptionBinding(Connection conn, Effect.OverrideOptionBinding e, List<Object> outEvents) throws SQLException {
        String upSql = "UPDATE \"ob-poc\".cbu_service_option_bindings " +
                       "SET valid_to = now(), coherence_status = 'stale', updated_at = now() " +
                       "WHERE cbu_id = ? AND service_id = ? AND service_option_def_id = ? AND valid_to IS NULL";
        try (PreparedStatement ps = conn.prepareStatement(upSql)) {
            ps.setObject(1, e.cbuId());
            ps.setObject(2, e.serviceId());
            ps.setObject(3, e.optionDefId());
            ps.executeUpdate();
        }
        
        UUID versionId = null;
        String optionKey = null;
        String qSql = "SELECT service_version_id, option_key FROM \"ob-poc\".service_option_defs WHERE service_option_def_id = ?";
        try (PreparedStatement ps = conn.prepareStatement(qSql)) {
            ps.setObject(1, e.optionDefId());
            try (ResultSet rs = ps.executeQuery()) {
                if (rs.next()) {
                    versionId = (UUID) rs.getObject("service_version_id");
                    optionKey = rs.getString("option_key");
                }
            }
        }
        
        if (versionId == null) {
            throw new SQLException("Option definition not found: " + e.optionDefId());
        }
        
        String valHash = hashCanonicalJson(e.value());
        String insSql = "INSERT INTO \"ob-poc\".cbu_service_option_bindings " +
                        "(cbu_id, product_id, service_id, service_version_id, service_option_def_id, option_key, value, source_kind, source_ref, source_version, value_hash, coherence_status, activation_run_id) " +
                        "VALUES (?, ?, ?, ?, ?, ?, ?::jsonb, 'manual', ?::jsonb, ?, ?, 'clean', ?) " +
                        "RETURNING binding_id";
        try (PreparedStatement ps = conn.prepareStatement(insSql)) {
            ps.setObject(1, e.cbuId());
            ps.setObject(2, e.productId());
            ps.setObject(3, e.serviceId());
            ps.setObject(4, versionId);
            ps.setObject(5, e.optionDefId());
            ps.setString(6, optionKey);
            ps.setString(7, e.value());
            ps.setString(8, e.sourceRef());
            ps.setString(9, e.sourceVersion());
            ps.setString(10, valHash);
            ps.setObject(11, e.activationRunId());
            try (ResultSet rs = ps.executeQuery()) {
                if (rs.next()) {
                    UUID bindingId = (UUID) rs.getObject("binding_id");
                    outEvents.add(new OverrideOptionBindingResult(bindingId));
                    return 1;
                }
            }
        }
        return 0;
    }

    public record OverrideOptionBindingResult(UUID bindingId) {}

    private static int applyDirtyFlagBindings(Connection conn, Effect.DirtyFlagBindings e) throws SQLException {
        String sql = "UPDATE \"ob-poc\".cbu_service_option_bindings " +
                     "SET coherence_status = 'dirty', updated_at = now() " +
                     "WHERE cbu_id = ? AND valid_to IS NULL " +
                     "  AND (?::uuid IS NULL OR service_id = ?) " +
                     "  AND (?::text IS NULL OR source_kind = ?)";
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setObject(1, e.cbuId());
            ps.setObject(2, e.serviceId());
            ps.setObject(3, e.serviceId());
            ps.setString(4, e.sourceKind());
            ps.setString(5, e.sourceKind());
            return ps.executeUpdate();
        }
    }

    private static int applyRecomputeBindings(Connection conn, Effect.RecomputeBindings e) throws SQLException {
        String sql = "UPDATE \"ob-poc\".cbu_service_option_bindings " +
                     "SET coherence_status = 'clean', updated_at = now() " +
                     "WHERE cbu_id = ? AND valid_to IS NULL AND coherence_status = 'dirty' " +
                     "  AND (?::uuid IS NULL OR service_id = ?)";
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setObject(1, e.cbuId());
            ps.setObject(2, e.serviceId());
            ps.setObject(3, e.serviceId());
            return ps.executeUpdate();
        }
    }

    private static int applyEnsureCbu(Connection conn, Effect.EnsureCbu e, List<Object> outEvents) throws SQLException {
        String sql = "INSERT INTO \"ob-poc\".cbus (cbu_id, name, jurisdiction, client_type, nature_purpose, commercial_client_entity_id) " +
                     "VALUES (?, ?, ?, ?, ?, ?) " +
                     "ON CONFLICT (name, jurisdiction) " +
                     "DO UPDATE SET client_type = COALESCE(EXCLUDED.client_type, \"ob-poc\".cbus.client_type), " +
                     "              nature_purpose = COALESCE(EXCLUDED.nature_purpose, \"ob-poc\".cbus.nature_purpose), " +
                     "              commercial_client_entity_id = COALESCE(EXCLUDED.commercial_client_entity_id, \"ob-poc\".cbus.commercial_client_entity_id), " +
                     "              updated_at = NOW() " +
                     "RETURNING cbu_id, (xmax = 0) AS is_insert";
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            UUID id = e.id() != null ? e.id() : UUID.randomUUID();
            ps.setObject(1, id);
            ps.setString(2, e.name());
            ps.setString(3, e.jurisdiction());
            ps.setString(4, e.clientType());
            ps.setString(5, e.naturePurpose());
            ps.setObject(6, e.commercialClientEntityId());
            try (ResultSet rs = ps.executeQuery()) {
                if (rs.next()) {
                    UUID resultId = (UUID) rs.getObject("cbu_id");
                    boolean isInsert = rs.getBoolean("is_insert");
                    outEvents.add(new InsertResult(resultId, isInsert));
                    return 1;
                }
            }
        }
        return 0;
    }

    private static int applyCreateFromClientGroup(Connection conn, Effect.CreateFromClientGroup e, List<Object> outEvents) throws SQLException {
        String sql = "SELECT DISTINCT " +
                     "  e.entity_id, " +
                     "  e.name, " +
                     "  COALESCE(elc.jurisdiction, ef.jurisdiction) as jurisdiction, " +
                     "  ef.gleif_category, " +
                     "  (SELECT r.name FROM \"ob-poc\".client_group_entity_roles cger " +
                     "   JOIN \"ob-poc\".roles r ON r.role_id = cger.role_id " +
                     "   WHERE cger.cge_id = cge.id " +
                     "   LIMIT 1) as group_role " +
                     "FROM \"ob-poc\".client_group_entity cge " +
                     "JOIN \"ob-poc\".entities e ON e.entity_id = cge.entity_id " +
                     "LEFT JOIN \"ob-poc\".entity_limited_companies elc ON elc.entity_id = e.entity_id " +
                     "LEFT JOIN \"ob-poc\".entity_funds ef ON ef.entity_id = e.entity_id " +
                     "WHERE cge.group_id = ? " +
                     "  AND cge.membership_type NOT IN ('historical', 'rejected') " +
                     "  AND e.deleted_at IS NULL " +
                     "  AND (? IS NULL OR ef.gleif_category = ?) " +
                     "  AND (? IS NULL OR EXISTS ( " +
                     "      SELECT 1 FROM \"ob-poc\".client_group_entity_roles cger2 " +
                     "      JOIN \"ob-poc\".roles r2 ON r2.role_id = cger2.role_id " +
                     "      WHERE cger2.cge_id = cge.id AND r2.name = ? " +
                     "  )) " +
                     "  AND (? IS NULL OR COALESCE(elc.jurisdiction, ef.jurisdiction) = ?) " +
                     "ORDER BY e.name " +
                     "LIMIT ?";
                     
        List<UUID> entityIds = new ArrayList<>();
        List<String> names = new ArrayList<>();
        List<String> jurisdictions = new ArrayList<>();
        List<String> gleifCategories = new ArrayList<>();
        List<String> groupRoles = new ArrayList<>();
        
        int limit = e.limit() != null ? e.limit() : 100;
        try (PreparedStatement ps = conn.prepareStatement(sql)) {
            ps.setObject(1, e.groupId());
            ps.setString(2, e.gleifCategory());
            ps.setString(3, e.gleifCategory());
            ps.setString(4, e.roleFilter());
            ps.setString(5, e.roleFilter());
            ps.setString(6, e.jurisdictionFilter());
            ps.setString(7, e.jurisdictionFilter());
            ps.setInt(8, limit);
            try (ResultSet rs = ps.executeQuery()) {
                while (rs.next()) {
                    entityIds.add((UUID) rs.getObject("entity_id"));
                    names.add(rs.getString("name"));
                    jurisdictions.add(rs.getString("jurisdiction"));
                    gleifCategories.add(rs.getString("gleif_category"));
                    groupRoles.add(rs.getString("group_role"));
                }
            }
        }
        
        List<String> dslStatements = new ArrayList<>();
        List<com.fasterxml.jackson.databind.JsonNode> entityInfo = new ArrayList<>();
        com.fasterxml.jackson.databind.ObjectMapper mapper = new com.fasterxml.jackson.databind.ObjectMapper();
        
        String defaultJurisdiction = e.defaultJurisdiction() != null ? e.defaultJurisdiction() : "LU";
        
        for (int i = 0; i < entityIds.size(); i++) {
            UUID entityId = entityIds.get(i);
            String name = names.get(i);
            String jurisdiction = jurisdictions.get(i) != null ? jurisdictions.get(i) : defaultJurisdiction;
            String gleifCategory = gleifCategories.get(i);
            String groupRole = groupRoles.get(i);
            
            String dsl = "(cbu.create :name \"" + name.replace("\"", "\\\"") + "\" :jurisdiction \"" + jurisdiction + "\" :fund-entity-id \"" + entityId + "\"";
            if (e.mancoEntityId() != null) {
                dsl += " :manco-entity-id \"" + e.mancoEntityId() + "\"";
            }
            dsl += ")";
            dslStatements.add(dsl);
            
            com.fasterxml.jackson.databind.node.ObjectNode node = mapper.createObjectNode();
            node.put("entity_id", entityId.toString());
            node.put("name", name);
            node.put("jurisdiction", jurisdiction);
            node.put("gleif_category", gleifCategory);
            node.put("group_role", groupRole);
            node.put("dsl", dsl);
            entityInfo.add(node);
        }
        
        String combinedDsl = String.join("\n", dslStatements);
        
        outEvents.add(new CreateFromClientGroupResult(
            e.groupId(),
            e.gleifCategory(),
            e.roleFilter(),
            e.jurisdictionFilter(),
            entityIds.size(),
            dslStatements,
            combinedDsl,
            entityInfo,
            e.dryRun()
        ));
        
        return entityIds.size();
    }
    
    public record CreateFromClientGroupResult(
        UUID groupId,
        String gleifCategory,
        String roleFilter,
        String jurisdictionFilter,
        int entitiesFound,
        List<String> dslBatch,
        String combinedDsl,
        List<com.fasterxml.jackson.databind.JsonNode> entities,
        boolean dryRun
    ) {}

    private static int applyDeleteCascade(Connection conn, Effect.DeleteCascade e, List<Object> outEvents) throws SQLException {
        String nameSql = "SELECT name FROM \"ob-poc\".cbus WHERE cbu_id = ? AND deleted_at IS NULL";
        String cbuName = null;
        try (PreparedStatement ps = conn.prepareStatement(nameSql)) {
            ps.setObject(1, e.cbuId());
            try (ResultSet rs = ps.executeQuery()) {
                if (rs.next()) {
                    cbuName = rs.getString("name");
                }
            }
        }
        if (cbuName == null) {
            throw new SQLException("CBU not found: " + e.cbuId());
        }
        
        java.util.Map<String, Long> deletedCounts = new java.util.HashMap<>();
        
        String unlinkGroupSql = "UPDATE \"ob-poc\".client_group_entity SET cbu_id = NULL, updated_at = NOW() WHERE cbu_id = ?";
        try (PreparedStatement ps = conn.prepareStatement(unlinkGroupSql)) {
            ps.setObject(1, e.cbuId());
            deletedCounts.put("client_group_entity_unlinked", (long) ps.executeUpdate());
        }
        
        int groupMembersAffected = 0;
        if (e.hardDelete()) {
            String sql = "DELETE FROM \"ob-poc\".cbu_group_members WHERE cbu_id = ?";
            try (PreparedStatement ps = conn.prepareStatement(sql)) {
                ps.setObject(1, e.cbuId());
                groupMembersAffected = ps.executeUpdate();
            }
        } else {
            String sql = "UPDATE \"ob-poc\".cbu_group_members SET effective_to = CURRENT_DATE WHERE cbu_id = ? AND effective_to IS NULL";
            try (PreparedStatement ps = conn.prepareStatement(sql)) {
                ps.setObject(1, e.cbuId());
                groupMembersAffected = ps.executeUpdate();
            }
        }
        deletedCounts.put("cbu_group_members", (long) groupMembersAffected);
        
        List<UUID> linkIds = new ArrayList<>();
        String linkSql = "SELECT link_id FROM \"ob-poc\".cbu_structure_links WHERE parent_cbu_id = ? OR child_cbu_id = ?";
        try (PreparedStatement ps = conn.prepareStatement(linkSql)) {
            ps.setObject(1, e.cbuId());
            ps.setObject(2, e.cbuId());
            try (ResultSet rs = ps.executeQuery()) {
                while (rs.next()) {
                    linkIds.add((UUID) rs.getObject("link_id"));
                }
            }
        }
        int structureLinksAffected = 0;
        for (UUID linkId : linkIds) {
            if (e.hardDelete()) {
                String sql = "DELETE FROM \"ob-poc\".cbu_structure_links WHERE link_id = ?";
                try (PreparedStatement ps = conn.prepareStatement(sql)) {
                    ps.setObject(1, linkId);
                    structureLinksAffected += ps.executeUpdate();
                }
            } else {
                String sql = "UPDATE \"ob-poc\".cbu_structure_links SET status = 'TERMINATED', terminated_at = NOW(), terminated_reason = 'cbu.delete-cascade', updated_at = NOW() WHERE link_id = ? AND status = 'ACTIVE'";
                try (PreparedStatement ps = conn.prepareStatement(sql)) {
                    ps.setObject(1, linkId);
                    structureLinksAffected += ps.executeUpdate();
                }
            }
        }
        deletedCounts.put("cbu_structure_links", (long) structureLinksAffected);
        
        long entitiesDeleted = 0;
        long entitiesPreserved = 0;
        if (e.deleteEntities()) {
            List<UUID> exclusiveEntities = new ArrayList<>();
            String exclSql = "SELECT DISTINCT cer.entity_id FROM \"ob-poc\".cbu_entity_roles cer WHERE cer.cbu_id = ? " +
                             "AND NOT EXISTS (SELECT 1 FROM \"ob-poc\".cbu_entity_roles other JOIN \"ob-poc\".cbus c ON c.cbu_id = other.cbu_id WHERE other.entity_id = cer.entity_id AND other.cbu_id <> ? AND c.deleted_at IS NULL)";
            try (PreparedStatement ps = conn.prepareStatement(exclSql)) {
                ps.setObject(1, e.cbuId());
                ps.setObject(2, e.cbuId());
                try (ResultSet rs = ps.executeQuery()) {
                    while (rs.next()) {
                        exclusiveEntities.add((UUID) rs.getObject("entity_id"));
                    }
                }
            }
            
            String sharedSql = "SELECT COUNT(DISTINCT cer.entity_id)::bigint FROM \"ob-poc\".cbu_entity_roles cer WHERE cer.cbu_id = ? " +
                               "AND EXISTS (SELECT 1 FROM \"ob-poc\".cbu_entity_roles other JOIN \"ob-poc\".cbus c ON c.cbu_id = other.cbu_id WHERE other.entity_id = cer.entity_id AND other.cbu_id <> ? AND c.deleted_at IS NULL)";
            try (PreparedStatement ps = conn.prepareStatement(sharedSql)) {
                ps.setObject(1, e.cbuId());
                ps.setObject(2, e.cbuId());
                try (ResultSet rs = ps.executeQuery()) {
                    if (rs.next()) {
                        entitiesPreserved = rs.getLong(1);
                    }
                }
            }
            
            String deactSql = "UPDATE \"ob-poc\".entities SET deleted_at = NOW(), updated_at = NOW() WHERE entity_id = ? AND deleted_at IS NULL";
            for (UUID entId : exclusiveEntities) {
                try (PreparedStatement ps = conn.prepareStatement(deactSql)) {
                    ps.setObject(1, entId);
                    ps.executeUpdate();
                }
            }
            entitiesDeleted = exclusiveEntities.size();
        }
        
        int roleTerminateAffected = 0;
        if (e.hardDelete()) {
            String sql = "DELETE FROM \"ob-poc\".cbu_entity_roles WHERE cbu_id = ?";
            try (PreparedStatement ps = conn.prepareStatement(sql)) {
                ps.setObject(1, e.cbuId());
                roleTerminateAffected = ps.executeUpdate();
            }
        } else {
            String sql = "UPDATE \"ob-poc\".cbu_entity_roles SET effective_to = CURRENT_DATE, updated_at = NOW() WHERE cbu_id = ? AND effective_to IS NULL";
            try (PreparedStatement ps = conn.prepareStatement(sql)) {
                ps.setObject(1, e.cbuId());
                roleTerminateAffected = ps.executeUpdate();
            }
        }
        deletedCounts.put("cbu_entity_roles", (long) roleTerminateAffected);
        
        String delCbuSql = "UPDATE \"ob-poc\".cbus SET deleted_at = NOW(), updated_at = NOW() WHERE cbu_id = ? AND deleted_at IS NULL";
        try (PreparedStatement ps = conn.prepareStatement(delCbuSql)) {
            ps.setObject(1, e.cbuId());
            deletedCounts.put("cbus", (long) ps.executeUpdate());
        }
        
        long totalDeleted = deletedCounts.values().stream().mapToLong(Long::longValue).sum();
        
        outEvents.add(new DeleteCascadeResult(
            e.cbuId(),
            cbuName,
            totalDeleted,
            entitiesDeleted,
            entitiesPreserved,
            deletedCounts
        ));
        
        return 1;
    }
    
    public record DeleteCascadeResult(
        UUID cbuId,
        String cbuName,
        long totalRecordsDeleted,
        long entitiesDeleted,
        long entitiesPreservedShared,
        java.util.Map<String, Long> byTable
    ) {}
}
