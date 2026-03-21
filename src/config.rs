use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    pub network: String,
    pub utxo_index: bool,
    pub ram_scale: f64,
    pub app_dir: String,
    pub log_level: String,
    pub async_threads: usize,
    #[serde(default)]
    pub auto_start_daemon: bool,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            network: "mainnet".to_string(),
            utxo_index: false,
            ram_scale: 1.0,
            app_dir: default_app_dir(),
            log_level: "WARN".to_string(),
            async_threads: num_cpus::get(),
            auto_start_daemon: false,
        }
    }
}

#[allow(dead_code)]
impl DaemonConfig {
    pub fn config_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".tui4kas")
            .join("config.toml")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(&path)?;
        let config: Self = toml::from_str(&contents)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(&path, contents)?;
        Ok(())
    }

    pub fn valid_networks() -> &'static [&'static str] {
        &["mainnet", "testnet-10", "testnet-11"]
    }

    pub fn valid_log_levels() -> &'static [&'static str] {
        &["TRACE", "DEBUG", "INFO", "WARN", "ERROR"]
    }

    pub fn cycle_network(&mut self) {
        let networks = Self::valid_networks();
        let idx = networks
            .iter()
            .position(|n| *n == self.network)
            .unwrap_or(0);
        self.network = networks[(idx + 1) % networks.len()].to_string();
    }

    pub fn cycle_log_level(&mut self) {
        let levels = Self::valid_log_levels();
        let idx = levels
            .iter()
            .position(|l| *l == self.log_level)
            .unwrap_or(3);
        self.log_level = levels[(idx + 1) % levels.len()].to_string();
    }
}

fn default_app_dir() -> String {
    dirs::home_dir()
        .map(|h| h.join(".tui4kas").to_string_lossy().into_owned())
        .unwrap_or_else(|| ".tui4kas".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let config = DaemonConfig::default();
        assert_eq!(config.network, "mainnet");
        assert!(!config.utxo_index);
        assert!((config.ram_scale - 1.0).abs() < f64::EPSILON);
        assert_eq!(config.log_level, "WARN");
        assert!(config.async_threads > 0);
    }

    #[test]
    fn serialize_deserialize_roundtrip() {
        let config = DaemonConfig {
            network: "testnet-10".to_string(),
            utxo_index: true,
            ram_scale: 2.5,
            app_dir: "/tmp/test".to_string(),
            log_level: "DEBUG".to_string(),
            async_threads: 4,
            auto_start_daemon: true,
        };
        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: DaemonConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.network, "testnet-10");
        assert!(deserialized.utxo_index);
        assert!((deserialized.ram_scale - 2.5).abs() < f64::EPSILON);
        assert_eq!(deserialized.log_level, "DEBUG");
        assert_eq!(deserialized.async_threads, 4);
    }

    #[test]
    fn cycle_network() {
        let mut config = DaemonConfig::default();
        assert_eq!(config.network, "mainnet");
        config.cycle_network();
        assert_eq!(config.network, "testnet-10");
        config.cycle_network();
        assert_eq!(config.network, "testnet-11");
        config.cycle_network();
        assert_eq!(config.network, "mainnet");
    }

    #[test]
    fn cycle_log_level() {
        let mut config = DaemonConfig::default();
        assert_eq!(config.log_level, "WARN");
        config.cycle_log_level();
        assert_eq!(config.log_level, "ERROR");
        config.cycle_log_level();
        assert_eq!(config.log_level, "TRACE");
    }
}
