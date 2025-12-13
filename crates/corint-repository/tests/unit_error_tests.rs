//! Unit tests for RepositoryError types and error handling

use corint_repository::RepositoryError;
use std::path::PathBuf;

#[test]
fn test_error_not_found_display() {
    let error = RepositoryError::NotFound {
        path: "test/file.yaml".to_string(),
    };

    assert_eq!(error.to_string(), "Artifact not found: test/file.yaml");
}

#[test]
fn test_error_invalid_path_display() {
    let error = RepositoryError::InvalidPath {
        path: PathBuf::from("/invalid/path"),
    };

    assert_eq!(error.to_string(), "Invalid path: /invalid/path");
}

#[test]
fn test_error_id_not_found_display() {
    let error = RepositoryError::IdNotFound {
        id: "my_rule".to_string(),
    };

    assert_eq!(error.to_string(), "Artifact ID not found: my_rule");
}

#[test]
fn test_error_parser_display() {
    use corint_parser::error::ParseError;
    let parse_err = ParseError::MissingField {
        field: "version".to_string(),
    };
    let error = RepositoryError::Parser(parse_err);

    assert!(error.to_string().contains("Parser error"));
    assert!(error.to_string().contains("version"));
}

#[test]
fn test_error_api_error_display() {
    let error = RepositoryError::ApiError("HTTP 404".to_string());

    assert_eq!(error.to_string(), "API error: HTTP 404");
}

#[test]
fn test_error_cache_display() {
    let error = RepositoryError::Cache("eviction failed".to_string());

    assert_eq!(error.to_string(), "Cache error: eviction failed");
}

#[test]
fn test_error_parse_error_display() {
    let error = RepositoryError::ParseError("malformed YAML".to_string());

    assert_eq!(error.to_string(), "Parse error: malformed YAML");
}

#[test]
fn test_error_other_display() {
    let error = RepositoryError::Other("unknown error".to_string());

    assert_eq!(error.to_string(), "Repository error: unknown error");
}

#[test]
fn test_error_io_conversion() {
    let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let repo_error: RepositoryError = io_error.into();

    assert!(repo_error.to_string().contains("I/O error"));
}

#[test]
fn test_error_yaml_parse_conversion() {
    let yaml_error = serde_yaml::from_str::<serde_yaml::Value>("invalid: yaml: :::").unwrap_err();
    let repo_error: RepositoryError = yaml_error.into();

    assert!(repo_error.to_string().contains("Failed to parse YAML"));
}

#[test]
fn test_error_parser_conversion() {
    use corint_parser::ParseError;

    // Create a real YAML error
    let yaml_err = serde_yaml::from_str::<serde_yaml::Value>("invalid: yaml: :::").unwrap_err();
    let parse_error = ParseError::YamlError(yaml_err);
    let repo_error: RepositoryError = parse_error.into();

    assert!(repo_error.to_string().contains("Parser error"));
}

// Integration tests for error handling in FileSystemRepository

#[tokio::test]
async fn test_file_system_repo_invalid_path() {
    use corint_repository::FileSystemRepository;

    let result = FileSystemRepository::new("/path/that/does/not/exist");

    assert!(result.is_err());
    match result {
        Err(RepositoryError::InvalidPath { path }) => {
            assert_eq!(path, PathBuf::from("/path/that/does/not/exist"));
        }
        _ => panic!("Expected InvalidPath error"),
    }
}

#[tokio::test]
async fn test_file_system_repo_file_not_found() {
    use corint_repository::{FileSystemRepository, Repository};
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let repo = FileSystemRepository::new(temp_dir.path()).unwrap();

    let result = repo.load_rule("nonexistent_rule.yaml").await;

    assert!(result.is_err());
    match result {
        Err(RepositoryError::NotFound { .. }) => (),
        _ => panic!("Expected NotFound error"),
    }
}

#[tokio::test]
async fn test_file_system_repo_id_not_found() {
    use corint_repository::{FileSystemRepository, Repository};
    use tempfile::TempDir;
    use tokio::fs;

    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Create library/rules directory but no file
    fs::create_dir_all(repo_path.join("library/rules"))
        .await
        .unwrap();

    let repo = FileSystemRepository::new(repo_path).unwrap();

    let result = repo.load_rule("nonexistent_id").await;

    assert!(result.is_err());
    match result {
        Err(RepositoryError::IdNotFound { id }) => {
            assert_eq!(id, "nonexistent_id");
        }
        _ => panic!("Expected IdNotFound error, got: {:?}", result),
    }
}

#[tokio::test]
async fn test_file_system_repo_parse_error() {
    use corint_repository::{FileSystemRepository, Repository};
    use tempfile::TempDir;
    use tokio::fs;

    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Create directory and invalid YAML file
    fs::create_dir_all(repo_path.join("library/rules"))
        .await
        .unwrap();

    let invalid_yaml = "invalid: yaml: syntax: :::";
    fs::write(repo_path.join("library/rules/invalid.yaml"), invalid_yaml)
        .await
        .unwrap();

    let repo = FileSystemRepository::new(repo_path).unwrap();

    let result = repo.load_rule("library/rules/invalid.yaml").await;

    assert!(result.is_err());
    // Should get a Parser or YamlParse error
    assert!(matches!(
        result,
        Err(RepositoryError::Parser(_)) | Err(RepositoryError::YamlParse(_))
    ));
}

#[tokio::test]
async fn test_file_system_repo_exists_error_handling() {
    use corint_repository::{FileSystemRepository, Repository};
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let repo = FileSystemRepository::new(temp_dir.path()).unwrap();

    // exists() should not error for non-existent files, just return false
    let result = repo.exists("nonexistent.yaml").await;

    assert!(result.is_ok());
    assert!(!result.unwrap());
}

#[tokio::test]
async fn test_file_system_repo_registry_not_found() {
    use corint_repository::{FileSystemRepository, Repository};
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let repo = FileSystemRepository::new(temp_dir.path()).unwrap();

    let result = repo.load_registry().await;

    assert!(result.is_err());
    match result {
        Err(RepositoryError::NotFound { path }) => {
            assert_eq!(path, "registry.yaml");
        }
        _ => panic!("Expected NotFound error for registry"),
    }
}

#[tokio::test]
async fn test_error_is_send_sync() {
    // Verify that RepositoryError implements Send + Sync
    // This is important for async usage
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<RepositoryError>();
}

#[test]
fn test_error_debug_format() {
    let error = RepositoryError::NotFound {
        path: "test.yaml".to_string(),
    };

    let debug_str = format!("{:?}", error);
    assert!(debug_str.contains("NotFound"));
    assert!(debug_str.contains("test.yaml"));
}
