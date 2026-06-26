package com.ob.poc.cbu.model;

import java.util.UUID;
import java.time.Instant;

public record CbuSubscriptionDto(
    UUID cbuId,
    String cbuName,
    String contractClient,
    UUID contractId,
    String productCode,
    Instant subscribedAt,
    UUID rateCardId,
    String rateCardName,
    String rateCardCurrency
) {}
