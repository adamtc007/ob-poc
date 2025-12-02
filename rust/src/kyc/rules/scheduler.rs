//! Scheduler for temporal rules (SLA checks, overdue documents, etc.)

use super::parser::Rule;

pub struct RuleScheduler {
    rules: Vec<Rule>,
}

impl RuleScheduler {
    pub fn new(rules: Vec<Rule>) -> Self {
        // Filter to scheduled rules only
        let scheduled_rules: Vec<Rule> = rules
            .into_iter()
            .filter(|r| r.trigger.is_scheduled())
            .collect();

        Self {
            rules: scheduled_rules,
        }
    }

    /// Get daily scheduled rules
    pub fn get_daily_rules(&self) -> Vec<&Rule> {
        self.rules
            .iter()
            .filter(|r| r.trigger.schedule.as_deref() == Some("daily"))
            .collect()
    }

    /// Get hourly scheduled rules
    pub fn get_hourly_rules(&self) -> Vec<&Rule> {
        self.rules
            .iter()
            .filter(|r| r.trigger.schedule.as_deref() == Some("hourly"))
            .collect()
    }

    /// Get all scheduled rules
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kyc::rules::parser::load_rules_from_str;

    #[test]
    fn test_scheduler_filters_scheduled_rules() {
        let yaml = r#"
rules:
  - name: daily-rule
    description: "Daily check"
    priority: 100
    trigger:
      event: scheduled
      schedule: daily
    condition:
      field: case.status
      equals: OPEN
    actions:
      - type: log-event
        params:
          event-type: DAILY_CHECK

  - name: event-rule
    description: "Event triggered"
    priority: 10
    trigger:
      event: workstream.created
    condition:
      field: entity.type
      equals: trust
    actions:
      - type: log-event
        params:
          event-type: TRUST_CREATED
"#;

        let all_rules = load_rules_from_str(yaml).unwrap();
        assert_eq!(all_rules.len(), 2);

        let scheduler = RuleScheduler::new(all_rules);
        assert_eq!(scheduler.rules().len(), 1);
        assert_eq!(scheduler.rules()[0].name, "daily-rule");
    }

    #[test]
    fn test_get_daily_rules() {
        let yaml = r#"
rules:
  - name: daily-rule
    description: "Daily check"
    priority: 100
    trigger:
      event: scheduled
      schedule: daily
    condition:
      field: case.status
      equals: OPEN
    actions:
      - type: log-event
        params:
          event-type: DAILY_CHECK

  - name: hourly-rule
    description: "Hourly check"
    priority: 100
    trigger:
      event: scheduled
      schedule: hourly
    condition:
      field: case.status
      equals: OPEN
    actions:
      - type: log-event
        params:
          event-type: HOURLY_CHECK
"#;

        let all_rules = load_rules_from_str(yaml).unwrap();
        let scheduler = RuleScheduler::new(all_rules);

        let daily = scheduler.get_daily_rules();
        assert_eq!(daily.len(), 1);
        assert_eq!(daily[0].name, "daily-rule");

        let hourly = scheduler.get_hourly_rules();
        assert_eq!(hourly.len(), 1);
        assert_eq!(hourly[0].name, "hourly-rule");
    }
}
