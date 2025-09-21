use ai_core::{coremem, logx};
use ai_core::cfg::{self, AppId};
use clap::{Parser, Subcommand};
use tracing::info;

const APP: AppId = AppId {
    qualifier: "com",
    organization: "local",
    application: env!("CARGO_PKG_NAME"),
};

#[derive(Parser)]
#[command(name=env!("CARGO_PKG_NAME"), version, about="AI memory scanner")]
struct Cli {
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command { Baseline, Probe { key: String } }

fn main() {
    let cli = Cli::parse();
    let level = match cli.verbose { 0 => "info", 1 => "debug", _ => "trace" };
    logx::init(level);

    // Use APP so config dir is created for this tool too:
    let _cfg = cfg::load_or_init(&APP).expect("cfg");

    match cli.cmd {
        Command::Baseline => {
            let (o, c) = coremem::stats();
            info!("baseline objects={o} chains={c}");
        }
        Command::Probe { key } => {
            let (o, c) = coremem::stats();
            info!("probe key={key} -> objects={o} chains={c}");
        }
    }
}
