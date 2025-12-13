//! HTTP API Repository implementation
//!
//! Loads rules, rulesets, templates, and pipelines from a remote HTTP API.
//!
//! # Overview
//!
//! The API repository allows centralized management of decision artifacts across
//! multiple CORINT instances. It fetches artifacts from a remote server via HTTP/HTTPS
//! and supports optional Bearer token authentication.
//!
//! # Features
//!
//! - Remote artifact loading via HTTP API
//! - Bearer token authentication support
//! - Configurable timeout (default: 30s)
//! - Local caching of fetched artifacts
//! - Automatic manifest-based discovery
//!
//! # Usage
//!
//! ```no_run
//! use corint_repository::{ApiRepository, Repository};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Create API repository
//!     let repo = ApiRepository::new("https://api.example.com/rules", None::<String>).await?;
//!
//!     // Or with API key
//!     let repo = ApiRepository::new(
//!         "https://api.example.com/rules",
//!         Some("your-api-key")
//!     ).await?;
//!
//!     // Load a pipeline
//!     let (pipeline, content) = repo.load_pipeline("fraud_detection_pipeline").await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! # API Specification
//!
//! The remote API must implement the following endpoints:
//!
//! ## GET /manifest
//!
//! Returns a JSON manifest with available artifacts:
//!
//! ```json
//! {
//!   "registry": "https://api.example.com/rules/registry.yaml",
//!   "pipelines": [
//!     {
//!       "id": "fraud_detection_pipeline",
//!       "url": "https://api.example.com/rules/pipelines/fraud_detection.yaml"
//!     }
//!   ],
//!   "rulesets": [...],
//!   "rules": [...]
//! }
//! ```
//!
//! ## GET /pipelines/{id}.yaml
//! ## GET /rulesets/{id}.yaml
//! ## GET /rules/{id}.yaml
//!
//! Returns the YAML content for the requested artifact.
//!
//! # Authentication
//!
//! If an API key is provided, it's sent as a Bearer token:
//!
//! ```text
//! Authorization: Bearer {api_key}
//! ```
//!
//! # See Also
//!
//! - [API_REPOSITORY.md](../../docs/API_REPOSITORY.md) - Complete API documentation

use async_trait::async_trait;
use corint_core::ast::{DecisionTemplate, Pipeline, Rule, Ruleset};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;

use crate::error::{RepositoryError, RepositoryResult};
use crate::traits::Repository;

/// HTTP API Repository
///
/// Loads decision artifacts from a remote HTTP API server.
pub struct ApiRepository {
    /// HTTP client for making requests
    client: Client,

    /// Base URL of the API (e.g., "https://api.example.com/rules")
    base_url: String,

    /// Optional API key for authentication
    api_key: Option<String>,

    /// Cached manifest from the API
    manifest: ApiManifest,

    /// In-memory cache of loaded artifacts
    cache: HashMap<String, CachedArtifact>,
}

/// Cached artifact with its raw content
#[derive(Clone)]
struct CachedArtifact {
    content: String,
}

/// API manifest response
#[derive(Debug, Clone, Deserialize)]
struct ApiManifest {
    /// Optional registry URL
    #[serde(default)]
    #[allow(dead_code)]
    registry: Option<String>,

    /// List of available pipelines
    #[serde(default)]
    pipelines: Vec<ArtifactRef>,

    /// List of available rulesets
    #[serde(default)]
    rulesets: Vec<ArtifactRef>,

    /// List of available rules
    #[serde(default)]
    rules: Vec<ArtifactRef>,

    /// List of available templates
    #[serde(default)]
    templates: Vec<ArtifactRef>,
}

/// Reference to an artifact in the API
#[derive(Debug, Clone, Deserialize)]
struct ArtifactRef {
    /// Artifact ID
    id: String,

    /// Full URL to fetch the artifact
    url: String,

    /// Optional description
    #[serde(default)]
    #[allow(dead_code)]
    description: Option<String>,
}

impl ApiRepository {
    /// Create a new API repository
    ///
    /// # Arguments
    ///
    /// * `base_url` - Base URL of the API server (e.g., "https://api.example.com/rules")
    /// * `api_key` - Optional API key for Bearer token authentication
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Failed to create HTTP client
    /// - Failed to fetch manifest from API
    /// - API returned non-success status
    /// - Failed to parse manifest JSON
    pub async fn new(
        base_url: impl Into<String>,
        api_key: Option<impl Into<String>>,
    ) -> RepositoryResult<Self> {
        let base_url = base_url.into();
        let api_key = api_key.map(|k| k.into());

        // Create HTTP client with timeout
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| {
                RepositoryError::ApiError(format!("Failed to create HTTP client: {}", e))
            })?;

        // Fetch manifest
        let manifest_url = format!("{}/manifest", base_url);
        let mut request = client.get(&manifest_url);

        if let Some(ref key) = api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        let response = request
            .send()
            .await
            .map_err(|e| RepositoryError::ApiError(format!("Failed to fetch manifest: {}", e)))?;

        if !response.status().is_success() {
            return Err(RepositoryError::ApiError(format!(
                "API returned error status: {}",
                response.status()
            )));
        }

        let manifest: ApiManifest = response
            .json()
            .await
            .map_err(|e| RepositoryError::ApiError(format!("Failed to parse manifest: {}", e)))?;

        Ok(Self {
            client,
            base_url,
            api_key,
            manifest,
            cache: HashMap::new(),
        })
    }

    /// Fetch content from a URL
    async fn fetch_content(&self, url: &str) -> RepositoryResult<String> {
        let mut request = self.client.get(url);

        if let Some(ref key) = self.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        let response = request
            .send()
            .await
            .map_err(|e| RepositoryError::ApiError(format!("Failed to fetch {}: {}", url, e)))?;

        if !response.status().is_success() {
            return Err(RepositoryError::ApiError(format!(
                "API returned error status {} for {}",
                response.status(),
                url
            )));
        }

        response
            .text()
            .await
            .map_err(|e| RepositoryError::ApiError(format!("Failed to read response body: {}", e)))
    }

    /// Find artifact URL by ID
    fn find_artifact_url(&self, id: &str, artifacts: &[ArtifactRef]) -> RepositoryResult<String> {
        artifacts
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.url.clone())
            .ok_or_else(|| RepositoryError::NotFound {
                path: format!("Artifact not found: {}", id),
            })
    }
}

#[async_trait]
impl Repository for ApiRepository {
    async fn load_rule(&self, identifier: &str) -> RepositoryResult<(Rule, String)> {
        // Check cache first
        if let Some(cached) = self.cache.get(identifier) {
            let rule = serde_yaml::from_str(&cached.content)
                .map_err(|e| RepositoryError::ParseError(format!("Failed to parse rule: {}", e)))?;
            return Ok((rule, cached.content.clone()));
        }

        // Find URL in manifest
        let url = self.find_artifact_url(identifier, &self.manifest.rules)?;

        // Fetch content
        let content = self.fetch_content(&url).await?;

        // Parse rule
        let rule = serde_yaml::from_str(&content)
            .map_err(|e| RepositoryError::ParseError(format!("Failed to parse rule: {}", e)))?;

        Ok((rule, content))
    }

    async fn load_ruleset(&self, identifier: &str) -> RepositoryResult<(Ruleset, String)> {
        // Check cache first
        if let Some(cached) = self.cache.get(identifier) {
            let ruleset = serde_yaml::from_str(&cached.content).map_err(|e| {
                RepositoryError::ParseError(format!("Failed to parse ruleset: {}", e))
            })?;
            return Ok((ruleset, cached.content.clone()));
        }

        // Find URL in manifest
        let url = self.find_artifact_url(identifier, &self.manifest.rulesets)?;

        // Fetch content
        let content = self.fetch_content(&url).await?;

        // Parse ruleset
        let ruleset = serde_yaml::from_str(&content)
            .map_err(|e| RepositoryError::ParseError(format!("Failed to parse ruleset: {}", e)))?;

        Ok((ruleset, content))
    }

    async fn load_template(
        &self,
        identifier: &str,
    ) -> RepositoryResult<(DecisionTemplate, String)> {
        // Check cache first
        if let Some(cached) = self.cache.get(identifier) {
            let template = serde_yaml::from_str(&cached.content).map_err(|e| {
                RepositoryError::ParseError(format!("Failed to parse template: {}", e))
            })?;
            return Ok((template, cached.content.clone()));
        }

        // Find URL in manifest
        let url = self.find_artifact_url(identifier, &self.manifest.templates)?;

        // Fetch content
        let content = self.fetch_content(&url).await?;

        // Parse template
        let template = serde_yaml::from_str(&content)
            .map_err(|e| RepositoryError::ParseError(format!("Failed to parse template: {}", e)))?;

        Ok((template, content))
    }

    async fn load_pipeline(&self, identifier: &str) -> RepositoryResult<(Pipeline, String)> {
        // Check cache first
        if let Some(cached) = self.cache.get(identifier) {
            let pipeline = serde_yaml::from_str(&cached.content).map_err(|e| {
                RepositoryError::ParseError(format!("Failed to parse pipeline: {}", e))
            })?;
            return Ok((pipeline, cached.content.clone()));
        }

        // Find URL in manifest
        let url = self.find_artifact_url(identifier, &self.manifest.pipelines)?;

        // Fetch content
        let content = self.fetch_content(&url).await?;

        // Parse pipeline
        let pipeline = serde_yaml::from_str(&content)
            .map_err(|e| RepositoryError::ParseError(format!("Failed to parse pipeline: {}", e)))?;

        Ok((pipeline, content))
    }

    async fn exists(&self, identifier: &str) -> RepositoryResult<bool> {
        Ok(self.manifest.rules.iter().any(|r| r.id == identifier)
            || self.manifest.rulesets.iter().any(|r| r.id == identifier)
            || self.manifest.templates.iter().any(|t| t.id == identifier)
            || self.manifest.pipelines.iter().any(|p| p.id == identifier))
    }

    async fn list_rules(&self) -> RepositoryResult<Vec<String>> {
        Ok(self.manifest.rules.iter().map(|r| r.id.clone()).collect())
    }

    async fn list_rulesets(&self) -> RepositoryResult<Vec<String>> {
        Ok(self
            .manifest
            .rulesets
            .iter()
            .map(|r| r.id.clone())
            .collect())
    }

    async fn list_templates(&self) -> RepositoryResult<Vec<String>> {
        Ok(self
            .manifest
            .templates
            .iter()
            .map(|t| t.id.clone())
            .collect())
    }

    async fn list_pipelines(&self) -> RepositoryResult<Vec<String>> {
        Ok(self
            .manifest
            .pipelines
            .iter()
            .map(|p| p.id.clone())
            .collect())
    }

    async fn load_registry(&self) -> RepositoryResult<String> {
        let url = format!("{}/registry", self.base_url);

        let mut request = self.client.get(&url);
        if let Some(api_key) = &self.api_key {
            request = request.header("X-API-Key", api_key);
        }

        let response = request
            .send()
            .await
            .map_err(|e| RepositoryError::Other(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            return Err(RepositoryError::NotFound {
                path: "registry".to_string(),
            });
        }

        response
            .text()
            .await
            .map_err(|e| RepositoryError::Other(format!("Failed to read response: {}", e)))
    }
}
