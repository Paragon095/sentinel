use tracing_subscriber::{fmt, EnvFilter};

/// Initialize `tracing` once. Respects `RUST_LOG`; falls back to `default_level`.
pub fn init(default_level: &str) {
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", default_level);
    }
    let _ = fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(true)
        .try_init();
}
