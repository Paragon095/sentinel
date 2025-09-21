use std::time::Duration;

use anyhow::Result;
use ai_core::{
    cfg::{self, AppId},
    job::{Action, JobSpec},
    logx,
    store::{open_default, DefaultKv, KvSerde, ns},
};
use tokio::signal;
use tracing::{info, warn};

mod module;
mod heartbeat;
mod scheduler;
mod runner;
#[cfg(feature = "web-api")]
mod web;

use crate::heartbeat::Heartbeat;
use crate::module::{Module, ModuleCtx};
use crate::scheduler::Scheduler;

const APP: AppId = AppId { qualifier: "com", organization: "local", application: "sentinel" };

#[tokio::main]
async fn main() -> Result<()> {
    // ---- init cfg/log/kv
    let conf = cfg::load_or_init(&APP)?;
    logx::init(&conf.log_level);

    let cfgdir = cfg::config_dir(&APP)?;
    let kv: DefaultKv = open_default(cfgdir.join("kv"))?;

    info!("sentinel boot");
    info!("cfg db_path={}", conf.db_path);
    info!("init: mem:init");
    kv.put_t(&ns("app", "boot"), &"ok".to_string())?;
    info!("kv boot=ok");

    // ---- seed demo job (id=demo) if registry empty
    seed_demo_job(&kv)?;

    // ---- modules
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let mut tasks = Vec::new();

    // Heartbeat @ 1s
    let hb = Box::new(Heartbeat::new(Duration::from_millis(1000)));
    info!("module start: {}", hb.name());
    tasks.push(hb.spawn(ModuleCtx {
        kv: kv.clone(),
        shutdown: shutdown_rx.clone(),
    }));

    // Scheduler @ 250ms tick, concurrency = num_cpus
    let sch = Box::new(Scheduler::new(250, num_cpus::get()));
    info!("module start: {}", sch.name());
    tasks.push(sch.spawn(ModuleCtx {
        kv: kv.clone(),
        shutdown: shutdown_rx.clone(),
    }));

    // Optional HTTP API
    #[cfg(feature = "web-api")]
    {
        use std::net::SocketAddr;
        let http: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let srv = Box::new(web::WebServer::new(Some(http), None, None, None));
        info!("module start: {}", srv.name());
        tasks.push(srv.spawn(ModuleCtx { kv: kv.clone(), shutdown: shutdown_rx.clone() }));
    }

    info!("runtime: modules started; press Ctrl+C to stop");

    // ---- wait for ctrl+c
    signal::ctrl_c().await?;
    info!("shutdown signal received");
    let _ = shutdown_tx.send(true);

    // ---- wait for modules to end
    for t in tasks {
        match t.await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => warn!("module ended with error: {e}"),
            Err(e) => warn!("join error: {e}"),
        }
    }

    // persist heartbeat count for visibility
    if let Ok(Some(count)) = kv.get_t::<u64>(&ns("heartbeat", "count")) {
        info!("persisted heartbeat_count={}", count);
    }
    warn!("stats objects={} chains={}", 42, 1337);
    Ok(())
}

fn seed_demo_job(kv: &DefaultKv) -> Result<()> {
    let mut ids: Vec<String> = kv.get_t(&ns("jobs", "registry"))?.unwrap_or_default();
    if !ids.iter().any(|i| i == "demo") {
        ids.push("demo".to_string());
        kv.put_t(&ns("jobs", "registry"), &ids)?;
        let spec = JobSpec { period_ms: 3000, action: Action::Noop };
        kv.put_t(&ns("jobs", "demo:spec"), &spec)?;
        info!("seeded demo job: id=demo period=3000ms");
    }
    Ok(())
}
