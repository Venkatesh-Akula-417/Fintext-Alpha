//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Robust Python Worker IPC & Process Supervisor
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::pipeline::ProcessedDocument;
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tracing::{error, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceRequest {
    pub command: String,
    pub document: ProcessedDocument,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceResponse {
    pub status: String,
    pub id: String,
    pub sentiment_score: f64,
    pub sentiment_label: String,
    pub confidence: f64,
    pub novelty_score: f64,
    pub signal_available_ts_us: i64,
    pub error: Option<String>,
}

struct WorkerProcess {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

pub struct PythonBridge {
    python_path: String,
    script_path: String,
    worker: Arc<Mutex<Option<WorkerProcess>>>,
}

impl PythonBridge {
    pub fn new(python_path: String, script_path: String) -> Self {
        Self {
            python_path,
            script_path,
            worker: Arc::new(Mutex::new(None)),
        }
    }

    /// Ensure Python worker subprocess is running and healthy.
    pub async fn ensure_started(&self) -> Result<(), String> {
        let mut guard = self.worker.lock().await;
        if guard.is_none() {
            let worker = self.spawn_worker().await?;
            *guard = Some(worker);
            info!("External worker subprocess spawned successfully.");
        }
        Ok(())
    }

    async fn spawn_worker(&self) -> Result<WorkerProcess, String> {
        let mut cmd = Command::new(&self.python_path);
        cmd.arg(&self.script_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        let mut child = cmd.spawn().map_err(|e| {
            format!(
                "Failed to spawn worker process ({} {}): {}",
                self.python_path, self.script_path, e
            )
        })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Failed to open stdin for worker process".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Failed to open stdout for worker process".to_string())?;

        Ok(WorkerProcess {
            child,
            stdin: BufWriter::new(stdin),
            stdout: BufReader::new(stdout),
        })
    }

    /// Dispatch document to external worker process.
    pub async fn process_document(
        &self,
        doc: ProcessedDocument,
    ) -> Result<InferenceResponse, String> {
        self.ensure_started().await?;

        let req = InferenceRequest {
            command: "process_and_store".to_string(),
            document: doc,
        };

        let req_json = serde_json::to_string(&req).map_err(|e| e.to_string())?;
        let mut guard = self.worker.lock().await;

        let worker = match guard.as_mut() {
            Some(w) => w,
            None => return Err("Python worker not available".to_string()),
        };

        // Write request line to stdin
        if let Err(e) = worker.stdin.write_all(req_json.as_bytes()).await {
            error!("Failed to write to Python stdin: {}. Restarting worker.", e);
            *guard = None;
            return Err(format!("IPC write error: {}", e));
        }
        if let Err(e) = worker.stdin.write_all(b"\n").await {
            *guard = None;
            return Err(format!("IPC newline error: {}", e));
        }
        if let Err(e) = worker.stdin.flush().await {
            *guard = None;
            return Err(format!("IPC flush error: {}", e));
        }

        // Read response line from stdout
        let mut line = String::new();
        match worker.stdout.read_line(&mut line).await {
            Ok(0) => {
                warn!("Python worker stdout closed (EOF). Process may have exited. Restarting.");
                *guard = None;
                Err("Python worker disconnected".to_string())
            }
            Ok(_) => {
                let resp: InferenceResponse = serde_json::from_str(line.trim()).map_err(|e| {
                    format!(
                        "Failed to parse Python JSON response ('{}'): {}",
                        line.trim(),
                        e
                    )
                })?;
                Ok(resp)
            }
            Err(e) => {
                error!(
                    "Failed to read from Python stdout: {}. Restarting worker.",
                    e
                );
                *guard = None;
                Err(format!("IPC read error: {}", e))
            }
        }
    }

    /// Graceful shutdown of child worker.
    pub async fn shutdown(&self) {
        let mut guard = self.worker.lock().await;
        if let Some(mut worker) = guard.take() {
            info!("Sending shutdown command to Python worker...");
            let shutdown_cmd = r#"{"command":"shutdown"}"#;
            let _ = worker.stdin.write_all(shutdown_cmd.as_bytes()).await;
            let _ = worker.stdin.write_all(b"\n").await;
            let _ = worker.stdin.flush().await;
            let _ = worker.child.kill().await;
        }
    }
}
