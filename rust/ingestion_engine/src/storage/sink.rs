//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — High-Speed Local JSONL Stream Buffer
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::ipc::InferenceResponse;
use crate::pipeline::ProcessedDocument;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteSignalRecord {
    pub document: ProcessedDocument,
    pub inference: InferenceResponse,
    pub stored_at_utc: String,
}

pub struct JsonlStreamSink {
    file_path: PathBuf,
    writer_lock: Mutex<()>,
}

impl JsonlStreamSink {
    pub fn new(path: PathBuf) -> Self {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        Self {
            file_path: path,
            writer_lock: Mutex::new(()),
        }
    }

    pub fn append(&self, record: &CompleteSignalRecord) -> Result<(), String> {
        let _guard = self.writer_lock.lock().map_err(|e| e.to_string())?;
        let json_line = serde_json::to_string(record).map_err(|e| e.to_string())?;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)
            .map_err(|e| format!("Failed to open jsonl sink file {:?}: {}", self.file_path, e))?;

        writeln!(file, "{}", json_line)
            .map_err(|e| format!("Failed to write to jsonl sink file: {}", e))?;

        Ok(())
    }
}
