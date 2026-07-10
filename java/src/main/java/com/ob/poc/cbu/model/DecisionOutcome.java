package com.ob.poc.cbu.model;

import java.util.List;

public sealed interface DecisionOutcome {
    record Accept(List<Effect> effects) implements DecisionOutcome {}
    record Refuse(String reason) implements DecisionOutcome {}
}
