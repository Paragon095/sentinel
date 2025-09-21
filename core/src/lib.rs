#![allow(clippy::multiple_crate_versions)] // transient windows-* dupes are fine

pub mod coremem {
    /// Initialize core memory.
    ///
    /// # Returns
    /// Marker string indicating initialization status.
    #[must_use]
    pub fn init_mem() -> String {
        "mem:init".to_string()
    }

    /// Returns (`active_objects`, `chains`).
    #[must_use]
    pub const fn stats() -> (usize, usize) {
        (42, 1337)
    }
}

pub mod cfg {
    use anyhow::{Context, Result};
    use directories::ProjectDirs;
    use serde::{Deserialize, Serialize};
    use std::{fs, path::PathBuf};

    /// Identifies the app for platform config dirs.
    #[derive(Debug, Clone, Copy)]
    pub struct AppId {
        pub qualifier: &'static str,     // e.g., "com"
        pub organization: &'static str,  // e.g., "local"
        pub application: &'static str,   // e.g., env!("CARGO_PKG_NAME")
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Config {
        pub log_level: String,
        pub db_path: String,
    }

    impl Default for Config {
        fn default() -> Self {
            Self { log_level: "info".into(), db_path: "mem.db".into() }
        }
    }

    /// Returns the OS-specific config directory for this app.
    ///
    /// # Errors
    /// Returns an error if the platform config directory cannot be resolved.
    pub fn config_dir(app: &AppId) -> Result<PathBuf> {
        let pd = ProjectDirs::from(app.qualifier, app.organization, app.application)
            .context("cannot resolve ProjectDirs")?;
        Ok(pd.config_dir().to_path_buf())
    }

    /// Loads `config.toml` or writes a default one if missing.
    ///
    /// # Errors
    /// Returns an error if reading, parsing, serializing, or writing the config fails.
    pub fn load_or_init(app: &AppId) -> Result<Config> {
        let dir = config_dir(app)?;
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("config.toml");
        if path.exists() {
            let s = fs::read_to_string(&path).context("read config.toml")?;
            let cfg: Config = toml::from_str(&s).context("parse config.toml")?;
            Ok(cfg)
        } else {
            let cfg = Config::default();
            let s = toml::to_string_pretty(&cfg).context("serialize default config")?;
            fs::write(&path, s).context("write default config.toml")?;
            Ok(cfg)
        }
    }
}

pub mod logx {
    use tracing_subscriber::{fmt, EnvFilter};

    /// Initialize global logging once.
    /// Respects `RUST_LOG` if set; falls back to `default_level`.
    pub fn init(default_level: &str) {
        let level = std::env::var("RUST_LOG").unwrap_or_else(|_| default_level.to_string());
        let filter = EnvFilter::try_new(level).unwrap_or_else(|_| EnvFilter::new("info"));
        let _ = fmt().with_env_filter(filter).try_init(); // idempotent
    }
}

#[cfg(test)]
mod tests {
    use super::coremem;
    #[test]
    fn stats_are_nonzero() {
        let (o, c) = coremem::stats();
        assert!(o > 0 && c > 0);
    }
}
