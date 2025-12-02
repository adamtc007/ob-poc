//! Build rule evaluation context from database and events

use chrono::{Duration, Utc};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

/// Context for rule evaluation containing all field values
#[derive(Debug, Clone, Default)]
pub struct RuleContext {
    values: HashMap<String, Value>,
}

impl RuleContext {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    pub fn set(&mut self, key: &str, value: impl Into<Value>) {
        self.values.insert(key.to_string(), value.into());
    }

    pub fn set_opt<T: Into<Value>>(&mut self, key: &str, value: Option<T>) {
        if let Some(v) = value {
            self.values.insert(key.to_string(), v.into());
        }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.values.get(key)
    }

    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.values.get(key).and_then(|v| v.as_str())
    }

    #[allow(dead_code)]
    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.values.get(key).and_then(|v| v.as_f64())
    }

    #[allow(dead_code)]
    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.values.get(key).and_then(|v| v.as_i64())
    }

    #[allow(dead_code)]
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.values.get(key).and_then(|v| v.as_bool())
    }

    pub fn get_uuid(&self, key: &str) -> Option<Uuid> {
        self.get_string(key).and_then(|s| Uuid::parse_str(s).ok())
    }

    /// Expand variables in a template string
    /// e.g., "Entity in ${entity.jurisdiction}" -> "Entity in KY"
    pub fn interpolate(&self, template: &str) -> String {
        let mut result = template.to_string();

        // Handle time expressions like ${now} and ${now + 3 days}
        result = self.interpolate_time_expressions(&result);

        // Handle field references like ${entity.jurisdiction}
        for (key, value) in &self.values {
            let placeholder = format!("${{{}}}", key);
            let replacement = match value {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                Value::Null => "null".to_string(),
                _ => value.to_string(),
            };
            result = result.replace(&placeholder, &replacement);
        }

        result
    }

    fn interpolate_time_expressions(&self, template: &str) -> String {
        let mut result = template.to_string();
        let now = Utc::now();

        // Replace ${now}
        result = result.replace("${now}", &now.to_rfc3339());

        // Replace ${now + N days}
        if let Ok(re) = regex::Regex::new(r"\$\{now\s*\+\s*(\d+)\s*days?\}") {
            result = re
                .replace_all(&result, |caps: &regex::Captures| {
                    let days: i64 = caps[1].parse().unwrap_or(0);
                    let future = now + Duration::days(days);
                    future.to_rfc3339()
                })
                .to_string();
        }

        // Replace ${now - N days}
        if let Ok(re) = regex::Regex::new(r"\$\{now\s*-\s*(\d+)\s*days?\}") {
            result = re
                .replace_all(&result, |caps: &regex::Captures| {
                    let days: i64 = caps[1].parse().unwrap_or(0);
                    let past = now - Duration::days(days);
                    past.to_rfc3339()
                })
                .to_string();
        }

        result
    }

    /// Snapshot context for audit logging
    pub fn snapshot(&self) -> Value {
        Value::Object(
            self.values
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_set_get() {
        let mut ctx = RuleContext::new();
        ctx.set("entity.jurisdiction", "KY");
        ctx.set("entity.type", "trust");

        assert_eq!(ctx.get_string("entity.jurisdiction"), Some("KY"));
        assert_eq!(ctx.get_string("entity.type"), Some("trust"));
    }

    #[test]
    fn test_interpolate() {
        let mut ctx = RuleContext::new();
        ctx.set("entity.jurisdiction", "KY");
        ctx.set("entity.name", "Test Corp");

        let result = ctx.interpolate("Entity ${entity.name} in ${entity.jurisdiction}");
        assert_eq!(result, "Entity Test Corp in KY");
    }

    #[test]
    fn test_get_uuid() {
        let mut ctx = RuleContext::new();
        let uuid = Uuid::new_v4();
        ctx.set("case.id", uuid.to_string());

        assert_eq!(ctx.get_uuid("case.id"), Some(uuid));
    }
}
