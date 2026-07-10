package com.ob.poc.cbu.model;

public sealed interface ValidationState {
    record ValidationPending() implements ValidationState {}
    record Validated() implements ValidationState {}
    record ValidationFailed() implements ValidationState {}
    record UpdatePendingProof() implements ValidationState {}
    record Evidenced() implements ValidationState {}
}
