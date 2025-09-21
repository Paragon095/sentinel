use anyhow::Result;
use ai_core::{
    cfg::{self, AppId},
    job::{Action, JobSpec, JobState},
    logx,
    // ✅ add Kv so get/put/delete methods resolve
    store::{open_default, DefaultKv, Kv, KvSerde, ns},
};
use clap::{Args, Parser, Subcommand};
use serde_json::{json, to_string_pretty};
use tracing::info;

const APP_SENTINEL: AppId = AppId { qualifier: "com", organization: "local", application: "sentinel" };

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    /// Target app namespace (default: sentinel)
    #[arg(long, default_value = "sentinel", value_parser = ["sentinel"])]
    app: String,

    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Show baseline stats
    Baseline,

    /// Get raw key from KV (prints <nil> if missing)
    Get {
        /// Key bytes (use '--' before key to stop option parsing)
        key: String,
    },

    /// Put raw bytes (UTF-8) into KV
    Put {
        key: String,
        value: String,
    },

    /// Delete key from KV
    Del {
        key: String,
    },

    /// Job registry operations
    Jobs {
        #[command(subcommand)]
        cmd: JobsCmd,
    },
}

#[derive(Subcommand, Debug)]
enum JobsCmd {
    /// List all jobs as JSON
    List,
    /// Add/replace a KV put job
    AddKvPut(AddKvPut),
    /// Add/replace an external process exec job
    AddExec(AddExec),
    /// Delete job by id
    Del {
        id: String,
    },
}

#[derive(Args, Debug)]
struct AddKvPut {
    id: String,
    #[arg(long)]
    key: String,
    #[arg(long)]
    value: String,
    /// decode: utf8 | string | u32 | u64
    #[arg(long, default_value = "utf8")]
    decode: String,
    #[arg(long, default_value_t = 1000)]
    period_ms: u64,
}

#[derive(Args, Debug)]
struct AddExec {
    id: String,
    #[arg(long)]
    cmd: String,
    /// Pass multiple: --args a b c
    #[arg(long)]
    args: Vec<String>,
    #[arg(long)]
    timeout_ms: Option<u64>,
    #[arg(long, default_value_t = 1000)]
    period_ms: u64,
}

fn app_by_name(name: &str) -> &'static AppId {
    match name {
        "sentinel" => &APP_SENTINEL,
        _ => &APP_SENTINEL,
    }
}

fn open_kv(app: &AppId) -> Result<DefaultKv> {
    let cfgdir = cfg::config_dir(app)?;
    open_default(cfgdir.join("kv"))
}

fn list_jobs(kv: &impl KvSerde) -> Result<Vec<serde_json::Value>> {
    let ids: Vec<String> = kv.get_t(&ns("jobs", "registry"))?.unwrap_or_default();
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        let spec: Option<JobSpec> = kv.get_t(&ns("jobs", &format!("{}:spec", id)))?;
        let state: JobState = kv.get_t(&ns("jobs", &format!("{}:state", id)))?.unwrap_or_default();
        out.push(json!({ "id": id, "spec": spec, "state": state }));
    }
    Ok(out)
}

fn upsert_job(kv: &impl KvSerde, id: &str, spec: JobSpec) -> Result<()> {
    let reg_key = ns("jobs", "registry");
    let mut ids: Vec<String> = kv.get_t(&reg_key)?.unwrap_or_default();
    if !ids.iter().any(|s| s == id) {
        ids.push(id.to_string());
        kv.put_t(&reg_key, &ids)?;
    }
    kv.put_t(&ns("jobs", &format!("{}:spec", id)), &spec)?;
    // reset state to start fresh
    kv.put_t(&ns("jobs", &format!("{}:state", id)), &JobState::default())?;
    Ok(())
}

fn del_job(kv: &impl KvSerde, id: &str) -> Result<()> {
    // remove spec/state
    let _ = kv.delete(&ns("jobs", &format!("{}:spec", id)));
    let _ = kv.delete(&ns("jobs", &format!("{}:state", id)));
    // prune from registry (typed read, then write if present)
    let reg_key = ns("jobs", "registry");
    if let Some(mut list) = kv.get_t::<Vec<String>>(&reg_key)? {
        list.retain(|s| s != id);
        kv.put_t(&reg_key, &list)?;
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    logx::init("info");

    let app = app_by_name(&cli.app);
    let kv = open_kv(app)?;

    match cli.cmd {
        Command::Baseline => {
            info!("baseline objects=42 chains=1337");
        }

        Command::Get { key } => {
            match kv.get(key.as_bytes()) {
                Some(v) => info!("get {} => {}", key, String::from_utf8_lossy(&v)),
                None => info!("get {} => <nil>", key),
            }
        }

        Command::Put { key, value } => {
            kv.put(key.as_bytes(), value.as_bytes());
            info!("put {} <= {}", key, value);
        }

        Command::Del { key } => {
            let existed = kv.delete(key.as_bytes());
            info!("del {} => {}", key, existed);
        }

        Command::Jobs { cmd: JobsCmd::List } => {
            let jobs = list_jobs(&kv)?;
            println!("{}", to_string_pretty(&jobs)?);
        }

        Command::Jobs { cmd: JobsCmd::AddKvPut(args) } => {
            let spec = JobSpec {
                period_ms: args.period_ms,
                action: Action::KvPut {
                    key: args.key,
                    decode: args.decode,
                    value: serde_json::Value::String(args.value),
                },
            };
            upsert_job(&kv, &args.id, spec)?;
            println!("ok");
        }

        Command::Jobs { cmd: JobsCmd::AddExec(args) } => {
            let spec = JobSpec {
                period_ms: args.period_ms,
                action: Action::Exec {
                    cmd: args.cmd,
                    args: args.args,
                    timeout_ms: args.timeout_ms,
                },
            };
            upsert_job(&kv, &args.id, spec)?;
            println!("ok");
        }

        Command::Jobs { cmd: JobsCmd::Del { id } } => {
            del_job(&kv, &id)?;
            println!("ok");
        }
    }

    Ok(())
}
