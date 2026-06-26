package com.ob.poc.cbu.model;

import java.util.UUID;
import java.time.LocalDate;

public record CbuStructureLinkDto(
    UUID linkId,
    UUID parentCbuId,
    String parentName,
    UUID childCbuId,
    String childName,
    String relationshipType,
    String relationshipSelector,
    String status,
    String capitalFlow,
    LocalDate effectiveFrom,
    LocalDate effectiveTo
) {}
