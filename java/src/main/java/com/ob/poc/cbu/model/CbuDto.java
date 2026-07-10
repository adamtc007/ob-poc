package com.ob.poc.cbu.model;

import java.util.UUID;
import java.time.Instant;

public record CbuDto(
    UUID cbuId,
    String name,
    String description,
    String naturePurpose,
    String clientType,
    String jurisdiction,
    String category,
    String status,
    String operationalStatus,
    String dispositionStatus,
    String discoveryState,
    Instant createdAt,
    Instant updatedAt
) {}
