//! DSL Formatter - Pretty-print DSL source
#![allow(dead_code)]

/// DSL source code formatter
pub struct DslFormatter {
    indent_size: usize,
}

impl DslFormatter {
    pub fn new() -> Self {
        Self { indent_size: 2 }
    }
    
    pub fn with_indent(indent_size: usize) -> Self {
        Self { indent_size }
    }
    
    /// Format DSL source with proper indentation
    pub fn format(&self, dsl: &str) -> String {
        // Simple formatter - split on newlines and trim
        dsl.lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }
    
    /// Format with indentation for nested structures
    pub fn format_pretty(&self, dsl: &str) -> String {
        let mut result = String::new();
        let mut depth = 0;
        let indent = " ".repeat(self.indent_size);
        
        for ch in dsl.chars() {
            match ch {
                '(' => {
                    if !result.is_empty() && !result.ends_with('\n') && !result.ends_with(' ') {
                        result.push('\n');
                        result.push_str(&indent.repeat(depth));
                    }
                    result.push(ch);
                    depth += 1;
                }
                ')' => {
                    depth = depth.saturating_sub(1);
                    result.push(ch);
                }
                '\n' => {
                    result.push(ch);
                    result.push_str(&indent.repeat(depth));
                }
                _ => result.push(ch),
            }
        }
        
        result
    }
}

impl Default for DslFormatter {
    fn default() -> Self {
        Self::new()
    }
}
