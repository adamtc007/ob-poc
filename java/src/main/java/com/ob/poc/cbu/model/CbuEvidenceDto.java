package com.ob.poc.cbu.model;

import java.util.UUID;
import java.time.Instant;

public record CbuEvidenceDto(
    UUID evidenceId,
    UUID cbuId,
    UUID documentId,
    String attestationRef,
    String evidenceType,
    String evidenceCategory,
    String description,
    Instant attachedAt,
    String attachedBy,
    Instant verifiedAt,
    String verifiedBy,
    String verificationStatus,
    String verificationNotes
) {}
