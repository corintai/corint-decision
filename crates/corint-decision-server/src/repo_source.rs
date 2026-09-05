//! Immutable published-document adapters. A backend publishes a complete document
//! in one transaction/HTTP representation, never a mutable per-file read sequence.
use corint_decision_toolchain::repository::{self, RepositoryIdentity, RepositorySnapshot};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum BackendConfig {
    Sqlite { path: PathBuf },
    Postgres { url_env: String },
    Http { url: String, token_env: String },
}
#[derive(Clone)]
pub enum Source {
    Filesystem(PathBuf),
    Sqlite(PathBuf),
    Postgres(String),
    Http { url: reqwest::Url, token: String },
}
impl Source {
    pub fn configure(
        root: &Path,
        path: &Path,
        backend: Option<BackendConfig>,
    ) -> anyhow::Result<Self> {
        Ok(match backend {
            None => {
                anyhow::ensure!(!path.as_os_str().is_empty(), "Repository path required");
                Self::Filesystem(root.join(path))
            }
            Some(backend) => {
                anyhow::ensure!(
                    path.as_os_str().is_empty(),
                    "Configure exactly one repository backend"
                );
                match backend {
                    BackendConfig::Sqlite { path } => Self::Sqlite(root.join(path)),
                    BackendConfig::Postgres { url_env } => Self::Postgres(
                        std::env::var(url_env)
                            .map_err(|_| anyhow::anyhow!("Missing repository credential"))?,
                    ),
                    BackendConfig::Http { url, token_env } => {
                        let url = reqwest::Url::parse(&url)
                            .map_err(|_| anyhow::anyhow!("Invalid repository URL"))?;
                        let local = url
                            .host_str()
                            .and_then(|v| v.parse::<std::net::IpAddr>().ok())
                            .is_some_and(|ip| ip.is_loopback());
                        anyhow::ensure!((url.scheme()=="https" || url.scheme()=="http" && local) && url.username().is_empty() && url.password().is_none() && url.query().is_none() && url.fragment().is_none(),"Repository requires HTTPS (or literal loopback HTTP), without URL credentials/query");
                        let token = std::env::var(token_env)
                            .map_err(|_| anyhow::anyhow!("Missing repository credential"))?;
                        anyhow::ensure!(
                            (32..=1024).contains(&token.len())
                                && token.bytes().all(|b| b.is_ascii_graphic()),
                            "Invalid repository credential"
                        );
                        Self::Http { url, token }
                    }
                }
            }
        })
    }
    /// Called on a blocking preparation worker; nested runtime is dropped before
    /// the independent behavior acceptance runtime is created.
    pub fn load(&self) -> anyhow::Result<RepositorySnapshot> {
        if let Self::Filesystem(path) = self {
            return Ok(repository::load(path)?);
        }
        let bytes = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(self.document())?;
        Ok(repository::load_sources(&bytes)?)
    }
    pub fn verify(&self, identity: &RepositoryIdentity) -> anyhow::Result<()> {
        if let Self::Filesystem(path) = self {
            return Ok(repository::verify_current(path, identity)?);
        }
        anyhow::ensure!(
            self.load()?.identity == *identity,
            "Repository publication changed during preparation"
        );
        Ok(())
    }
    async fn document(&self) -> anyhow::Result<Vec<u8>> {
        let bytes = match self {
            Self::Filesystem(_) => unreachable!(),
            Self::Sqlite(path) => {
                use sqlx::{sqlite::SqliteConnectOptions, Connection};
                let options = SqliteConnectOptions::new()
                    .filename(path)
                    .read_only(true)
                    .busy_timeout(std::time::Duration::from_secs(5));
                let mut conn = sqlx::SqliteConnection::connect_with(&options).await?;
                let document: String=sqlx::query_scalar("SELECT document FROM corint_core_publication WHERE slot='published' AND length(CAST(document AS BLOB))<=33554432").fetch_one(&mut conn).await?;
                document.into_bytes()
            }
            Self::Postgres(url) => {
                use sqlx::Connection;
                let mut conn = tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    sqlx::PgConnection::connect(url),
                )
                .await??;
                let document: String=tokio::time::timeout(std::time::Duration::from_secs(5),sqlx::query_scalar("SELECT document FROM corint_core_publication WHERE slot='published' AND octet_length(document)<=33554432").fetch_one(&mut conn)).await??;
                document.into_bytes()
            }
            Self::Http { url, token } => {
                let client = reqwest::Client::builder()
                    .redirect(reqwest::redirect::Policy::none())
                    .timeout(std::time::Duration::from_secs(10))
                    .build()?;
                let mut response = client.get(url.clone()).bearer_auth(token).send().await?;
                anyhow::ensure!(
                    response.status() == reqwest::StatusCode::OK,
                    "Repository response must be 200"
                );
                let mut bytes = Vec::new();
                while let Some(chunk) = response.chunk().await? {
                    anyhow::ensure!(
                        bytes.len() + chunk.len() <= 32 * 1024 * 1024,
                        "Repository document too large"
                    );
                    bytes.extend_from_slice(&chunk);
                }
                bytes
            }
        };
        anyhow::ensure!(
            bytes.len() <= 32 * 1024 * 1024,
            "Repository document too large"
        );
        Ok(bytes)
    }
}
