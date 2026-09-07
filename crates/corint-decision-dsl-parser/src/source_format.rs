//! Presentation constraints must be checked before YAML is decoded to values.
use std::collections::HashMap;
use yaml_rust2::parser::{Event, Parser};

#[derive(Debug, Clone, thiserror::Error)]
#[error("ruleset.rules must use a block sequence, with one '- rule_id' per line; inline lists (including []), aliases and JSON arrays are not supported (line {line}, column {column})")]
pub struct RulesFormatError {
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum Path {
    Root,
    Ruleset,
    Rules,
    Other,
}

struct Frame {
    path: Path,
    mapping: bool,
    key: Option<String>,
}

/// Check literal `ruleset.rules` block sequences in every source document.
/// Other arrays (including import.rules) retain their existing syntax.
/// Callers must also parse YAML normally: this checks presentation, not validity.
pub fn validate_rules_format(source: &str) -> Result<(), RulesFormatError> {
    let chars: Vec<char> = source.chars().collect();
    let mut parser = Parser::new_from_str(source);
    let mut stack: Vec<Frame> = Vec::new();
    let mut scalar_anchors = HashMap::new();
    while let Ok((event, mark)) = parser.next_token() {
        match event {
            Event::StreamEnd => break,
            Event::DocumentStart | Event::DocumentEnd => {
                stack.clear();
                scalar_anchors.clear();
            }
            Event::MappingEnd | Event::SequenceEnd => {
                stack.pop();
            }
            Event::Scalar(..)
            | Event::Alias(..)
            | Event::MappingStart(..)
            | Event::SequenceStart(..) => {
                if let Event::Scalar(value, _, anchor, _) = &event {
                    if *anchor != 0 {
                        scalar_anchors.insert(*anchor, value.clone());
                    }
                }
                let path = if let Some(parent) = stack.last_mut() {
                    if parent.mapping && parent.key.is_none() {
                        parent.key = Some(match &event {
                            Event::Scalar(key, ..) => key.clone(),
                            Event::Alias(anchor) => {
                                scalar_anchors.get(anchor).cloned().unwrap_or_default()
                            }
                            _ => String::new(),
                        });
                        Path::Other
                    } else if parent.mapping {
                        match (parent.path, parent.key.take().as_deref()) {
                            (Path::Root, Some("ruleset")) => Path::Ruleset,
                            (Path::Ruleset, Some("rules")) => Path::Rules,
                            _ => Path::Other,
                        }
                    } else {
                        Path::Other
                    }
                } else {
                    Path::Root
                };
                if (path == Path::Rules
                    && (!matches!(event, Event::SequenceStart(..))
                        || chars.get(mark.index()) == Some(&'[')))
                    || (path == Path::Ruleset && matches!(event, Event::Alias(..)))
                {
                    return Err(RulesFormatError {
                        line: mark.line(),
                        column: mark.col() + 1,
                    });
                }
                if matches!(event, Event::MappingStart(..) | Event::SequenceStart(..)) {
                    stack.push(Frame {
                        path,
                        mapping: matches!(event, Event::MappingStart(..)),
                        key: None,
                    });
                }
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_block_lists_without_restricting_other_fields() {
        for source in [
            "ruleset:\n  rules:\n    - first\n    - second\n",
            "ruleset:\n  'rules': # comment\n  - first\n",
            "import: {rules: [rules/a.yaml]}\n---\nruleset:\n  rules: &ids\n    - first\n",
            "ruleset:\n  extends: base\n  description: |\n    rules: [example]\n",
            "metadata: {rules: [example]}\nruleset:\n  rules:\n    - first\n",
        ] {
            assert!(validate_rules_format(source).is_ok(), "{source}");
        }
    }

    #[test]
    fn rejects_flow_lists_and_alias_bypasses() {
        for source in [
            "ruleset:\n  rules: [first, second]\n",
            "ruleset:\n  rules: [\n    first,\n    second\n  ]\n",
            "ruleset:\n  rules: []\n",
            "ruleset:\n  rules:\n",
            "ruleset:\n  \"rules\": &ids [first]\n",
            "ruleset:\n  description: &key rules\n  *key : [first]\n",
            "ids: &ids [first]\nruleset:\n  rules: *ids\n",
            "base: &base {rules: [first]}\nruleset: *base\n",
            "{\"ruleset\": {\"rules\": [\"first\"]}}",
            "version: '0.1'\n---\nruleset:\n  rules: [first]\n",
        ] {
            assert!(validate_rules_format(source).is_err(), "{source}");
        }
        let error = validate_rules_format("# 中文\nruleset:\n  rules: [first]\n").unwrap_err();
        assert_eq!((error.line, error.column), (3, 10));
    }
}
