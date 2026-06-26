package com.ob.poc.cbu.model;

import java.util.UUID;

public record CbuStatus(
    UUID id,
    String name,
    OperationalState op,
    ValidationState val,
    StructuralState struct,
    DispositionState disposition,
    String clientType,
    String jurisdiction,
    String rawStatus,
    String rawOpStatus,
    String rawDispStatus
) {
    public CbuStatus(
        UUID id,
        String name,
        OperationalState op,
        ValidationState val,
        StructuralState struct,
        DispositionState disposition,
        String clientType,
        String jurisdiction
    ) {
        this(id, name, op, val, struct, disposition, clientType, jurisdiction, null, null, null);
    }
}
