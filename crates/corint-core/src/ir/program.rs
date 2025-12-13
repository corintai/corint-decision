//! IR Program
//!
//! A program is a sequence of IR instructions with associated metadata.

use crate::ir::Instruction;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// An IR program ready for execution
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Program {
    /// The sequence of instructions
    pub instructions: Vec<Instruction>,

    /// Program metadata
    pub metadata: ProgramMetadata,
}

/// Metadata associated with a program
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProgramMetadata {
    /// Source rule/ruleset/pipeline ID
    pub source_id: String,

    /// Source type ("rule", "ruleset", or "pipeline")
    pub source_type: String,

    /// Optional name
    pub name: Option<String>,

    /// Optional description
    pub description: Option<String>,

    /// Custom metadata fields
    #[serde(default)]
    pub custom: HashMap<String, String>,

    /// Version of the compiler that generated this
    pub compiler_version: String,
}

impl Program {
    /// Create a new program
    pub fn new(instructions: Vec<Instruction>, metadata: ProgramMetadata) -> Self {
        Self {
            instructions,
            metadata,
        }
    }

    /// Get the number of instructions
    pub fn instruction_count(&self) -> usize {
        self.instructions.len()
    }

    /// Check if program is empty
    pub fn is_empty(&self) -> bool {
        self.instructions.is_empty()
    }

    /// Add an instruction to the end
    pub fn push_instruction(&mut self, instruction: Instruction) {
        self.instructions.push(instruction);
    }

    /// Get instruction at index
    pub fn get_instruction(&self, index: usize) -> Option<&Instruction> {
        self.instructions.get(index)
    }
}

impl ProgramMetadata {
    /// Create new metadata for a rule
    pub fn for_rule(rule_id: String) -> Self {
        Self {
            source_id: rule_id,
            source_type: "rule".to_string(),
            name: None,
            description: None,
            custom: HashMap::new(),
            compiler_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Create new metadata for a ruleset
    pub fn for_ruleset(ruleset_id: String) -> Self {
        Self {
            source_id: ruleset_id,
            source_type: "ruleset".to_string(),
            name: None,
            description: None,
            custom: HashMap::new(),
            compiler_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Create new metadata for a pipeline
    pub fn for_pipeline(pipeline_id: String) -> Self {
        Self {
            source_id: pipeline_id,
            source_type: "pipeline".to_string(),
            name: None,
            description: None,
            custom: HashMap::new(),
            compiler_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Set the name
    pub fn with_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    /// Set the description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Add a custom metadata field
    pub fn with_custom(mut self, key: String, value: String) -> Self {
        self.custom.insert(key, value);
        self
    }
}

impl Default for ProgramMetadata {
    fn default() -> Self {
        Self {
            source_id: "unknown".to_string(),
            source_type: "unknown".to_string(),
            name: None,
            description: None,
            custom: HashMap::new(),
            compiler_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::Instruction;
    use crate::Value;

    #[test]
    fn test_program_creation() {
        let instructions = vec![
            Instruction::LoadConst {
                value: Value::Number(42.0),
            },
            Instruction::Return,
        ];

        let metadata = ProgramMetadata::for_rule("test_rule".to_string())
            .with_name("Test Rule".to_string())
            .with_description("A test rule".to_string());

        let program = Program::new(instructions, metadata);

        assert_eq!(program.instruction_count(), 2);
        assert!(!program.is_empty());
        assert_eq!(program.metadata.source_id, "test_rule");
        assert_eq!(program.metadata.source_type, "rule");
    }

    #[test]
    fn test_program_modification() {
        let mut program = Program::new(vec![], ProgramMetadata::for_rule("test".to_string()));

        assert!(program.is_empty());

        program.push_instruction(Instruction::LoadConst {
            value: Value::Number(1.0),
        });

        program.push_instruction(Instruction::Return);

        assert_eq!(program.instruction_count(), 2);
        assert!(!program.is_empty());
    }

    #[test]
    fn test_get_instruction() {
        let program = Program::new(
            vec![
                Instruction::LoadConst {
                    value: Value::Number(42.0),
                },
                Instruction::Return,
            ],
            ProgramMetadata::default(),
        );

        assert!(program.get_instruction(0).is_some());
        assert!(program.get_instruction(1).is_some());
        assert!(program.get_instruction(2).is_none());

        if let Some(Instruction::LoadConst { value }) = program.get_instruction(0) {
            assert_eq!(*value, Value::Number(42.0));
        } else {
            panic!("Expected LoadConst instruction");
        }
    }

    #[test]
    fn test_metadata_for_rule() {
        let metadata = ProgramMetadata::for_rule("my_rule".to_string())
            .with_name("My Rule".to_string())
            .with_description("This is my rule".to_string())
            .with_custom("author".to_string(), "Alice".to_string());

        assert_eq!(metadata.source_id, "my_rule");
        assert_eq!(metadata.source_type, "rule");
        assert_eq!(metadata.name, Some("My Rule".to_string()));
        assert_eq!(metadata.description, Some("This is my rule".to_string()));
        assert_eq!(metadata.custom.get("author"), Some(&"Alice".to_string()));
        assert!(!metadata.compiler_version.is_empty());
    }

    #[test]
    fn test_metadata_for_ruleset() {
        let metadata = ProgramMetadata::for_ruleset("fraud_detection".to_string());

        assert_eq!(metadata.source_id, "fraud_detection");
        assert_eq!(metadata.source_type, "ruleset");
    }

    #[test]
    fn test_metadata_for_pipeline() {
        let metadata = ProgramMetadata::for_pipeline("main_pipeline".to_string());

        assert_eq!(metadata.source_id, "main_pipeline");
        assert_eq!(metadata.source_type, "pipeline");
    }

    #[test]
    fn test_default_metadata() {
        let metadata = ProgramMetadata::default();

        assert_eq!(metadata.source_id, "unknown");
        assert_eq!(metadata.source_type, "unknown");
        assert!(metadata.name.is_none());
        assert!(metadata.description.is_none());
        assert!(metadata.custom.is_empty());
    }

    #[test]
    fn test_program_serde() {
        let program = Program::new(
            vec![
                Instruction::LoadConst {
                    value: Value::Number(42.0),
                },
                Instruction::Return,
            ],
            ProgramMetadata::for_rule("test_rule".to_string()),
        );

        // Serialize to JSON
        let json = serde_json::to_string_pretty(&program).unwrap();

        // Debug: print the JSON to see what it looks like
        eprintln!("Generated JSON:\n{}", json);

        // Check for the fields (they should exist in the metadata object)
        assert!(
            json.contains("test_rule"),
            "JSON should contain 'test_rule'"
        );
        assert!(json.contains("rule"), "JSON should contain 'rule'");

        // Deserialize back
        let deserialized: Program = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, program);
    }

    #[test]
    fn test_complex_program() {
        // Simulate a compiled rule program
        let instructions = vec![
            // Check event type
            Instruction::CheckEventType {
                expected: "login".to_string(),
            },
            // Load user.age
            Instruction::LoadField {
                path: vec!["user".to_string(), "age".to_string()],
            },
            // Load constant 18
            Instruction::LoadConst {
                value: Value::Number(18.0),
            },
            // Compare: age > 18
            Instruction::Compare {
                op: crate::ast::Operator::Gt,
            },
            // If false, skip to end
            Instruction::JumpIfFalse { offset: 3 },
            // Set score
            Instruction::SetScore { value: 50 },
            // Mark rule triggered
            Instruction::MarkRuleTriggered {
                rule_id: "age_check".to_string(),
            },
            // Return
            Instruction::Return,
        ];

        let metadata = ProgramMetadata::for_rule("age_check".to_string())
            .with_name("Age Check Rule".to_string())
            .with_description("Check if user is over 18".to_string());

        let program = Program::new(instructions, metadata);

        assert_eq!(program.instruction_count(), 8);
        assert_eq!(program.metadata.source_type, "rule");
    }
}
