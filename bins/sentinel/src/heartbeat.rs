use ai_core::store::{KvSerde, ns};
use crate::module::{Module, ModuleCtx};
use tokio::time::{interval, Duration};
use tracing::info;

pub struct Heartbeat { period: Duration }
impl Heartbeat { pub fn new(period: Duration) -> Self { Self { period } } }

impl Module for Heartbeat {
    fn name(&self) -> &'static str { "heartbeat" }
    fn spawn(self: Box<Self>, mut ctx: ModuleCtx) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        tokio::spawn(async move {
            let mut count: u64 = ctx.kv.get_t(&ns("heartbeat", "count"))?.unwrap_or(0);
            let mut tick = interval(self.period);
            loop {
                tokio::select! {
                    _ = tick.tick() => {
                        count += 1;
                        ctx.kv.put_t(&ns("heartbeat", "count"), &count)?;
                        info!("heartbeat tick {}", count);
                    }
                    changed = ctx.shutdown.changed() => {
                        if changed.is_ok() && *ctx.shutdown.borrow() {
                            ctx.kv.put_t(&ns("heartbeat", "count"), &count)?;
                            info!("heartbeat stopping at {}", count);
                            break;
                        }
                    }
                }
            }
            Ok(())
        })
    }
}
