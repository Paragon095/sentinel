use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Identifier used to compute per-app configuration directories.
#[derive(Clone, Copy)]
pub struct AppId {
    /// Reverse-DNS style qualifier, e.g. `"com"`.
    pub qualifier: &'static str,
    /// Organization or vendor name, e.g. `"local"`.
    pub organization: &'static str,
    /// Application name, e.g. `"sentinel"`.
    pub application: &'static str,
}

/// Application configuration persisted to `config.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Tracing level to use if `RUST_LOG` is not set (e.g. `"info"`).
    pub log_level: String,
    /// Optional DB path (legacy compat; not used by FS-KV).
    #[serde(default = "default_db_path")]
    pub db_path: String,
}

fn default_db_path() -> String { "mem.db".to_string() }

/// Return the configuration directory for this app, creating it if needed.
pub fn config_dir(app: &AppId) -> Result<PathBuf> {
    let pd = ProjectDirs::from(app.qualifier, app.organization, app.application)
        .ok_or_else(|| anyhow::anyhow!("failed to resolve ProjectDirs"))?;
    let dir = pd.config_dir().to_path_buf();
    fs::create_dir_all(&dir).with_context(|| format!("create config dir {}", dir.display()))?;
    Ok(dir)
}

/// Load `config.toml` from the app config dir or create a default one.
pub fn load_or_init(app: &AppId) -> Result<Config> {
    let dir = config_dir(app)?;
    let path = dir.join("config.toml");
    if path.exists() {
        let txt = fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
        let cfg: Config = toml::from_str(&txt)
            .with_context(|| format!("parse {}", path.display()))?;
        Ok(cfg)
    } else {
        let cfg = Config { log_level: "info".to_string(), db_path: default_db_path() };
        save_config(&path, &cfg)?;
        Ok(cfg)
    }
}

fn save_config(path: &Path, cfg: &Config) -> Result<()> {
    let s = toml::to_string_pretty(cfg)?;
    fs::write(path, s).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}
