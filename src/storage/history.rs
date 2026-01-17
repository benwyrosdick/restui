use super::request::ApiRequest;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

/// A history entry for a completed request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub request: ApiRequest,
    pub timestamp: DateTime<Utc>,
    pub status_code: Option<u16>,
    pub duration_ms: u64,
}

impl HistoryEntry {
    pub fn new(request: ApiRequest, status_code: Option<u16>, duration_ms: u64) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            request,
            timestamp: Utc::now(),
            status_code,
            duration_ms,
        }
    }

    /// Format for display in the history list
    pub fn display(&self) -> String {
        let status = self
            .status_code
            .map(|s| s.to_string())
            .unwrap_or_else(|| "ERR".to_string());
        let path = self
            .request
            .url
            .split("://")
            .nth(1)
            .and_then(|s| s.find('/').map(|i| &s[i..]))
            .unwrap_or(&self.request.url);
        format!("{} {} {}", self.request.method, path, status)
    }
}

/// Manager for request history
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HistoryManager {
    pub entries: Vec<HistoryEntry>,
    #[serde(skip)]
    max_entries: usize,
}

impl HistoryManager {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            max_entries: 100,
        }
    }

    /// Load history from a JSON file
    pub fn load(path: &Path) -> Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let mut manager: HistoryManager = serde_json::from_str(&content)?;
            manager.max_entries = 100;
            Ok(manager)
        } else {
            Ok(Self::new())
        }
    }

    /// Save history to a JSON file
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Add a new entry to the history
    pub fn add(&mut self, entry: HistoryEntry) {
        self.entries.insert(0, entry);
        // Keep only the most recent entries
        if self.entries.len() > self.max_entries {
            self.entries.truncate(self.max_entries);
        }
    }

    /// Get recent entries (most recent first)
    pub fn recent(&self, count: usize) -> &[HistoryEntry] {
        let end = count.min(self.entries.len());
        &self.entries[..end]
    }

    /// Clear all history
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}
