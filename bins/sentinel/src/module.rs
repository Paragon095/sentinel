use ai_core::store::DefaultKv;
use tokio::task::JoinHandle;

#[derive(Clone)]
pub struct ModuleCtx {
    pub kv: DefaultKv,
    pub shutdown: tokio::sync::watch::Receiver<bool>,
}

pub trait Module: Send + 'static {
    fn name(&self) -> &'static str;
    fn spawn(self: Box<Self>, ctx: ModuleCtx) -> JoinHandle<anyhow::Result<()>>;
}
