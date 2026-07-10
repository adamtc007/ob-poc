package com.ob.poc.cbu.model;

import java.util.UUID;
import java.util.List;

public record CbuRoleValidationDto(
    UUID cbuId,
    boolean valid,
    List<String> issues,
    String cbuCategory,
    String clientType
) {}
