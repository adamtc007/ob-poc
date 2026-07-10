package com.ob.poc.cbu.model;

public sealed interface StructuralState {
    record Discovered() implements StructuralState {}
    record Structured() implements StructuralState {}
    record Configuring() implements StructuralState {}
}
