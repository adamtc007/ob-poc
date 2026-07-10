package com.ob.poc.cbu.model;

public sealed interface DispositionState {
    record Active() implements DispositionState {}
    record UnderRemediation() implements DispositionState {}
    record SoftDeleted() implements DispositionState {}
    record HardDeleted() implements DispositionState {}
}
