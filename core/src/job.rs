use serde::{Deserialize, Serialize};

/// A scheduled job specification.
#[derive(Serialize, Deserialize, Clone)]
pub struct JobSpec {
    /// Period between runs (milliseconds).
    pub period_ms: u64,
    /// The action to perform when the job triggers.
    pub action: Action,
}

/// Actions that a job can perform.
#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Do nothing.
    Noop,
    /// Execute an OS command.
    Exec {
        /// Command/executable.
        cmd: String,
        /// Arguments.
        #[serde(default)]
        args: Vec<String>,
        /// Optional timeout (ms).
        #[serde(default)]
        timeout_ms: Option<u64>,
    },
    /// Simple HTTP call (optional in runtime; kept for completeness).
    Http {
        /// URL.
        url: String,
        /// Method (GET by default).
        #[serde(default)]
        method: Option<String>,
        /// Body (optional).
        #[serde(default)]
        body: Option<String>,
        /// Optional timeout (ms).
        #[serde(default)]
        timeout_ms: Option<u64>,
    },
    /// Write a value into KV.
    KvPut {
        /// Key to write.
        key: String,
        /// Decode mode for value: "utf8" | "string" | "u32" | "u64".
        decode: String,
        /// Value (typed by `decode`).
        value: serde_json::Value,
    },
    /// Delete a key from KV.
    KvDel {
        /// Key to delete.
        key: String,
    },
}

/// Legacy job spec kept for compatibility (cmd + period).
#[derive(Serialize, Deserialize, Clone)]
pub struct LegacySpec {
    /// Legacy command (often "noop").
    pub cmd: String,
    /// Period (ms).
    pub period_ms: u64,
}

/// Runtime state for a job.
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct JobState {
    /// Timestamp (ms since epoch) of last run.
    pub last_run_ms: u64,
    /// Successful runs count.
    pub runs: u64,
    /// Consecutive failure count.
    pub failures: u64,
    /// Current backoff (ms), 0 when disabled.
    pub backoff_ms: u64,
}
