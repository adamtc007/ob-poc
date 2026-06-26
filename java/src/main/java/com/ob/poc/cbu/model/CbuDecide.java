package com.ob.poc.cbu.model;

import java.util.List;
import java.util.Set;
import java.util.Collections;
import java.util.UUID;

public final class CbuDecide {

    private CbuDecide() {}

    public record DecisionContext(
        boolean isFundLinkedAlready,
        boolean parentExists,
        boolean childExists,
        UUID existingLinkId
    ) {
        public DecisionContext(boolean isFundLinkedAlready) {
            this(isFundLinkedAlready, false, false, null);
        }
        public DecisionContext(boolean parentExists, boolean childExists, UUID existingLinkId) {
            this(false, parentExists, childExists, existingLinkId);
        }
    }

    public static DecisionOutcome decide(
        CbuCommand command,
        CbuStatus currentStatus,
        DecisionContext context
    ) {
        return switch (command) {
            case CbuCommand.Suspend cmd -> decideSuspend(cmd, currentStatus);
            case CbuCommand.Create cmd -> decideCreate(cmd, currentStatus, context);
            case CbuCommand.SubmitForValidation cmd -> decideSubmitForValidation(cmd, currentStatus);
            case CbuCommand.Confirm cmd -> decideConfirm(cmd, currentStatus);
            case CbuCommand.Reject cmd -> decideReject(cmd, currentStatus);
            case CbuCommand.RequestProofUpdate cmd -> decideRequestProofUpdate(cmd, currentStatus);
            case CbuCommand.ReopenValidation cmd -> decideReopenValidation(cmd, currentStatus);
            case CbuCommand.ResubmitProof cmd -> decideResubmitProof(cmd, currentStatus);
            case CbuCommand.Reinstate cmd -> decideReinstate(cmd, currentStatus);
            case CbuCommand.Restrict cmd -> decideRestrict(cmd, currentStatus);
            case CbuCommand.Unrestrict cmd -> decideUnrestrict(cmd, currentStatus);
            case CbuCommand.BeginWindingDown cmd -> decideBeginWindingDown(cmd, currentStatus);
            case CbuCommand.CompleteOffboard cmd -> decideCompleteOffboard(cmd, currentStatus);
            case CbuCommand.FlagForRemediation cmd -> decideFlagForRemediation(cmd, currentStatus);
            case CbuCommand.ClearRemediation cmd -> decideClearRemediation(cmd, currentStatus);
            case CbuCommand.SoftDelete cmd -> decideSoftDelete(cmd, currentStatus);
            case CbuCommand.Restore cmd -> decideRestore(cmd, currentStatus);
            case CbuCommand.HardDelete cmd -> decideHardDelete(cmd, currentStatus);
            case CbuCommand.LinkStructure cmd -> decideLinkStructure(cmd, currentStatus, context);
            case CbuCommand.UnlinkStructure cmd -> decideUnlinkStructure(cmd, currentStatus);
            case CbuCommand.TerminateRole cmd -> decideTerminateRole(cmd, currentStatus);
            case CbuCommand.RemoveMember cmd -> decideRemoveMember(cmd, currentStatus);
        };
    }

    private static DecisionOutcome decideSuspend(CbuCommand.Suspend cmd, CbuStatus status) {
        if (status == null) {
            return new DecisionOutcome.Refuse("CBU not found");
        }
        
        // Disposition check (must be active or under remediation)
        if (!(status.disposition() instanceof DispositionState.Active) 
            && !(status.disposition() instanceof DispositionState.UnderRemediation)) {
            return new DecisionOutcome.Refuse("CBU is not in a live disposition state");
        }
        
        // Role check
        Set<String> actorRoles = cmd.actor().roles();
        boolean hasComplianceRole = actorRoles.contains("compliance_officer") 
            || actorRoles.contains("senior_compliance") 
            || actorRoles.contains("mlro");
        if (!hasComplianceRole) {
            return new DecisionOutcome.Refuse("Principal does not have compliance roles required to suspend");
        }

        // Validation -> Operational Gate
        if (!(status.val() instanceof ValidationState.Validated)) {
            return new DecisionOutcome.Refuse("CBU must be VALIDATED before operational status can be modified");
        }

        // Operational state check
        if (!(status.op() instanceof OperationalState.OperationallyActive)) {
            return new DecisionOutcome.Refuse("CBU must be in OperationallyActive state to suspend");
        }

        String opStatusStr = status.rawOpStatus() != null ? status.rawOpStatus().toLowerCase() : "actively_trading";

        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateOperationalStatus(
                cmd.id(),
                opStatusStr,
                "suspended",
                cmd.reason()
            )
        ));
    }

    private static DecisionOutcome decideCreate(
        CbuCommand.Create cmd,
        CbuStatus status,
        DecisionContext context
    ) {
        // Idempotency: if CBU already exists with same name/jurisdiction
        if (status != null) {
            // Already exists, skip creation
            return new DecisionOutcome.Accept(Collections.emptyList());
        }

        // Idempotency: if fund entity is already linked
        if (context != null && context.isFundLinkedAlready()) {
            return new DecisionOutcome.Accept(Collections.emptyList());
        }

        // Generate effects for creation
        List<Effect> effects = new java.util.ArrayList<>();
        effects.add(new Effect.InsertCbu(
            cmd.id(),
            cmd.name(),
            cmd.jurisdiction(),
            cmd.clientType(),
            cmd.naturePurpose(),
            cmd.description(),
            cmd.commercialClientEntityId()
        ));

        if (cmd.fundEntityId() != null) {
            effects.add(new Effect.AssignFundRole(cmd.id(), cmd.fundEntityId(), "ASSET_OWNER"));
            effects.add(new Effect.LinkCbu(cmd.id(), cmd.fundEntityId()));
        }

        if (cmd.mancoEntityId() != null) {
            effects.add(new Effect.AssignFundRole(cmd.id(), cmd.mancoEntityId(), "MANAGEMENT_COMPANY"));
        }

        // Side effect: EmitPendingStateAdvance (only on genuine creation)
        effects.add(new Effect.EmitPendingStateAdvance(
            cmd.id(),
            "cbu:onboarded",
            "cbu/trading-profile",
            "cbu.create — new client business unit"
        ));

        return new DecisionOutcome.Accept(List.copyOf(effects));
    }

    private static DecisionOutcome decideSubmitForValidation(CbuCommand.SubmitForValidation cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        String statusStr = status.rawStatus() != null ? status.rawStatus().toUpperCase() : "DISCOVERED";
        if (!statusStr.equals("DISCOVERED") && !statusStr.equals("VALIDATION_FAILED")) {
            return new DecisionOutcome.Refuse("CBU must be in DISCOVERED or VALIDATION_FAILED status to submit for validation");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateValidationStatus(cmd.id(), statusStr, "VALIDATION_PENDING")
        ));
    }

    private static DecisionOutcome decideConfirm(CbuCommand.Confirm cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        String statusStr = status.rawStatus() != null ? status.rawStatus().toUpperCase() : "DISCOVERED";
        if (!statusStr.equals("VALIDATION_PENDING")) {
            return new DecisionOutcome.Refuse("CBU must be in VALIDATION_PENDING status to confirm");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateValidationStatus(cmd.id(), "VALIDATION_PENDING", "VALIDATED")
        ));
    }

    private static DecisionOutcome decideReject(CbuCommand.Reject cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        String statusStr = status.rawStatus() != null ? status.rawStatus().toUpperCase() : "DISCOVERED";
        if (!statusStr.equals("VALIDATION_PENDING")) {
            return new DecisionOutcome.Refuse("CBU must be in VALIDATION_PENDING status to reject");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateValidationStatus(cmd.id(), "VALIDATION_PENDING", "VALIDATION_FAILED")
        ));
    }

    private static DecisionOutcome decideRequestProofUpdate(CbuCommand.RequestProofUpdate cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        String statusStr = status.rawStatus() != null ? status.rawStatus().toUpperCase() : "DISCOVERED";
        if (!statusStr.equals("VALIDATED")) {
            return new DecisionOutcome.Refuse("CBU must be in VALIDATED status to request proof update");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateValidationStatus(cmd.id(), "VALIDATED", "UPDATE_PENDING_PROOF")
        ));
    }

    private static DecisionOutcome decideReopenValidation(CbuCommand.ReopenValidation cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        String statusStr = status.rawStatus() != null ? status.rawStatus().toUpperCase() : "DISCOVERED";
        if (!statusStr.equals("VALIDATION_FAILED")) {
            return new DecisionOutcome.Refuse("CBU must be in VALIDATION_FAILED status to reopen validation");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateValidationStatus(cmd.id(), "VALIDATION_FAILED", "VALIDATION_PENDING")
        ));
    }

    private static DecisionOutcome decideResubmitProof(CbuCommand.ResubmitProof cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        String statusStr = status.rawStatus() != null ? status.rawStatus().toUpperCase() : "DISCOVERED";
        if (!statusStr.equals("UPDATE_PENDING_PROOF")) {
            return new DecisionOutcome.Refuse("CBU must be in UPDATE_PENDING_PROOF status to resubmit proof");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateValidationStatus(cmd.id(), "UPDATE_PENDING_PROOF", "VALIDATION_PENDING")
        ));
    }

    private static DecisionOutcome decideReinstate(CbuCommand.Reinstate cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        if (!(status.val() instanceof ValidationState.Validated)) {
            return new DecisionOutcome.Refuse("CBU must be VALIDATED before operational status can be modified");
        }
        String opStatusStr = status.rawOpStatus() != null ? status.rawOpStatus().toLowerCase() : "";
        if (!opStatusStr.equals("suspended")) {
            return new DecisionOutcome.Refuse("CBU must be in suspended state to reinstate");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateOperationalStatus(cmd.id(), "suspended", "actively_trading", null)
        ));
    }

    private static DecisionOutcome decideRestrict(CbuCommand.Restrict cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        if (!(status.val() instanceof ValidationState.Validated)) {
            return new DecisionOutcome.Refuse("CBU must be VALIDATED before operational status can be modified");
        }
        String opStatusStr = status.rawOpStatus() != null ? status.rawOpStatus().toLowerCase() : "";
        if (!opStatusStr.equals("actively_trading") && !opStatusStr.equals("trade_permissioned")) {
            return new DecisionOutcome.Refuse("CBU must be in actively_trading or trade_permissioned state to restrict");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateOperationalStatus(cmd.id(), opStatusStr, "restricted", null)
        ));
    }

    private static DecisionOutcome decideUnrestrict(CbuCommand.Unrestrict cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        if (!(status.val() instanceof ValidationState.Validated)) {
            return new DecisionOutcome.Refuse("CBU must be VALIDATED before operational status can be modified");
        }
        String opStatusStr = status.rawOpStatus() != null ? status.rawOpStatus().toLowerCase() : "";
        if (!opStatusStr.equals("restricted")) {
            return new DecisionOutcome.Refuse("CBU must be in restricted state to unrestrict");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateOperationalStatus(cmd.id(), "restricted", "actively_trading", null)
        ));
    }

    private static DecisionOutcome decideBeginWindingDown(CbuCommand.BeginWindingDown cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        if (!(status.val() instanceof ValidationState.Validated)) {
            return new DecisionOutcome.Refuse("CBU must be VALIDATED before operational status can be modified");
        }
        Set<String> actorRoles = cmd.actor().roles();
        boolean hasRequiredRole = actorRoles.contains("compliance_admin") 
            || actorRoles.contains("senior_compliance") 
            || actorRoles.contains("mlro");
        if (!hasRequiredRole) {
            return new DecisionOutcome.Refuse("Principal does not have roles required to begin winding down");
        }
        String opStatusStr = status.rawOpStatus() != null ? status.rawOpStatus().toLowerCase() : "";
        if (!opStatusStr.equals("actively_trading") && !opStatusStr.equals("trade_permissioned") 
            && !opStatusStr.equals("suspended") && !opStatusStr.equals("restricted")) {
            return new DecisionOutcome.Refuse("CBU must be in an active, restricted, or suspended state to wind down");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateOperationalStatus(cmd.id(), opStatusStr, "winding_down", cmd.reason())
        ));
    }

    private static DecisionOutcome decideCompleteOffboard(CbuCommand.CompleteOffboard cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        if (!(status.val() instanceof ValidationState.Validated)) {
            return new DecisionOutcome.Refuse("CBU must be VALIDATED before operational status can be modified");
        }
        Set<String> actorRoles = cmd.actor().roles();
        boolean hasRequiredRole = actorRoles.contains("compliance_admin") 
            || actorRoles.contains("senior_compliance") 
            || actorRoles.contains("mlro");
        if (!hasRequiredRole) {
            return new DecisionOutcome.Refuse("Principal does not have roles required to complete offboarding");
        }
        String opStatusStr = status.rawOpStatus() != null ? status.rawOpStatus().toLowerCase() : "";
        if (!opStatusStr.equals("winding_down")) {
            return new DecisionOutcome.Refuse("CBU must be in winding_down state to complete offboarding");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateOperationalStatus(cmd.id(), "winding_down", "offboarded", null)
        ));
    }

    private static DecisionOutcome decideFlagForRemediation(CbuCommand.FlagForRemediation cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        String dispStatusStr = status.rawDispStatus() != null ? status.rawDispStatus().toLowerCase() : "active";
        if (!dispStatusStr.equals("active")) {
            return new DecisionOutcome.Refuse("CBU must be active to flag for remediation");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateDispositionStatus(cmd.id(), dispStatusStr, "under_remediation", cmd.reason())
        ));
    }

    private static DecisionOutcome decideClearRemediation(CbuCommand.ClearRemediation cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        String dispStatusStr = status.rawDispStatus() != null ? status.rawDispStatus().toLowerCase() : "";
        if (!dispStatusStr.equals("under_remediation")) {
            return new DecisionOutcome.Refuse("CBU must be under remediation to clear it");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateDispositionStatus(cmd.id(), "under_remediation", "active", null)
        ));
    }

    private static DecisionOutcome decideSoftDelete(CbuCommand.SoftDelete cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        String dispStatusStr = status.rawDispStatus() != null ? status.rawDispStatus().toLowerCase() : "active";
        if (!dispStatusStr.equals("active") && !dispStatusStr.equals("under_remediation")) {
            return new DecisionOutcome.Refuse("CBU must be active or under remediation to soft delete");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateDispositionStatus(cmd.id(), dispStatusStr, "soft_deleted", cmd.reason())
        ));
    }

    private static DecisionOutcome decideRestore(CbuCommand.Restore cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        String dispStatusStr = status.rawDispStatus() != null ? status.rawDispStatus().toLowerCase() : "";
        if (!dispStatusStr.equals("soft_deleted")) {
            return new DecisionOutcome.Refuse("CBU must be soft deleted to restore");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateDispositionStatus(cmd.id(), "soft_deleted", "active", null)
        ));
    }

    private static DecisionOutcome decideHardDelete(CbuCommand.HardDelete cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        Set<String> actorRoles = cmd.actor().roles();
        boolean hasRequiredRole = actorRoles.contains("compliance_admin") 
            || actorRoles.contains("senior_compliance") 
            || actorRoles.contains("mlro");
        if (!hasRequiredRole) {
            return new DecisionOutcome.Refuse("Principal does not have roles required to hard delete");
        }
        String dispStatusStr = status.rawDispStatus() != null ? status.rawDispStatus().toLowerCase() : "active";
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateDispositionStatus(cmd.id(), dispStatusStr, "hard_deleted", null)
        ));
    }

    private static DecisionOutcome decideLinkStructure(
        CbuCommand.LinkStructure cmd,
        CbuStatus status,
        DecisionContext context
    ) {
        if (context == null || !context.parentExists()) {
            return new DecisionOutcome.Refuse("cbu.link-structure: parent CBU not found: " + cmd.parentCbuId());
        }
        if (!context.childExists()) {
            return new DecisionOutcome.Refuse("cbu.link-structure: child CBU not found: " + cmd.childCbuId());
        }

        String relationshipType = cmd.relationshipType().replace('-', '_').toUpperCase();
        List<String> allowedTypes = List.of("FEEDER", "PARALLEL", "AGGREGATOR", "MASTER", "CO_INVEST_VEHICLE");
        if (!allowedTypes.contains(relationshipType)) {
            return new DecisionOutcome.Refuse("cbu.link-structure: unsupported relationship-type '" + cmd.relationshipType() + "'");
        }

        String capitalFlow = null;
        if (cmd.capitalFlow() != null) {
            capitalFlow = cmd.capitalFlow().replace('-', '_').toUpperCase();
            List<String> allowedFlows = List.of("UPSTREAM", "DOWNSTREAM", "CO_INVEST");
            if (!allowedFlows.contains(capitalFlow)) {
                return new DecisionOutcome.Refuse("cbu.link-structure: unsupported capital-flow '" + cmd.capitalFlow() + "'");
            }
        }

        String selector = cmd.relationshipSelector();
        if (selector == null || selector.trim().isEmpty()) {
            selector = cmd.relationshipType().replace('-', '_').toLowerCase();
        }

        return new DecisionOutcome.Accept(List.of(
            new Effect.LinkStructure(
                cmd.parentCbuId(),
                cmd.childCbuId(),
                relationshipType,
                selector,
                capitalFlow,
                cmd.effectiveFrom(),
                cmd.effectiveTo(),
                context.existingLinkId()
            )
        ));
    }

    private static DecisionOutcome decideUnlinkStructure(
        CbuCommand.UnlinkStructure cmd,
        CbuStatus status
    ) {
        return new DecisionOutcome.Accept(List.of(
            new Effect.UnlinkStructure(
                cmd.linkId(),
                cmd.reason(),
                cmd.hardDelete() != null ? cmd.hardDelete() : false
            )
        ));
    }

    private static DecisionOutcome decideTerminateRole(
        CbuCommand.TerminateRole cmd,
        CbuStatus status
    ) {
        List<Effect> effects = new java.util.ArrayList<>();
        effects.add(new Effect.TerminateRole(
            cmd.cbuId(),
            cmd.hardDelete() != null ? cmd.hardDelete() : false
        ));
        effects.add(new Effect.EmitPendingStateAdvance(
            cmd.cbuId(),
            "cbu-role:terminated",
            "cbu/role-graph",
            "cbu-role.terminate"
        ));
        return new DecisionOutcome.Accept(List.copyOf(effects));
    }

    private static DecisionOutcome decideRemoveMember(
        CbuCommand.RemoveMember cmd,
        CbuStatus status
    ) {
        List<Effect> effects = new java.util.ArrayList<>();
        effects.add(new Effect.RemoveMember(
            cmd.cbuId(),
            cmd.hardDelete() != null ? cmd.hardDelete() : false
        ));
        effects.add(new Effect.EmitPendingStateAdvance(
            cmd.cbuId(),
            "cbu-group-member:removed",
            "cbu/group-membership",
            "cbu-group.remove-member"
        ));
        return new DecisionOutcome.Accept(List.copyOf(effects));
    }
}
