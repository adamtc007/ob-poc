package com.ob.poc.cbu.model;

import java.util.UUID;
import java.util.Set;

public sealed interface CbuCommand {
    UUID id();
    Principal actor();

    record Principal(String username, Set<String> roles) {}

    record Suspend(
        UUID id,
        Principal actor,
        String reason
    ) implements CbuCommand {}

    record Create(
        UUID id, // Can be null for genesis creation, or auto-generated
        Principal actor,
        String name,
        String jurisdiction,
        UUID fundEntityId,
        UUID mancoEntityId,
        String clientType,
        String naturePurpose,
        String description,
        UUID commercialClientEntityId
    ) implements CbuCommand {}

    record SubmitForValidation(UUID id, Principal actor) implements CbuCommand {}
    record Confirm(UUID id, Principal actor) implements CbuCommand {}
    record Reject(UUID id, Principal actor) implements CbuCommand {}
    record RequestProofUpdate(UUID id, Principal actor) implements CbuCommand {}
    record ReopenValidation(UUID id, Principal actor) implements CbuCommand {}
    record ResubmitProof(UUID id, Principal actor) implements CbuCommand {}

    record Reinstate(UUID id, Principal actor) implements CbuCommand {}
    record Restrict(UUID id, Principal actor, String scope) implements CbuCommand {}
    record Unrestrict(UUID id, Principal actor) implements CbuCommand {}
    record BeginWindingDown(UUID id, Principal actor, String reason) implements CbuCommand {}
    record CompleteOffboard(UUID id, Principal actor) implements CbuCommand {}

    record FlagForRemediation(UUID id, Principal actor, String reason) implements CbuCommand {}
    record ClearRemediation(UUID id, Principal actor) implements CbuCommand {}
    record SoftDelete(UUID id, Principal actor, String reason) implements CbuCommand {}
    record Restore(UUID id, Principal actor) implements CbuCommand {}
    record HardDelete(UUID id, Principal actor) implements CbuCommand {}

    record LinkStructure(
        UUID parentCbuId,
        Principal actor,
        UUID childCbuId,
        String relationshipType,
        String relationshipSelector,
        String capitalFlow,
        java.time.LocalDate effectiveFrom,
        java.time.LocalDate effectiveTo
    ) implements CbuCommand {
        @Override
        public UUID id() {
            return parentCbuId;
        }
    }

    record UnlinkStructure(
        UUID linkId,
        Principal actor,
        String reason,
        Boolean hardDelete
    ) implements CbuCommand {
        @Override
        public UUID id() {
            return linkId;
        }
    }

    record TerminateRole(
        UUID cbuId,
        Principal actor,
        Boolean hardDelete
    ) implements CbuCommand {
        @Override
        public UUID id() {
            return cbuId;
        }
    }

    record RemoveMember(
        UUID cbuId,
        Principal actor,
        Boolean hardDelete
    ) implements CbuCommand {
        @Override
        public UUID id() {
            return cbuId;
        }
    }

    record Rename(
        UUID cbuId,
        Principal actor,
        String name
    ) implements CbuCommand {
        @Override
        public UUID id() {
            return cbuId;
        }
    }

    record Update(
        UUID cbuId,
        Principal actor,
        String description,
        String naturePurpose,
        String category
    ) implements CbuCommand {
        @Override
        public UUID id() {
            return cbuId;
        }
    }

    record SetJurisdiction(
        UUID cbuId,
        Principal actor,
        String jurisdiction
    ) implements CbuCommand {
        @Override
        public UUID id() {
            return cbuId;
        }
    }

    record SetClientType(
        UUID cbuId,
        Principal actor,
        String clientType
    ) implements CbuCommand {
        @Override
        public UUID id() {
            return cbuId;
        }
    }

    record SetCommercialClient(
        UUID cbuId,
        Principal actor,
        UUID commercialClientEntityId
    ) implements CbuCommand {
        @Override
        public UUID id() {
            return cbuId;
        }
    }

    record SetCategory(
        UUID cbuId,
        Principal actor,
        String category
    ) implements CbuCommand {
        @Override
        public UUID id() {
            return cbuId;
        }
    }

    record AddProduct(
        UUID cbuId,
        Principal actor,
        String product,
        String configJson,
        String optionsJson
    ) implements CbuCommand {
        @Override
        public UUID id() {
            return cbuId;
        }
    }

    record RemoveProduct(
        UUID cbuId,
        Principal actor,
        String product
    ) implements CbuCommand {
        @Override
        public UUID id() {
            return cbuId;
        }
    }

    record SubmitForReview(
        UUID eventId,
        Principal actor
    ) implements CbuCommand {
        @Override
        public UUID id() {
            return eventId;
        }
    }

    record ApproveCa(
        UUID eventId,
        Principal actor
    ) implements CbuCommand {
        @Override
        public UUID id() {
            return eventId;
        }
    }

    record RejectCa(
        UUID eventId,
        Principal actor,
        String reason
    ) implements CbuCommand {
        @Override
        public UUID id() {
            return eventId;
        }
    }

    record WithdrawCa(
        UUID eventId,
        Principal actor
    ) implements CbuCommand {
        @Override
        public UUID id() {
            return eventId;
        }
    }

    record MarkImplementedCa(
        UUID eventId,
        Principal actor
    ) implements CbuCommand {
        @Override
        public UUID id() {
            return eventId;
        }
    }

    record AssignRole(
        UUID cbuId,
        Principal actor,
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
    ) implements CbuCommand {
        @Override
        public UUID id() {
            return cbuId;
        }
    }

    record RemoveRole(
        UUID cbuId,
        Principal actor,
        UUID entityId,
        String role
    ) implements CbuCommand {
        @Override
        public UUID id() {
            return cbuId;
        }
    }

    record AttachEvidence(
        UUID cbuId,
        Principal actor,
        UUID documentId,
        String attestationRef,
        String evidenceType,
        String evidenceCategory,
        String description,
        String attachedBy
    ) implements CbuCommand {
        @Override
        public UUID id() {
            return cbuId;
        }
    }

    record VerifyEvidence(
        UUID evidenceId,
        Principal actor,
        String verificationStatus,
        String verifiedBy,
        String verificationNotes
    ) implements CbuCommand {
        @Override
        public UUID id() {
            return evidenceId;
        }
    }

    record BindServiceOptions(
        UUID cbuId,
        Principal actor,
        UUID productId,
        UUID serviceId,
        String service,
        UUID serviceVersionId,
        String serviceVersion,
        String optionsJson,
        String sourceRef,
        String sourceVersion,
        UUID activationRunId
    ) implements CbuCommand {
        @Override
        public UUID id() {
            return cbuId;
        }
    }

    record OverrideOptionBinding(
        UUID cbuId,
        Principal actor,
        UUID serviceId,
        String service,
        UUID optionDefId,
        String optionKey,
        String value,
        UUID productId,
        String sourceRef,
        String sourceVersion,
        UUID activationRunId
    ) implements CbuCommand {
        @Override
        public UUID id() {
            return cbuId;
        }
    }

    record DirtyFlagBindings(
        UUID cbuId,
        Principal actor,
        UUID serviceId,
        String sourceKind
    ) implements CbuCommand {
        @Override
        public UUID id() {
            return cbuId;
        }
    }

    record RecomputeBindings(
        UUID cbuId,
        Principal actor,
        UUID serviceId
    ) implements CbuCommand {
        @Override
        public UUID id() {
            return cbuId;
        }
    }

    record CreateFromClientGroup(
        UUID groupId,
        Principal actor,
        String gleifCategory,
        String roleFilter,
        String jurisdictionFilter,
        String defaultJurisdiction,
        UUID mancoEntityId,
        Integer limit,
        Boolean dryRun
    ) implements CbuCommand {
        @Override
        public UUID id() {
            return groupId;
        }
    }

    record DeleteCascade(
        UUID cbuId,
        Principal actor,
        Boolean deleteEntities,
        Boolean hardDelete
    ) implements CbuCommand {
        @Override
        public UUID id() {
            return cbuId;
        }
    }

    record Ensure(
        UUID id,
        Principal actor,
        String name,
        String jurisdiction,
        String clientType,
        String naturePurpose,
        UUID commercialClientEntityId
    ) implements CbuCommand {}
}

