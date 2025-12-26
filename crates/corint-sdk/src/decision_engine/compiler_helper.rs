//! Rule compilation and loading utilities

use crate::error::{Result, SdkError};
use corint_compiler::{Compiler, CompilerOptions as CompilerOpts};
use corint_core::ir::Program;
use corint_parser::{PipelineParser, RegistryParser, RuleParser, RulesetParser};
use std::collections::HashMap;
use std::path::Path;

pub(super) struct CompilerHelper;

impl CompilerHelper {
pub(super) async fn load_and_compile_rules(path: &Path, compiler: &mut Compiler) -> Result<Vec<Program>> {
    use corint_parser::YamlParser;

    // Read file
    let content = tokio::fs::read_to_string(path).await?;

    tracing::debug!("Loading file: {}", path.display());

    let mut programs = Vec::new();
    let mut has_pipeline = false;
    let mut pipeline_count = 0;

    // Parse multi-document YAML (supports files with --- separators)
    let documents = YamlParser::parse_multi_document(&content)?;

    // Try to parse each document
    for doc in documents.iter() {
        // Try rule first
        if let Ok(rule) = RuleParser::parse_from_yaml(doc) {
            let prog = compiler.compile_rule(&rule)?;
            programs.push(prog);
            continue;
        }

        // Try ruleset
        if let Ok(ruleset) = RulesetParser::parse_from_yaml(doc) {
            let prog = compiler.compile_ruleset(&ruleset)?;
            programs.push(prog);
            continue;
        }

        // Try pipeline
        if let Ok(pipeline) = PipelineParser::parse_from_yaml(doc) {
            has_pipeline = true;
            pipeline_count += 1;

            // Validate: Pipeline must have when condition
            if pipeline.when.is_none() {
                return Err(SdkError::InvalidRuleFile(format!(
                    "Pipeline '{}' in file '{}' is missing mandatory 'when' condition. \
                     All pipelines must specify when conditions to filter events.",
                    &pipeline.id,
                    path.display()
                )));
            }

            tracing::debug!(
                "Parsed pipeline: when={:?}, steps={}",
                pipeline.when,
                pipeline.steps.len()
            );
            let prog = compiler.compile_pipeline(&pipeline)?;
            tracing::debug!(
                "Compiled pipeline: {} instructions",
                prog.instructions.len()
            );
            programs.push(prog);
            continue;
        }

        // Skip documents that don't match any known type (e.g., metadata sections)
    }

    // If no valid documents were found, return error
    if programs.is_empty() {
        return Err(SdkError::InvalidRuleFile(format!(
            "File does not contain a valid rule, ruleset, or pipeline: {}",
            path.display()
        )));
    }

    // Validate: File must contain at least one pipeline
    if !has_pipeline {
        return Err(SdkError::InvalidRuleFile(format!(
            "Rule file '{}' must contain at least one pipeline definition. \
             Pipelines are the entry points for rule execution and must have 'when' conditions. \
             Rules and rulesets cannot be used as top-level entry points.",
            path.display()
        )));
    }

    tracing::info!(
        "✓ Loaded file '{}': {} pipeline(s), {} total definitions",
        path.display(),
        pipeline_count,
        programs.len()
    );

    Ok(programs)
}

/// Compile rules from content string (from repository)
pub(super) async fn compile_rules_from_content(
    id: &str,
    content: &str,
    compiler: &mut Compiler,
) -> Result<Vec<Program>> {
    use corint_parser::YamlParser;

    tracing::debug!("Compiling content from: {}", id);

    let mut programs = Vec::new();
    let mut has_pipeline = false;
    let mut pipeline_count = 0;

    // First, try to parse as a pipeline with imports (most common case for repository content)
    if let Ok(document) = corint_parser::PipelineParser::parse_with_imports(content) {
        has_pipeline = true;
        pipeline_count += 1;

        // Validate: Pipeline must have when condition
        if document.definition.when.is_none() {
            return Err(SdkError::InvalidRuleFile(format!(
                "Pipeline '{}' from '{}' is missing mandatory 'when' condition. \
                 All pipelines must specify when conditions to filter events.",
                &document.definition.id,
                id
            )));
        }

        tracing::debug!(
            "Parsed pipeline with imports: when={:?}, steps={}, imports={:?}",
            document.definition.when,
            document.definition.steps.len(),
            document.imports.is_some()
        );

        // Resolve imports and compile dependencies
        let resolved = compiler
            .import_resolver_mut()
            .resolve_imports(&document)
            .map_err(SdkError::CompileError)?;

        tracing::debug!(
            "Resolved imports: {} rules, {} rulesets",
            resolved.rules.len(),
            resolved.rulesets.len()
        );

        // Compile all resolved rules first
        for rule in &resolved.rules {
            let rule_prog = compiler.compile_rule(rule)?;
            programs.push(rule_prog);
        }

        // Compile all resolved rulesets
        for ruleset in &resolved.rulesets {
            let ruleset_prog = compiler.compile_ruleset(ruleset)?;
            programs.push(ruleset_prog);
        }

        // Finally compile the pipeline itself
        let prog = compiler.compile_pipeline(&document.definition)?;
        tracing::debug!(
            "Compiled pipeline: {} instructions",
            prog.instructions.len()
        );
        programs.push(prog);

        // IMPORTANT: Also parse inline rules and rulesets from the same YAML file
        // This supports the format where pipeline, rules, and rulesets are in the same file
        let documents = YamlParser::parse_multi_document(content)?;
        if documents.len() > 1 {
            tracing::debug!(
                "Found {} documents in file, checking for inline rules/rulesets",
                documents.len()
            );

            use corint_parser::{RuleParser, RulesetParser};

            for (doc_idx, doc) in documents.iter().enumerate() {
                // Skip the first document (already processed as pipeline metadata)
                if doc_idx == 0 {
                    continue;
                }

                // Skip if it's the pipeline definition (already compiled above)
                if doc.get("pipeline").is_some() {
                    continue;
                }

                // Try to parse as rule
                if let Ok(rule) = RuleParser::parse_from_yaml(doc) {
                    tracing::debug!("Found inline rule: {}", rule.id);
                    let rule_prog = compiler.compile_rule(&rule)?;
                    programs.push(rule_prog);
                    continue;
                }

                // Try to parse as ruleset
                if let Ok(ruleset) = RulesetParser::parse_from_yaml(doc) {
                    tracing::debug!("Found inline ruleset: {}", ruleset.id);
                    let ruleset_prog = compiler.compile_ruleset(&ruleset)?;
                    programs.push(ruleset_prog);
                    continue;
                }
            }
        }
    } else {
        // Fallback: Parse as multi-document YAML for individual rules/rulesets
        let documents = YamlParser::parse_multi_document(content)?;

        // Try to parse each document
        for doc in documents.iter() {
            // Try rule first
            if let Ok(rule) = RuleParser::parse_from_yaml(doc) {
                let prog = compiler.compile_rule(&rule)?;
                programs.push(prog);
                continue;
            }

            // Try ruleset
            if let Ok(ruleset) = RulesetParser::parse_from_yaml(doc) {
                let prog = compiler.compile_ruleset(&ruleset)?;
                programs.push(prog);
                continue;
            }

            // Try pipeline (without imports, since parse_with_imports already failed above)
            if let Ok(pipeline) = PipelineParser::parse_from_yaml(doc) {
                has_pipeline = true;
                pipeline_count += 1;

                // Validate: Pipeline must have when condition
                if pipeline.when.is_none() {
                    return Err(SdkError::InvalidRuleFile(format!(
                        "Pipeline '{}' from '{}' is missing mandatory 'when' condition. \
                     All pipelines must specify when conditions to filter events.",
                        &pipeline.id,
                        id
                    )));
                }

                let prog = compiler.compile_pipeline(&pipeline)?;
                tracing::debug!(
                    "Compiled pipeline (no imports): {} instructions",
                    prog.instructions.len()
                );
                programs.push(prog);
                continue;
            }

            // Skip documents that don't match any known type (e.g., metadata sections)
        }
    } // Close the else block

    // If no valid documents were found, return error
    if programs.is_empty() {
        return Err(SdkError::InvalidRuleFile(format!(
            "Content from '{}' does not contain a valid rule, ruleset, or pipeline",
            id
        )));
    }

    // Validate: Content must contain at least one pipeline
    if !has_pipeline {
        return Err(SdkError::InvalidRuleFile(format!(
            "Content from '{}' must contain at least one pipeline definition. \
             Pipelines are the entry points for rule execution and must have 'when' conditions. \
             Rules and rulesets cannot be used as top-level entry points.",
            id
        )));
    }

    tracing::info!(
        "✓ Loaded content '{}': {} pipeline(s), {} total definitions",
        id,
        pipeline_count,
        programs.len()
    );

    Ok(programs)
}

/// Load registry from file
pub(super) async fn load_registry(path: &Path) -> Result<corint_core::ast::PipelineRegistry> {
    let content = tokio::fs::read_to_string(path).await?;
    let registry = RegistryParser::parse(&content)?;
    Ok(registry)
}

}
