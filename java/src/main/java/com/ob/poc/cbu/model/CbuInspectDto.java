package com.ob.poc.cbu.model;

import java.util.UUID;
import java.util.List;

public record CbuInspectDto(
    UUID cbuId,
    String name,
    String jurisdiction,
    String clientType,
    String category,
    String naturePurpose,
    String description,
    String createdAt,
    String updatedAt,
    String asOfDate,
    List<EntityDetail> entities,
    List<DocumentDetail> documents,
    List<ServiceDetail> services,
    Summary summary
) {
    public record EntityDetail(
        UUID entityId,
        String name,
        String entityType,
        String jurisdiction,
        List<String> roles
    ) {}

    public record DocumentDetail(
        UUID docId,
        String name,
        String typeCode,
        String typeName,
        String status
    ) {}

    public record ServiceDetail(
        UUID deliveryId,
        String product,
        String productCode,
        String service,
        String status
    ) {}

    public record Summary(
        int entityCount,
        int documentCount,
        int serviceCount
    ) {}
}
