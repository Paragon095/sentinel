use ai_core::{cfg::{self, AppId}, coremem, logx};
use clap::Parser;
use tracing::{debug, info};

const APP: AppId = AppId {
    qualifier: "com",
    organization: "local",
    application: env!("CARGO_PKG_NAME"), // "trainer"
};

#[derive(Parser)]
#[command(name=env!("CARGO_PKG_NAME"), version, about="AI trainer")]
struct Cli {
    /// Steps to run
    #[arg(long, default_value_t = 100)]
    steps: u32,
    /// Log level override (info,debug,trace)
    #[arg(long)]
    log: Option<String>,
}

fn main() {
    let cli = Cli::parse();

    // Config path derives from this crate's package name (no literal).
    let c = cfg::load_or_init(&APP).expect("cfg");
    let level = cli.log.as_deref().unwrap_or(&c.log_level);
    logx::init(level);

    info!("{} start steps={}", APP.application, cli.steps);
    let warm = coremem::init_mem();
    debug!("warmup={warm}");
}
