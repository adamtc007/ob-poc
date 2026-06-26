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
}
