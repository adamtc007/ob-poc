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
        UUID existingLinkId,
        UUID existingFundCbuId,
        boolean linkExists,
        boolean roleExists,
        boolean memberExists,
        String caStatus
    ) {
        public DecisionContext(boolean isFundLinkedAlready) {
            this(isFundLinkedAlready, false, false, null, null, false, false, false, null);
        }
        public DecisionContext(boolean isFundLinkedAlready, UUID existingFundCbuId) {
            this(isFundLinkedAlready, false, false, null, existingFundCbuId, false, false, false, null);
        }
        public DecisionContext(boolean parentExists, boolean childExists, UUID existingLinkId) {
            this(false, parentExists, childExists, existingLinkId, null, false, false, false, null);
        }
        public static DecisionContext forUnlink(boolean linkExists) {
            return new DecisionContext(false, false, false, null, null, linkExists, false, false, null);
        }
        public static DecisionContext forRole(boolean roleExists) {
            return new DecisionContext(false, false, false, null, null, false, roleExists, false, null);
        }
        public static DecisionContext forMember(boolean memberExists) {
            return new DecisionContext(false, false, false, null, null, false, false, memberExists, null);
        }
        public static DecisionContext forCa(String caStatus) {
            return new DecisionContext(false, false, false, null, null, false, false, false, caStatus);
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
            case CbuCommand.UnlinkStructure cmd -> decideUnlinkStructure(cmd, currentStatus, context);
            case CbuCommand.TerminateRole cmd -> decideTerminateRole(cmd, currentStatus, context);
            case CbuCommand.RemoveMember cmd -> decideRemoveMember(cmd, currentStatus, context);
            case CbuCommand.Rename cmd -> decideRename(cmd, currentStatus);
            case CbuCommand.Update cmd -> decideUpdate(cmd, currentStatus);
            case CbuCommand.SetJurisdiction cmd -> decideSetJurisdiction(cmd, currentStatus);
            case CbuCommand.SetClientType cmd -> decideSetClientType(cmd, currentStatus);
            case CbuCommand.SetCommercialClient cmd -> decideSetCommercialClient(cmd, currentStatus);
            case CbuCommand.SetCategory cmd -> decideSetCategory(cmd, currentStatus);
            case CbuCommand.AddProduct cmd -> decideAddProduct(cmd, currentStatus);
            case CbuCommand.RemoveProduct cmd -> decideRemoveProduct(cmd, currentStatus);
            case CbuCommand.SubmitForReview cmd -> decideSubmitForReview(cmd, context);
            case CbuCommand.ApproveCa cmd -> decideApproveCa(cmd, context);
            case CbuCommand.RejectCa cmd -> decideRejectCa(cmd, context);
            case CbuCommand.WithdrawCa cmd -> decideWithdrawCa(cmd, context);
            case CbuCommand.MarkImplementedCa cmd -> decideMarkImplementedCa(cmd, context);
            case CbuCommand.AssignRole cmd -> decideAssignRole(cmd, currentStatus);
            case CbuCommand.RemoveRole cmd -> decideRemoveRole(cmd, currentStatus);
            case CbuCommand.AttachEvidence cmd -> decideAttachEvidence(cmd, currentStatus);
            case CbuCommand.VerifyEvidence cmd -> decideVerifyEvidence(cmd);
            case CbuCommand.BindServiceOptions cmd -> decideBindServiceOptions(cmd, currentStatus);
            case CbuCommand.OverrideOptionBinding cmd -> decideOverrideOptionBinding(cmd, currentStatus);
            case CbuCommand.DirtyFlagBindings cmd -> decideDirtyFlagBindings(cmd, currentStatus);
            case CbuCommand.RecomputeBindings cmd -> decideRecomputeBindings(cmd, currentStatus);
            case CbuCommand.CreateFromClientGroup cmd -> decideCreateFromClientGroup(cmd);
            case CbuCommand.DeleteCascade cmd -> decideDeleteCascade(cmd, currentStatus);
            case CbuCommand.Ensure cmd -> decideEnsure(cmd);
        };
    }

    private static DecisionOutcome decideSuspend(CbuCommand.Suspend cmd, CbuStatus status) {
        if (status == null) {
            return new DecisionOutcome.Refuse("CBU not found");
        }
        
        // Disposition check (must be active or under remediation)
        boolean isValidDisp = switch (status.disposition()) {
            case DispositionState.Active(), DispositionState.UnderRemediation() -> true;
            case DispositionState.SoftDeleted(), DispositionState.HardDeleted() -> false;
            case null -> false;
        };
        if (!isValidDisp) {
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
        boolean isValidOp = switch (status.op()) {
            case OperationalState.OperationallyActive() -> true;
            case OperationalState.PreValidated(),
                 OperationalState.Suspended(),
                 OperationalState.Restricted(),
                 OperationalState.WindingDown(),
                 OperationalState.Offboarded(),
                 OperationalState.Dormant(),
                 OperationalState.Archived() -> false;
            case null -> false;
        };
        if (!isValidOp) {
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
        boolean canSubmit = switch (status.val()) {
            case null -> (status.struct() instanceof StructuralState.Discovered);
            case ValidationState.ValidationFailed() -> true;
            case ValidationState.ValidationPending(),
                 ValidationState.Validated(),
                 ValidationState.UpdatePendingProof(),
                 ValidationState.Evidenced() -> false;
        };
        if (!canSubmit) {
            return new DecisionOutcome.Refuse("CBU must be in DISCOVERED or VALIDATION_FAILED status to submit for validation");
        }
        String statusStr = status.rawStatus() != null ? status.rawStatus().toUpperCase() : "DISCOVERED";
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateValidationStatus(cmd.id(), statusStr, "VALIDATION_PENDING")
        ));
    }

    private static DecisionOutcome decideConfirm(CbuCommand.Confirm cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        boolean isValid = switch (status.val()) {
            case ValidationState.ValidationPending() -> true;
            case ValidationState.Validated(),
                 ValidationState.ValidationFailed(),
                 ValidationState.UpdatePendingProof(),
                 ValidationState.Evidenced() -> false;
            case null -> false;
        };
        if (!isValid) {
            return new DecisionOutcome.Refuse("CBU must be in VALIDATION_PENDING status to confirm");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateValidationStatus(cmd.id(), "VALIDATION_PENDING", "VALIDATED")
        ));
    }

    private static DecisionOutcome decideReject(CbuCommand.Reject cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        boolean isValid = switch (status.val()) {
            case ValidationState.ValidationPending() -> true;
            case ValidationState.Validated(),
                 ValidationState.ValidationFailed(),
                 ValidationState.UpdatePendingProof(),
                 ValidationState.Evidenced() -> false;
            case null -> false;
        };
        if (!isValid) {
            return new DecisionOutcome.Refuse("CBU must be in VALIDATION_PENDING status to reject");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateValidationStatus(cmd.id(), "VALIDATION_PENDING", "VALIDATION_FAILED")
        ));
    }

    private static DecisionOutcome decideRequestProofUpdate(CbuCommand.RequestProofUpdate cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        boolean isValid = switch (status.val()) {
            case ValidationState.Validated() -> true;
            case ValidationState.ValidationPending(),
                 ValidationState.ValidationFailed(),
                 ValidationState.UpdatePendingProof(),
                 ValidationState.Evidenced() -> false;
            case null -> false;
        };
        if (!isValid) {
            return new DecisionOutcome.Refuse("CBU must be in VALIDATED status to request proof update");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateValidationStatus(cmd.id(), "VALIDATED", "UPDATE_PENDING_PROOF")
        ));
    }

    private static DecisionOutcome decideReopenValidation(CbuCommand.ReopenValidation cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        boolean isValid = switch (status.val()) {
            case ValidationState.ValidationFailed() -> true;
            case ValidationState.ValidationPending(),
                 ValidationState.Validated(),
                 ValidationState.UpdatePendingProof(),
                 ValidationState.Evidenced() -> false;
            case null -> false;
        };
        if (!isValid) {
            return new DecisionOutcome.Refuse("CBU must be in VALIDATION_FAILED status to reopen validation");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateValidationStatus(cmd.id(), "VALIDATION_FAILED", "VALIDATION_PENDING")
        ));
    }

    private static DecisionOutcome decideResubmitProof(CbuCommand.ResubmitProof cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        boolean isValid = switch (status.val()) {
            case ValidationState.UpdatePendingProof() -> true;
            case ValidationState.ValidationPending(),
                 ValidationState.Validated(),
                 ValidationState.ValidationFailed(),
                 ValidationState.Evidenced() -> false;
            case null -> false;
        };
        if (!isValid) {
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
        boolean isValid = switch (status.op()) {
            case OperationalState.Suspended() -> true;
            case OperationalState.PreValidated(),
                 OperationalState.OperationallyActive(),
                 OperationalState.Restricted(),
                 OperationalState.WindingDown(),
                 OperationalState.Offboarded(),
                 OperationalState.Dormant(),
                 OperationalState.Archived() -> false;
            case null -> false;
        };
        if (!isValid) {
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
        boolean isValid = switch (status.op()) {
            case OperationalState.OperationallyActive() -> true;
            case OperationalState.PreValidated(),
                 OperationalState.Suspended(),
                 OperationalState.Restricted(),
                 OperationalState.WindingDown(),
                 OperationalState.Offboarded(),
                 OperationalState.Dormant(),
                 OperationalState.Archived() -> false;
            case null -> false;
        };
        if (!isValid) {
            return new DecisionOutcome.Refuse("CBU must be in actively_trading or trade_permissioned state to restrict");
        }
        String opStatusStr = status.rawOpStatus() != null ? status.rawOpStatus().toLowerCase() : "actively_trading";
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateOperationalStatus(cmd.id(), opStatusStr, "restricted", null)
        ));
    }

    private static DecisionOutcome decideUnrestrict(CbuCommand.Unrestrict cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        if (!(status.val() instanceof ValidationState.Validated)) {
            return new DecisionOutcome.Refuse("CBU must be VALIDATED before operational status can be modified");
        }
        boolean isValid = switch (status.op()) {
            case OperationalState.Restricted() -> true;
            case OperationalState.PreValidated(),
                 OperationalState.OperationallyActive(),
                 OperationalState.Suspended(),
                 OperationalState.WindingDown(),
                 OperationalState.Offboarded(),
                 OperationalState.Dormant(),
                 OperationalState.Archived() -> false;
            case null -> false;
        };
        if (!isValid) {
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
        boolean isValidOp = switch (status.op()) {
            case OperationalState.OperationallyActive(),
                 OperationalState.Suspended(),
                 OperationalState.Restricted() -> true;
            case OperationalState.PreValidated(),
                 OperationalState.WindingDown(),
                 OperationalState.Offboarded(),
                 OperationalState.Dormant(),
                 OperationalState.Archived() -> false;
            case null -> false;
        };
        if (!isValidOp) {
            return new DecisionOutcome.Refuse("CBU must be in an active, restricted, or suspended state to wind down");
        }
        String opStatusStr = status.rawOpStatus() != null ? status.rawOpStatus().toLowerCase() : "actively_trading";
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
        boolean isValid = switch (status.op()) {
            case OperationalState.WindingDown() -> true;
            case OperationalState.PreValidated(),
                 OperationalState.OperationallyActive(),
                 OperationalState.Suspended(),
                 OperationalState.Restricted(),
                 OperationalState.Offboarded(),
                 OperationalState.Dormant(),
                 OperationalState.Archived() -> false;
            case null -> false;
        };
        if (!isValid) {
            return new DecisionOutcome.Refuse("CBU must be in winding_down state to complete offboarding");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateOperationalStatus(cmd.id(), "winding_down", "offboarded", null)
        ));
    }

    private static DecisionOutcome decideFlagForRemediation(CbuCommand.FlagForRemediation cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        boolean isValid = switch (status.disposition()) {
            case DispositionState.Active() -> true;
            case DispositionState.UnderRemediation(),
                 DispositionState.SoftDeleted(),
                 DispositionState.HardDeleted() -> false;
            case null -> false;
        };
        if (!isValid) {
            return new DecisionOutcome.Refuse("CBU must be active to flag for remediation");
        }
        String dispStatusStr = status.rawDispStatus() != null ? status.rawDispStatus().toLowerCase() : "active";
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateDispositionStatus(cmd.id(), dispStatusStr, "under_remediation", cmd.reason())
        ));
    }

    private static DecisionOutcome decideClearRemediation(CbuCommand.ClearRemediation cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        boolean isValid = switch (status.disposition()) {
            case DispositionState.UnderRemediation() -> true;
            case DispositionState.Active(),
                 DispositionState.SoftDeleted(),
                 DispositionState.HardDeleted() -> false;
            case null -> false;
        };
        if (!isValid) {
            return new DecisionOutcome.Refuse("CBU must be under remediation to clear it");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateDispositionStatus(cmd.id(), "under_remediation", "active", null)
        ));
    }

    private static DecisionOutcome decideSoftDelete(CbuCommand.SoftDelete cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        boolean isValid = switch (status.disposition()) {
            case DispositionState.Active(), DispositionState.UnderRemediation() -> true;
            case DispositionState.SoftDeleted(), DispositionState.HardDeleted() -> false;
            case null -> false;
        };
        if (!isValid) {
            return new DecisionOutcome.Refuse("CBU must be active or under remediation to soft delete");
        }
        String dispStatusStr = status.rawDispStatus() != null ? status.rawDispStatus().toLowerCase() : "active";
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateDispositionStatus(cmd.id(), dispStatusStr, "soft_deleted", cmd.reason())
        ));
    }

    private static DecisionOutcome decideRestore(CbuCommand.Restore cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        boolean isValid = switch (status.disposition()) {
            case DispositionState.SoftDeleted() -> true;
            case DispositionState.Active(),
                 DispositionState.UnderRemediation(),
                 DispositionState.HardDeleted() -> false;
            case null -> false;
        };
        if (!isValid) {
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
        boolean isValid = switch (status.disposition()) {
            case DispositionState.SoftDeleted() -> true;
            case DispositionState.Active(),
                 DispositionState.UnderRemediation(),
                 DispositionState.HardDeleted() -> false;
            case null -> false;
        };
        if (!isValid) {
            return new DecisionOutcome.Refuse("CBU must be soft deleted to hard delete");
        }
        String dispStatusStr = status.rawDispStatus() != null ? status.rawDispStatus().toLowerCase() : "soft_deleted";
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
        CbuStatus status,
        DecisionContext context
    ) {
        if (context == null || !context.linkExists()) {
            return new DecisionOutcome.Refuse("Structure link not found or inactive: " + cmd.linkId());
        }
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
        CbuStatus status,
        DecisionContext context
    ) {
        if (context == null || !context.roleExists()) {
            return new DecisionOutcome.Refuse("Role not found or inactive for CBU: " + cmd.cbuId());
        }
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
        CbuStatus status,
        DecisionContext context
    ) {
        if (context == null || !context.memberExists()) {
            return new DecisionOutcome.Refuse("Member not found or inactive for CBU: " + cmd.cbuId());
        }
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

    private static DecisionOutcome decideRename(CbuCommand.Rename cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        if (!(status.disposition() instanceof DispositionState.Active) 
            && !(status.disposition() instanceof DispositionState.UnderRemediation)) {
            return new DecisionOutcome.Refuse("CBU is not in a live disposition state");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateCbuName(cmd.cbuId(), cmd.name())
        ));
    }

    private static DecisionOutcome decideUpdate(CbuCommand.Update cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        if (!(status.disposition() instanceof DispositionState.Active) 
            && !(status.disposition() instanceof DispositionState.UnderRemediation)) {
            return new DecisionOutcome.Refuse("CBU is not in a live disposition state");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateCbuFields(cmd.cbuId(), cmd.description(), cmd.naturePurpose(), cmd.category())
        ));
    }

    private static DecisionOutcome decideSetJurisdiction(CbuCommand.SetJurisdiction cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        if (!(status.disposition() instanceof DispositionState.Active) 
            && !(status.disposition() instanceof DispositionState.UnderRemediation)) {
            return new DecisionOutcome.Refuse("CBU is not in a live disposition state");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateCbuJurisdiction(cmd.cbuId(), cmd.jurisdiction())
        ));
    }

    private static DecisionOutcome decideSetClientType(CbuCommand.SetClientType cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        if (!(status.disposition() instanceof DispositionState.Active) 
            && !(status.disposition() instanceof DispositionState.UnderRemediation)) {
            return new DecisionOutcome.Refuse("CBU is not in a live disposition state");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateCbuClientType(cmd.cbuId(), cmd.clientType())
        ));
    }

    private static DecisionOutcome decideSetCommercialClient(CbuCommand.SetCommercialClient cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        if (!(status.disposition() instanceof DispositionState.Active) 
            && !(status.disposition() instanceof DispositionState.UnderRemediation)) {
            return new DecisionOutcome.Refuse("CBU is not in a live disposition state");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateCbuCommercialClient(cmd.cbuId(), cmd.commercialClientEntityId())
        ));
    }

    private static DecisionOutcome decideSetCategory(CbuCommand.SetCategory cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        if (!(status.disposition() instanceof DispositionState.Active) 
            && !(status.disposition() instanceof DispositionState.UnderRemediation)) {
            return new DecisionOutcome.Refuse("CBU is not in a live disposition state");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.UpdateCbuCategory(cmd.cbuId(), cmd.category())
        ));
    }

    private static DecisionOutcome decideAddProduct(CbuCommand.AddProduct cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        if (!(status.disposition() instanceof DispositionState.Active) 
            && !(status.disposition() instanceof DispositionState.UnderRemediation)) {
            return new DecisionOutcome.Refuse("CBU is not in a live disposition state");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.AddProduct(cmd.cbuId(), cmd.product(), cmd.configJson())
        ));
    }

    private static DecisionOutcome decideRemoveProduct(CbuCommand.RemoveProduct cmd, CbuStatus status) {
        if (status == null) return new DecisionOutcome.Refuse("CBU not found");
        if (!(status.disposition() instanceof DispositionState.Active) 
            && !(status.disposition() instanceof DispositionState.UnderRemediation)) {
            return new DecisionOutcome.Refuse("CBU is not in a live disposition state");
        }
        return new DecisionOutcome.Accept(List.of(
            new Effect.RemoveProduct(cmd.cbuId(), cmd.product())
        ));
    }

    private static DecisionOutcome decideSubmitForReview(CbuCommand.SubmitForReview cmd, DecisionContext context) {
        if (context == null || context.caStatus() == null) {
            return new DecisionOutcome.Refuse("Corporate Action event not found");
        }
        if (!"proposed".equals(context.caStatus())) {
            return new DecisionOutcome.Refuse("Corporate Action must be in proposed state to submit for review, actual: " + context.caStatus());
        }
        Set<String> actorRoles = cmd.actor().roles();
        boolean hasComplianceRole = actorRoles.contains("compliance_officer") 
            || actorRoles.contains("senior_compliance") 
            || actorRoles.contains("mlro");
        if (!hasComplianceRole) {
            return new DecisionOutcome.Refuse("Principal does not have compliance roles required to submit for review");
        }
        return new DecisionOutcome.Accept(List.of(new Effect.UpdateCaStatus(cmd.eventId(), "proposed", "under_review", null)));
    }

    private static DecisionOutcome decideApproveCa(CbuCommand.ApproveCa cmd, DecisionContext context) {
        if (context == null || context.caStatus() == null) {
            return new DecisionOutcome.Refuse("Corporate Action event not found");
        }
        if (!"under_review".equals(context.caStatus())) {
            return new DecisionOutcome.Refuse("Corporate Action must be in under_review state to approve, actual: " + context.caStatus());
        }
        Set<String> actorRoles = cmd.actor().roles();
        boolean hasComplianceRole = actorRoles.contains("compliance_officer") 
            || actorRoles.contains("senior_compliance") 
            || actorRoles.contains("mlro");
        if (!hasComplianceRole) {
            return new DecisionOutcome.Refuse("Principal does not have compliance roles required to approve corporate action");
        }
        return new DecisionOutcome.Accept(List.of(new Effect.UpdateCaStatus(cmd.eventId(), "under_review", "approved", null)));
    }

    private static DecisionOutcome decideRejectCa(CbuCommand.RejectCa cmd, DecisionContext context) {
        if (context == null || context.caStatus() == null) {
            return new DecisionOutcome.Refuse("Corporate Action event not found");
        }
        if (!"under_review".equals(context.caStatus())) {
            return new DecisionOutcome.Refuse("Corporate Action must be in under_review state to reject, actual: " + context.caStatus());
        }
        Set<String> actorRoles = cmd.actor().roles();
        boolean hasComplianceRole = actorRoles.contains("compliance_officer") 
            || actorRoles.contains("senior_compliance") 
            || actorRoles.contains("mlro");
        if (!hasComplianceRole) {
            return new DecisionOutcome.Refuse("Principal does not have compliance roles required to reject corporate action");
        }
        return new DecisionOutcome.Accept(List.of(new Effect.UpdateCaStatus(cmd.eventId(), "under_review", "rejected", cmd.reason())));
    }

    private static DecisionOutcome decideWithdrawCa(CbuCommand.WithdrawCa cmd, DecisionContext context) {
        if (context == null || context.caStatus() == null) {
            return new DecisionOutcome.Refuse("Corporate Action event not found");
        }
        if (!"proposed".equals(context.caStatus()) && !"under_review".equals(context.caStatus())) {
            return new DecisionOutcome.Refuse("Corporate Action must be in proposed or under_review state to withdraw, actual: " + context.caStatus());
        }
        return new DecisionOutcome.Accept(List.of(new Effect.UpdateCaStatus(cmd.eventId(), context.caStatus(), "withdrawn", null)));
    }

    private static DecisionOutcome decideMarkImplementedCa(CbuCommand.MarkImplementedCa cmd, DecisionContext context) {
        if (context == null || context.caStatus() == null) {
            return new DecisionOutcome.Refuse("Corporate Action event not found");
        }
        if (!"effective".equals(context.caStatus())) {
            return new DecisionOutcome.Refuse("Corporate Action must be in effective state to mark implemented, actual: " + context.caStatus());
        }
        return new DecisionOutcome.Accept(List.of(new Effect.UpdateCaStatus(cmd.eventId(), "effective", "implemented", null)));
    }

    private static DecisionOutcome decideAssignRole(CbuCommand.AssignRole cmd, CbuStatus status) {
        if (status == null) {
            return new DecisionOutcome.Refuse("CBU not found");
        }
        List<Effect> effects = new java.util.ArrayList<>();
        effects.add(new Effect.AssignRoleEffect(
            cmd.cbuId(),
            cmd.roleType(),
            cmd.role(),
            cmd.ownerEntityId(),
            cmd.ownedEntityId(),
            cmd.controllerEntityId(),
            cmd.controlledEntityId(),
            cmd.trustEntityId(),
            cmd.participantEntityId(),
            cmd.entityId(),
            cmd.fundEntityId(),
            cmd.providerEntityId(),
            cmd.clientEntityId(),
            cmd.personEntityId(),
            cmd.forEntityId(),
            cmd.percentage(),
            cmd.ownershipType(),
            cmd.effectiveFrom(),
            cmd.controlType(),
            cmd.appointmentDate(),
            cmd.interestPercentage(),
            cmd.interestType(),
            cmd.classDescription(),
            cmd.investmentPercentage(),
            cmd.isRegulated(),
            cmd.regulatoryJurisdiction(),
            cmd.serviceAgreementDate(),
            cmd.authorityLimit(),
            cmd.authorityCurrency(),
            cmd.requiresCoSignatory(),
            cmd.expectedVersion()
        ));
        
        effects.add(new Effect.EmitPendingStateAdvance(
            cmd.cbuId(),
            "cbu-role:assigned",
            "cbu/role-graph",
            "assigned role under cbu " + cmd.cbuId()
        ));
        
        return new DecisionOutcome.Accept(List.copyOf(effects));
    }

    private static DecisionOutcome decideRemoveRole(CbuCommand.RemoveRole cmd, CbuStatus status) {
        if (status == null) {
            return new DecisionOutcome.Refuse("CBU not found");
        }
        return new DecisionOutcome.Accept(List.of(new Effect.RemoveRoleEffect(
            cmd.cbuId(),
            cmd.entityId(),
            cmd.role()
        )));
    }

    private static DecisionOutcome decideAttachEvidence(CbuCommand.AttachEvidence cmd, CbuStatus status) {
        if (status == null) {
            return new DecisionOutcome.Refuse("CBU not found");
        }
        return new DecisionOutcome.Accept(List.of(new Effect.AttachEvidence(
            cmd.cbuId(),
            cmd.documentId(),
            cmd.attestationRef(),
            cmd.evidenceType(),
            cmd.evidenceCategory(),
            cmd.description(),
            cmd.attachedBy()
        )));
    }

    private static DecisionOutcome decideVerifyEvidence(CbuCommand.VerifyEvidence cmd) {
        return new DecisionOutcome.Accept(List.of(new Effect.VerifyEvidence(
            cmd.evidenceId(),
            cmd.verificationStatus(),
            cmd.verifiedBy(),
            cmd.verificationNotes()
        )));
    }

    private static DecisionOutcome decideBindServiceOptions(CbuCommand.BindServiceOptions cmd, CbuStatus status) {
        if (status == null) {
            return new DecisionOutcome.Refuse("CBU not found");
        }
        return new DecisionOutcome.Accept(List.of(new Effect.BindServiceOptions(
            cmd.cbuId(),
            cmd.productId(),
            cmd.serviceId(),
            cmd.service(),
            cmd.serviceVersionId(),
            cmd.serviceVersion(),
            cmd.optionsJson(),
            cmd.sourceRef(),
            cmd.sourceVersion(),
            cmd.activationRunId()
        )));
    }

    private static DecisionOutcome decideOverrideOptionBinding(CbuCommand.OverrideOptionBinding cmd, CbuStatus status) {
        if (status == null) {
            return new DecisionOutcome.Refuse("CBU not found");
        }
        return new DecisionOutcome.Accept(List.of(new Effect.OverrideOptionBinding(
            cmd.cbuId(),
            cmd.serviceId(),
            cmd.service(),
            cmd.optionDefId(),
            cmd.optionKey(),
            cmd.value(),
            cmd.productId(),
            cmd.sourceRef(),
            cmd.sourceVersion(),
            cmd.activationRunId()
        )));
    }

    private static DecisionOutcome decideDirtyFlagBindings(CbuCommand.DirtyFlagBindings cmd, CbuStatus status) {
        if (status == null) {
            return new DecisionOutcome.Refuse("CBU not found");
        }
        return new DecisionOutcome.Accept(List.of(new Effect.DirtyFlagBindings(
            cmd.cbuId(),
            cmd.serviceId(),
            cmd.sourceKind()
        )));
    }

    private static DecisionOutcome decideRecomputeBindings(CbuCommand.RecomputeBindings cmd, CbuStatus status) {
        if (status == null) {
            return new DecisionOutcome.Refuse("CBU not found");
        }
        return new DecisionOutcome.Accept(List.of(new Effect.RecomputeBindings(
            cmd.cbuId(),
            cmd.serviceId()
        )));
    }

    private static DecisionOutcome decideCreateFromClientGroup(CbuCommand.CreateFromClientGroup cmd) {
        return new DecisionOutcome.Accept(List.of(new Effect.CreateFromClientGroup(
            cmd.groupId(),
            cmd.gleifCategory(),
            cmd.roleFilter(),
            cmd.jurisdictionFilter(),
            cmd.defaultJurisdiction(),
            cmd.mancoEntityId(),
            cmd.limit(),
            cmd.dryRun()
        )));
    }

    private static DecisionOutcome decideDeleteCascade(CbuCommand.DeleteCascade cmd, CbuStatus status) {
        if (status == null) {
            return new DecisionOutcome.Refuse("CBU not found");
        }
        Set<String> actorRoles = cmd.actor().roles();
        boolean hasRequiredRole = actorRoles.contains("compliance_admin") 
            || actorRoles.contains("senior_compliance") 
            || actorRoles.contains("mlro");
        if (!hasRequiredRole) {
            return new DecisionOutcome.Refuse("Principal does not have roles required to delete cascade");
        }
        return new DecisionOutcome.Accept(List.of(new Effect.DeleteCascade(
            cmd.cbuId(),
            cmd.deleteEntities(),
            cmd.hardDelete()
        )));
    }

    private static DecisionOutcome decideEnsure(CbuCommand.Ensure cmd) {
        return new DecisionOutcome.Accept(List.of(new Effect.EnsureCbu(
            cmd.id(),
            cmd.name(),
            cmd.jurisdiction(),
            cmd.clientType(),
            cmd.naturePurpose(),
            cmd.commercialClientEntityId()
        )));
    }
}

