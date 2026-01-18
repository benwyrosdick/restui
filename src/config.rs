use anyhow::Result;
use std::path::PathBuf;

/// Application configuration
#[derive(Debug, Clone)]
pub struct Config {
    /// Base directory for all data (XDG config dir)
    pub data_dir: PathBuf,
    /// Collections directory
    pub collections_dir: PathBuf,
    /// History file path
    pub history_file: PathBuf,
    /// Environments file path
    pub environments_file: PathBuf,
}

impl Config {
    pub fn new() -> Result<Self> {
        // Use ~/.config on all platforms for consistency
        let base_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config")
            .join("restui");

        let collections_dir = base_dir.join("collections");
        let history_file = base_dir.join("history.json");
        let environments_file = base_dir.join("environments.json");

        Ok(Self {
            data_dir: base_dir,
            collections_dir,
            history_file,
            environments_file,
        })
    }

    /// Ensure all required directories exist
    pub fn ensure_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        std::fs::create_dir_all(&self.collections_dir)?;
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new().expect("Failed to create default config")
    }
}
