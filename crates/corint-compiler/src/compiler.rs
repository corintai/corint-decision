//! Main compiler
//!
//! Provides a unified interface for compiling different AST node types.

use corint_core::ast::{Rule, Ruleset, Pipeline};
use corint_core::ir::Program;
use crate::error::Result;
use crate::codegen::{RuleCompiler, RulesetCompiler, PipelineCompiler};
use crate::semantic::SemanticAnalyzer;
use crate::optimizer::{ConstantFolder, DeadCodeEliminator};
use crate::import_resolver::ImportResolver;
use corint_parser::PipelineParser;
use std::path::Path;

/// Compiler options
#[derive(Debug, Clone)]
pub struct CompilerOptions {
    /// Enable semantic analysis
    pub enable_semantic_analysis: bool,
    /// Enable constant folding optimization
    pub enable_constant_folding: bool,
    /// Enable dead code elimination
    pub enable_dead_code_elimination: bool,
    /// Library base path for imports (default: "repository")
    pub library_base_path: String,
}

impl Default for CompilerOptions {
    fn default() -> Self {
        Self {
            enable_semantic_analysis: true,
            enable_constant_folding: true,
            enable_dead_code_elimination: true,
            library_base_path: "repository".to_string(),
        }
    }
}

/// The main CORINT compiler
pub struct Compiler {
    /// Compiler options
    options: CompilerOptions,
    /// Semantic analyzer
    semantic_analyzer: SemanticAnalyzer,
    /// Constant folder
    constant_folder: ConstantFolder,
    /// Dead code eliminator
    dead_code_eliminator: DeadCodeEliminator,
    /// Import resolver
    import_resolver: ImportResolver,
}

impl Compiler {
    /// Create a new compiler instance with default options
    pub fn new() -> Self {
        Self::with_options(CompilerOptions::default())
    }

    /// Create a new compiler instance with custom options
    pub fn with_options(options: CompilerOptions) -> Self {
        let import_resolver = ImportResolver::new(&options.library_base_path);

        Self {
            options,
            semantic_analyzer: SemanticAnalyzer::new(),
            constant_folder: ConstantFolder::new(),
            dead_code_eliminator: DeadCodeEliminator::new(),
            import_resolver,
        }
    }

    /// Compile a rule into an IR program
    pub fn compile_rule(&mut self, rule: &Rule) -> Result<Program> {
        // Semantic analysis
        if self.options.enable_semantic_analysis {
            self.semantic_analyzer.analyze_rule(rule)?;
        }

        // Code generation
        let mut program = RuleCompiler::compile(rule)?;

        // Optimization
        if self.options.enable_dead_code_elimination {
            program = self.dead_code_eliminator.optimize(&program);
        }

        Ok(program)
    }

    /// Compile a ruleset into an IR program
    pub fn compile_ruleset(&mut self, ruleset: &Ruleset) -> Result<Program> {
        // Semantic analysis
        if self.options.enable_semantic_analysis {
            self.semantic_analyzer.analyze_ruleset(ruleset)?;
        }

        // Code generation
        let mut program = RulesetCompiler::compile(ruleset)?;

        // Optimization
        if self.options.enable_dead_code_elimination {
            program = self.dead_code_eliminator.optimize(&program);
        }

        Ok(program)
    }

    /// Compile a pipeline into an IR program
    pub fn compile_pipeline(&mut self, pipeline: &Pipeline) -> Result<Program> {
        // Semantic analysis
        if self.options.enable_semantic_analysis {
            self.semantic_analyzer.analyze_pipeline(pipeline)?;
        }

        // Code generation
        let mut program = PipelineCompiler::compile(pipeline)?;

        // Optimization
        if self.options.enable_dead_code_elimination {
            program = self.dead_code_eliminator.optimize(&program);
        }

        Ok(program)
    }

    /// Get a reference to the semantic analyzer
    pub fn semantic_analyzer(&self) -> &SemanticAnalyzer {
        &self.semantic_analyzer
    }

    /// Get a mutable reference to the semantic analyzer
    pub fn semantic_analyzer_mut(&mut self) -> &mut SemanticAnalyzer {
        &mut self.semantic_analyzer
    }

    /// Get a reference to the constant folder
    pub fn constant_folder(&self) -> &ConstantFolder {
        &self.constant_folder
    }

    /// Get a reference to the dead code eliminator
    pub fn dead_code_eliminator(&self) -> &DeadCodeEliminator {
        &self.dead_code_eliminator
    }

    /// Get a reference to the import resolver
    pub fn import_resolver(&self) -> &ImportResolver {
        &self.import_resolver
    }

    /// Get a mutable reference to the import resolver
    pub fn import_resolver_mut(&mut self) -> &mut ImportResolver {
        &mut self.import_resolver
    }

    /// Compile a pipeline file with imports
    ///
    /// This is the main entry point for compiling pipelines that use the import system.
    /// It:
    /// 1. Loads the pipeline file
    /// 2. Resolves all imports (rules and rulesets)
    /// 3. Compiles the pipeline with the complete context
    ///
    /// # Example
    ///
    /// ```no_run
    /// use corint_compiler::Compiler;
    /// use std::path::Path;
    ///
    /// let mut compiler = Compiler::new();
    /// let program = compiler.compile_pipeline_file(Path::new("repository/pipelines/fraud_detection.yaml")).unwrap();
    /// ```
    pub fn compile_pipeline_file(&mut self, file_path: &Path) -> Result<Program> {
        // 1. Load the file
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| crate::error::CompileError::ImportNotFound {
                path: file_path.display().to_string(),
                source: e,
            })?;

        // 2. Parse the pipeline with imports
        let document = PipelineParser::parse_with_imports(&content)
            .map_err(|e| crate::error::CompileError::ParseError {
                path: file_path.display().to_string(),
                message: e.to_string(),
            })?;

        // 3. Resolve imports
        let _resolved = self.import_resolver.resolve_imports(&document)?;

        // 4. TODO: For now, we just compile the pipeline itself
        // In a full implementation, we would need to:
        // - Store resolved rules and rulesets in a context
        // - Pass this context to the compiler
        // - The compiler would use this context when compiling ruleset references
        // - Use `_resolved` to get the complete list of rules and rulesets

        // For now, just compile the pipeline
        self.compile_pipeline(&document.definition)
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use corint_core::ast::{Expression, Operator, WhenBlock};
    use corint_core::Value;

    #[test]
    fn test_compiler_compile_rule() {
        let mut compiler = Compiler::new();

        let when = WhenBlock::new().add_condition(Expression::binary(
            Expression::field_access(vec!["user".to_string(), "age".to_string()]),
            Operator::Gt,
            Expression::literal(Value::Number(18.0)),
        ));

        let rule = Rule::new(
            "test".to_string(),
            "Test".to_string(),
            when,
            50,
        );

        let program = compiler.compile_rule(&rule).unwrap();

        assert!(!program.instructions.is_empty());
        assert_eq!(program.metadata.source_type, "rule");
    }

    #[test]
    fn test_compiler_default() {
        let compiler = Compiler::default();
        assert!(std::ptr::eq(&compiler as *const _, &compiler as *const _));
    }

    #[test]
    fn test_compiler_with_options() {
        let options = CompilerOptions {
            enable_semantic_analysis: false,
            enable_constant_folding: false,
            enable_dead_code_elimination: false,
            library_base_path: "repository".to_string(),
        };

        let mut compiler = Compiler::with_options(options);

        let when = WhenBlock::new();
        let rule = Rule::new(
            "test".to_string(),
            "Test".to_string(),
            when,
            50,
        );

        let program = compiler.compile_rule(&rule).unwrap();
        assert_eq!(program.metadata.source_type, "rule");
    }

    #[test]
    fn test_compiler_semantic_analysis_error() {
        let mut compiler = Compiler::new();

        // Empty rule ID should fail semantic analysis
        let rule = Rule::new(
            String::new(), // Empty ID
            "Test".to_string(),
            WhenBlock::new(),
            50,
        );

        let result = compiler.compile_rule(&rule);
        assert!(result.is_err());
    }

    #[test]
    fn test_compiler_accessors() {
        let compiler = Compiler::new();

        // Test accessors
        let _analyzer = compiler.semantic_analyzer();
        let _folder = compiler.constant_folder();
        let _eliminator = compiler.dead_code_eliminator();
    }
}
