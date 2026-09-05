use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Runtime configuration. Override with `OPENHUB_*` on the GCP VM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub public_origin: String,
    pub data_dir: PathBuf,
    pub cells_dir: PathBuf,
    pub cells_storage_dir: PathBuf,
    pub cells_lease_ttl_secs: u64,
    pub database_url: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: std::env::var("OPENHUB_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: std::env::var("OPENHUB_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            public_origin: std::env::var("OPENHUB_PUBLIC_ORIGIN")
                .unwrap_or_else(|_| "https://openhub.run".to_string()),
            data_dir: std::env::var("OPENHUB_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("./data/repos")),
            cells_dir: std::env::var("OPENHUB_CELLS_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("./data/cells")),
            cells_storage_dir: std::env::var("OPENHUB_CELLS_STORAGE_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("./data/cells-storage")),
            cells_lease_ttl_secs: std::env::var("OPENHUB_CELLS_LEASE_TTL_SECS")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(60),
            database_url: std::env::var("OPENHUB_DATABASE_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("./data/openhub.db")),
        }
    }
}
