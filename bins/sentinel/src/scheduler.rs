use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::Semaphore;
use tokio::time::interval;
use tracing::{info, warn};

use ai_core::job::{Action, JobSpec, JobState, LegacySpec};
use ai_core::store::{KvSerde, ns};
use crate::module::{Module, ModuleCtx};
use crate::runner::execute;

/// Cooperative scheduler running periodic jobs persisted in KV.
pub struct Scheduler {
    tick_ms: u64,
    max_concurrency: usize,
    max_backoff_ms: u64,
}

impl Scheduler {
    pub fn new(tick_ms: u64, max_concurrency: usize) -> Self {
        Self { tick_ms, max_concurrency, max_backoff_ms: 60_000 }
    }
}

impl Module for Scheduler {
    fn name(&self) -> &'static str { "scheduler" }

    fn spawn(self: Box<Self>, mut ctx: ModuleCtx) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        tokio::spawn(async move {
            let sem = Arc::new(Semaphore::new(self.max_concurrency));
            let mut tick = interval(Duration::from_millis(self.tick_ms));
            let backoff_cap = self.max_backoff_ms;

            loop {
                tokio::select! {
                    _ = tick.tick() => {
                        let ids: Vec<String> = ctx.kv.get_t(&ns("jobs", "registry"))?.unwrap_or_default();
                        let now = now_ms();

                        for id in ids {
                            let spec_key = ns("jobs", &format!("{id}:spec"));

                            // Try new -> legacy -> stored String(JSON) fallbacks
                            let spec = ctx.kv.get_t::<JobSpec>(&spec_key).ok().flatten()
                                .or_else(|| {
                                    if let Ok(Some(old)) = ctx.kv.get_t::<LegacySpec>(&spec_key) {
                                        let action = match old.cmd.as_str() {
                                            "noop" => Action::Noop,
                                            _ => { warn!("legacy cmd {} -> noop", old.cmd); Action::Noop }
                                        };
                                        Some(JobSpec { period_ms: old.period_ms, action })
                                    } else { None }
                                })
                                .or_else(|| {
                                    if let Ok(Some(s)) = ctx.kv.get_t::<String>(&spec_key) {
                                        serde_json::from_str::<JobSpec>(&s).ok()
                                    } else { None }
                                });

                            let Some(spec) = spec else { continue };

                            let state_key = ns("jobs", &format!("{id}:state"));
                            let state = ctx.kv.get_t::<JobState>(&state_key)?.unwrap_or_default();

                            let effective_period = if state.backoff_ms > 0 { state.backoff_ms } else { spec.period_ms };
                            let due = now.saturating_sub(state.last_run_ms) >= effective_period;
                            if !due { continue; }

                            // Concurrency gate
                            let permit = match sem.clone().try_acquire_owned() {
                                Ok(p) => p,
                                Err(_) => { continue; } // saturated
                            };

                            let kvc = ctx.kv.clone();
                            let idc = id.clone();
                            let specc = spec.clone();
                            let state_keyc = state_key.clone();

                            tokio::spawn(async move {
                                let res = execute(&specc.action, &kvc).await;

                                let mut st = kvc.get_t::<JobState>(&state_keyc).ok().flatten().unwrap_or_default();
                                st.last_run_ms = now_ms();
                                match res {
                                    Ok(_) => {
                                        st.runs = st.runs.saturating_add(1);
                                        st.failures = 0;
                                        st.backoff_ms = 0;
                                        let _ = kvc.put_t(&state_keyc, &st);
                                        info!("job ok id={}", idc);
                                    }
                                    Err(e) => {
                                        st.failures = st.failures.saturating_add(1);
                                        st.backoff_ms = (st.backoff_ms.max(specc.period_ms).saturating_mul(2)).min(backoff_cap);
                                        let _ = kvc.put_t(&state_keyc, &st);
                                        warn!("job err id={} err={}", idc, e);
                                    }
                                }
                                drop(permit);
                            });
                        }
                    }
                    changed = ctx.shutdown.changed() => {
                        if changed.is_ok() && *ctx.shutdown.borrow() {
                            info!("scheduler stopping");
                            break;
                        }
                    }
                }
            }
            Ok(())
        })
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
