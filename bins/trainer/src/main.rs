use anyhow::Result;
use ai_core::{
    cfg::{self, AppId},
    logx,
    store::{open_default, DefaultKv, KvSerde, ns},
};
use clap::Parser;
use tracing::{debug, info};

const APP: AppId = AppId { qualifier: "com", organization: "local", application: "sentinel" };

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    /// Number of steps to “train”
    #[arg(long, default_value_t = 100)]
    steps: u32,

    /// Log level override (info|debug|trace…)
    #[arg(long)]
    log: Option<String>,
}

fn open_kv() -> Result<DefaultKv> {
    let dir = cfg::config_dir(&APP)?;
    open_default(dir.join("kv"))
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut conf = cfg::load_or_init(&APP)?;
    if let Some(lv) = &cli.log {
        conf.log_level = lv.clone();
    }
    logx::init(&conf.log_level);

    let kv = open_kv()?;

    info!("trainer start steps={}", cli.steps);
    debug!("warmup=mem:init");

    if let Some(prev) = kv.get_t::<u32>(&ns("trainer", "last_steps"))? {
        debug!("previous last_steps={}", prev);
    }

    kv.put_t(&ns("trainer", "last_steps"), &cli.steps)?;
    Ok(())
}
