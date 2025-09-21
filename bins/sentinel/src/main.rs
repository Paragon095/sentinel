use ai_core::{cfg::{self, AppId}, coremem, logx};
use tracing::{info, warn};

const APP: AppId = AppId {
    qualifier: "com",
    organization: "local",
    application: env!("CARGO_PKG_NAME"), // <- no literal; comes from crate name
};

fn main() {
    let cfg = cfg::load_or_init(&APP).expect("config");
    logx::init(&cfg.log_level);

    info!("{} boot", APP.application);
    info!("cfg db_path={}", cfg.db_path);
    info!("init: {}", coremem::init_mem());

    let (objs, chains) = coremem::stats();
    warn!("stats objects={objs} chains={chains}");
}
