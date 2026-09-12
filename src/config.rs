use anyhow::{Context, Result, bail};
use std::{collections::HashSet, path::PathBuf};

#[derive(Clone, Debug)]
pub struct Config {
    pub bind: String,
    pub database: PathBuf,
    pub static_dir: PathBuf,
    pub backup_dir: PathBuf,
    pub public_url: String,
    pub github_client_id: String,
    pub github_client_secret: String,
    pub allowed_emails: HashSet<String>,
    pub legacy_token: Option<String>,
    pub retention_days: u32,
    pub queue_bytes: u32,
    pub github_authorize_url: String,
    pub github_token_url: String,
    pub github_api_url: String,
}
impl Config {
    pub fn from_env() -> Result<Self> {
        let get =
            |key: &str, fallback: &str| std::env::var(key).unwrap_or_else(|_| fallback.into());
        let public_url = get("PUBLIC_BASE_URL", "http://localhost:3000")
            .trim_end_matches('/')
            .to_owned();
        let url = url::Url::parse(&public_url).context("Invalid PUBLIC_BASE_URL")?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            bail!("PUBLIC_BASE_URL must be an http(s) origin without path or credentials");
        }
        if url.scheme() == "http"
            && !matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))
        {
            bail!("PUBLIC_BASE_URL must use HTTPS outside localhost");
        }
        let queue_bytes: u32 = get("QUEUE_BYTES", "67108864")
            .parse()
            .context("Invalid QUEUE_BYTES")?;
        if !(1024..=1024 * 1024 * 1024).contains(&queue_bytes) {
            bail!("QUEUE_BYTES must be 1 KiB to 1 GiB");
        }
        Ok(Self {
            bind: get("BIND_ADDR", "127.0.0.1:3000"),
            database: get("DATABASE_PATH", "data/traces.sqlite3").into(),
            static_dir: get("STATIC_DIR", "web/build").into(),
            backup_dir: get("BACKUP_DIR", "data/backups").into(),
            public_url,
            github_client_id: get("GITHUB_CLIENT_ID", ""),
            github_client_secret: get("GITHUB_CLIENT_SECRET", ""),
            allowed_emails: get("ALLOWED_EMAILS", "")
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect(),
            legacy_token: std::env::var("TRACE_INGEST_TOKEN")
                .ok()
                .filter(|s| !s.is_empty()),
            retention_days: get("RETENTION_DAYS", "30")
                .parse()
                .context("Invalid RETENTION_DAYS")?,
            queue_bytes,
            github_authorize_url: "https://github.com/login/oauth/authorize".into(),
            github_token_url: "https://github.com/login/oauth/access_token".into(),
            github_api_url: "https://api.github.com".into(),
        })
    }
    pub fn oauth_ready(&self) -> bool {
        !self.github_client_id.is_empty()
            && !self.github_client_secret.is_empty()
            && !self.allowed_emails.is_empty()
    }
    pub fn secure(&self) -> bool {
        self.public_url.starts_with("https://")
    }
}
