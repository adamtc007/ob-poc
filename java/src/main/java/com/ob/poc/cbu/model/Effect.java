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
}
