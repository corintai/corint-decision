//! Unit tests for ServerConfig and RepositoryType

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// Re-create config types for testing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RepositoryType {
    FileSystem {
        #[serde(default = "default_repository_path")]
        path: PathBuf,
    },
    Database {
        db_type: DatabaseType,
        url: String,
    },
    Api {
        base_url: String,
        #[serde(default)]
        api_key: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseType {
    PostgreSQL,
    MySQL,
    SQLite,
}

fn default_repository_path() -> PathBuf {
    PathBuf::from("repository")
}

impl Default for RepositoryType {
    fn default() -> Self {
        RepositoryType::FileSystem {
            path: default_repository_path(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub repository: RepositoryType,
    pub enable_metrics: bool,
    pub enable_tracing: bool,
    pub log_level: String,
    #[serde(default)]
    pub database_url: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            repository: RepositoryType::default(),
            enable_metrics: true,
            enable_tracing: true,
            log_level: "info".to_string(),
            database_url: None,
        }
    }
}

// Tests

#[test]
fn test_server_config_default() {
    let config = ServerConfig::default();

    assert_eq!(config.host, "127.0.0.1");
    assert_eq!(config.port, 8080);
    assert!(config.enable_metrics);
    assert!(config.enable_tracing);
    assert_eq!(config.log_level, "info");
    assert!(config.database_url.is_none());
}

#[test]
fn test_repository_type_default() {
    let repo = RepositoryType::default();

    match repo {
        RepositoryType::FileSystem { path } => {
            assert_eq!(path, PathBuf::from("repository"));
        }
        _ => panic!("Expected FileSystem repository"),
    }
}

#[test]
fn test_repository_type_filesystem_serialization() {
    let repo = RepositoryType::FileSystem {
        path: PathBuf::from("/custom/path"),
    };

    let json = serde_json::to_string(&repo).unwrap();
    assert!(json.contains("filesystem"));
    assert!(json.contains("/custom/path"));
}

#[test]
fn test_repository_type_filesystem_deserialization() {
    let json = r#"{"type":"filesystem","path":"/custom/path"}"#;
    let repo: RepositoryType = serde_json::from_str(json).unwrap();

    match repo {
        RepositoryType::FileSystem { path } => {
            assert_eq!(path, PathBuf::from("/custom/path"));
        }
        _ => panic!("Expected FileSystem repository"),
    }
}

#[test]
fn test_repository_type_database_postgresql() {
    let repo = RepositoryType::Database {
        db_type: DatabaseType::PostgreSQL,
        url: "postgresql://localhost/test".to_string(),
    };

    let json = serde_json::to_string(&repo).unwrap();
    assert!(json.contains("database"));
    assert!(json.contains("postgresql"));
}

#[test]
fn test_repository_type_database_deserialization() {
    let json = r#"{"type":"database","db_type":"postgresql","url":"postgresql://localhost/test"}"#;
    let repo: RepositoryType = serde_json::from_str(json).unwrap();

    match repo {
        RepositoryType::Database { db_type, url } => {
            assert!(matches!(db_type, DatabaseType::PostgreSQL));
            assert_eq!(url, "postgresql://localhost/test");
        }
        _ => panic!("Expected Database repository"),
    }
}

#[test]
fn test_repository_type_api_without_key() {
    let repo = RepositoryType::Api {
        base_url: "https://api.example.com".to_string(),
        api_key: None,
    };

    let json = serde_json::to_string(&repo).unwrap();
    assert!(json.contains("api"));
    assert!(json.contains("https://api.example.com"));
}

#[test]
fn test_repository_type_api_with_key() {
    let repo = RepositoryType::Api {
        base_url: "https://api.example.com".to_string(),
        api_key: Some("secret_key".to_string()),
    };

    let json = serde_json::to_string(&repo).unwrap();
    assert!(json.contains("api"));
    assert!(json.contains("secret_key"));
}

#[test]
fn test_repository_type_api_deserialization() {
    let json = r#"{"type":"api","base_url":"https://api.example.com","api_key":"secret"}"#;
    let repo: RepositoryType = serde_json::from_str(json).unwrap();

    match repo {
        RepositoryType::Api { base_url, api_key } => {
            assert_eq!(base_url, "https://api.example.com");
            assert_eq!(api_key, Some("secret".to_string()));
        }
        _ => panic!("Expected API repository"),
    }
}

#[test]
fn test_server_config_serialization() {
    let config = ServerConfig {
        host: "0.0.0.0".to_string(),
        port: 3000,
        repository: RepositoryType::default(),
        enable_metrics: false,
        enable_tracing: true,
        log_level: "debug".to_string(),
        database_url: Some("postgresql://localhost/db".to_string()),
    };

    let json = serde_json::to_string(&config).unwrap();
    assert!(json.contains("0.0.0.0"));
    assert!(json.contains("3000"));
    assert!(json.contains("debug"));
}

#[test]
fn test_server_config_deserialization() {
    let json = r#"{
        "host": "192.168.1.1",
        "port": 9090,
        "repository": {"type": "filesystem", "path": "/data"},
        "enable_metrics": true,
        "enable_tracing": false,
        "log_level": "warn",
        "database_url": null
    }"#;

    let config: ServerConfig = serde_json::from_str(json).unwrap();

    assert_eq!(config.host, "192.168.1.1");
    assert_eq!(config.port, 9090);
    assert!(config.enable_metrics);
    assert!(!config.enable_tracing);
    assert_eq!(config.log_level, "warn");
    assert!(config.database_url.is_none());
}

#[test]
fn test_server_config_partial_deserialization_with_defaults() {
    let json = r#"{
        "host": "localhost",
        "port": 8000,
        "enable_metrics": true,
        "enable_tracing": true,
        "log_level": "info"
    }"#;

    let config: ServerConfig = serde_json::from_str(json).unwrap();

    assert_eq!(config.host, "localhost");
    assert_eq!(config.port, 8000);
    // repository should use default
    match config.repository {
        RepositoryType::FileSystem { path } => {
            assert_eq!(path, PathBuf::from("repository"));
        }
        _ => panic!("Expected default FileSystem repository"),
    }
}

#[test]
fn test_database_type_variants() {
    let pg = DatabaseType::PostgreSQL;
    let mysql = DatabaseType::MySQL;
    let sqlite = DatabaseType::SQLite;

    assert_eq!(format!("{:?}", pg), "PostgreSQL");
    assert_eq!(format!("{:?}", mysql), "MySQL");
    assert_eq!(format!("{:?}", sqlite), "SQLite");
}

#[test]
fn test_config_with_all_repository_types() {
    // Test with FileSystem
    let config1 = ServerConfig {
        repository: RepositoryType::FileSystem {
            path: PathBuf::from("/data"),
        },
        ..Default::default()
    };
    assert!(matches!(
        config1.repository,
        RepositoryType::FileSystem { .. }
    ));

    // Test with Database
    let config2 = ServerConfig {
        repository: RepositoryType::Database {
            db_type: DatabaseType::PostgreSQL,
            url: "postgresql://localhost/test".to_string(),
        },
        ..Default::default()
    };
    assert!(matches!(
        config2.repository,
        RepositoryType::Database { .. }
    ));

    // Test with API
    let config3 = ServerConfig {
        repository: RepositoryType::Api {
            base_url: "https://api.example.com".to_string(),
            api_key: None,
        },
        ..Default::default()
    };
    assert!(matches!(config3.repository, RepositoryType::Api { .. }));
}

#[test]
fn test_config_is_clone() {
    let config = ServerConfig::default();
    let cloned = config.clone();

    assert_eq!(config.host, cloned.host);
    assert_eq!(config.port, cloned.port);
}

#[test]
fn test_repository_type_is_clone() {
    let repo = RepositoryType::default();
    let cloned = repo.clone();

    match (repo, cloned) {
        (RepositoryType::FileSystem { path: p1 }, RepositoryType::FileSystem { path: p2 }) => {
            assert_eq!(p1, p2);
        }
        _ => panic!("Clone should maintain the same variant"),
    }
}
