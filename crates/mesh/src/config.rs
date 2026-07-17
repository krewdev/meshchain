use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MeshConfig {
    pub default_wallet: String,
    pub default_cold_key: String,
    pub radio_port: Option<String>,
    pub tx_delay_ms: u64,
    pub portnum: u32,
    pub compression: bool,
}

impl Default for MeshConfig {
    fn default() -> Self {
        Self {
            default_wallet: "wallet.json".to_string(),
            default_cold_key: "cold.json".to_string(),
            radio_port: None,
            tx_delay_ms: 150,
            portnum: 265,
            compression: true,
        }
    }
}

impl MeshConfig {
    pub fn config_path(data_dir: &Path) -> PathBuf {
        data_dir.join("config.json")
    }

    pub fn load_or_default(data_dir: &Path) -> Self {
        let path = Self::config_path(data_dir);
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str::<Self>(&content) {
                return cfg;
            }
        }
        Self::default()
    }

    pub fn save(&self, data_dir: &Path) -> Result<PathBuf> {
        fs::create_dir_all(data_dir)?;
        let path = Self::config_path(data_dir);
        let json = serde_json::to_string_pretty(self)?;
        fs::write(&path, json)
            .with_context(|| format!("Failed to write config to {}", path.display()))?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_init_and_roundtrip() {
        let tmp = std::env::temp_dir().join("meshchain_test_cfg_roundtrip");
        let _ = fs::remove_dir_all(&tmp);
        let mut cfg = MeshConfig::default();
        cfg.radio_port = Some("/dev/ttyACM0".to_string());
        cfg.tx_delay_ms = 200;
        let saved_path = cfg.save(&tmp).unwrap();
        assert!(saved_path.exists());

        let loaded = MeshConfig::load_or_default(&tmp);
        assert_eq!(loaded, cfg);
        let _ = fs::remove_dir_all(&tmp);
    }
}
