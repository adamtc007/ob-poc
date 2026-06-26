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

        StructuralState struct = switch (statusStr != null ? statusStr.toUpperCase() : "") {
            case "DISCOVERED" -> new StructuralState.Discovered();
            case "CONFIGURING" -> new StructuralState.Configuring();
            default -> new StructuralState.Structured();
        };

        ValidationState val = switch (statusStr != null ? statusStr.toUpperCase() : "") {
            case "VALIDATION_PENDING" -> new ValidationState.ValidationPending();
            case "VALIDATED" -> new ValidationState.Validated();
            case "VALIDATION_FAILED" -> new ValidationState.ValidationFailed();
            case "UPDATE_PENDING_PROOF" -> new ValidationState.UpdatePendingProof();
            case "EVIDENCED" -> new ValidationState.Evidenced();
            default -> null;
        };

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

        DispositionState disposition = switch (dispStatusStr != null ? dispStatusStr.toLowerCase() : "") {
            case "active" -> new DispositionState.Active();
            case "under_remediation" -> new DispositionState.UnderRemediation();
            case "soft_deleted" -> new DispositionState.SoftDeleted();
            case "hard_deleted" -> new DispositionState.HardDeleted();
            default -> new DispositionState.Active();
        };
        if (deletedAt != null) {
            disposition = new DispositionState.SoftDeleted();
        }

        return new CbuStatus(id, name, op, val, struct, disposition, clientType, jurisdiction, statusStr, opStatusStr, dispStatusStr);
    }
}
