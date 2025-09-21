#![cfg(feature = "web-api")]

use std::net::SocketAddr;
use std::time::Instant;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::task::JoinHandle;
use tracing::info;

use ai_core::job::{Action, JobSpec, JobState, LegacySpec};
use ai_core::store::{Kv, KvSerde, DefaultKv, ns};
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

#[derive(Deserialize)]
struct KvGetQuery { decode: Option<String> }

#[derive(Deserialize)]
struct KvPutBody {
    decode: String,                 // "raw"|"utf8"|"string"|"u32"|"u64"
    value: serde_json::Value,
}

#[derive(Serialize)]
struct JobView { id: String, spec: JobSpec, state: JobState }

#[derive(Deserialize)]
struct JobUpsertLegacy { id: String, cmd: String, period_ms: u64 }

#[derive(Deserialize)]
struct JobUpsertNew { id: String, spec: JobSpec }

#[derive(Deserialize)]
#[serde(untagged)]
enum JobUpsertEither { Legacy(JobUpsertLegacy), New(JobUpsertNew) }

pub struct WebServer {
    pub http_addr: Option<SocketAddr>,
    pub https_addr: Option<SocketAddr>,
    pub tls_cert_pem: Option<String>,
    pub tls_key_pem: Option<String>,
}

impl WebServer {
    pub fn new(http: Option<SocketAddr>, https: Option<SocketAddr>, cert: Option<String>, key: Option<String>) -> Self {
        Self { http_addr: http, https_addr: https, tls_cert_pem: cert, tls_key_pem: key }
    }
}

impl Module for WebServer {
    fn name(&self) -> &'static str { "web" }

    fn spawn(self: Box<Self>, ctx: ModuleCtx) -> JoinHandle<anyhow::Result<()>> {
        tokio::spawn(async move {
            let state = AppState { kv: ctx.kv.clone(), started: Instant::now() };
            let app = Router::new()
                .route("/status", get(status))
                .route("/kv/:key", get(kv_get).put(kv_put).delete(kv_del))
                .route("/jobs", get(jobs_list).post(jobs_upsert))
                .route("/jobs/:id", delete(jobs_delete))
                .with_state(state);

            let mut servers = Vec::<tokio::task::JoinHandle<anyhow::Result<()>>>::new();

            if let Some(addr) = self.http_addr {
                info!("web http listening on http://{}", addr);
                let app_clone = app.clone();
                let mut sd = ctx.shutdown.clone();
                servers.push(tokio::spawn(async move {
                    let listener = tokio::net::TcpListener::bind(addr).await?;
                    let serve_fut = axum::serve(listener, app_clone);
                    tokio::select! {
                        r = serve_fut => { r?; }
                        _ = sd.changed() => { /* shutdown */ }
                    }
                    Ok(())
                }));
            }

            if let (Some(addr), Some(cert), Some(key)) = (self.https_addr, self.tls_cert_pem.clone(), self.tls_key_pem.clone()) {
                info!("web https listening on https://{}", addr);
                let app_clone = app.clone();
                let mut sd = ctx.shutdown.clone();
                servers.push(tokio::spawn(async move {
                    let config = axum_server::tls_rustls::RustlsConfig::from_pem_file(cert, key).await?;
                    let serve_fut = axum_server::bind_rustls(addr, config).serve(app_clone.into_make_service());
                    tokio::select! {
                        r = serve_fut => { r?; }
                        _ = sd.changed() => { /* shutdown */ }
                    }
                    Ok(())
                }));
            }

            let _ = ctx.shutdown.clone().changed().await;
            for s in servers { let _ = s.await??; }
            Ok(())
        })
    }
}

async fn status(State(state): State<AppState>) -> impl IntoResponse {
    let count = state.kv.get_t::<u64>(&ns("heartbeat", "count")).ok().flatten().unwrap_or(0);
    let uptime_ms = state.started.elapsed().as_millis() as u64;
    Json(json!(Status { heartbeat_count: count, uptime_ms }))
}

async fn kv_get(Path(key): Path<String>, State(state): State<AppState>, Query(q): Query<KvGetQuery>) -> impl IntoResponse {
    let kb = key.as_bytes();
    match q.decode.as_deref().unwrap_or("raw") {
        "raw" => match state.kv.get(kb) {
            Some(v) => Json(json!({"key": key, "value": String::from_utf8_lossy(&v)})).into_response(),
            None => (StatusCode::NOT_FOUND, "nil").into_response(),
        },
        "utf8" => match state.kv.get(kb) {
            Some(v) => match String::from_utf8(v) {
                Ok(s) => Json(json!({"key": key, "value": s})).into_response(),
                Err(_) => (StatusCode::BAD_REQUEST, "non-utf8").into_response(),
            },
            None => (StatusCode::NOT_FOUND, "nil").into_response(),
        },
        "string" => match state.kv.get_t::<String>(kb) {
            Ok(Some(s)) => Json(json!({"key": key, "value": s})).into_response(),
            Ok(None) => (StatusCode::NOT_FOUND, "nil").into_response(),
            Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
        },
        "u32" => match state.kv.get_t::<u32>(kb) {
            Ok(Some(n)) => Json(json!({"key": key, "value": n})).into_response(),
            Ok(None) => (StatusCode::NOT_FOUND, "nil").into_response(),
            Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
        },
        "u64" => match state.kv.get_t::<u64>(kb) {
            Ok(Some(n)) => Json(json!({"key": key, "value": n})).into_response(),
            Ok(None) => (StatusCode::NOT_FOUND, "nil").into_response(),
            Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
        },
        _ => (StatusCode::BAD_REQUEST, "unknown decode").into_response(),
    }
}

async fn kv_put(Path(key): Path<String>, State(state): State<AppState>, Json(body): Json<KvPutBody>) -> impl IntoResponse {
    let kb = key.as_bytes();
    let result = match body.decode.as_str() {
        "raw" | "utf8" => {
            if let Some(s) = body.value.as_str() {
                state.kv.put(kb, s.as_bytes());
                Ok(())
            } else { Err("value must be string".to_string()) }
        }
        "string" => {
            if let Some(s) = body.value.as_str() {
                state.kv.put_t(kb, &s.to_string()).map_err(|e| e.to_string())
            } else { Err("value must be string".to_string()) }
        }
        "u32" => {
            if let Some(n) = body.value.as_u64() {
                match u32::try_from(n) {
                    Ok(n32) => state.kv.put_t(kb, &n32).map_err(|e| e.to_string()),
                    Err(_) => Err("out of range".to_string()),
                }
            } else { Err("value must be number".to_string()) }
        }
        "u64" => {
            if let Some(n) = body.value.as_u64() {
                state.kv.put_t(kb, &n).map_err(|e| e.to_string())
            } else { Err("value must be number".to_string()) }
        }
        _ => Err("unknown decode".to_string()),
    };
    match result {
        Ok(()) => (StatusCode::CREATED, "ok").into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e).into_response(),
    }
}

async fn kv_del(Path(key): Path<String>, State(state): State<AppState>) -> impl IntoResponse {
    let existed = state.kv.delete(key.as_bytes());
    Json(json!({ "deleted": existed }))
}

async fn jobs_list(State(state): State<AppState>) -> impl IntoResponse {
    let ids: Vec<String> = state.kv.get_t(&ns("jobs", "registry")).ok().flatten().unwrap_or_default();
    let mut out = Vec::new();
    for id in ids {
        let k = ns("jobs", &format!("{id}:spec"));
        let spec_opt = state.kv.get_t::<JobSpec>(&k).ok().flatten()
            .or_else(|| {
                if let Ok(Some(old)) = state.kv.get_t::<LegacySpec>(&k) {
                    let action = match old.cmd.as_str() { "noop" => Action::Noop, _ => Action::Noop };
                    Some(JobSpec { period_ms: old.period_ms, action })
                } else { None }
            })
            .or_else(|| {
                if let Ok(Some(s)) = state.kv.get_t::<String>(&k) {
                    serde_json::from_str::<JobSpec>(&s).ok()
                } else { None }
            });
        if let Some(spec) = spec_opt {
            let state_j = state.kv.get_t::<JobState>(&ns("jobs", &format!("{id}:state"))).ok().flatten().unwrap_or_default();
            out.push(JobView { id, spec, state: state_j });
        }
    }
    Json(out)
}

async fn jobs_upsert(State(state): State<AppState>, Json(payload): Json<JobUpsertEither>) -> impl IntoResponse {
    let (id, legacy, newspec) = match payload {
        JobUpsertEither::Legacy(j) => (j.id, Some(LegacySpec { cmd: j.cmd, period_ms: j.period_ms }), None),
        JobUpsertEither::New(j)    => (j.id, None, Some(j.spec)),
    };

    // update registry
    let mut ids: Vec<String> = state.kv.get_t(&ns("jobs", "registry")).ok().flatten().unwrap_or_default();
    if !ids.iter().any(|i| i == &id) { ids.push(id.clone()); }
    let _ = state.kv.put_t(&ns("jobs", "registry"), &ids);

    // store spec
    if let Some(l) = legacy {
        let _ = state.kv.put_t(&ns("jobs", &format!("{}:spec", id)), &l);
    }
    if let Some(s) = newspec {
        let _ = state.kv.put_t(&ns("jobs", &format!("{}:spec", id)), &s);
    }

    Json(json!({"ok": true}))
}

async fn jobs_delete(Path(id): Path<String>, State(state): State<AppState>) -> impl IntoResponse {
    let mut ids: Vec<String> = state.kv.get_t(&ns("jobs", "registry")).ok().flatten().unwrap_or_default();
    ids.retain(|i| i != &id);
    let _ = state.kv.put_t(&ns("jobs", "registry"), &ids);
    let _ = state.kv.delete(&ns("jobs", &format!("{id}:spec")));
    let _ = state.kv.delete(&ns("jobs", &format!("{id}:state")));
    Json(json!({"ok": true}))
}
