//! Import and dependency management AST definitions
//!
//! This module defines structures for the CORINT module system,
//! enabling reusable rules, rulesets, and pipelines.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

/// Import declaration for external dependencies
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Imports {
    /// Imported rule files
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<String>,

    /// Imported ruleset files
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rulesets: Vec<String>,

    /// Imported pipeline files
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pipelines: Vec<String>,

    /// Imported template files (future use)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub templates: Vec<String>,
}

/// RDL Document wraps the actual definition with optional imports
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RdlDocument<T> {
    /// File format version
    pub version: String,

    /// Optional imports section
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imports: Option<Imports>,

    /// The actual definition (Rule, Ruleset, or Pipeline)
    #[serde(flatten)]
    pub definition: T,
}

/// Import resolution context
#[derive(Debug, Clone)]
pub struct ImportContext {
    /// Base directory for resolving relative paths
    pub base_dir: PathBuf,

    /// Already loaded file paths (to detect circular dependencies)
    pub loaded_files: HashSet<PathBuf>,

    /// Repository root directory
    pub repository_root: Option<PathBuf>,
}

impl Imports {
    /// Create a new empty imports section
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a rule import
    pub fn add_rule(mut self, path: String) -> Self {
        self.rules.push(path);
        self
    }

    /// Add multiple rule imports
    pub fn with_rules(mut self, rules: Vec<String>) -> Self {
        self.rules = rules;
        self
    }

    /// Add a ruleset import
    pub fn add_ruleset(mut self, path: String) -> Self {
        self.rulesets.push(path);
        self
    }

    /// Add multiple ruleset imports
    pub fn with_rulesets(mut self, rulesets: Vec<String>) -> Self {
        self.rulesets = rulesets;
        self
    }

    /// Add a pipeline import
    pub fn add_pipeline(mut self, path: String) -> Self {
        self.pipelines.push(path);
        self
    }

    /// Add multiple pipeline imports
    pub fn with_pipelines(mut self, pipelines: Vec<String>) -> Self {
        self.pipelines = pipelines;
        self
    }

    /// Add a template import
    pub fn add_template(mut self, path: String) -> Self {
        self.templates.push(path);
        self
    }

    /// Add multiple template imports
    pub fn with_templates(mut self, templates: Vec<String>) -> Self {
        self.templates = templates;
        self
    }

    /// Check if there are any imports
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
            && self.rulesets.is_empty()
            && self.pipelines.is_empty()
            && self.templates.is_empty()
    }

    /// Get all import paths
    pub fn all_paths(&self) -> Vec<&str> {
        let mut paths = Vec::new();
        paths.extend(self.rules.iter().map(|s| s.as_str()));
        paths.extend(self.rulesets.iter().map(|s| s.as_str()));
        paths.extend(self.pipelines.iter().map(|s| s.as_str()));
        paths.extend(self.templates.iter().map(|s| s.as_str()));
        paths
    }
}

impl<T> RdlDocument<T> {
    /// Create a new RDL document without imports
    pub fn new(version: String, definition: T) -> Self {
        Self {
            version,
            imports: None,
            definition,
        }
    }

    /// Create a new RDL document with imports
    pub fn with_imports(version: String, imports: Imports, definition: T) -> Self {
        Self {
            version,
            imports: Some(imports),
            definition,
        }
    }

    /// Get the imports, or an empty Imports if none
    pub fn imports(&self) -> Imports {
        self.imports.clone().unwrap_or_default()
    }

    /// Check if this document has any imports
    pub fn has_imports(&self) -> bool {
        self.imports
            .as_ref()
            .map(|i| !i.is_empty())
            .unwrap_or(false)
    }
}

impl ImportContext {
    /// Create a new import context
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir: base_dir.clone(),
            loaded_files: HashSet::new(),
            repository_root: Some(base_dir),
        }
    }

    /// Create an import context with a specific repository root
    pub fn with_repository_root(base_dir: PathBuf, repository_root: PathBuf) -> Self {
        Self {
            base_dir,
            loaded_files: HashSet::new(),
            repository_root: Some(repository_root),
        }
    }

    /// Mark a file as loaded
    pub fn mark_loaded(&mut self, path: PathBuf) -> bool {
        self.loaded_files.insert(path)
    }

    /// Check if a file has been loaded (circular dependency detection)
    pub fn is_loaded(&self, path: &PathBuf) -> bool {
        self.loaded_files.contains(path)
    }

    /// Resolve a relative import path to an absolute path
    pub fn resolve_path(&self, import_path: &str) -> PathBuf {
        if let Some(repo_root) = &self.repository_root {
            // Resolve relative to repository root
            repo_root.join(import_path)
        } else {
            // Fallback to base_dir
            self.base_dir.join(import_path)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_imports_creation() {
        let imports = Imports::new()
            .add_rule("library/rules/fraud/fraud_farm.yaml".to_string())
            .add_rule("library/rules/payment/card_testing.yaml".to_string())
            .add_ruleset("library/rulesets/fraud_detection_core.yaml".to_string());

        assert_eq!(imports.rules.len(), 2);
        assert_eq!(imports.rulesets.len(), 1);
        assert!(!imports.is_empty());
    }

    #[test]
    fn test_imports_with_multiple() {
        let imports = Imports::new().with_rules(vec![
            "rule1.yaml".to_string(),
            "rule2.yaml".to_string(),
        ]);

        assert_eq!(imports.rules.len(), 2);
    }

    #[test]
    fn test_imports_is_empty() {
        let empty_imports = Imports::new();
        assert!(empty_imports.is_empty());

        let non_empty = Imports::new().add_rule("test.yaml".to_string());
        assert!(!non_empty.is_empty());
    }

    #[test]
    fn test_imports_all_paths() {
        let imports = Imports::new()
            .add_rule("rule1.yaml".to_string())
            .add_ruleset("ruleset1.yaml".to_string());

        let paths = imports.all_paths();
        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&"rule1.yaml"));
        assert!(paths.contains(&"ruleset1.yaml"));
    }

    #[test]
    fn test_rdl_document_without_imports() {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        struct TestDef {
            id: String,
        }

        let doc = RdlDocument::new("0.1".to_string(), TestDef { id: "test".to_string() });

        assert_eq!(doc.version, "0.1");
        assert!(!doc.has_imports());
        assert_eq!(doc.definition.id, "test");
    }

    #[test]
    fn test_rdl_document_with_imports() {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        struct TestDef {
            id: String,
        }

        let imports = Imports::new().add_rule("test_rule.yaml".to_string());
        let doc = RdlDocument::with_imports(
            "0.1".to_string(),
            imports.clone(),
            TestDef { id: "test".to_string() },
        );

        assert!(doc.has_imports());
        assert_eq!(doc.imports().rules.len(), 1);
    }

    #[test]
    fn test_import_context_path_resolution() {
        use std::env;

        let base_dir = env::current_dir().unwrap();
        let repo_root = base_dir.join("repository");
        let context = ImportContext::with_repository_root(base_dir.clone(), repo_root.clone());

        let resolved = context.resolve_path("library/rules/fraud_farm.yaml");
        assert_eq!(
            resolved,
            repo_root.join("library/rules/fraud_farm.yaml")
        );
    }

    #[test]
    fn test_import_context_circular_detection() {
        use std::env;

        let base_dir = env::current_dir().unwrap();
        let mut context = ImportContext::new(base_dir.clone());

        let file1 = base_dir.join("file1.yaml");
        let file2 = base_dir.join("file2.yaml");

        assert!(!context.is_loaded(&file1));
        assert!(context.mark_loaded(file1.clone()));
        assert!(context.is_loaded(&file1));

        // Try to mark the same file again (circular dependency)
        assert!(!context.mark_loaded(file1.clone()));

        // Different file should work
        assert!(context.mark_loaded(file2.clone()));
    }

    #[test]
    fn test_imports_serde() {
        let imports = Imports::new()
            .add_rule("rule1.yaml".to_string())
            .add_ruleset("ruleset1.yaml".to_string());

        // Serialize to JSON
        let json = serde_json::to_string(&imports).unwrap();
        assert!(json.contains("\"rules\""));
        assert!(json.contains("rule1.yaml"));

        // Deserialize back
        let deserialized: Imports = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, imports);
    }
}
