package com.ob.poc.cbu.model;

import java.util.List;
import java.util.UUID;

public sealed interface CbuExecutionResult {
    record Success(
        UUID cbuId,
        boolean created,
        List<Object> events
    ) implements CbuExecutionResult {}

    record Failure(
        String reason
    ) implements CbuExecutionResult {}
}
