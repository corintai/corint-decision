//! File system based repository implementation

use async_trait::async_trait;
use corint_core::ast::{Pipeline, Rule, Ruleset};
use corint_parser::{PipelineParser, RuleParser, RulesetParser};
use path_absolutize::Absolutize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::fs;
use tokio::sync::RwLock;

use crate::{error::RepositoryError, models::*, traits::*, CacheStats, RepositoryResult};

/// File system based repository
///
/// Loads artifacts from YAML files on disk with optional caching.
pub struct FileSystemRepository {
    /// Root path of the repository
    root_path: PathBuf,
    /// Cache for rules
    rule_cache: Arc<RwLock<HashMap<String, CachedArtifact<Rule>>>>,
    /// Cache for rulesets
    ruleset_cache: Arc<RwLock<HashMap<String, CachedArtifact<Ruleset>>>>,
    /// Cache for pipelines
    pipeline_cache: Arc<RwLock<HashMap<String, CachedArtifact<Pipeline>>>>,
    /// Cache configuration
    cache_config: Arc<Mutex<CacheConfig>>,
    /// Cache statistics
    stats: Arc<Mutex<CacheStats>>,
}

impl FileSystemRepository {
    /// Create a new file system repository
    ///
    /// # Arguments
    /// * `root_path` - The root directory containing the repository
    ///
    /// # Example
    /// ```no_run
    /// use corint_repository::FileSystemRepository;
    ///
    /// let repo = FileSystemRepository::new("repository").unwrap();
    /// ```
    pub fn new<P: AsRef<Path>>(root_path: P) -> RepositoryResult<Self> {
        let path = root_path.as_ref();

        // Validate that the path exists
        if !path.exists() {
            return Err(RepositoryError::InvalidPath {
                path: path.to_path_buf(),
            });
        }

        // Get absolute path
        let abs_path = path
            .absolutize()
            .map_err(|e| RepositoryError::Other(format!("Failed to absolutize path: {}", e)))?
            .to_path_buf();

        Ok(Self {
            root_path: abs_path,
            rule_cache: Arc::new(RwLock::new(HashMap::new())),
            ruleset_cache: Arc::new(RwLock::new(HashMap::new())),
            pipeline_cache: Arc::new(RwLock::new(HashMap::new())),
            cache_config: Arc::new(Mutex::new(CacheConfig::default())),
            stats: Arc::new(Mutex::new(CacheStats::default())),
        })
    }

    /// Create a new file system repository with custom cache configuration
    pub fn with_cache_config<P: AsRef<Path>>(
        root_path: P,
        config: CacheConfig,
    ) -> RepositoryResult<Self> {
        let mut repo = Self::new(root_path)?;
        repo.cache_config = Arc::new(Mutex::new(config));
        Ok(repo)
    }

    /// Resolve a path relative to the repository root
    fn resolve_path(&self, identifier: &str) -> PathBuf {
        if identifier.ends_with(".yaml") || identifier.ends_with(".yml") {
            // It's a relative path
            self.root_path.join(identifier)
        } else {
            // It's an ID - we need to search for it
            // For now, just return an invalid path - will be handled by find_by_id
            self.root_path.join(format!("{}.yaml", identifier))
        }
    }

    /// Find an artifact by ID in standard locations
    ///
    /// Searches in:
    /// - library/rules/**/*.yaml
    /// - library/rulesets/**/*.yaml
    /// - pipelines/**/*.yaml
    async fn find_by_id(&self, id: &str, artifact_type: &str) -> RepositoryResult<PathBuf> {
        let search_dirs = match artifact_type {
            "rule" => vec!["library/rules"],
            "ruleset" => vec!["library/rulesets"],
            "pipeline" => vec!["pipelines"],
            _ => vec![],
        };

        for dir in search_dirs {
            let search_path = self.root_path.join(dir);
            if let Ok(path) = self.find_yaml_by_id(&search_path, id).await {
                return Ok(path);
            }
        }

        Err(RepositoryError::IdNotFound { id: id.to_string() })
    }

    /// Recursively search for a YAML file containing an artifact with the given ID
    async fn find_yaml_by_id(&self, dir: &Path, id: &str) -> RepositoryResult<PathBuf> {
        if !dir.exists() {
            return Err(RepositoryError::NotFound {
                path: dir.display().to_string(),
            });
        }

        let mut entries = fs::read_dir(dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.is_dir() {
                // Recursively search subdirectories
                if let Ok(found) = Box::pin(self.find_yaml_by_id(&path, id)).await {
                    return Ok(found);
                }
            } else if path.extension().and_then(|s| s.to_str()) == Some("yaml")
                || path.extension().and_then(|s| s.to_str()) == Some("yml")
            {
                // Check if this file contains the artifact with the given ID
                if let Ok(content) = fs::read_to_string(&path).await {
                    // Quick check without full parsing
                    if content.contains(&format!("id: {}", id))
                        || content.contains(&format!("id: \"{}\"", id))
                    {
                        return Ok(path);
                    }
                }
            }
        }

        Err(RepositoryError::IdNotFound { id: id.to_string() })
    }

    /// Check cache and potentially load from cache
    async fn check_cache<T: Clone>(
        &self,
        cache: &Arc<RwLock<HashMap<String, CachedArtifact<T>>>>,
        identifier: &str,
    ) -> Option<(T, String)> {
        let enabled = self.cache_config.lock().unwrap().enabled;
        if !enabled {
            return None;
        }

        let cache_read = cache.read().await;
        if let Some(cached) = cache_read.get(identifier) {
            if !cached.is_expired() {
                // Cache hit
                self.stats.lock().unwrap().hits += 1;
                return Some((cached.data.clone(), cached.content.clone()));
            }
        }

        // Cache miss
        self.stats.lock().unwrap().misses += 1;
        None
    }

    /// Store in cache
    async fn store_in_cache<T: Clone>(
        &self,
        cache: &Arc<RwLock<HashMap<String, CachedArtifact<T>>>>,
        identifier: &str,
        data: T,
        content: String,
    ) {
        let (enabled, ttl) = {
            let config = self.cache_config.lock().unwrap();
            (config.enabled, config.default_ttl)
        };

        if !enabled {
            return;
        }

        let cached = CachedArtifact::new(data, content, ttl);
        let mut cache_write = cache.write().await;
        cache_write.insert(identifier.to_string(), cached);

        // Update stats
        self.stats.lock().unwrap().size = cache_write.len();
    }
}

#[async_trait]
impl Repository for FileSystemRepository {
    async fn load_rule(&self, identifier: &str) -> RepositoryResult<(Rule, String)> {
        // Check cache first
        if let Some(cached) = self.check_cache(&self.rule_cache, identifier).await {
            return Ok(cached);
        }

        // Resolve path
        let path = self.resolve_path(identifier);

        // If path doesn't exist and identifier doesn't end with .yaml, try finding by ID
        let path = if !path.exists() && !identifier.ends_with(".yaml") {
            self.find_by_id(identifier, "rule").await?
        } else {
            path
        };

        // Load and parse
        let content = fs::read_to_string(&path)
            .await
            .map_err(|_| RepositoryError::NotFound {
                path: path.display().to_string(),
            })?;

        let doc = RuleParser::parse_with_imports(&content)?;

        // Store in cache
        self.store_in_cache(
            &self.rule_cache,
            identifier,
            doc.definition.clone(),
            content.clone(),
        )
        .await;

        Ok((doc.definition, content))
    }

    async fn load_ruleset(&self, identifier: &str) -> RepositoryResult<(Ruleset, String)> {
        // Check cache first
        if let Some(cached) = self.check_cache(&self.ruleset_cache, identifier).await {
            return Ok(cached);
        }

        // Resolve path
        let path = self.resolve_path(identifier);

        // If path doesn't exist and identifier doesn't end with .yaml, try finding by ID
        let path = if !path.exists() && !identifier.ends_with(".yaml") {
            self.find_by_id(identifier, "ruleset").await?
        } else {
            path
        };

        // Load and parse
        let content = fs::read_to_string(&path)
            .await
            .map_err(|_| RepositoryError::NotFound {
                path: path.display().to_string(),
            })?;

        let doc = RulesetParser::parse_with_imports(&content)?;

        // Store in cache
        self.store_in_cache(
            &self.ruleset_cache,
            identifier,
            doc.definition.clone(),
            content.clone(),
        )
        .await;

        Ok((doc.definition, content))
    }

    async fn load_pipeline(&self, identifier: &str) -> RepositoryResult<(Pipeline, String)> {
        // Check cache first
        if let Some(cached) = self.check_cache(&self.pipeline_cache, identifier).await {
            return Ok(cached);
        }

        // Resolve path
        let path = self.resolve_path(identifier);

        // If path doesn't exist and identifier doesn't end with .yaml, try finding by ID
        let path = if !path.exists() && !identifier.ends_with(".yaml") {
            self.find_by_id(identifier, "pipeline").await?
        } else {
            path
        };

        // Load and parse
        let content = fs::read_to_string(&path)
            .await
            .map_err(|_| RepositoryError::NotFound {
                path: path.display().to_string(),
            })?;

        let doc = PipelineParser::parse_with_imports(&content)?;

        // Store in cache
        self.store_in_cache(
            &self.pipeline_cache,
            identifier,
            doc.definition.clone(),
            content.clone(),
        )
        .await;

        Ok((doc.definition, content))
    }

    async fn exists(&self, identifier: &str) -> RepositoryResult<bool> {
        let path = self.resolve_path(identifier);
        Ok(path.exists())
    }

    async fn list_rules(&self) -> RepositoryResult<Vec<String>> {
        self.list_yaml_files("library/rules").await
    }

    async fn list_rulesets(&self) -> RepositoryResult<Vec<String>> {
        self.list_yaml_files("library/rulesets").await
    }

    async fn list_pipelines(&self) -> RepositoryResult<Vec<String>> {
        self.list_yaml_files("pipelines").await
    }

    async fn load_registry(&self) -> RepositoryResult<String> {
        let registry_path = self.root_path.join("registry.yaml");

        if !registry_path.exists() {
            return Err(RepositoryError::NotFound {
                path: "registry.yaml".to_string(),
            });
        }

        let content = fs::read_to_string(&registry_path).await?;

        Ok(content)
    }
}

impl FileSystemRepository {
    /// List all YAML files in a directory recursively
    async fn list_yaml_files(&self, relative_dir: &str) -> RepositoryResult<Vec<String>> {
        let dir_path = self.root_path.join(relative_dir);

        if !dir_path.exists() {
            return Ok(Vec::new());
        }

        let mut files = Vec::new();
        self.collect_yaml_files(&dir_path, &self.root_path, &mut files)
            .await?;

        eprintln!("[DEBUG] Found {} YAML files in {}: {:?}", files.len(), relative_dir, files);

        Ok(files)
    }

    /// Recursively collect YAML files
    #[allow(clippy::only_used_in_recursion)]
    fn collect_yaml_files<'a>(
        &'a self,
        dir: &'a Path,
        root: &'a Path,
        files: &'a mut Vec<String>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = RepositoryResult<()>> + Send + 'a>>
    {
        Box::pin(async move {
            let mut entries = fs::read_dir(dir).await?;

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();

                if path.is_dir() {
                    self.collect_yaml_files(&path, root, files).await?;
                } else if path.extension().and_then(|s| s.to_str()) == Some("yaml")
                    || path.extension().and_then(|s| s.to_str()) == Some("yml")
                {
                    // Store relative path from root
                    if let Ok(rel_path) = path.strip_prefix(root) {
                        files.push(rel_path.display().to_string());
                    }
                }
            }

            Ok(())
        })
    }
}

#[async_trait]
impl CacheableRepository for FileSystemRepository {
    fn clear_cache(&mut self) {
        // Note: This is a synchronous method but clears async locks
        // We use a blocking approach here
        let rule_cache = self.rule_cache.clone();
        let ruleset_cache = self.ruleset_cache.clone();
        let pipeline_cache = self.pipeline_cache.clone();

        // Schedule cache clearing in background task
        // In practice, users should call this from an async context
        tokio::spawn(async move {
            rule_cache.write().await.clear();
            ruleset_cache.write().await.clear();
            pipeline_cache.write().await.clear();
        });

        self.stats.lock().unwrap().size = 0;
    }

    fn clear_cache_entry(&mut self, identifier: &str) {
        let rule_cache = self.rule_cache.clone();
        let ruleset_cache = self.ruleset_cache.clone();
        let pipeline_cache = self.pipeline_cache.clone();
        let id = identifier.to_string();

        tokio::spawn(async move {
            rule_cache.write().await.remove(&id);
            ruleset_cache.write().await.remove(&id);
            pipeline_cache.write().await.remove(&id);
        });
    }

    fn cache_stats(&self) -> CacheStats {
        self.stats.lock().unwrap().clone()
    }

    fn set_cache_enabled(&mut self, enabled: bool) {
        self.cache_config.lock().unwrap().enabled = enabled;
    }

    fn is_cache_enabled(&self) -> bool {
        self.cache_config.lock().unwrap().enabled
    }
}
