package com.ob.poc.cbu.model;

import java.util.UUID;
import java.util.List;

public record CbuOptionCoverageDto(
    UUID cbuId,
    UUID serviceId,
    String status,
    List<Gap> gaps
) {
    public record Gap(
        int level,
        String code,
        String optionKey
    ) {}
}
