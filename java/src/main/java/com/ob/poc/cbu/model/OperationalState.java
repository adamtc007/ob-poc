package com.ob.poc.cbu.model;

public sealed interface OperationalState permits
    OperationalState.PreValidated,
    OperationalState.OperationallyActive,
    OperationalState.Suspended,
    OperationalState.Restricted,
    OperationalState.WindingDown,
    OperationalState.Offboarded,
    OperationalState.Dormant,
    OperationalState.Archived
{
    record PreValidated() implements OperationalState {}
    record OperationallyActive() implements OperationalState {}
    record Suspended() implements OperationalState {}
    record Restricted() implements OperationalState {}
    record WindingDown() implements OperationalState {}
    record Offboarded() implements OperationalState {}
    record Dormant() implements OperationalState {}
    record Archived() implements OperationalState {}
}
