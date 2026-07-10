package com.ob.poc.cbu.model;

import java.util.UUID;
import java.time.LocalDate;

public record CbuPartyDto(
    UUID cbuId,
    UUID entityId,
    UUID roleId,
    String entityName,
    String entityType,
    String roleName,
    LocalDate effectiveFrom,
    LocalDate effectiveTo
) {}
