package com.ob.poc.cbu.model;

import java.util.UUID;

public sealed interface Effect {
    record UpdateOperationalStatus(
        UUID cbuId,
        String fromStatus,
        String toStatus,
        String reason
    ) implements Effect {}

    record InsertCbu(
        UUID cbuId,
        String name,
        String jurisdiction,
        String clientType,
        String naturePurpose,
        String description,
        UUID commercialClientEntityId
    ) implements Effect {}

    record AssignFundRole(
        UUID cbuId,
        UUID entityId,
        String role
    ) implements Effect {}

    record LinkCbu(
        UUID cbuId,
        UUID entityId
    ) implements Effect {}

    record EmitPendingStateAdvance(
        UUID cbuId,
        String stateAdvanceKey,
        String advancementType,
        String description
    ) implements Effect {}

    record UpdateValidationStatus(
        UUID cbuId,
        String fromStatus,
        String toStatus
    ) implements Effect {}

    record UpdateDispositionStatus(
        UUID cbuId,
        String fromStatus,
        String toStatus,
        String reason
    ) implements Effect {}

    record LinkStructure(
        UUID parentCbuId,
        UUID childCbuId,
        String relationshipType,
        String relationshipSelector,
        String capitalFlow,
        java.time.LocalDate effectiveFrom,
        java.time.LocalDate effectiveTo,
        UUID existingLinkId
    ) implements Effect {}

    record UnlinkStructure(
        UUID linkId,
        String reason,
        Boolean hardDelete
    ) implements Effect {}

    record TerminateRole(
        UUID cbuId,
        Boolean hardDelete
    ) implements Effect {}

    record RemoveMember(
        UUID cbuId,
        Boolean hardDelete
    ) implements Effect {}

    record UpdateCbuName(
        UUID cbuId,
        String name
    ) implements Effect {}

    record UpdateCbuFields(
        UUID cbuId,
        String description,
        String naturePurpose,
        String category
    ) implements Effect {}

    record UpdateCbuJurisdiction(
        UUID cbuId,
        String jurisdiction
    ) implements Effect {}

    record UpdateCbuClientType(
        UUID cbuId,
        String clientType
    ) implements Effect {}

    record UpdateCbuCommercialClient(
        UUID cbuId,
        UUID commercialClientEntityId
    ) implements Effect {}

    record UpdateCbuCategory(
        UUID cbuId,
        String category
    ) implements Effect {}

    record AddProduct(
        UUID cbuId,
        String product,
        String configJson
    ) implements Effect {}

    record RemoveProduct(
        UUID cbuId,
        String product
    ) implements Effect {}

    record UpdateCaStatus(
        UUID eventId,
        String fromStatus,
        String toStatus,
        String rejectedReason
    ) implements Effect {}

    record AssignRoleEffect(
        UUID cbuId,
        String roleType,
        String role,
        UUID ownerEntityId,
        UUID ownedEntityId,
        UUID controllerEntityId,
        UUID controlledEntityId,
        UUID trustEntityId,
        UUID participantEntityId,
        UUID entityId,
        UUID fundEntityId,
        UUID providerEntityId,
        UUID clientEntityId,
        UUID personEntityId,
        UUID forEntityId,
        String percentage,
        String ownershipType,
        String effectiveFrom,
        String controlType,
        String appointmentDate,
        String interestPercentage,
        String interestType,
        String classDescription,
        String investmentPercentage,
        Boolean isRegulated,
        String regulatoryJurisdiction,
        String serviceAgreementDate,
        String authorityLimit,
        String authorityCurrency,
        Boolean requiresCoSignatory,
        Long expectedVersion
    ) implements Effect {}

    record RemoveRoleEffect(
        UUID cbuId,
        UUID entityId,
        String role
    ) implements Effect {}

    record AttachEvidence(
        UUID cbuId,
        UUID documentId,
        String attestationRef,
        String evidenceType,
        String evidenceCategory,
        String description,
        String attachedBy
    ) implements Effect {}

    record VerifyEvidence(
        UUID evidenceId,
        String verificationStatus,
        String verifiedBy,
        String verificationNotes
    ) implements Effect {}

    record BindServiceOptions(
        UUID cbuId,
        UUID productId,
        UUID serviceId,
        String service,
        UUID serviceVersionId,
        String serviceVersion,
        String optionsJson,
        String sourceRef,
        String sourceVersion,
        UUID activationRunId
    ) implements Effect {}

    record OverrideOptionBinding(
        UUID cbuId,
        UUID serviceId,
        String service,
        UUID optionDefId,
        String optionKey,
        String value,
        UUID productId,
        String sourceRef,
        String sourceVersion,
        UUID activationRunId
    ) implements Effect {}

    record DirtyFlagBindings(
        UUID cbuId,
        UUID serviceId,
        String sourceKind
    ) implements Effect {}

    record RecomputeBindings(
        UUID cbuId,
        UUID serviceId
    ) implements Effect {}

    record CreateFromClientGroup(
        UUID groupId,
        String gleifCategory,
        String roleFilter,
        String jurisdictionFilter,
        String defaultJurisdiction,
        UUID mancoEntityId,
        Integer limit,
        Boolean dryRun
    ) implements Effect {}

    record DeleteCascade(
        UUID cbuId,
        Boolean deleteEntities,
        Boolean hardDelete
    ) implements Effect {}

    record EnsureCbu(
        UUID id,
        String name,
        String jurisdiction,
        String clientType,
        String naturePurpose,
        UUID commercialClientEntityId
    ) implements Effect {}
}

