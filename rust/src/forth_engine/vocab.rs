//! Vocabulary and Word definitions for the DSL Forth Engine.

use crate::forth_engine::errors::VmError;
use crate::forth_engine::vm::VM;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WordId(pub usize);

pub type WordImpl = Arc<dyn for<'a> Fn(&mut VM<'a>) -> Result<(), VmError> + Send + Sync>;

#[derive(Clone)]
pub struct WordSpec {
    pub id: WordId,
    pub name: String,
    pub domain: String,
    pub stack_effect: (usize, usize), // (inputs, outputs)
    pub impl_fn: WordImpl,
}

pub struct Vocab {
    pub by_name: HashMap<String, WordId>,
    pub specs: Vec<WordSpec>,
}

impl Vocab {
    pub fn new(specs: Vec<WordSpec>) -> Self {
        let mut by_name = HashMap::new();
        for (i, spec) in specs.iter().enumerate() {
            by_name.insert(spec.name.clone(), WordId(i));
        }
        Vocab { by_name, specs }
    }

    pub fn lookup(&self, name: &str) -> Option<&WordId> {
        self.by_name.get(name)
    }

    pub fn spec(&self, id: &WordId) -> Option<&WordSpec> {
        self.specs.get(id.0)
    }
}
