use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct PlaybookSourceMap {
    pub step_spans: HashMap<usize, StepSpan>,
}

#[derive(Debug, Clone)]
pub struct SourceSpan {
    pub line: u32,
    pub column: u32,
    pub length: u32,
}

#[derive(Debug, Clone)]
pub struct StepSpan {
    pub verb: SourceSpan,
    pub args: HashMap<String, SourceSpan>,
}

impl PlaybookSourceMap {
    pub fn verb_span(&self, step_idx: usize) -> Option<&SourceSpan> {
        self.step_spans.get(&step_idx).map(|s| &s.verb)
    }
}
