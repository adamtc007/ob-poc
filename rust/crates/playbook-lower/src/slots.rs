use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Default)]
pub struct SlotState {
    values: HashMap<String, SlotValue>,
}

#[derive(Debug, Clone)]
pub enum SlotValue {
    Uuid(Uuid),
    String(String),
}

impl SlotState {
    pub fn new() -> Self { Self::default() }
    
    pub fn set(&mut self, name: &str, value: SlotValue) {
        self.values.insert(name.to_string(), value);
    }
    
    pub fn get(&self, name: &str) -> Option<&SlotValue> {
        self.values.get(name)
    }
}
