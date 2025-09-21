use std::net::SocketAddr;
use std::time::Instant;

use axum::{extract::State, response::IntoResponse, routing::get, Json, Router};
use serde::Serialize;
use serde_json::json;
use tokio::task::JoinHandle;
use tracing::info;

use ai_core::store::{KvSerde, ns, DefaultKv};
use crate::module::{Module, ModuleCtx};

#[derive(Clone)]
struct AppState {
    kv: DefaultKv,
    started: Instant,
}

#[derive(Serialize)]
struct Status {
    heartbeat_count: u64,
    uptime_ms: u64,
}

pub struct StatusServer {
    addr: SocketAddr,
}

impl StatusServer {
    pub fn new(addr: SocketAddr) -> Self { Self { addr } }
}

impl Module for StatusServer {
    fn name(&self) -> &'static str { "status" }

    fn spawn(self: Box<Self>, ctx: ModuleCtx) -> JoinHandle<anyhow::Result<()>> {
        tokio::spawn(async move {
            let state = AppState { kv: ctx.kv.clone(), started: Instant::now() };

            let app = Router::new()
                .route("/status", get(status))
                .with_state(state.clone());

            let listener = tokio::net::TcpListener::bind(self.addr).await?;
            info!("status server listening on http://{}", self.addr);

            // clone into a mutable receiver to await .changed()
            let mut shutdown = ctx.shutdown.clone();

            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = shutdown.changed().await;
                })
                .await?;

            Ok(())
        })
    }
}

async fn status(State(state): State<AppState>) -> impl IntoResponse {
    let count = state.kv.get_t::<u64>(&ns("heartbeat", "count")).ok().flatten().unwrap_or(0);
    let uptime_ms = state.started.elapsed().as_millis() as u64;
    Json(json!(Status { heartbeat_count: count, uptime_ms }))
}
