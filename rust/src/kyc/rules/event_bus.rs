//! Event bus for routing KYC events to rules engine

use super::context::RuleContext;
use super::evaluator::RuleEvaluator;
use super::parser::Rule;
use anyhow::Result;
use uuid::Uuid;

/// KYC events that can trigger rules
#[derive(Debug, Clone)]
pub enum KycEvent {
    // Workstream events
    WorkstreamCreated {
        case_id: Uuid,
        workstream_id: Uuid,
        entity_id: Uuid,
    },
    WorkstreamStatusChanged {
        case_id: Uuid,
        workstream_id: Uuid,
        old_status: String,
        new_status: String,
    },
    WorkstreamBlocked {
        case_id: Uuid,
        workstream_id: Uuid,
    },
    WorkstreamCompleted {
        case_id: Uuid,
        workstream_id: Uuid,
    },

    // Screening events
    ScreeningStarted {
        case_id: Uuid,
        workstream_id: Uuid,
        screening_id: Uuid,
    },
    ScreeningCompleted {
        case_id: Uuid,
        workstream_id: Uuid,
        screening_id: Uuid,
    },
    ScreeningReviewed {
        case_id: Uuid,
        workstream_id: Uuid,
        screening_id: Uuid,
    },

    // Document events
    DocRequestCreated {
        case_id: Uuid,
        workstream_id: Uuid,
        request_id: Uuid,
    },
    DocRequestReceived {
        case_id: Uuid,
        workstream_id: Uuid,
        request_id: Uuid,
    },
    DocRequestVerified {
        case_id: Uuid,
        workstream_id: Uuid,
        request_id: Uuid,
    },
    DocRequestRejected {
        case_id: Uuid,
        workstream_id: Uuid,
        request_id: Uuid,
    },

    // Red flag events
    RedFlagRaised {
        case_id: Uuid,
        red_flag_id: Uuid,
    },
    RedFlagMitigated {
        case_id: Uuid,
        red_flag_id: Uuid,
    },
    RedFlagWaived {
        case_id: Uuid,
        red_flag_id: Uuid,
    },

    // Holding events
    HoldingCreated {
        case_id: Uuid,
        workstream_id: Uuid,
        holding_id: Uuid,
    },

    // Case events
    CaseCreated {
        case_id: Uuid,
    },
    CaseStatusChanged {
        case_id: Uuid,
        old_status: String,
        new_status: String,
    },
    CaseEscalated {
        case_id: Uuid,
        level: String,
    },
}

impl KycEvent {
    /// Get the event name for rule matching
    pub fn event_name(&self) -> &'static str {
        match self {
            KycEvent::WorkstreamCreated { .. } => "workstream.created",
            KycEvent::WorkstreamStatusChanged { .. } => "workstream.status-changed",
            KycEvent::WorkstreamBlocked { .. } => "workstream.blocked",
            KycEvent::WorkstreamCompleted { .. } => "workstream.completed",
            KycEvent::ScreeningStarted { .. } => "screening.started",
            KycEvent::ScreeningCompleted { .. } => "screening.completed",
            KycEvent::ScreeningReviewed { .. } => "screening.reviewed",
            KycEvent::DocRequestCreated { .. } => "doc-request.created",
            KycEvent::DocRequestReceived { .. } => "doc-request.received",
            KycEvent::DocRequestVerified { .. } => "doc-request.verified",
            KycEvent::DocRequestRejected { .. } => "doc-request.rejected",
            KycEvent::RedFlagRaised { .. } => "red-flag.raised",
            KycEvent::RedFlagMitigated { .. } => "red-flag.mitigated",
            KycEvent::RedFlagWaived { .. } => "red-flag.waived",
            KycEvent::HoldingCreated { .. } => "holding.created",
            KycEvent::CaseCreated { .. } => "case.created",
            KycEvent::CaseStatusChanged { .. } => "case.status-changed",
            KycEvent::CaseEscalated { .. } => "case.escalated",
        }
    }

    pub fn case_id(&self) -> Uuid {
        match self {
            KycEvent::WorkstreamCreated { case_id, .. } => *case_id,
            KycEvent::WorkstreamStatusChanged { case_id, .. } => *case_id,
            KycEvent::WorkstreamBlocked { case_id, .. } => *case_id,
            KycEvent::WorkstreamCompleted { case_id, .. } => *case_id,
            KycEvent::ScreeningStarted { case_id, .. } => *case_id,
            KycEvent::ScreeningCompleted { case_id, .. } => *case_id,
            KycEvent::ScreeningReviewed { case_id, .. } => *case_id,
            KycEvent::DocRequestCreated { case_id, .. } => *case_id,
            KycEvent::DocRequestReceived { case_id, .. } => *case_id,
            KycEvent::DocRequestVerified { case_id, .. } => *case_id,
            KycEvent::DocRequestRejected { case_id, .. } => *case_id,
            KycEvent::RedFlagRaised { case_id, .. } => *case_id,
            KycEvent::RedFlagMitigated { case_id, .. } => *case_id,
            KycEvent::RedFlagWaived { case_id, .. } => *case_id,
            KycEvent::HoldingCreated { case_id, .. } => *case_id,
            KycEvent::CaseCreated { case_id } => *case_id,
            KycEvent::CaseStatusChanged { case_id, .. } => *case_id,
            KycEvent::CaseEscalated { case_id, .. } => *case_id,
        }
    }

    #[allow(dead_code)]
    pub fn workstream_id(&self) -> Option<Uuid> {
        match self {
            KycEvent::WorkstreamCreated { workstream_id, .. } => Some(*workstream_id),
            KycEvent::WorkstreamStatusChanged { workstream_id, .. } => Some(*workstream_id),
            KycEvent::WorkstreamBlocked { workstream_id, .. } => Some(*workstream_id),
            KycEvent::WorkstreamCompleted { workstream_id, .. } => Some(*workstream_id),
            KycEvent::ScreeningStarted { workstream_id, .. } => Some(*workstream_id),
            KycEvent::ScreeningCompleted { workstream_id, .. } => Some(*workstream_id),
            KycEvent::ScreeningReviewed { workstream_id, .. } => Some(*workstream_id),
            KycEvent::DocRequestCreated { workstream_id, .. } => Some(*workstream_id),
            KycEvent::DocRequestReceived { workstream_id, .. } => Some(*workstream_id),
            KycEvent::DocRequestVerified { workstream_id, .. } => Some(*workstream_id),
            KycEvent::DocRequestRejected { workstream_id, .. } => Some(*workstream_id),
            KycEvent::HoldingCreated { workstream_id, .. } => Some(*workstream_id),
            _ => None,
        }
    }
}

/// Event bus that routes events to the rules engine
/// Note: This is a simplified version without database access.
/// For production use, this would be integrated with sqlx and the executor.
pub struct KycEventBus {
    rules: Vec<Rule>,
    evaluator: RuleEvaluator,
}

impl KycEventBus {
    pub fn new(rules: Vec<Rule>) -> Self {
        Self {
            rules,
            evaluator: RuleEvaluator::new(),
        }
    }

    /// Get rules that match the given event
    pub fn get_matching_rules(&self, event_name: &str) -> Vec<&Rule> {
        let mut matching: Vec<&Rule> = self
            .rules
            .iter()
            .filter(|r| r.trigger.matches_event(event_name))
            .collect();

        // Sort by priority (lower = higher priority)
        matching.sort_by_key(|r| r.priority);
        matching
    }

    /// Evaluate a rule against a context
    pub fn evaluate_rule(&self, rule: &Rule, context: &RuleContext) -> bool {
        self.evaluator.evaluate(&rule.condition, context)
    }

    /// Process an event with a pre-built context
    /// Returns the rules that matched and should have their actions executed
    pub fn process_event(&self, event: &KycEvent, context: &RuleContext) -> Result<Vec<&Rule>> {
        let event_name = event.event_name();
        tracing::debug!(
            event = %event_name,
            case_id = %event.case_id(),
            "Processing event"
        );

        let matching_rules = self.get_matching_rules(event_name);

        if matching_rules.is_empty() {
            return Ok(vec![]);
        }

        tracing::debug!(count = matching_rules.len(), "Found matching rules");

        // Evaluate each rule and collect those that match
        let triggered_rules: Vec<&Rule> = matching_rules
            .into_iter()
            .filter(|rule| {
                let matched = self.evaluate_rule(rule, context);
                tracing::debug!(rule = %rule.name, matched = matched, "Rule evaluated");
                matched
            })
            .collect();

        Ok(triggered_rules)
    }

    /// Get all rules
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_name() {
        let event = KycEvent::WorkstreamCreated {
            case_id: Uuid::new_v4(),
            workstream_id: Uuid::new_v4(),
            entity_id: Uuid::new_v4(),
        };
        assert_eq!(event.event_name(), "workstream.created");
    }

    #[test]
    fn test_case_id() {
        let case_id = Uuid::new_v4();
        let event = KycEvent::CaseCreated { case_id };
        assert_eq!(event.case_id(), case_id);
    }
}
