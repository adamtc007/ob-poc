package com.ob.poc.cbu.model;

import java.util.UUID;

public record CbuResourceFanoutDto(
    UUID serviceId,
    UUID resourceId,
    String fanoutAxis,
    String fanoutValue
) {}
