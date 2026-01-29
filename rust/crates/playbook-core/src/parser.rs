use crate::{PlaybookSourceMap, PlaybookSpec, SourceSpan, StepSpan};
use marked_yaml::parse_yaml;
use std::collections::HashMap;

pub struct ParseOutput {
    pub spec: PlaybookSpec,
    pub source_map: PlaybookSourceMap,
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("YAML error at line {line}: {msg}")]
    Yaml { line: u32, msg: String },
}

/// Two-pass parsing:
/// 1. serde_yaml for typed PlaybookSpec deserialization
/// 2. marked-yaml for source span tracking via Marker{line, col}
pub fn parse_playbook(source: &str) -> Result<ParseOutput, ParseError> {
    // Pass 1: Parse into typed spec
    let spec: PlaybookSpec = serde_yaml::from_str(source).map_err(|e| ParseError::Yaml {
        line: e.location().map(|l| l.line() as u32).unwrap_or(1),
        msg: e.to_string(),
    })?;

    // Pass 2: Parse with marked-yaml for source locations
    let source_map = build_source_map_marked(source, &spec)?;

    Ok(ParseOutput { spec, source_map })
}

/// Build source map using marked-yaml's Marker API
fn build_source_map_marked(
    source: &str,
    spec: &PlaybookSpec,
) -> Result<PlaybookSourceMap, ParseError> {
    let mut map = PlaybookSourceMap::default();

    // Parse with marked-yaml (file_id = 0)
    let node = parse_yaml(0, source).map_err(|e| ParseError::Yaml {
        line: 1,
        msg: e.to_string(),
    })?;

    // Navigate to steps array
    let root = match node.as_mapping() {
        Some(m) => m,
        None => return Ok(map),
    };

    let steps_node = match root.get_node("steps") {
        Some(n) => n,
        None => return Ok(map),
    };

    let steps = match steps_node.as_sequence() {
        Some(s) => s,
        None => return Ok(map),
    };

    // Extract spans for each step
    for (idx, step_node) in steps.iter().enumerate() {
        if idx >= spec.steps.len() {
            break;
        }

        if let Some(step_map) = step_node.as_mapping() {
            let mut step_span = StepSpan {
                verb: SourceSpan {
                    line: 0,
                    column: 0,
                    length: 0,
                },
                args: HashMap::new(),
            };

            // Extract verb span
            if let Some(verb_scalar) = step_map.get_scalar("verb") {
                let span = verb_scalar.span();
                if let Some(marker) = span.start() {
                    step_span.verb = SourceSpan {
                        line: marker.line() as u32,
                        column: marker.column() as u32,
                        length: verb_scalar.as_str().len() as u32,
                    };
                }
            }

            // Extract args spans
            if let Some(args_map) = step_map.get_mapping("args") {
                for (key, value) in args_map.iter() {
                    let key_str = key.as_str();
                    if let Some(val_scalar) = value.as_scalar() {
                        let span = val_scalar.span();
                        if let Some(marker) = span.start() {
                            step_span.args.insert(
                                key_str.to_string(),
                                SourceSpan {
                                    line: marker.line() as u32,
                                    column: marker.column() as u32,
                                    length: val_scalar.as_str().len() as u32,
                                },
                            );
                        }
                    }
                }
            }

            map.step_spans.insert(idx, step_span);
        }
    }

    Ok(map)
}
